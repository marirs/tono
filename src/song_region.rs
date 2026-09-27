//! Song-region selection (spec section 0).
//!
//! The Python worker only *measures*: per short frame it reports loudness and
//! AudioSet speech / music / singing probabilities. Choosing the region is
//! deterministic Rust logic here, so it can be unit-tested and tuned without
//! touching the model.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// One analysis frame from `analyze.py --detect-regions`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RegionFrame {
    pub start: f64,
    pub end: f64,
    /// Frame RMS level in dBFS.
    pub rms_db: f64,
    pub speech: f64,
    pub music: f64,
    pub singing: f64,
}

#[derive(Debug, Deserialize)]
pub struct RegionFramesReport {
    pub version: u32,
    pub duration: f64,
    pub model: String,
    pub frames: Vec<RegionFrame>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameLabel {
    Silence,
    Speech,
    Music,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct LabeledSpan {
    pub start: f64,
    pub end: f64,
    pub label: FrameLabel,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegionMethod {
    AutoSongRegion,
    Manual,
    FullFile,
}

#[derive(Debug, Clone, Serialize)]
pub struct SelectedRegion {
    pub start: f64,
    pub end: f64,
    pub method: RegionMethod,
    /// Only meaningful for auto detection; 1.0 for manual/full-file.
    pub confidence: f64,
}

impl SelectedRegion {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

/// Tunable thresholds. Defaults are a first guess and must be revisited on
/// real reels (M4 feedback), which is why they live in one struct.
#[derive(Debug, Clone)]
pub struct RegionSettings {
    /// Frames quieter than this are silence regardless of classifier output.
    pub silence_floor_db: f64,
    /// Also silence when this far below the loudest frames (95th percentile).
    pub silence_relative_db: f64,
    /// Speech wins when its probability exceeds this and beats music/singing.
    pub speech_threshold: f64,
    /// Music (or singing) probability needed to call a frame musical.
    pub music_threshold: f64,
    /// Non-music gaps up to this long inside a song are bridged, except speech.
    pub max_bridged_gap_seconds: f64,
    /// Speech gaps up to this long are bridged (a sung phrase misread as speech).
    pub max_bridged_speech_seconds: f64,
    pub min_region_seconds: f64,
}

impl Default for RegionSettings {
    fn default() -> Self {
        RegionSettings {
            silence_floor_db: -55.0,
            silence_relative_db: 35.0,
            // PANNs framewise probabilities are modest: on the synthetic
            // fixture, speech scored 0.41-0.67 and music 0.26-0.61.
            speech_threshold: 0.35,
            music_threshold: 0.2,
            max_bridged_gap_seconds: 2.0,
            max_bridged_speech_seconds: 0.75,
            min_region_seconds: 5.0,
        }
    }
}

pub fn label_frames(frames: &[RegionFrame], settings: &RegionSettings) -> Vec<FrameLabel> {
    let loud_reference_db = percentile(frames.iter().map(|frame| frame.rms_db).collect(), 0.95);
    let silence_db = settings
        .silence_floor_db
        .max(loud_reference_db - settings.silence_relative_db);

    let raw_labels: Vec<FrameLabel> = frames
        .iter()
        .map(|frame| {
            let musical = frame.music.max(frame.singing);
            if frame.rms_db < silence_db {
                FrameLabel::Silence
            } else if frame.speech >= settings.speech_threshold && frame.speech > musical {
                // Talking, possibly over quiet background music: never melody.
                FrameLabel::Speech
            } else if musical >= settings.music_threshold {
                FrameLabel::Music
            } else {
                FrameLabel::Other
            }
        })
        .collect();
    median_smooth_labels(&raw_labels)
}

/// 3-frame majority vote removes single-frame flicker.
fn median_smooth_labels(labels: &[FrameLabel]) -> Vec<FrameLabel> {
    (0..labels.len())
        .map(|index| {
            if index == 0 || index + 1 == labels.len() {
                return labels[index];
            }
            let (before, current, after) = (labels[index - 1], labels[index], labels[index + 1]);
            if before == after && before != current {
                before
            } else {
                current
            }
        })
        .collect()
}

fn percentile(mut values: Vec<f64>, fraction: f64) -> f64 {
    if values.is_empty() {
        return f64::NEG_INFINITY;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let index = ((values.len() - 1) as f64 * fraction).round() as usize;
    values[index]
}

pub fn spans_from_labels(frames: &[RegionFrame], labels: &[FrameLabel]) -> Vec<LabeledSpan> {
    let mut spans: Vec<LabeledSpan> = Vec::new();
    for (frame, &label) in frames.iter().zip(labels) {
        match spans.last_mut() {
            Some(span) if span.label == label => span.end = frame.end,
            _ => spans.push(LabeledSpan { start: frame.start, end: frame.end, label }),
        }
    }
    spans
}

#[derive(Debug, Clone)]
struct Candidate {
    start: f64,
    end: f64,
    first_frame: usize,
    last_frame: usize,
}

/// Picks the main musical section: music spans joined across short gaps,
/// scored by duration x confidence (spec: prefer longest/highest-confidence).
pub fn select_song_region(
    report: &RegionFramesReport,
    settings: &RegionSettings,
) -> Result<(SelectedRegion, Vec<LabeledSpan>)> {
    if report.version != 1 {
        bail!("unsupported region report version {}", report.version);
    }
    let frames = &report.frames;
    let labels = label_frames(frames, settings);
    let spans = spans_from_labels(frames, &labels);

    let candidates = build_candidates(frames, &labels, settings);
    let best = candidates
        .iter()
        .map(|candidate| (candidate, region_confidence(frames, &labels, candidate)))
        .filter(|(candidate, _)| candidate.end - candidate.start >= settings.min_region_seconds)
        .max_by(|(a, a_conf), (b, b_conf)| {
            let a_score = (a.end - a.start) * a_conf;
            let b_score = (b.end - b.start) * b_conf;
            a_score.total_cmp(&b_score)
        })
        .with_context(|| {
            format!(
                "no musical region of at least {:.0}s found; pass --from/--to to choose one manually",
                settings.min_region_seconds
            )
        })?;

    let (candidate, confidence) = best;
    Ok((
        SelectedRegion {
            start: candidate.start,
            end: candidate.end.min(report.duration),
            method: RegionMethod::AutoSongRegion,
            confidence: (confidence * 1000.0).round() / 1000.0,
        },
        spans,
    ))
}

fn build_candidates(frames: &[RegionFrame], labels: &[FrameLabel], settings: &RegionSettings) -> Vec<Candidate> {
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut open: Option<Candidate> = None;
    let mut gap_seconds = 0.0;
    let mut gap_speech_seconds = 0.0;

    for (index, (frame, &label)) in frames.iter().zip(labels).enumerate() {
        if label == FrameLabel::Music {
            match open.as_mut() {
                Some(candidate) => {
                    candidate.end = frame.end;
                    candidate.last_frame = index;
                }
                None => {
                    open = Some(Candidate { start: frame.start, end: frame.end, first_frame: index, last_frame: index })
                }
            }
            gap_seconds = 0.0;
            gap_speech_seconds = 0.0;
            continue;
        }
        if open.is_none() {
            continue;
        }
        let frame_seconds = frame.end - frame.start;
        gap_seconds += frame_seconds;
        if label == FrameLabel::Speech {
            gap_speech_seconds += frame_seconds;
        }
        let gap_too_long = gap_seconds > settings.max_bridged_gap_seconds
            || gap_speech_seconds > settings.max_bridged_speech_seconds;
        if gap_too_long {
            // Candidate ends at its last music frame, not inside the gap.
            candidates.extend(open.take());
            gap_seconds = 0.0;
            gap_speech_seconds = 0.0;
        }
    }
    candidates.extend(open);
    candidates
}

/// Mean musical probability over the region, weighted by the share of
/// frames actually labeled music (bridged gaps lower confidence).
fn region_confidence(frames: &[RegionFrame], labels: &[FrameLabel], candidate: &Candidate) -> f64 {
    let range = candidate.first_frame..=candidate.last_frame;
    let frame_count = range.clone().count() as f64;
    let mean_musical = frames[range.clone()]
        .iter()
        .map(|frame| frame.music.max(frame.singing))
        .sum::<f64>()
        / frame_count;
    let music_share = labels[range].iter().filter(|&&label| label == FrameLabel::Music).count() as f64 / frame_count;
    (mean_musical * music_share).clamp(0.0, 1.0)
}

/// Manual `--from/--to` (either may be absent) always wins over detection.
pub fn manual_region(start: f64, end: Option<f64>, source_duration: f64) -> Result<SelectedRegion> {
    if start >= source_duration {
        bail!("--from ({start:.2}s) is beyond the end of the source ({source_duration:.2}s)");
    }
    let end = end.unwrap_or(source_duration).min(source_duration);
    Ok(SelectedRegion { start, end, method: RegionMethod::Manual, confidence: 1.0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP: f64 = 0.25;

    /// Builds frames from a compact script: (seconds, kind) where kind is
    /// 's' speech, 'm' music, '.' silence, 'o' other.
    fn frames_from_script(script: &[(f64, char)]) -> Vec<RegionFrame> {
        let mut frames = Vec::new();
        let mut time = 0.0;
        for &(seconds, kind) in script {
            let frame_count = (seconds / HOP).round() as usize;
            for _ in 0..frame_count {
                let (rms_db, speech, music, singing) = match kind {
                    's' => (-20.0, 0.9, 0.1, 0.05),
                    'm' => (-12.0, 0.2, 0.9, 0.6),
                    '.' => (-80.0, 0.0, 0.0, 0.0),
                    _ => (-30.0, 0.1, 0.1, 0.0),
                };
                frames.push(RegionFrame { start: time, end: time + HOP, rms_db, speech, music, singing });
                time += HOP;
            }
        }
        frames
    }

    fn report(script: &[(f64, char)]) -> RegionFramesReport {
        let frames = frames_from_script(script);
        let duration = frames.last().map_or(0.0, |frame| frame.end);
        RegionFramesReport { version: 1, duration, model: "test".into(), frames }
    }

    fn select(script: &[(f64, char)]) -> Result<SelectedRegion> {
        select_song_region(&report(script), &RegionSettings::default()).map(|(region, _)| region)
    }

    #[test]
    fn excludes_speech_intro_and_outro() {
        let region = select(&[(4.0, 's'), (1.0, '.'), (30.0, 'm'), (3.0, 's')]).unwrap();
        assert_eq!((region.start, region.end), (5.0, 35.0));
        assert_eq!(region.method, RegionMethod::AutoSongRegion);
        assert!(region.confidence > 0.8);
    }

    #[test]
    fn bridges_short_silence_but_not_long_speech() {
        // 1.5 s pause inside the song is bridged.
        let region = select(&[(10.0, 'm'), (1.5, '.'), (10.0, 'm')]).unwrap();
        assert_eq!((region.start, region.end), (0.0, 21.5));
        // 3 s of talking splits the song; the longer part wins.
        let region = select(&[(10.0, 'm'), (3.0, 's'), (20.0, 'm')]).unwrap();
        assert_eq!((region.start, region.end), (13.0, 33.0));
    }

    #[test]
    fn single_flicker_frame_does_not_split() {
        let region = select(&[(10.0, 'm'), (0.25, 's'), (10.0, 'm')]).unwrap();
        assert_eq!((region.start, region.end), (0.0, 20.25));
    }

    #[test]
    fn fails_loudly_when_no_music() {
        let error = select(&[(20.0, 's'), (5.0, '.')]).unwrap_err().to_string();
        assert!(error.contains("--from/--to"), "{error}");
        assert!(select(&[(3.0, 'm'), (10.0, 's')]).is_err(), "3 s is below the minimum");
    }

    #[test]
    fn spans_summarize_labels() {
        let report = report(&[(2.0, 's'), (5.0, 'm')]);
        let (_, spans) = select_song_region(&report, &RegionSettings::default()).unwrap();
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].label, FrameLabel::Speech);
        assert_eq!((spans[1].start, spans[1].end), (2.0, 7.0));
    }

    #[test]
    fn manual_region_clamps_and_validates() {
        let region = manual_region(12.0, Some(48.0), 60.0).unwrap();
        assert_eq!((region.start, region.end, region.method), (12.0, 48.0, RegionMethod::Manual));
        assert_eq!(manual_region(12.0, None, 60.0).unwrap().end, 60.0);
        assert_eq!(manual_region(0.0, Some(90.0), 60.0).unwrap().end, 60.0);
        assert!(manual_region(70.0, None, 60.0).is_err());
    }
}

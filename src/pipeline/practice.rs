//! M3: cleaned melody + BGM -> fingering timeline -> validated practice.mp4.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::analysis::worker::Analysis;
use crate::instruments::diagram::UpperHand;
use crate::instruments::fingering::{map_notes_to_fingerings, FingeringTable};
use crate::media::audio::{
    count_in_samples, measure_filter_offset_seconds, synthesize_metronome_samples, write_mono_wav,
    GUIDE_SAMPLE_RATE,
};
use crate::media::validation::ValidatedMedia;
use crate::music::notes::{validate_monophonic_sequence, NoteEvent};
use crate::music::range::{fit_melody_to_table, FittedMelody, RangePolicy};
use crate::music::timeline::{BeatTrack, CountIn, PracticeTimeline};
use crate::render::{render_practice_video, AudioBed, MediaTools, MetronomeMode, RenderSettings};

/// atempo handles this range in one filter; wider scales would need
/// chained filters and are not useful for practice.
pub const TEMPO_SCALE_RANGE: std::ops::RangeInclusive<f64> = 0.5..=2.0;

pub struct PracticeRequest<'a> {
    pub title: &'a str,
    pub instrument: crate::instruments::Instrument,
    pub fingering_mode: Option<crate::instruments::brisa::FingeringMode>,
    pub cleaned_notes: &'a [NoteEvent],
    pub analysis: &'a Analysis,
    pub region_duration_seconds: f64,
    pub tempo_scale: f64,
    pub count_in_beats: u8,
    pub upper_hand: UpperHand,
    pub metronome: MetronomeMode,
    pub range_policy: RangePolicy,
    pub low_confidence: bool,
    pub out_dir: &'a Path,
    pub output_video: Option<&'a Path>,
    pub work_dir: &'a Path,
    pub ffmpeg: &'a Path,
    pub ffprobe: &'a Path,
}

pub struct PracticeOutcome {
    pub instrument: crate::instruments::Instrument,
    pub fitted: FittedMelody,
    pub table_verified: bool,
    pub validated: ValidatedMedia,
    pub metronome: MetronomeMode,
    /// Median of the detected beat intervals (display only).
    pub practice_bpm: Option<f64>,
    pub beat_count: usize,
    pub practice_duration_seconds: f64,
    pub count_in: Option<CountIn>,
    /// Measured time-stretch offset compensated in the backing (s, + = was early).
    pub bed_offset_seconds: Option<f64>,
    pub warnings: Vec<String>,
}

pub fn build_practice_video(request: &PracticeRequest) -> Result<PracticeOutcome> {
    if !TEMPO_SCALE_RANGE.contains(&request.tempo_scale) {
        bail!("--tempo-scale must be between 0.5 and 2.0");
    }
    let mut warnings = Vec::new();

    let table = FingeringTable::load_for_mode(request.instrument, request.fingering_mode)?;
    println!(
        "· {} setup: {}",
        request.instrument.name(),
        table.required_settings
    );
    let fitted = fit_melody_to_table(request.cleaned_notes, &table, request.range_policy)?;
    let practice_notes = scale_note_times(&fitted.notes, request.tempo_scale);
    validate_monophonic_sequence(&practice_notes)?;
    let entries = map_notes_to_fingerings(&practice_notes, &table)?;

    let practice_duration = request.region_duration_seconds / request.tempo_scale;
    let beats = BeatTrack::detected(
        &request.analysis.beat_times,
        request.tempo_scale,
        practice_duration,
    );
    let metronome = effective_metronome(request.metronome, &beats, &mut warnings);
    let practice_bpm = beats.median_bpm();
    let beat_count = beats.times.len();
    let mut timeline = PracticeTimeline::with_beats(entries, beats, practice_duration);
    if request.count_in_beats > 0 {
        let bpm = practice_bpm.unwrap_or_else(|| {
            warnings.push("no detected tempo: preparation count uses 100 bpm".to_owned());
            100.0
        });
        timeline.prepend_count_in(CountIn::new(request.count_in_beats, bpm));
    }
    let count_in_seconds = timeline.count_in.map_or(0.0, CountIn::duration);
    write_fingering_json(
        request,
        &table,
        &fitted,
        &timeline.entries,
        count_in_seconds,
    )?;

    let (bed, bed_offset_seconds) = practice_audio_bed(
        request,
        count_in_seconds,
        fitted.octave_shift,
        &mut warnings,
    )?;
    let click_track = if metronome.plays_click() || timeline.count_in.is_some() {
        let (mut cues, warning) = count_in_samples(&timeline, request.ffmpeg, request.work_dir);
        if let Some(warning) = warning {
            warnings.push(warning);
        }
        if timeline.count_in.is_some() {
            write_mono_wav(
                &request.work_dir.join("count-in.wav"),
                &cues,
                GUIDE_SAMPLE_RATE,
            )?;
        }
        if metronome.plays_click() {
            let clicks = synthesize_metronome_samples(&timeline, GUIDE_SAMPLE_RATE);
            write_mono_wav(
                &request.work_dir.join("metronome.wav"),
                &clicks,
                GUIDE_SAMPLE_RATE,
            )?;
            for (sample, click) in cues.iter_mut().zip(clicks) {
                *sample += click;
            }
        }
        let path = request.work_dir.join("practice_cues.wav");
        write_mono_wav(&path, &cues, GUIDE_SAMPLE_RATE)?;
        Some(path)
    } else {
        None
    };

    let settings = RenderSettings {
        title: Some(request.title.to_owned()),
        instrument: request.instrument,
        layout_keys: table.layout_keys.clone(),
        required_settings: table.required_settings.clone(),
        upper_hand: request.upper_hand,
        fingering_is_unverified: !table.verified,
        metronome,
        subtitle: subtitle(practice_bpm, request.tempo_scale).replacen(
            "AE-01",
            request.instrument.name(),
            1,
        ),
        video_warnings: video_warnings(&fitted, request.low_confidence),
        ..RenderSettings::default()
    };
    let ffmpeg_log = request.work_dir.join("ffmpeg-render.log");
    let default_video = request.out_dir.join("practice.mp4");
    let validated = render_practice_video(
        &timeline,
        &settings,
        &AudioBed {
            backing: &bed,
            metronome_click: click_track.as_deref(),
        },
        &MediaTools {
            ffmpeg: request.ffmpeg,
            ffprobe: request.ffprobe,
            ffmpeg_log: &ffmpeg_log,
        },
        request.output_video.unwrap_or(&default_video),
    )?;

    Ok(PracticeOutcome {
        instrument: request.instrument,
        fitted,
        table_verified: table.verified,
        validated,
        metronome,
        practice_bpm,
        beat_count,
        practice_duration_seconds: timeline.total_duration_seconds,
        count_in: timeline.count_in,
        bed_offset_seconds,
        warnings,
    })
}

/// Tempo scale 0.75 = 75 % speed: every time is divided by the scale.
fn scale_note_times(notes: &[NoteEvent], tempo_scale: f64) -> Vec<NoteEvent> {
    notes
        .iter()
        .map(|note| NoteEvent {
            start: note.start / tempo_scale,
            end: note.end / tempo_scale,
            ..*note
        })
        .collect()
}

/// Clicks and flashes use the detected beat timestamps themselves. With
/// too few beats a metronome would be invented, so it is switched off.
const MIN_BEATS_FOR_METRONOME: usize = 4;

fn effective_metronome(
    requested: MetronomeMode,
    beats: &BeatTrack,
    warnings: &mut Vec<String>,
) -> MetronomeMode {
    if requested == MetronomeMode::Off || beats.times.len() >= MIN_BEATS_FOR_METRONOME {
        return requested;
    }
    warnings.push("no steady beat detected: metronome disabled".to_owned());
    MetronomeMode::Off
}

/// The BGM exactly as prepared, or a pitch-preserving tempo-scaled copy.
fn practice_audio_bed(
    request: &PracticeRequest,
    count_in_seconds: f64,
    octave_shift: i32,
    warnings: &mut Vec<String>,
) -> Result<(PathBuf, Option<f64>)> {
    let backing = request.out_dir.join("backing.wav");
    if request.tempo_scale == 1.0 && count_in_seconds == 0.0 && octave_shift == 0 {
        return Ok((backing, None));
    }
    let scaled = request.work_dir.join("practice_bed.wav");
    let mut filters = Vec::new();
    if octave_shift != 0 {
        // Resample pitch, then restore duration with bounded atempo stages.
        let ratio = 2.0_f64.powi(octave_shift);
        filters.push(format!(
            "asetrate={},aresample={}",
            (GUIDE_SAMPLE_RATE as f64 * ratio).round() as u32,
            GUIDE_SAMPLE_RATE
        ));
        let mut tempo = request.tempo_scale / ratio;
        while tempo < 0.5 {
            filters.push("atempo=0.5".to_owned());
            tempo /= 0.5;
        }
        while tempo > 2.0 {
            filters.push("atempo=2".to_owned());
            tempo /= 2.0;
        }
        if (tempo - 1.0).abs() > 1e-9 {
            filters.push(format!("atempo={tempo}"));
        }
    } else if request.tempo_scale != 1.0 {
        filters.push(format!("atempo={}", request.tempo_scale));
    }
    // Stretch/pitch stages shift events slightly; measure and undo that so
    // the slowed backing stays on the picture (see media::audio).
    let offset_seconds = if filters.is_empty() {
        None
    } else {
        let stretch = filters.join(",");
        match measure_filter_offset_seconds(
            request.ffmpeg,
            &stretch,
            request.tempo_scale,
            request.work_dir,
        ) {
            Ok(offset) => Some(offset),
            Err(error) => {
                warnings.push(format!(
                    "backing time-stretch not calibrated ({error:#}); it may be a few ms off"
                ));
                None
            }
        }
    };
    let rate = GUIDE_SAMPLE_RATE as f64;
    let early = offset_seconds.unwrap_or(0.0);
    if early < 0.0 {
        // Output late: drop the extra lead-in so events land on time.
        filters.push(format!(
            "atrim=start_sample={},asetpts=PTS-STARTPTS",
            (-early * rate).round() as u64
        ));
    }
    let samples = (request.region_duration_seconds / request.tempo_scale * rate).round() as u64;
    filters.push(format!("apad,atrim=end_sample={samples}"));
    // Early output is delayed by the same amount, on top of the count-in.
    let delay_samples = ((count_in_seconds + early.max(0.0)) * rate).round() as u64;
    filters.push(format!("adelay={delay_samples}S:all=1"));
    let filter = filters.join(",");
    let output = Command::new(request.ffmpeg)
        .args(["-hide_banner", "-nostdin", "-y", "-v", "error", "-i"])
        .arg(&backing)
        .args(["-af", &filter, "-c:a", "pcm_s24le"])
        .arg(&scaled)
        .output()
        .context("running ffmpeg atempo")?;
    if !output.status.success() {
        bail!(
            "ffmpeg could not tempo-scale the BGM: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok((scaled, offset_seconds))
}

fn subtitle(practice_bpm: Option<f64>, tempo_scale: f64) -> String {
    let mut parts = vec!["AE-01".to_owned()];
    if let Some(bpm) = practice_bpm {
        parts.push(format!("~{bpm:.0} bpm"));
    }
    if tempo_scale != 1.0 {
        parts.push(format!("{:.0}% speed", tempo_scale * 100.0));
    }
    parts.join("  ·  ")
}

/// Conditions the player must see on screen, not just in the terminal.
fn video_warnings(fitted: &FittedMelody, low_confidence: bool) -> Vec<String> {
    let mut lines = Vec::new();
    if fitted.octave_shift != 0 {
        lines.push(format!(
            "MELODY AND BACKING TRANSPOSED {:+} SEMITONES",
            fitted.octave_shift * 12
        ));
    }
    if low_confidence {
        lines.push("LOW-CONFIDENCE TRANSCRIPTION".to_owned());
    }
    lines
}

fn write_fingering_json(
    request: &PracticeRequest,
    table: &FingeringTable,
    fitted: &FittedMelody,
    entries: &[crate::instruments::fingering::FingeringTimelineEntry],
    count_in_seconds: f64,
) -> Result<()> {
    let document = json!({
        "version": 1,
        "instrument": table.instrument,
            "fingering_mode": request.fingering_mode,
        "verified": table.verified,
        "required_settings": table.required_settings,
        "sources": table.sources,
        // Video time = region time / tempo_scale + count_in_seconds.
        "timebase": "practice_video",
        "tempo_scale": request.tempo_scale,
        "count_in_seconds": count_in_seconds,
        "upper_hand": request.upper_hand,
        "range_fit": fitted,
        "backing_transpose_semitones": fitted.transpose_semitones(),
        "fingering_alternatives": table.alternatives,
        "timeline": entries,
    });
    let path = request.out_dir.join("fingering.json");
    std::fs::write(&path, serde_json::to_string_pretty(&document)? + "\n")
        .with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slower_tempo_stretches_note_times() {
        let notes = [NoteEvent {
            start: 1.5,
            end: 3.0,
            midi: 60,
            confidence: 0.9,
        }];
        let scaled = scale_note_times(&notes, 0.75);
        assert_eq!((scaled[0].start, scaled[0].end), (2.0, 4.0));
    }

    #[test]
    fn subtitle_shows_tempo() {
        let text = subtitle(Some(90.0), 0.75);
        assert!(text.contains("90 bpm") && text.contains("75% speed"));
        assert_eq!(subtitle(None, 1.0), "AE-01");
    }

    fn fitted_with_shift(octave_shift: i32) -> FittedMelody {
        FittedMelody {
            notes: vec![],
            policy: RangePolicy::Strict,
            octave_shift,
        }
    }

    #[test]
    fn transposition_and_low_confidence_are_shown_in_the_video() {
        let lines = video_warnings(&fitted_with_shift(1), true);
        assert!(lines[0].contains("TRANSPOSED +12 SEMITONES"));
        assert!(lines[1].contains("LOW-CONFIDENCE"));
        assert!(video_warnings(&fitted_with_shift(0), false).is_empty());
    }

    #[test]
    fn metronome_is_disabled_without_detected_beats() {
        let mut warnings = Vec::new();
        let none = BeatTrack::detected(&[], 1.0, 10.0);
        assert_eq!(
            effective_metronome(MetronomeMode::Both, &none, &mut warnings),
            MetronomeMode::Off
        );
        assert_eq!(warnings.len(), 1);
        let some = BeatTrack::detected(&[0.5, 1.0, 1.5, 2.0], 1.0, 10.0);
        assert_eq!(
            effective_metronome(MetronomeMode::Audio, &some, &mut warnings),
            MetronomeMode::Audio
        );
    }
}

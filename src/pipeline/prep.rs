//! `tono prep` (M2 + M3): source -> song region -> stems/BGM ->
//! transcription -> cleaned notes -> AE-01 fingerings -> validated
//! practice.mp4 + project.json.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::analysis::cleanup::{clean_notes, CleanupSettings};
use crate::analysis::region::{
    manual_region, select_song_region, FrameLabel, LabeledSpan, RegionMethod, RegionSettings,
    SelectedRegion,
};
use crate::analysis::worker::{self as ml_worker, Analysis, WorkerPaths};
use crate::instruments::diagram::UpperHand;
use crate::media::source::{normalize_source, trim_region, SampleRange, ANALYSIS_SAMPLE_RATE};
use crate::media::validation::probe_audio_duration_seconds;
use crate::music::notes::validate_monophonic_sequence;
use crate::music::range::RangePolicy;
use crate::music::timecode::parse_clip_window;
use crate::paths;
use crate::pipeline::practice::{build_practice_video, PracticeOutcome, PracticeRequest};
use crate::render::MetronomeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Part {
    Vocal,
    Lead,
}

impl Part {
    fn worker_name(self) -> &'static str {
        match self {
            Part::Vocal => "vocal",
            Part::Lead => "lead",
        }
    }
}

#[derive(Clone)]
pub struct PrepOptions {
    pub instrument: crate::instruments::Instrument,
    pub input: PathBuf,
    pub from_timecode: Option<String>,
    pub to_timecode: Option<String>,
    pub part: Part,
    pub auto_song_region: bool,
    pub output_directory: PathBuf,
    /// Explicit MP4 destination for the short CLI form.
    pub output_video: Option<PathBuf>,
    pub keep_work: bool,
    pub min_note_confidence: f64,
    /// Recorded for M3 rendering; M2 does not use them.
    pub tempo_scale: f64,
    pub count_in_beats: u8,
    pub upper_hand: UpperHand,
    pub metronome: MetronomeMode,
    pub range_policy: RangePolicy,
}

/// Region choice plus the labeled timeline it was based on (auto only).
struct RegionDecision {
    region: SelectedRegion,
    labeled_spans: Vec<LabeledSpan>,
    detector_model: Option<String>,
}

pub fn run_prep(options: &PrepOptions) -> Result<()> {
    run_prep_inner(options, true)
}

pub(crate) fn run_prep_inner(options: &PrepOptions, show_paths: bool) -> Result<()> {
    crate::instruments::fingering::FingeringTable::load_instrument(options.instrument)?;
    if !options.input.is_file() {
        bail!("input not found: {}", options.input.display());
    }
    // Validated before any ML work so a typo fails in milliseconds.
    if !crate::pipeline::practice::TEMPO_SCALE_RANGE.contains(&options.tempo_scale) {
        bail!("--tempo-scale must be between 0.5 and 2.0");
    }
    let ffmpeg = paths::ffmpeg_executable().context("ffmpeg not found; run `tono doctor`")?;
    let ffprobe = paths::ffprobe_executable().context("ffprobe not found; run `tono doctor`")?;

    let out = &options.output_directory;
    let work = out.join("work");
    std::fs::create_dir_all(&work).with_context(|| format!("creating {}", work.display()))?;

    let result = prepare_project(options, &ffmpeg, &ffprobe, &work);
    match &result {
        // Spec: work/ only with --keep-work; kept on failure for debugging.
        Ok(()) if !options.keep_work => std::fs::remove_dir_all(&work)
            .with_context(|| format!("removing {}", work.display()))?,
        Ok(()) => {}
        Err(_) => eprintln!("work files kept for debugging: {}", work.display()),
    }
    result?;
    if show_paths {
        print_next_steps(out, options.output_video.as_deref(), options.keep_work);
    }
    Ok(())
}

fn prepare_project(
    options: &PrepOptions,
    ffmpeg: &Path,
    ffprobe: &Path,
    work: &Path,
) -> Result<()> {
    let out = &options.output_directory;

    let source_wav = work.join("source.wav");
    normalize_source(ffmpeg, &options.input, &source_wav)?;
    let source_duration = probe_audio_duration_seconds(ffprobe, &source_wav)?;
    println!("✓ source normalized ({source_duration:.2}s, 44.1 kHz stereo)");

    let decision = choose_region(options, &source_wav, source_duration, work)?;
    report_region(&decision);

    let range = SampleRange::from_seconds(decision.region.start, decision.region.end);
    let region_wav = work.join("region.wav");
    trim_region(ffmpeg, &source_wav, range, &region_wav)?;

    let analysis = ml_worker::analyze_region(
        &region_wav,
        options.part.worker_name(),
        &WorkerPaths {
            work_dir: work,
            out_dir: out,
        },
        range.duration_seconds(),
    )?;
    let separation_quality = assess_separation(&analysis)?;
    report_separation(&analysis, &separation_quality);

    if options.keep_work {
        // Written before cleanup so a cleanup failure can be reproduced.
        write_json(
            &work.join("notes_raw.json"),
            &notes_document(&analysis.notes, range, "basic-pitch raw"),
        )?;
    }
    let cleanup_settings = CleanupSettings {
        min_confidence: options.min_note_confidence,
        ..CleanupSettings::default()
    };
    let cleaned = clean_notes(&analysis.notes, &cleanup_settings);
    validate_monophonic_sequence(&cleaned).context("cleanup produced an invalid sequence")?;
    println!(
        "✓ melody transcribed ({} raw note events)",
        analysis.notes.len()
    );
    if cleaned.is_empty() {
        bail!("no melody notes survived cleanup; the lead may be missing or the region wrong (try --from/--to)");
    }
    println!("✓ melody cleaned ({} notes)", cleaned.len());
    // notes.json keeps the transcribed pitches; fingering.json holds what is played.
    write_json(
        &out.join("notes.json"),
        &notes_document(&cleaned, range, "cleaned"),
    )?;

    let practice = build_practice_video(&PracticeRequest {
        instrument: options.instrument,
        cleaned_notes: &cleaned,
        analysis: &analysis,
        region_duration_seconds: range.duration_seconds(),
        tempo_scale: options.tempo_scale,
        count_in_beats: options.count_in_beats,
        upper_hand: options.upper_hand,
        metronome: options.metronome,
        range_policy: options.range_policy,
        low_confidence: separation_quality.low_confidence,
        out_dir: out,
        output_video: options.output_video.as_deref(),
        work_dir: work,
        ffmpeg,
        ffprobe,
    })?;
    report_practice(&practice);

    let summary = ProjectSummary {
        options,
        source_duration,
        range,
        decision: &decision,
        analysis: &analysis,
        separation_quality: &separation_quality,
        cleanup_settings: &cleanup_settings,
        clean_note_count: cleaned.len(),
        practice: &practice,
    };
    write_json(&out.join("project.json"), &project_document(&summary))?;
    Ok(())
}

fn report_practice(practice: &PracticeOutcome) {
    let fitted = &practice.fitted;
    let mut adjustments = Vec::new();
    if fitted.octave_shift != 0 {
        adjustments.push(format!(
            "melody and backing shifted {:+} octave(s) into range",
            fitted.octave_shift
        ));
    }
    if fitted.shape_changed() {
        adjustments.push(format!(
            "{} note(s) octave-folded",
            fitted.folded_notes.len()
        ));
    }
    let detail = if fitted.is_unchanged() {
        "all notes in charted range".to_owned()
    } else {
        adjustments.join(", ")
    };
    println!(
        "✓ {} fingerings mapped ({} notes; {detail})",
        practice.instrument.name(),
        fitted.notes.len()
    );
    if fitted.shape_changed() {
        println!(
            "! --range-policy fold moved {} note(s) by octaves: the melody's shape differs from the song (warning shown in the video)",
            fitted.folded_notes.len()
        );
    }
    if !practice.table_verified {
        println!(
            "! fingering table not yet checked on a physical {} (video shows a caution banner)",
            practice.instrument.name()
        );
    }
    for warning in &practice.warnings {
        println!("! {warning}");
    }
    let media = &practice.validated;
    println!(
        "✓ practice video rendered with BGM ({:.2}s, metronome {:?})",
        practice.practice_duration_seconds, practice.metronome
    );
    println!(
        "✓ final audio/video sync validated ({} {:.3}s / {} {:.3}s, within 100 ms)",
        media.video_codec,
        media.video_duration_seconds,
        media.audio_codec,
        media.audio_duration_seconds
    );
}

fn choose_region(
    options: &PrepOptions,
    source_wav: &Path,
    source_duration: f64,
    work: &Path,
) -> Result<RegionDecision> {
    let has_manual_bound = options.from_timecode.is_some() || options.to_timecode.is_some();
    if has_manual_bound {
        // Spec rule 18: manual boundaries always override auto detection.
        let (start, end) = parse_clip_window(
            options.from_timecode.as_deref(),
            options.to_timecode.as_deref(),
        )?;
        return Ok(RegionDecision {
            region: manual_region(start, end, source_duration)?,
            labeled_spans: Vec::new(),
            detector_model: None,
        });
    }
    if !options.auto_song_region {
        return Ok(RegionDecision {
            region: SelectedRegion {
                start: 0.0,
                end: source_duration,
                method: RegionMethod::FullFile,
                confidence: 1.0,
            },
            labeled_spans: Vec::new(),
            detector_model: None,
        });
    }
    let report = ml_worker::detect_region_frames(source_wav, work)?;
    let (region, labeled_spans) = select_song_region(&report, &RegionSettings::default())?;
    Ok(RegionDecision {
        region,
        labeled_spans,
        detector_model: Some(report.model),
    })
}

/// Spans outside the region, in source time; these are what we excluded.
fn excluded_spans(decision: &RegionDecision) -> Vec<&LabeledSpan> {
    decision
        .labeled_spans
        .iter()
        .filter(|span| span.end <= decision.region.start || span.start >= decision.region.end)
        .collect()
}

fn excluded_speech_seconds(decision: &RegionDecision) -> (f64, f64) {
    let speech_length = |before: bool| {
        excluded_spans(decision)
            .into_iter()
            .filter(|span| {
                span.label == FrameLabel::Speech && (span.end <= decision.region.start) == before
            })
            .map(|span| span.end - span.start)
            .sum::<f64>()
    };
    (speech_length(true), speech_length(false))
}

fn report_region(decision: &RegionDecision) {
    let region = &decision.region;
    match region.method {
        RegionMethod::Manual => println!(
            "✓ manual region {:.2}s - {:.2}s (auto detection skipped)",
            region.start, region.end
        ),
        RegionMethod::FullFile => {
            println!("✓ full file used ({:.2}s, auto detection off)", region.end)
        }
        RegionMethod::AutoSongRegion => {
            println!(
                "✓ song region detected: {:.2}s - {:.2}s ({:.1}s, confidence {:.2})",
                region.start,
                region.end,
                region.duration(),
                region.confidence
            );
            let (intro_speech, outro_speech) = excluded_speech_seconds(decision);
            if intro_speech + outro_speech > 0.0 {
                println!("✓ speech-only intro/outro excluded (intro {intro_speech:.1}s, outro {outro_speech:.1}s)");
            } else {
                println!("· no speech-only intro/outro detected");
            }
        }
    }
}

/// Stem levels relative to the input mix. They establish only that a stem
/// has content; they do not measure separation quality, which Tono cannot
/// measure without a reference. Hence "low confidence" is also set whenever
/// a choice was assumed rather than made by the model.
///
/// Lead below MISSING: nothing to transcribe but bleed -> fail.
/// BGM below MISSING: unaccompanied vocal or failed separation; a usable BGM
/// is mandatory (spec "Stem separation") -> fail.
const LEAD_MISSING_BELOW_MIX_DB: f64 = -35.0;
const LEAD_WEAK_BELOW_MIX_DB: f64 = -25.0;
/// BGM thresholds from measurements (provisional until real songs, M4):
/// accompanied clips measured -2.7 and -7.0 dB; an unaccompanied voice left
/// Demucs residue at -21.5 dB. -18 rejects residue with margin while
/// leaving room for sparse real accompaniment (e.g. voice + soft guitar).
const BACKING_MISSING_BELOW_MIX_DB: f64 = -18.0;
const BACKING_WEAK_BELOW_MIX_DB: f64 = -12.0;

/// Worker's `lead_selection` when --part lead took the `other` stem on trust.
const ASSUMED_LEAD_SELECTION: &str = "assumed-other-stem";

struct SeparationQuality {
    low_confidence: bool,
    warnings: Vec<String>,
}

fn assess_separation(analysis: &Analysis) -> Result<SeparationQuality> {
    let separation = &analysis.separation;
    let stem = &separation.lead_stem;
    if separation.lead_to_mix_db < LEAD_MISSING_BELOW_MIX_DB {
        bail!(
            "the separated `{stem}` stem is {:.1} dB below the mix: no {stem} found in this region. \
             Try --part lead (instrumental melody) or choose the sung part with --from/--to",
            separation.lead_to_mix_db
        );
    }
    if separation.backing_to_mix_db < BACKING_MISSING_BELOW_MIX_DB {
        bail!(
            "the backing track is {:.1} dB below the mix: there is no usable accompaniment \
             (unaccompanied `{stem}` or failed separation). Choose a clip or region with accompaniment",
            separation.backing_to_mix_db
        );
    }

    let mut warnings = analysis.warnings.clone();
    if separation.lead_to_mix_db < LEAD_WEAK_BELOW_MIX_DB {
        warnings.push(format!(
            "`{stem}` stem is only {:.1} dB relative to the mix: transcription is low-confidence",
            separation.lead_to_mix_db
        ));
    }
    if separation.backing_to_mix_db < BACKING_WEAK_BELOW_MIX_DB {
        warnings.push(format!(
            "backing track is only {:.1} dB relative to the mix: it may be mostly separation residue",
            separation.backing_to_mix_db
        ));
    }
    if separation.backing_gain_limited {
        warnings.push(format!(
            "backing track was very quiet; gain limited to +{:.1} dB so residue is not amplified (peak stays below -1 dBFS)",
            separation.backing_gain_db
        ));
    }
    if separation.lead_selection == ASSUMED_LEAD_SELECTION {
        // DEFERRED: no lead-instrument detection yet (see ml/tono_ml/separation.py).
        warnings.push(format!(
            "--part lead ASSUMES the whole `{stem}` stem is the lead: it may include chords/pads or miss the melody. Not verified"
        ));
    }
    let low_confidence = !warnings.is_empty();
    Ok(SeparationQuality {
        low_confidence,
        warnings,
    })
}

fn report_separation(analysis: &Analysis, quality: &SeparationQuality) {
    let separation = &analysis.separation;
    println!(
        "✓ {} separated ({}, lead {:.1} dB vs mix)",
        separation.lead_stem, separation.model, separation.lead_to_mix_db
    );
    println!(
        "✓ backing/BGM created ({}, {} ch, BGM {:.1} dB vs mix, {:+.1} dB gain)",
        separation.backing_stems.join(" + "),
        separation.channels,
        separation.backing_to_mix_db,
        separation.backing_gain_db
    );
    for warning in &quality.warnings {
        println!("! {warning}");
    }
}

fn notes_document<T: serde::Serialize>(
    notes: &[T],
    range: SampleRange,
    stage: &str,
) -> serde_json::Value {
    json!({
        "version": 1,
        "stage": stage,
        // Times are relative to the selected region: t = 0 is region.start in the source.
        "timebase": "selected_region",
        "region_start_in_source": range.start_seconds(),
        "notes": notes,
    })
}

/// Everything project.json reports, gathered once.
struct ProjectSummary<'a> {
    options: &'a PrepOptions,
    source_duration: f64,
    range: SampleRange,
    decision: &'a RegionDecision,
    analysis: &'a Analysis,
    separation_quality: &'a SeparationQuality,
    cleanup_settings: &'a CleanupSettings,
    clean_note_count: usize,
    practice: &'a PracticeOutcome,
}

fn project_document(summary: &ProjectSummary) -> serde_json::Value {
    let ProjectSummary {
        options,
        source_duration,
        range,
        decision,
        analysis,
        separation_quality,
        cleanup_settings,
        clean_note_count,
        practice,
    } = summary;
    let (source_duration, range, clean_note_count) = (*source_duration, *range, *clean_note_count);
    let region = &decision.region;
    json!({
        "version": 1,
        "tono": env!("CARGO_PKG_VERSION"),
        "instrument": options.instrument,
        "part": options.part.worker_name(),
        "source": { "path": options.input.canonicalize().unwrap_or_else(|_| options.input.clone()), "duration": source_duration },
        "source_duration": source_duration,
        "selected_region": {
            // Sample-snapped values actually used for analysis.
            "start": range.start_seconds(),
            "end": range.end_seconds(),
            "method": region.method,
            "confidence": region.confidence,
        },
        "region_detection": decision.detector_model.as_ref().map(|model| json!({
            "model": model,
            "timeline": decision.labeled_spans,
            "excluded": excluded_spans(decision),
        })),
        "backing": {
            "source": "stem_mix",
            "excluded_part": analysis.separation.lead_stem,
            "stems": analysis.separation.backing_stems,
            "file": "backing.wav",
            "sample_rate": analysis.sample_rate,
            "channels": analysis.separation.channels,
            "format": "pcm_s24le",
            "gain_db": analysis.separation.backing_gain_db,
            "gain_limited": analysis.separation.backing_gain_limited,
            "duration": analysis.duration,
        },
        "lead": { "file": "lead.wav", "stem": analysis.separation.lead_stem },
        "separation": {
            "model": analysis.separation.model,
            "device": analysis.separation.device,
            "lead_selection": analysis.separation.lead_selection,
            "lead_to_mix_db": analysis.separation.lead_to_mix_db,
            "backing_to_mix_db": analysis.separation.backing_to_mix_db,
            "low_confidence": separation_quality.low_confidence,
            // Stem levels show presence only; separation quality is unmeasured.
            "confidence_basis": "stem levels relative to the mix (presence only); separation quality not measured",
        },
        "transcription": {
            "model": analysis.transcription.model,
            "parameters": analysis.transcription.parameters,
            "raw_note_count": analysis.notes.len(),
            "clean_note_count": clean_note_count,
            "cleanup": cleanup_settings,
            // Spec rule 11: model completion is not correctness.
            "verified_by_listening": false,
        },
        "bpm": analysis.bpm,
        "fingering": {
            "file": "fingering.json",
            "table_verified": practice.table_verified,
            "range_policy": practice.fitted.policy,
            "octave_shift": practice.fitted.octave_shift,
            "backing_transpose_semitones": practice.fitted.octave_shift * 12,
            "melody_shape_changed": practice.fitted.shape_changed(),
            "folded_note_count": practice.fitted.folded_notes.len(),
        },
        "practice": {
            "file": options.output_video.as_ref().map(|path| path.canonicalize().unwrap_or_else(|_| path.clone())).unwrap_or_else(|| PathBuf::from("practice.mp4")),
            "duration": practice.practice_duration_seconds,
            "tempo_scale": options.tempo_scale,
            "count_in": practice.count_in,
            "count_in_seconds": practice.count_in.map_or(0.0, crate::music::timeline::CountIn::duration),
            "upper_hand": options.upper_hand,
            "notation": "played-pitch treble staff, no inferred rhythmic notation",
            "metronome_requested": format!("{:?}", options.metronome).to_lowercase(),
            "metronome": format!("{:?}", practice.metronome).to_lowercase(),
            "bpm": practice.practice_bpm,
            "beat_source": "detected beat timestamps (tempo-scaled, offset by count-in)",
            "beat_count": practice.beat_count,
            // Bar position is not detected, so no downbeat accents are drawn or clicked.
            "downbeat_accents": false,
            "audio_bed": "backing.wav, transposed by backing_transpose_semitones, tempo-scaled if requested, delayed by count-in_seconds",
            "validation": {
                "tool": "ffprobe",
                "video_codec": practice.validated.video_codec,
                "audio_codec": practice.validated.audio_codec,
                "video_duration": practice.validated.video_duration_seconds,
                "audio_duration": practice.validated.audio_duration_seconds,
                "passed": true,
            },
        },
        "analysis_sample_rate": ANALYSIS_SAMPLE_RATE,
        "warnings": separation_quality.warnings.iter().chain(&practice.warnings).collect::<Vec<_>>(),
    })
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text + "\n").with_context(|| format!("writing {}", path.display()))
}

fn print_next_steps(out: &Path, output_video: Option<&Path>, keep_work: bool) {
    if let Some(video) = output_video {
        println!("\n✓ Ready: {}", video.display());
        println!("  Supporting files: {}", out.display());
        return;
    }
    let video = output_video
        .map(Path::to_path_buf)
        .unwrap_or_else(|| out.join("practice.mp4"));
    println!("\n  {}", video.display());
    for name in [
        "backing.wav",
        "lead.wav",
        "notes.json",
        "fingering.json",
        "project.json",
    ] {
        println!("  {}", out.join(name).display());
    }
    if keep_work {
        println!(
            "  {}  (stems, notes_raw.json, regions.json, logs)",
            out.join("work").display()
        );
    }
    println!("\nPractice and inspect:");
    println!("  open {}", video.display());
    println!(
        "  afplay {}   # vocals/lead should be absent or much quieter",
        out.join("backing.wav").display()
    );
    println!(
        "  afplay {}      # the melody that was transcribed",
        out.join("lead.wav").display()
    );
    println!(
        "  jq '.selected_region, .separation, .fingering, .warnings' {}",
        out.join("project.json").display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::worker::analysis_for_tests;

    fn analysis_with_lead_level(lead_to_mix_db: f64) -> Analysis {
        let mut analysis = analysis_for_tests(10.0, vec![]);
        analysis.separation.lead_to_mix_db = lead_to_mix_db;
        analysis
    }

    fn analysis_with_backing(backing_to_mix_db: f64, gain_limited: bool) -> Analysis {
        let mut analysis = analysis_for_tests(10.0, vec![]);
        analysis.separation.backing_to_mix_db = backing_to_mix_db;
        analysis.separation.backing_gain_limited = gain_limited;
        analysis
    }

    #[test]
    fn missing_backing_fails_instead_of_amplifying_residue() {
        // Reviewer case: residual BGM far below the mix, lead check passes.
        let error = assess_separation(&analysis_with_backing(-60.0, true))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("no usable accompaniment"), "{error}");
    }

    #[test]
    fn weak_or_gain_limited_backing_is_low_confidence() {
        // Residue level measured on an unaccompanied voice must be rejected.
        assert!(assess_separation(&analysis_with_backing(-21.5, false)).is_err());
        let quality = assess_separation(&analysis_with_backing(-15.0, false)).unwrap();
        assert!(quality.low_confidence && quality.warnings.len() == 1);
        let quality = assess_separation(&analysis_with_backing(-10.0, true)).unwrap();
        assert!(quality.low_confidence);
        assert!(quality.warnings[0].contains("gain limited"));
    }

    #[test]
    fn assumed_lead_stem_is_always_low_confidence() {
        let mut analysis = analysis_with_lead_level(-1.0); // loud `other` stem
        analysis.separation.lead_stem = "other".into();
        analysis.separation.lead_selection = ASSUMED_LEAD_SELECTION.into();
        let quality = assess_separation(&analysis).unwrap();
        assert!(quality.low_confidence, "loudness must not imply confidence");
        assert!(quality.warnings[0].contains("ASSUMES"));
    }

    #[test]
    fn missing_lead_fails_with_advice() {
        let error = assess_separation(&analysis_with_lead_level(-43.1))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("--part lead"), "{error}");
    }

    #[test]
    fn weak_lead_is_low_confidence_with_warning() {
        let quality = assess_separation(&analysis_with_lead_level(-30.0)).unwrap();
        assert!(quality.low_confidence);
        assert_eq!(quality.warnings.len(), 1);
    }

    #[test]
    fn clear_lead_is_confident() {
        let quality = assess_separation(&analysis_with_lead_level(-8.0)).unwrap();
        assert!(!quality.low_confidence);
        assert!(quality.warnings.is_empty());
    }
}

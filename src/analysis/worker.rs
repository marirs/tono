//! Invokes the Python ML worker and validates its versioned JSON output.
//!
//! Everything from Python is treated as untrusted input: versions, files and
//! durations are checked here before any other module sees them (spec rules
//! 7 and 10).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::analysis::cleanup::RawNoteEvent;
use crate::analysis::region::RegionFramesReport;
use crate::paths;

const SUPPORTED_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct SeparationInfo {
    pub model: String,
    pub device: String,
    pub lead_stem: String,
    pub backing_stems: Vec<String>,
    pub channels: u32,
    /// Lead stem energy vs the input mix: shows the lead is PRESENT, says
    /// nothing about how cleanly it was separated.
    pub lead_to_mix_db: f64,
    /// BGM energy vs the input mix, measured before normalization.
    pub backing_to_mix_db: f64,
    pub backing_gain_db: f64,
    /// True when the worker's gain cap stopped the BGM reaching -1 dBFS.
    pub backing_gain_limited: bool,
    /// How the lead stem was chosen, e.g. `assumed-other-stem`.
    pub lead_selection: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionInfo {
    pub model: String,
    pub parameters: serde_json::Value,
}

/// Spec "ML interface" contract, plus separation/transcription metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct Analysis {
    pub version: u32,
    pub sample_rate: u32,
    pub duration: f64,
    pub bpm: Option<f64>,
    /// Beat positions (seconds, region timebase) from the beat tracker.
    #[serde(default)]
    pub beat_times: Vec<f64>,
    pub lead_file: PathBuf,
    pub backing_file: PathBuf,
    pub stems: BTreeMap<String, PathBuf>,
    pub notes: Vec<RawNoteEvent>,
    /// Monophonic pitch evidence on the lead stem (absent in older contracts).
    #[serde(default)]
    pub pitch_track: Option<crate::analysis::evidence::PitchTrack>,
    pub separation: SeparationInfo,
    pub transcription: TranscriptionInfo,
    #[serde(default)]
    pub warnings: Vec<String>,
}

pub struct WorkerPaths<'a> {
    pub work_dir: &'a Path,
    pub out_dir: &'a Path,
}

fn worker_command() -> Result<Command> {
    let mut command = paths::python_command()?;
    command
        .arg(paths::ml_worker_script())
        // Runs must never download models; setup_venv.sh does that.
        .env("HF_HUB_OFFLINE", "1")
        .env("PYTHONUNBUFFERED", "1");
    Ok(command)
}

fn run_worker(mut command: Command, what: &str, log_path: &Path) -> Result<()> {
    let output = command
        .output()
        .with_context(|| format!("starting ML worker for {what}"))?;
    // Keep the worker's stderr (model warnings, tracebacks) for debugging.
    std::fs::write(log_path, &output.stderr)
        .with_context(|| format!("writing {}", log_path.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(8).collect();
        bail!(
            "ML worker failed during {what} ({}):\n{}\nfull log: {}",
            output.status,
            tail.into_iter().rev().collect::<Vec<_>>().join("\n"),
            log_path.display()
        );
    }
    Ok(())
}

pub fn detect_region_frames(input_wav: &Path, work_dir: &Path) -> Result<RegionFramesReport> {
    let output_path = work_dir.join("regions.json");
    let mut command = worker_command()?;
    command
        .arg("--detect-regions")
        .arg("--input")
        .arg(input_wav)
        .arg("--output")
        .arg(&output_path);
    run_worker(
        command,
        "song-region detection",
        &work_dir.join("ml-regions.log"),
    )?;

    let report: RegionFramesReport = read_json(&output_path)?;
    if report.version != SUPPORTED_CONTRACT_VERSION {
        bail!("region report version {} unsupported", report.version);
    }
    if report.frames.is_empty() {
        bail!("region detection returned no frames");
    }
    Ok(report)
}

pub fn analyze_region(
    region_wav: &Path,
    part: &str,
    model: &str,
    separation_only: bool,
    worker_paths: &WorkerPaths,
    expected_duration: f64,
) -> Result<Analysis> {
    let output_path = worker_paths.work_dir.join("analysis.json");
    let mut command = worker_command()?;
    command
        .arg("--input")
        .arg(region_wav)
        .args(["--part", part, "--separation-model", model])
        .arg("--output")
        .arg(&output_path)
        .arg("--work-dir")
        .arg(worker_paths.work_dir)
        .arg("--out-dir")
        .arg(worker_paths.out_dir);
    if separation_only {
        command.arg("--separate-only");
    }
    run_worker(
        command,
        "separation + transcription",
        &worker_paths.work_dir.join("ml-analysis.log"),
    )?;

    let analysis: Analysis = read_json(&output_path)?;
    validate_analysis(&analysis, expected_duration)?;
    Ok(analysis)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// One audio sample of slack; anything else means the worker changed length.
const DURATION_SLACK_SECONDS: f64 = 0.001;

fn validate_analysis(analysis: &Analysis, expected_duration: f64) -> Result<()> {
    if analysis.version != SUPPORTED_CONTRACT_VERSION {
        bail!("analysis contract version {} unsupported", analysis.version);
    }
    if !expected_duration.is_finite()
        || expected_duration <= 0.0
        || !analysis.duration.is_finite()
        || analysis.duration <= 0.0
        || analysis.sample_rate != 44_100
    {
        bail!("invalid analysis duration or sample rate (expected 44100 Hz)");
    }
    if analysis
        .bpm
        .is_some_and(|bpm| !bpm.is_finite() || bpm <= 0.0)
        || analysis
            .beat_times
            .iter()
            .any(|t| !t.is_finite() || *t < 0.0 || *t > analysis.duration + DURATION_SLACK_SECONDS)
        || analysis
            .beat_times
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        bail!("worker produced invalid tempo or beat timestamps");
    }
    if let Some(track) = &analysis.pitch_track {
        let covered = track.f0_midi.len() as f64 * track.hop_seconds;
        if !track.is_consistent()
            || !covered.is_finite()
            || (covered - analysis.duration).abs() > 2.0 * track.hop_seconds
        {
            bail!("worker produced an invalid pitch track (values, lengths or region coverage)");
        }
    }
    if (analysis.duration - expected_duration).abs() > DURATION_SLACK_SECONDS {
        bail!(
            "worker analysed {:.3}s but the selected region is {:.3}s",
            analysis.duration,
            expected_duration
        );
    }
    for file in [&analysis.lead_file, &analysis.backing_file]
        .into_iter()
        .chain(analysis.stems.values())
    {
        if !file.is_file() {
            bail!("worker reported {} but it does not exist", file.display());
        }
    }
    for note in &analysis.notes {
        let in_bounds = note.start >= 0.0 && note.end <= analysis.duration + DURATION_SLACK_SECONDS;
        if !in_bounds
            || note.end <= note.start
            || note.midi > 127
            || !note.start.is_finite()
            || !note.end.is_finite()
            || !note.confidence.is_finite()
            || !(0.0..=1.0).contains(&note.confidence)
            || note
                .attack
                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            bail!("worker produced an invalid note event {note:?}");
        }
    }
    Ok(())
}

/// Minimal valid Analysis for unit tests in other modules.
#[cfg(test)]
pub fn analysis_for_tests(duration: f64, notes: Vec<RawNoteEvent>) -> Analysis {
    let existing_file = crate::paths::project_root().join("Cargo.toml");
    Analysis {
        version: 1,
        sample_rate: 44_100,
        duration,
        bpm: Some(100.0),
        beat_times: vec![],
        pitch_track: None,
        lead_file: existing_file.clone(),
        backing_file: existing_file,
        stems: BTreeMap::new(),
        notes,
        separation: SeparationInfo {
            model: "htdemucs".into(),
            device: "cpu".into(),
            lead_stem: "vocals".into(),
            backing_stems: vec!["drums".into()],
            channels: 2,
            lead_to_mix_db: -6.0,
            backing_to_mix_db: -3.0,
            backing_gain_db: 0.0,
            backing_gain_limited: false,
            lead_selection: "model-vocals-stem".into(),
        },
        transcription: TranscriptionInfo {
            model: "basic-pitch".into(),
            parameters: serde_json::Value::Null,
        },
        warnings: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed_pitch_evidence_instead_of_silently_ignoring_it() {
        use crate::analysis::evidence::test_support::track;
        let mut valid = analysis_for_tests(1.0, vec![]);
        valid.pitch_track = Some(track(&[(1.0, Some(60.0), -12.0)]));
        validate_analysis(&valid, 1.0).unwrap();
        for case in 0..6 {
            let mut bad = valid.clone();
            let t = bad.pitch_track.as_mut().unwrap();
            match case {
                0 => {
                    t.level_db.pop();
                }
                1 => t.hop_seconds = f64::INFINITY,
                2 => t.voiced_probability[0] = 1.1,
                3 => t.f0_midi[0] = Some(f64::NAN),
                4 => t.level_db[0] = f64::INFINITY,
                _ => t.hop_seconds = 0.001, // track no longer covers region
            }
            assert!(validate_analysis(&bad, 1.0).is_err(), "case {case}");
        }
        // Older worker contracts can still omit the new evidence field.
        valid.pitch_track = None;
        validate_analysis(&valid, 1.0).unwrap();
    }

    #[test]
    fn rejects_invalid_numbers_probabilities_and_beats() {
        let valid = analysis_for_tests(
            1.0,
            vec![RawNoteEvent {
                start: 0.0,
                end: 0.5,
                midi: 60,
                confidence: 0.8,
                attack: Some(0.9),
            }],
        );
        for case in 0..7 {
            let mut bad = valid.clone();
            match case {
                0 => bad.duration = f64::NAN,
                1 => bad.sample_rate = 0,
                2 => bad.notes[0].confidence = 2.0,
                3 => bad.notes[0].attack = Some(-1.0),
                4 => bad.bpm = Some(f64::INFINITY),
                5 => bad.beat_times = vec![0.5, 0.4],
                _ => bad.beat_times = vec![2.0],
            }
            assert!(validate_analysis(&bad, 1.0).is_err(), "case {case}");
        }
    }

    #[test]
    fn accepts_consistent_analysis() {
        let notes = vec![RawNoteEvent {
            start: 0.5,
            end: 1.0,
            midi: 64,
            confidence: 0.8,
            attack: None,
        }];
        validate_analysis(&analysis_for_tests(10.0, notes), 10.0).unwrap();
    }

    #[test]
    fn rejects_duration_mismatch_and_out_of_range_notes() {
        assert!(validate_analysis(&analysis_for_tests(9.0, vec![]), 10.0).is_err());
        let late_note = vec![RawNoteEvent {
            start: 9.5,
            end: 10.5,
            midi: 64,
            confidence: 0.8,
            attack: None,
        }];
        assert!(validate_analysis(&analysis_for_tests(10.0, late_note), 10.0).is_err());
    }

    #[test]
    fn rejects_missing_files() {
        let mut analysis = analysis_for_tests(10.0, vec![]);
        analysis.backing_file = PathBuf::from("/nonexistent/backing.wav");
        assert!(validate_analysis(&analysis, 10.0).is_err());
    }
}

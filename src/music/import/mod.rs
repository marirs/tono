//! Explicit melody input. No ML cleanup, scale snapping or invented timing.
mod midi;
mod musicxml;
use super::notes::{validate_monophonic_sequence, NoteEvent};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug)]
pub struct ImportedMelody {
    pub notes: Vec<NoteEvent>,
    pub duration: f64,
    pub beat_times: Vec<f64>,
    pub warnings: Vec<String>,
    pub format: &'static str,
}

pub fn is_melody_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "mid" | "midi" | "musicxml" | "xml" | "json" | "mxl"
    )
}

pub fn load(path: &Path, track: Option<u16>) -> Result<ImportedMelody> {
    if track == Some(0) {
        bail!("melody track indices start at 1");
    }
    if std::fs::metadata(path)?.len() > 16 * 1024 * 1024 {
        bail!("melody file exceeds 16 MiB");
    }
    let bytes = std::fs::read(path)?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut song = match ext.as_str() {
        "mid" | "midi" => midi::parse(&bytes, track)?,
        "musicxml" | "xml" => musicxml::parse(std::str::from_utf8(&bytes)?, track)?,
        "json" => {
            if track.is_some() {
                bail!("--melody-track applies only to MIDI/MusicXML");
            }
            parse_json(&bytes)?
        }
        "mxl" => bail!("compressed MusicXML is not supported yet; export uncompressed .musicxml"),
        _ => bail!("unsupported melody format"),
    };
    song.notes.sort_by(|a, b| a.start.total_cmp(&b.start));
    validate_monophonic_sequence(&song.notes).context(
        "import needs a single non-overlapping melody; select a track or export a monophonic part",
    )?;
    if song.notes.is_empty() {
        bail!("melody contains no notes");
    }
    if !song.duration.is_finite() || song.duration <= 0.0 || song.duration > 7200.0 {
        bail!("melody duration must be positive and at most two hours");
    }
    if song.notes.iter().any(|n| {
        n.midi > 127
            || !n.confidence.is_finite()
            || !(0.0..=1.0).contains(&n.confidence)
            || n.end > song.duration + 1e-6
    }) {
        bail!("invalid imported pitch, confidence or duration");
    }
    Ok(song)
}

#[derive(Deserialize)]
struct JsonNote {
    start: f64,
    end: f64,
    midi: u8,
    #[serde(default = "one")]
    confidence: f64,
}
fn one() -> f64 {
    1.0
}
#[derive(Deserialize)]
struct JsonMelody {
    notes: Vec<JsonNote>,
    duration_seconds: Option<f64>,
    #[serde(alias = "region_start_in_source")]
    source_offset_seconds: Option<f64>,
    #[serde(default)]
    warnings: Vec<String>,
    status: Option<String>,
}
fn parse_json(bytes: &[u8]) -> Result<ImportedMelody> {
    let j: JsonMelody = serde_json::from_slice(bytes)?;
    let notes: Vec<_> = j
        .notes
        .into_iter()
        .map(|n| NoteEvent {
            start: n.start,
            end: n.end,
            midi: n.midi,
            confidence: n.confidence,
        })
        .collect();
    let duration = j
        .duration_seconds
        .unwrap_or_else(|| notes.iter().map(|n| n.end).fold(0.0, f64::max));
    let mut warnings = j.warnings;
    if let Some(offset) = j.source_offset_seconds.filter(|o| *o != 0.0) {
        warnings.push(format!("JSON suggests source offset {offset}s; use --audio-offset {offset} with the matching full recording (not applied automatically)"));
    }
    if let Some(status) = j.status {
        warnings.push(format!("Imported source status: {status}"));
    }
    Ok(ImportedMelody {
        notes,
        duration,
        beat_times: vec![],
        warnings,
        format: "json",
    })
}

/// Tempo events are quarter-note positions and microseconds per quarter.
pub(super) fn seconds_at(beat: f64, tempos: &[(f64, f64)]) -> f64 {
    let mut last = 0.0;
    let mut seconds = 0.0;
    let mut us = 500_000.0;
    for &(at, next) in tempos {
        if at > beat {
            break;
        }
        seconds += (at - last) * us / 1e6;
        last = at;
        us = next;
    }
    seconds + (beat - last) * us / 1e6
}
pub(super) fn beats_until(end: f64, tempos: &[(f64, f64)]) -> Vec<f64> {
    (0..100_000)
        .map(|b| seconds_at(b as f64, tempos))
        .take_while(|t| *t < end)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn json_file(text: &str) -> tempfile::NamedTempFile {
        let f = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        std::fs::write(f.path(), text).unwrap();
        f
    }
    #[test]
    fn json_preserves_rests_repeated_notes_and_low_confidence() {
        let f = json_file(
            r#"{"duration_seconds":4,"source_offset_seconds":7.68,"notes":[{"start":0.5,"end":1,"midi":62,"confidence":0.01},{"start":2,"end":3,"midi":62}]}"#,
        );
        let s = load(f.path(), None).unwrap();
        assert_eq!(s.duration, 4.0);
        assert_eq!(s.notes[1].start, 2.0);
        assert_eq!(s.notes[0].confidence, 0.01);
        assert!(s.warnings[0].contains("--audio-offset 7.68"));
    }
    #[test]
    fn refuses_overlap_out_of_range_and_short_declared_duration() {
        for text in [
            r#"{"notes":[{"start":0,"end":2,"midi":60},{"start":1,"end":3,"midi":64}]}"#,
            r#"{"notes":[{"start":0,"end":1,"midi":200}]}"#,
            r#"{"duration_seconds":0.5,"notes":[{"start":0,"end":1,"midi":60}]}"#,
        ] {
            assert!(load(json_file(text).path(), None).is_err());
        }
    }
}

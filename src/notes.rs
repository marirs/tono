//! Note events: the pitch/timing contract shared by the ML worker output,
//! note cleanup, fingering mapping and the renderer.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoteEvent {
    /// Seconds from the start of the (clipped) practice timeline.
    pub start: f64,
    pub end: f64,
    pub midi: u8,
    pub confidence: f64,
}

/// Rejects sequences the renderer cannot display unambiguously: unsorted,
/// overlapping (monophonic instrument), zero-length or non-finite events.
pub fn validate_monophonic_sequence(notes: &[NoteEvent]) -> Result<()> {
    for (index, note) in notes.iter().enumerate() {
        if !note.start.is_finite() || !note.end.is_finite() || note.start < 0.0 {
            bail!("note #{index} has invalid timing: {note:?}");
        }
        if note.end <= note.start {
            bail!("note #{index} has non-positive duration: {note:?}");
        }
        if let Some(previous) = index.checked_sub(1).map(|i| notes[i]) {
            if note.start < previous.end {
                bail!("note #{index} overlaps previous note: {previous:?} / {note:?}");
            }
        }
    }
    Ok(())
}

/// M1 hard-coded melody: "Mary Had a Little Lamb" in C major, trimmed to the
/// first 12 notes (spec M1: 8-12 events) with the final G held.
///
/// Chosen because it contains stepwise motion, a leap (E→G) and several
/// repeated notes, which exercises the re-attack display between identical
/// fingerings. Each note sounds for 85% of its beat so repeated notes are
/// separated by an audible and visible gap.
pub fn demo_melody(beats_per_minute: f64) -> Vec<NoteEvent> {
    const E4: u8 = 64;
    const D4: u8 = 62;
    const C4: u8 = 60;
    const G4: u8 = 67;
    // (midi, length in beats)
    let phrase: [(u8, f64); 12] = [
        (E4, 1.0), (D4, 1.0), (C4, 1.0), (D4, 1.0),
        (E4, 1.0), (E4, 1.0), (E4, 2.0),
        (D4, 1.0), (D4, 1.0), (D4, 2.0),
        (E4, 1.0), (G4, 3.0),
    ];
    let seconds_per_beat = 60.0 / beats_per_minute;
    // One bar of count-in so the player sees the first fingering before
    // having to play it.
    let count_in_beats = 4.0;
    let sounding_fraction = 0.85;

    let mut beat_cursor = count_in_beats;
    phrase
        .iter()
        .map(|&(midi, length_beats)| {
            let start = beat_cursor * seconds_per_beat;
            let end = start + length_beats * sounding_fraction * seconds_per_beat;
            beat_cursor += length_beats;
            NoteEvent { start, end, midi, confidence: 1.0 }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: f64, end: f64) -> NoteEvent {
        NoteEvent { start, end, midi: 60, confidence: 1.0 }
    }

    #[test]
    fn demo_melody_is_valid_and_starts_after_count_in() {
        let melody = demo_melody(100.0);
        validate_monophonic_sequence(&melody).unwrap();
        assert!((8..=12).contains(&melody.len()), "spec M1 asks for 8-12 notes");
        assert!((melody[0].start - 2.4).abs() < 1e-9, "count-in is 4 beats at 100 bpm");
    }

    #[test]
    fn repeated_demo_notes_have_a_gap() {
        let melody = demo_melody(100.0);
        for pair in melody.windows(2) {
            assert!(pair[1].start - pair[0].end > 0.05, "no articulation gap: {pair:?}");
        }
    }

    #[test]
    fn rejects_overlap_and_bad_durations() {
        assert!(validate_monophonic_sequence(&[note(0.0, 1.0), note(0.5, 2.0)]).is_err());
        assert!(validate_monophonic_sequence(&[note(1.0, 1.0)]).is_err());
        assert!(validate_monophonic_sequence(&[note(-1.0, 1.0)]).is_err());
        assert!(validate_monophonic_sequence(&[note(0.0, 1.0), note(1.0, 2.0)]).is_ok());
    }
}

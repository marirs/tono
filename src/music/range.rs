//! Fits a transcribed melody into an instrument profile's charted range.
//!
//! Only notes present in the profile's fingering table can be shown (never
//! guessed). The melody is shifted as a WHOLE by the smallest whole-octave
//! amount that puts every note in range, so every interval is preserved and
//! the practice backing is transposed by the same amount. If no shift fits,
//! fitting fails and lists the offending notes.
//!
//! Individual octave folding (moving single notes by octaves) is not
//! implemented: it changes the melody's shape (a rising step can become a
//! downward leap). `RangePolicy::Fold` survives only so the existing
//! `--range-policy` option still parses; the engine rejects it here too, so
//! no frontend can bypass the CLI check.

use anyhow::{bail, Result};
use serde::Serialize;

use crate::instruments::fingering::FingeringTable;
use crate::music::notes::NoteEvent;

/// Candidate whole-melody shifts in octaves, in order of preference.
const OCTAVE_SHIFT_CANDIDATES: [i32; 5] = [0, 1, -1, 2, -2];

/// Out-of-range notes listed in the failure message.
const MAX_LISTED_NOTES: usize = 8;

pub const FOLDING_DISABLED_MESSAGE: &str =
    "individual octave folding is disabled; use --range-policy strict to preserve the melody";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum RangePolicy {
    Strict,
    /// Legacy value: accepted by the parser, always rejected by the engine.
    Fold,
}

#[derive(Debug, Clone, Serialize)]
pub struct FittedMelody {
    #[serde(skip)]
    pub notes: Vec<NoteEvent>,
    pub policy: RangePolicy,
    /// Whole-melody shift; the backing is transposed by the same amount.
    pub octave_shift: i32,
}

impl FittedMelody {
    pub fn is_unchanged(&self) -> bool {
        self.octave_shift == 0
    }

    pub fn transpose_semitones(&self) -> i32 {
        self.octave_shift * 12
    }
}

fn shift_midi(midi: u8, semitones: i32) -> Option<u8> {
    u8::try_from(midi as i32 + semitones)
        .ok()
        .filter(|shifted| *shifted <= 127)
}

pub fn fit_melody_to_table(
    notes: &[NoteEvent],
    table: &FingeringTable,
    policy: RangePolicy,
) -> Result<FittedMelody> {
    if policy == RangePolicy::Fold {
        bail!(FOLDING_DISABLED_MESSAGE);
    }
    let playable = |midi: u8| table.lookup(midi).is_ok();
    let fits =
        |note: &NoteEvent, octaves: i32| shift_midi(note.midi, octaves * 12).is_some_and(playable);
    let fitting_count = |octaves: i32| notes.iter().filter(|note| fits(note, octaves)).count();

    let whole_shift = OCTAVE_SHIFT_CANDIDATES
        .iter()
        .copied()
        .find(|&octaves| fitting_count(octaves) == notes.len());
    if let Some(octave_shift) = whole_shift {
        let shifted = notes
            .iter()
            .map(|note| NoteEvent {
                midi: shift_midi(note.midi, octave_shift * 12).expect("checked by fits"),
                ..*note
            })
            .collect();
        return Ok(FittedMelody {
            notes: shifted,
            policy,
            octave_shift,
        });
    }

    // max_by_key keeps the LAST maximum; iterate candidates reversed so the
    // most preferred shift wins ties.
    let best_shift = OCTAVE_SHIFT_CANDIDATES
        .iter()
        .rev()
        .copied()
        .max_by_key(|&octaves| fitting_count(octaves))
        .unwrap_or(0);
    bail!(range_failure_message(notes, table, best_shift, &fits))
}

fn range_failure_message(
    notes: &[NoteEvent],
    table: &FingeringTable,
    best_shift: i32,
    fits: &impl Fn(&NoteEvent, i32) -> bool,
) -> String {
    let lowest = notes.iter().map(|n| n.midi).min().unwrap_or(0);
    let highest = notes.iter().map(|n| n.midi).max().unwrap_or(0);
    let (range_low, range_high) = table.charted_range().unwrap_or((0, 0));
    let outliers: Vec<String> = notes
        .iter()
        .filter(|note| !fits(note, best_shift))
        .take(MAX_LISTED_NOTES)
        .map(|note| format!("{:.2}s MIDI {}", note.start, note.midi))
        .collect();
    let outlier_count = notes.iter().filter(|note| !fits(note, best_shift)).count();
    format!(
        "melody spans MIDI {lowest}-{highest} ({} semitones) but the profile range is MIDI {range_low}-{range_high}; \
         no whole-octave shift fits every note. With the best shift ({best_shift:+} octave) {outlier_count} note(s) remain out of range: {}{}. \
         Choose a narrower region with --from/--to; individual octave folding is disabled",
        highest - lowest,
        outliers.join(", "),
        if outlier_count > MAX_LISTED_NOTES { ", ..." } else { "" },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> FingeringTable {
        {
            let mut table =
                FingeringTable::load_from_file(&crate::paths::ae01_fingering_table()).unwrap();
            // Keep narrow-table regressions independent of the production range.
            table
                .fingerings
                .retain(|midi, _| (59..=73).contains(&midi.parse::<u8>().unwrap()));
            table
        }
    }

    fn melody(pitches: &[u8]) -> Vec<NoteEvent> {
        pitches
            .iter()
            .enumerate()
            .map(|(i, &midi)| NoteEvent {
                start: i as f64,
                end: i as f64 + 0.5,
                midi,
                confidence: 0.9,
            })
            .collect()
    }

    fn fit(pitches: &[u8], policy: RangePolicy) -> Result<FittedMelody> {
        fit_melody_to_table(&melody(pitches), &table(), policy)
    }

    fn played(fitted: &FittedMelody) -> Vec<u8> {
        fitted.notes.iter().map(|n| n.midi).collect()
    }

    #[test]
    fn full_table_fits_poove_without_individual_folding() {
        let full = FingeringTable::load_from_file(&crate::paths::ae01_fingering_table()).unwrap();
        let fitted =
            fit_melody_to_table(&melody(&[39, 63, 65, 70]), &full, RangePolicy::Strict).unwrap();
        assert_eq!(fitted.octave_shift, 1);
        assert_eq!(played(&fitted), vec![51, 75, 77, 82]);
        let inside =
            fit_melody_to_table(&melody(&[47, 60, 85]), &full, RangePolicy::Strict).unwrap();
        assert!(inside.is_unchanged());
    }

    #[test]
    fn in_range_melody_is_unchanged() {
        let fitted = fit(&[60, 62, 64, 67], RangePolicy::Strict).unwrap();
        assert!(fitted.is_unchanged());
    }

    #[test]
    fn low_voice_is_shifted_up_one_octave_intact() {
        // Male voice around C3-A3: +1 octave fits every note.
        let fitted = fit(&[48, 50, 52, 55, 57], RangePolicy::Strict).unwrap();
        assert_eq!(fitted.octave_shift, 1);
        assert_eq!(played(&fitted), vec![60, 62, 64, 67, 69]);
    }

    #[test]
    fn strict_prefers_no_shift_then_up() {
        assert_eq!(fit(&[60, 72], RangePolicy::Strict).unwrap().octave_shift, 0);
        // 48..=59 fits with +1 (60..=71) only.
        assert_eq!(fit(&[48, 59], RangePolicy::Strict).unwrap().octave_shift, 1);
    }

    #[test]
    fn strict_rejects_melody_that_needs_folding() {
        // Reviewer regression: folding would turn the final rising step 72->76
        // into a downward leap 72->64. Strict must refuse instead.
        let error = fit(&[64, 67, 72, 76], RangePolicy::Strict)
            .unwrap_err()
            .to_string();
        assert!(error.contains("3.00s MIDI 76"), "{error}");
        assert!(error.contains("folding is disabled"), "{error}");
    }

    #[test]
    fn strict_never_changes_intervals() {
        let original = [48u8, 50, 52, 53, 55, 57, 59, 60]; // fits with +1 octave
        let fitted = fit(&original, RangePolicy::Strict).unwrap();
        let played = played(&fitted);
        let intervals = |p: &[u8]| {
            p.windows(2)
                .map(|w| w[1] as i32 - w[0] as i32)
                .collect::<Vec<_>>()
        };
        assert_eq!(intervals(&played), intervals(&original));
    }

    #[test]
    fn engine_rejects_individual_folding_even_without_the_cli() {
        // A frontend calling the library directly must not be able to fold.
        let error = fit(&[64, 67, 72, 76], RangePolicy::Fold)
            .unwrap_err()
            .to_string();
        assert!(error.contains("folding is disabled"), "{error}");
        // Even when the melody would fit unchanged, Fold is refused.
        assert!(fit(&[60, 62], RangePolicy::Fold).is_err());
    }
}

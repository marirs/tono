//! Fits a transcribed melody into the fingerable AE-01 range.
//!
//! Only notes present in the verified-data fingering table can be shown
//! (rule 9), expanded by Roland's documented octave controls to B2-C#6. Two policies:
//!
//! * `strict` (default): shift the WHOLE melody by the smallest whole-octave
//!   amount that puts every note in range. Intervals are preserved, so the
//!   melody is the same tune an octave up/down. If no shift fits, fail and
//!   list the offending notes.
//! * `fold` (explicit opt-in): best whole-octave shift, then move each
//!   remaining outlier by octaves into range. This CHANGES the melody's
//!   shape (a rising step can become a downward leap), so it is reported on
//!   the console, in fingering.json and as a warning in the video.

use anyhow::{bail, Result};
use serde::Serialize;

use crate::fingering::FingeringTable;
use crate::notes::NoteEvent;

/// Candidate whole-melody shifts in octaves, in order of preference.
const OCTAVE_SHIFT_CANDIDATES: [i32; 5] = [0, 1, -1, 2, -2];

/// Out-of-range notes listed in the strict-mode error.
const MAX_LISTED_NOTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum RangePolicy {
    Strict,
    Fold,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FoldedNote {
    pub index: usize,
    pub start: f64,
    /// Pitch after the whole-melody shift, before folding.
    pub shifted_midi: u8,
    pub played_midi: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct FittedMelody {
    #[serde(skip)]
    pub notes: Vec<NoteEvent>,
    pub policy: RangePolicy,
    pub octave_shift: i32,
    /// Always empty under `strict`.
    pub folded_notes: Vec<FoldedNote>,
}

impl FittedMelody {
    pub fn is_unchanged(&self) -> bool {
        self.octave_shift == 0 && self.folded_notes.is_empty()
    }

    /// True when the played melody no longer has the original intervals.
    pub fn shape_changed(&self) -> bool {
        !self.folded_notes.is_empty()
    }
}

fn shift_midi(midi: u8, semitones: i32) -> Option<u8> {
    u8::try_from(midi as i32 + semitones).ok().filter(|shifted| *shifted <= 127)
}

pub fn fit_melody_to_table(notes: &[NoteEvent], table: &FingeringTable, policy: RangePolicy) -> Result<FittedMelody> {
    let playable = |midi: u8| table.lookup(midi).is_ok();
    let fits = |note: &NoteEvent, octaves: i32| shift_midi(note.midi, octaves * 12).is_some_and(playable);
    let fitting_count = |octaves: i32| notes.iter().filter(|note| fits(note, octaves)).count();

    let whole_shift = OCTAVE_SHIFT_CANDIDATES.iter().copied().find(|&octaves| fitting_count(octaves) == notes.len());
    if let Some(octave_shift) = whole_shift {
        let shifted = notes
            .iter()
            .map(|note| NoteEvent { midi: shift_midi(note.midi, octave_shift * 12).expect("checked by fits"), ..*note })
            .collect();
        return Ok(FittedMelody { notes: shifted, policy, octave_shift, folded_notes: Vec::new() });
    }

    // max_by_key keeps the LAST maximum; iterate candidates reversed so the
    // most preferred shift wins ties.
    let best_shift = OCTAVE_SHIFT_CANDIDATES.iter().rev().copied().max_by_key(|&octaves| fitting_count(octaves)).unwrap_or(0);
    match policy {
        RangePolicy::Strict => bail!(strict_failure_message(notes, table, best_shift, &fits)),
        RangePolicy::Fold => fold_outliers(notes, best_shift, &playable),
    }
}

fn strict_failure_message(
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

fn fold_outliers(notes: &[NoteEvent], octave_shift: i32, playable: &impl Fn(u8) -> bool) -> Result<FittedMelody> {
    let mut fitted_notes = Vec::with_capacity(notes.len());
    let mut folded_notes = Vec::new();
    for (index, note) in notes.iter().enumerate() {
        let Some(shifted) = shift_midi(note.midi, octave_shift * 12) else {
            bail!("MIDI {} cannot be shifted by {octave_shift:+} octave", note.midi);
        };
        let played = if playable(shifted) {
            shifted
        } else {
            match nearest_playable_octave(shifted, playable) {
                Some(folded) => folded,
                None => bail!("MIDI {shifted} has no charted fingering in any octave"),
            }
        };
        if played != shifted {
            folded_notes.push(FoldedNote { index, start: note.start, shifted_midi: shifted, played_midi: played });
        }
        fitted_notes.push(NoteEvent { midi: played, ..*note });
    }
    Ok(FittedMelody { notes: fitted_notes, policy: RangePolicy::Fold, octave_shift, folded_notes })
}

fn nearest_playable_octave(midi: u8, playable: &impl Fn(u8) -> bool) -> Option<u8> {
    (1..=10)
        .flat_map(|octaves| [-octaves, octaves])
        .filter_map(|octaves| shift_midi(midi, octaves * 12))
        .find(|candidate| playable(*candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> FingeringTable {
        {
            let mut table = FingeringTable::load_from_file(&crate::paths::ae01_fingering_table()).unwrap();
            // Keep narrow-table regressions independent of the production range.
            table.fingerings.retain(|midi, _| (59..=73).contains(&midi.parse::<u8>().unwrap()));
            table
        }
    }

    fn melody(pitches: &[u8]) -> Vec<NoteEvent> {
        pitches
            .iter()
            .enumerate()
            .map(|(i, &midi)| NoteEvent { start: i as f64, end: i as f64 + 0.5, midi, confidence: 0.9 })
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
        let fitted = fit_melody_to_table(&melody(&[39, 63, 65, 70]), &full, RangePolicy::Strict).unwrap();
        assert_eq!(fitted.octave_shift, 1);
        assert_eq!(played(&fitted), vec![51, 75, 77, 82]);
        assert!(!fitted.shape_changed());
        let inside = fit_melody_to_table(&melody(&[47, 60, 85]), &full, RangePolicy::Strict).unwrap();
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
        assert!(!fitted.shape_changed());
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
        let error = fit(&[64, 67, 72, 76], RangePolicy::Strict).unwrap_err().to_string();
        assert!(error.contains("3.00s MIDI 76"), "{error}");
        assert!(error.contains("folding is disabled"), "{error}");
    }

    #[test]
    fn strict_never_changes_intervals() {
        let original = [48u8, 50, 52, 53, 55, 57, 59, 60]; // fits with +1 octave
        let fitted = fit(&original, RangePolicy::Strict).unwrap();
        let played = played(&fitted);
        let intervals = |p: &[u8]| p.windows(2).map(|w| w[1] as i32 - w[0] as i32).collect::<Vec<_>>();
        assert_eq!(intervals(&played), intervals(&original));
    }

    #[test]
    fn fold_is_explicit_and_reports_shape_change() {
        let fitted = fit(&[64, 67, 72, 76], RangePolicy::Fold).unwrap();
        assert_eq!(fitted.policy, RangePolicy::Fold);
        assert!(fitted.shape_changed());
        assert_eq!(fitted.folded_notes, vec![FoldedNote { index: 3, start: 3.0, shifted_midi: 76, played_midi: 64 }]);
        assert_eq!(played(&fitted), vec![64, 67, 72, 64]);
    }

    #[test]
    fn fold_makes_every_pitch_fingerable() {
        let table = table();
        let wide: Vec<u8> = (36..=96).collect();
        let fitted = fit_melody_to_table(&melody(&wide), &table, RangePolicy::Fold).unwrap();
        assert!(fitted.notes.iter().all(|n| table.lookup(n.midi).is_ok()));
    }
}

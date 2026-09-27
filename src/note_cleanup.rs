//! Deterministic note cleanup (spec "Note cleanup").
//!
//! Basic Pitch is polyphonic and reacts to vibrato, slides and harmonics.
//! The AE-01 plays one note at a time, so raw events are reduced to a clean
//! monophonic line here. Pitches are never invented: every output note's
//! MIDI value comes from a raw event.

use serde::{Deserialize, Serialize};

use crate::notes::NoteEvent;

/// Transcriber output before cleanup. `attack` is the onset activation at
/// the note start (0..1), when the transcriber provides one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RawNoteEvent {
    pub start: f64,
    pub end: f64,
    pub midi: u8,
    pub confidence: f64,
    #[serde(default)]
    pub attack: Option<f64>,
}

impl RawNoteEvent {
    fn has_real_attack(&self, threshold: f64) -> bool {
        self.attack.is_some_and(|attack| attack >= threshold)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupSettings {
    /// Rule 1: events below this confidence are discarded.
    pub min_confidence: f64,
    /// Rule 2: shorter events are discarded unless merged into a neighbour.
    pub min_note_seconds: f64,
    /// Rule 3: identical adjacent pitches closer than this are one note...
    pub merge_gap_seconds: f64,
    /// ...unless (rule 5) the second one starts with an onset at least this
    /// strong: then it is a real repeated note. Basic Pitch already emits
    /// notes at onsets >= 0.5, but vibrato alone triggers 0.50-0.60 there;
    /// on the synthetic test reel real re-attacks scored 0.90-0.96. 0.75 sits
    /// in that gap. Re-check on real singing in M4.
    pub repeat_attack_threshold: f64,
    /// Rule 4: a +/-1 semitone excursion shorter than this, between two
    /// notes of the same pitch, is vibrato and absorbed.
    pub vibrato_max_excursion_seconds: f64,
}

impl Default for CleanupSettings {
    fn default() -> Self {
        CleanupSettings {
            min_confidence: 0.3,
            min_note_seconds: 0.060,
            merge_gap_seconds: 0.080,
            repeat_attack_threshold: 0.75,
            vibrato_max_excursion_seconds: 0.150,
        }
    }
}

pub fn clean_notes(raw_notes: &[RawNoteEvent], settings: &CleanupSettings) -> Vec<NoteEvent> {
    let confident: Vec<RawNoteEvent> = raw_notes
        .iter()
        .copied()
        .filter(|note| note.confidence >= settings.min_confidence && note.end > note.start)
        .collect();
    let monophonic = reduce_to_monophonic(confident);
    let merged = merge_identical_neighbours(monophonic, settings);
    let without_vibrato = absorb_vibrato(merged, settings);
    let merged_again = merge_identical_neighbours(without_vibrato, settings);
    merged_again
        .into_iter()
        .filter(|note| note.end - note.start >= settings.min_note_seconds)
        .map(|note| NoteEvent { start: note.start, end: note.end, midi: note.midi, confidence: note.confidence })
        .collect()
}

/// Resolves overlaps so at most one note sounds at a time: in every
/// elementary time slice the most confident active note wins (ties: earlier
/// start, then lower pitch). Slices won by the same source note are joined,
/// so a weaker note interrupted by a stronger one resumes as a separate
/// segment. Correct by construction, O(n^2) for the few hundred notes of a clip.
fn reduce_to_monophonic(notes: Vec<RawNoteEvent>) -> Vec<RawNoteEvent> {
    let mut boundaries: Vec<f64> = notes.iter().flat_map(|note| [note.start, note.end]).collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup();

    let mut line: Vec<RawNoteEvent> = Vec::new();
    let mut previous_winner: Option<usize> = None;
    for slice in boundaries.windows(2) {
        let (slice_start, slice_end) = (slice[0], slice[1]);
        let winner = notes
            .iter()
            .enumerate()
            .filter(|(_, note)| note.start <= slice_start && note.end >= slice_end)
            .max_by(|(_, a), (_, b)| {
                a.confidence
                    .total_cmp(&b.confidence)
                    .then(b.start.total_cmp(&a.start))
                    .then(b.midi.cmp(&a.midi))
            })
            .map(|(index, _)| index);

        match (winner, previous_winner) {
            (Some(index), Some(previous)) if index == previous => {
                line.last_mut().expect("previous winner was pushed").end = slice_end;
            }
            (Some(index), _) => {
                let source = notes[index];
                // Only a segment beginning at the note's own onset keeps the
                // attack; a resumed tail or a note unmasked mid-way does not.
                let attack = if source.start == slice_start { source.attack } else { None };
                line.push(RawNoteEvent { start: slice_start, end: slice_end, attack, ..source });
            }
            (None, _) => {}
        }
        previous_winner = winner;
    }
    line
}

/// Rule 3/5: joins same-pitch neighbours separated by a tiny gap (a
/// transcription break), keeps them apart when the gap is long or the
/// second note has its own attack (a real repeated note).
fn merge_identical_neighbours(notes: Vec<RawNoteEvent>, settings: &CleanupSettings) -> Vec<RawNoteEvent> {
    let mut merged: Vec<RawNoteEvent> = Vec::with_capacity(notes.len());
    for note in notes {
        match merged.last_mut() {
            Some(previous)
                if previous.midi == note.midi
                    && note.start - previous.end <= settings.merge_gap_seconds
                    && !note.has_real_attack(settings.repeat_attack_threshold) =>
            {
                *previous = combine(previous, &note);
            }
            _ => merged.push(note),
        }
    }
    merged
}

/// Rule 4: A, (A+/-1 short), A  ->  one long A.
fn absorb_vibrato(notes: Vec<RawNoteEvent>, settings: &CleanupSettings) -> Vec<RawNoteEvent> {
    let mut result: Vec<RawNoteEvent> = Vec::with_capacity(notes.len());
    let mut index = 0;
    while index < notes.len() {
        let is_vibrato_excursion = index + 2 <= notes.len() - 1
            && is_vibrato_triple(&notes[index], &notes[index + 1], &notes[index + 2], settings);
        if is_vibrato_excursion {
            let combined = combine(&combine(&notes[index], &notes[index + 1]), &notes[index + 2]);
            let combined = RawNoteEvent { midi: notes[index].midi, ..combined };
            // Feed the result back in so chained wobbles collapse fully.
            let mut remaining = vec![combined];
            remaining.extend_from_slice(&notes[index + 3..]);
            let mut collapsed = absorb_vibrato(remaining, settings);
            result.append(&mut collapsed);
            return result;
        }
        result.push(notes[index]);
        index += 1;
    }
    result
}

fn is_vibrato_triple(first: &RawNoteEvent, middle: &RawNoteEvent, last: &RawNoteEvent, settings: &CleanupSettings) -> bool {
    let contiguous = middle.start - first.end <= settings.merge_gap_seconds
        && last.start - middle.end <= settings.merge_gap_seconds;
    first.midi == last.midi
        && first.midi.abs_diff(middle.midi) == 1
        && middle.end - middle.start <= settings.vibrato_max_excursion_seconds
        && contiguous
        // A re-attacked return to the main pitch is a real note (e.g. a trill-like figure).
        && !last.has_real_attack(settings.repeat_attack_threshold)
}

/// Duration-weighted confidence; keeps the first note's pitch and attack.
fn combine(first: &RawNoteEvent, second: &RawNoteEvent) -> RawNoteEvent {
    let first_length = first.end - first.start;
    let second_length = second.end - second.start;
    let total_length = (first_length + second_length).max(f64::EPSILON);
    RawNoteEvent {
        start: first.start,
        end: second.end.max(first.end),
        midi: first.midi,
        confidence: (first.confidence * first_length + second.confidence * second_length) / total_length,
        attack: first.attack,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::validate_monophonic_sequence;

    fn note(start: f64, end: f64, midi: u8, confidence: f64) -> RawNoteEvent {
        RawNoteEvent { start, end, midi, confidence, attack: None }
    }

    fn attacked(start: f64, end: f64, midi: u8, attack: f64) -> RawNoteEvent {
        RawNoteEvent { attack: Some(attack), ..note(start, end, midi, 0.9) }
    }

    fn pitches(notes: &[NoteEvent]) -> Vec<u8> {
        notes.iter().map(|n| n.midi).collect()
    }

    fn clean(raw: &[RawNoteEvent]) -> Vec<NoteEvent> {
        let cleaned = clean_notes(raw, &CleanupSettings::default());
        validate_monophonic_sequence(&cleaned).unwrap();
        cleaned
    }

    #[test]
    fn drops_low_confidence_and_short_notes() {
        let cleaned = clean(&[
            note(0.0, 0.5, 60, 0.9),
            note(0.6, 1.0, 62, 0.1),  // rule 1
            note(1.2, 1.24, 64, 0.9), // rule 2: 40 ms blip
            note(1.5, 2.0, 65, 0.9),
        ]);
        assert_eq!(pitches(&cleaned), vec![60, 65]);
    }

    #[test]
    fn merges_split_identical_notes_but_keeps_real_repeats() {
        let cleaned = clean(&[
            note(0.0, 0.5, 60, 0.8),
            note(0.55, 1.0, 60, 0.6), // 50 ms gap: same note (rule 3)
            note(1.3, 1.8, 60, 0.8),  // 300 ms gap: real repeat (rule 5)
        ]);
        assert_eq!(cleaned.len(), 2);
        assert_eq!((cleaned[0].start, cleaned[0].end), (0.0, 1.0));
        assert!((cleaned[0].confidence - 0.7).abs() < 0.02, "duration-weighted");
    }

    #[test]
    fn keeps_repeat_with_real_attack_despite_short_gap() {
        // Rule 5: 50 ms gap, but the second note has its own onset.
        let cleaned = clean(&[attacked(0.0, 0.5, 64, 0.9), attacked(0.55, 1.0, 64, 0.8)]);
        assert_eq!(cleaned.len(), 2);
        // A weak onset (vibrato-level, 0.55) is a transcription wobble: merge.
        let cleaned = clean(&[attacked(0.0, 0.5, 64, 0.9), attacked(0.55, 1.0, 64, 0.55)]);
        assert_eq!(cleaned.len(), 1);
    }

    #[test]
    fn short_note_next_to_same_pitch_survives_by_merging() {
        // Rule 2 exception: a 40 ms fragment continuing the same pitch.
        let cleaned = clean(&[note(0.0, 0.5, 67, 0.9), note(0.52, 0.56, 67, 0.9)]);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0].end, 0.56);
    }

    #[test]
    fn absorbs_vibrato_wobble() {
        let cleaned = clean(&[
            note(0.0, 0.4, 64, 0.9),
            note(0.4, 0.5, 65, 0.5), // 100 ms, +1 semitone
            note(0.5, 0.9, 64, 0.9),
            note(0.9, 1.0, 63, 0.5), // chained -1 wobble
            note(1.0, 1.4, 64, 0.9),
        ]);
        assert_eq!(pitches(&cleaned), vec![64]);
        assert_eq!((cleaned[0].start, cleaned[0].end), (0.0, 1.4));
    }

    #[test]
    fn keeps_real_stepwise_melody() {
        // Long semitone steps are melody, not vibrato.
        let cleaned = clean(&[note(0.0, 0.5, 64, 0.9), note(0.5, 1.0, 65, 0.9), note(1.0, 1.5, 64, 0.9)]);
        assert_eq!(pitches(&cleaned), vec![64, 65, 64]);
    }

    #[test]
    fn resolves_polyphonic_overlap_by_confidence() {
        let cleaned = clean(&[
            note(0.0, 1.0, 60, 0.5),
            note(0.3, 0.6, 72, 0.9), // stronger octave harmonic in the middle
            note(0.1, 0.4, 55, 0.2), // weak, below threshold
        ]);
        assert_eq!(pitches(&cleaned), vec![60, 72, 60]);
        assert_eq!((cleaned[1].start, cleaned[1].end), (0.3, 0.6));
        assert_eq!((cleaned[2].start, cleaned[2].end), (0.6, 1.0));
    }

    #[test]
    fn weaker_overlap_keeps_only_its_tail() {
        let cleaned = clean(&[note(0.0, 1.0, 60, 0.9), note(0.5, 1.5, 62, 0.6)]);
        assert_eq!(pitches(&cleaned), vec![60, 62]);
        assert_eq!(cleaned[1].start, 1.0);
    }

    /// Deterministic LCG so the fuzz test needs no extra crate.
    struct Lcg(u64);
    impl Lcg {
        fn next_unit(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    #[test]
    fn fuzz_polyphonic_input_always_yields_valid_monophonic_line() {
        let mut rng = Lcg(42);
        for case in 0..5_000 {
            let note_count = 1 + (rng.next_unit() * 40.0) as usize;
            let raw: Vec<RawNoteEvent> = (0..note_count)
                .map(|_| {
                    let start = (rng.next_unit() * 10.0 * 100.0).round() / 100.0;
                    let length = 0.01 + (rng.next_unit() * 1.5 * 100.0).round() / 100.0;
                    RawNoteEvent {
                        start,
                        end: start + length,
                        midi: 58 + (rng.next_unit() * 8.0) as u8,
                        confidence: (rng.next_unit() * 100.0).round() / 100.0,
                        attack: (rng.next_unit() > 0.5).then(|| rng.next_unit()),
                    }
                })
                .collect();
            let cleaned = clean_notes(&raw, &CleanupSettings::default());
            if let Err(error) = validate_monophonic_sequence(&cleaned) {
                panic!("case {case}: {error}\nraw: {raw:?}\ncleaned: {cleaned:?}");
            }
        }
    }

    #[test]
    fn empty_input_is_fine() {
        assert!(clean(&[]).is_empty());
    }
}

//! Deterministic note cleanup (spec "Note cleanup").
//!
//! Basic Pitch is polyphonic and reacts to vibrato, slides and harmonics.
//! The AE-01 plays one note at a time, so raw events are reduced to a clean
//! monophonic line here. Pitches come from raw events or a sufficiently steady
//! measured f0; evidence-based corrections are recorded for review.

use serde::{Deserialize, Serialize};

use crate::analysis::evidence::{NoteEvidence, PitchTrack};
use crate::music::notes::NoteEvent;

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
    /// Rule 2: shorter events need measured pitch support or their own attack.
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
    /// Evidence rules (only with a pitch track). Thresholds come from
    /// PooveSempoove (real song): bleed notes sat ~70 dB below loud singing,
    /// sung notes within ~8 dB of it.
    pub evidence: EvidenceSettings,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceSettings {
    /// Lead stem this far below loud singing is separation bleed: drop.
    pub bleed_below_loud_db: f64,
    /// Short, audible but unvoiced notes (consonants, breath) are unpitched.
    pub unpitched_max_seconds: f64,
    pub unpitched_max_voiced_fraction: f64,
    /// ...unless pYIN's decoded f0 matches the note's pitch (within
    /// `unpitched_pitch_tolerance`) on at least this share of frames. Real
    /// fast notes can get low voicing probability with the right decoded
    /// pitch (validation: a 0.93-attack sung note decoded at +0.09 semitone
    /// with voicing probability 0.01-0.37); consonants and breath decode no
    /// matching pitch.
    pub unpitched_rescue_min_pitch_agreement: f64,
    pub unpitched_pitch_tolerance: f64,
    /// Pitch evidence is trusted only this voiced and this steady.
    pub trusted_voiced_fraction: f64,
    pub trusted_max_spread: f64,
    /// Transcribed pitch this far from the sung f0 is wrong: re-pitch.
    pub repitch_min_deviation: f64,
    /// Neighbours whose sung f0 centres are this close are one sung tone...
    pub same_tone_max_center_difference: f64,
    /// ...if the pitch across both stays within this band (vibrato width).
    pub same_tone_max_joint_spread: f64,
    /// A note this short whose f0 sweeps at least `slide_min_spread` between
    /// its neighbours, without its own attack, is a slide into the next note.
    pub slide_max_seconds: f64,
    pub slide_min_spread: f64,
    /// Overlapping candidates: measured pitch may override transcription
    /// confidence only over at least this many pitch-track hops (pYIN's
    /// ~93 ms window cannot resolve shorter transitions)...
    pub candidate_min_hops: f64,
    /// ...when a candidate lies this close (semitones) to the measured centre
    /// (a centre between two semitones decides nothing)...
    pub candidate_match_tolerance: f64,
    /// ...and, for candidates an octave apart from the confidence winner,
    /// only with this much voicing, because pYIN itself makes octave errors.
    pub octave_choice_min_voiced_fraction: f64,
    /// A short note at least this far (semitones) from BOTH neighbours,
    /// without its own attack and without trusted pitch evidence, is not
    /// treated as melody: on its own it could force a whole-song octave
    /// transposition. (Provenance only: observed on a real recording where
    /// one weak 116 ms event two octaves above the melody did this. The rule
    /// itself uses no song, timestamp, pitch or instrument.)
    pub register_outlier_semitones: u8,
    pub register_outlier_max_seconds: f64,
}

impl Default for EvidenceSettings {
    fn default() -> Self {
        EvidenceSettings {
            bleed_below_loud_db: -35.0,
            unpitched_max_seconds: 0.12,
            unpitched_max_voiced_fraction: 0.2,
            unpitched_rescue_min_pitch_agreement: 0.5,
            unpitched_pitch_tolerance: 0.5,
            trusted_voiced_fraction: 0.6,
            trusted_max_spread: 1.0,
            repitch_min_deviation: 0.8,
            same_tone_max_center_difference: 0.6,
            same_tone_max_joint_spread: 1.2,
            slide_max_seconds: 0.15,
            slide_min_spread: 0.8,
            candidate_min_hops: 3.0,
            // 0.35 accepted centres like 61.34 / 60.69 that an independent CQT
            // check on Poove did not support; 0.25 keeps the clear 58.09 case.
            candidate_match_tolerance: 0.25,
            octave_choice_min_voiced_fraction: 0.85,
            register_outlier_semitones: 12,
            register_outlier_max_seconds: 0.15,
        }
    }
}

/// What evidence-based cleanup did to one note (written to work/ for review).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanupDecision {
    pub start: f64,
    pub end: f64,
    pub midi: u8,
    pub action: &'static str,
    pub detail: String,
}

pub struct CleanupOutcome {
    pub notes: Vec<NoteEvent>,
    pub decisions: Vec<CleanupDecision>,
}

impl Default for CleanupSettings {
    fn default() -> Self {
        CleanupSettings {
            min_confidence: 0.3,
            min_note_seconds: 0.060,
            merge_gap_seconds: 0.080,
            repeat_attack_threshold: 0.75,
            vibrato_max_excursion_seconds: 0.150,
            evidence: EvidenceSettings::default(),
        }
    }
}

/// Duration-only cleanup (no audio evidence available).
pub fn clean_notes(raw_notes: &[RawNoteEvent], settings: &CleanupSettings) -> Vec<NoteEvent> {
    clean_notes_with_evidence(raw_notes, settings, None).notes
}

/// Full cleanup. With a pitch track, audio evidence first removes bleed and
/// unpitched noise, fixes clearly wrong pitches, joins split sung tones and
/// absorbs slides; the duration rules retain that evidence. Short notes that
/// the evidence shows are real (steady pitch or their own attack) stay:
/// ornaments and deliberate repeats are not deleted for being short.
pub fn clean_notes_with_evidence(
    raw_notes: &[RawNoteEvent],
    settings: &CleanupSettings,
    track: Option<&PitchTrack>,
) -> CleanupOutcome {
    let confident: Vec<RawNoteEvent> = raw_notes
        .iter()
        .copied()
        .filter(|note| note.confidence >= settings.min_confidence && note.end > note.start)
        .collect();
    let mut decisions = Vec::new();
    let usable_track = track.filter(|track| track.is_consistent());
    let evidence = usable_track.map(|track| Evidence {
        track,
        loud_reference_db: track.loud_reference_db(),
        settings: &settings.evidence,
    });

    let candidates = match &evidence {
        Some(evidence) => evidence.drop_unsung(confident, &mut decisions),
        None => confident,
    };
    let mut line = reduce_to_monophonic(candidates, evidence.as_ref(), &mut decisions);
    if let Some(evidence) = &evidence {
        line = evidence.repitch_mismatches(line, &mut decisions);
        line = evidence.merge_same_sung_tone(line, settings, &mut decisions);
        line = evidence.absorb_slides(line, settings, &mut decisions);
        line = evidence.drop_register_outliers(line, settings, &mut decisions);
    }
    let notes = duration_rules(line, settings, evidence.as_ref(), &mut decisions);
    CleanupOutcome { notes, decisions }
}

fn duration_rules(
    monophonic: Vec<RawNoteEvent>,
    settings: &CleanupSettings,
    evidence: Option<&Evidence<'_>>,
    decisions: &mut Vec<CleanupDecision>,
) -> Vec<NoteEvent> {
    let merged = merge_identical_neighbours(monophonic, settings, evidence);
    let without_vibrato = absorb_vibrato(merged, settings, evidence, decisions);
    let merged_again = merge_identical_neighbours(without_vibrato, settings, evidence);
    merged_again
        .into_iter()
        .filter(|note| {
            let keep = note.end - note.start >= settings.min_note_seconds
                || note.has_real_attack(settings.repeat_attack_threshold)
                || evidence.is_some_and(|e| e.supports_pitch(note));
            if !keep {
                decisions.push(decision(
                    note,
                    "dropped-short",
                    "short fragment without a strong attack or steady matching pitch".into(),
                ));
            }
            keep
        })
        .map(|note| NoteEvent {
            start: note.start,
            end: note.end,
            midi: note.midi,
            confidence: note.confidence,
        })
        .collect()
}

struct Evidence<'a> {
    track: &'a PitchTrack,
    loud_reference_db: f64,
    settings: &'a EvidenceSettings,
}

impl Evidence<'_> {
    fn supports_pitch(&self, note: &RawNoteEvent) -> bool {
        self.of(note)
            .and_then(|e| self.trusted_center(&e))
            .is_some_and(|center| (center - note.midi as f64).abs() < 0.5)
    }

    /// Require several measured frames: a sub-frame boundary is not a rest.
    fn silent_gap(&self, start: f64, end: f64) -> bool {
        end - start >= 3.0 * self.track.hop_seconds
            && self
                .track
                .evidence(start, end, self.loud_reference_db)
                .is_some_and(|e| e.level_below_loud_db < self.settings.bleed_below_loud_db)
    }

    fn of(&self, note: &RawNoteEvent) -> Option<NoteEvidence> {
        self.track
            .evidence(note.start, note.end, self.loud_reference_db)
    }

    /// Among overlapping transcription candidates, the one the audio shows is
    /// sounding, when that differs from the most confident one. Conservative:
    /// long enough slice, trusted (voiced, steady, audible) pitch, a candidate
    /// within tolerance of the measured centre, and stronger voicing for
    /// octave-apart choices. Returns (candidate index, reason).
    /// pYIN decoded this note's pitch on most of its frames, even if it gave
    /// them a low voicing probability.
    fn decoded_pitch_matches(&self, note: &RawNoteEvent) -> bool {
        self.track
            .pitch_agreement(
                note.start,
                note.end,
                note.midi,
                self.settings.unpitched_pitch_tolerance,
            )
            .is_some_and(|share| share >= self.settings.unpitched_rescue_min_pitch_agreement)
    }

    /// Long enough for pYIN's ~93 ms window to say anything about it.
    fn resolvable(&self, start: f64, end: f64) -> bool {
        end - start >= self.settings.candidate_min_hops * self.track.hop_seconds
    }

    fn measured_candidate(
        &self,
        notes: &[RawNoteEvent],
        active: &[usize],
        confident: usize,
        start: f64,
        end: f64,
    ) -> Option<(usize, String)> {
        if !self.resolvable(start, end) {
            return None;
        }
        let evidence = self.track.evidence(start, end, self.loud_reference_db)?;
        if evidence.level_below_loud_db < self.settings.bleed_below_loud_db {
            return None;
        }
        let center = self.trusted_center(&evidence)?;
        let chosen = active
            .iter()
            .copied()
            .filter(|&index| {
                (notes[index].midi as f64 - center).abs() <= self.settings.candidate_match_tolerance
            })
            .max_by(|&a, &b| notes[a].confidence.total_cmp(&notes[b].confidence))?;
        if chosen == confident || notes[chosen].midi == notes[confident].midi {
            return None;
        }
        let octave_apart = notes[chosen].midi.abs_diff(notes[confident].midi) % 12 == 0;
        if octave_apart
            && evidence.voiced_fraction < self.settings.octave_choice_min_voiced_fraction
        {
            return None;
        }
        Some((
            chosen,
            format!(
                "measured {center:.2} over {:.0} ms; confidence {:.2} vs {:.2}",
                (end - start) * 1000.0,
                notes[confident].confidence,
                notes[chosen].confidence
            ),
        ))
    }

    /// Evidence good enough to state the sung pitch of this span.
    fn trusted_center(&self, evidence: &NoteEvidence) -> Option<f64> {
        let steady = evidence
            .f0_spread
            .is_some_and(|spread| spread <= self.settings.trusted_max_spread);
        (evidence.voiced_fraction >= self.settings.trusted_voiced_fraction && steady)
            .then_some(evidence.f0_center)
            .flatten()
    }

    /// Bleed (lead stem near-silent) and short unvoiced noise are not melody.
    fn drop_unsung(
        &self,
        notes: Vec<RawNoteEvent>,
        decisions: &mut Vec<CleanupDecision>,
    ) -> Vec<RawNoteEvent> {
        notes
            .into_iter()
            .filter(|note| {
                let Some(evidence) = self.of(note) else {
                    return true;
                };
                let reason = if evidence.level_below_loud_db < self.settings.bleed_below_loud_db {
                    Some((
                        "dropped-bleed",
                        format!(
                            "lead stem {:.0} dB below loud singing",
                            evidence.level_below_loud_db
                        ),
                    ))
                } else if note.end - note.start <= self.settings.unpitched_max_seconds
                    && evidence.voiced_fraction < self.settings.unpitched_max_voiced_fraction
                    && !self.decoded_pitch_matches(note)
                {
                    Some((
                        "dropped-unpitched",
                        format!("{:.0}% voiced frames", evidence.voiced_fraction * 100.0),
                    ))
                } else {
                    None
                };
                match reason {
                    Some((action, detail)) => {
                        decisions.push(decision(note, action, detail));
                        false
                    }
                    None => true,
                }
            })
            .collect()
    }

    /// Transcribed pitch clearly disagreeing with a steady sung f0. Octave
    /// disagreements keep the transcription: pYIN itself often errs by an
    /// octave, and neither source is clearly better there.
    fn repitch_mismatches(
        &self,
        notes: Vec<RawNoteEvent>,
        decisions: &mut Vec<CleanupDecision>,
    ) -> Vec<RawNoteEvent> {
        notes
            .into_iter()
            .map(|note| {
                // pYIN cannot resolve events shorter than a few hops; the
                // transcription's pitch stands for those.
                if !self.resolvable(note.start, note.end) {
                    return note;
                }
                let Some(center) = self.of(&note).and_then(|e| self.trusted_center(&e)) else {
                    return note;
                };
                let deviation = center - note.midi as f64;
                let octave_error = (deviation.abs() - 12.0).abs() <= 0.6;
                if deviation.abs() < self.settings.repitch_min_deviation || octave_error {
                    return note;
                }
                let sung = center.round().clamp(0.0, 127.0) as u8;
                decisions.push(decision(
                    &note,
                    "repitched",
                    format!("sung f0 {center:.2} -> MIDI {sung}"),
                ));
                RawNoteEvent { midi: sung, ..note }
            })
            .collect()
    }

    /// One wavering sung tone split into neighbouring pitches (e.g. a singer
    /// between D and D#): joined when both halves sit on the same f0 centre
    /// and the second has no attack of its own. The joined pitch is whichever
    /// transcribed pitch is nearest the sung centre.
    fn merge_same_sung_tone(
        &self,
        notes: Vec<RawNoteEvent>,
        settings: &CleanupSettings,
        decisions: &mut Vec<CleanupDecision>,
    ) -> Vec<RawNoteEvent> {
        let mut merged: Vec<RawNoteEvent> = Vec::with_capacity(notes.len());
        for note in notes {
            if let Some(previous) = merged.last_mut() {
                if let Some(center) = self.same_tone_center(previous, &note, settings) {
                    let midi = [previous.midi, note.midi]
                        .into_iter()
                        .min_by(|a, b| {
                            (*a as f64 - center)
                                .abs()
                                .total_cmp(&(*b as f64 - center).abs())
                        })
                        .expect("two candidates");
                    decisions.push(decision(
                        &note,
                        "merged-same-tone",
                        format!(
                            "sung centre {center:.2}; joined with MIDI {} at {:.2}s",
                            previous.midi, previous.start
                        ),
                    ));
                    *previous = RawNoteEvent {
                        midi,
                        ..combine(previous, &note)
                    };
                    continue;
                }
            }
            merged.push(note);
        }
        merged
    }

    fn same_tone_center(
        &self,
        previous: &RawNoteEvent,
        note: &RawNoteEvent,
        settings: &CleanupSettings,
    ) -> Option<f64> {
        let different_pitch = previous.midi != note.midi;
        let adjacent = note.start - previous.end <= settings.merge_gap_seconds;
        if !different_pitch
            || !adjacent
            || note.has_real_attack(settings.repeat_attack_threshold)
            || self.silent_gap(previous.end, note.start)
        {
            return None;
        }
        // A deliberately attacked fragment shorter than pYIN can resolve has
        // no measured tone of its own: its neighbour's pitch must not absorb
        // it. (The second note's attack is already checked above; weak,
        // unresolvable flickers inside one continuous tone may still join.)
        if previous.has_real_attack(settings.repeat_attack_threshold)
            && !self.resolvable(previous.start, previous.end)
        {
            return None;
        }
        let first = self.of(previous)?;
        let second = self.of(note)?;
        let voiced = |e: &NoteEvidence| e.voiced_fraction >= self.settings.trusted_voiced_fraction;
        if !voiced(&first) || !voiced(&second) {
            return None;
        }
        // Joining transcribed pitches an octave apart is an octave decision
        // taken from pYIN alone: demand the same voicing as elsewhere.
        let octave_apart = previous.midi.abs_diff(note.midi) % 12 == 0;
        let octave_voicing = self.settings.octave_choice_min_voiced_fraction;
        if octave_apart
            && (first.voiced_fraction < octave_voicing || second.voiced_fraction < octave_voicing)
        {
            return None;
        }
        let close = (first.f0_center? - second.f0_center?).abs()
            <= self.settings.same_tone_max_center_difference;
        let joint = self
            .track
            .evidence(previous.start, note.end, self.loud_reference_db)?;
        let narrow = joint.f0_spread? <= self.settings.same_tone_max_joint_spread;
        (close && narrow).then_some(joint.f0_center).flatten()
    }

    /// Weak, short events an octave or more away from both neighbours. Real
    /// ornaments and grace notes sit close in register, or are voiced, or
    /// have their own attack; any one of those keeps the note.
    fn drop_register_outliers(
        &self,
        notes: Vec<RawNoteEvent>,
        settings: &CleanupSettings,
        decisions: &mut Vec<CleanupDecision>,
    ) -> Vec<RawNoteEvent> {
        let limit = self.settings.register_outlier_semitones;
        let is_outlier = |index: usize| {
            let note = &notes[index];
            let (Some(previous), Some(next)) = (
                index.checked_sub(1).map(|i| &notes[i]),
                notes.get(index + 1),
            ) else {
                return false; // the first/last note has no register context on one side
            };
            let far = note.midi.abs_diff(previous.midi) >= limit
                && note.midi.abs_diff(next.midi) >= limit;
            let short = note.end - note.start <= self.settings.register_outlier_max_seconds;
            let confirmed = self
                .of(note)
                .and_then(|e| self.trusted_center(&e))
                .is_some();
            far && short && !confirmed && !note.has_real_attack(settings.repeat_attack_threshold)
        };
        let outliers: Vec<bool> = (0..notes.len()).map(is_outlier).collect();
        notes
            .into_iter()
            .zip(outliers)
            .filter_map(|(note, outlier)| {
                if outlier {
                    decisions.push(decision(
                        &note,
                        "dropped-register-outlier",
                        "short, unattacked, unconfirmed by pitch evidence, an octave or more from both neighbours".into(),
                    ));
                    None
                } else {
                    Some(note)
                }
            })
            .collect()
    }

    /// Short pitch sweep between two notes without its own attack: a slide
    /// into the next note, which starts where the slide started. A short
    /// note with steady pitch or a real attack is an ornament and stays.
    fn absorb_slides(
        &self,
        notes: Vec<RawNoteEvent>,
        settings: &CleanupSettings,
        decisions: &mut Vec<CleanupDecision>,
    ) -> Vec<RawNoteEvent> {
        let mut result: Vec<RawNoteEvent> = Vec::with_capacity(notes.len());
        let mut index = 0;
        while index < notes.len() {
            let note = notes[index];
            let next = notes.get(index + 1);
            let is_slide = next.is_some_and(|next| {
                next.start - note.end <= settings.merge_gap_seconds
                    && self.is_slide(result.last(), &note, next, settings)
            });
            if let (true, Some(next)) = (is_slide, next) {
                decisions.push(decision(
                    &note,
                    "absorbed-slide",
                    format!("pitch sweep into MIDI {} at {:.2}s", next.midi, next.start),
                ));
                result.push(RawNoteEvent {
                    start: note.start,
                    ..*next
                });
                index += 2;
                continue;
            }
            result.push(note);
            index += 1;
        }
        result
    }

    fn is_slide(
        &self,
        previous: Option<&RawNoteEvent>,
        note: &RawNoteEvent,
        next: &RawNoteEvent,
        settings: &CleanupSettings,
    ) -> bool {
        let short = note.end - note.start <= self.settings.slide_max_seconds;
        if !short || note.has_real_attack(settings.repeat_attack_threshold) {
            return false;
        }
        let Some(evidence) = self.of(note) else {
            return false;
        };
        let sweeping = evidence
            .f0_spread
            .is_some_and(|spread| spread >= self.settings.slide_min_spread);
        let Some(center) = evidence.f0_center else {
            return false;
        };
        // The sweep must head towards the next note: its centre lies between
        // where it came from and where it lands.
        let from = previous.map_or(note.midi as f64, |p| p.midi as f64);
        let to = next.midi as f64;
        let between = center >= from.min(to) - 0.5 && center <= from.max(to) + 0.5;
        sweeping && between && from != to
    }
}

fn decision(note: &RawNoteEvent, action: &'static str, detail: String) -> CleanupDecision {
    CleanupDecision {
        start: note.start,
        end: note.end,
        midi: note.midi,
        action,
        detail,
    }
}

/// Resolves overlaps so at most one note sounds at a time: in every
/// elementary time slice the most confident active note wins (ties: earlier
/// start, then lower pitch), unless trusted pitch evidence identifies a
/// different EXISTING candidate as the one actually sounding (see
/// `Evidence::measured_candidate`). Slices won by the same source note are
/// joined, so a weaker note interrupted by a stronger one resumes as a
/// separate segment. Correct by construction, O(n^2) for a clip's notes.
fn reduce_to_monophonic(
    notes: Vec<RawNoteEvent>,
    evidence: Option<&Evidence<'_>>,
    decisions: &mut Vec<CleanupDecision>,
) -> Vec<RawNoteEvent> {
    let mut boundaries: Vec<f64> = notes
        .iter()
        .flat_map(|note| [note.start, note.end])
        .collect();
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup();

    let mut line: Vec<RawNoteEvent> = Vec::new();
    let mut previous_winner: Option<usize> = None;
    for slice in boundaries.windows(2) {
        let (slice_start, slice_end) = (slice[0], slice[1]);
        let active: Vec<usize> = (0..notes.len())
            .filter(|&index| notes[index].start <= slice_start && notes[index].end >= slice_end)
            .collect();
        let by_confidence = active.iter().copied().max_by(|&a, &b| {
            let (a, b) = (&notes[a], &notes[b]);
            a.confidence
                .total_cmp(&b.confidence)
                .then(b.start.total_cmp(&a.start))
                .then(b.midi.cmp(&a.midi))
        });
        let measured = match (evidence, by_confidence) {
            (Some(evidence), Some(confident)) if active.len() >= 2 => {
                evidence.measured_candidate(&notes, &active, confident, slice_start, slice_end)
            }
            _ => None,
        };
        if let (Some((chosen, detail)), Some(confident)) = (&measured, by_confidence) {
            record_measured_choice(
                decisions,
                &notes[confident],
                slice_start,
                slice_end,
                notes[*chosen].midi,
                detail,
            );
        }
        let winner = measured.map(|(chosen, _)| chosen).or(by_confidence);

        match (winner, previous_winner) {
            (Some(index), Some(previous)) if index == previous => {
                line.last_mut().expect("previous winner was pushed").end = slice_end;
            }
            (Some(index), _) => {
                let source = notes[index];
                // Only a segment beginning at the note's own onset keeps the
                // attack; a resumed tail or a note unmasked mid-way does not.
                let attack = if source.start == slice_start {
                    source.attack
                } else {
                    None
                };
                line.push(RawNoteEvent {
                    start: slice_start,
                    end: slice_end,
                    attack,
                    ..source
                });
            }
            (None, _) => {}
        }
        previous_winner = winner;
    }
    line
}

/// One decision per contiguous run of slices where evidence overrode the
/// confidence winner (runs are extended instead of logging every slice).
fn record_measured_choice(
    decisions: &mut Vec<CleanupDecision>,
    overridden: &RawNoteEvent,
    start: f64,
    end: f64,
    chosen_midi: u8,
    detail: &str,
) {
    if let Some(last) = decisions.last_mut() {
        if last.action == "chose-measured-candidate"
            && last.end == start
            && last.midi == overridden.midi
        {
            last.end = end;
            return;
        }
    }
    decisions.push(CleanupDecision {
        start,
        end,
        midi: overridden.midi,
        action: "chose-measured-candidate",
        detail: format!("{detail}; kept overlapping MIDI {chosen_midi} instead"),
    });
}

/// Rule 3/5: joins same-pitch neighbours separated by a tiny gap (a
/// transcription break), keeps them apart when the gap is long or the
/// second note has its own attack (a real repeated note).
fn merge_identical_neighbours(
    notes: Vec<RawNoteEvent>,
    settings: &CleanupSettings,
    evidence: Option<&Evidence<'_>>,
) -> Vec<RawNoteEvent> {
    let mut merged: Vec<RawNoteEvent> = Vec::with_capacity(notes.len());
    for note in notes {
        match merged.last_mut() {
            Some(previous)
                if previous.midi == note.midi
                    && note.start - previous.end <= settings.merge_gap_seconds
                    && !evidence.is_some_and(|e| e.silent_gap(previous.end, note.start))
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
fn absorb_vibrato(
    notes: Vec<RawNoteEvent>,
    settings: &CleanupSettings,
    evidence: Option<&Evidence<'_>>,
    decisions: &mut Vec<CleanupDecision>,
) -> Vec<RawNoteEvent> {
    let mut result: Vec<RawNoteEvent> = Vec::with_capacity(notes.len());
    let mut index = 0;
    while index < notes.len() {
        let is_vibrato_excursion = index + 2 <= notes.len() - 1
            && is_vibrato_triple(
                &notes[index],
                &notes[index + 1],
                &notes[index + 2],
                settings,
            )
            && evidence.is_none_or(|e| {
                // With audio available, duration alone must not erase a
                // semitone ornament. Require evidence for the surrounding tone.
                let middle = &notes[index + 1];
                let surrounding = RawNoteEvent {
                    midi: notes[index].midi,
                    ..*middle
                };
                e.supports_pitch(&surrounding)
                    && !e.silent_gap(notes[index].end, middle.start)
                    && !e.silent_gap(middle.end, notes[index + 2].start)
            });
        if is_vibrato_excursion {
            decisions.push(decision(
                &notes[index + 1],
                "absorbed-vibrato",
                "short weak excursion into the surrounding pitch".into(),
            ));
            let combined = combine(
                &combine(&notes[index], &notes[index + 1]),
                &notes[index + 2],
            );
            let combined = RawNoteEvent {
                midi: notes[index].midi,
                ..combined
            };
            // Feed the result back in so chained wobbles collapse fully.
            let mut remaining = vec![combined];
            remaining.extend_from_slice(&notes[index + 3..]);
            let mut collapsed = absorb_vibrato(remaining, settings, evidence, decisions);
            result.append(&mut collapsed);
            return result;
        }
        result.push(notes[index]);
        index += 1;
    }
    result
}

fn is_vibrato_triple(
    first: &RawNoteEvent,
    middle: &RawNoteEvent,
    last: &RawNoteEvent,
    settings: &CleanupSettings,
) -> bool {
    let contiguous = middle.start - first.end <= settings.merge_gap_seconds
        && last.start - middle.end <= settings.merge_gap_seconds;
    first.midi == last.midi
        && first.midi.abs_diff(middle.midi) == 1
        && middle.end - middle.start <= settings.vibrato_max_excursion_seconds
        && contiguous
        && !middle.has_real_attack(settings.repeat_attack_threshold)
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
        confidence: (first.confidence * first_length + second.confidence * second_length)
            / total_length,
        attack: first.attack,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::notes::validate_monophonic_sequence;

    fn note(start: f64, end: f64, midi: u8, confidence: f64) -> RawNoteEvent {
        RawNoteEvent {
            start,
            end,
            midi,
            confidence,
            attack: None,
        }
    }

    fn attacked(start: f64, end: f64, midi: u8, attack: f64) -> RawNoteEvent {
        RawNoteEvent {
            attack: Some(attack),
            ..note(start, end, midi, 0.9)
        }
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
        assert!(
            (cleaned[0].confidence - 0.7).abs() < 0.02,
            "duration-weighted"
        );
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
        let cleaned = clean(&[
            note(0.0, 0.5, 64, 0.9),
            note(0.5, 1.0, 65, 0.9),
            note(1.0, 1.5, 64, 0.9),
        ]);
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
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
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

    // ---- evidence rules (pitch track built at 10 ms per frame) ----
    use crate::analysis::evidence::test_support::track;

    fn clean_with(raw: &[RawNoteEvent], track: &PitchTrack) -> CleanupOutcome {
        let outcome = clean_notes_with_evidence(raw, &CleanupSettings::default(), Some(track));
        validate_monophonic_sequence(&outcome.notes).unwrap();
        outcome
    }

    fn actions(outcome: &CleanupOutcome) -> Vec<&'static str> {
        outcome.decisions.iter().map(|d| d.action).collect()
    }

    const LOUD: f64 = -12.0;

    #[test]
    fn keeps_measured_semitone_ornament_even_without_strong_attack() {
        let measured = track(&[
            (0.5, Some(60.0), LOUD),
            (0.1, Some(61.0), LOUD),
            (0.5, Some(60.0), LOUD),
        ]);
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 60, 0.8),
                note(0.5, 0.6, 61, 0.7),
                note(0.6, 1.1, 60, 0.8),
            ],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![60, 61, 60]);
    }

    #[test]
    fn keeps_attacked_semitone_ornament_without_audio_evidence() {
        let outcome = clean(&[
            note(0.0, 0.5, 60, 0.8),
            attacked(0.5, 0.6, 61, 0.9),
            note(0.6, 1.1, 60, 0.8),
        ]);
        assert_eq!(pitches(&outcome), vec![60, 61, 60]);
    }

    #[test]
    fn keeps_repeated_notes_across_measured_silence() {
        let measured = track(&[
            (0.5, Some(60.0), LOUD),
            (0.06, None, -82.0),
            (0.5, Some(60.0), LOUD),
        ]);
        let outcome = clean_with(
            &[note(0.0, 0.5, 60, 0.8), note(0.56, 1.06, 60, 0.8)],
            &measured,
        );
        assert_eq!(outcome.notes.len(), 2);
        assert_eq!((outcome.notes[0].end, outcome.notes[1].start), (0.5, 0.56));
    }

    #[test]
    fn keeps_short_measured_note_below_duration_cutoff() {
        let measured = track(&[(0.5, Some(60.0), LOUD), (0.05, Some(62.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 0.5, 60, 0.8), note(0.5, 0.55, 62, 0.8)],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![60, 62]);
    }

    #[test]
    fn uncertain_pitch_does_not_authorize_vibrato_collapse() {
        let mut measured = track(&[(1.2, Some(60.0), LOUD)]);
        // Enough voicing to retain the event, but insufficient to decide
        // whether the semitone excursion is a deliberate ornament.
        for i in 50..64 {
            measured.f0_midi[i] = (i % 2 == 0).then_some(60.5);
            measured.voiced_probability[i] = if i % 2 == 0 { 0.9 } else { 0.02 };
        }
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 60, 0.8),
                note(0.5, 0.64, 61, 0.7),
                note(0.64, 1.2, 60, 0.8),
            ],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![60, 61, 60]);
    }

    // ---- repeated same-pitch notes: split vs re-attack ----
    //
    // Validation (161 same-pitch pairs, 8 recordings) found converging audio
    // evidence for 27 genuine re-attacks (level dip >= 9 dB plus a timbre
    // change) but only 3 continuous splits; 131 were ambiguous. Tono therefore
    // decides from the transcriber's attack and measured silence only, and
    // does not join same-pitch notes on weaker audio cues.

    #[test]
    fn zero_gap_repeat_with_strong_attack_stays_separate() {
        // Shape of the clear vocal re-attacks: no transcription gap, strong
        // onset on the second note, continuous measured pitch.
        let measured = track(&[(1.0, Some(59.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 0.45, 59, 0.8), attacked(0.45, 1.0, 59, 0.9)],
            &measured,
        );
        assert_eq!(outcome.notes.len(), 2);
        assert_eq!(outcome.notes[1].start, 0.45);
    }

    #[test]
    fn zero_gap_split_without_attack_is_joined() {
        // Shape of the clear continuous splits: no gap, weak onset, steady pitch.
        let measured = track(&[(1.2, Some(67.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 0.8, 67, 0.8), attacked(0.8, 1.2, 67, 0.3)],
            &measured,
        );
        assert_eq!(outcome.notes.len(), 1);
        assert_eq!((outcome.notes[0].start, outcome.notes[0].end), (0.0, 1.2));
    }

    #[test]
    fn short_fragment_before_attacked_same_pitch_note_is_left_separate() {
        // The ambiguous class (e.g. a rescued fragment where the pitch arrives
        // before the transcribed onset). Evidence could not tell an early
        // onset from a quick re-articulation, so both events are kept and the
        // fragment is not deleted: a documented limit, not a claimed fix.
        let mut measured = track(&[
            (0.5, Some(62.0), LOUD),
            (0.09, Some(64.0), LOUD),
            (0.5, Some(64.0), LOUD),
        ]);
        for i in 50..59 {
            measured.voiced_probability[i] = 0.1; // low voicing, decoded E4
        }
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 62, 0.8),
                attacked(0.5, 0.58, 64, 0.5),
                attacked(0.59, 1.09, 64, 0.9),
            ],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![62, 64, 64]);
    }

    #[test]
    fn continuous_pitch_across_small_gap_still_merges() {
        let measured = track(&[(1.1, Some(60.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 0.5, 60, 0.8), note(0.56, 1.06, 60, 0.8)],
            &measured,
        );
        assert_eq!(outcome.notes.len(), 1);
        assert_eq!((outcome.notes[0].start, outcome.notes[0].end), (0.0, 1.06));
    }

    // ---- overlapping candidates: measured pitch vs transcription confidence ----

    #[test]
    fn overlapping_accompaniment_loses_to_the_measured_lead() {
        // Poove-like: lead 58 (conf 0.46) overlapped by a louder-scored 62
        // (conf 0.56) while the lead stem measures 58.
        let measured = track(&[(1.0, Some(58.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 1.0, 58, 0.46), note(0.3, 0.6, 62, 0.56)],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![58]);
        assert!(actions(&outcome).contains(&"chose-measured-candidate"));
    }

    #[test]
    fn strong_measured_fundamental_beats_confident_octave_harmonic() {
        let measured = track(&[(1.0, Some(60.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 1.0, 60, 0.5), note(0.2, 0.6, 72, 0.8)],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![60]);
    }

    #[test]
    fn weakly_voiced_octave_disagreement_keeps_transcription_choice() {
        // Only ~70 % voiced: pYIN may itself be an octave off, so it must not
        // overrule the more confident candidate an octave away.
        let mut measured = track(&[(1.0, Some(60.0), LOUD)]);
        for i in (20..60).step_by(3) {
            measured.f0_midi[i] = None;
            measured.voiced_probability[i] = 0.02;
        }
        let outcome = clean_with(
            &[note(0.0, 1.0, 60, 0.5), note(0.2, 0.6, 72, 0.8)],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![60, 72, 60]);
    }

    #[test]
    fn short_or_ambiguous_overlaps_keep_transcription_choice() {
        // 20 ms overlap: shorter than three pitch-track hops.
        let measured = track(&[(1.0, Some(58.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 1.0, 58, 0.46), attacked(0.5, 0.52, 62, 0.9)],
            &measured,
        );
        assert!(pitches(&outcome.notes).contains(&62));
        // Measured centre between two semitones decides nothing.
        let between = track(&[(1.0, Some(60.5), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 1.0, 60, 0.5), note(0.3, 0.6, 61, 0.7)],
            &between,
        );
        assert!(!actions(&outcome).contains(&"chose-measured-candidate"));
    }

    #[test]
    fn measured_choice_never_invents_a_pitch() {
        // Measured 59 matches no candidate: confidence rule stands.
        let measured = track(&[(1.0, Some(59.0), LOUD)]);
        let outcome = clean_with(
            &[note(0.0, 1.0, 58, 0.46), note(0.3, 0.6, 62, 0.56)],
            &measured,
        );
        assert!(!actions(&outcome).contains(&"chose-measured-candidate"));
    }

    // ---- register outliers (Ek Pyar: one weak D6 forced a -12 transposition) ----

    fn with_attack(mut raw: RawNoteEvent, attack: f64) -> RawNoteEvent {
        raw.attack = Some(attack);
        raw
    }

    #[test]
    fn weak_isolated_two_octave_event_is_not_melody() {
        // D4/E4 melody, one quiet 116 ms D6 with weak voicing and attack 0.26.
        let mut measured = track(&[
            (0.5, Some(62.0), LOUD),
            (0.4, None, -80.0),
            (0.12, Some(74.0), -40.0),
            (0.4, None, -80.0),
            (0.5, Some(64.0), LOUD),
        ]);
        for i in 90..102 {
            measured.voiced_probability[i] = if i % 3 == 0 { 0.6 } else { 0.1 };
        }
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 62, 0.8),
                with_attack(note(0.9, 1.016, 86, 0.47), 0.26),
                note(1.42, 1.92, 64, 0.8),
            ],
            &measured,
        );
        assert_eq!(pitches(&outcome.notes), vec![62, 64]);
        assert_eq!(actions(&outcome), vec!["dropped-register-outlier"]);
    }

    #[test]
    fn register_leap_is_kept_when_attacked_voiced_long_or_near_a_neighbour() {
        let high = |f0: Option<f64>| {
            track(&[
                (0.5, Some(62.0), LOUD),
                (0.12, f0, LOUD),
                (0.5, Some(64.0), LOUD),
            ])
        };
        // Voiced at its own pitch: a real high note.
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 62, 0.8),
                note(0.5, 0.62, 86, 0.6),
                note(0.62, 1.12, 64, 0.8),
            ],
            &high(Some(86.0)),
        );
        assert!(pitches(&outcome.notes).contains(&86));
        // Strong attack: a deliberate event even if pYIN cannot confirm it.
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 62, 0.8),
                with_attack(note(0.5, 0.62, 86, 0.6), 0.9),
                note(0.62, 1.12, 64, 0.8),
            ],
            // Partly voiced like the real case (unconfirmed, but not noise).
            &{
                let mut partial = high(Some(74.0));
                for i in 50..62 {
                    if i % 3 != 0 {
                        partial.voiced_probability[i] = 0.1;
                    }
                }
                partial
            },
        );
        assert!(pitches(&outcome.notes).contains(&86));
        // Grace note a step away is never a register outlier.
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 62, 0.8),
                note(0.5, 0.62, 66, 0.6),
                note(0.62, 1.12, 64, 0.8),
            ],
            &high(None),
        );
        assert!(!actions(&outcome).contains(&"dropped-register-outlier"));
    }

    // ---- register-outlier preservation, independent of register and time ----
    //
    // Every case is built relative to an arbitrary base pitch and start time
    // and checked in low, middle and high registers (bass to piano range), so
    // the rule is shown to depend only on intervals, duration, attack and
    // evidence -- never on a particular song, pitch or instrument.

    const BASES: [u8; 3] = [30, 57, 84];
    const OFFSETS: [f64; 2] = [0.0, 7.3];

    struct Candidate {
        /// Semitones from the base (previous note) to the candidate.
        leap: i32,
        /// Semitones from the base to the following note.
        next: i32,
        seconds: f64,
        attack: Option<f64>,
        evidence: CandidateEvidence,
    }

    enum CandidateEvidence {
        /// Partly voiced (~1/3 of frames): neither noise nor trusted pitch.
        Unconfirmed,
        /// Steady voiced pitch at this many semitones from the candidate.
        Steady(f64),
    }

    /// Runs base -> candidate -> next through cleanup; returns whether the
    /// candidate pitch survived and whether the outlier rule fired.
    fn run_candidate(base: u8, offset: f64, candidate: &Candidate) -> (bool, bool) {
        let pitch = |semitones: i32| (base as i32 + semitones) as u8;
        let high = pitch(candidate.leap);
        let (hop, lead_in) = (0.01, 0.5);
        let mut track = track(&[(offset, None, -80.0)]);
        let push = |track: &mut PitchTrack, seconds: f64, f0: Option<f64>, voiced: f64| {
            for _ in 0..(seconds / hop).round() as usize {
                track.f0_midi.push(f0);
                track.voiced_probability.push(voiced);
                track.level_db.push(LOUD);
            }
        };
        push(&mut track, lead_in, Some(base as f64), 0.9);
        let frames = (candidate.seconds / hop).round() as usize;
        for frame in 0..frames {
            match candidate.evidence {
                CandidateEvidence::Unconfirmed => {
                    let voiced = if frame % 3 == 0 { 0.6 } else { 0.1 };
                    push(&mut track, hop, Some(high as f64), voiced);
                }
                CandidateEvidence::Steady(delta) => {
                    push(&mut track, hop, Some(high as f64 + delta), 0.9)
                }
            }
        }
        push(&mut track, lead_in, Some(pitch(candidate.next) as f64), 0.9);
        let start = offset + lead_in;
        let end = start + candidate.seconds;
        let raw = [
            note(offset, start, base, 0.8),
            RawNoteEvent {
                attack: candidate.attack,
                ..note(start, end, high, 0.6)
            },
            note(end, end + lead_in, pitch(candidate.next), 0.8),
        ];
        let outcome = clean_with(&raw, &track);
        let kept = outcome
            .notes
            .iter()
            .any(|n| n.midi == high && n.start < end && n.end > start);
        (
            kept,
            actions(&outcome).contains(&"dropped-register-outlier"),
        )
    }

    /// Preservation cases return to the base pitch (`next: 0`) like the
    /// control, so each differs from a dropped outlier in exactly one factor.
    fn assert_kept_everywhere(label: &str, candidate: Candidate) {
        for base in BASES {
            for offset in OFFSETS {
                let (kept, fired) = run_candidate(base, offset, &candidate);
                assert!(
                    kept && !fired,
                    "{label}: dropped at base {base}, offset {offset}"
                );
            }
        }
    }

    #[test]
    fn control_weak_short_two_way_leap_is_dropped_in_every_register() {
        // Proves the harness can detect the rule firing.
        for leap in [12, 24, -12] {
            for base in BASES {
                for offset in OFFSETS {
                    let (kept, fired) = run_candidate(
                        base,
                        offset,
                        &Candidate {
                            leap,
                            next: 0,
                            seconds: 0.12,
                            attack: Some(0.26),
                            evidence: CandidateEvidence::Unconfirmed,
                        },
                    );
                    assert!(!kept && fired, "leap {leap} base {base} offset {offset}");
                }
            }
        }
    }

    #[test]
    fn high_note_with_strong_attack_is_kept() {
        for leap in [12, 19, 24] {
            assert_kept_everywhere(
                "strong attack",
                Candidate {
                    leap,
                    next: 0,
                    seconds: 0.12,
                    attack: Some(0.9),
                    evidence: CandidateEvidence::Unconfirmed,
                },
            );
        }
    }

    #[test]
    fn high_note_with_supporting_pitch_evidence_is_kept() {
        // Steady voiced pitch at the note, and the common pYIN octave-below
        // reading of a high note: both are measured support, not noise.
        for delta in [0.0, 0.2, -12.0] {
            assert_kept_everywhere(
                "pitch evidence",
                Candidate {
                    leap: 24,
                    next: 0,
                    seconds: 0.12,
                    attack: Some(0.2),
                    evidence: CandidateEvidence::Steady(delta),
                },
            );
        }
    }

    #[test]
    fn sustained_high_note_is_kept_without_attack_or_evidence() {
        for seconds in [0.16, 0.4, 1.2] {
            assert_kept_everywhere(
                "sustained",
                Candidate {
                    leap: 24,
                    next: 0,
                    seconds,
                    attack: None,
                    evidence: CandidateEvidence::Unconfirmed,
                },
            );
        }
    }

    #[test]
    fn legitimate_octave_jumps_are_kept() {
        // The melody moves to the new register and stays near it, so the
        // note is close to one neighbour: a real jump, not a stray event.
        for (leap, next) in [(12, 14), (12, 12), (12, 10), (-12, -10), (24, 22)] {
            assert_kept_everywhere(
                "octave jump",
                Candidate {
                    leap,
                    next,
                    seconds: 0.12,
                    attack: None,
                    evidence: CandidateEvidence::Unconfirmed,
                },
            );
        }
        // Less than an octave on either side is never an outlier.
        assert_kept_everywhere(
            "large leap below an octave",
            Candidate {
                leap: 11,
                next: 0,
                seconds: 0.12,
                attack: None,
                evidence: CandidateEvidence::Unconfirmed,
            },
        );
    }

    #[test]
    fn first_and_last_notes_are_never_register_outliers() {
        for base in BASES {
            let high = base + 24;
            let track = track(&[(1.2, Some(base as f64), LOUD)]);
            for raw in [
                vec![note(0.0, 0.12, high, 0.6), note(0.12, 1.2, base, 0.8)],
                vec![note(0.0, 1.08, base, 0.8), note(1.08, 1.2, high, 0.6)],
            ] {
                let outcome = clean_with(&raw, &track);
                assert!(
                    !actions(&outcome).contains(&"dropped-register-outlier"),
                    "base {base}"
                );
            }
        }
    }

    // ---- unpitched rule vs pYIN voicing probability ----
    //
    // Validation found pYIN decoding the right pitch on fast sung notes while
    // giving them low voicing probability; the unpitched rule then deleted
    // strongly attacked real notes. Decoded pitch agreement now keeps them.

    /// Short note with pYIN's decoded f0 at `f0` but voicing probability 0.1.
    fn low_probability_track(note_f0: Option<f64>) -> PitchTrack {
        let mut measured = track(&[
            (0.5, Some(60.0), LOUD),
            (0.1, note_f0, LOUD),
            (0.5, Some(60.0), LOUD),
        ]);
        for i in 50..60 {
            measured.voiced_probability[i] = 0.1;
        }
        measured
    }

    fn short_note_between_long_ones(midi: u8) -> Vec<RawNoteEvent> {
        vec![
            note(0.0, 0.5, 60, 0.8),
            attacked(0.5, 0.6, midi, 0.9),
            note(0.6, 1.1, 60, 0.8),
        ]
    }

    #[test]
    fn short_note_with_matching_decoded_pitch_survives_low_voicing_probability() {
        for (midi, offset) in [(62, 0.0), (63, 0.09), (62, -0.3)] {
            let outcome = clean_with(
                &short_note_between_long_ones(midi),
                &low_probability_track(Some(midi as f64 + offset)),
            );
            assert!(
                pitches(&outcome.notes).contains(&midi),
                "MIDI {midi} offset {offset}"
            );
            assert!(!actions(&outcome).contains(&"dropped-unpitched"));
        }
    }

    #[test]
    fn short_unvoiced_or_mismatched_event_is_still_unpitched() {
        // No decoded pitch at all (consonant, breath).
        let outcome = clean_with(
            &short_note_between_long_ones(67),
            &low_probability_track(None),
        );
        assert!(actions(&outcome).contains(&"dropped-unpitched"));
        // Decoded pitch nowhere near the transcribed one.
        let outcome = clean_with(
            &short_note_between_long_ones(67),
            &low_probability_track(Some(62.0)),
        );
        assert!(actions(&outcome).contains(&"dropped-unpitched"));
    }

    #[test]
    fn drops_bleed_where_the_lead_stem_is_silent() {
        // Real song: intro "notes" transcribed from a stem ~70 dB down.
        let track = track(&[(2.0, None, -82.0), (2.0, Some(60.0), LOUD)]);
        let outcome = clean_with(&[note(0.5, 1.0, 39, 0.6), note(2.2, 2.8, 60, 0.8)], &track);
        assert_eq!(pitches(&outcome.notes), vec![60]);
        assert_eq!(actions(&outcome), vec!["dropped-bleed"]);
    }

    #[test]
    fn drops_short_unvoiced_noise_but_keeps_short_sung_notes() {
        // 80 ms consonant noise (audible, unvoiced) vs an 80 ms sung grace note.
        let track = track(&[
            (1.0, Some(62.0), LOUD),
            (0.1, None, LOUD),
            (0.9, Some(62.0), LOUD),
            (0.08, Some(64.0), LOUD),
            (0.92, Some(62.0), LOUD),
        ]);
        let outcome = clean_with(
            &[
                note(0.0, 0.95, 62, 0.8),
                note(1.01, 1.09, 67, 0.6), // unvoiced noise
                note(1.1, 1.98, 62, 0.8),
                attacked(2.0, 2.08, 64, 0.9), // sung grace note
                note(2.08, 3.0, 62, 0.8),
            ],
            &track,
        );
        assert_eq!(pitches(&outcome.notes), vec![62, 62, 64, 62]);
        assert_eq!(actions(&outcome), vec!["dropped-unpitched"]);
    }

    #[test]
    fn joins_one_wavering_tone_split_across_two_pitches() {
        // Singer sits at 62.6: Basic Pitch says 63 then 62; one note, pitch 63.
        let track = track(&[(1.2, Some(62.6), LOUD)]);
        let outcome = clean_with(&[note(0.0, 0.5, 63, 0.7), note(0.5, 1.2, 62, 0.7)], &track);
        assert_eq!(pitches(&outcome.notes), vec![63]);
        assert_eq!((outcome.notes[0].start, outcome.notes[0].end), (0.0, 1.2));
        assert_eq!(actions(&outcome), vec!["merged-same-tone"]);
    }

    #[test]
    fn real_attack_or_real_step_is_not_joined() {
        let same_tone = track(&[(1.2, Some(62.6), LOUD)]);
        // Re-articulated syllable: its own attack keeps it separate.
        let outcome = clean_with(
            &[note(0.0, 0.5, 63, 0.7), attacked(0.5, 1.2, 62, 0.9)],
            &same_tone,
        );
        assert_eq!(outcome.notes.len(), 2);
        // A sung semitone step (62 -> 63) has two distinct centres.
        let step = track(&[(0.5, Some(62.0), LOUD), (0.7, Some(63.0), LOUD)]);
        let outcome = clean_with(&[note(0.0, 0.5, 62, 0.7), note(0.5, 1.2, 63, 0.7)], &step);
        assert_eq!(pitches(&outcome.notes), vec![62, 63]);
        assert!(outcome.decisions.is_empty());
    }

    #[test]
    fn absorbs_slide_into_the_next_note_but_keeps_steady_ornament() {
        // 100 ms sweep 60 -> 64 between steady notes: a slide into 64.
        let mut segments = vec![(0.5, Some(60.0), LOUD)];
        for step in 0..10 {
            segments.push((0.01, Some(60.0 + step as f64 * 0.4), LOUD));
        }
        segments.push((0.5, Some(64.0), LOUD));
        let sweep = track(&segments);
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 60, 0.8),
                note(0.5, 0.6, 62, 0.5),
                note(0.6, 1.1, 64, 0.8),
            ],
            &sweep,
        );
        assert_eq!(pitches(&outcome.notes), vec![60, 64]);
        assert_eq!(
            outcome.notes[1].start, 0.5,
            "the target note starts where the slide began"
        );
        assert_eq!(actions(&outcome), vec!["absorbed-slide"]);
        // Same timing but a steady sung 62: an ornament, kept.
        let steady = track(&[
            (0.5, Some(60.0), LOUD),
            (0.1, Some(62.0), LOUD),
            (0.5, Some(64.0), LOUD),
        ]);
        let outcome = clean_with(
            &[
                note(0.0, 0.5, 60, 0.8),
                note(0.5, 0.6, 62, 0.5),
                note(0.6, 1.1, 64, 0.8),
            ],
            &steady,
        );
        assert_eq!(pitches(&outcome.notes), vec![60, 62, 64]);
    }

    #[test]
    fn repitches_clear_mismatch_but_not_octave_disagreement() {
        let sung_61 = track(&[(1.0, Some(61.0), LOUD)]);
        let outcome = clean_with(&[note(0.0, 1.0, 63, 0.7)], &sung_61);
        assert_eq!(pitches(&outcome.notes), vec![61]);
        assert_eq!(actions(&outcome), vec!["repitched"]);
        // pYIN one octave off: keep the transcription.
        let outcome = clean_with(&[note(0.0, 1.0, 73, 0.7)], &sung_61);
        assert_eq!(pitches(&outcome.notes), vec![73]);
    }

    #[test]
    fn inconsistent_track_falls_back_to_duration_rules() {
        let mut broken = track(&[(1.0, Some(60.0), LOUD)]);
        broken.level_db.pop();
        let outcome = clean_with(&[note(0.0, 1.0, 60, 0.7)], &broken);
        assert_eq!(pitches(&outcome.notes), vec![60]);
        assert!(outcome.decisions.is_empty());
    }

    #[test]
    fn fuzz_with_random_evidence_still_yields_valid_monophonic_line() {
        let mut rng = Lcg(7);
        for case in 0..1_500 {
            let segments: Vec<(f64, Option<f64>, f64)> = (0..40)
                .map(|_| {
                    let voiced = rng.next_unit() > 0.3;
                    let f0 = voiced.then(|| 56.0 + rng.next_unit() * 12.0);
                    let level = if rng.next_unit() > 0.2 { -10.0 } else { -80.0 };
                    (0.05 + rng.next_unit() * 0.3, f0, level)
                })
                .collect();
            let track = track(&segments);
            let raw: Vec<RawNoteEvent> = (0..1 + (rng.next_unit() * 40.0) as usize)
                .map(|_| {
                    let start = (rng.next_unit() * 8.0 * 100.0).round() / 100.0;
                    RawNoteEvent {
                        start,
                        end: start + 0.02 + (rng.next_unit() * 0.8 * 100.0).round() / 100.0,
                        midi: 56 + (rng.next_unit() * 12.0) as u8,
                        confidence: 0.3 + rng.next_unit() * 0.7,
                        attack: (rng.next_unit() > 0.5).then(|| rng.next_unit()),
                    }
                })
                .collect();
            let outcome =
                clean_notes_with_evidence(&raw, &CleanupSettings::default(), Some(&track));
            if let Err(error) = validate_monophonic_sequence(&outcome.notes) {
                panic!("case {case}: {error}\nraw: {raw:?}");
            }
        }
    }

    #[test]
    fn empty_input_is_fine() {
        assert!(clean(&[]).is_empty());
    }
}

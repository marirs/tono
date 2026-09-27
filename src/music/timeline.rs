//! Practice timeline: answers "what should the screen show at time t?".
//!
//! Kept free of rendering code so frame timing can be unit-tested exactly.

use crate::instruments::fingering::FingeringTimelineEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NowState {
    /// Before note `index` starts: count-in or articulation gap. Shown as
    /// "get ready" so repeated identical fingerings visibly re-attack.
    Ready(usize),
    /// Note `index` is sounding.
    Sounding(usize),
    /// All notes are over.
    Finished,
}

impl NowState {
    /// Index of the note occupying the large NOW diagram.
    pub fn current_index(self) -> Option<usize> {
        match self {
            NowState::Ready(index) | NowState::Sounding(index) => Some(index),
            NowState::Finished => None,
        }
    }
}

/// Metronome beats as explicit timestamps. Real songs use the beat
/// tracker's detected times directly, so tempo changes are followed instead
/// of drifting from an idealised grid.
#[derive(Debug, Clone, Default)]
pub struct BeatTrack {
    /// Sorted beat times in timeline seconds.
    pub times: Vec<f64>,
    /// Beats per bar when the bar position is actually known (then beat 0
    /// is a downbeat). `None` = bar position unknown: no accents and no
    /// bar counting, because either would claim knowledge we do not have.
    pub beats_per_bar: Option<u32>,
}

impl BeatTrack {
    /// Evenly spaced beats from t = 0 with known bars (M1 demo melody).
    pub fn regular_bars(
        beats_per_minute: f64,
        beats_per_bar: u32,
        total_duration_seconds: f64,
    ) -> Self {
        let seconds_per_beat = 60.0 / beats_per_minute;
        let times = (0..)
            .map(|index| index as f64 * seconds_per_beat)
            .take_while(|time| *time < total_duration_seconds)
            .collect();
        BeatTrack {
            times,
            beats_per_bar: Some(beats_per_bar),
        }
    }

    /// Detected beats (region seconds) moved into practice time.
    pub fn detected(
        region_beat_times: &[f64],
        tempo_scale: f64,
        total_duration_seconds: f64,
    ) -> Self {
        let times = region_beat_times
            .iter()
            .map(|time| time / tempo_scale)
            .filter(|time| *time >= 0.0 && *time < total_duration_seconds)
            .collect();
        BeatTrack {
            times,
            beats_per_bar: None,
        }
    }

    pub fn is_accented(&self, beat_index: usize) -> bool {
        self.beats_per_bar
            .is_some_and(|per_bar| beat_index % per_bar as usize == 0)
    }

    /// Median inter-beat interval as BPM, for display only.
    pub fn median_bpm(&self) -> Option<f64> {
        let mut intervals: Vec<f64> = self
            .times
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .collect();
        if intervals.is_empty() {
            return None;
        }
        intervals.sort_by(f64::total_cmp);
        Some(60.0 / intervals[intervals.len() / 2])
    }
}

/// Where `t` sits relative to the beats: the latest beat at or before it,
/// and how far (0..1) toward the next one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatPosition {
    pub index: usize,
    pub fraction: f64,
}

/// Preparation pulses before source time zero, not a claim about bar phase.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct CountIn {
    pub beats: u8,
    pub seconds_per_beat: f64,
}

impl CountIn {
    pub fn new(beats: u8, bpm: f64) -> Self {
        // Whole samples keep audio delay and displayed timestamps identical.
        Self {
            beats,
            seconds_per_beat: (60.0 / bpm * 44_100.0).round() / 44_100.0,
        }
    }
    pub fn duration(self) -> f64 {
        self.beats as f64 * self.seconds_per_beat
    }
    pub fn number_at(self, time: f64) -> Option<u8> {
        if time < 0.0 || time >= self.duration() {
            None
        } else {
            Some((time / self.seconds_per_beat).floor() as u8 + 1)
        }
    }
}

/// Arrival cues show for this long after an onset...
const ARRIVAL_CUE_SECONDS: f64 = 0.35;
/// ...but never for more than this share of the time to the next onset,
/// so even fast passages get a preparation phase.
const ARRIVAL_CUE_MAX_SHARE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CuePhase {
    /// Just after an onset: rings confirm the change into this note.
    Arrival,
    /// Until the next onset: rings show the change to make next.
    Prepare,
}

pub struct PracticeTimeline {
    pub count_in: Option<CountIn>,
    pub entries: Vec<FingeringTimelineEntry>,
    pub beats: BeatTrack,
    pub total_duration_seconds: f64,
}

impl PracticeTimeline {
    /// M1 demo: `tail_seconds` of silence is kept after the last note so the
    /// video does not cut off on the final release; bars of 4 from t = 0.
    pub fn new(
        entries: Vec<FingeringTimelineEntry>,
        beats_per_minute: f64,
        tail_seconds: f64,
    ) -> Self {
        let total = entries.last().map_or(0.0, |entry| entry.end) + tail_seconds;
        let beats = BeatTrack::regular_bars(beats_per_minute, 4, total);
        Self::with_beats(entries, beats, total)
    }

    /// For real songs: the video must last exactly as long as the prepared
    /// region (and its BGM), independent of where the last note ends.
    pub fn with_beats(
        entries: Vec<FingeringTimelineEntry>,
        beats: BeatTrack,
        total_duration_seconds: f64,
    ) -> Self {
        PracticeTimeline {
            count_in: None,
            entries,
            beats,
            total_duration_seconds,
        }
    }

    pub fn prepend_count_in(&mut self, count_in: CountIn) {
        let delay = count_in.duration();
        for entry in &mut self.entries {
            entry.start += delay;
            entry.end += delay;
        }
        for beat in &mut self.beats.times {
            *beat += delay;
        }
        self.total_duration_seconds += delay;
        self.count_in = (count_in.beats > 0).then_some(count_in);
    }

    pub fn count_in_number_at(&self, time: f64) -> Option<u8> {
        self.count_in.and_then(|count| count.number_at(time))
    }

    pub fn now_state_at(&self, time_seconds: f64) -> NowState {
        // Notes are sorted and non-overlapping (validated upstream), so the
        // first note that has not ended yet is the current/upcoming one.
        let first_unfinished = self
            .entries
            .partition_point(|entry| entry.end <= time_seconds);
        match self.entries.get(first_unfinished) {
            None => NowState::Finished,
            Some(entry) if time_seconds >= entry.start => NowState::Sounding(first_unfinished),
            Some(_) => NowState::Ready(first_unfinished),
        }
    }

    pub fn next_index_after(&self, now: NowState) -> Option<usize> {
        let next = now.current_index()? + 1;
        (next < self.entries.len()).then_some(next)
    }

    /// The fingering change the player must make next, as (held note,
    /// upcoming note): while note i sounds, the change into i + 1; during a
    /// gap or count-in before note i, the change into i. `None` after the
    /// last note starts, because nothing is left to prepare.
    pub fn upcoming_change(&self, now: NowState) -> Option<(Option<usize>, usize)> {
        match now {
            NowState::Ready(index) => Some((index.checked_sub(1), index)),
            NowState::Sounding(index) => {
                let next = index + 1;
                (next < self.entries.len()).then_some((Some(index), next))
            }
            NowState::Finished => None,
        }
    }

    /// Which change the NOW rings describe at `time_seconds`: right after a
    /// note starts they confirm the change just made (arrival); for the rest
    /// of the wait they show the change to prepare next.
    pub fn cue_phase_at(&self, time_seconds: f64, now: NowState) -> CuePhase {
        let NowState::Sounding(index) = now else {
            return CuePhase::Prepare;
        };
        if index + 1 >= self.entries.len() {
            return CuePhase::Arrival;
        }
        let onset = self.entries[index].start;
        let wait = self.entries[index + 1].start - onset;
        let arrival_window = ARRIVAL_CUE_SECONDS.min(wait * ARRIVAL_CUE_MAX_SHARE);
        if time_seconds - onset < arrival_window {
            CuePhase::Arrival
        } else {
            CuePhase::Prepare
        }
    }

    /// Countdown to the next note onset: (fraction of the wait elapsed,
    /// seconds left). The wait runs from the latest onset (or video start)
    /// to the next onset, gaps included, so it keeps counting even between
    /// detached notes where the old per-note bar sat empty.
    pub fn change_countdown_at(&self, time_seconds: f64) -> Option<(f64, f64)> {
        let started = self
            .entries
            .partition_point(|entry| entry.start <= time_seconds);
        let next_start = self.entries.get(started)?.start;
        let anchor = started
            .checked_sub(1)
            .map_or(0.0, |index| self.entries[index].start);
        let wait = next_start - anchor;
        if wait <= 0.0 {
            return None;
        }
        let fraction = ((time_seconds - anchor) / wait).clamp(0.0, 1.0);
        Some((fraction, next_start - time_seconds))
    }

    /// Seconds since the most recent note onset (for the onset flash).
    pub fn seconds_since_onset(&self, time_seconds: f64) -> Option<f64> {
        let started = self
            .entries
            .partition_point(|entry| entry.start <= time_seconds);
        let latest = self.entries.get(started.checked_sub(1)?)?;
        Some(time_seconds - latest.start)
    }

    /// Fraction [0, 1] of the sounding note elapsed; 0 while not sounding.
    pub fn note_progress_at(&self, time_seconds: f64) -> f64 {
        match self.now_state_at(time_seconds) {
            NowState::Sounding(index) => {
                let entry = &self.entries[index];
                ((time_seconds - entry.start) / (entry.end - entry.start)).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    pub fn timeline_progress_at(&self, time_seconds: f64) -> f64 {
        if self.total_duration_seconds <= 0.0 {
            return 1.0;
        }
        (time_seconds / self.total_duration_seconds).clamp(0.0, 1.0)
    }

    /// `None` before the first beat or when there are no beats.
    pub fn beat_position_at(&self, time_seconds: f64) -> Option<BeatPosition> {
        let times = &self.beats.times;
        let beats_started = times.partition_point(|&beat| beat <= time_seconds);
        let index = beats_started.checked_sub(1)?;
        // After the last beat, keep fading with the previous interval.
        let interval = match (times.get(index + 1), index.checked_sub(1)) {
            (Some(next), _) => next - times[index],
            (None, Some(previous)) => times[index] - times[previous],
            (None, None) => 0.5,
        };
        let fraction = ((time_seconds - times[index]) / interval).clamp(0.0, 1.0);
        Some(BeatPosition { index, fraction })
    }

    /// Every beat with its accent flag (for the audio click track).
    pub fn beat_times(&self) -> Vec<(f64, bool)> {
        self.beats
            .times
            .iter()
            .enumerate()
            .map(|(index, &time)| (time, self.beats.is_accented(index)))
            .collect()
    }

    /// Number of video frames covering the timeline. Frame `i` shows the
    /// state at exactly `i / fps`.
    pub fn frame_count(&self, frames_per_second: u32) -> u64 {
        (self.total_duration_seconds * frames_per_second as f64).ceil() as u64
    }
}

pub fn frame_time_seconds(frame_index: u64, frames_per_second: u32) -> f64 {
    frame_index as f64 / frames_per_second as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fingering::{Fingering, OctaveShift};

    fn entry(start: f64, end: f64) -> FingeringTimelineEntry {
        FingeringTimelineEntry {
            start,
            end,
            midi: 60,
            fingering: Fingering {
                octave: OctaveShift::Normal,
                keys: vec![],
            },
            transition_to_next: None,
        }
    }

    fn two_note_timeline() -> PracticeTimeline {
        PracticeTimeline::new(vec![entry(1.0, 2.0), entry(2.5, 3.0)], 120.0, 1.0)
    }

    #[test]
    fn count_in_shifts_every_event_and_keeps_boundaries_aligned() {
        let mut timeline = two_note_timeline();
        let old_beats = timeline.beats.times.clone();
        let count = CountIn::new(4, 100.0);
        timeline.prepend_count_in(count);
        let offset = count.duration();
        assert!((offset - 2.4).abs() < 1e-9);
        assert!((timeline.entries[0].start - 3.4).abs() < 1e-9);
        assert!((timeline.total_duration_seconds - 6.4).abs() < 1e-9);
        for (before, after) in old_beats.iter().zip(&timeline.beats.times) {
            assert!((after - before - offset).abs() < 1e-9);
        }
        assert_eq!(timeline.count_in_number_at(0.0), Some(1));
        assert_eq!(timeline.count_in_number_at(0.6), Some(2));
        assert_eq!(timeline.count_in_number_at(2.399), Some(4));
        assert_eq!(timeline.count_in_number_at(2.4), None);
        assert_eq!(timeline.now_state_at(3.399), NowState::Ready(0));
        assert_eq!(timeline.now_state_at(3.4), NowState::Sounding(0));
    }

    #[test]
    fn zero_count_in_preserves_timing() {
        let mut timeline = two_note_timeline();
        timeline.prepend_count_in(CountIn::new(0, 100.0));
        assert!(timeline.count_in.is_none());
        assert_eq!(timeline.entries[0].start, 1.0);
        assert_eq!(timeline.total_duration_seconds, 4.0);
    }

    #[test]
    fn now_state_boundaries_are_half_open() {
        let timeline = two_note_timeline();
        assert_eq!(timeline.now_state_at(0.0), NowState::Ready(0));
        assert_eq!(timeline.now_state_at(0.999), NowState::Ready(0));
        assert_eq!(timeline.now_state_at(1.0), NowState::Sounding(0));
        assert_eq!(timeline.now_state_at(1.999), NowState::Sounding(0));
        // Gap after note 0 is "get ready" for note 1.
        assert_eq!(timeline.now_state_at(2.0), NowState::Ready(1));
        assert_eq!(timeline.now_state_at(2.5), NowState::Sounding(1));
        assert_eq!(timeline.now_state_at(3.0), NowState::Finished);
    }

    #[test]
    fn next_index_follows_current() {
        let timeline = two_note_timeline();
        assert_eq!(timeline.next_index_after(NowState::Ready(0)), Some(1));
        assert_eq!(timeline.next_index_after(NowState::Sounding(1)), None);
        assert_eq!(timeline.next_index_after(NowState::Finished), None);
    }

    #[test]
    fn progress_values() {
        let timeline = two_note_timeline();
        assert_eq!(timeline.note_progress_at(0.5), 0.0);
        assert!((timeline.note_progress_at(1.5) - 0.5).abs() < 1e-9);
        assert!((timeline.timeline_progress_at(2.0) - 0.5).abs() < 1e-9);
        assert_eq!(timeline.timeline_progress_at(99.0), 1.0);
    }

    #[test]
    fn regular_bars_for_demo() {
        let timeline = two_note_timeline(); // 120 bpm, 4.0 s
        assert_eq!(
            timeline.beat_position_at(0.0),
            Some(BeatPosition {
                index: 0,
                fraction: 0.0
            })
        );
        assert_eq!(timeline.beat_position_at(0.49).map(|p| p.index), Some(0));
        let position = timeline.beat_position_at(1.25).unwrap();
        assert_eq!(position.index, 2);
        assert!((position.fraction - 0.5).abs() < 1e-9);
        let accents: Vec<bool> = timeline
            .beat_times()
            .iter()
            .map(|&(_, accent)| accent)
            .collect();
        assert_eq!(&accents[..5], &[true, false, false, false, true]);
    }

    #[test]
    fn detected_beats_are_followed_exactly_even_when_tempo_varies() {
        // Beats speed up: intervals 0.6, 0.5, 0.4 s. A fixed grid would drift.
        let detected = [0.3, 0.9, 1.4, 1.8];
        let timeline =
            PracticeTimeline::with_beats(vec![], BeatTrack::detected(&detected, 1.0, 3.0), 3.0);
        assert_eq!(
            timeline.beat_position_at(0.2),
            None,
            "no flash before the first beat"
        );
        assert_eq!(timeline.beat_position_at(1.4).map(|p| p.index), Some(2));
        let position = timeline.beat_position_at(1.6).unwrap();
        assert_eq!(position.index, 2);
        assert!(
            (position.fraction - 0.5).abs() < 1e-9,
            "fraction uses the real 0.4 s interval"
        );
        let clicks: Vec<f64> = timeline
            .beat_times()
            .iter()
            .map(|&(time, _)| time)
            .collect();
        assert_eq!(clicks, detected.to_vec());
    }

    #[test]
    fn detected_beats_have_no_accents_and_scale_with_tempo() {
        let beats = BeatTrack::detected(&[0.3, 0.9, 1.5, 2.1, 2.7], 0.75, 3.0);
        assert!(beats.beats_per_bar.is_none());
        assert!((0..5).all(|index| !beats.is_accented(index)));
        // 0.75x speed: 0.3 -> 0.4 s ... 2.1 -> 2.8 s; 2.7 -> 3.6 s is past the end.
        let expected = [0.4, 1.2, 2.0, 2.8];
        assert_eq!(beats.times.len(), expected.len());
        assert!(beats
            .times
            .iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() < 1e-9));
        assert!((beats.median_bpm().unwrap() - 75.0).abs() < 1e-6);
    }

    #[test]
    fn upcoming_change_looks_ahead_while_a_note_sounds() {
        let timeline = two_note_timeline(); // notes 1.0-2.0 and 2.5-3.0
        assert_eq!(
            timeline.upcoming_change(NowState::Ready(0)),
            Some((None, 0))
        );
        // While note 0 sounds, prepare the change INTO note 1 (not out of nothing).
        assert_eq!(
            timeline.upcoming_change(NowState::Sounding(0)),
            Some((Some(0), 1))
        );
        assert_eq!(
            timeline.upcoming_change(NowState::Ready(1)),
            Some((Some(0), 1))
        );
        assert_eq!(timeline.upcoming_change(NowState::Sounding(1)), None);
        assert_eq!(timeline.upcoming_change(NowState::Finished), None);
    }

    #[test]
    fn cue_phase_confirms_arrival_then_prepares_next() {
        let timeline = two_note_timeline(); // onsets 1.0 and 2.5
        assert_eq!(
            timeline.cue_phase_at(0.5, NowState::Ready(0)),
            CuePhase::Prepare
        );
        assert_eq!(
            timeline.cue_phase_at(1.1, NowState::Sounding(0)),
            CuePhase::Arrival
        );
        assert_eq!(
            timeline.cue_phase_at(1.4, NowState::Sounding(0)),
            CuePhase::Prepare
        );
        // Last note: nothing to prepare.
        assert_eq!(
            timeline.cue_phase_at(2.9, NowState::Sounding(1)),
            CuePhase::Arrival
        );
        // Fast passage (0.2 s between onsets): arrival capped at half.
        let fast = PracticeTimeline::new(vec![entry(0.0, 0.2), entry(0.2, 0.4)], 120.0, 0.0);
        assert_eq!(
            fast.cue_phase_at(0.09, NowState::Sounding(0)),
            CuePhase::Arrival
        );
        assert_eq!(
            fast.cue_phase_at(0.11, NowState::Sounding(0)),
            CuePhase::Prepare
        );
    }

    #[test]
    fn countdown_runs_from_onset_to_onset_including_gaps() {
        let timeline = two_note_timeline();
        // Before the first note: counts from video start.
        let (fraction, left) = timeline.change_countdown_at(0.5).unwrap();
        assert!((fraction - 0.5).abs() < 1e-9 && (left - 0.5).abs() < 1e-9);
        // In the gap 2.0-2.5 the countdown keeps running toward 2.5.
        let (fraction, left) = timeline.change_countdown_at(2.25).unwrap();
        assert!((fraction - 0.8333).abs() < 1e-3 && (left - 0.25).abs() < 1e-9);
        // After the last onset there is nothing to count down to.
        assert!(timeline.change_countdown_at(2.6).is_none());
        assert_eq!(timeline.seconds_since_onset(0.5), None);
        assert!((timeline.seconds_since_onset(2.6).unwrap() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn frame_count_and_note_onset_frames() {
        let timeline = two_note_timeline(); // 4.0 s total
        assert_eq!(timeline.frame_count(30), 120);
        // The first frame showing a note as sounding must be the frame whose
        // timestamp is the first >= note start (no early/late frame).
        let first_sounding_frame = (0..timeline.frame_count(30))
            .find(|&frame| {
                timeline.now_state_at(frame_time_seconds(frame, 30)) == NowState::Sounding(1)
            })
            .unwrap();
        assert_eq!(first_sounding_frame, 75); // 2.5 s * 30 fps
    }
}

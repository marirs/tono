//! Audio evidence for note cleanup: a monophonic pitch track measured on the
//! lead stem by the ML worker (pYIN f0, voicing probability, frame level).
//!
//! Basic Pitch is level-invariant and polyphonic: it happily transcribes
//! separation bleed at -70 dB and splits one wavering sung tone into several
//! notes. The pitch track lets cleanup ask "is somebody actually singing
//! this, and at what pitch?" instead of guessing from durations alone.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct PitchTrack {
    pub method: String,
    pub hop_seconds: f64,
    /// Fractional MIDI pitch per frame; `None` where no f0 was found.
    pub f0_midi: Vec<Option<f64>>,
    pub voiced_probability: Vec<f64>,
    /// Frame RMS level in dBFS.
    pub level_db: Vec<f64>,
}

/// A frame counts as sung when pYIN is at least this sure it is voiced.
const VOICED_PROBABILITY: f64 = 0.5;
/// Level of "loud singing": this percentile of all frame levels.
const LOUD_REFERENCE_PERCENTILE: f64 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvidence {
    /// Share of the note's frames that are voiced (0..1).
    pub voiced_fraction: f64,
    /// Median f0 of the voiced frames, fractional MIDI.
    pub f0_center: Option<f64>,
    /// 90th minus 10th percentile of voiced f0 (semitones): how much the
    /// pitch moves inside the note (vibrato, slide).
    pub f0_spread: Option<f64>,
    /// Median frame level relative to loud singing (dB, <= ~0).
    pub level_below_loud_db: f64,
}

impl PitchTrack {
    pub fn is_consistent(&self) -> bool {
        self.hop_seconds.is_finite()
            && self.hop_seconds > 0.0
            && !self.f0_midi.is_empty()
            && self.f0_midi.len() == self.voiced_probability.len()
            && self.f0_midi.len() == self.level_db.len()
            && self
                .f0_midi
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0.0..=127.0).contains(v))
            && self
                .voiced_probability
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && self.level_db.iter().all(|v| v.is_finite())
    }

    pub fn loud_reference_db(&self) -> f64 {
        percentile(self.level_db.clone(), LOUD_REFERENCE_PERCENTILE).unwrap_or(0.0)
    }

    /// Frames whose centre lies in [start, end); at least the nearest frame.
    fn frame_range(&self, start: f64, end: f64) -> std::ops::Range<usize> {
        let frame_count = self.f0_midi.len();
        if frame_count == 0 || start >= frame_count as f64 * self.hop_seconds {
            return 0..0;
        }
        let first = ((start / self.hop_seconds).ceil().max(0.0) as usize).min(frame_count - 1);
        let past_last = ((end / self.hop_seconds).ceil().max(0.0) as usize).min(frame_count);
        if past_last > first {
            first..past_last
        } else {
            first..first + 1
        }
    }

    /// Share of the span's frames where pYIN decoded an f0 within `tolerance`
    /// semitones of `midi`, whatever its voicing probability. pYIN's decoded
    /// pitch can be right while its voicing probability stays low (short,
    /// fast or rough-voiced notes), so this is pitch evidence in its own right.
    pub fn pitch_agreement(&self, start: f64, end: f64, midi: u8, tolerance: f64) -> Option<f64> {
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
            return None;
        }
        let frames = self.frame_range(start, end);
        if frames.is_empty() || self.f0_midi.len() != self.voiced_probability.len() {
            return None;
        }
        let total = frames.len() as f64;
        let agreeing = self.f0_midi[frames]
            .iter()
            .filter(|f0| f0.is_some_and(|f0| (f0 - midi as f64).abs() <= tolerance))
            .count();
        Some(agreeing as f64 / total)
    }

    pub fn evidence(&self, start: f64, end: f64, loud_reference_db: f64) -> Option<NoteEvidence> {
        // Cheap structural checks also protect direct library callers; full
        // value validation happens once at the worker boundary / cleanup entry.
        if !self.hop_seconds.is_finite()
            || self.hop_seconds <= 0.0
            || self.f0_midi.len() != self.voiced_probability.len()
            || self.f0_midi.len() != self.level_db.len()
            || !start.is_finite()
            || !end.is_finite()
            || start < 0.0
            || end <= start
            || !loud_reference_db.is_finite()
        {
            return None;
        }
        let frames = self.frame_range(start, end);
        if frames.is_empty() {
            return None;
        }
        let frame_count = frames.len() as f64;
        let voiced_f0: Vec<f64> = frames
            .clone()
            .filter(|&i| self.voiced_probability[i] >= VOICED_PROBABILITY)
            .filter_map(|i| self.f0_midi[i])
            .collect();
        let voiced_fraction = voiced_f0.len() as f64 / frame_count;
        let level = percentile(self.level_db[frames].to_vec(), 0.5).unwrap_or(-120.0);
        let spread = match (
            percentile(voiced_f0.clone(), 0.9),
            percentile(voiced_f0.clone(), 0.1),
        ) {
            (Some(high), Some(low)) if voiced_f0.len() >= 3 => Some(high - low),
            _ => None,
        };
        Some(NoteEvidence {
            voiced_fraction,
            f0_center: percentile(voiced_f0, 0.5),
            f0_spread: spread,
            level_below_loud_db: level - loud_reference_db,
        })
    }
}

fn percentile(mut values: Vec<f64>, fraction: f64) -> Option<f64> {
    values.retain(|value| value.is_finite());
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let position = (values.len() - 1) as f64 * fraction;
    let (low, high) = (position.floor() as usize, position.ceil() as usize);
    let weight = position - low as f64;
    Some(values[low] * (1.0 - weight) + values[high] * weight)
}

#[cfg(test)]
pub mod test_support {
    use super::PitchTrack;

    /// Builds a 10 ms-hop track from (seconds, f0 or None for silence, level dB).
    pub fn track(segments: &[(f64, Option<f64>, f64)]) -> PitchTrack {
        let hop = 0.01;
        let mut track = PitchTrack {
            method: "test".into(),
            hop_seconds: hop,
            f0_midi: vec![],
            voiced_probability: vec![],
            level_db: vec![],
        };
        for &(seconds, f0, level) in segments {
            for _ in 0..(seconds / hop).round() as usize {
                track.f0_midi.push(f0);
                track
                    .voiced_probability
                    .push(if f0.is_some() { 0.9 } else { 0.02 });
                track.level_db.push(level);
            }
        }
        track
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::track;

    #[test]
    fn evidence_reports_voicing_center_spread_and_level() {
        // 1 s silence-level bleed, then 1 s sung at 62.6.
        let track = track(&[(1.0, None, -80.0), (1.0, Some(62.6), -12.0)]);
        let loud = track.loud_reference_db();
        assert_eq!(loud, -12.0);
        let bleed = track.evidence(0.2, 0.6, loud).unwrap();
        assert_eq!(bleed.voiced_fraction, 0.0);
        assert!(bleed.f0_center.is_none() && bleed.level_below_loud_db < -60.0);
        let sung = track.evidence(1.2, 1.6, loud).unwrap();
        assert!((sung.f0_center.unwrap() - 62.6).abs() < 1e-9);
        assert!(sung.voiced_fraction > 0.99 && sung.f0_spread.unwrap() < 1e-9);
        assert!(sung.level_below_loud_db.abs() < 1e-9);
    }

    #[test]
    fn direct_queries_with_bad_shapes_or_outside_coverage_are_safe() {
        let mut t = track(&[(1.0, Some(60.0), -12.0)]);
        assert!(t.evidence(2.0, 3.0, -12.0).is_none());
        assert!(t.evidence(f64::NAN, 0.5, -12.0).is_none());
        t.level_db.clear();
        assert!(t.evidence(0.0, 0.5, -12.0).is_none());
    }

    #[test]
    fn very_short_span_still_gets_one_frame() {
        let track = track(&[(1.0, Some(60.0), -10.0)]);
        let evidence = track.evidence(0.503, 0.505, -10.0).unwrap();
        assert_eq!(evidence.f0_center, Some(60.0));
    }
}

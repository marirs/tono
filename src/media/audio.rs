//! Synthesized audio: M1 melody guide and the metronome click track.
//!
//! Before any ML exists there is no real backing track, so M1 generates a
//! guide with a soft tone per note. The metronome is a separate track (spec
//! "Metronome") mixed in only at render time, so it can later be excluded
//! from performance exports.

use std::f64::consts::TAU;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::{Context, Result};

use crate::music::timeline::PracticeTimeline;

pub const GUIDE_SAMPLE_RATE: u32 = 44_100;

const MELODY_PEAK_AMPLITUDE: f64 = 0.28;
const CLICK_PEAK_AMPLITUDE: f64 = 0.22;
const NOTE_ATTACK_SECONDS: f64 = 0.012;
const NOTE_RELEASE_SECONDS: f64 = 0.040;
const CLICK_LENGTH_SECONDS: f64 = 0.035;

pub fn midi_to_frequency_hz(midi: u8) -> f64 {
    440.0 * 2f64.powf((midi as f64 - 69.0) / 12.0)
}

fn silent_buffer_for(timeline: &PracticeTimeline, sample_rate: u32) -> Vec<f64> {
    vec![0.0; (timeline.total_duration_seconds * sample_rate as f64).ceil() as usize]
}

/// Renders one tone per note as mono f64 samples (no click).
pub fn synthesize_melody_guide_samples(timeline: &PracticeTimeline, sample_rate: u32) -> Vec<f64> {
    let rate = sample_rate as f64;
    let mut samples = silent_buffer_for(timeline, sample_rate);
    for entry in &timeline.entries {
        add_note_tone(
            &mut samples,
            rate,
            entry.start,
            entry.end,
            midi_to_frequency_hz(entry.midi),
        );
    }
    samples
}

/// Renders a click on every beat (accented downbeat) as mono f64 samples.
pub fn synthesize_metronome_samples(timeline: &PracticeTimeline, sample_rate: u32) -> Vec<f64> {
    let rate = sample_rate as f64;
    let mut samples = silent_buffer_for(timeline, sample_rate);
    for (beat_time, is_downbeat) in timeline.beat_times() {
        add_click(
            &mut samples,
            rate,
            beat_time,
            if is_downbeat { 1_600.0 } else { 1_000.0 },
        );
    }
    samples
}

/// Audible preparation: numbered speech and a short pulse on each count.
/// macOS speech is local; unavailable speech falls back to pulses with a warning.
pub fn count_in_samples(
    timeline: &PracticeTimeline,
    ffmpeg: &Path,
    work: &Path,
) -> (Vec<f64>, Option<String>) {
    let mut samples = silent_buffer_for(timeline, GUIDE_SAMPLE_RATE);
    let Some(count) = timeline.count_in else {
        return (samples, None);
    };
    let mut warning = None;
    for index in 0..count.beats {
        let at = index as f64 * count.seconds_per_beat;
        add_click(&mut samples, GUIDE_SAMPLE_RATE as f64, at, 1_200.0);
        match spoken_number(index + 1, ffmpeg, work) {
            Ok(voice) => mix_spoken_number(&mut samples, &voice, at, count.seconds_per_beat),
            Err(error) => {
                warning = Some(format!(
                    "spoken count-in unavailable; using count-in clicks: {error}"
                ))
            }
        }
    }
    (samples, warning)
}

fn spoken_number(number: u8, ffmpeg: &Path, work: &Path) -> Result<Vec<f64>> {
    // The CLI limits --count-in to 4; library callers may ask for more, and
    // those beats fall back to clicks instead of panicking on the lookup.
    let word = ["one", "two", "three", "four"]
        .get((number as usize).wrapping_sub(1))
        .with_context(|| format!("no spoken word for count {number}"))?;
    let speech = work.join(format!("count-{number}.aiff"));
    let status = std::process::Command::new("/usr/bin/say")
        .args(["-v", "Samantha", "-r", "220", "-o"])
        .arg(&speech)
        .arg(word)
        .output()
        .context("running macOS speech")?;
    anyhow::ensure!(status.status.success(), "macOS speech failed");
    let output = std::process::Command::new(ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(&speech)
        .args([
            "-af",
            "silenceremove=start_periods=1:start_duration=0.005:start_threshold=-50dB",
            "-ac",
            "1",
            "-ar",
            "44100",
            "-f",
            "s16le",
            "pipe:1",
        ])
        .output()
        .context("decoding count-in speech")?;
    anyhow::ensure!(output.status.success(), "count-in speech decode failed");
    let mut voice: Vec<f64> = output
        .stdout
        .chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]) as f64 / i16::MAX as f64)
        .collect();
    // Remove trailing silence before fitting the word within a beat.
    while voice.last().is_some_and(|sample| sample.abs() < 0.001) {
        voice.pop();
    }
    let peak = voice.iter().map(|s| s.abs()).fold(0.0_f64, f64::max);
    anyhow::ensure!(peak > 0.001, "count-in speech is empty");
    for sample in &mut voice {
        *sample *= 0.55 / peak;
    }
    Ok(voice)
}

fn mix_spoken_number(samples: &mut [f64], voice: &[f64], at: f64, beat_seconds: f64) {
    let first = (at * GUIDE_SAMPLE_RATE as f64).round() as usize;
    // Keep even a fast count inside its own beat. Slower counts retain normal speech speed.
    let length = voice
        .len()
        .min((beat_seconds * GUIDE_SAMPLE_RATE as f64 * 0.85) as usize);
    for offset in 0..length {
        if let Some(sample) = samples.get_mut(first + offset) {
            *sample += voice[offset * voice.len() / length];
        }
    }
}

fn add_note_tone(samples: &mut [f64], rate: f64, start: f64, end: f64, frequency_hz: f64) {
    let first_sample = (start * rate).round() as usize;
    let last_sample = ((end * rate).round() as usize).min(samples.len());
    let note_length = end - start;
    for sample_index in first_sample..last_sample {
        let elapsed = (sample_index - first_sample) as f64 / rate;
        let remaining = note_length - elapsed;
        let envelope = (elapsed / NOTE_ATTACK_SECONDS)
            .min(remaining / NOTE_RELEASE_SECONDS)
            .clamp(0.0, 1.0);
        // Fundamental plus a quiet 2nd harmonic: reedy enough to hear pitch
        // clearly without being harsh.
        let phase = TAU * frequency_hz * elapsed;
        let tone = phase.sin() + 0.3 * (2.0 * phase).sin();
        samples[sample_index] += MELODY_PEAK_AMPLITUDE * envelope * tone / 1.3;
    }
}

fn add_click(samples: &mut [f64], rate: f64, at_seconds: f64, frequency_hz: f64) {
    let first_sample = (at_seconds * rate).round() as usize;
    let click_samples = (CLICK_LENGTH_SECONDS * rate) as usize;
    for offset in 0..click_samples {
        let Some(sample) = samples.get_mut(first_sample + offset) else {
            break;
        };
        let elapsed = offset as f64 / rate;
        let decay = (-elapsed / (CLICK_LENGTH_SECONDS / 5.0)).exp();
        *sample += CLICK_PEAK_AMPLITUDE * decay * (TAU * frequency_hz * elapsed).sin();
    }
}

// ---- Time-stretch calibration -------------------------------------------
//
// ffmpeg's atempo (and asetrate+atempo octave shifts) does not keep events
// exactly in place: measured on ffmpeg 9, clicks come out ~11-17 ms early
// at 0.75x and ~35 ms early at 0.5x. That would put a slowed backing ahead
// of the picture, so the same filter chain is run on a click train of
// known timing and the measured offset is compensated.

const CALIBRATION_FIRST_CLICK: f64 = 1.0;
const CALIBRATION_SPACING: f64 = 0.5;
const CALIBRATION_CLICKS: usize = 20;
/// Short tone burst: sharp enough to locate within a sample or two.
const CALIBRATION_BURST_SAMPLES: usize = 200;
const CALIBRATION_BURST_HZ: f64 = 2_000.0;
/// Search window around each expected click, and the share that must be found.
const CALIBRATION_SEARCH_SECONDS: f64 = 0.1;
const CALIBRATION_MIN_FOUND_SHARE: f64 = 0.8;

fn calibration_click_train(rate: f64) -> (Vec<f64>, Vec<f64>) {
    let total = CALIBRATION_FIRST_CLICK + CALIBRATION_SPACING * (CALIBRATION_CLICKS as f64 + 2.0);
    let mut samples = vec![0.0; (total * rate) as usize];
    let mut peaks = Vec::with_capacity(CALIBRATION_CLICKS);
    for index in 0..CALIBRATION_CLICKS {
        let start =
            ((CALIBRATION_FIRST_CLICK + index as f64 * CALIBRATION_SPACING) * rate) as usize;
        for offset in 0..CALIBRATION_BURST_SAMPLES {
            let window = (std::f64::consts::PI * offset as f64 / CALIBRATION_BURST_SAMPLES as f64)
                .sin()
                .powi(2);
            samples[start + offset] =
                0.8 * window * (TAU * CALIBRATION_BURST_HZ * offset as f64 / rate).sin();
        }
        peaks.push((start + CALIBRATION_BURST_SAMPLES / 2) as f64 / rate);
    }
    (samples, peaks)
}

/// Median of (expected - found) click peak times: positive means the
/// processed audio runs EARLY by that many seconds. `None` when too few
/// clicks survive to trust the measurement.
pub fn click_train_offset(processed: &[f32], rate: f64, expected_peaks: &[f64]) -> Option<f64> {
    if processed.is_empty()
        || expected_peaks.is_empty()
        || !rate.is_finite()
        || rate <= 0.0
        || expected_peaks.iter().any(|t| !t.is_finite() || *t < 0.0)
        || processed.iter().any(|sample| !sample.is_finite())
    {
        return None;
    }
    let mut offsets: Vec<f64> = expected_peaks
        .iter()
        .filter_map(|&expected| {
            let bound = |time: f64| (time * rate).clamp(0.0, processed.len() as f64) as usize;
            let range = bound(expected - CALIBRATION_SEARCH_SECONDS)
                ..bound(expected + CALIBRATION_SEARCH_SECONDS);
            let (index, peak) = range
                .map(|i| (i, processed[i].abs()))
                .max_by(|a, b| a.1.total_cmp(&b.1))?;
            (peak > 0.05).then(|| expected - index as f64 / rate)
        })
        .collect();
    if (offsets.len() as f64) < expected_peaks.len() as f64 * CALIBRATION_MIN_FOUND_SHARE {
        return None;
    }
    offsets.sort_by(f64::total_cmp);
    Some(offsets[offsets.len() / 2])
}

/// Runs `filter` (the bed's time-stretch/pitch chain, `tempo_scale` being
/// its overall time ratio) on a click train and returns how early (+) or
/// late (-) its output is, in seconds.
pub fn measure_filter_offset_seconds(
    ffmpeg: &Path,
    filter: &str,
    tempo_scale: f64,
    work: &Path,
) -> Result<f64> {
    let rate = GUIDE_SAMPLE_RATE as f64;
    let (clicks, peaks) = calibration_click_train(rate);
    let input = work.join("stretch-calibration.wav");
    write_mono_wav(&input, &clicks, GUIDE_SAMPLE_RATE)?;
    let output = std::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-v", "error", "-i"])
        .arg(&input)
        .args([
            "-af",
            filter,
            "-ac",
            "1",
            "-ar",
            &GUIDE_SAMPLE_RATE.to_string(),
            "-f",
            "f32le",
            "-",
        ])
        .output()
        .context("running ffmpeg for stretch calibration")?;
    anyhow::ensure!(
        output.status.success(),
        "ffmpeg stretch calibration failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let processed: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();
    let expected: Vec<f64> = peaks.iter().map(|peak| peak / tempo_scale).collect();
    click_train_offset(&processed, rate, &expected).context("stretch calibration clicks not found")
}

/// Writes 16-bit PCM mono WAV. Hand-rolled to avoid a dependency for a
/// 44-byte header.
pub fn write_mono_wav(path: &Path, samples: &[f64], sample_rate: u32) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = BufWriter::new(file);
    let data_bytes = (samples.len() * 2) as u32;
    let byte_rate = sample_rate * 2;

    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + data_bytes).to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&16u32.to_le_bytes())?; // fmt chunk size
    writer.write_all(&1u16.to_le_bytes())?; // PCM
    writer.write_all(&1u16.to_le_bytes())?; // mono
    writer.write_all(&sample_rate.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&2u16.to_le_bytes())?; // block align
    writer.write_all(&16u16.to_le_bytes())?; // bits per sample
    writer.write_all(b"data")?;
    writer.write_all(&data_bytes.to_le_bytes())?;
    for sample in samples {
        let quantized = (sample.clamp(-1.0, 1.0) * i16::MAX as f64).round() as i16;
        writer.write_all(&quantized.to_le_bytes())?;
    }
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod calibration_tests {
    use super::*;

    #[test]
    fn offset_measures_early_and_late_output() {
        let rate = 8_000.0;
        let (clicks, peaks) = calibration_click_train(rate);
        let shifted = |shift_samples: isize| -> Vec<f32> {
            (0..clicks.len() as isize)
                .map(|i| {
                    clicks
                        .get((i + shift_samples).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0) as f32
                })
                .collect()
        };
        // Output 40 samples (5 ms) early: positive offset.
        let early = click_train_offset(&shifted(40), rate, &peaks).unwrap();
        assert!((early - 0.005).abs() < 0.0005, "{early}");
        let late = click_train_offset(&shifted(-80), rate, &peaks).unwrap();
        assert!((late + 0.010).abs() < 0.0005, "{late}");
        // Peak picking on a windowed sine is within half a tone period (<0.3 ms).
        let aligned = click_train_offset(&shifted(0), rate, &peaks).unwrap();
        assert!(aligned.abs() < 0.0003, "{aligned}");
    }

    #[test]
    fn invalid_calibration_inputs_return_no_measurement() {
        assert!(click_train_offset(&[0.1], 44100.0, &[]).is_none());
        assert!(click_train_offset(&[], 44100.0, &[1.0]).is_none());
        assert!(click_train_offset(&[0.1], 0.0, &[1.0]).is_none());
        assert!(click_train_offset(&[0.1], f64::NAN, &[1.0]).is_none());
        assert!(click_train_offset(&[0.1], 44100.0, &[f64::MAX]).is_none());
        assert!(click_train_offset(&[f32::NAN], 44100.0, &[0.0]).is_none());
    }

    #[test]
    fn too_few_clicks_is_not_a_measurement() {
        let silence = vec![0.0f32; 80_000];
        assert!(click_train_offset(&silence, 8_000.0, &[1.0, 1.5, 2.0]).is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_counts_stay_inside_their_beats() {
        let voice = vec![0.5; 44_100];
        let mut samples = vec![0.0; 88_200];
        mix_spoken_number(&mut samples, &voice, 0.5, 0.25);
        assert!(samples[..22_050].iter().all(|&s| s == 0.0));
        assert!(samples[22_050..25_000].iter().all(|&s| s > 0.0));
        assert!(samples[33_075..].iter().all(|&s| s == 0.0));
    }

    #[test]
    fn a4_is_440_and_c4_is_middle_c() {
        assert!((midi_to_frequency_hz(69) - 440.0).abs() < 1e-9);
        assert!((midi_to_frequency_hz(60) - 261.6256).abs() < 1e-3);
    }

    fn demo_timeline() -> PracticeTimeline {
        use crate::instruments::fingering::{Fingering, FingeringTimelineEntry, OctaveShift};
        let entries = crate::music::notes::demo_melody(100.0)
            .iter()
            .map(|note| FingeringTimelineEntry {
                start: note.start,
                end: note.end,
                midi: note.midi,
                fingering: Fingering {
                    octave: OctaveShift::Normal,
                    keys: vec![],
                },
                transition_to_next: None,
            })
            .collect();
        PracticeTimeline::new(entries, 100.0, 1.0)
    }

    #[test]
    fn tracks_match_timeline_length_and_do_not_clip() {
        let timeline = demo_timeline();
        let expected_len = (timeline.total_duration_seconds * 8_000.0).ceil() as usize;
        let melody = synthesize_melody_guide_samples(&timeline, 8_000);
        let clicks = synthesize_metronome_samples(&timeline, 8_000);
        assert_eq!(melody.len(), expected_len);
        assert_eq!(clicks.len(), expected_len);
        // Mixed at render time with unity gain, so the sum must stay in range.
        assert!(melody
            .iter()
            .zip(&clicks)
            .all(|(m, c)| (m + c).abs() <= 1.0));
    }

    #[test]
    fn melody_sounds_during_notes_and_clicks_land_on_beats() {
        let timeline = demo_timeline();
        let melody = synthesize_melody_guide_samples(&timeline, 8_000);
        let clicks = synthesize_metronome_samples(&timeline, 8_000);
        let first_note_sample = (timeline.entries[0].start * 8_000.0) as usize + 400;
        assert!(melody[first_note_sample].abs() > 0.0);
        assert_eq!(melody[100], 0.0, "melody is silent during count-in");
        // Beat 1 at 100 bpm = 0.6 s: click energy right after, silence mid-beat.
        let beat_one = (0.6 * 8_000.0) as usize;
        assert!(clicks[beat_one + 5].abs() > 0.0);
        assert_eq!(clicks[beat_one + 2_000], 0.0);
    }
}

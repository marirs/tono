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
    let word = ["one", "two", "three", "four"][(number - 1) as usize];
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

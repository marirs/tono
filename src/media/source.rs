//! FFmpeg steps on the user's source: normalize to one analysis format, and
//! cut the selected region sample-accurately.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Analysis format for every downstream step. 44.1 kHz matches htdemucs;
/// float avoids clipping decoded sources that exceed 0 dBFS.
pub const ANALYSIS_SAMPLE_RATE: u32 = 44_100;

fn run_ffmpeg(ffmpeg: &Path, args: &[&str], input: &Path, output: &Path, what: &str) -> Result<()> {
    let result = Command::new(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-y", "-v", "error", "-i"])
        .arg(input)
        .args(args)
        .arg(output)
        .output()
        .with_context(|| format!("running ffmpeg for {what}"))?;
    if !result.status.success() {
        bail!(
            "ffmpeg failed to {what}: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(())
}

/// Extracts the audio (also from video) as 44.1 kHz stereo float WAV.
pub fn normalize_source(ffmpeg: &Path, input: &Path, output_wav: &Path) -> Result<()> {
    let sample_rate = ANALYSIS_SAMPLE_RATE.to_string();
    run_ffmpeg(
        ffmpeg,
        &["-vn", "-ac", "2", "-ar", &sample_rate, "-c:a", "pcm_f32le"],
        input,
        output_wav,
        "extract/normalize the source audio",
    )
}

/// Region boundaries snapped to whole samples, so the trimmed file, the
/// ML output and project.json all agree on exactly the same length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleRange {
    pub start_sample: u64,
    pub end_sample: u64,
}

impl SampleRange {
    pub fn from_seconds(start: f64, end: f64) -> Self {
        let rate = ANALYSIS_SAMPLE_RATE as f64;
        SampleRange {
            start_sample: (start * rate).round() as u64,
            end_sample: (end * rate).round() as u64,
        }
    }

    pub fn start_seconds(&self) -> f64 {
        self.start_sample as f64 / ANALYSIS_SAMPLE_RATE as f64
    }

    pub fn end_seconds(&self) -> f64 {
        self.end_sample as f64 / ANALYSIS_SAMPLE_RATE as f64
    }

    pub fn duration_seconds(&self) -> f64 {
        (self.end_sample - self.start_sample) as f64 / ANALYSIS_SAMPLE_RATE as f64
    }
}

/// Cuts `range` out of the normalized source; the result starts at t = 0.
pub fn trim_region(
    ffmpeg: &Path,
    normalized_wav: &Path,
    range: SampleRange,
    output_wav: &Path,
) -> Result<()> {
    if range.end_sample <= range.start_sample {
        bail!("empty region {range:?}");
    }
    // atrim with sample positions is exact; -ss/-t would round to packets.
    let filter = format!(
        "atrim=start_sample={}:end_sample={},asetpts=PTS-STARTPTS",
        range.start_sample, range.end_sample
    );
    run_ffmpeg(
        ffmpeg,
        &["-af", &filter, "-c:a", "pcm_f32le"],
        normalized_wav,
        output_wav,
        "cut the song region",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_range_round_trips_seconds() {
        let range = SampleRange::from_seconds(7.85, 29.65);
        assert_eq!(range.start_sample, 346_185);
        assert_eq!(range.end_sample, 1_307_565);
        assert!((range.duration_seconds() - 21.8).abs() < 1.0 / 44_100.0);
        assert!((range.start_seconds() - 7.85).abs() < 1.0 / 44_100.0);
    }
}

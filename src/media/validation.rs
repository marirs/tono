//! Post-render validation of `practice.mp4` with ffprobe (spec "MP4
//! rendering" and rules 16/20): the command must fail rather than report
//! success for a silent, truncated or mismatched file.

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// Maximum allowed difference between video, audio and expected durations.
pub const DURATION_TOLERANCE_SECONDS: f64 = 0.100;

/// Audio codecs we intentionally produce.
const ACCEPTED_AUDIO_CODECS: [&str; 1] = ["aac"];

#[derive(Debug, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    /// ffprobe reports durations as decimal strings.
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedMedia {
    pub video_codec: String,
    pub audio_codec: String,
    pub video_duration_seconds: f64,
    pub audio_duration_seconds: f64,
}

fn run_ffprobe_json(ffprobe: &Path, media_path: &Path) -> Result<String> {
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(media_path)
        .output()
        .with_context(|| format!("running {}", ffprobe.display()))?;
    if !output.status.success() {
        bail!(
            "ffprobe could not read {}: {}",
            media_path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Duration of the first audio stream (or the container) of any media file.
/// Used to detect backing audio shorter than the practice timeline.
pub fn probe_audio_duration_seconds(ffprobe: &Path, media_path: &Path) -> Result<f64> {
    let probe: ProbeOutput = serde_json::from_str(&run_ffprobe_json(ffprobe, media_path)?)
        .context("parsing ffprobe JSON")?;
    let audio_stream_duration = probe
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("audio"))
        .with_context(|| format!("{} has no audio stream", media_path.display()))?
        .duration
        .as_deref()
        .and_then(|raw| raw.parse::<f64>().ok());
    let container_duration = probe
        .format
        .and_then(|format| format.duration)
        .and_then(|raw| raw.parse::<f64>().ok());
    audio_stream_duration
        .or(container_duration)
        .with_context(|| format!("ffprobe reported no duration for {}", media_path.display()))
}

pub fn validate_practice_video(
    ffprobe: &Path,
    video_path: &Path,
    expected_duration_seconds: f64,
) -> Result<ValidatedMedia> {
    let file_size = std::fs::metadata(video_path)
        .with_context(|| format!("{} was not created", video_path.display()))?
        .len();
    if file_size == 0 {
        bail!("{} is empty", video_path.display());
    }
    let probe_json = run_ffprobe_json(ffprobe, video_path)?;
    validate_probe_json(&probe_json, expected_duration_seconds)
        .with_context(|| format!("validating {}", video_path.display()))
}

/// Pure validation over ffprobe's JSON so it can be unit-tested.
fn validate_probe_json(probe_json: &str, expected_duration_seconds: f64) -> Result<ValidatedMedia> {
    let probe: ProbeOutput = serde_json::from_str(probe_json).context("parsing ffprobe JSON")?;
    let find_stream = |kind: &str| {
        probe
            .streams
            .iter()
            .find(|stream| stream.codec_type.as_deref() == Some(kind))
    };
    let video = find_stream("video").context("no video stream")?;
    let audio =
        find_stream("audio").context("no audio stream (practice video must not be silent)")?;

    let audio_codec = audio.codec_name.clone().unwrap_or_default();
    if !ACCEPTED_AUDIO_CODECS.contains(&audio_codec.as_str()) {
        bail!("unexpected audio codec `{audio_codec}` (expected AAC)");
    }

    let stream_duration = |stream: &ProbeStream, kind: &str| -> Result<f64> {
        stream
            .duration
            .as_deref()
            .and_then(|raw| raw.parse::<f64>().ok())
            .with_context(|| format!("{kind} stream has no duration"))
    };
    let video_duration = stream_duration(video, "video")?;
    let audio_duration = stream_duration(audio, "audio")?;

    let audio_video_gap = (video_duration - audio_duration).abs();
    if audio_video_gap > DURATION_TOLERANCE_SECONDS {
        bail!(
            "audio ({audio_duration:.3}s) and video ({video_duration:.3}s) durations differ by {:.0} ms (max {:.0} ms)",
            audio_video_gap * 1000.0,
            DURATION_TOLERANCE_SECONDS * 1000.0
        );
    }
    let expected_gap = (video_duration - expected_duration_seconds).abs();
    if expected_gap > DURATION_TOLERANCE_SECONDS {
        bail!(
            "video is {video_duration:.3}s but the prepared timeline is {expected_duration_seconds:.3}s"
        );
    }

    Ok(ValidatedMedia {
        video_codec: video.codec_name.clone().unwrap_or_default(),
        audio_codec,
        video_duration_seconds: video_duration,
        audio_duration_seconds: audio_duration,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_json(video_duration: &str, audio: Option<(&str, &str)>) -> String {
        let audio_stream = audio
            .map(|(codec, duration)| {
                format!(
                    r#",{{"codec_type":"audio","codec_name":"{codec}","duration":"{duration}"}}"#
                )
            })
            .unwrap_or_default();
        format!(
            r#"{{"streams":[{{"codec_type":"video","codec_name":"h264","duration":"{video_duration}"}}{audio_stream}],"format":{{"duration":"{video_duration}"}}}}"#
        )
    }

    #[test]
    fn accepts_matching_streams() {
        let media =
            validate_probe_json(&probe_json("12.000", Some(("aac", "11.987"))), 12.0).unwrap();
        assert_eq!(media.video_codec, "h264");
        assert_eq!(media.audio_codec, "aac");
    }

    #[test]
    fn rejects_missing_audio() {
        let error = validate_probe_json(&probe_json("12.0", None), 12.0).unwrap_err();
        assert!(format!("{error:#}").contains("no audio stream"));
    }

    #[test]
    fn rejects_short_audio() {
        // Reproduces the review case: 5.433 s video, 1 s audio.
        let error =
            validate_probe_json(&probe_json("5.433", Some(("aac", "1.000"))), 5.433).unwrap_err();
        assert!(format!("{error:#}").contains("differ by"));
    }

    #[test]
    fn rejects_duration_mismatch_with_timeline() {
        assert!(validate_probe_json(&probe_json("10.0", Some(("aac", "10.0"))), 12.0).is_err());
    }

    #[test]
    fn rejects_unexpected_audio_codec() {
        assert!(validate_probe_json(&probe_json("10.0", Some(("mp3", "10.0"))), 10.0).is_err());
    }
}

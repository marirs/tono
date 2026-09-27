//! `tono demo` (milestone M1): hard-coded melody → AE-01 fingering video.
//!
//! Proves rendering and audio/visual timing before any ML exists.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::json;

use crate::audio::{
    synthesize_melody_guide_samples, synthesize_metronome_samples, write_mono_wav, GUIDE_SAMPLE_RATE,
};
use crate::fingering::{map_notes_to_fingerings, FingeringTable};
use crate::media_validation::{probe_audio_duration_seconds, DURATION_TOLERANCE_SECONDS};
use crate::notes::{demo_melody, validate_monophonic_sequence};
use crate::paths;
use crate::render::{render_practice_video, AudioBed, MediaTools, MetronomeMode, RenderSettings};
use crate::timeline::PracticeTimeline;

pub struct DemoOptions {
    pub instrument: crate::instruments::Instrument,
    pub output_directory: PathBuf,
    pub beats_per_minute: f64,
    /// Optional audio to mux instead of the synthesized guide. Its first
    /// seconds play under the demo; it is not aligned to the melody.
    pub backing_audio: Option<PathBuf>,
    pub metronome: MetronomeMode,
    pub keep_work: bool,
}

const TAIL_SECONDS: f64 = 1.5;

pub fn run_demo(options: &DemoOptions) -> Result<()> {
    if !(20.0..=300.0).contains(&options.beats_per_minute) {
        bail!("--bpm must be between 20 and 300");
    }
    if let Some(backing) = &options.backing_audio {
        if !backing.is_file() {
            bail!("--backing file not found: {}", backing.display());
        }
    }
    let ffmpeg = paths::ffmpeg_executable().context("ffmpeg not found; run `tono doctor`")?;
    let ffprobe = paths::ffprobe_executable().context("ffprobe not found; run `tono doctor`")?;

    let out = &options.output_directory;
    let work = out.join("work");
    std::fs::create_dir_all(&work).with_context(|| format!("creating {}", work.display()))?;

    let result = build_demo_outputs(options, &ffmpeg, &ffprobe, &work);
    match &result {
        // Spec: work/ exists only with --keep-work. On failure it is kept
        // regardless, because it holds the ffmpeg log needed to debug.
        Ok(()) if !options.keep_work => {
            std::fs::remove_dir_all(&work).with_context(|| format!("removing {}", work.display()))?
        }
        Ok(()) => {}
        Err(_) => eprintln!("work files kept for debugging: {}", work.display()),
    }
    result?;
    print_next_steps(out);
    Ok(())
}

fn build_demo_outputs(options: &DemoOptions, ffmpeg: &Path, ffprobe: &Path, work: &Path) -> Result<()> {
    let out = &options.output_directory;

    let notes = demo_melody(options.beats_per_minute);
    validate_monophonic_sequence(&notes)?;
    println!("✓ {} hard-coded notes at {} bpm", notes.len(), options.beats_per_minute);

    let table = FingeringTable::load_instrument(options.instrument)?;
    let entries = map_notes_to_fingerings(&notes, &table)?;
    println!(
        "✓ fingerings mapped ({})",
        if table.verified { "verified table" } else { "table NOT yet verified on the instrument" }
    );

    write_json(&out.join("notes.json"), &json!({ "version": 1, "source": "m1-demo", "notes": notes }))?;
    write_json(
        &out.join("fingering.json"),
        &json!({
            "version": 1,
            "instrument": table.instrument,
            "verified": table.verified,
            "timeline": entries,
        }),
    )?;

    let timeline = PracticeTimeline::new(entries, options.beats_per_minute, TAIL_SECONDS);

    let backing_path = prepare_backing_audio(options, &timeline, ffprobe)?;
    let click_path = if options.metronome.plays_click() {
        let click_path = work.join("metronome.wav");
        write_mono_wav(&click_path, &synthesize_metronome_samples(&timeline, GUIDE_SAMPLE_RATE), GUIDE_SAMPLE_RATE)?;
        Some(click_path)
    } else {
        None
    };

    let settings = RenderSettings {
        instrument: options.instrument,
        layout_keys: table.layout_keys.clone(),
        required_settings: table.required_settings.clone(),
        upper_hand: options.instrument.default_upper_hand(),
        fingering_is_unverified: !table.verified,
        metronome: options.metronome,
        subtitle: format!("{}  ·  timing demo  ·  {} bpm", options.instrument.name(), options.beats_per_minute),
        ..RenderSettings::default()
    };
    println!(
        "… rendering {} frames ({:.1}s @ {} fps)",
        timeline.frame_count(settings.frames_per_second),
        timeline.total_duration_seconds,
        settings.frames_per_second
    );
    let ffmpeg_log = work.join("ffmpeg-render.log");
    let validated = render_practice_video(
        &timeline,
        &settings,
        &AudioBed { backing: &backing_path, metronome_click: click_path.as_deref() },
        &MediaTools { ffmpeg, ffprobe, ffmpeg_log: &ffmpeg_log },
        &out.join("practice.mp4"),
    )?;
    println!("✓ practice video rendered");
    println!(
        "✓ ffprobe: {} video {:.3}s, {} audio {:.3}s (within {:.0} ms)",
        validated.video_codec,
        validated.video_duration_seconds,
        validated.audio_codec,
        validated.audio_duration_seconds,
        DURATION_TOLERANCE_SECONDS * 1000.0
    );
    Ok(())
}

/// Returns the audio bed: the user's `--backing` file, or a synthesized
/// melody guide written to `guide.wav`.
fn prepare_backing_audio(options: &DemoOptions, timeline: &PracticeTimeline, ffprobe: &Path) -> Result<PathBuf> {
    let Some(backing) = &options.backing_audio else {
        let guide_path = options.output_directory.join("guide.wav");
        let samples = synthesize_melody_guide_samples(timeline, GUIDE_SAMPLE_RATE);
        write_mono_wav(&guide_path, &samples, GUIDE_SAMPLE_RATE)?;
        println!("✓ guide audio synthesized (melody tones)");
        return Ok(guide_path);
    };

    // The renderer pads short audio with silence so the MP4 streams match;
    // say so explicitly instead of silently producing a quiet ending.
    let backing_duration = probe_audio_duration_seconds(ffprobe, backing)?;
    let shortfall = timeline.total_duration_seconds - backing_duration;
    if shortfall > DURATION_TOLERANCE_SECONDS {
        println!(
            "! backing audio is {backing_duration:.2}s, video is {:.2}s: last {shortfall:.2}s will be silence",
            timeline.total_duration_seconds
        );
    }
    Ok(backing.clone())
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    std::fs::write(path, text + "\n").with_context(|| format!("writing {}", path.display()))
}

fn print_next_steps(out: &Path) {
    println!("\nOutputs:");
    for name in ["practice.mp4", "guide.wav", "notes.json", "fingering.json", "work"] {
        let path = out.join(name);
        if path.exists() {
            println!("  {}", path.display());
        }
    }
    println!("\nCheck timing:");
    println!("  open {}", out.join("practice.mp4").display());
    println!("  # each tone should start exactly when NOW turns solid and PLAY appears;");
    println!("  # with --metronome audio|both, each click should land on a beat-dot flash.");
}

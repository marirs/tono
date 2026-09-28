//! Practice-video composition and ffmpeg rendering.
//! Practice-video renderer.
//!
//! Strategy (spec "MP4 rendering"): vector graphics are rasterized with
//! resvg only when the displayed fingering state changes. Per frame we copy
//! that cached raster and paint the few moving parts (progress bars, beat
//! dots) directly with tiny-skia, then stream raw RGBA to ffmpeg. This keeps
//! frame timing exact (frame i == time i/fps) without rendering 30 vector
//! frames per second.

pub mod cues;
pub mod staff;

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};
use resvg::tiny_skia::{self, Color, Paint, PathBuilder, Pixmap, Rect, Transform};
use resvg::usvg;

use crate::instruments::ae01::DIAGRAM_WIDTH;
use crate::instruments::diagram::{DiagramState, PressedStyle, UpperHand, FONT_FAMILY};
use crate::instruments::fingering::KeyTransition;
use crate::media::validation::{validate_practice_video, ValidatedMedia};
use crate::music::timeline::{frame_time_seconds, CuePhase, NowState, PracticeTimeline};

/// Spec `--metronome off|visual|audio|both`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum MetronomeMode {
    Off,
    Visual,
    Audio,
    Both,
}

impl MetronomeMode {
    pub fn shows_beat_dots(self) -> bool {
        matches!(self, MetronomeMode::Visual | MetronomeMode::Both)
    }

    pub fn plays_click(self) -> bool {
        matches!(self, MetronomeMode::Audio | MetronomeMode::Both)
    }
}

pub struct RenderSettings {
    /// Source song title; demos without one retain the TONO heading.
    pub title: Option<String>,
    pub instrument: crate::instruments::Instrument,
    pub layout_keys: Vec<crate::instruments::LayoutKey>,
    pub required_settings: String,
    pub upper_hand: UpperHand,
    pub width: u32,
    pub height: u32,
    pub frames_per_second: u32,
    /// Shows a caution banner; stays on until the fingering table has been
    /// checked on a physical AE-01 (`verified: true`).
    pub fingering_is_unverified: bool,
    pub metronome: MetronomeMode,
    pub subtitle: String,
    /// Shown in amber under the header on every frame (e.g. octave-folded
    /// notes, low-confidence transcription).
    pub video_warnings: Vec<String>,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings {
            title: None,
            instrument: crate::instruments::Instrument::Ae01,
            layout_keys: Vec::new(),
            required_settings: String::new(),
            upper_hand: UpperHand::Left,
            width: 1920,
            height: 1080,
            frames_per_second: 30,
            fingering_is_unverified: true,
            metronome: MetronomeMode::Visual,
            subtitle: String::new(),
            video_warnings: Vec::new(),
        }
    }
}

/// Audio muxed under the animation. The backing track is the primary bed;
/// the click track stays a separate file and is only mixed here, at render
/// time, so exports without it remain possible (spec "Metronome").
pub struct AudioBed<'a> {
    pub backing: &'a Path,
    pub metronome_click: Option<&'a Path>,
}

pub struct MediaTools<'a> {
    pub ffmpeg: &'a Path,
    pub ffprobe: &'a Path,
    pub ffmpeg_log: &'a Path,
}

// Layout in 1920x1080 frame coordinates. The renderer assumes this frame
// size; `render_practice_video` rejects others rather than drawing garbage.
const NEXT_DIAGRAM_SCALE: f32 = 0.55;
const NEXT_DIAGRAM_Y: f32 = 245.0;
const NOW_DIAGRAM_SCALE: f32 = 0.8;
const NOW_DIAGRAM_Y: f32 = 210.0;
const NOTE_PROGRESS_BAR: (f32, f32, f32, f32) = (90.0, 945.0, 520.0, 12.0); // x, y, w, h
const BEAT_DOTS_CENTER_Y: f32 = 990.0;
const BEAT_DOT_SPACING: f32 = 70.0;
const BEAT_DOT_RADIUS: f32 = 18.0;
const TIMELINE_BAR: (f32, f32, f32, f32) = (120.0, 1040.0, 1680.0, 10.0);

const BACKGROUND_COLOR: &str = "#12151c";
const ACCENT_RGB: (u8, u8, u8) = (0xff, 0xb0, 0x00);
const DOWNBEAT_RGB: (u8, u8, u8) = (0xff, 0xff, 0xff);

/// Renders, encodes and then validates the MP4 with ffprobe. Success is
/// only returned for a file that passed validation (spec rule 20).
pub fn render_practice_video(
    timeline: &PracticeTimeline,
    settings: &RenderSettings,
    audio: &AudioBed,
    tools: &MediaTools,
    output_path: &Path,
) -> Result<ValidatedMedia> {
    if (settings.width, settings.height) != (1920, 1080) {
        bail!("v0 renderer layout is fixed at 1920x1080");
    }

    let svg_options = svg_options_with_system_fonts();
    let mut ffmpeg_process = spawn_ffmpeg_encoder(timeline, settings, audio, tools, output_path)?;
    let mut ffmpeg_stdin = ffmpeg_process
        .stdin
        .take()
        .context("ffmpeg stdin unavailable")?;

    let mut frame = Pixmap::new(settings.width, settings.height).context("allocating frame")?;
    // Only one static layer is cached: states advance monotonically, so a
    // state never reappears once left.
    let mut cached_static_layer: Option<(NowState, Option<u8>, CuePhase, Pixmap)> = None;

    let total_frames = timeline.frame_count(settings.frames_per_second);
    for frame_index in 0..total_frames {
        let time_seconds = frame_time_seconds(frame_index, settings.frames_per_second);
        let now_state = timeline.now_state_at(time_seconds);
        let count = timeline.count_in_number_at(time_seconds);
        let phase = timeline.cue_phase_at(time_seconds, now_state);

        let needs_new_layer =
            cached_static_layer
                .as_ref()
                .map_or(true, |(state, cached_count, cached_phase, _)| {
                    *state != now_state || *cached_count != count || *cached_phase != phase
                });
        if needs_new_layer {
            let svg = static_layer_svg(timeline, settings, now_state, count, phase);
            let layer = rasterize_svg(&svg, &svg_options, settings)?;
            cached_static_layer = Some((now_state, count, phase, layer));
        }
        let (_, _, _, static_layer) = cached_static_layer.as_ref().expect("layer cached above");

        frame.data_mut().copy_from_slice(static_layer.data());
        paint_dynamic_overlays(&mut frame, timeline, settings, time_seconds);

        if let Err(error) = ffmpeg_stdin.write_all(frame.data()) {
            let _ = ffmpeg_process.wait();
            bail!(
                "ffmpeg stopped accepting frames at frame {frame_index} ({error}); see {}",
                tools.ffmpeg_log.display()
            );
        }
    }

    drop(ffmpeg_stdin); // EOF lets ffmpeg finish the file.
    let status = ffmpeg_process.wait().context("waiting for ffmpeg")?;
    if !status.success() {
        bail!(
            "ffmpeg failed with {status}; see {}",
            tools.ffmpeg_log.display()
        );
    }

    // Compare with the prepared timeline, not the frame-rounded encode
    // length; rounding is < 1 frame and well inside the 100 ms tolerance.
    validate_practice_video(tools.ffprobe, output_path, timeline.total_duration_seconds)
}

fn svg_options_with_system_fonts() -> usvg::Options<'static> {
    let mut options = usvg::Options::default();
    options.font_family = "DejaVu Sans".to_owned();
    options.fontdb_mut().load_system_fonts();
    if let Some(root) = crate::runtime::installed_root() {
        options.fontdb_mut().load_fonts_dir(root.join("fonts"));
        options
            .fontdb_mut()
            .load_fonts_dir(root.join("share/fonts"));
    }
    options
}

/// Audio filter graph producing `[aout]`. Every input is converted to
/// 44.1 kHz stereo (so mono guide + stereo BGM mix cleanly) and padded with
/// silence; `-t` then cuts the result to exactly the video length, so a
/// short backing track yields trailing silence instead of a short stream.
fn audio_filter_graph(has_click_track: bool) -> String {
    const NORMALIZE: &str = "aformat=sample_rates=44100:channel_layouts=stereo,apad";
    if has_click_track {
        // normalize=0 keeps unity gain: amix would otherwise halve the BGM.
        format!("[1:a]{NORMALIZE}[bed];[2:a]{NORMALIZE}[click];[bed][click]amix=inputs=2:normalize=0:duration=first[aout]")
    } else {
        format!("[1:a]{NORMALIZE}[aout]")
    }
}

fn spawn_ffmpeg_encoder(
    timeline: &PracticeTimeline,
    settings: &RenderSettings,
    audio: &AudioBed,
    tools: &MediaTools,
    output_path: &Path,
) -> Result<std::process::Child> {
    let log_file = File::create(tools.ffmpeg_log)
        .with_context(|| format!("creating {}", tools.ffmpeg_log.display()))?;
    let frame_size = format!("{}x{}", settings.width, settings.height);
    let encoded_duration =
        timeline.frame_count(settings.frames_per_second) as f64 / settings.frames_per_second as f64;
    let duration = format!("{encoded_duration:.3}");

    let mut command = Command::new(tools.ffmpeg);
    command
        .args(["-hide_banner", "-y"])
        .args([
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &frame_size,
        ])
        .args([
            "-framerate",
            &settings.frames_per_second.to_string(),
            "-i",
            "pipe:0",
        ])
        .arg("-i")
        .arg(audio.backing);
    if let Some(click_track) = audio.metronome_click {
        command.arg("-i").arg(click_track);
    }
    command
        .args([
            "-filter_complex",
            &audio_filter_graph(audio.metronome_click.is_some()),
        ])
        .args(["-map", "0:v:0", "-map", "[aout]"])
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-tune",
            "animation",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
        ])
        .args(["-c:a", "aac", "-b:a", "192k"])
        .args(["-t", &duration, "-movflags", "+faststart"])
        .arg(output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log_file))
        .spawn()
        .with_context(|| format!("spawning {}", tools.ffmpeg.display()))
}

fn rasterize_svg(svg: &str, options: &usvg::Options, settings: &RenderSettings) -> Result<Pixmap> {
    let tree = usvg::Tree::from_str(svg, options).context("parsing generated frame SVG")?;
    let mut pixmap = Pixmap::new(settings.width, settings.height).context("allocating layer")?;
    resvg::render(&tree, Transform::identity(), &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Everything that only changes when the NOW/NEXT fingering changes.
fn static_layer_svg(
    timeline: &PracticeTimeline,
    settings: &RenderSettings,
    now_state: NowState,
    count: Option<u8>,
    phase: CuePhase,
) -> String {
    let (width, height) = (settings.width, settings.height);
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}"><rect width="100%" height="100%" fill="{BACKGROUND_COLOR}"/>"##
    );

    push_header(&mut svg, settings);
    if settings.instrument.is_horizontal() {
        push_horizontal_sections(&mut svg, timeline, now_state, settings, count, phase);
    } else {
        push_next_section(&mut svg, timeline, now_state, settings);
        push_now_section(&mut svg, timeline, now_state, settings, count, phase);
    }
    svg.push_str(&crate::render::staff::staff_svg(timeline, now_state, count));
    push_bar_tracks(&mut svg, timeline, settings.metronome.shows_beat_dots());

    svg.push_str("</svg>");
    svg
}

fn practice_heading(title: &str) -> (String, f32) {
    // Conservative glyph widths, including wide Unicode scripts. Truncation
    // happens on character boundaries before XML escaping.
    let mut heading = String::new();
    let mut units = 0.0_f32;
    for c in format!("Practice: {title}").chars() {
        let width = if c.is_ascii() { 0.85 } else { 1.2 };
        if units + width > 34.0 {
            heading.push('…');
            units += 1.2;
            break;
        }
        heading.push(c);
        units += width;
    }
    (heading, (880.0 / units.max(1.0)).clamp(24.0, 36.0))
}

fn push_header(svg: &mut String, settings: &RenderSettings) {
    if let Some(title) = &settings.title {
        // A bounded single-line title stays clear of the verification banner.
        // The full, unabridged title is retained in project.json.
        let (heading, font_size) = practice_heading(title);
        let _ = write!(
            svg,
            r##"<text x="60" y="35" font-family="{FONT_FAMILY}" font-size="18" font-weight="800" fill="#8a93a3" letter-spacing="3">TONO</text><text x="60" y="76" font-family="{FONT_FAMILY}" font-size="{font_size}" font-weight="700" fill="#ffffff">{}</text>"##,
            xml_escape(&heading)
        );
    } else {
        let _ = write!(
            svg,
            r##"<text x="60" y="72" font-family="{FONT_FAMILY}" font-size="40" font-weight="800" fill="#ffffff" letter-spacing="6">TONO</text>"##
        );
    }
    let _ = write!(
        svg,
        r##"<text x="60" y="108" font-family="{FONT_FAMILY}" font-size="24" fill="#8a93a3">{}</text>"##,
        xml_escape(&settings.subtitle)
    );
    let _ = write!(
        svg,
        r##"<text x="60" y="140" font-family="{FONT_FAMILY}" font-size="16" fill="#8a93a3">{}</text>"##,
        xml_escape(&settings.required_settings)
    );
    for (line_index, warning) in settings.video_warnings.iter().enumerate() {
        let _ = write!(
            svg,
            r##"<text x="1880" y="{y}" font-family="{FONT_FAMILY}" font-size="22" font-weight="700" fill="#ffb000" text-anchor="end">! {text}</text>"##,
            y = 124 + line_index * 30,
            text = xml_escape(warning),
        );
    }
    if settings.fingering_is_unverified {
        svg.push_str(&format!(
            r##"<rect x="1000" y="36" width="880" height="56" rx="10" fill="#4a3a10" stroke="#ffb000" stroke-width="3"/><text x="1440" y="72" font-family="{FONT_FAMILY}" font-size="20" font-weight="700" fill="#ffe0a0" text-anchor="middle">PROFILE FINGERINGS - NOT YET CHECKED ON INSTRUMENT</text>"##
        ));
    }
}

fn push_horizontal_sections(
    svg: &mut String,
    timeline: &PracticeTimeline,
    now: NowState,
    settings: &RenderSettings,
    count: Option<u8>,
    phase: CuePhase,
) {
    let label = if count.is_some() {
        "COUNT IN"
    } else {
        match now {
            NowState::Ready(_) => "GET READY",
            NowState::Sounding(_) if settings.instrument.is_pan() => "STRIKE / LET RING",
            NowState::Sounding(_) => "PLAY",
            NowState::Finished => "DONE",
        }
    };
    let _ = write!(
        svg,
        r##"<text x="460" y="205" font-family="{FONT_FAMILY}" font-size="40" font-weight="800" fill="#ffb000" text-anchor="middle">{label}</text>"##
    );
    push_change_cue(
        svg,
        timeline,
        now,
        settings,
        CueAnchor::UnderNowLabel,
        phase,
    );
    if let Some(index) = now.current_index() {
        let keys = timeline.entries[index].fingering.pressed_key_ids();
        // Positional renderers describe the selected/current position in
        // their own captions. Feeding them an outgoing transition would pair
        // the next action with the current pitch (e.g. lift and replace the
        // finger when the next note is actually open). Their NEXT text above
        // supplies the look-ahead; wind-key rings can preview outgoing changes.
        let transition = if settings.instrument.is_fretted()
            || settings.instrument == crate::instruments::Instrument::Violin
        {
            Some(incoming_transition(timeline, index))
        } else {
            now_ring_transition(timeline, now, phase)
        };
        let state = DiagramState {
            upper_hand: settings.upper_hand,
            pressed_keys: &keys,
            pressed_style: if matches!(now, NowState::Sounding(_)) {
                PressedStyle::Sounding
            } else {
                PressedStyle::Ready
            },
            transition_hint: transition.as_ref(),
        };
        svg.push_str(&crate::instruments::horizontal_svg(
            settings.instrument,
            &settings.layout_keys,
            &state,
            85.0,
            300.0,
            0.95,
            "now-",
        ));
        if let Some(next) = timeline.next_index_after(now) {
            let keys = timeline.entries[next].fingering.pressed_key_ids();
            let transition = KeyTransition::between(
                &timeline.entries[index].fingering,
                &timeline.entries[next].fingering,
            );
            let state = DiagramState {
                upper_hand: settings.upper_hand,
                pressed_keys: &keys,
                pressed_style: PressedStyle::Preview,
                transition_hint: Some(&transition),
            };
            let _ = write!(
                svg,
                r##"<text x="460" y="710" font-family="{FONT_FAMILY}" font-size="24" fill="#8a93a3" text-anchor="middle">NEXT</text>"##
            );
            svg.push_str(&crate::instruments::horizontal_svg(
                settings.instrument,
                &settings.layout_keys,
                &state,
                245.0,
                740.0,
                0.45,
                "next-",
            ));
        }
    }
}

fn push_next_section(
    svg: &mut String,
    timeline: &PracticeTimeline,
    now_state: NowState,
    settings: &RenderSettings,
) {
    if matches!(now_state, NowState::Finished) {
        return;
    }
    let _ = write!(
        svg,
        r##"<text x="760" y="210" font-family="{FONT_FAMILY}" font-size="30" font-weight="700" fill="#8a93a3" text-anchor="middle" letter-spacing="8">NEXT</text>"##
    );
    let next_diagram_x = 771.0 - DIAGRAM_WIDTH * NEXT_DIAGRAM_SCALE / 2.0;
    let transform =
        format!("translate({next_diagram_x} {NEXT_DIAGRAM_Y}) scale({NEXT_DIAGRAM_SCALE})");

    match (
        now_state.current_index(),
        timeline.next_index_after(now_state),
    ) {
        (Some(current_index), Some(next_index)) => {
            let next_keys = timeline.entries[next_index].fingering.pressed_key_ids();
            let transition: Option<&KeyTransition> =
                timeline.entries[current_index].transition_to_next.as_ref();
            let state = DiagramState {
                upper_hand: settings.upper_hand,
                pressed_keys: &next_keys,
                pressed_style: PressedStyle::Preview,
                transition_hint: transition,
            };
            svg.push_str(&crate::instruments::wind_svg(
                settings.instrument,
                &settings.layout_keys,
                &state,
                &transform,
                "next-",
            ));
        }
        _ => {
            let _ = write!(
                svg,
                r##"<text x="760" y="450" font-family="{FONT_FAMILY}" font-size="34" fill="#5a6272" text-anchor="middle">LAST NOTE</text>"##
            );
        }
    }

    // Left arrow from the smaller NEXT preview to the large current fingering.
    svg.push_str(r##"<path d="M650 510 L650 550 L620 530 Z" fill="#5a6272"/>"##);
}

fn push_now_section(
    svg: &mut String,
    timeline: &PracticeTimeline,
    now_state: NowState,
    settings: &RenderSettings,
    count: Option<u8>,
    phase: CuePhase,
) {
    let (label, label_color, style) = match now_state {
        _ if count.is_some() => ("COUNT IN", "#ffb000", PressedStyle::Ready),
        NowState::Ready(_) => ("GET READY", "#c8a24a", PressedStyle::Ready),
        NowState::Sounding(_) => ("PLAY", "#ffb000", PressedStyle::Sounding),
        NowState::Finished => ("DONE", "#8a93a3", PressedStyle::Sounding),
    };
    let _ = write!(
        svg,
        r##"<text x="350" y="190" font-family="{FONT_FAMILY}" font-size="40" font-weight="800" fill="{label_color}" text-anchor="middle" letter-spacing="10">{label}</text>"##
    );
    let pressed_keys = now_state
        .current_index()
        .map(|index| timeline.entries[index].fingering.pressed_key_ids())
        .unwrap_or_default();
    let now_diagram_x = 366.0 - DIAGRAM_WIDTH * NOW_DIAGRAM_SCALE / 2.0;
    let transform =
        format!("translate({now_diagram_x} {NOW_DIAGRAM_Y}) scale({NOW_DIAGRAM_SCALE})");
    let transition = now_ring_transition(timeline, now_state, phase);
    let state = DiagramState {
        upper_hand: settings.upper_hand,
        pressed_keys: &pressed_keys,
        pressed_style: style,
        transition_hint: transition.as_ref(),
    };
    svg.push_str(&crate::instruments::wind_svg(
        settings.instrument,
        &settings.layout_keys,
        &state,
        &transform,
        "now-",
    ));
    push_change_cue(
        svg,
        timeline,
        now_state,
        settings,
        CueAnchor::BelowNextDiagram,
        phase,
    );
}

/// Where the "next change" words sit in each layout.
#[derive(Clone, Copy)]
enum CueAnchor {
    /// Vertical winds: free space under the small NEXT diagram.
    BelowNextDiagram,
    /// Horizontal instruments: one line under the NOW label.
    UnderNowLabel,
}

fn push_change_cue(
    svg: &mut String,
    timeline: &PracticeTimeline,
    now: NowState,
    settings: &RenderSettings,
    anchor: CueAnchor,
    phase: CuePhase,
) {
    let Some((held, upcoming)) = timeline.upcoming_change(now) else {
        return;
    };
    // The box lights up once the NOW rings switch to this change.
    let border = if phase == CuePhase::Prepare {
        "#3ddc84"
    } else {
        "#3a4150"
    };
    let cue = crate::render::cues::change_cue(
        settings.instrument,
        &settings.layout_keys,
        held.map(|index| &timeline.entries[index]),
        &timeline.entries[upcoming],
    );
    let colour = if cue.is_repeat { "#7fd4ff" } else { "#ffffff" };
    match anchor {
        CueAnchor::BelowNextDiagram => {
            let _ = write!(
                svg,
                r##"<rect x="630" y="760" width="370" height="{h}" rx="12" fill="#1b2029" stroke="{border}" stroke-width="3"/><text x="815" y="795" font-family="{FONT_FAMILY}" font-size="22" font-weight="700" fill="#8a93a3" text-anchor="middle" letter-spacing="3">{title}</text>"##,
                h = 50 + 38 * cue.lines.len().max(1),
                title = cue.title,
            );
            for (line_index, line) in cue.lines.iter().enumerate() {
                let _ = write!(
                    svg,
                    r##"<text x="815" y="{y}" font-family="{FONT_FAMILY}" font-size="{size}" font-weight="800" fill="{colour}" text-anchor="middle">{text}</text>"##,
                    y = 836 + 38 * line_index,
                    // ~17 bold caps fit the 370 px box at 30 px.
                    size = if line.chars().count() <= 17 { 30 } else { 22 },
                    text = xml_escape(line),
                );
            }
        }
        CueAnchor::UnderNowLabel => {
            let text = format!("{}: {}", cue.title, cue.lines.join(" · "));
            let rows = horizontal_cue_rows(&text);
            let longest = rows
                .iter()
                .map(|row| row.chars().count())
                .max()
                .unwrap_or(1);
            let size = (870.0 / (longest as f32 * 0.75)).clamp(14.0, 28.0);
            for (index, row) in rows.iter().enumerate() {
                let y = if rows.len() == 1 {
                    245
                } else {
                    233 + index * 28
                };
                let _ = write!(
                    svg,
                    r##"<text x="460" y="{y}" font-family="{FONT_FAMILY}" font-size="{size}" font-weight="800" fill="{colour}" text-anchor="middle">{}</text>"##,
                    xml_escape(row)
                );
            }
        }
    }
}

/// Keep the entire cue readable without colliding with the instrument heading.
fn horizontal_cue_rows(text: &str) -> Vec<String> {
    if text.chars().count() <= 52 {
        return vec![text.into()];
    }
    let middle = text.chars().count() / 2;
    let split = text
        .char_indices()
        .enumerate()
        .filter(|(_, (_, c))| *c == ' ')
        .min_by_key(|(i, _)| i.abs_diff(middle))
        .map(|(_, (byte, _))| byte);
    match split {
        Some(byte) => vec![text[..byte].trim().into(), text[byte..].trim().into()],
        None => vec![text.into()],
    }
}

/// NOW rings: the change just made during the arrival phase, the change to
/// make next during the preparation phase.
fn now_ring_transition(
    timeline: &PracticeTimeline,
    now: NowState,
    phase: CuePhase,
) -> Option<KeyTransition> {
    match (phase, now) {
        (CuePhase::Arrival, NowState::Sounding(index)) => {
            Some(incoming_transition(timeline, index))
        }
        _ => upcoming_transition(timeline, now),
    }
}

/// The change to prepare next (see `PracticeTimeline::upcoming_change`).
fn upcoming_transition(timeline: &PracticeTimeline, now: NowState) -> Option<KeyTransition> {
    let (held, upcoming) = timeline.upcoming_change(now)?;
    Some(match held {
        Some(held) => KeyTransition::between(
            &timeline.entries[held].fingering,
            &timeline.entries[upcoming].fingering,
        ),
        None => incoming_transition(timeline, upcoming),
    })
}

fn incoming_transition(timeline: &PracticeTimeline, index: usize) -> KeyTransition {
    if index == 0 {
        KeyTransition {
            press: timeline.entries[0].fingering.pressed_key_ids(),
            lift: Default::default(),
        }
    } else {
        KeyTransition::between(
            &timeline.entries[index - 1].fingering,
            &timeline.entries[index].fingering,
        )
    }
}

/// Empty tracks for the dynamic bars, plus note spans on the timeline so the
/// player can see phrase shape ahead of time.
fn push_bar_tracks(svg: &mut String, timeline: &PracticeTimeline, show_beat_dots: bool) {
    let (x, y, width, height) = NOTE_PROGRESS_BAR;
    let _ = write!(
        svg,
        r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="8" fill="#262c38"/>"##
    );

    let visible_beat_dots = if show_beat_dots {
        beat_dot_count(timeline)
    } else {
        0
    };
    for dot_index in 0..visible_beat_dots {
        let center_x = beat_dot_center_x(dot_index, visible_beat_dots);
        let _ = write!(
            svg,
            r##"<circle cx="{center_x}" cy="{BEAT_DOTS_CENTER_Y}" r="{BEAT_DOT_RADIUS}" fill="none" stroke="#4a5263" stroke-width="3"/>"##
        );
    }

    let (x, y, width, height) = TIMELINE_BAR;
    let _ = write!(
        svg,
        r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="5" fill="#262c38"/>"##
    );
    let seconds_to_pixels = width as f64 / timeline.total_duration_seconds;
    for entry in &timeline.entries {
        let span_x = x as f64 + entry.start * seconds_to_pixels;
        let span_width = ((entry.end - entry.start) * seconds_to_pixels).max(1.0);
        let _ = write!(
            svg,
            r##"<rect x="{span_x:.1}" y="{y}" width="{span_width:.1}" height="{height}" fill="#4a5263"/>"##
        );
    }
}

/// One dot per beat of the bar when bars are known; otherwise a single
/// pulsing dot, since cycling dots would imply a bar count we do not know.
fn beat_dot_count(timeline: &PracticeTimeline) -> u32 {
    timeline.beats.beats_per_bar.unwrap_or(1)
}

fn beat_dot_center_x(dot_index: u32, dot_count: u32) -> f32 {
    let offset_from_middle = dot_index as f32 - (dot_count as f32 - 1.0) / 2.0;
    350.0 + offset_from_middle * BEAT_DOT_SPACING
}

/// Per-frame moving parts, painted straight onto the frame buffer.
fn paint_dynamic_overlays(
    frame: &mut Pixmap,
    timeline: &PracticeTimeline,
    settings: &RenderSettings,
    time_seconds: f64,
) {
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };

    paint_change_countdown(frame, &mut paint, timeline, time_seconds);
    paint_onset_flash(frame, &mut paint, timeline, settings, time_seconds);

    if settings.metronome.shows_beat_dots() {
        paint_beat_dot(frame, &mut paint, timeline, time_seconds);
    }

    // Timeline: translucent fill for elapsed time plus a white playhead.
    let (x, y, width, height) = TIMELINE_BAR;
    let elapsed_width = width * timeline.timeline_progress_at(time_seconds) as f32;
    paint.set_color_rgba8(255, 255, 255, 70);
    fill_rect(frame, &paint, x, y, elapsed_width, height);
    paint.set_color(Color::WHITE);
    fill_rect(
        frame,
        &paint,
        x + elapsed_width - 2.0,
        y - 8.0,
        4.0,
        height + 16.0,
    );
}

/// In the last moments before a change the countdown turns green: "now".
const CHANGE_NOW_SECONDS: f64 = 0.3;
const CHANGE_NOW_RGB: (u8, u8, u8) = (0x3d, 0xdc, 0x84);
/// A note onset frames the NOW area briefly so back-to-back repeated notes
/// are visible events even though the diagram does not change.
const ONSET_FLASH_SECONDS: f64 = 0.15;

/// Bar filling from the latest onset to the next one: how long until the
/// fingers must move, including across rests.
fn paint_change_countdown(
    frame: &mut Pixmap,
    paint: &mut Paint,
    timeline: &PracticeTimeline,
    time_seconds: f64,
) {
    let Some((fraction, seconds_left)) = timeline.change_countdown_at(time_seconds) else {
        return;
    };
    let rgb = if seconds_left <= CHANGE_NOW_SECONDS {
        CHANGE_NOW_RGB
    } else {
        ACCENT_RGB
    };
    let (x, y, width, height) = NOTE_PROGRESS_BAR;
    paint.set_color_rgba8(rgb.0, rgb.1, rgb.2, 255);
    fill_rect(frame, paint, x, y, width * fraction as f32, height);
}

fn paint_onset_flash(
    frame: &mut Pixmap,
    paint: &mut Paint,
    timeline: &PracticeTimeline,
    settings: &RenderSettings,
    time_seconds: f64,
) {
    let Some(age) = timeline.seconds_since_onset(time_seconds) else {
        return;
    };
    if age >= ONSET_FLASH_SECONDS {
        return;
    }
    let alpha = (220.0 * (1.0 - age / ONSET_FLASH_SECONDS)).round() as u8;
    paint.set_color_rgba8(ACCENT_RGB.0, ACCENT_RGB.1, ACCENT_RGB.2, alpha);
    let (x, y, width, height) = if settings.instrument.is_horizontal() {
        (50.0, 160.0, 940.0, 540.0)
    } else {
        (50.0, 150.0, 590.0, 780.0)
    };
    let thickness = 8.0;
    fill_rect(frame, paint, x, y, width, thickness);
    fill_rect(frame, paint, x, y + height - thickness, width, thickness);
    fill_rect(frame, paint, x, y, thickness, height);
    fill_rect(frame, paint, x + width - thickness, y, thickness, height);
}

/// Beat dot flashes on each beat and fades until the next one. Only a
/// known downbeat gets the accent colour.
fn paint_beat_dot(
    frame: &mut Pixmap,
    paint: &mut Paint,
    timeline: &PracticeTimeline,
    time_seconds: f64,
) {
    let Some(position) = timeline.beat_position_at(time_seconds) else {
        return; // before the first beat
    };
    let dot_count = beat_dot_count(timeline);
    let dot_index = (position.index % dot_count as usize) as u32;
    let rgb = if timeline.beats.is_accented(position.index) {
        DOWNBEAT_RGB
    } else {
        ACCENT_RGB
    };
    let alpha = (255.0 * (1.0 - 0.7 * position.fraction)).round() as u8;
    paint.set_color_rgba8(rgb.0, rgb.1, rgb.2, alpha);
    let center_x = beat_dot_center_x(dot_index, dot_count);
    if let Some(circle) =
        PathBuilder::from_circle(center_x, BEAT_DOTS_CENTER_Y, BEAT_DOT_RADIUS - 2.0)
    {
        frame.fill_path(
            &circle,
            paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn fill_rect(frame: &mut Pixmap, paint: &Paint, x: f32, y: f32, width: f32, height: f32) {
    if let Some(rect) = Rect::from_xywh(x, y, width, height) {
        frame.fill_rect(rect, paint, Transform::identity(), None);
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod current_cue_tests {
    use super::*;
    use crate::instruments::fingering::{Fingering, FingeringTimelineEntry, OctaveShift};

    #[test]
    fn current_cues_use_previous_to_current_not_current_to_next() {
        let entries = [
            vec!["left_1"],
            vec!["left_2"],
            vec!["left_2"],
            vec!["right_1"],
        ]
        .iter()
        .enumerate()
        .map(|(i, keys)| FingeringTimelineEntry {
            start: i as f64,
            end: i as f64 + 0.5,
            midi: 60,
            fingering: Fingering {
                octave: OctaveShift::Normal,
                keys: keys.iter().map(|s| s.to_string()).collect(),
            },
            transition_to_next: None,
        })
        .collect();
        let timeline = PracticeTimeline::new(entries, 100.0, 0.0);
        assert!(incoming_transition(&timeline, 0).press.contains("left_1"));
        let change = incoming_transition(&timeline, 1);
        assert_eq!(change.press, ["left_2".to_string()].into_iter().collect());
        assert_eq!(change.lift, ["left_1".to_string()].into_iter().collect());
        assert_eq!(incoming_transition(&timeline, 2).changed_key_count(), 0);
        let mut svg = String::new();
        // Arrival phase: the NOW rings still confirm previous -> current.
        let arrival =
            now_ring_transition(&timeline, NowState::Sounding(1), CuePhase::Arrival).unwrap();
        assert_eq!(arrival, change);
        push_now_section(
            &mut svg,
            &timeline,
            NowState::Sounding(1),
            &RenderSettings::default(),
            None,
            CuePhase::Arrival,
        );
        assert!(svg.contains(">PRESS</text>"));
        assert!(svg.contains(">LIFT</text>"));
        // Preparation phase: rings switch to the upcoming change (1 -> 2 is a
        // repeat), and the cue box says so.
        let prepare =
            now_ring_transition(&timeline, NowState::Sounding(1), CuePhase::Prepare).unwrap();
        assert_eq!(prepare.changed_key_count(), 0);
        svg.clear();
        push_now_section(
            &mut svg,
            &timeline,
            NowState::Ready(2),
            &RenderSettings::default(),
            None,
            CuePhase::Prepare,
        );
        assert!(svg.contains("SAME KEYS - RE-TONGUE"));
    }
}

#[cfg(test)]
mod title_tests {
    use super::*;
    #[test]
    fn violin_lookahead_does_not_replace_current_placement_instructions() {
        use crate::instruments::{
            fingering::{map_notes_to_fingerings, FingeringTable},
            Instrument,
        };
        use crate::music::notes::NoteEvent;
        let table = FingeringTable::load_instrument(Instrument::Violin).unwrap();
        let notes = [
            NoteEvent {
                start: 0.0,
                end: 0.8,
                midi: 64,
                confidence: 1.0,
            },
            NoteEvent {
                start: 0.8,
                end: 1.8,
                midi: 62,
                confidence: 1.0,
            },
        ];
        let timeline =
            PracticeTimeline::new(map_notes_to_fingerings(&notes, &table).unwrap(), 100.0, 0.0);
        let settings = RenderSettings {
            instrument: Instrument::Violin,
            ..RenderSettings::default()
        };
        let svg = static_layer_svg(
            &timeline,
            &settings,
            NowState::Sounding(0),
            None,
            CuePhase::Prepare,
        );
        assert!(svg.contains("NEXT: D STRING OPEN"));
        assert!(
            !svg.contains("LIFT 1 · PLACE 1"),
            "next open string must not become a current-finger re-placement"
        );
    }

    #[test]
    fn titles_are_safe_svg_and_long_headings_stay_bounded() {
        let options = svg_options_with_system_fonts();
        for title in [
            "Love & <Music>",
            "காதல் இசை",
            &"A very long song title ".repeat(20),
        ] {
            let settings = RenderSettings {
                title: Some(title.into()),
                ..RenderSettings::default()
            };
            let mut svg = String::from(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">"#,
            );
            push_header(&mut svg, &settings);
            svg.push_str("</svg>");
            let tree = usvg::Tree::from_str(&svg, &options).unwrap();
            assert!(tree.size().width() > 0.0);
            assert!(svg.contains("Practice:"));
            assert!(!svg.contains("<Music>"));
        }
        let (heading, _) = practice_heading(&"長い曲名".repeat(50));
        assert!(heading.ends_with('…'));
    }
}

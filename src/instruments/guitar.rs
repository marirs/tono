//! Horizontal guitar fretboard and string/fret position decoding.
use super::diagram::{DiagramState, PressedStyle, FONT_FAMILY};
use std::fmt::Write;

/// Guitar string 1 is high E; fret zero means pluck the open string.
pub fn guitar_position(key: &str) -> Option<(usize, u8)> {
    let (string, fret) = key.strip_prefix('s')?.split_once("_f")?;
    let string: usize = string.parse().ok()?;
    let fret: u8 = fret.parse().ok()?;
    (1..=6).contains(&string).then_some((string, fret))
}

/// Horizontal single-note guitar view: string 1 (high E) at the top, nut left.
/// A five-fret window keeps the active position readable on a phone.
pub fn render(
    instrument: super::Instrument,
    state: &DiagramState,
    x: f32,
    y: f32,
    scale: f32,
    prefix: &str,
) -> String {
    let tuning = instrument.tuning();
    let spacing = 220.0 / (tuning.len() - 1) as f32;
    let selected = state
        .pressed_keys
        .iter()
        .find_map(|key| guitar_position(key));
    let fret = selected.map_or(0, |(_, f)| f);
    let first = fret.saturating_sub(2).max(1).min(15);
    let mut svg = format!(
        r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><rect x="110" y="30" width="660" height="260" rx="12" fill="#30271f" stroke="#6a5b48" stroke-width="3"/>"##
    );
    let _ = write!(
        svg,
        r##"<text x="115" y="0" fill="#c8cfdb" font-size="24">FRETS {first}–{} · HIGHEST STRING AT TOP</text>"##,
        first + 4
    );
    for col in 0..=5 {
        let xx = 110 + col * 132;
        let _ = write!(
            svg,
            r##"<path d="M{xx} 30 V290" stroke="#8a8174" stroke-width="4"/>"##
        );
        if col < 5 {
            let _ = write!(
                svg,
                r##"<text x="{}" y="326" font-size="25" text-anchor="middle" fill="#c8cfdb">{}</text>"##,
                xx + 66,
                first + col as u8
            );
        }
    }
    for (i, midi) in tuning.iter().enumerate() {
        let label = format!("{} {}", i + 1, super::piano::note_name(*midi));
        let yy = 50.0 + i as f32 * spacing;
        let _ = write!(
            svg,
            r##"<text x="55" y="{}" font-size="24" text-anchor="end" fill="#c8cfdb">{label}</text><path d="M75 {yy} H770" stroke="#a4a9b3" stroke-width="{}"/>"##,
            yy + 8.0,
            1.5 + i as f32 * 0.35
        );
    }
    let point = |string: usize, f: u8| -> Option<(f32, f32)> {
        let xx = if f == 0 {
            82.0
        } else if (first..=first + 4).contains(&f) {
            176.0 + (f - first) as f32 * 132.0
        } else {
            return None;
        };
        Some((xx, 50.0 + (string - 1) as f32 * spacing))
    };
    if let Some((string, f)) = selected {
        let (xx, yy) = point(string, f).expect("active fret always visible");
        let fill = if state.pressed_style == PressedStyle::Ready {
            "#6b4f0a"
        } else {
            "#ffb000"
        };
        let _ = write!(
            svg,
            r##"<circle id="{prefix}s{string}_f{f}" cx="{xx}" cy="{yy}" r="19" fill="{fill}" stroke="#ffb000" stroke-width="4"/><text x="440" y="380" font-size="34" font-weight="700" text-anchor="middle" fill="#ffb000">PLUCK STRING {string} · {}</text>"##,
            if f == 0 {
                "OPEN".to_owned()
            } else {
                format!("FRET {f}")
            }
        );
    }
    if let Some(change) = state.transition_hint {
        for (keys, color, dash) in [
            (&change.press, "#3ddc84", ""),
            (&change.lift, "#ff5c5c", " stroke-dasharray=\"9 7\""),
        ] {
            for key in keys {
                if let Some((string, f)) = guitar_position(key) {
                    if let Some((xx, yy)) = point(string, f) {
                        let _ = write!(
                            svg,
                            r##"<circle cx="{xx}" cy="{yy}" r="27" fill="none" stroke="{color}" stroke-width="5"{dash}/>"##
                        );
                    }
                }
            }
        }
        let releases: Vec<_> = change
            .lift
            .iter()
            .filter_map(|key| guitar_position(key))
            .filter(|(_, f)| *f > 0)
            .map(|(s, f)| format!("S{s} F{f}"))
            .collect();
        if !releases.is_empty() {
            let _ = write!(
                svg,
                r##"<text x="440" y="420" font-size="23" text-anchor="middle" fill="#ff5c5c">RELEASE {}</text>"##,
                releases.join(", ")
            );
        }
    }
    svg.push_str("</g>");
    svg
}

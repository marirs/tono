//! 41-key piano accordion: right-hand melody at 8-foot register only.
use super::diagram::{DiagramState, PressedStyle, FONT_FAMILY};
use std::fmt::Write;
pub fn render(state: &DiagramState, x: f32, y: f32, scale: f32, prefix: &str) -> String {
    let mut svg = format!(
        r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><rect x="255" y="0" width="270" height="330" rx="18" fill="#374355"/><text x="390" y="-18" text-anchor="middle" fill="#c8cfdb" font-size="22">RIGHT HAND · LOW AT TOP</text>"##
    );
    let black = |m: u8| matches!(m % 12, 1 | 3 | 6 | 8 | 10);
    for dark in [false, true] {
        for midi in 53u8..=93 {
            if black(midi) != dark {
                continue;
            }
            let n = (53..midi).filter(|m| !black(*m)).count() as f32;
            let ky = if dark { n * 13.0 - 4.0 } else { n * 13.0 };
            let h = if dark { 8.0 } else { 13.0 };
            let w = if dark { 80 } else { 140 };
            let id = format!("key_{midi}");
            let active = state.pressed_keys.contains(&id);
            let fill = if active {
                if state.pressed_style == PressedStyle::Ready {
                    "#a47e27"
                } else {
                    "#ffb000"
                }
            } else if dark {
                "#111827"
            } else {
                "#e5e9ef"
            };
            let _ = write!(
                svg,
                r##"<rect id="{prefix}{id}" x="320" y="{ky}" width="{w}" height="{h}" fill="{fill}" stroke="#677184" stroke-width="1"/>"##
            );
        }
    }
    if let Some(key) = state
        .pressed_keys
        .iter()
        .find_map(|k| k.strip_prefix("key_").and_then(|m| m.parse::<u8>().ok()))
    {
        let _ = write!(
            svg,
            r##"<text x="550" y="195" fill="#ffb000" font-size="28">{}</text>"##,
            super::piano::note_name(key)
        );
    }
    svg.push_str(r##"<text x="390" y="362" text-anchor="middle" fill="#c8cfdb" font-size="22">MELODY ONLY · 8-FOOT REGISTER</text><text x="390" y="390" text-anchor="middle" fill="#8a93a3" font-size="18">BASS, CHORDS AND BELLOWS DIRECTION NOT SCORED</text></g>"##);
    svg
}

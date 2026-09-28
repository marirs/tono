//! Shared sax drawing; model-specific controls come from profile JSON.
use super::{
    diagram::{DiagramState, PressedStyle, FONT_FAMILY},
    LayoutKey,
};
use std::fmt::Write;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render(keys: &[LayoutKey], state: &DiagramState, transform: &str, prefix: &str) -> String {
    let mut svg = format!(
        r##"<g transform="{transform}" font-family="{FONT_FAMILY}"><path d="M270 10 L325 10 L345 95 L255 95 Z" fill="#3a4150" stroke="#5a6272" stroke-width="3"/><path d="M255 95 L350 95 L460 520 L385 865 L220 865 L155 620 L225 350 Z" fill="#1b2029" stroke="#4a5263" stroke-width="4"/><rect x="480" y="130" width="150" height="360" rx="22" fill="none" stroke="#4a5263" stroke-dasharray="6 6"/><text x="555" y="105" font-size="21" text-anchor="middle" fill="#8a93a3">BACK / OCTAVE</text>"##
    );
    let (upper, lower) = match state.upper_hand {
        super::diagram::UpperHand::Left => ("LEFT", "RIGHT"),
        _ => ("RIGHT", "LEFT"),
    };
    let _ = write!(
        svg,
        r##"<text x="120" y="240" font-size="22" text-anchor="end" fill="#8a93a3">{upper}</text><text x="120" y="270" font-size="22" text-anchor="end" fill="#8a93a3">HAND</text><text x="120" y="650" font-size="22" text-anchor="end" fill="#8a93a3">{lower}</text><text x="120" y="680" font-size="22" text-anchor="end" fill="#8a93a3">HAND</text>"##
    );
    let _ = write!(
        svg,
        r##"<text x="555" y="125" font-size="17" text-anchor="middle" fill="#8a93a3">{upper} THUMB</text>"##
    );
    for key in keys {
        let pressed = state.pressed_keys.contains(&key.id);
        let fill = if pressed {
            if state.pressed_style == PressedStyle::Ready {
                "#6b4f0a"
            } else {
                "#ffb000"
            }
        } else {
            "#262c38"
        };
        let stroke = if pressed { "#ffb000" } else { "#5a6272" };
        let _ = write!(
            svg,
            r##"<g id="{}{}">{}</g>"##,
            escape(prefix),
            escape(&key.id),
            key_shape(key, 0.0, fill, stroke, false)
        );
        if let Some(change) = state.transition_hint {
            if change.press.contains(&key.id) {
                svg.push_str(&key_shape(key, 8.0, "none", "#3ddc84", false));
            }
            if change.lift.contains(&key.id) {
                svg.push_str(&key_shape(key, 8.0, "none", "#ff5c5c", true));
            }
        }
        let color = if pressed { "#12151c" } else { "#c8cfdb" };
        let _ = write!(
            svg,
            r##"<text x="{}" y="{}" font-size="21" font-weight="700" text-anchor="middle" fill="{color}">{}</text>"##,
            key.x,
            key.y + 7.0,
            escape(&key.label)
        );
    }
    if let Some(change) = state.transition_hint {
        for (x, label, color, present) in [
            (230, "PRESS", "#3ddc84", !change.press.is_empty()),
            (390, "LIFT", "#ff5c5c", !change.lift.is_empty()),
        ] {
            if present {
                let _ = write!(
                    svg,
                    r##"<text x="{x}" y="910" font-size="30" text-anchor="middle" font-weight="800" fill="{color}">{label}</text>"##
                );
            }
        }
    }
    svg.push_str("</g>");
    svg
}

fn key_shape(key: &LayoutKey, grow: f32, fill: &str, stroke: &str, dashed: bool) -> String {
    let dash = if dashed {
        " stroke-dasharray=\"9 7\""
    } else {
        ""
    };
    if key.shape == "circle" {
        format!(
            r##"<circle cx="{}" cy="{}" r="{}" fill="{fill}" stroke="{stroke}" stroke-width="4"{dash}/>"##,
            key.x,
            key.y,
            24.0 + grow
        )
    } else {
        let width = if key.shape == "octave" { 100.0 } else { 44.0 };
        format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="9" fill="{fill}" stroke="{stroke}" stroke-width="4"{dash}/>"##,
            key.x - width / 2.0 - grow,
            key.y - 18.0 - grow,
            width + 2.0 * grow,
            36.0 + 2.0 * grow
        )
    }
}

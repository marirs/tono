//! Full keyboard overview and a larger, pitch-accurate active-key view.
use super::diagram::{DiagramState, PressedStyle, FONT_FAMILY};
use std::fmt::Write;

pub fn note_name(midi: u8) -> String {
    let name = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ][midi as usize % 12];
    format!("{name}{}", midi as i16 / 12 - 1)
}

fn black(midi: u8) -> bool {
    matches!(midi % 12, 1 | 3 | 6 | 8 | 10)
}
fn pitch(key: &str) -> Option<u8> {
    key.strip_prefix("key_")?.parse().ok()
}

fn keyboard(
    range: (u8, u8),
    state: &DiagramState,
    y: f32,
    height: f32,
    prefix: &str,
    labels: bool,
) -> String {
    let (low, high) = range;
    let whites = (low..=high).filter(|m| !black(*m)).count();
    let width = 780.0 / whites as f32;
    let mut svg = String::new();
    // Paint whites first so black keys sit on top of their neighbours.
    for dark in [false, true] {
        for midi in low..=high {
            if black(midi) != dark {
                continue;
            }
            let before = (low..midi).filter(|m| !black(*m)).count() as f32;
            let x = if dark {
                before * width - width * 0.31
            } else {
                before * width
            };
            let w = if dark { width * 0.62 } else { width };
            let h = if dark { height * 0.62 } else { height };
            let id = format!("key_{midi}");
            let pressed = state.pressed_keys.contains(&id);
            let fill = if pressed {
                if state.pressed_style == PressedStyle::Ready {
                    "#8a681b"
                } else {
                    "#ffb000"
                }
            } else if dark {
                "#151922"
            } else {
                "#e5e9f0"
            };
            let _ = write!(
                svg,
                r##"<rect id="{prefix}{id}" x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}" stroke="#566070" stroke-width="1"/>"##
            );
            if let Some(change) = state.transition_hint {
                let color = if change.press.contains(&id) {
                    Some("#3ddc84")
                } else if change.lift.contains(&id) {
                    Some("#ff5c5c")
                } else {
                    None
                };
                if let Some(color) = color {
                    let dash = if change.lift.contains(&id) {
                        " stroke-dasharray=\"5 3\""
                    } else {
                        ""
                    };
                    let _ = write!(
                        svg,
                        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{color}" stroke-width="3"{dash}/>"##,
                        x + 2.0,
                        y + 2.0,
                        (w - 4.0).max(1.0),
                        h - 4.0
                    );
                }
            }
            if labels && (!dark || pressed) {
                let color = if dark { "#ffffff" } else { "#151922" };
                let _ = write!(
                    svg,
                    r##"<text x="{}" y="{}" text-anchor="middle" font-size="17" fill="{color}">{}</text>"##,
                    x + w / 2.0,
                    y + h - 12.0,
                    note_name(midi)
                );
            }
        }
    }
    svg
}

pub fn render(
    range: (u8, u8),
    state: &DiagramState,
    x: f32,
    y: f32,
    scale: f32,
    prefix: &str,
) -> String {
    let active = state.pressed_keys.iter().find_map(|key| pitch(key));
    let mut svg = format!(
        r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><text x="390" y="-20" font-size="23" text-anchor="middle" fill="#c8cfdb">{} KEYS · {}–{} · LOW TO HIGH</text>"##,
        range.1 - range.0 + 1,
        note_name(range.0),
        note_name(range.1)
    );
    svg.push_str(&keyboard(
        range,
        state,
        0.0,
        135.0,
        &format!("{prefix}full-"),
        false,
    ));
    let center = active.unwrap_or(60).clamp(range.0, range.1);
    let mut low = center.saturating_sub(12).max(range.0).min(range.1 - 24);
    if black(low) {
        low = low.saturating_sub(1).max(range.0);
    }
    let mut high = (low + 24).min(range.1);
    if black(high) {
        high -= 1;
    }
    svg.push_str(&keyboard((low, high), state, 195.0, 175.0, prefix, true));
    let _ = write!(
        svg,
        r##"<text x="390" y="178" font-size="20" text-anchor="middle" fill="#8a93a3">ACTIVE KEYS ENLARGED</text>"##
    );
    if let Some(midi) = active {
        let same = state
            .transition_hint
            .is_some_and(|t| t.press.is_empty() && t.lift.is_empty());
        let released = state
            .transition_hint
            .map(|t| {
                t.lift
                    .iter()
                    .filter_map(|key| pitch(key))
                    .map(note_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let cue = if same {
            "SAME AGAIN".to_owned()
        } else if released.is_empty() {
            "PRESS".to_owned()
        } else {
            format!("PRESS · RELEASE {released}")
        };
        let _ = write!(
            svg,
            r##"<text x="390" y="410" font-size="28" font-weight="700" text-anchor="middle" fill="#ffb000">{} · {cue}</text>"##,
            note_name(midi)
        );
    }
    svg.push_str("</g>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::{
        diagram::UpperHand,
        fingering::{Fingering, KeyTransition, OctaveShift},
    };
    #[test]
    fn full_keyboard_contains_every_key_and_transition_cues() {
        let previous = Fingering {
            octave: OctaveShift::Normal,
            keys: vec!["key_60".into()],
        };
        let current = Fingering {
            octave: OctaveShift::Normal,
            keys: vec!["key_61".into()],
        };
        let keys = current.pressed_key_ids();
        let transition = KeyTransition::between(&previous, &current);
        let state = DiagramState {
            upper_hand: UpperHand::Right,
            pressed_keys: &keys,
            pressed_style: PressedStyle::Sounding,
            transition_hint: Some(&transition),
        };
        let svg = render((21, 108), &state, 0.0, 0.0, 1.0, "now-");
        for midi in 21..=108 {
            assert!(svg.contains(&format!("id=\"now-full-key_{midi}\"")));
        }
        assert!(svg.contains("#3ddc84") && svg.contains("#ff5c5c"));
        assert!(svg.contains("C#4"));
        assert!(svg.contains("PRESS · RELEASE C4"));
    }
}

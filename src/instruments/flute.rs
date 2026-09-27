//! Acoustic concert flute controls and air-register cues, independent of Brisa.
use super::{
    diagram::{DiagramState, PressedStyle, FONT_FAMILY},
    LayoutKey,
};
use std::fmt::Write;

pub fn render(
    keys: &[LayoutKey],
    state: &DiagramState,
    x: f32,
    y: f32,
    scale: f32,
    prefix: &str,
) -> String {
    let mut svg = format!(
        r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><rect x="10" y="130" width="780" height="40" rx="15" fill="#3b4555" stroke="#8994a3" stroke-width="3"/><ellipse cx="42" cy="150" rx="22" ry="13" fill="#111824"/><text x="210" y="30" text-anchor="middle" font-size="22" fill="#8a93a3">LEFT HAND</text><text x="535" y="30" text-anchor="middle" font-size="22" fill="#8a93a3">RIGHT HAND</text><text x="180" y="242" text-anchor="middle" font-size="19" fill="#8a93a3">THUMB LEVERS</text>"##
    );
    for key in keys.iter().filter(|k| !k.id.starts_with("register_")) {
        let (kx, ky) = (key.x, key.y);
        let pressed = state.pressed_keys.contains(&key.id);
        let fill = if pressed {
            if state.pressed_style == PressedStyle::Ready {
                "#6b4f0a"
            } else {
                "#ffb000"
            }
        } else {
            "#252d39"
        };
        let label_color = if pressed { "#151922" } else { "#d6deeb" };
        if key.shape == "circle" {
            let _ = write!(
                svg,
                r##"<circle id="{prefix}{}" cx="{kx}" cy="{ky}" r="27" fill="{fill}" stroke="#8994a3" stroke-width="3"/>"##,
                key.id
            );
        } else {
            let _ = write!(
                svg,
                r##"<rect id="{prefix}{}" x="{}" y="{}" width="56" height="36" rx="8" fill="{fill}" stroke="#8994a3" stroke-width="3"/>"##,
                key.id,
                kx - 28.0,
                ky - 18.0
            );
        }
        if let Some(t) = state.transition_hint {
            let press = t.press.contains(&key.id);
            let lift = t.lift.contains(&key.id);
            if press || lift {
                let stroke = if press { "#3ddc84" } else { "#ff5c5c" };
                let dash = if press {
                    ""
                } else {
                    " stroke-dasharray=\"8 5\""
                };
                let _ = write!(
                    svg,
                    r##"<circle cx="{kx}" cy="{ky}" r="35" fill="none" stroke="{stroke}" stroke-width="4"{dash}/>"##
                );
            }
        }
        let _ = write!(
            svg,
            r##"<text x="{kx}" y="{}" text-anchor="middle" font-size="18" font-weight="700" fill="{label_color}">{}</text>"##,
            ky + 6.0,
            key.label
        );
    }
    let register = if state.pressed_keys.contains("register_high") {
        "HIGH"
    } else if state.pressed_keys.contains("register_middle") {
        "MIDDLE"
    } else {
        "LOW"
    };
    let _ = write!(
        svg,
        r##"<text x="395" y="350" text-anchor="middle" font-size="26" font-weight="700" fill="#ffb000">{register} REGISTER - ADJUST AIR / EMBOUCHURE</text>"##
    );
    let register_change = state
        .transition_hint
        .is_some_and(|t| t.press.iter().any(|k| k.starts_with("register_")));
    let hint = if register_change {
        "CHANGE AIR REGISTER; FOLLOW HIGHLIGHTED KEYS"
    } else {
        "GREEN: PRESS / RED: LIFT"
    };
    let _ = write!(
        svg,
        r##"<text x="395" y="395" text-anchor="middle" font-size="21" fill="#c8cfdb">{hint}</text></g>"##
    );
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::{
        diagram::UpperHand,
        fingering::{FingeringTable, KeyTransition},
        Instrument,
    };
    #[test]
    fn footjoint_range_and_register_changes_are_explicit() {
        let c = FingeringTable::load_instrument(Instrument::Flute).unwrap();
        let b = FingeringTable::load_instrument(Instrument::FluteBFoot).unwrap();
        assert!(c.lookup(59).is_err());
        assert!(b.lookup(59).unwrap().keys.contains(&"B".into()));
        assert!(!c.layout_keys.iter().any(|k| k.id == "B"));
        assert_eq!(
            c.lookup(74).unwrap().keys,
            ["2", "3", "4", "5", "6", "thumb_b", "register_middle"]
        );
        assert!(c.alternatives["70"]
            .iter()
            .any(|f| f.keys.contains(&"thumb_bb".into())));
        let change = KeyTransition::between(c.lookup(64).unwrap(), c.lookup(76).unwrap());
        assert_eq!(change.press, ["register_middle".to_owned()].into());
        assert_eq!(change.lift, ["register_low".to_owned()].into());
        let pressed = c.lookup(76).unwrap().pressed_key_ids();
        let state = DiagramState {
            upper_hand: UpperHand::Left,
            pressed_keys: &pressed,
            pressed_style: PressedStyle::Sounding,
            transition_hint: Some(&change),
        };
        let svg = render(&c.layout_keys, &state, 0.0, 0.0, 1.0, "now-");
        assert!(svg.contains("MIDDLE REGISTER"));
        assert!(svg.contains("CHANGE AIR REGISTER"));
        assert!(!svg.contains("id=\"now-register_"));
        assert!(!svg.contains("OCTAVE"));
    }
}

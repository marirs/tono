//! Soprano recorder holes, split lower holes and the vented rear thumb.
use super::{
    diagram::{DiagramState, PressedStyle, UpperHand, FONT_FAMILY},
    LayoutKey,
};
use std::fmt::Write;

pub fn render(keys: &[LayoutKey], state: &DiagramState, transform: &str, prefix: &str) -> String {
    let mut svg = format!(
        r##"<g transform="{transform}" font-family="{FONT_FAMILY}"><path d="M270 15 H330 L342 110 L355 820 Q300 880 245 820 L258 110 Z" fill="#242c37" stroke="#758091" stroke-width="3"/><rect x="279" y="75" width="42" height="30" fill="#101722"/><rect x="470" y="170" width="120" height="175" rx="20" fill="none" stroke="#758091" stroke-dasharray="6 6"/><text x="530" y="145" text-anchor="middle" font-size="22" fill="#8a93a3">BACK / THUMB</text>"##
    );
    let (upper, lower) = if state.upper_hand == UpperHand::Right {
        ("RIGHT", "LEFT")
    } else {
        ("LEFT", "RIGHT")
    };
    let _ = write!(
        svg,
        r##"<text x="175" y="270" text-anchor="end" font-size="23" fill="#8a93a3">{upper} HAND</text><text x="175" y="570" text-anchor="end" font-size="23" fill="#8a93a3">{lower} HAND</text>"##
    );
    for key in keys.iter().filter(|k| k.id != "0_vent") {
        let (x, y) = (key.x, key.y);
        let vent = key.id == "0" && state.pressed_keys.contains("0_vent");
        let pressed = state.pressed_keys.contains(&key.id);
        let color = if state.pressed_style == PressedStyle::Ready {
            "#6b4f0a"
        } else {
            "#ffb000"
        };
        let fill = if pressed { color } else { "#101722" };
        let radius = if key.id.ends_with('b') {
            12.0
        } else if key.id.ends_with('a') {
            17.0
        } else {
            25.0
        };
        let _ = write!(
            svg,
            r##"<circle id="{prefix}{}" cx="{x}" cy="{y}" r="{radius}" fill="{fill}" stroke="#758091" stroke-width="3"/>"##,
            key.id
        );
        if vent {
            let _ = write!(
                svg,
                r##"<path id="{prefix}0_vent" d="M{x} {y} L{x} {} A25 25 0 1 1 {} {y} Z" fill="{color}"/>"##,
                y - 25.0,
                x - 25.0
            );
        }
        if let Some(t) = state.transition_hint {
            let press = t.press.contains(&key.id) || (key.id == "0" && t.press.contains("0_vent"));
            let lift = t.lift.contains(&key.id) || (key.id == "0" && t.lift.contains("0_vent"));
            if press || lift {
                let stroke = if press { "#3ddc84" } else { "#ff5c5c" };
                let dash = if press {
                    ""
                } else {
                    " stroke-dasharray=\"8 5\""
                };
                let _ = write!(
                    svg,
                    r##"<circle cx="{x}" cy="{y}" r="{}" fill="none" stroke="{stroke}" stroke-width="4"{dash}/>"##,
                    radius + 7.0
                );
            }
        }
        let label_y = if key.id == "0" { y + 54.0 } else { y + 6.0 };
        let label_size = if radius < 20.0 { 12 } else { 18 };
        let label_color = if pressed && key.id != "0" {
            "#151922"
        } else {
            "#c8cfdb"
        };
        let _ = write!(
            svg,
            r##"<text x="{x}" y="{label_y}" text-anchor="middle" font-size="{label_size}" fill="{label_color}">{}</text>"##,
            key.label
        );
    }
    let thumb = if state.pressed_keys.contains("0_vent") {
        "THUMB: LEAVE 1/4 OPEN"
    } else if state.pressed_keys.contains("0") {
        "THUMB: SEAL HOLE"
    } else {
        "THUMB: OPEN"
    };
    let _ = write!(
        svg,
        r##"<text x="340" y="880" text-anchor="middle" font-size="27" font-weight="700" fill="#ffb000">{thumb}</text>"##
    );
    if state
        .transition_hint
        .is_some_and(|t| !t.press.is_empty() || !t.lift.is_empty())
    {
        svg.push_str(r##"<text x="340" y="910" text-anchor="middle" font-size="21" fill="#c8cfdb">GREEN: COVER / RED: UNCOVER</text>"##);
    }
    svg.push_str("</g>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::{fingering::FingeringTable, Instrument};
    #[test]
    fn recorder_uses_sounding_octave_and_distinct_systems() {
        let b = FingeringTable::load_instrument(Instrument::RecorderBaroque).unwrap();
        let g = FingeringTable::load_instrument(Instrument::RecorderGerman).unwrap();
        assert_eq!(b.charted_range(), Some((72, 98)));
        assert!(b.lookup(60).is_err());
        assert!(b.lookup(77).unwrap().keys.contains(&"7b".into()));
        assert_eq!(g.lookup(77).unwrap().keys, ["0", "1", "2", "3", "4"]);
        assert!(b.lookup(73).unwrap().keys.contains(&"7a".into()));
        assert!(!b.lookup(73).unwrap().keys.contains(&"7b".into()));
        assert!(b.alternatives.contains_key("95"));
        assert!(g.alternatives.contains_key("90"));
        let pressed = b.lookup(88).unwrap().pressed_key_ids();
        let state = DiagramState {
            upper_hand: UpperHand::Left,
            pressed_keys: &pressed,
            pressed_style: PressedStyle::Sounding,
            transition_hint: None,
        };
        let svg = render(&b.layout_keys, &state, "translate(0 0)", "now-");
        assert!(svg.contains("id=\"now-0_vent\""));
        assert!(svg.contains("LEAVE 1/4 OPEN"));
    }
}

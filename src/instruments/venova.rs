//! Alto Venova controls, numbered as in Yamaha’s chart, shown mouthpiece-up.
use super::{
    diagram::{DiagramState, PressedStyle, UpperHand, FONT_FAMILY},
    LayoutKey,
};
use std::fmt::Write;

pub fn render(keys: &[LayoutKey], state: &DiagramState, transform: &str, prefix: &str) -> String {
    let mut svg = format!(
        r##"<g transform="{transform}" font-family="{FONT_FAMILY}"><path d="M280 15 H320 L330 110 L335 820 Q300 845 265 820 L270 110 Z" fill="#242c37" stroke="#758091" stroke-width="3"/><rect x="279" y="75" width="42" height="30" fill="#101722"/><rect x="475" y="170" width="140" height="240" rx="20" fill="none" stroke="#758091" stroke-dasharray="6 6"/><text x="530" y="145" text-anchor="middle" font-size="22" fill="#8a93a3">BACK / THUMB 8</text>"##
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
    for key in keys.iter().filter(|k| k.id != "8vent") {
        let (x, y) = (key.x, key.y);
        let vent = key.id == "8h" && state.pressed_keys.contains("8vent");
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
                r##"<path id="{prefix}8vent" d="M{x} {y} L{x} {} A25 25 0 1 1 {} {y} Z" fill="{color}"/>"##,
                y - 25.0,
                x - 25.0
            );
        }
        if let Some(t) = state.transition_hint {
            let press = t.press.contains(&key.id) || (key.id == "8h" && t.press.contains("8vent"));
            let lift = t.lift.contains(&key.id) || (key.id == "8h" && t.lift.contains("8vent"));
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
        let label_y = y + 43.0;
        let label_size = if radius < 20.0 { 12 } else { 18 };
        let label_color = if pressed && key.id != "8h" {
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
    let thumb = if state.pressed_keys.contains("8vent") {
        "THUMB: COVER 3/4 / KEY RELEASED"
    } else if state.pressed_keys.contains("8key") {
        "THUMB: SEAL + PRESS OCTAVE"
    } else if state.pressed_keys.contains("8h") {
        "THUMB: SEAL / KEY RELEASED"
    } else {
        "THUMB: OPEN / KEY RELEASED"
    };
    let _ = write!(
        svg,
        r##"<text x="340" y="860" text-anchor="middle" font-size="22" font-weight="700" fill="#ffb000">{thumb}</text>"##
    );
    if state
        .transition_hint
        .is_some_and(|t| !t.press.is_empty() || !t.lift.is_empty())
    {
        svg.push_str(r##"<text x="340" y="890" text-anchor="middle" font-size="21" fill="#c8cfdb">GREEN: PRESS/COVER / RED: RELEASE</text>"##);
    }
    svg.push_str("</g>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::{fingering::FingeringTable, Instrument};
    #[test]
    fn concert_pitch_chart_and_thumb_actions_are_distinct() {
        let t = FingeringTable::load_instrument(Instrument::Yvs120).unwrap();
        assert_eq!(t.charted_range(), Some((53, 77)));
        assert!(t.lookup(52).is_err() && t.lookup(78).is_err());
        assert_eq!(
            t.lookup(53).unwrap().keys,
            ["1a", "1b", "2a", "2b", "3", "4h", "5", "6", "7", "8h"]
        );
        assert_eq!(t.lookup(60).unwrap().keys, ["5", "6", "7", "8h"]);
        assert_eq!(t.lookup(67).unwrap().keys, ["6"]);
        assert_eq!(t.lookup(77).unwrap().keys, ["6", "8h", "8key"]);
        assert_eq!(t.alternatives["71"].len(), 3);
        for (midi, cue) in [
            (67, "THUMB: OPEN"),
            (68, "THUMB: COVER 3/4"),
            (77, "THUMB: SEAL + PRESS OCTAVE"),
        ] {
            let pressed = t.lookup(midi).unwrap().pressed_key_ids();
            let state = DiagramState {
                upper_hand: UpperHand::Left,
                pressed_keys: &pressed,
                pressed_style: PressedStyle::Sounding,
                transition_hint: None,
            };
            let svg = render(&t.layout_keys, &state, "translate(0 0)", "now-");
            assert!(svg.contains(cue));
            assert!(svg.contains("now-8h"));
            assert!(!svg.contains("octave_up"));
        }
    }
}

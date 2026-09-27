//! Fretless first-position violin guide. Placements are not guitar frets.
use super::diagram::{DiagramState, PressedStyle, FONT_FAMILY};
use std::fmt::Write;

const STRINGS: [&str; 4] = ["E", "A", "D", "G"];
// Reviewed first-position chart: E/A use low fourth at +6; D/G use high third.
const FINGERS: [[u8; 8]; 4] = [
    [0, 1, 1, 2, 2, 3, 4, 4],
    [0, 1, 1, 2, 2, 3, 4, 4],
    [0, 1, 1, 2, 2, 3, 3, 4],
    [0, 1, 1, 2, 2, 3, 3, 4],
];

/// (string 1-4, semitones above open 0-7, left finger 0-4).
pub fn position(key: &str) -> Option<(usize, u8, u8)> {
    let (string, rest) = key.strip_prefix('s')?.split_once("_p")?;
    let (offset, finger) = rest.split_once("_n")?;
    let (string, offset, finger): (usize, u8, u8) = (
        string.parse().ok()?,
        offset.parse().ok()?,
        finger.parse().ok()?,
    );
    if !(1..=4).contains(&string) || offset > 7 {
        return None;
    }
    (FINGERS[string - 1][offset as usize] == finger
        && key == format!("s{string}_p{offset}_n{finger}"))
    .then_some((string, offset, finger))
}

fn placement(offset: u8, finger: u8) -> String {
    match (offset, finger) {
        (0, _) => "OPEN STRING".into(),
        (1, 1) | (3, 2) | (6, 4) => format!("LOW {finger}"),
        (4, 2) | (6, 3) => format!("HIGH {finger}"),
        _ => format!("{finger}"),
    }
}

fn coordinates(string: usize, offset: u8) -> (f32, f32) {
    let x = if offset == 0 {
        72.0
    } else {
        // Equal temperament: spacing gets closer towards the bridge.
        125.0
            + 610.0 * (1.0 - 2.0_f32.powf(-(offset as f32) / 12.0))
                / (1.0 - 2.0_f32.powf(-7.0 / 12.0))
    };
    (x, 65.0 + (string - 1) as f32 * 65.0)
}

pub fn render(state: &DiagramState, x: f32, y: f32, scale: f32, prefix: &str) -> String {
    let selected = state
        .pressed_keys
        .iter()
        .find_map(|k| position(k).map(|p| (k, p)));
    let mut svg = format!(
        r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><text x="395" y="0" text-anchor="middle" font-size="23" fill="#c8cfdb">FIRST POSITION · LEFT HAND · E STRING AT TOP</text><path d="M125 38 L780 22 V306 L125 282 Z" fill="#202632" stroke="#657083" stroke-width="3"/><path d="M125 38 V282" stroke="#b0b6c0" stroke-width="7"/>"##
    );
    for string in 1..=4 {
        let (_, yy) = coordinates(string, 0);
        let _ = write!(
            svg,
            r##"<path d="M55 {yy} H775" stroke="#8994a3" stroke-width="{}"/><text x="20" y="{}" text-anchor="middle" font-size="24" fill="#c8cfdb">{}</text>"##,
            1.5 + string as f32 * 0.5,
            yy + 8.0,
            STRINGS[string - 1]
        );
        for offset in 1..=7 {
            let (xx, yy) = coordinates(string, offset);
            let _ = write!(
                svg,
                r##"<circle cx="{xx}" cy="{yy}" r="3" fill="#626c7c"/>"##
            );
        }
    }
    let previous = state
        .transition_hint
        .and_then(|t| t.lift.iter().find_map(|k| position(k)));
    if let Some((string, offset, finger)) = previous {
        if finger != 0 {
            let (xx, yy) = coordinates(string, offset);
            let _ = write!(
                svg,
                r##"<circle cx="{xx}" cy="{yy}" r="25" fill="#202632" stroke="#ff5c5c" stroke-width="4" stroke-dasharray="6 4"/><text x="{xx}" y="{}" text-anchor="middle" font-size="22" fill="#ff5c5c">{finger}</text>"##,
                yy + 8.0
            );
        }
    }
    if let Some((key, (string, offset, finger))) = selected {
        let (xx, yy) = coordinates(string, offset);
        let fill = if state.pressed_style == PressedStyle::Ready {
            "#6b4f0a"
        } else {
            "#ffb000"
        };
        let _ = write!(
            svg,
            r##"<circle id="{prefix}{key}" cx="{xx}" cy="{yy}" r="24" fill="{fill}" stroke="#ffb000" stroke-width="3"/><text x="{xx}" y="{}" text-anchor="middle" font-size="25" font-weight="700" fill="#101620">{finger}</text>"##,
            yy + 9.0
        );
        if state.transition_hint.is_some_and(|t| t.press.contains(key)) {
            let _ = write!(
                svg,
                r##"<circle cx="{xx}" cy="{yy}" r="30" fill="none" stroke="#3ddc84" stroke-width="4"/>"##
            );
        }
        let description = placement(offset, finger);
        let finger_name = ["no finger", "index", "middle", "ring", "little"][finger as usize];
        let _ = write!(
            svg,
            r##"<text x="395" y="340" text-anchor="middle" font-size="28" font-weight="700" fill="#ffb000">{} STRING · {description} ({finger_name})</text>"##,
            STRINGS[string - 1]
        );
        let mut actions = Vec::new();
        if let Some((old_string, _, old_finger)) = previous {
            if old_string != string {
                actions.push(format!(
                    "CHANGE {} TO {} STRING",
                    STRINGS[old_string - 1],
                    STRINGS[string - 1]
                ));
            }
            if old_finger != 0 {
                actions.push(format!("LIFT {old_finger}"));
            }
        }
        if state
            .transition_hint
            .is_some_and(|t| t.changed_key_count() == 0)
        {
            actions.push("SAME AGAIN".into());
        } else if finger == 0 {
            actions.push("PLAY OPEN".into());
        } else {
            actions.push(format!("PLACE {description}"));
        }
        let _ = write!(
            svg,
            r##"<text x="395" y="377" text-anchor="middle" font-size="21" fill="#c8cfdb">{}</text>"##,
            actions.join(" · ")
        );
    }
    svg.push_str(r##"<text x="395" y="407" text-anchor="middle" font-size="17" fill="#8a93a3">Dots guide pitch placement, not frets · Bow direction not specified</text></g>"##);
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
    fn chart_positions_and_alternatives_match_sounding_pitch() {
        let table = FingeringTable::load_instrument(Instrument::Violin).unwrap();
        for (midi, primary) in &table.fingerings {
            for f in
                std::iter::once(primary).chain(table.alternatives.get(midi).into_iter().flatten())
            {
                let (string, offset, _) = position(&f.keys[0]).unwrap();
                assert_eq!(
                    Instrument::Violin.tuning()[string - 1] + offset,
                    midi.parse::<u8>().unwrap()
                );
            }
        }
        assert_eq!(table.lookup(55).unwrap().keys, ["s4_p0_n0"]);
        assert_eq!(table.lookup(83).unwrap().keys, ["s1_p7_n4"]);
        assert_eq!(table.lookup(82).unwrap().keys, ["s1_p6_n4"]);
        assert_eq!(table.lookup(68).unwrap().keys, ["s3_p6_n3"]);
        assert_eq!(table.lookup(76).unwrap().keys, ["s1_p0_n0"]);
        assert!(table.alternatives["76"]
            .iter()
            .any(|f| f.keys == ["s2_p7_n4"]));
        assert!(table.lookup(54).is_err() && table.lookup(84).is_err());
        for bad in [
            "s5_p0_n0",
            "s1_p8_n4",
            "s1_p6_n3",
            "s3_p6_n4",
            "s1_p0_n1",
            "s01_p0_n0",
        ] {
            assert!(position(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn current_view_shows_release_string_change_open_and_repeat() {
        let table = FingeringTable::load_instrument(Instrument::Violin).unwrap();
        for (a, b, expected) in [
            (68, 69, "CHANGE D TO A STRING"),
            (69, 70, "PLACE LOW 1"),
            (70, 71, "PLACE 1"),
            (71, 71, "SAME AGAIN"),
        ] {
            let change = KeyTransition::between(table.lookup(a).unwrap(), table.lookup(b).unwrap());
            let pressed = table.lookup(b).unwrap().pressed_key_ids();
            let state = DiagramState {
                upper_hand: UpperHand::Left,
                pressed_keys: &pressed,
                pressed_style: PressedStyle::Sounding,
                transition_hint: Some(&change),
            };
            let svg = render(&state, 0.0, 0.0, 1.0, "now-");
            assert!(svg.contains(expected), "{svg}");
            if b == 69 {
                assert!(svg.contains("LIFT 3") && svg.contains("PLAY OPEN"));
            }
            if a == 69 {
                assert!(!svg.contains("LIFT 0"));
            }
            assert!(svg.contains("id=\"now-"));
            assert!(!svg.contains("FRET "));
        }
    }
}

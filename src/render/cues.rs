//! Words for the NOW panel's "next change" cue.
//!
//! The diagram rings show *which* controls change; this text says *what to
//! do* in the player's terms: which keys to lift and press on winds, which
//! string/fret or key comes next elsewhere, and that a repeated note needs a
//! fresh attack even though nothing moves.

use crate::instruments::fingering::{FingeringTimelineEntry, KeyTransition};
use crate::instruments::{Instrument, LayoutKey};

/// Beyond this many keys a list stops being readable at a glance.
const MAX_LISTED_KEYS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeCue {
    /// Short headline, e.g. "NEXT CHANGE" or "REPEAT".
    pub title: &'static str,
    pub lines: Vec<String>,
    /// Repeated note: same fingering, new attack.
    pub is_repeat: bool,
}

pub fn change_cue(
    instrument: Instrument,
    layout_keys: &[LayoutKey],
    held: Option<&FingeringTimelineEntry>,
    upcoming: &FingeringTimelineEntry,
) -> ChangeCue {
    let transition = match held {
        Some(held) => KeyTransition::between(&held.fingering, &upcoming.fingering),
        None => KeyTransition {
            press: upcoming.fingering.pressed_key_ids(),
            lift: Default::default(),
        },
    };
    let is_repeat = held.is_some() && transition.changed_key_count() == 0;
    if is_repeat {
        return ChangeCue {
            title: "REPEAT",
            lines: vec![repeat_instruction(instrument).to_owned()],
            is_repeat: true,
        };
    }
    if !uses_press_lift(instrument) {
        return ChangeCue {
            title: "NEXT",
            lines: vec![describe_position(instrument, upcoming)],
            is_repeat: false,
        };
    }
    let label = |id: &String| key_label(layout_keys, id);
    let mut lines = Vec::new();
    if !transition.lift.is_empty() {
        lines.push(format!("LIFT {}", list(transition.lift.iter().map(label))));
    }
    if !transition.press.is_empty() {
        lines.push(format!(
            "PRESS {}",
            list(transition.press.iter().map(label))
        ));
    }
    ChangeCue {
        title: if held.is_some() {
            "NEXT CHANGE"
        } else {
            "FIRST NOTE"
        },
        lines,
        is_repeat: false,
    }
}

/// Wind controls are held keys, so the change is a set of lifts/presses.
fn uses_press_lift(instrument: Instrument) -> bool {
    !(instrument.is_fretted()
        || instrument.keyboard_range().is_some()
        || instrument == Instrument::Violin)
}

fn repeat_instruction(instrument: Instrument) -> &'static str {
    if instrument.keyboard_range().is_some() {
        "SAME KEY - RELEASE AND PLAY AGAIN"
    } else if instrument.is_fretted() || instrument == Instrument::Violin {
        "SAME NOTE - PLAY AGAIN"
    } else {
        "SAME KEYS - RE-TONGUE"
    }
}

fn list(labels: impl Iterator<Item = String>) -> String {
    let labels: Vec<String> = labels.collect();
    if labels.len() > MAX_LISTED_KEYS {
        format!("{} KEYS", labels.len())
    } else {
        labels.join(", ")
    }
}

/// Printed label from the profile layout; AE-01 draws its own controls, so
/// its IDs map to the numbers/symbols printed on the instrument.
fn key_label(layout_keys: &[LayoutKey], id: &str) -> String {
    if let Some(key) = layout_keys.iter().find(|key| key.id == id) {
        return key.label.clone();
    }
    let ae01 = match id {
        "left_1" => "1",
        "left_2" => "2",
        "left_3" => "3",
        "right_1" => "4",
        "right_2" => "5",
        "right_3" => "6",
        "right_4" => "7",
        "sharp" => "#",
        "flat" => "b",
        "octave_up" => "OCT UP",
        "octave_down" => "OCT DOWN",
        "octave_up2" => "OCT UP x2",
        "octave_down2" => "OCT DOWN x2",
        other => other,
    };
    ae01.to_owned()
}

const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn note_name(midi: u8) -> String {
    format!("{}{}", NOTE_NAMES[midi as usize % 12], midi as i32 / 12 - 1)
}

/// Next position for instruments without press/lift keys, from the
/// position IDs their profiles use (`sN_fM`, `sN_pP_nF`, `key_NN`).
fn describe_position(instrument: Instrument, upcoming: &FingeringTimelineEntry) -> String {
    let Some(id) = upcoming.fingering.keys.first() else {
        return note_name(upcoming.midi);
    };
    if instrument.keyboard_range().is_some() {
        return format!("KEY {}", note_name(upcoming.midi));
    }
    let parts: Vec<&str> = id.split('_').collect();
    let number = |prefix: char| {
        parts
            .iter()
            .find_map(|part| part.strip_prefix(prefix).and_then(|n| n.parse::<u8>().ok()))
    };
    match (number('s'), number('f'), number('n')) {
        (Some(string), Some(0), _) => format!("STRING {string} OPEN"),
        (Some(string), Some(fret), _) => format!("STRING {string} - FRET {fret}"),
        (Some(string), None, Some(0)) => format!("STRING {string} OPEN"),
        (Some(string), None, Some(finger)) => format!("STRING {string} - FINGER {finger}"),
        _ => note_name(upcoming.midi),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fingering::{Fingering, OctaveShift};

    fn entry(midi: u8, keys: &[&str]) -> FingeringTimelineEntry {
        FingeringTimelineEntry {
            start: 0.0,
            end: 1.0,
            midi,
            fingering: Fingering {
                octave: OctaveShift::Normal,
                keys: keys.iter().map(|k| k.to_string()).collect(),
            },
            transition_to_next: None,
        }
    }

    fn layout(pairs: &[(&str, &str)]) -> Vec<LayoutKey> {
        pairs
            .iter()
            .map(|(id, label)| LayoutKey {
                id: id.to_string(),
                label: label.to_string(),
                x: 0.0,
                y: 0.0,
                shape: "circle".into(),
            })
            .collect()
    }

    #[test]
    fn wind_change_names_keys_to_lift_and_press() {
        let keys = layout(&[("4", "4"), ("Eb", "Eb"), ("C", "C")]);
        let held = entry(63, &["1", "4", "Eb"]);
        let next = entry(62, &["1", "4", "C"]);
        let cue = change_cue(Instrument::Ae20, &keys, Some(&held), &next);
        assert_eq!(cue.title, "NEXT CHANGE");
        assert_eq!(cue.lines, vec!["LIFT Eb", "PRESS C"]);
        assert!(!cue.is_repeat);
    }

    #[test]
    fn repeated_note_asks_for_a_fresh_attack() {
        let held = entry(62, &["1", "2"]);
        let cue = change_cue(Instrument::Ae20, &[], Some(&held), &held.clone());
        assert!(cue.is_repeat);
        assert_eq!(cue.lines, vec!["SAME KEYS - RE-TONGUE"]);
        let piano = entry(60, &["key_60"]);
        let cue = change_cue(Instrument::Piano, &[], Some(&piano), &piano.clone());
        assert_eq!(cue.lines, vec!["SAME KEY - RELEASE AND PLAY AGAIN"]);
    }

    #[test]
    fn ae01_ids_use_printed_numbers_and_long_lists_are_counted() {
        let held = entry(
            60,
            &[
                "left_1", "left_2", "left_3", "right_1", "right_2", "right_3", "right_4",
            ],
        );
        let next = entry(72, &["left_2"]);
        let cue = change_cue(Instrument::Ae01, &[], Some(&held), &next);
        assert_eq!(cue.lines, vec!["LIFT 6 KEYS"]);
        let next = entry(
            71,
            &[
                "left_1", "left_2", "left_3", "right_1", "right_2", "right_3", "sharp",
            ],
        );
        let cue = change_cue(Instrument::Ae01, &[], Some(&held), &next);
        assert_eq!(cue.lines, vec!["LIFT 7", "PRESS #"]);
    }

    #[test]
    fn string_and_keyboard_positions_are_described() {
        let cue = change_cue(
            Instrument::Guitar,
            &[],
            Some(&entry(40, &["s6_f0"])),
            &entry(43, &["s6_f3"]),
        );
        assert_eq!(cue.lines, vec!["STRING 6 - FRET 3"]);
        let cue = change_cue(Instrument::Violin, &[], None, &entry(62, &["s3_p0_n0"]));
        assert_eq!(cue.lines, vec!["STRING 3 OPEN"]);
        let cue = change_cue(Instrument::Piano, &[], None, &entry(63, &["key_63"]));
        assert_eq!(cue.lines, vec!["KEY D#4"]);
    }
}

//! Instrument identifiers and data-driven profiles (instruments/*.json).
pub mod ae01;
pub mod ae05;
pub mod ae10;
pub mod ae20;
pub mod diagram;
pub mod fingering;
pub mod guitar;
mod sax;

use self::diagram::UpperHand;
#[cfg(test)]
use self::guitar::guitar_position;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Instrument {
    #[default]
    Ae01,
    Ae05,
    Ae10,
    Ae20,
    Guitar,
}

impl Instrument {
    pub fn id(self) -> &'static str {
        match self {
            Self::Ae01 => "ae01",
            Self::Ae05 => "ae05",
            Self::Ae10 => "ae10",
            Self::Ae20 => "ae20",
            Self::Guitar => "guitar",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Ae01 => "AE-01",
            Self::Ae05 => "AE-05",
            Self::Ae10 => "AE-10",
            Self::Ae20 => "AE-20",
            Self::Guitar => "Guitar",
        }
    }
    pub fn table_id(self) -> String {
        if self == Self::Guitar {
            "guitar".to_owned()
        } else {
            format!("roland-{}", self.id())
        }
    }
    pub fn profile_path(self) -> PathBuf {
        crate::paths::project_root()
            .join("instruments")
            .join(format!("{}.json", self.id()))
    }
    pub fn default_upper_hand(self) -> UpperHand {
        if self == Self::Ae01 {
            UpperHand::Right
        } else {
            UpperHand::Left
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LayoutKey {
    pub id: String,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub shape: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fingering::{FingeringTable, OctaveShift};
    use crate::music::notes::NoteEvent;
    use crate::music::range::{fit_melody_to_table, RangePolicy};

    #[test]
    fn every_profile_loads_with_contiguous_range_and_sources() {
        for (instrument, low, high) in [
            (Instrument::Ae01, 47, 85),
            (Instrument::Ae05, 46, 85),
            (Instrument::Ae10, 34, 97),
            (Instrument::Ae20, 34, 97),
            (Instrument::Guitar, 40, 83),
        ] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            assert_eq!(table.charted_range(), Some((low, high)));
            assert!(!table.verified);
            assert!(!table.sources.is_empty() && !table.required_settings.is_empty());
            for midi in low..=high {
                table.lookup(midi).unwrap();
            }
        }
    }

    #[test]
    fn sax_fingerings_use_their_own_controls_and_octave_modes() {
        for instrument in [Instrument::Ae05, Instrument::Ae10, Instrument::Ae20] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            assert_eq!(
                table.lookup(60).unwrap().keys,
                ["1", "2", "3", "4", "5", "6", "C"]
            );
            assert_eq!(table.lookup(66).unwrap().keys, ["1", "2", "3", "5"]);
            assert_eq!(table.lookup(70).unwrap().keys, ["1", "2", "Ta"]);
            assert_eq!(table.lookup(72).unwrap().keys, ["2"]);
            assert_eq!(table.lookup(60).unwrap().octave, OctaveShift::Normal);
            assert!(table.alternatives["72"]
                .iter()
                .any(|f| f.keys == ["1", "Tc"] && f.octave == OctaveShift::Normal));
            assert!(table
                .lookup(46)
                .unwrap()
                .pressed_key_ids()
                .contains("octave_down"));
            if instrument != Instrument::Ae05 {
                assert!(table
                    .lookup(34)
                    .unwrap()
                    .pressed_key_ids()
                    .contains("octave_down2"));
                assert!(table
                    .lookup(97)
                    .unwrap()
                    .pressed_key_ids()
                    .contains("octave_up2"));
            }
        }
    }

    #[test]
    fn guitar_positions_and_alternatives_have_exact_sounding_pitch() {
        let table = FingeringTable::load_instrument(Instrument::Guitar).unwrap();
        assert_eq!(table.lookup(40).unwrap().keys, ["s6_f0"]);
        assert_eq!(table.lookup(64).unwrap().keys, ["s1_f0"]);
        assert_eq!(table.lookup(83).unwrap().keys, ["s1_f19"]);
        assert!(table.alternatives["64"].iter().any(|f| f.keys == ["s2_f5"]));
        assert!(guitar_position("s7_f0").is_none());
    }

    #[test]
    fn range_fit_and_mapping_follow_selected_profile() {
        let notes: Vec<_> = [39, 63, 70]
            .iter()
            .enumerate()
            .map(|(i, &midi)| NoteEvent {
                start: i as f64,
                end: i as f64 + 0.5,
                midi,
                confidence: 1.0,
            })
            .collect();
        for (instrument, shift) in [
            (Instrument::Ae01, 1),
            (Instrument::Ae05, 1),
            (Instrument::Ae10, 0),
            (Instrument::Ae20, 0),
            (Instrument::Guitar, 1),
        ] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            let fitted = fit_melody_to_table(&notes, &table, RangePolicy::Strict).unwrap();
            assert_eq!(fitted.octave_shift, shift);
            assert!(!fitted.shape_changed());
            crate::instruments::fingering::map_notes_to_fingerings(&fitted.notes, &table).unwrap();
        }
    }
}
#[cfg(test)]
mod diagram_tests {
    use super::diagram::{DiagramState, PressedStyle};
    use super::*;
    use crate::instruments::fingering::{FingeringTable, KeyTransition};
    #[test]
    fn octave_two_and_current_transition_are_drawn() {
        let table = FingeringTable::load_instrument(Instrument::Ae20).unwrap();
        let pressed = table.lookup(97).unwrap().pressed_key_ids();
        let change = KeyTransition::between(table.lookup(73).unwrap(), table.lookup(97).unwrap());
        let state = DiagramState {
            upper_hand: crate::instruments::diagram::UpperHand::Left,
            pressed_keys: &pressed,
            pressed_style: PressedStyle::Sounding,
            transition_hint: Some(&change),
        };
        let svg = wind_svg(
            Instrument::Ae20,
            &table.layout_keys,
            &state,
            "translate(0 0)",
            "now-",
        );
        assert!(svg.contains("id=\"now-octave_up2\""));
        assert!(svg.contains("#3ddc84"));
        assert!(svg.contains(">+2</text>"));
    }
    #[test]
    fn guitar_open_and_high_fret_are_visible() {
        for key in ["s6_f0", "s1_f19"] {
            let pressed = [key.to_owned()].into();
            let state = DiagramState {
                upper_hand: crate::instruments::diagram::UpperHand::Left,
                pressed_keys: &pressed,
                pressed_style: PressedStyle::Sounding,
                transition_hint: None,
            };
            let svg = guitar::render(&state, 0.0, 0.0, 1.0, "now-");
            assert!(svg.contains(&format!("id=\"now-{key}\"")));
            assert!(svg.contains(if key.ends_with("f0") {
                "OPEN"
            } else {
                "FRET 19"
            }));
        }
    }
}

/// Dispatch wind drawing without making the video compositor own model details.
pub fn wind_svg(
    instrument: Instrument,
    keys: &[LayoutKey],
    state: &diagram::DiagramState,
    transform: &str,
    prefix: &str,
) -> String {
    match instrument {
        Instrument::Ae01 => ae01::instrument_group_svg(state, transform, prefix),
        Instrument::Ae05 => ae05::render(keys, state, transform, prefix),
        Instrument::Ae10 => ae10::render(keys, state, transform, prefix),
        Instrument::Ae20 => ae20::render(keys, state, transform, prefix),
        Instrument::Guitar => unreachable!("guitar uses its horizontal renderer"),
    }
}

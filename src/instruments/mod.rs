//! Instrument identifiers and data-driven profiles (instruments/*.json).
pub mod ae01;
pub mod ae05;
pub mod ae10;
pub mod ae20;
pub mod brisa;
pub mod diagram;
pub mod fingering;
pub mod flute;
pub mod guitar;
pub mod piano;
pub mod recorder;
mod sax;
pub mod violin;
pub mod yamaha;

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
    #[value(name = "yds120", alias = "yds-120")]
    Yds120,
    #[value(name = "yds150", alias = "yds-150")]
    Yds150,
    #[value(name = "ae-brisa")]
    #[serde(rename = "ae-brisa")]
    AeBrisa,
    #[value(
        alias = "guitar-6string",
        alias = "guitar-acoustic",
        alias = "guitar-classical",
        alias = "guitar-electric"
    )]
    Guitar,
    #[value(
        name = "guitar-bass",
        alias = "bass",
        alias = "guitar-4string",
        alias = "bass-4string"
    )]
    #[serde(rename = "guitar-bass")]
    Bass,
    #[value(name = "guitar-bass-5string", alias = "bass-5string")]
    #[serde(rename = "guitar-bass-5string")]
    Bass5,
    #[value(name = "piano", alias = "piano-88")]
    #[serde(rename = "piano")]
    Piano,
    #[value(name = "keyboard-76", alias = "piano-76")]
    #[serde(rename = "keyboard-76")]
    Keyboard76,
    #[value(name = "keyboard-61", alias = "piano-61")]
    #[serde(rename = "keyboard-61")]
    Keyboard61,
    #[value(name = "ukulele", alias = "ukulele-high-g")]
    Ukulele,
    #[value(name = "ukulele-low-g")]
    #[serde(rename = "ukulele-low-g")]
    UkuleleLowG,
    #[value(name = "ukulele-baritone")]
    #[serde(rename = "ukulele-baritone")]
    UkuleleBaritone,
    #[value(name = "recorder-baroque", alias = "recorder-soprano-baroque")]
    #[serde(rename = "recorder-baroque")]
    RecorderBaroque,
    #[value(name = "recorder-german", alias = "recorder-soprano-german")]
    #[serde(rename = "recorder-german")]
    RecorderGerman,
    #[value(name = "flute", alias = "flute-cfoot")]
    Flute,
    #[value(name = "flute-bfoot")]
    #[serde(rename = "flute-bfoot")]
    FluteBFoot,
    Violin,
}

impl Instrument {
    pub fn id(self) -> &'static str {
        match self {
            Self::Ae01 => "ae01",
            Self::Ae05 => "ae05",
            Self::Ae10 => "ae10",
            Self::Ae20 => "ae20",
            Self::Yds120 => "yds120",
            Self::Yds150 => "yds150",
            Self::AeBrisa => "ae-brisa",
            Self::Guitar => "guitar",
            Self::Bass => "guitar-bass",
            Self::Bass5 => "guitar-bass-5string",
            Self::Piano => "piano",
            Self::Keyboard76 => "keyboard-76",
            Self::Keyboard61 => "keyboard-61",
            Self::Ukulele => "ukulele",
            Self::UkuleleLowG => "ukulele-low-g",
            Self::UkuleleBaritone => "ukulele-baritone",
            Self::RecorderBaroque => "recorder-baroque",
            Self::RecorderGerman => "recorder-german",
            Self::Flute => "flute",
            Self::FluteBFoot => "flute-bfoot",
            Self::Violin => "violin",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Ae01 => "AE-01",
            Self::Ae05 => "AE-05",
            Self::Ae10 => "AE-10",
            Self::Ae20 => "AE-20",
            Self::Yds120 => "Yamaha YDS-120",
            Self::Yds150 => "Yamaha YDS-150",
            Self::AeBrisa => "AE-BRISA",
            Self::Guitar => "Guitar",
            Self::Bass => "Bass (4 strings)",
            Self::Bass5 => "Bass (5 strings)",
            Self::Piano => "Piano (88 keys)",
            Self::Keyboard76 => "Keyboard (76 keys)",
            Self::Keyboard61 => "Keyboard (61 keys)",
            Self::Ukulele => "Ukulele (high G)",
            Self::UkuleleLowG => "Ukulele (low G)",
            Self::UkuleleBaritone => "Baritone ukulele",
            Self::RecorderBaroque => "Soprano recorder (Baroque)",
            Self::RecorderGerman => "Soprano recorder (German)",
            Self::Flute => "Concert flute (C foot)",
            Self::FluteBFoot => "Concert flute (B foot)",
            Self::Violin => "Violin (first position)",
        }
    }
    pub fn table_id(self) -> String {
        if self.is_fretted()
            || self.keyboard_range().is_some()
            || self.is_recorder()
            || self.is_flute()
            || self == Self::Violin
        {
            self.id().to_owned()
        } else if matches!(self, Self::Yds120 | Self::Yds150) {
            format!("yamaha-{}", self.id())
        } else {
            format!("roland-{}", self.id())
        }
    }
    pub fn profile_path(self) -> PathBuf {
        crate::paths::project_root()
            .join("instruments")
            .join(format!("{}.json", self.id()))
    }
    pub fn is_fretted(self) -> bool {
        matches!(
            self,
            Self::Guitar
                | Self::Bass
                | Self::Bass5
                | Self::Ukulele
                | Self::UkuleleLowG
                | Self::UkuleleBaritone
        )
    }
    pub fn is_recorder(self) -> bool {
        matches!(self, Self::RecorderBaroque | Self::RecorderGerman)
    }
    pub fn is_flute(self) -> bool {
        matches!(self, Self::Flute | Self::FluteBFoot)
    }
    pub fn is_horizontal(self) -> bool {
        self.is_fretted()
            || self.keyboard_range().is_some()
            || self.is_flute()
            || self == Self::Violin
            || self == Self::AeBrisa
    }
    pub fn fret_count(self) -> u8 {
        match self {
            Self::Ukulele | Self::UkuleleLowG | Self::UkuleleBaritone => 12,
            Self::Guitar | Self::Bass | Self::Bass5 => 19,
            _ => 0,
        }
    }
    /// Sounding MIDI range, with middle C = 60 (C4).
    pub fn keyboard_range(self) -> Option<(u8, u8)> {
        match self {
            Self::Piano => Some((21, 108)),
            Self::Keyboard76 => Some((28, 103)),
            Self::Keyboard61 => Some((36, 96)),
            _ => None,
        }
    }
    /// Open-string sounding MIDI, string 1 first (high-G ukulele is reentrant).
    pub fn tuning(self) -> &'static [u8] {
        match self {
            Self::Violin => &[76, 69, 62, 55],
            Self::Guitar => &[64, 59, 55, 50, 45, 40],
            Self::Bass => &[43, 38, 33, 28],
            Self::Bass5 => &[43, 38, 33, 28, 23],
            Self::Ukulele => &[69, 64, 60, 67],
            Self::UkuleleLowG => &[69, 64, 60, 55],
            Self::UkuleleBaritone => &[64, 59, 55, 50],
            _ => &[],
        }
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
            (Instrument::Yds120, 57, 90),
            (Instrument::Yds150, 57, 90),
            (Instrument::Guitar, 40, 83),
            (Instrument::Bass, 28, 62),
            (Instrument::Bass5, 23, 62),
            (Instrument::Piano, 21, 108),
            (Instrument::Keyboard76, 28, 103),
            (Instrument::Keyboard61, 36, 96),
            (Instrument::Ukulele, 60, 81),
            (Instrument::UkuleleLowG, 55, 81),
            (Instrument::UkuleleBaritone, 50, 76),
            (Instrument::RecorderBaroque, 72, 98),
            (Instrument::RecorderGerman, 72, 98),
            (Instrument::Flute, 60, 96),
            (Instrument::FluteBFoot, 59, 96),
            (Instrument::Violin, 55, 83),
        ] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            assert_eq!(table.charted_range(), Some((low, high)));
            assert!(!table.verified);
            assert!(!table.sources.is_empty() && !table.required_settings.is_empty());
            if instrument.is_fretted() {
                assert_eq!(table.tuning_midi, instrument.tuning());
            }
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
    fn ukulele_positions_respect_reentrant_tuning_and_twelve_frets() {
        for instrument in [
            Instrument::Ukulele,
            Instrument::UkuleleLowG,
            Instrument::UkuleleBaritone,
        ] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            assert_eq!(table.fret_count, 12);
            for (midi, primary) in &table.fingerings {
                for f in std::iter::once(primary)
                    .chain(table.alternatives.get(midi).into_iter().flatten())
                {
                    let (string, fret) = guitar_position(&f.keys[0]).unwrap();
                    assert!(string <= 4 && fret <= 12);
                    assert_eq!(
                        instrument.tuning()[string - 1] + fret,
                        midi.parse::<u8>().unwrap()
                    );
                }
            }
            let pressed = ["s1_f12".to_owned()].into();
            let state = diagram::DiagramState {
                upper_hand: UpperHand::Left,
                pressed_keys: &pressed,
                pressed_style: diagram::PressedStyle::Sounding,
                transition_hint: None,
            };
            let svg = guitar::render(instrument, &state, 0.0, 0.0, 1.0, "now-");
            assert!(svg.contains("FRETS 8–12"));
            assert!(svg.contains("STRING 1 AT TOP"));
        }
        let high = FingeringTable::load_instrument(Instrument::Ukulele).unwrap();
        assert_eq!(high.lookup(67).unwrap().keys, ["s4_f0"]);
        assert!(high.lookup(55).is_err());
        let low = FingeringTable::load_instrument(Instrument::UkuleleLowG).unwrap();
        assert_eq!(low.lookup(55).unwrap().keys, ["s4_f0"]);
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
            let svg = guitar::render(Instrument::Guitar, &state, 0.0, 0.0, 1.0, "now-");
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
        Instrument::RecorderBaroque | Instrument::RecorderGerman => {
            recorder::render(keys, state, transform, prefix)
        }
        Instrument::Yds120 | Instrument::Yds150 => yamaha::render(keys, state, transform, prefix),
        _ => unreachable!("keyboards and fretted instruments use horizontal rendering"),
    }
}

/// Shared placement for horizontal keyboards and fretboards.
pub fn horizontal_svg(
    instrument: Instrument,
    keys: &[LayoutKey],
    state: &diagram::DiagramState,
    x: f32,
    y: f32,
    scale: f32,
    prefix: &str,
) -> String {
    if instrument == Instrument::AeBrisa {
        brisa::render(keys, state, x, y, scale, prefix)
    } else if instrument == Instrument::Violin {
        violin::render(state, x, y, scale, prefix)
    } else if instrument.is_flute() {
        flute::render(keys, state, x, y, scale, prefix)
    } else if let Some(range) = instrument.keyboard_range() {
        piano::render(range, state, x, y, scale, prefix)
    } else {
        guitar::render(instrument, state, x, y, scale, prefix)
    }
}

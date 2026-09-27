//! Instrument identifiers and data-driven profiles (instruments/*.json).
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::ae01_diagram::UpperHand;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Instrument { #[default] Ae01, Ae05, Ae10, Ae20, Guitar }

impl Instrument {
    pub fn id(self) -> &'static str {
        match self { Self::Ae01=>"ae01", Self::Ae05=>"ae05", Self::Ae10=>"ae10", Self::Ae20=>"ae20", Self::Guitar=>"guitar" }
    }
    pub fn name(self) -> &'static str {
        match self { Self::Ae01=>"AE-01", Self::Ae05=>"AE-05", Self::Ae10=>"AE-10", Self::Ae20=>"AE-20", Self::Guitar=>"Guitar" }
    }
    pub fn table_id(self) -> String {
        if self == Self::Guitar { "guitar".to_owned() } else { format!("roland-{}", self.id()) }
    }
    pub fn profile_path(self) -> PathBuf {
        crate::paths::project_root().join("instruments").join(format!("{}.json", self.id()))
    }
    pub fn default_upper_hand(self) -> UpperHand {
        if self == Self::Ae01 { UpperHand::Right } else { UpperHand::Left }
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

/// Guitar string 1 is high E; fret zero means pluck the open string.
pub fn guitar_position(key: &str) -> Option<(usize, u8)> {
    let (string, fret) = key.strip_prefix('s')?.split_once("_f")?;
    let string: usize = string.parse().ok()?;
    let fret: u8 = fret.parse().ok()?;
    (1..=6).contains(&string).then_some((string, fret))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingering::{FingeringTable, OctaveShift};
    use crate::notes::NoteEvent;
    use crate::melody_range::{fit_melody_to_table, RangePolicy};

    #[test]
    fn every_profile_loads_with_contiguous_range_and_sources() {
        for (instrument, low, high) in [(Instrument::Ae01,47,85),(Instrument::Ae05,46,85),(Instrument::Ae10,34,97),(Instrument::Ae20,34,97),(Instrument::Guitar,40,83)] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            assert_eq!(table.charted_range(), Some((low,high)));
            assert!(!table.verified);
            assert!(!table.sources.is_empty() && !table.required_settings.is_empty());
            for midi in low..=high { table.lookup(midi).unwrap(); }
        }
    }

    #[test]
    fn sax_fingerings_use_their_own_controls_and_octave_modes() {
        for instrument in [Instrument::Ae05, Instrument::Ae10, Instrument::Ae20] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            assert_eq!(table.lookup(60).unwrap().keys, ["1","2","3","4","5","6","C"]);
            assert_eq!(table.lookup(66).unwrap().keys, ["1","2","3","5"]);
            assert_eq!(table.lookup(70).unwrap().keys, ["1","2","Ta"]);
            assert_eq!(table.lookup(72).unwrap().keys, ["2"]);
            assert_eq!(table.lookup(60).unwrap().octave, OctaveShift::Normal);
            assert!(table.alternatives["72"].iter().any(|f| f.keys == ["1","Tc"] && f.octave == OctaveShift::Normal));
            assert!(table.lookup(46).unwrap().pressed_key_ids().contains("octave_down"));
            if instrument != Instrument::Ae05 {
                assert!(table.lookup(34).unwrap().pressed_key_ids().contains("octave_down2"));
                assert!(table.lookup(97).unwrap().pressed_key_ids().contains("octave_up2"));
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
        let notes:Vec<_> = [39,63,70].iter().enumerate().map(|(i,&midi)|NoteEvent{start:i as f64,end:i as f64+0.5,midi,confidence:1.0}).collect();
        for (instrument, shift) in [(Instrument::Ae01,1),(Instrument::Ae05,1),(Instrument::Ae10,0),(Instrument::Ae20,0),(Instrument::Guitar,1)] {
            let table=FingeringTable::load_instrument(instrument).unwrap();
            let fitted=fit_melody_to_table(&notes,&table,RangePolicy::Strict).unwrap();
            assert_eq!(fitted.octave_shift,shift);
            assert!(!fitted.shape_changed());
            crate::fingering::map_notes_to_fingerings(&fitted.notes,&table).unwrap();
        }
    }
}

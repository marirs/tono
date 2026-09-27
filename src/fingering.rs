//! Deterministic AE-01 fingering lookup.
//!
//! Fingerings are sourced data loaded from `instruments/*.json`, never
//! inferred. A MIDI note without an entry is an error: rule 9 of the spec
//! forbids fabricating missing fingerings, so we fail loudly instead of
//! guessing a "nearest" one.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::ae01_diagram::is_known_key_id;
use crate::notes::NoteEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OctaveShift {
    Down,
    Down2,
    Normal,
    Up,
    Up2,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fingering {
    pub octave: OctaveShift,
    pub keys: Vec<String>,
}

impl Fingering {
    /// Every SVG key region that must be shown as pressed, including the
    /// octave button implied by `octave`.
    pub fn pressed_key_ids(&self) -> BTreeSet<String> {
        let mut pressed: BTreeSet<String> = self.keys.iter().cloned().collect();
        match self.octave {
            OctaveShift::Up => {
                pressed.insert("octave_up".to_owned());
            }
            OctaveShift::Down => {
                pressed.insert("octave_down".to_owned());
            }
            OctaveShift::Up2 => { pressed.insert("octave_up2".to_owned()); }
            OctaveShift::Down2 => { pressed.insert("octave_down2".to_owned()); }
            OctaveShift::Normal => {}
        }
        pressed
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FingeringTable {
    pub instrument: String,
    #[serde(default)]
    pub layout_keys: Vec<crate::instruments::LayoutKey>,
    #[serde(default)]
    pub required_settings: String,
    #[serde(default)]
    pub sources: Vec<serde_json::Value>,
    #[serde(default)]
    pub tuning_midi: Vec<u8>,
    #[serde(default)]
    pub fret_count: u8,
    pub version: u32,
    /// `false` until the table has been checked against Roland documentation
    /// and the physical instrument. Rendered videos display a warning banner
    /// while this is false.
    #[serde(default)]
    pub verified: bool,
    /// Keys are MIDI numbers as strings, matching the spec's JSON schema.
    pub fingerings: BTreeMap<String, Fingering>,
    #[serde(default)]
    pub alternatives: BTreeMap<String, Vec<Fingering>>,
}

impl FingeringTable {
    pub fn load_instrument(instrument: crate::instruments::Instrument) -> Result<Self> {
        let table = Self::load_from_file(&instrument.profile_path())?;
        if table.instrument != instrument.table_id() { bail!("profile identity does not match {}", instrument.id()); }
        Ok(table)
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let raw_json = std::fs::read_to_string(path)
            .with_context(|| format!("reading fingering table {}", path.display()))?;
        let table: FingeringTable = serde_json::from_str(&raw_json)
            .with_context(|| format!("parsing fingering table {}", path.display()))?;
        table.validate().with_context(|| format!("validating {}", path.display()))?;
        Ok(table)
    }

    fn validate(&self) -> Result<()> {
        if !["roland-ae01", "roland-ae05", "roland-ae10", "roland-ae20", "guitar"].contains(&self.instrument.as_str()) {
            bail!("unsupported instrument profile `{}`", self.instrument);
        }
        if self.version != 1 {
            bail!("unsupported fingering table version {} (expected 1)", self.version);
        }
        if self.fingerings.is_empty() { bail!("empty fingering table"); }
        let mut ids = BTreeSet::new();
        for key in &self.layout_keys {
            if !ids.insert(&key.id) || !key.x.is_finite() || !key.y.is_finite()
                || !["circle", "rect", "octave"].contains(&key.shape.as_str()) {
                bail!("invalid or duplicate layout key {}", key.id);
            }
        }
        if self.instrument == "guitar" && (self.tuning_midi != [64,59,55,50,45,40] || self.fret_count != 19) {
            bail!("guitar profile requires standard six-string tuning and 19 frets");
        }
        for (midi_key, fingering) in &self.fingerings {
            let midi: u8 = midi_key.parse().with_context(|| format!("fingering key `{midi_key}` is not a MIDI number"))?;
            if midi > 127 || midi.to_string() != *midi_key { bail!("invalid MIDI number {midi_key}"); }
            self.validate_fingering(midi, fingering)?;
        }
        for (midi, alternatives) in &self.alternatives {
            if !self.fingerings.contains_key(midi) { bail!("alternatives without primary fingering: {midi}"); }
            for fingering in alternatives { self.validate_fingering(midi.parse()?, fingering)?; }
        }
        Ok(())
    }

    fn validate_fingering(&self, midi: u8, fingering: &Fingering) -> Result<()> {
        if self.instrument == "guitar" {
            if fingering.octave != OctaveShift::Normal || fingering.keys.len() != 1 { bail!("MIDI {midi}: expected one guitar position"); }
            let (string, fret) = crate::instruments::guitar_position(&fingering.keys[0]).context("invalid string/fret")?;
            if fret > self.fret_count || self.tuning_midi[string-1] as u16 + fret as u16 != midi as u16 { bail!("MIDI {midi}: guitar pitch does not match string/fret"); }
        } else {
            if fingering.keys.iter().collect::<BTreeSet<_>>().len() != fingering.keys.len() { bail!("MIDI {midi}: duplicate key"); }
            for key in fingering.pressed_key_ids() {
                let known = if self.instrument == "roland-ae01" { is_known_key_id(&key) }
                    else { self.layout_keys.iter().any(|region| region.id == key) };
                if !known { bail!("MIDI {midi}: unknown {} key {key}", self.instrument); }
            }
        }
        Ok(())
    }

    /// Lowest and highest MIDI numbers with a charted fingering.
    pub fn charted_range(&self) -> Option<(u8, u8)> {
        let charted = self.fingerings.keys().filter_map(|key| key.parse::<u8>().ok());
        Some((charted.clone().min()?, charted.max()?))
    }

    pub fn lookup(&self, midi: u8) -> Result<&Fingering> {
        self.fingerings.get(&midi.to_string()).with_context(|| {
            format!(
                "no {} fingering for MIDI {midi}; refusing to guess (spec rule 9)",
                self.instrument
            )
        })
    }
}

/// Which keys change between two consecutive fingerings. Drives the
/// optional LIFT / PRESS transition hints on the NEXT diagram.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KeyTransition {
    pub press: BTreeSet<String>,
    pub lift: BTreeSet<String>,
}

impl KeyTransition {
    pub fn between(current: &Fingering, next: &Fingering) -> Self {
        let current_keys = current.pressed_key_ids();
        let next_keys = next.pressed_key_ids();
        KeyTransition {
            press: next_keys.difference(&current_keys).cloned().collect(),
            lift: current_keys.difference(&next_keys).cloned().collect(),
        }
    }

    pub fn changed_key_count(&self) -> usize {
        self.press.len() + self.lift.len()
    }
}

/// One entry of the output `fingering.json` timeline.
#[derive(Debug, Clone, Serialize)]
pub struct FingeringTimelineEntry {
    pub start: f64,
    pub end: f64,
    pub midi: u8,
    pub fingering: Fingering,
    /// Change needed to reach the following note; `None` for the last note.
    pub transition_to_next: Option<KeyTransition>,
}

pub fn map_notes_to_fingerings(
    notes: &[NoteEvent],
    table: &FingeringTable,
) -> Result<Vec<FingeringTimelineEntry>> {
    let fingerings: Vec<&Fingering> = notes
        .iter()
        .map(|note| table.lookup(note.midi))
        .collect::<Result<_>>()?;

    Ok(notes
        .iter()
        .enumerate()
        .map(|(index, note)| FingeringTimelineEntry {
            start: note.start,
            end: note.end,
            midi: note.midi,
            fingering: fingerings[index].clone(),
            transition_to_next: fingerings
                .get(index + 1)
                .map(|next| KeyTransition::between(fingerings[index], next)),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_from_json(json: &str) -> Result<FingeringTable> {
        let table: FingeringTable = serde_json::from_str(json)?;
        table.validate()?;
        Ok(table)
    }

    const SMALL_TABLE: &str = r#"{
        "instrument": "roland-ae01", "version": 1,
        "fingerings": {
            "60": {"octave": "normal", "keys": ["left_1", "left_2", "left_3"]},
            "62": {"octave": "normal", "keys": ["left_1", "left_2"]},
            "72": {"octave": "up", "keys": ["left_1", "left_2", "left_3"]}
        }
    }"#;

    #[test]
    fn lookup_known_and_missing_notes() {
        let table = table_from_json(SMALL_TABLE).unwrap();
        assert_eq!(table.lookup(62).unwrap().keys, vec!["left_1", "left_2"]);
        let error = table.lookup(61).unwrap_err().to_string();
        assert!(error.contains("refusing to guess"), "{error}");
        assert!(!table.verified, "verified must default to false");
    }

    #[test]
    fn rejects_unknown_key_ids_and_wrong_instrument() {
        let unknown_key = SMALL_TABLE.replace("\"left_3\"]}", "\"left_9\"]}");
        assert!(table_from_json(&unknown_key).is_err());
        let wrong_instrument = SMALL_TABLE.replace("roland-ae01", "recorder");
        assert!(table_from_json(&wrong_instrument).is_err());
    }

    #[test]
    fn octave_shift_adds_octave_key() {
        let table = table_from_json(SMALL_TABLE).unwrap();
        assert!(table.lookup(72).unwrap().pressed_key_ids().contains("octave_up"));
        assert!(!table.lookup(60).unwrap().pressed_key_ids().contains("octave_up"));
    }

    #[test]
    fn transition_lists_pressed_and_lifted_keys() {
        let table = table_from_json(SMALL_TABLE).unwrap();
        let transition = KeyTransition::between(table.lookup(62).unwrap(), table.lookup(72).unwrap());
        let press: Vec<&str> = transition.press.iter().map(String::as_str).collect();
        assert_eq!(press, vec!["left_3", "octave_up"]);
        assert!(transition.lift.is_empty());
        assert_eq!(transition.changed_key_count(), 2);
    }

    #[test]
    fn mapping_fails_on_unmapped_note() {
        let table = table_from_json(SMALL_TABLE).unwrap();
        let notes = [
            NoteEvent { start: 0.0, end: 0.5, midi: 60, confidence: 1.0 },
            NoteEvent { start: 0.5, end: 1.0, midi: 61, confidence: 1.0 },
        ];
        assert!(map_notes_to_fingerings(&notes, &table).is_err());
        let mapped = map_notes_to_fingerings(&notes[..1], &table).unwrap();
        assert!(mapped[0].transition_to_next.is_none());
    }

    fn bundled_table() -> FingeringTable {
        FingeringTable::load_from_file(&crate::paths::ae01_fingering_table()).unwrap()
    }

    #[test]
    fn bundled_table_covers_documented_range_and_demo() {
        let table = bundled_table();
        // Roland's Recorder chart documents B3 (59) to C#5 (73), no gaps.
        for midi in 59..=73 {
            let fingering = table.lookup(midi).unwrap();
            assert_eq!(fingering.octave, OctaveShift::Normal, "MIDI {midi}");
        }
        for midi in 47..=85 { table.lookup(midi).unwrap(); }
        assert!(table.lookup(46).is_err() && table.lookup(86).is_err());
        assert_eq!(table.lookup(48).unwrap().octave, OctaveShift::Down);
        assert_eq!(table.lookup(84).unwrap().keys, vec!["left_2"]);
        assert_eq!(table.lookup(85).unwrap().keys.len(), 0);
        assert!(table.lookup(85).unwrap().pressed_key_ids().contains("octave_up"));
        assert!(table.alternatives["60"].iter().any(|f| f.octave == OctaveShift::Down && f.keys == vec!["left_2"]));
        for midi in crate::notes::demo_melody(100.0).iter().map(|n| n.midi) {
            table.lookup(midi).unwrap();
        }
    }

    #[test]
    fn bundled_table_matches_roland_chart_spot_checks() {
        // Re-read from the Roland chart PDF (Recorder section) to guard the
        // transcription against accidental edits.
        let table = bundled_table();
        let keys = |midi: u8| table.lookup(midi).unwrap().keys.join(" ");
        assert_eq!(keys(59), "left_1 left_2 left_3 flat right_1 right_2 right_3 right_4");
        assert_eq!(keys(60), "left_1 left_2 left_3 right_1 right_2 right_3 right_4");
        assert_eq!(keys(65), "left_1 left_2 left_3 right_1", "F4 is not forked on the AE-01");
        assert_eq!(keys(72), "left_2");
        assert_eq!(keys(73), "");
    }
}

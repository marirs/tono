//! Pitched pan setup and strike diagrams. Never infer notes from a scale name.
use super::{
    diagram::{DiagramState, PressedStyle, FONT_FAMILY},
    Instrument, LayoutKey,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanStyle {
    Major,
    Minor,
    Celtic,
    Arabic,
    Relax,
    Indian,
    Meditation,
    Japanese,
    Equinox,
    Romantic,
    Dreamy,
    Aegean,
}
impl PanStyle {
    pub fn id(self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Celtic => "celtic",
            Self::Arabic => "arabic",
            Self::Relax => "relax",
            Self::Indian => "indian",
            Self::Meditation => "meditation",
            Self::Japanese => "japanese",
            Self::Equinox => "equinox",
            Self::Romantic => "romantic",
            Self::Dreamy => "dreamy",
            Self::Aegean => "aegean",
        }
    }
}
pub fn validate_style(instrument: Instrument, style: Option<PanStyle>) -> Result<()> {
    instrument.validate_melody_support()?;
    match (instrument, style) {
        (Instrument::Moodpan, None) => bail!("--instrument moodpan requires --pan-style (major, minor, celtic, arabic, relax, indian, meditation, japanese, equinox, romantic, dreamy, aegean). Match the Style knob; use Handpan tone, factory tuning and no pitch-shifting effects."),
        (Instrument::Moodpan, Some(_)) | (_, None) => Ok(()),
        _ => bail!("--pan-style is only supported with --instrument moodpan"),
    }
}

/// Top view, player at bottom. Labels identify tone fields, not fingers.
pub fn render(
    keys: &[LayoutKey],
    state: &DiagramState,
    x: f32,
    y: f32,
    scale: f32,
    prefix: &str,
) -> String {
    let mut svg = format!(
        r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><g transform="translate(54.6 0) scale(0.86)"><ellipse cx="390" cy="190" rx="230" ry="195" fill="#25313d" stroke="#758091" stroke-width="4"/>"##
    );
    for key in keys {
        let active = state.pressed_keys.contains(&key.id);
        let fill = if active {
            if state.pressed_style == PressedStyle::Ready {
                "#6b4f0a"
            } else {
                "#ffb000"
            }
        } else {
            "#131e2b"
        };
        let ink = if active { "#ffffff" } else { "#aab6c7" };
        let radius = if key.id == "pad_1" { 49 } else { 34 };
        let _ = write!(
            svg,
            r##"<circle id="{prefix}{}" cx="{}" cy="{}" r="{radius}" fill="{fill}" stroke="#8a93a3" stroke-width="3"/><text x="{}" y="{}" fill="{ink}" text-anchor="middle" font-size="20">{}</text>"##,
            key.id,
            key.x,
            key.y,
            key.x,
            key.y + 7.0,
            key.label
        );
    }
    svg.push_str("</g>");
    let cue = if state.pressed_style == PressedStyle::Ready {
        "PREPARE HIGHLIGHTED PAD"
    } else {
        "STRIKE ONCE · LET RING"
    };
    let _ = write!(
        svg,
        r##"<text x="390" y="365" text-anchor="middle" fill="#ffb000" font-size="23">{cue}</text><text x="390" y="392" text-anchor="middle" fill="#8a93a3" font-size="18">PLAYER AT BOTTOM · REPEATED NOTE = STRIKE AGAIN</text></g>"##
    );
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        instruments::{
            diagram::UpperHand,
            fingering::{map_notes_to_fingerings, FingeringTable},
        },
        music::{
            notes::NoteEvent,
            range::{fit_melody_to_table, RangePolicy},
            timeline::PracticeTimeline,
        },
        render::RenderSettings,
    };
    use clap::ValueEnum;
    fn notes(pitches: &[u8]) -> Vec<NoteEvent> {
        pitches
            .iter()
            .enumerate()
            .map(|(i, &midi)| NoteEvent {
                start: i as f64,
                end: i as f64 + 0.8,
                midi,
                confidence: 1.0,
            })
            .collect()
    }
    #[test]
    fn setup_and_unsupported_instruments_fail_without_loading_profiles() {
        assert!(FingeringTable::load_instrument(Instrument::Moodpan)
            .unwrap_err()
            .to_string()
            .contains("--pan-style"));
        assert!(validate_style(Instrument::Handpan, Some(PanStyle::Minor)).is_err());
        for i in [Instrument::Taiko, Instrument::Octapad] {
            let error = FingeringTable::load_instrument(i).unwrap_err().to_string();
            assert!(error.contains("does not support lead-melody"));
            assert!(error.contains("not implemented"));
        }
    }
    #[test]
    fn every_factory_style_maps_exactly_nine_pitched_pads() {
        for style in PanStyle::value_variants() {
            let table =
                FingeringTable::load_for_setup(Instrument::Moodpan, None, Some(*style)).unwrap();
            assert_eq!(table.fingerings.len(), 9);
            assert!(!table.verified);
            for (index, &midi) in table.tuning_midi.iter().enumerate() {
                assert_eq!(
                    table.lookup(midi).unwrap().keys,
                    [format!("pad_{}", index + 1)]
                );
            }
        }
        let minor =
            FingeringTable::load_for_setup(Instrument::Moodpan, None, Some(PanStyle::Minor))
                .unwrap();
        assert_eq!(minor.tuning_midi, [50, 57, 58, 60, 62, 64, 65, 67, 69]);
        let major =
            FingeringTable::load_for_setup(Instrument::Moodpan, None, Some(PanStyle::Major))
                .unwrap();
        assert_eq!(major.tuning_midi, [50, 55, 57, 59, 61, 62, 64, 66, 69]);
    }
    #[test]
    fn sparse_fit_preserves_every_interval_and_refuses_missing_pitches() {
        let table = FingeringTable::load_instrument(Instrument::Handpan).unwrap();
        let original = notes(&[59, 61, 63, 64, 66, 68]);
        let fitted = fit_melody_to_table(&original, &table, RangePolicy::Strict).unwrap();
        assert_eq!(fitted.transpose_semitones(), 1);
        for (a, b) in original.iter().zip(&fitted.notes) {
            assert_eq!(b.midi, a.midi + 1);
            assert_eq!((a.start, a.end), (b.start, b.end));
        }
        assert!(fit_melody_to_table(
            &notes(&(50..=61).collect::<Vec<_>>()),
            &table,
            RangePolicy::Strict
        )
        .is_err());
        assert!(fit_melody_to_table(&notes(&[50, 69]), &table, RangePolicy::Fold).is_err());
    }
    #[test]
    fn pad_diagrams_and_sheets_teach_strikes_not_held_keys() {
        let table =
            FingeringTable::load_for_setup(Instrument::Moodpan, None, Some(PanStyle::Minor))
                .unwrap();
        let entries = map_notes_to_fingerings(&notes(&[50, 50, 57]), &table).unwrap();
        let cue = crate::render::cues::change_cue(
            Instrument::Moodpan,
            &table.layout_keys,
            Some(&entries[0]),
            &entries[1],
        );
        assert_eq!(cue.lines, ["SAME PAD - STRIKE AGAIN"]);
        let pressed = table.lookup(50).unwrap().pressed_key_ids();
        let state = DiagramState {
            upper_hand: UpperHand::Left,
            pressed_keys: &pressed,
            pressed_style: PressedStyle::Sounding,
            transition_hint: None,
        };
        let svg = format!(
            "<svg>{}</svg>",
            render(&table.layout_keys, &state, 0.0, 0.0, 1.0, "now-")
        );
        roxmltree::Document::parse(&svg).unwrap();
        assert!(svg.contains("now-pad_1"));
        assert!(!svg.contains("LIFT"));
        let settings = RenderSettings {
            instrument: Instrument::Moodpan,
            layout_keys: table.layout_keys,
            ..RenderSettings::default()
        };
        let timeline = PracticeTimeline::new(entries, 100.0, 1.0);
        let dir = tempfile::tempdir().unwrap();
        crate::pipeline::sheet::write_sheet(dir.path(), &timeline, &settings, "practice.mp4")
            .unwrap();
        let html = std::fs::read_to_string(dir.path().join("practice.html")).unwrap();
        assert!(html.contains("Strike the highlighted pad"));
        assert!(!html.contains(" · hold "));
        assert_eq!(html.matches("<article>").count(), 3);
    }
    #[test]
    fn accordion_uses_the_documented_41_key_range() {
        let table = FingeringTable::load_instrument(Instrument::Accordion).unwrap();
        assert_eq!(table.charted_range(), Some((53, 93)));
        assert_eq!(table.fingerings.len(), 41);
        for midi in 53..=93 {
            assert_eq!(table.lookup(midi).unwrap().keys, [format!("key_{midi}")]);
        }
    }
}

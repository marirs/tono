//! AE-BRISA mode selection and horizontal flute controls.
use super::{
    diagram::{DiagramState, PressedStyle, FONT_FAMILY},
    Instrument, LayoutKey,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FingeringMode {
    Brisa,
    Flute,
}

impl FingeringMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Brisa => "brisa",
            Self::Flute => "flute",
        }
    }
}

/// Shared by the CLI and library, before audio extraction or ML work.
pub fn validate_mode(instrument: Instrument, mode: Option<FingeringMode>) -> Result<()> {
    instrument.validate_melody_support()?;
    match (instrument,mode) {
        (Instrument::AeBrisa,None) => bail!("--instrument ae-brisa requires --fingering-mode brisa|flute; select the same Fingering Mode on the instrument"),
        (Instrument::AeBrisa,Some(_)) | (_,None) => Ok(()),
        _ => bail!("--fingering-mode is only supported with --instrument ae-brisa"),
    }
}

pub fn render(
    keys: &[LayoutKey],
    state: &DiagramState,
    x: f32,
    y: f32,
    scale: f32,
    prefix: &str,
) -> String {
    let mut svg = format!(
        r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><path d="M15 132 H775 V171 H15 Z" fill="#3a4351" stroke="#758091" stroke-width="3"/><ellipse cx="24" cy="150" rx="20" ry="27" fill="#242c37" stroke="#758091" stroke-width="3"/><text x="170" y="35" text-anchor="middle" fill="#8a93a3" font-size="22">LEFT HAND</text><text x="555" y="35" text-anchor="middle" fill="#8a93a3" font-size="22">RIGHT HAND</text><text x="140" y="250" text-anchor="middle" fill="#8a93a3" font-size="19">REAR KEYS (L / R)</text>"##
    );
    for key in keys.iter().filter(|key| !key.id.starts_with("breath_")) {
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
        let outline = if pressed { "#ffb000" } else { "#758091" };
        let (kx, ky) = (key.x, key.y);
        if key.shape == "circle" {
            let _ = write!(
                svg,
                r##"<circle id="{prefix}{}" cx="{kx}" cy="{ky}" r="26" fill="{fill}" stroke="{outline}" stroke-width="3"/>"##,
                key.id
            );
        } else {
            let _ = write!(
                svg,
                r##"<rect id="{prefix}{}" x="{}" y="{}" width="48" height="34" rx="9" fill="{fill}" stroke="{outline}" stroke-width="3"/>"##,
                key.id,
                kx - 24.0,
                ky - 17.0
            );
        }
        if let Some(change) = state.transition_hint {
            let color = if change.press.contains(&key.id) {
                Some("#3ddc84")
            } else if change.lift.contains(&key.id) {
                Some("#ff5c5c")
            } else {
                None
            };
            if let Some(color) = color {
                let dash = if change.lift.contains(&key.id) {
                    " stroke-dasharray=\"7 5\""
                } else {
                    ""
                };
                let _ = write!(
                    svg,
                    r##"<circle cx="{kx}" cy="{ky}" r="33" fill="none" stroke="{color}" stroke-width="4"{dash}/>"##
                );
            }
        }
        let color = if pressed { "#151922" } else { "#d6deeb" };
        let _ = write!(
            svg,
            r##"<text x="{kx}" y="{}" text-anchor="middle" font-size="20" font-weight="700" fill="{color}">{}</text>"##,
            ky + 7.0,
            key.label
        );
    }
    let both = state.pressed_keys.contains("breath_both");
    let upper = state.pressed_keys.contains("breath_upper");
    let cue = if both {
        "BLOW INTO BOTH HOLES"
    } else if upper {
        "COVER LOWER HOLE; BLOW INTO UPPER"
    } else if state.pressed_keys.contains("thumb_left") {
        "OCTAVE +2: HOLD BOTH REAR KEYS"
    } else if state.pressed_keys.contains("thumb_right") {
        "OCTAVE +1: HOLD RIGHT REAR KEY"
    } else {
        "BASE OCTAVE: RELEASE BOTH REAR KEYS"
    };
    if both || upper {
        for (dy, on) in [(0.0, true), (18.0, both)] {
            let fill = if on { "#ffb000" } else { "#394355" };
            let _ = write!(
                svg,
                r##"<rect x="335" y="{}" width="90" height="12" rx="5" fill="{fill}"/>"##,
                275.0 + dy
            );
        }
    }
    let _ = write!(
        svg,
        r##"<text x="390" y="355" font-size="25" font-weight="700" text-anchor="middle" fill="#ffb000">{cue}</text>"##
    );
    let same = state
        .transition_hint
        .is_some_and(|t| t.press.is_empty() && t.lift.is_empty());
    let hint = if same {
        "SAME AGAIN"
    } else {
        "GREEN: PRESS  /  RED: LIFT"
    };
    let _ = write!(
        svg,
        r##"<text x="390" y="400" font-size="22" text-anchor="middle" fill="#c8cfdb">{hint}</text></g>"##
    );
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fingering::FingeringTable;
    #[test]
    fn flute_breath_cue_changes_without_turning_into_an_octave_button() {
        let table =
            FingeringTable::load_for_mode(Instrument::AeBrisa, Some(FingeringMode::Flute)).unwrap();
        for (midi, cue) in [
            (60, "BLOW INTO BOTH HOLES"),
            (74, "COVER LOWER HOLE; BLOW INTO UPPER"),
        ] {
            let keys = table.lookup(midi).unwrap().pressed_key_ids();
            let state = DiagramState {
                upper_hand: super::super::diagram::UpperHand::Left,
                pressed_keys: &keys,
                pressed_style: PressedStyle::Sounding,
                transition_hint: None,
            };
            let svg = render(&table.layout_keys, &state, 0.0, 0.0, 1.0, "now-");
            assert!(svg.contains(cue));
            assert!(!svg.contains("OCTAVE +"));
        }
    }

    #[test]
    fn mode_is_explicit_and_only_valid_for_brisa() {
        assert!(validate_mode(Instrument::AeBrisa, None).is_err());
        assert!(validate_mode(Instrument::Ae01, Some(FingeringMode::Brisa)).is_err());
        assert!(FingeringTable::load_instrument(Instrument::AeBrisa).is_err());
    }
    #[test]
    fn chart_ranges_rear_keys_and_breath_states_are_mode_specific() {
        let brisa =
            FingeringTable::load_for_mode(Instrument::AeBrisa, Some(FingeringMode::Brisa)).unwrap();
        let flute =
            FingeringTable::load_for_mode(Instrument::AeBrisa, Some(FingeringMode::Flute)).unwrap();
        assert_eq!(brisa.charted_range(), Some((60, 97)));
        assert_eq!(flute.charted_range(), Some((60, 96)));
        assert!(!brisa.verified && !flute.verified);
        for midi in 60..=97 {
            brisa.lookup(midi).unwrap();
        }
        for midi in 60..=96 {
            flute.lookup(midi).unwrap();
        }
        assert_eq!(brisa.lookup(66).unwrap().keys, ["1", "2", "3", "5"]);
        assert!(flute.lookup(66).unwrap().keys.contains(&"6".into()));
        assert!(!flute.lookup(66).unwrap().keys.contains(&"5".into()));
        assert_eq!(
            brisa.lookup(97).unwrap().keys,
            ["thumb_left", "thumb_right"]
        );
        assert!(brisa
            .lookup(85)
            .unwrap()
            .keys
            .contains(&"thumb_right".into()));
        assert!(flute
            .lookup(60)
            .unwrap()
            .keys
            .contains(&"breath_both".into()));
        assert!(flute
            .lookup(74)
            .unwrap()
            .keys
            .contains(&"breath_upper".into()));
        assert!(flute.alternatives["72"]
            .iter()
            .any(|f| f.keys.contains(&"breath_upper".into())));
        assert_eq!(
            flute.lookup(94).unwrap().keys,
            ["4", "thumb_right", "trill2", "breath_upper"]
        );
    }
}

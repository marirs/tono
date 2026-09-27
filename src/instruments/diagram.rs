//! Shared diagram state and presentation primitives.
use super::fingering::KeyTransition;
use std::collections::BTreeSet;

/// Display labels for the player's grip; physical key IDs never change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpperHand {
    Right,
    Left,
}

impl UpperHand {
    pub(super) fn labels(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Right => ("RIGHT", "LEFT", "RIGHT THUMB"),
            Self::Left => ("LEFT", "RIGHT", "LEFT THUMB"),
        }
    }
}

/// How pressed keys are painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressedStyle {
    /// Note is sounding now: solid fill.
    Sounding,
    /// Note is coming up (count-in or articulation gap): outlined, dim fill.
    Ready,
    /// Small NEXT preview.
    Preview,
}

pub struct DiagramState<'a> {
    pub upper_hand: UpperHand,
    pub pressed_keys: &'a BTreeSet<String>,
    pub pressed_style: PressedStyle,
    /// When set, keys that change are ringed (green = press, red = lift).
    pub transition_hint: Option<&'a KeyTransition>,
}

pub const FONT_FAMILY: &str = "Helvetica Neue, Helvetica, Arial, DejaVu Sans, sans-serif";

//! Vector diagram of the AE-01 control surface.
//!
//! Every pressable area is a stable SVG region identified by a key id (spec
//! "AE-01 visual"). Fingerings reference these ids, never pixel positions.
//!
//! Layout source: Roland AE-01 Owner's Manual p. 2 (panel), p. 6 (key
//! numbering), p. 11 (hand positions). Front, from the mouthpiece: keys 1-3
//! (left index/middle/ring), small [#] above [b] (left little finger), keys
//! 4-7 (right index..little; key 7 sits slightly offset). Back: octave UP and
//! DOWN keys for the left thumb. There is no thumb hole. The instrument is
//! drawn upright, mouthpiece at the top, with configurable hand labels above and below the accidental keys.
//! Not yet compared against a physical instrument.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::fingering::KeyTransition;

/// Diagram coordinate space before scaling: the instrument spans
/// 640 x 880 units, including hand labels and the separate rear-key inset.
pub const DIAGRAM_WIDTH: f32 = 640.0;

/// Display labels for the player's grip; physical key IDs never change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpperHand { Right, Left }

impl UpperHand {
    fn labels(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Right => ("RIGHT", "LEFT", "RIGHT THUMB"),
            Self::Left => ("LEFT", "RIGHT", "LEFT THUMB"),
        }
    }
}

enum KeyShape {
    Circle { center_x: f32, center_y: f32, radius: f32 },
    RoundedRect { x: f32, y: f32, width: f32, height: f32 },
}

struct KeyRegion {
    id: &'static str,
    shape: KeyShape,
    /// Printed on the key; front finger holes stay unlabeled so the default
    /// beginner view is purely visual.
    label: Option<&'static str>,
}

const fn finger_hole(id: &'static str, center_y: f32) -> KeyRegion {
    KeyRegion {
        id,
        shape: KeyShape::Circle { center_x: 300.0, center_y, radius: 31.0 },
        label: None,
    }
}

/// The two small accidental keys between key 3 and key 4.
const fn accidental_key(id: &'static str, center_y: f32, label: &'static str) -> KeyRegion {
    KeyRegion {
        id,
        shape: KeyShape::Circle { center_x: 333.0, center_y, radius: 19.0 },
        label: Some(label),
    }
}

const fn small_button(id: &'static str, x: f32, y: f32, label: &'static str) -> KeyRegion {
    KeyRegion {
        id,
        shape: KeyShape::RoundedRect { x, y, width: 120.0, height: 50.0 },
        label: Some(label),
    }
}

/// Ordered from the mouthpiece, matching Roland's key numbering.
const KEY_REGIONS: [KeyRegion; 11] = [
    finger_hole("left_1", 190.0),  // key 1
    finger_hole("left_2", 270.0),  // key 2
    finger_hole("left_3", 350.0),  // key 3
    accidental_key("sharp", 411.0, "#"),
    accidental_key("flat", 457.0, "b"),
    finger_hole("right_1", 525.0), // key 4
    finger_hole("right_2", 605.0), // key 5
    finger_hole("right_3", 685.0), // key 6
    KeyRegion {
        // Key 7 is offset to the player's right, as in the front-view chart.
        id: "right_4",
        shape: KeyShape::Circle { center_x: 275.0, center_y: 765.0, radius: 31.0 },
        label: None,
    },
    small_button("octave_up", 460.0, 215.0, "UP"),
    small_button("octave_down", 460.0, 285.0, "DOWN"),
];

pub fn is_known_key_id(key_id: &str) -> bool {
    KEY_REGIONS.iter().any(|region| region.id == key_id)
}

#[cfg(test)]
pub fn all_key_ids() -> Vec<&'static str> {
    KEY_REGIONS.iter().map(|region| region.id).collect()
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

/// Only label LIFT/PRESS when the change is small enough to read at a
/// glance; for bigger changes the full diagram already says everything.
const MAX_CHANGED_KEYS_FOR_TEXT_HINT: usize = 2;

const IDLE_FILL: &str = "#262c38";
const IDLE_STROKE: &str = "#5a6272";
const PRESSED_FILL: &str = "#ffb000";
const READY_FILL: &str = "#6b4f0a";
const PRESS_HINT_COLOR: &str = "#3ddc84";
const LIFT_HINT_COLOR: &str = "#ff5c5c";
pub const FONT_FAMILY: &str = "Helvetica Neue, Helvetica, Arial, sans-serif";

/// Returns an SVG `<g>` drawing the instrument, positioned by `transform`.
/// `region_id_prefix` keeps ids unique when several diagrams share a frame
/// (e.g. `now-left_1`, `next-left_1`).
pub fn instrument_group_svg(state: &DiagramState, transform: &str, region_id_prefix: &str) -> String {
    let mut svg = String::new();
    let _ = write!(svg, r##"<g transform="{transform}">"##);
    svg.push_str(&instrument_body_svg(state.upper_hand));

    for region in &KEY_REGIONS {
        let is_pressed = state.pressed_keys.contains(region.id);
        let (fill, stroke, stroke_width) = key_paint(is_pressed, state.pressed_style);
        let _ = write!(
            svg,
            r##"<g id="{region_id_prefix}{id}">{shape}"##,
            id = region.id,
            shape = shape_svg(&region.shape, fill, stroke, stroke_width, None),
        );
        if let Some(label) = region.label {
            let (center_x, center_y) = shape_center(&region.shape);
            let text_color = if is_pressed { "#12151c" } else { "#c8cfdb" };
            let _ = write!(
                svg,
                r##"<text x="{center_x}" y="{y}" font-family="{FONT_FAMILY}" font-size="26" font-weight="700" fill="{text_color}" text-anchor="middle">{label}</text>"##,
                y = center_y + 9.0,
            );
        }
        svg.push_str("</g>");
    }

    if let Some(transition) = state.transition_hint {
        svg.push_str(&transition_hint_svg(transition));
    }
    svg.push_str("</g>");
    svg
}

fn instrument_body_svg(upper_hand: UpperHand) -> String {
    let (upper, lower, thumb) = upper_hand.labels();
    format!(
        concat!(
            r##"<path d="M277 12 L323 12 L338 105 L262 105 Z" fill="#3a4150" stroke="{idle}" stroke-width="3"/>"##,
            r##"<path d="M262 95 Q230 110 230 150 L230 795 Q230 865 300 865 Q370 865 370 795 L370 150 Q370 110 338 95 Z" fill="#1b2029" stroke="#4a5263" stroke-width="4"/>"##,
            r##"<path d="M205 155 L185 155 L185 385 L205 385 M205 490 L185 490 L185 805 L205 805" fill="none" stroke="#4a5263" stroke-width="3"/>"##,
            r##"<text x="160" y="260" font-family="{font}" font-size="24" fill="#8a93a3" text-anchor="end" letter-spacing="2">{upper}</text><text x="160" y="290" font-family="{font}" font-size="24" fill="#8a93a3" text-anchor="end" letter-spacing="2">HAND</text>"##,
            r##"<text x="160" y="625" font-family="{font}" font-size="24" fill="#8a93a3" text-anchor="end" letter-spacing="2">{lower}</text><text x="160" y="655" font-family="{font}" font-size="24" fill="#8a93a3" text-anchor="end" letter-spacing="2">HAND</text>"##,
            r##"<rect x="440" y="195" width="160" height="160" rx="22" fill="none" stroke="#4a5263" stroke-width="2" stroke-dasharray="6 6"/>"##,
            r##"<text x="520" y="145" font-family="{font}" font-size="20" fill="#8a93a3" text-anchor="middle">BACK</text><text x="520" y="175" font-family="{font}" font-size="18" fill="#6b7383" text-anchor="middle">{thumb}</text>"##,
        ),
        idle = IDLE_STROKE,
        font = FONT_FAMILY,
        upper = upper, lower = lower, thumb = thumb,
    )
}

fn key_paint(is_pressed: bool, style: PressedStyle) -> (&'static str, &'static str, f32) {
    match (is_pressed, style) {
        (false, _) => (IDLE_FILL, IDLE_STROKE, 3.0),
        (true, PressedStyle::Sounding) => (PRESSED_FILL, "#ffffff", 5.0),
        (true, PressedStyle::Ready) => (READY_FILL, PRESSED_FILL, 5.0),
        (true, PressedStyle::Preview) => (PRESSED_FILL, PRESSED_FILL, 3.0),
    }
}

fn shape_svg(
    shape: &KeyShape,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    dash_pattern: Option<&str>,
) -> String {
    let dash = dash_pattern
        .map(|pattern| format!(r##" stroke-dasharray="{pattern}""##))
        .unwrap_or_default();
    match *shape {
        KeyShape::Circle { center_x, center_y, radius } => format!(
            r##"<circle cx="{center_x}" cy="{center_y}" r="{radius}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}"{dash}/>"##
        ),
        KeyShape::RoundedRect { x, y, width, height } => format!(
            r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="14" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}"{dash}/>"##
        ),
    }
}

fn shape_center(shape: &KeyShape) -> (f32, f32) {
    match *shape {
        KeyShape::Circle { center_x, center_y, .. } => (center_x, center_y),
        KeyShape::RoundedRect { x, y, width, height } => (x + width / 2.0, y + height / 2.0),
    }
}

/// Outer rings (and optional words) around keys that change in the
/// upcoming transition.
fn transition_hint_svg(transition: &KeyTransition) -> String {
    let show_words = transition.changed_key_count() <= MAX_CHANGED_KEYS_FOR_TEXT_HINT;
    let mut svg = String::new();
    let hint_groups = [
        (&transition.press, PRESS_HINT_COLOR, "PRESS", None),
        (&transition.lift, LIFT_HINT_COLOR, "LIFT", Some("14 10")),
    ];
    for (key_ids, color, word, dash_pattern) in hint_groups {
        let rings: Vec<KeyShape> = key_ids
            .iter()
            // Unknown ids cannot occur: they are rejected when the table loads.
            .filter_map(|key_id| KEY_REGIONS.iter().find(|region| region.id == key_id.as_str()))
            .map(|region| grow_shape(&region.shape, 12.0))
            .collect();
        for ring in &rings {
            svg.push_str(&shape_svg(ring, "none", color, 9.0, dash_pattern));
        }
        if show_words && !rings.is_empty() {
            let x = if word == "PRESS" { 200.0 } else { 400.0 };
            let _ = write!(
                svg,
                r##"<text x="{x}" y="910" font-family="{FONT_FAMILY}" font-size="32" font-weight="800" fill="{color}" text-anchor="middle">{word}</text>"##,
            );
        }
    }
    svg
}

fn grow_shape(shape: &KeyShape, margin: f32) -> KeyShape {
    match *shape {
        KeyShape::Circle { center_x, center_y, radius } => {
            KeyShape::Circle { center_x, center_y, radius: radius + margin }
        }
        KeyShape::RoundedRect { x, y, width, height } => KeyShape::RoundedRect {
            x: x - margin,
            y: y - margin,
            width: width + 2.0 * margin,
            height: height + 2.0 * margin,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hand_labels_match_selected_grip_without_changing_key_ids() {
        let right = instrument_body_svg(UpperHand::Right);
        let left = instrument_body_svg(UpperHand::Left);
        assert!(right.contains("RIGHT THUMB"));
        assert!(left.contains("LEFT THUMB"));
        assert!(right.contains("letter-spacing=\"2\">RIGHT</text>"));
        assert_eq!(all_key_ids().len(), 11);
    }

    #[test]
    fn key_ids_are_unique() {
        let ids = all_key_ids();
        let unique: BTreeSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }

    #[test]
    fn spec_example_ids_exist() {
        for id in ["octave_up", "octave_down", "left_1", "left_2", "left_3", "right_1", "right_2", "right_3", "right_4", "sharp", "flat"] {
            assert!(is_known_key_id(id), "{id}");
        }
    }

    #[test]
    fn every_key_is_emitted_with_prefixed_id() {
        let pressed: BTreeSet<String> = ["left_1".to_owned()].into();
        let state = DiagramState {
            upper_hand: UpperHand::Right,
            pressed_keys: &pressed,
            pressed_style: PressedStyle::Sounding,
            transition_hint: None,
        };
        let svg = instrument_group_svg(&state, "translate(0 0)", "now-");
        for id in all_key_ids() {
            assert!(svg.contains(&format!(r##"id="now-{id}""##)), "{id} missing");
        }
        assert!(svg.contains(PRESSED_FILL));
    }
}

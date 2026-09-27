//! YDS-120/150 share Yamaha's chart and controls, including Oct and Low A.
use super::{diagram::DiagramState, LayoutKey};

pub fn render(keys: &[LayoutKey], state: &DiagramState, transform: &str, prefix: &str) -> String {
    super::sax::render(keys, state, transform, prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::{
        diagram::{PressedStyle, UpperHand},
        fingering::{FingeringTable, KeyTransition, OctaveShift},
        Instrument,
    };

    #[test]
    fn yamaha_chart_controls_and_alternatives_are_preserved() {
        for instrument in [Instrument::Yds120, Instrument::Yds150] {
            let table = FingeringTable::load_instrument(instrument).unwrap();
            assert_eq!(
                table.lookup(57).unwrap().keys,
                ["1", "2", "3", "4", "5", "6", "C", "Bb", "low_a"]
            );
            assert_eq!(table.lookup(73).unwrap().keys, Vec::<String>::new());
            assert_eq!(
                table.lookup(74).unwrap().keys,
                ["1", "2", "3", "4", "5", "6", "oct"]
            );
            assert_eq!(
                table.lookup(90).unwrap().keys,
                ["C1", "C2", "C3", "C4", "C5", "oct"]
            );
            assert_eq!(table.alternatives["70"].len(), 3);
            assert!(table.alternatives["88"]
                .iter()
                .any(|f| f.keys == ["X", "2", "3", "oct"]));
            for midi in 57..=90 {
                assert_eq!(table.lookup(midi).unwrap().octave, OctaveShift::Normal);
            }
            let low = table.lookup(57).unwrap();
            let high = table.lookup(90).unwrap();
            let transition = KeyTransition::between(low, high);
            let pressed = high.pressed_key_ids();
            let state = DiagramState {
                upper_hand: UpperHand::Left,
                pressed_keys: &pressed,
                pressed_style: PressedStyle::Sounding,
                transition_hint: Some(&transition),
            };
            let svg = render(&table.layout_keys, &state, "translate(0 0)", "now-");
            assert!(svg.contains("id=\"now-oct\""));
            assert!(svg.contains("id=\"now-low_a\""));
            assert!(svg.contains("#3ddc84") && svg.contains("#ff5c5c"));
            assert!(!svg.contains("octave_down"));
        }
    }
}

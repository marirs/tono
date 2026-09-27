//! Live treble-staff pitch guide, using the notes actually being played.
//! No time signature or quantized rhythm is inferred: duration stays in the
//! existing animation, and each note name/accidental is explicit.
use crate::instruments::diagram::FONT_FAMILY;
use crate::music::timeline::{NowState, PracticeTimeline};
use std::fmt::Write;

const NOTES_PER_ROW: usize = 6;

fn pitch(midi: u8) -> (i32, &'static str, bool, i32) {
    const STEPS: [i32; 12] = [0, 0, 1, 1, 2, 3, 3, 4, 4, 5, 5, 6];
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let class = midi as usize % 12;
    let octave = midi as i32 / 12 - 1;
    (
        octave * 7 + STEPS[class],
        NAMES[class],
        matches!(class, 1 | 3 | 6 | 8 | 10),
        octave,
    )
}

fn pitch_y(midi: u8, bottom: f32) -> f32 {
    // E4 is the bottom line of a treble staff; each diatonic step is half a line.
    bottom - (pitch(midi).0 - 30) as f32 * 10.0
}

// Keep extreme register notes inside the panel, using explicit octave
// indications rather than clipping ledger lines or changing the played pitch.
fn staff_register(midi: u8) -> (u8, &'static str) {
    let mut written = midi as i32;
    let mut octaves = 0;
    while written < 59 {
        written += 12;
        octaves -= 1;
    }
    while written > 84 {
        written -= 12;
        octaves += 1;
    }
    let mark = match octaves {
        -4 => "29mb",
        -3 => "22mb",
        -2 => "15mb",
        -1 => "8vb",
        1 => "8va",
        2 => "15ma",
        3 => "22ma",
        4 => "29ma",
        _ => "",
    };
    (written as u8, mark)
}

pub fn staff_svg(timeline: &PracticeTimeline, state: NowState, count: Option<u8>) -> String {
    let mut svg = format!(
        r##"<g font-family="{FONT_FAMILY}"><rect x="1010" y="205" width="850" height="765" rx="24" fill="#191e28" stroke="#303846" stroke-width="2"/><text x="1050" y="252" font-size="27" font-weight="700" fill="#c8cfdb" letter-spacing="5">MELODY</text><text x="1050" y="286" font-size="21" fill="#8a93a3">Played pitches · follow the highlighted note</text>"##
    );
    if let Some(number) = count {
        let total = timeline.count_in.map_or(4, |count| count.beats);
        for i in 1..=total {
            let x = 1120.0 + (i as f32 - 1.0) * 180.0;
            let color = if i == number { "#ffb000" } else { "#4a5263" };
            let _ = write!(
                svg,
                r##"<text x="{x}" y="380" font-size="74" font-weight="800" fill="{color}" text-anchor="middle">{i}</text>"##
            );
        }
    } else if let Some(index) = state.current_index() {
        let (_, name, _, octave) = pitch(timeline.entries[index].midi);
        let label = if matches!(state, NowState::Sounding(_)) {
            "NOW"
        } else {
            "UP NEXT"
        };
        let _ = write!(
            svg,
            r##"<text x="1050" y="374" font-size="64" font-weight="700" fill="#ffb000">{name}{octave}</text><text x="1200" y="368" font-size="23" fill="#8a93a3">{label}</text>"##
        );
    } else {
        svg.push_str(r##"<text x="1050" y="370" font-size="42" fill="#8a93a3">FINISHED</text>"##);
    }
    let current = state
        .current_index()
        .unwrap_or_else(|| timeline.entries.len().saturating_sub(1));
    let page = current / NOTES_PER_ROW * NOTES_PER_ROW;
    for row in 0..2 {
        let bottom = 550.0 + row as f32 * 255.0;
        for line in 0..5 {
            let y = bottom - line as f32 * 20.0;
            let _ = write!(
                svg,
                r##"<path d="M1050 {y} H1820" stroke="#667080" stroke-width="2"/>"##
            );
        }
        let _ = write!(
            svg,
            r##"<text x="1060" y="{y}" font-family="Apple Symbols" font-size="114" fill="#aab2bf">𝄞</text>"##,
            y = bottom + 13.0
        );
        for column in 0..NOTES_PER_ROW {
            let index = page + row * NOTES_PER_ROW + column;
            let Some(note) = timeline.entries.get(index) else {
                break;
            };
            let x = 1180.0 + column as f32 * 120.0;
            let (written_midi, octave_mark) = staff_register(note.midi);
            let y = pitch_y(written_midi, bottom);
            let (_, name, sharp, octave) = pitch(note.midi);
            let active = state.current_index() == Some(index);
            let color = if active {
                "#ffb000"
            } else if index < current {
                "#687182"
            } else {
                "#e2e6ed"
            };
            if active {
                let _ = write!(
                    svg,
                    r##"<rect x="{x}" y="{y}" width="90" height="205" rx="14" fill="#ffb000" opacity="0.09"/>"##,
                    x = x - 45.0,
                    y = bottom - 125.0
                );
            }
            // Ledger lines for pitches below E4 or above F5.
            let mut ledger = bottom + 20.0;
            while ledger <= y {
                let _ = write!(
                    svg,
                    r##"<path d="M{left} {ledger} H{right}" stroke="{color}" stroke-width="2"/>"##,
                    left = x - 22.0,
                    right = x + 22.0
                );
                ledger += 20.0;
            }
            ledger = bottom - 100.0;
            while ledger >= y {
                let _ = write!(
                    svg,
                    r##"<path d="M{left} {ledger} H{right}" stroke="{color}" stroke-width="2"/>"##,
                    left = x - 22.0,
                    right = x + 22.0
                );
                ledger -= 20.0;
            }
            if !octave_mark.is_empty() {
                let _ = write!(
                    svg,
                    r##"<text x="{x}" y="{y}" font-size="18" text-anchor="middle" fill="{color}">{octave_mark}</text>"##,
                    y = bottom + 105.0
                );
            }
            let accidental = if sharp { "♯" } else { "♮" };
            let _ = write!(
                svg,
                r##"<g id="staff-note-{index}"><text x="{ax}" y="{ay}" font-family="Apple Symbols" font-size="35" fill="{color}" text-anchor="middle">{accidental}</text><ellipse cx="{x}" cy="{y}" rx="14" ry="10" transform="rotate(-18 {x} {y})" fill="{color}"/><text x="{x}" y="{label_y}" font-size="23" fill="{color}" text-anchor="middle">{name}{octave}</text></g>"##,
                ax = x - 32.0,
                ay = y + 11.0,
                label_y = bottom + 65.0
            );
        }
    }
    svg.push_str(r##"<text x="1050" y="940" font-size="19" fill="#8a93a3">Pitch guide · the bar under NOW counts down to the next note</text></g>"##);
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_profile_ranges_keep_staff_visible_without_changing_pitch_class() {
        for midi in 21..=108 {
            let (written, mark) = staff_register(midi);
            assert_eq!(written % 12, midi % 12);
            assert!((59..=84).contains(&written));
            assert_eq!(mark.is_empty(), written == midi);
            assert!(
                pitch_y(written, 550.0) <= 580.0,
                "ledger lines must not overlap pitch labels"
            );
        }
        assert_eq!(staff_register(40), (64, "15mb"));
        assert_eq!(staff_register(97), (73, "15ma"));
    }
    #[test]
    fn treble_staff_pitch_positions_and_accidentals() {
        assert_eq!(pitch_y(64, 550.0), 550.0); // E4, bottom line
        assert_eq!(pitch_y(60, 550.0), 570.0); // C4, first ledger line
        assert_eq!(pitch_y(59, 550.0), 580.0); // B3, below that ledger
        assert_eq!(pitch_y(72, 550.0), 500.0); // C5
        assert_eq!(pitch_y(63, 550.0), pitch_y(62, 550.0));
        assert!(pitch(63).2);
        assert!(!pitch(62).2);
    }
}

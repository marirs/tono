//! Model-specific diagrams. Control positions come from profile JSON.
use std::fmt::Write;
use crate::ae01_diagram::{self, DiagramState, PressedStyle, FONT_FAMILY};
use crate::instruments::{guitar_position, Instrument, LayoutKey};

fn escape(text: &str) -> String { text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;") }

pub fn wind_svg(instrument: Instrument, keys: &[LayoutKey], state: &DiagramState, transform: &str, prefix: &str) -> String {
    if instrument == Instrument::Ae01 { return ae01_diagram::instrument_group_svg(state, transform, prefix); }
    let mut svg = format!(r##"<g transform="{transform}" font-family="{FONT_FAMILY}"><path d="M270 10 L325 10 L345 95 L255 95 Z" fill="#3a4150" stroke="#5a6272" stroke-width="3"/><path d="M255 95 L350 95 L460 520 L385 865 L220 865 L155 620 L225 350 Z" fill="#1b2029" stroke="#4a5263" stroke-width="4"/><rect x="480" y="130" width="150" height="360" rx="22" fill="none" stroke="#4a5263" stroke-dasharray="6 6"/><text x="555" y="105" font-size="21" text-anchor="middle" fill="#8a93a3">BACK / OCTAVE</text>"##);
    let (upper, lower) = match state.upper_hand { ae01_diagram::UpperHand::Left => ("LEFT", "RIGHT"), _ => ("RIGHT", "LEFT") };
    let _ = write!(svg, r##"<text x="120" y="240" font-size="22" text-anchor="end" fill="#8a93a3">{upper}</text><text x="120" y="270" font-size="22" text-anchor="end" fill="#8a93a3">HAND</text><text x="120" y="650" font-size="22" text-anchor="end" fill="#8a93a3">{lower}</text><text x="120" y="680" font-size="22" text-anchor="end" fill="#8a93a3">HAND</text>"##);
    for key in keys {
        let pressed = state.pressed_keys.contains(&key.id);
        let fill = if pressed { if state.pressed_style == PressedStyle::Ready { "#6b4f0a" } else { "#ffb000" } } else { "#262c38" };
        let stroke = if pressed { "#ffb000" } else { "#5a6272" };
        let _ = write!(svg, r##"<g id="{}{}">{}</g>"##, escape(prefix), escape(&key.id), key_shape(key, 0.0, fill, stroke, false));
        if let Some(change) = state.transition_hint {
            if change.press.contains(&key.id) { svg.push_str(&key_shape(key, 8.0, "none", "#3ddc84", false)); }
            if change.lift.contains(&key.id) { svg.push_str(&key_shape(key, 8.0, "none", "#ff5c5c", true)); }
        }
        let color = if pressed { "#12151c" } else { "#c8cfdb" };
        let _ = write!(svg, r##"<text x="{}" y="{}" font-size="21" font-weight="700" text-anchor="middle" fill="{color}">{}</text>"##, key.x, key.y+7.0, escape(&key.label));
    }
    if let Some(change) = state.transition_hint {
        for (x, label, color, present) in [(230,"PRESS","#3ddc84",!change.press.is_empty()),(390,"LIFT","#ff5c5c",!change.lift.is_empty())] {
            if present { let _ = write!(svg, r##"<text x="{x}" y="910" font-size="30" text-anchor="middle" font-weight="800" fill="{color}">{label}</text>"##); }
        }
    }
    svg.push_str("</g>"); svg
}

fn key_shape(key: &LayoutKey, grow: f32, fill: &str, stroke: &str, dashed: bool) -> String {
    let dash = if dashed { " stroke-dasharray=\"9 7\"" } else { "" };
    if key.shape == "circle" {
        format!(r##"<circle cx="{}" cy="{}" r="{}" fill="{fill}" stroke="{stroke}" stroke-width="4"{dash}/>"##, key.x,key.y,24.0+grow)
    } else {
        let width = if key.shape == "octave" { 100.0 } else { 44.0 };
        format!(r##"<rect x="{}" y="{}" width="{}" height="{}" rx="9" fill="{fill}" stroke="{stroke}" stroke-width="4"{dash}/>"##,key.x-width/2.0-grow,key.y-18.0-grow,width+2.0*grow,36.0+2.0*grow)
    }
}

/// Horizontal single-note guitar view: string 1 (high E) at the top, nut left.
/// A five-fret window keeps the active position readable on a phone.
pub fn guitar_svg(state: &DiagramState, x: f32, y: f32, scale: f32, prefix: &str) -> String {
    let selected = state.pressed_keys.iter().find_map(|key| guitar_position(key));
    let fret = selected.map_or(0, |(_, f)| f);
    let first = fret.saturating_sub(2).max(1).min(15);
    let mut svg = format!(r##"<g transform="translate({x} {y}) scale({scale})" font-family="{FONT_FAMILY}"><rect x="110" y="30" width="660" height="260" rx="12" fill="#30271f" stroke="#6a5b48" stroke-width="3"/>"##);
    let _ = write!(svg, r##"<text x="115" y="0" fill="#c8cfdb" font-size="24">FRETS {first}–{} · HIGH E AT TOP</text>"##, first+4);
    for col in 0..=5 {
        let xx=110+col*132;
        let _=write!(svg,r##"<path d="M{xx} 30 V290" stroke="#8a8174" stroke-width="4"/>"##);
        if col<5 { let _=write!(svg,r##"<text x="{}" y="326" font-size="25" text-anchor="middle" fill="#c8cfdb">{}</text>"##,xx+66,first+col as u8); }
    }
    for (i,label) in ["1 e","2 B","3 G","4 D","5 A","6 E"].iter().enumerate() {
        let yy=50+i*44;
        let _=write!(svg,r##"<text x="55" y="{}" font-size="24" text-anchor="end" fill="#c8cfdb">{label}</text><path d="M75 {yy} H770" stroke="#a4a9b3" stroke-width="{}"/>"##,yy+8,1.5+i as f32*0.35);
    }
    let point = |string:usize, f:u8| -> Option<(f32,f32)> {
        let xx = if f==0 { 82.0 } else if (first..=first+4).contains(&f) { 176.0+(f-first) as f32*132.0 } else { return None };
        Some((xx,50.0+(string-1) as f32*44.0))
    };
    if let Some((string,f))=selected {
        let (xx,yy)=point(string,f).expect("active fret always visible");
        let fill=if state.pressed_style==PressedStyle::Ready { "#6b4f0a" } else { "#ffb000" };
        let _=write!(svg,r##"<circle id="{prefix}s{string}_f{f}" cx="{xx}" cy="{yy}" r="19" fill="{fill}" stroke="#ffb000" stroke-width="4"/><text x="440" y="380" font-size="34" font-weight="700" text-anchor="middle" fill="#ffb000">PLUCK STRING {string} · {}</text>"##,if f==0 { "OPEN".to_owned() } else {format!("FRET {f}")});
    }
    if let Some(change)=state.transition_hint {
        for (keys,color,dash) in [(&change.press,"#3ddc84",""),(&change.lift,"#ff5c5c"," stroke-dasharray=\"9 7\"")] {
            for key in keys {
                if let Some((string,f))=guitar_position(key) {
                    if let Some((xx,yy))=point(string,f) { let _=write!(svg,r##"<circle cx="{xx}" cy="{yy}" r="27" fill="none" stroke="{color}" stroke-width="5"{dash}/>"##); }
                }
            }
        }
        let releases: Vec<_>=change.lift.iter().filter_map(|key| guitar_position(key)).filter(|(_,f)| *f>0).map(|(s,f)|format!("S{s} F{f}")).collect();
        if !releases.is_empty() { let _=write!(svg,r##"<text x="440" y="420" font-size="23" text-anchor="middle" fill="#ff5c5c">RELEASE {}</text>"##,releases.join(", ")); }
    }
    svg.push_str("</g>");svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingering::{FingeringTable, KeyTransition};
    #[test]
    fn octave_two_and_current_transition_are_drawn() {
        let table=FingeringTable::load_instrument(Instrument::Ae20).unwrap();
        let pressed=table.lookup(97).unwrap().pressed_key_ids();
        let change=KeyTransition::between(table.lookup(73).unwrap(),table.lookup(97).unwrap());
        let state=DiagramState{upper_hand:crate::ae01_diagram::UpperHand::Left,pressed_keys:&pressed,pressed_style:PressedStyle::Sounding,transition_hint:Some(&change)};
        let svg=wind_svg(Instrument::Ae20,&table.layout_keys,&state,"translate(0 0)","now-");
        assert!(svg.contains("id=\"now-octave_up2\""));
        assert!(svg.contains("#3ddc84"));
        assert!(svg.contains(">+2</text>"));
    }
    #[test]
    fn guitar_open_and_high_fret_are_visible() {
        for key in ["s6_f0","s1_f19"] {
            let pressed=[key.to_owned()].into();
            let state=DiagramState{upper_hand:crate::ae01_diagram::UpperHand::Left,pressed_keys:&pressed,pressed_style:PressedStyle::Sounding,transition_hint:None};
            let svg=guitar_svg(&state,0.0,0.0,1.0,"now-");
            assert!(svg.contains(&format!("id=\"now-{key}\"")));
            assert!(svg.contains(if key.ends_with("f0") { "OPEN" } else { "FRET 19" }));
        }
    }
}

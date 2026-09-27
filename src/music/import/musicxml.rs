use super::{beats_until, seconds_at, ImportedMelody};
use crate::music::notes::NoteEvent;
use anyhow::{bail, Context, Result};
use roxmltree::Node;

fn child<'a, 'i>(n: Node<'a, 'i>, tag: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|x| x.has_tag_name(tag))
}
fn text<'a, 'i>(n: Node<'a, 'i>, tag: &str) -> Option<&'a str> {
    child(n, tag).and_then(|x| x.text())
}
fn num(n: Node<'_, '_>, tag: &str) -> Result<f64> {
    let v: f64 = text(n, tag)
        .with_context(|| format!("missing MusicXML {tag}"))?
        .parse()?;
    if !v.is_finite() {
        bail!("non-finite MusicXML {tag}");
    }
    Ok(v)
}

pub(super) fn parse(xml: &str, track: Option<u16>) -> Result<ImportedMelody> {
    // Never resolve external entities. Standard DOCTYPE declarations are accepted,
    // but entity declarations (including internal expansion) are not.
    if xml.contains("<!ENTITY") {
        bail!("MusicXML entity declarations are not supported");
    }
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    )?;
    let root = doc.root_element();
    if !root.has_tag_name("score-partwise") {
        bail!("export score-partwise MusicXML");
    }
    let parts: Vec<_> = root.children().filter(|n| n.has_tag_name("part")).collect();
    let idx = match track {
        Some(n) => usize::from(n) - 1,
        None if parts.len() == 1 => 0,
        _ => bail!(
            "MusicXML has {} parts; select --melody-track 1..{}",
            parts.len(),
            parts.len()
        ),
    };
    let part = *parts.get(idx).context("MusicXML part index out of range")?;
    for n in part.descendants() {
        if [
            "repeat",
            "ending",
            "backup",
            "forward",
            "chord",
            "grace",
            "unpitched",
            "ornaments",
        ]
        .iter()
        .any(|t| n.has_tag_name(*t))
        {
            bail!("MusicXML <{}> needs resolving into one linear monophonic performance before import",n.tag_name().name());
        }
        if n.has_tag_name("sound")
            && ["dacapo", "dalsegno", "tocoda", "fine"]
                .iter()
                .any(|a| n.attribute(*a).is_some())
        {
            bail!("MusicXML playback jumps must be expanded before import");
        }
    }
    let measures: Vec<_> = part
        .children()
        .filter(|n| n.has_tag_name("measure"))
        .collect();
    let mut divisions = 0.0;
    let mut expected = None;
    let mut transpose = 0i32;
    let mut cursor = 0.0;
    let mut notes: Vec<NoteEvent> = vec![];
    let mut tempos: Vec<(f64, f64)> = vec![];
    let mut pending_tie: Option<usize> = None;
    let mut warnings =
        vec!["Imported notes are preserved; their musical correctness is not verified.".into()];
    let mut voice: Option<String> = None;
    for (mi, me) in measures.iter().enumerate() {
        let begin = cursor;
        for n in me.children().filter(|n| n.is_element()) {
            match n.tag_name().name() {
                "attributes" => {
                    if child(n, "divisions").is_some() {
                        divisions = num(n, "divisions")?;
                        if divisions <= 0.0 {
                            bail!("MusicXML divisions must be positive");
                        }
                    }
                    if let Some(time) = child(n, "time") {
                        if child(time, "senza-misura").is_some() {
                            expected = None;
                        } else {
                            let beats = num(time, "beats")?;
                            let unit = num(time, "beat-type")?;
                            if beats <= 0.0 || unit <= 0.0 {
                                bail!("invalid MusicXML meter");
                            }
                            expected = Some(beats * 4.0 / unit);
                        }
                    }
                    if let Some(tr) = child(n, "transpose") {
                        let chromatic = num(tr, "chromatic")?;
                        let oct = text(tr, "octave-change").unwrap_or("0").parse::<i32>()?;
                        if chromatic.fract() != 0.0
                            || chromatic.abs() > 24.0
                            || !(-8..=8).contains(&oct)
                            || child(tr, "double").is_some()
                        {
                            bail!("unsupported MusicXML transposition");
                        }
                        transpose = chromatic as i32 + 12 * oct;
                    }
                }
                "direction" => {
                    if child(n, "offset").is_some() && num(n, "offset")? != 0.0 {
                        bail!("MusicXML direction offsets need resolving before import");
                    }
                    let sound = n
                        .descendants()
                        .find(|x| x.has_tag_name("sound") && x.attribute("tempo").is_some());
                    let bpm = if let Some(s) = sound {
                        Some(s.attribute("tempo").unwrap().parse::<f64>()?)
                    } else if let Some(m) = n.descendants().find(|x| x.has_tag_name("metronome")) {
                        let unit = match text(m, "beat-unit").unwrap_or("") {
                            "whole" => 4.0,
                            "half" => 2.0,
                            "quarter" => 1.0,
                            "eighth" => 0.5,
                            "16th" => 0.25,
                            _ => bail!("unsupported MusicXML metronome unit"),
                        };
                        let dots = m
                            .children()
                            .filter(|x| x.has_tag_name("beat-unit-dot"))
                            .count();
                        Some(num(m, "per-minute")? * unit * (2.0 - 2f64.powi(-(dots as i32))))
                    } else {
                        None
                    };
                    if let Some(bpm) = bpm {
                        if !bpm.is_finite() || bpm <= 0.0 {
                            bail!("invalid MusicXML tempo");
                        }
                        tempos.push((cursor, 60_000_000.0 / bpm));
                    }
                }
                "note" => {
                    if divisions <= 0.0 {
                        bail!("MusicXML needs divisions before notes");
                    }
                    let duration = num(n, "duration")? / divisions;
                    if duration <= 0.0 {
                        bail!("non-positive MusicXML note duration");
                    }
                    if let Some(v) = text(n, "voice") {
                        if voice.as_deref().is_some_and(|old| old != v) {
                            bail!("multiple MusicXML voices; export a single melody voice");
                        }
                        voice = Some(v.into());
                    }
                    if child(n, "rest").is_none() {
                        let p = child(n, "pitch").context("MusicXML note needs pitch or rest")?;
                        let pc = match text(p, "step").unwrap_or("") {
                            "C" => 0,
                            "D" => 2,
                            "E" => 4,
                            "F" => 5,
                            "G" => 7,
                            "A" => 9,
                            "B" => 11,
                            _ => bail!("invalid pitch step"),
                        };
                        let alter = text(p, "alter").unwrap_or("0").parse::<f64>()?;
                        if !alter.is_finite() || alter.fract() != 0.0 || alter.abs() > 2.0 {
                            bail!("microtonal/invalid alterations are unsupported");
                        }
                        let octave = text(p, "octave")
                            .context("missing octave")?
                            .parse::<i32>()?;
                        if !(-1..=9).contains(&octave) {
                            bail!("invalid MusicXML octave");
                        }
                        let midi = 12 * (octave + 1) + pc + alter as i32 + transpose;
                        if !(0..=127).contains(&midi) {
                            bail!("MusicXML pitch outside MIDI range");
                        }
                        let stop = n
                            .children()
                            .any(|x| x.has_tag_name("tie") && x.attribute("type") == Some("stop"));
                        let start = n
                            .children()
                            .any(|x| x.has_tag_name("tie") && x.attribute("type") == Some("start"));
                        let ni = if stop {
                            let ni = pending_tie
                                .take()
                                .context("MusicXML tie stop without start")?;
                            if notes[ni].midi != midi as u8 || (notes[ni].end - cursor).abs() > 1e-8
                            {
                                bail!("MusicXML tie changes pitch or crosses a rest");
                            }
                            notes[ni].end = cursor + duration;
                            ni
                        } else {
                            if pending_tie.is_some() {
                                bail!("MusicXML tie missing stop");
                            }
                            notes.push(NoteEvent {
                                start: cursor,
                                end: cursor + duration,
                                midi: midi as u8,
                                confidence: 1.0,
                            });
                            notes.len() - 1
                        };
                        if start {
                            pending_tie = Some(ni);
                        }
                    }
                    cursor += duration;
                }
                _ => {}
            }
        }
        if let Some(length) = expected {
            let actual = cursor - begin;
            if actual > length + 1e-6 {
                bail!("MusicXML measure {} contains {actual} quarter beats but its meter allows {length}; repair timing/rests or use the MIDI/JSON source",mi+1);
            }
            if actual < length - 1e-6 {
                if mi != 0 && mi + 1 != measures.len() && me.attribute("implicit") != Some("yes") {
                    bail!(
                        "MusicXML measure {} is underfilled; add explicit rests",
                        mi + 1
                    );
                }
                warnings.push(format!(
                    "MusicXML measure {} is short; imported its explicit duration without padding.",
                    mi + 1
                ));
            }
        }
    }
    if pending_tie.is_some() {
        bail!("MusicXML has an unfinished tie");
    }
    if tempos.first().is_none_or(|x| x.0 > 0.0) {
        bail!("MusicXML needs an explicit starting tempo (sound tempo or metronome)");
    }
    for n in &mut notes {
        n.start = seconds_at(n.start, &tempos);
        n.end = seconds_at(n.end, &tempos);
    }
    let duration = seconds_at(cursor, &tempos);
    let beat_times = beats_until(duration, &tempos);
    Ok(ImportedMelody {
        notes,
        duration,
        beat_times,
        warnings,
        format: "musicxml",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn score(body: &str) -> String {
        format!(
            r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Test</part-name></score-part></part-list><part id="P1">{body}</part></score-partwise>"#
        )
    }
    const ATTR: &str = r#"<attributes><divisions>2</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><direction><sound tempo="120"/></direction>"#;
    fn note(duration: u32, tie: &str) -> String {
        format!("<note><pitch><step>C</step><alter>1</alter><octave>4</octave></pitch><duration>{duration}</duration>{tie}</note>")
    }
    #[test]
    fn ties_rests_transpose_and_tempo_changes_keep_performance_time() {
        let x=score(&format!("<measure>{ATTR}<attributes><transpose><chromatic>-2</chromatic></transpose></attributes><note><rest/><duration>4</duration></note>{}</measure><measure><direction><sound tempo=\"60\"/></direction>{}<note><rest/><duration>4</duration></note></measure>",note(4,"<tie type=\"start\"/>"),note(4,"<tie type=\"stop\"/>")));
        let s = parse(&x, None).unwrap();
        assert_eq!(s.notes.len(), 1);
        assert_eq!(s.notes[0].midi, 59);
        assert_eq!(s.notes[0].start, 1.0);
        assert_eq!(s.notes[0].end, 4.0);
        assert_eq!(s.duration, 6.0);
    }
    #[test]
    fn rejects_overfull_measure_like_original_ek_pyar_sample() {
        let x = score(&format!("<measure>{ATTR}{}</measure>", note(59, "")));
        assert!(parse(&x, None)
            .unwrap_err()
            .to_string()
            .contains("meter allows"));
    }
    #[test]
    fn short_edge_measures_warn_but_are_not_silently_padded() {
        let x = score(&format!(
            "<measure>{ATTR}{}</measure><measure>{}</measure>",
            note(6, ""),
            note(6, "")
        ));
        let s = parse(&x, None).unwrap();
        assert_eq!(s.duration, 3.0);
        assert_eq!(s.warnings.len(), 3);
    }
    #[test]
    fn unsupported_performance_constructs_fail_explicitly() {
        for tag in ["repeat", "grace", "chord", "backup", "ornaments"] {
            let x = score(&format!("<measure>{ATTR}{}<{tag}/></measure>", note(8, "")));
            assert!(parse(&x, None).is_err(), "{tag}");
        }
        let x = score(&format!("<measure>{}</measure>", note(8, "")));
        assert!(parse(&x, None).is_err());
    }
    #[test]
    fn external_doctype_is_not_fetched_and_entities_are_rejected() {
        let x = format!(
            "<!DOCTYPE score-partwise SYSTEM \"https://invalid.invalid/score.dtd\">{}",
            score(&format!("<measure>{ATTR}{}</measure>", note(8, "")))
        );
        assert!(parse(&x, None).is_ok());
        assert!(parse("<!DOCTYPE x [<!ENTITY attack 'x'>]><score-partwise/>", None).is_err());
    }
}

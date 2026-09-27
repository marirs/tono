use super::{beats_until, seconds_at, ImportedMelody};
use crate::music::notes::NoteEvent;
use anyhow::{bail, Context, Result};
use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use std::collections::BTreeMap;

pub(super) fn parse(bytes: &[u8], track: Option<u16>) -> Result<ImportedMelody> {
    let smf = Smf::parse(bytes).context("reading MIDI")?;
    if smf.header.format == Format::Sequential {
        bail!("MIDI type 2 has independent sequences; export type 0 or 1");
    }
    let ppq = match smf.header.timing {
        Timing::Metrical(n) if n.as_int() > 0 => n.as_int() as f64,
        _ => bail!("MIDI import requires metrical timing"),
    };
    let candidates:Vec<usize>=smf.tracks.iter().enumerate().filter(|(_,t)| t.iter().any(|e| matches!(e.kind,TrackEventKind::Midi{message:MidiMessage::NoteOn{vel,..},..} if vel.as_int()>0))).map(|(i,_)|i).collect();
    let selected = match track {
        Some(n) => {
            let i = usize::from(n) - 1;
            if !candidates.contains(&i) {
                bail!(
                    "MIDI track {n} has no notes; note tracks: {:?}",
                    candidates.iter().map(|i| i + 1).collect::<Vec<_>>()
                );
            }
            i
        }
        None if candidates.len() == 1 => candidates[0],
        _ => bail!(
            "choose --melody-track from MIDI note tracks {:?}",
            candidates.iter().map(|i| i + 1).collect::<Vec<_>>()
        ),
    };
    let mut tempo_ticks = BTreeMap::new();
    for t in &smf.tracks {
        let mut tick = 0u64;
        for e in t {
            tick += u64::from(e.delta.as_int());
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = e.kind {
                if us.as_int() == 0 {
                    bail!("MIDI tempo cannot be zero");
                }
                if let Some(old) = tempo_ticks.insert(tick, us.as_int()) {
                    if old != us.as_int() {
                        bail!("conflicting MIDI tempos at tick {tick}");
                    }
                }
            }
        }
    }
    let tempos: Vec<_> = tempo_ticks
        .into_iter()
        .map(|(t, us)| (t as f64 / ppq, us as f64))
        .collect();
    let mut warnings =
        vec!["Imported notes are preserved; their musical correctness is not verified.".into()];
    if tempos.is_empty() {
        warnings.push("MIDI has no tempo event; MIDI default 120 bpm used.".into());
    }
    let mut tick = 0u64;
    let mut active = BTreeMap::new();
    let mut notes = Vec::new();
    for e in &smf.tracks[selected] {
        tick += u64::from(e.delta.as_int());
        if let TrackEventKind::Midi { channel, message } = e.kind {
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    if channel.as_int() == 9 {
                        bail!("selected MIDI track contains percussion; choose a pitched melody");
                    }
                    if active
                        .insert((channel.as_int(), key.as_int()), tick)
                        .is_some()
                    {
                        bail!("overlapping MIDI note-ons for the same pitch");
                    }
                }
                MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => {
                    let start = active
                        .remove(&(channel.as_int(), key.as_int()))
                        .context("MIDI note-off without a matching note-on")?;
                    notes.push(NoteEvent {
                        start: seconds_at(start as f64 / ppq, &tempos),
                        end: seconds_at(tick as f64 / ppq, &tempos),
                        midi: key.as_int(),
                        confidence: 1.0,
                    });
                }
                MidiMessage::PitchBend { bend } if bend.as_int() != 8192 => {
                    bail!("MIDI pitch bends need conversion to explicit notes before import")
                }
                MidiMessage::Controller { controller, value }
                    if (controller.as_int() == 64 && value.as_int() >= 64)
                        || controller.as_int() == 66
                        || controller.as_int() == 101
                        || controller.as_int() == 100 =>
                {
                    bail!("MIDI sustain/tuning controls need baking into notes before import")
                }
                _ => {}
            }
        }
    }
    if !active.is_empty() {
        bail!("MIDI contains unterminated notes");
    }
    let duration = seconds_at(tick as f64 / ppq, &tempos);
    let beat_times = beats_until(duration, &tempos);
    Ok(ImportedMelody {
        notes,
        duration,
        beat_times,
        warnings,
        format: "midi",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::{Header, TrackEvent};
    fn event(delta: u32, kind: TrackEventKind<'static>) -> TrackEvent<'static> {
        TrackEvent {
            delta: delta.into(),
            kind,
        }
    }
    fn on(p: u8) -> TrackEventKind<'static> {
        TrackEventKind::Midi {
            channel: 0.into(),
            message: MidiMessage::NoteOn {
                key: p.into(),
                vel: 90.into(),
            },
        }
    }
    fn off(p: u8) -> TrackEventKind<'static> {
        TrackEventKind::Midi {
            channel: 0.into(),
            message: MidiMessage::NoteOff {
                key: p.into(),
                vel: 0.into(),
            },
        }
    }
    fn data(tracks: Vec<Vec<TrackEvent<'static>>>) -> Vec<u8> {
        let s = Smf {
            header: Header::new(Format::Parallel, Timing::Metrical(480.into())),
            tracks,
        };
        let mut b = vec![];
        s.write_std(&mut b).unwrap();
        b
    }
    #[test]
    fn tempo_map_on_conductor_track_preserves_gaps_and_end_rest() {
        let b = data(vec![
            vec![
                event(0, TrackEventKind::Meta(MetaMessage::Tempo(500_000.into()))),
                event(
                    480,
                    TrackEventKind::Meta(MetaMessage::Tempo(1_000_000.into())),
                ),
            ],
            vec![
                event(480, on(60)),
                event(480, off(60)),
                event(480, on(62)),
                event(480, off(62)),
                event(480, TrackEventKind::Meta(MetaMessage::EndOfTrack)),
            ],
        ]);
        let s = parse(&b, None).unwrap();
        assert_eq!(s.notes[0].start, 0.5);
        assert_eq!(s.notes[0].end, 1.5);
        assert_eq!(s.notes[1].start, 2.5);
        assert_eq!(s.duration, 4.5);
        assert_eq!(s.beat_times, vec![0.0, 0.5, 1.5, 2.5, 3.5]);
    }
    #[test]
    fn multiple_note_tracks_require_selection_and_missing_off_fails() {
        let b = data(vec![
            vec![event(0, on(60)), event(480, off(60))],
            vec![event(0, on(64)), event(480, off(64))],
        ]);
        assert!(parse(&b, None).is_err());
        assert_eq!(parse(&b, Some(2)).unwrap().notes[0].midi, 64);
        assert!(parse(&data(vec![vec![event(0, on(60))]]), None).is_err());
    }
    #[test]
    fn rejects_pitch_bend_instead_of_silently_changing_song() {
        let b = data(vec![vec![
            event(0, on(60)),
            event(
                1,
                TrackEventKind::Midi {
                    channel: 0.into(),
                    message: MidiMessage::PitchBend {
                        bend: midly::PitchBend(9000.into()),
                    },
                },
            ),
            event(480, off(60)),
        ]]);
        assert!(parse(&b, None).is_err());
    }
}

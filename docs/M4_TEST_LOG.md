# M4 test log

Spec M4: play a real clip on the AE-01 using only the visual fingering, and
record what went wrong. Fix those before adding features.

## 1. Prepare the clip

```bash
tono /path/to/clip.mp4 ./tono-m4.mp4 --instrument ae01
# Optional slower practice copy:
tono /path/to/clip.mp4 ./tono-m4-slow.mp4 --instrument ae01 --tempo-scale 0.75 --metronome both
```

MP3 and other supported audio/video inputs also work. Open `tono-m4.mp4` to
practice. Supporting audio and JSON are in `tono-m4.tono/`; add `--keep-work`
when diagnostic files are needed (`work/cleanup_decisions.json` lists every
dropped, merged, re-pitched or slide-absorbed note with its reason). Range
fitting never changes melody intervals: a whole-octave shift (which also
transposes the backing) is used when needed, otherwise Tono reports the
out-of-range notes. Individual octave folding is disabled.
`tono doctor --ml` is an optional troubleshooting check, not a required step.

| Field | Value |
|---|---|
| Date | |
| Clip (file, length) | |
| Command used | |
| Selected region (project.json) | |
| Warnings printed | |
| Octave shift / range policy | |

A four-count now precedes the song; use `--count-in 0` to disable it.
Hand labels default to upper right/lower left; use `--upper-hand left` if needed.

## 2. Before playing: listen and look

| Check | OK? | Notes |
|---|---|---|
| Region starts/ends at the song, no talking included | | |
| `backing.wav`: vocal/lead gone or clearly reduced, accompaniment intact | | |
| `lead.wav`: the melody you expect | | |
| `notes.json` roughly matches the melody you hear | | |
| Metronome flashes/clicks sit on the beat | | |
| Slowed copy: backing still lines up with the NOW changes | | |
| No notes during instrumental intro/solo sections (bleed removed) | | |
| Short notes that remain are real (ornaments, quick syllables), not wobble | | |
| Merged notes: a wavering sung tone shown as one note is the right pitch | | |

## 2b. Following the NOW panel

| Check | OK? | Notes |
|---|---|---|
| The NEXT CHANGE box and NOW rings give enough warning before each change | | |
| LIFT/PRESS key names match the keys you actually move | | |
| REPEAT cue + onset flash make repeated notes clear enough to re-tongue | | |
| Countdown bar turning green matches when you need to move | | |
| Count-in / GET READY gives enough time for the first note | | |

## 3. Play along: problems found

One row per problem. Types: transition (wrong note change), timing (too
fast / late / early), transcription (wrong or missing or extra note),
fingering (diagram fingering does not produce the note).

| # | Time in practice.mp4 | Type | What happened | Expected |
|---|---|---|---|---|
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |

## 4. Fingering table check (all 39 before `verified: true`)

Generated from `instruments/ae01.json` (Roland Recorder chart). Keys:
1-3 left hand, # and b small keys, 4-7 right hand. Play each on the AE-01
(Recorder fingering, transpose C) and compare with a tuner.

| MIDI | Note | Keys | Sounds correct? | Notes |
|---|---|---|---|---|
| 59 | B3 | 1 2 3 b 4 5 6 7 | | |
| 60 | C4 | 1 2 3 4 5 6 7 | | |
| 61 | C#4 | 1 2 3 # 4 5 6 7 | | |
| 62 | D4 | 1 2 3 4 5 6 | | |
| 63 | D#4 | 1 2 3 # 4 5 6 | | |
| 64 | E4 | 1 2 3 4 5 | | |
| 65 | F4 | 1 2 3 4 | | |
| 66 | F#4 | 1 2 3 # 4 | | |
| 67 | G4 | 1 2 3 | | |
| 68 | G#4 | 1 2 3 # | | |
| 69 | A4 | 1 2 | | |
| 70 | A#4 | 1 2 # | | |
| 71 | B4 | 1 | | |
| 72 | C5 | 2 | | |
| 73 | C#5 | (none) | | |

Open questions to settle on the instrument:

- Is Recorder mode the factory default?
- Do the octave UP/DOWN keys act while held, or latch?
- Where the chart shows two fingerings, the first is used: does it play cleanly?

Only when every row above is confirmed: set `"verified": true` in
`instruments/ae01.json` (the video caution banner then disappears).

The table above covers the 15 neutral fingerings. Also verify every octave-down
note MIDI 47–58 and every octave-up note MIDI 74–85 using the current
`instruments/ae01.json`. Check the rear UP/DOWN cues and transitions; all 39
primary fingerings must be confirmed before setting `verified: true`.

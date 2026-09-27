# Instrument profiles

The JSON files in the root `instruments/` directory are Tono runtime data, not prompts or agent skills. Select one
with `--instrument <profile>` (see the table below). No ML work is needed to load and
validate a profile; invalid or missing data fails before processing the source.

| Profile | Diagram | Sounding MIDI coverage | Required setup |
| --- | --- | --- | --- |
| ae01 | Vertical recorder controls | 47–85 | Recorder, transpose 0 |
| ae05 | Vertical sax controls | 46–85 | Sax, transpose 0, tone octave shift 0 |
| ae10 | Vertical sax controls | 34–97 | Sax, transpose 0, tone octave shift 0, Oct Key OCT2 |
| ae20 | Vertical sax controls | 34–97 | Sax, transpose 0, tone octave shift 0, Octave Key Oct2 |
| guitar | Horizontal fretboard | 40–83 | Six strings, E2 A2 D3 G3 B3 E4, no capo, frets 0–19 |
| guitar-bass | Horizontal four-string fretboard | 28–62 | E1 A1 D2 G2, no capo, frets 0–19 |
| guitar-bass-5string | Horizontal five-string fretboard | 23–62 | B0 E1 A1 D2 G2, no capo, frets 0–19 |
| piano | Full 88-key overview + active-key detail | 21–108 | A0–C8, transpose 0 |
| keyboard-76 | Full 76-key overview + active-key detail | 28–103 | E1–G7, transpose 0 |
| keyboard-61 | Full 61-key overview + active-key detail | 36–96 | C2–C7, transpose 0 |

AE-01 keeps the user's right-hand-upper label preference. Other winds default
to the manuals' left-hand-upper grip. `--upper-hand right|left` changes labels
only; it never mirrors controls or alters fingerings. It does not affect guitar.

## Data and source checks

Each file contains primary `fingerings`, preserved `alternatives`, verification
status, required settings, and source URLs. Wind profiles also contain physical
control IDs and schematic coordinates under `layout_keys` (AE-01 retains its
existing dedicated drawing). Every referenced key must exist in that model's
diagram. Unknown instruments, key IDs and malformed guitar positions are errors.

The new sax tables were visually transcribed from the **Bb3–C#5 segment** in:

- [AE-05 manual](https://static.roland.com/assets/media/pdf/AE-05_eng05_W.pdf): chart p13, key layout and octave behavior p6.
- [AE-10 manual](https://static.roland.com/assets/media/pdf/AE-10_eng03_W.pdf): chart PDF p13, layout p4, octave settings p8.
- [AE-20 manual](https://static.roland.com/assets/media/pdf/AE-20_eng01_W.pdf): chart p22, layout and octave controls p6.

Each model's chart was checked independently. Their chosen base patterns match,
but their controls and octave modes differ. The C5 alternative (1 + Tc) is retained.
`build_profiles.py` deterministically expands this documented subset using ±1
(AE-05) or ±2 (AE-10/20) octave controls. Neutral fingerings are preferred, then
smaller octave displacements. Altissimo, customized fingerings and other modes
are not included; these ranges are **profile coverage, not the instrument's
absolute limits**. No AE-01 patterns are reused as sax fingerings.

Guitar mappings follow [Yamaha's tuning/fret guide](https://www.yamaha.com/en/musical_instrument_guide/acoustic_guitar/mechanism/mechanism002.html):
`pitch = open-string MIDI + fret`. All positions through fret 19 are retained.
The primary uses the lowest fret, breaking ties by string number. This is a
simple deterministic position choice, not an optimized concert arrangement.
The renderer shows a five-fret window, string/fret, open strings and plucking
instructions. It plays the transcribed **single-note melody**, not chords or
strumming. Staff pitches are sounding pitches; extreme registers carry octave
marks so ledger lines stay inside the panel.

`verified` remains false until checked on the corresponding physical instrument.
The JSON `required_settings` are shown in the video and terminal; Tono cannot
read or change settings on a connected instrument.

## Editing and regenerating

- `ae01.json` is the canonical AE-01 profile, including chart corrections and
  octave-key alternatives.
- To update new sax data, check the manual first, edit `SAX_BASE` or the
  model-specific layout in `build_profiles.py`, then run `python3 instruments/build_profiles.py` from the repository root.
- The generator also builds `guitar.json` from tuning and fret arithmetic.
- Run `cargo test`; never change key mappings by guesswork or force `verified`.
- Strict range fitting remains the default. A whole-octave shift also shifts
  the backing, and individual folding is disabled.

Examples:

```sh
tono song.mp3 sax.mp4 --instrument ae20 --tempo-scale 0.75 --metronome both
tono song.mp3 guitar.mp4 --instrument guitar --tempo-scale 0.5
tono demo --instrument ae05 --out /tmp/tono-ae05-demo
tono --help
```

## Keyboard and guitar selection

- `--piano`, `--instrument piano`, and `--instrument piano-88` select the exact
  same 88-key profile. Output filenames use the canonical name `piano`.
- `piano-76` and `piano-61` alias `keyboard-76` and `keyboard-61`.
- `guitar-6string`, `guitar-acoustic`, `guitar-classical`, and `guitar-electric`
  alias the existing standard six-string `guitar` profile. Their note positions
  are identical under standard tuning. These aliases do not simulate tone or
  change the current conservative 19-fret coverage.
- `bass`, `bass-4string`, and `guitar-4string` alias `guitar-bass`: specifically
  standard four-string bass, not tenor guitar or ukulele.
- `bass-5string` aliases `guitar-bass-5string`, with a low B string (not high C).
- Smart guitars use the matching string/tuning profile when their physical
  layout is conventional. Proprietary button layouts need their own researched
  profile; there is no generic smart-guitar mapping or device connection.

Keyboard profiles highlight physical keys, including black keys, with press and
release cues. They show a complete keyboard plus an enlarged active region.
They do not assign finger numbers, hand splits, chords or pedal technique.
Grand, upright and digital 88-key pianos share the same note/key mapping.

All note names here use scientific pitch notation (middle C = C4 = MIDI 60).
The [Yamaha P-105 keyboard diagram](https://europe.yamaha.com/files/download/other_assets/6/328146/p105_en_om_a0.pdf)
and [PSR keyboard reference](https://uk.yamaha.com/files/download/other_assets/3/2291233/PSR-E383_reference_manual_En_B0_web.pdf)
use Yamaha's octave numbering, one lower. The 76- and 61-key profiles describe
these common ranges; check your actual keyboard endpoints.
Bass tuning follows [Yamaha's four-/five-string guide](https://hub.yamaha.com/guitars/bass/choosing-the-right-bass-part-1-four-string-or-five-string/).

Selecting a bass profile adapts the selected melody for bass. It does **not**
select or transcribe the source bass stem: `--part vocal|lead` retains its existing
meaning, and no Python ML behavior changes. Demo melodies shift as a whole into
the selected instrument range before synthesizing their guide audio.

```sh
tono song.mp3 --piano
tono song.mp3 --instrument piano-88 --tempo-scale 0.75
tono song.mp3 --instrument keyboard-61
tono song.mp3 --instrument guitar-6string
tono song.mp3 --instrument guitar-bass
tono song.mp3 --instrument bass-5string
```

## AE-BRISA: explicit fingering mode required

```sh
tono song.mp3 --instrument ae-brisa --fingering-mode brisa
tono song.mp3 --instrument ae-brisa --fingering-mode flute
tono demo --instrument ae-brisa --fingering-mode flute
```

`--instrument ae-brisa` requires `--fingering-mode brisa|flute` in direct, `prep`
and `demo` commands. Missing, unknown or incompatible modes fail before ML;
there is no inferred/default mode. Other instruments reject `--fingering-mode`.
The library enforces the same rule via `FingeringTable::load_for_mode`.
Set the matching **Fingering Mode** on the physical instrument; Tono cannot set it.

Both tables live in the flat `instruments/ae-brisa.json` file under `modes`.
`build_profiles.py` retains the reviewed chart patterns and expands only the
Brisa octave-button combinations. No AE-01 or sax fingerings are substituted.

| Mode | Profile coverage | Register control |
| --- | --- | --- |
| brisa | MIDI 60–97 (C4–C#7) | Neither rear key: base; right rear key: +1; both: +2 |
| flute | MIDI 60–96 (C4–C7) | Both breath holes for low register; cover lower hole and blow upper for middle/high |

In Flute mode the rear keys are performance keys, not octave switches. The
horizontal diagram shows their actual pressed states and a separate breath cue.
C5/C#5 have both low-register and upper-hole charted fingerings: the first charted
(low-register) version is primary; alternatives remain in JSON. The high-register
fingerings are independently charted, not derived by shifting low-register keys.
Uncharted substitutes and Brisa trill-derived range extensions are excluded.
Trumpet, Left and Right hardware modes are not supported by Tono yet.

Provenance: [Roland Fingering Chart, pp. 1–2](https://static.roland.com/assets/media/pdf/AE-BRISA_Fingering_Chart_multi01_W.pdf),
[rear-key operations](https://static.roland.com/manuals/ae-brisa_reference/en-US/334327947338651915.html),
[panel descriptions](https://static.roland.com/manuals/ae-brisa_reference/en-US/334329099336039563.html).
Key IDs 1–6 identify the main chart keys from the head joint; T1–T3 identify the
three small levers in chart order. Rear L/R follows the chart inset. `breath_both`
and `breath_upper` are explicit performance cues, not physical keys.

Use factory key/breath mappings, transpose 0 and tone octave 0. Custom tone
assignments can change sounding pitch. `verified` remains false until tested on
a physical Brisa. Videos show the mode/setup, and project/fingering JSON records
`fingering_mode`. Automatic output names include the mode, e.g.
`song_ae-brisa_flute_YYYYMMDD.mp4`, to keep the two arrangements separate.

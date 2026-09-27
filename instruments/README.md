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
| yds120 | Vertical sax controls | 57–90 | Factory fingering, voice transposition 0 (e.g. C.01), no added pitch/octave shift |
| yds150 | Vertical sax controls | 57–90 | Factory fingering, voice transposition 0 (e.g. C.01), no added pitch/octave shift |
| guitar | Horizontal fretboard | 40–83 | Six strings, E2 A2 D3 G3 B3 E4, no capo, frets 0–19 |
| guitar-bass | Horizontal four-string fretboard | 28–62 | E1 A1 D2 G2, no capo, frets 0–19 |
| guitar-bass-5string | Horizontal five-string fretboard | 23–62 | B0 E1 A1 D2 G2, no capo, frets 0–19 |
| piano | Full 88-key overview + active-key detail | 21–108 | A0–C8, transpose 0 |
| keyboard-76 | Full 76-key overview + active-key detail | 28–103 | E1–G7, transpose 0 |
| keyboard-61 | Full 61-key overview + active-key detail | 36–96 | C2–C7, transpose 0 |
| ukulele | Horizontal four-string fretboard | 60–81 | High G: G4 C4 E4 A4, no capo, frets 0–12 |
| ukulele-low-g | Horizontal four-string fretboard | 55–81 | G3 C4 E4 A4, no capo, frets 0–12 |
| ukulele-baritone | Horizontal four-string fretboard | 50–76 | D3 G3 B3 E4, no capo, frets 0–12 |
| recorder-baroque | Vertical holes and thumb vent | 72–98 | Soprano in C, Baroque, double holes 6/7 |
| recorder-german | Vertical holes and thumb vent | 72–98 | Soprano in C, German, double holes 6/7 |
| flute | Horizontal keys and register cues | 60–96 | Concert C flute, Boehm system, C footjoint |
| flute-bfoot | Horizontal keys and register cues | 59–96 | Concert C flute, Boehm system, B footjoint |
| violin | Horizontal fretless fingerboard | 55–83 | G3 D4 A4 E5, first position |

AE-01 keeps the user's right-hand-upper label preference. Other winds default
to the manuals' left-hand-upper grip. `--upper-hand right|left` changes labels
only; it never mirrors controls or alters fingerings. It does not affect guitar.

## Data and source checks

Each file contains primary `fingerings`, preserved `alternatives`, verification
status, required settings, and source URLs. Wind profiles also contain physical
control IDs and schematic coordinates under `layout_keys` (AE-01 retains its
existing dedicated drawing). Every referenced key must exist in that model's
diagram. Unknown instruments, key IDs and malformed guitar positions are errors.

The Roland sax tables were visually transcribed from the **Bb3–C#5 segment** in:

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
  the backing, and individual folding is disabled in both the CLI and the
  library (a frontend cannot fold notes either).

## Practice cues per instrument

The NOW panel names the next change using each profile's own data, so cue
text is only as good as the profile's control labels:

| Profiles | Next-change cue | Repeated note |
| --- | --- | --- |
| Winds with `layout_keys` | `LIFT <labels> · PRESS <labels>` from `layout_keys[].label`; more than 4 keys are counted | `SAME KEYS - RE-TONGUE` |
| AE-01 (dedicated drawing) | Printed key numbers: 1-3, 4-7, `#`, `b`, `OCT UP/DOWN` | `SAME KEYS - RE-TONGUE` |
| Guitar, bass, ukulele | `STRING n - FRET m` / `STRING n OPEN` from `s<n>_f<m>` | `SAME NOTE - PLAY AGAIN` |
| Violin | Named string with low/high finger placement, e.g. `A STRING - LOW 1`, or `D STRING OPEN` | `SAME NOTE - PLAY AGAIN` |
| Piano and keyboards | `KEY <note>` | `SAME KEY - RELEASE AND PLAY AGAIN` |

Flute `register_*` markers produce air/register instructions, not PRESS/LIFT.
Brisa breath markers describe which hole(s) to blow through; recorder holes use
COVER/UNCOVER and thumb states use SEAL, OPEN or VENT 1/4. These cues describe
technique without changing the charted fingering. Long horizontal cues wrap so
they stay above the diagram.

Wind-key rings on NOW confirm the change just made for the first 0.35 s
of a note (at most half the time to the next note), then show the next change.
Fretted and violin diagrams keep the current placement and arrival cues; the
NEXT text above NOW describes the upcoming position.
Renaming a `layout_keys` label changes the cue text; IDs stay the contract.

Examples:

```sh
tono song.mp3 --instrument ae20 --tempo-scale 0.75 --metronome both
tono song.mp3 --instrument guitar --tempo-scale 0.5
tono song.mp3 --instrument ae05 --practice-dir ~/Movies/Practices
tono --help
```

Output filenames are automatic: `./tono-practices/<song>_<instrument>_<YYYYMMDD>.mp4`.
Use `--practice-dir` (or `-d`) to change the output folder.
`tono demo --instrument ae05` is a separate built-in melody demo, requiring no
input song and writing to `./tono-demo/`.

## Ukulele, soprano recorder and concert flute

```sh
tono song.mp3 --instrument ukulele --tempo-scale 0.75
tono song.mp3 --instrument ukulele-low-g
tono song.mp3 --instrument ukulele-baritone
tono song.mp3 --instrument recorder-baroque
tono song.mp3 --instrument recorder-german
tono song.mp3 --instrument flute
tono song.mp3 --instrument flute-bfoot
```

Ukulele profiles use [Kala's documented tunings](https://kalabrand.com/blogs/home/ukulele-tuning-decoded).
`ukulele-high-g` aliases `ukulele`; choose the profile matching your actual strings
and tuning, rather than the instrument's body size. String 1 (A4, or E4 on baritone)
is at the top of the diagram. High-G tuning is reentrant: string 4 is higher than
string 3. Each profile conservatively covers frets 0–12, preserves alternate
positions, and chooses the lowest fret first. These are single-note melody guides,
not chord or strumming arrangements.

Recorder requires an explicit system in the profile name; `recorder` alone is not
accepted. `recorder-soprano-baroque` and `recorder-soprano-german` are longer aliases.
The profiles cover **sounding C5–D7**, not C4–D6: Yamaha's chart uses an octave-up
treble clef. Hole 0 is the rear thumb; `0_vent` means leave approximately a quarter
of that hole open, not press another key. Holes 6 and 7 each have a larger `a`
and smaller `b` opening; the diagram shows their coverage independently. These
profiles assume double-hole instruments and do not cover alto/tenor recorders.
Sources: Yamaha's [Baroque chart](https://www.yamaha.com/en/musical_instrument_guide/common/images/recorder/fingering_baroque.pdf)
and [German chart](https://www.yamaha.com/en/musical_instrument_guide/common/images/recorder/fingering_german.pdf).
Each table and its alternatives were reviewed independently.

`flute-cfoot` aliases `flute`. Both acoustic flute profiles use Yamaha's
[concert flute chart](https://www.yamaha.com/en/musical_instrument_guide/common/images/flute/fingering.pdf),
with charted alternatives preserved; `flute-bfoot` additionally includes the low B
from the [owner's manual](https://usa.yamaha.com/files/download/other_assets/3/335903/piccolo_flute_en_om_a0.pdf),
printed p16. They are standard Boehm-system concert C flutes, not piccolo, alto,
bansuri or Brisa. Open-hole keys must be sealed fully for the shown fingerings.
The thumb B/Bb levers, trill levers and foot keys are shown separately.
Register cues describe the air/embouchure change: there is no acoustic flute
octave button. A fingering picture alone cannot teach tone production or guarantee
the register; these charts still need instrument and musician verification.

Reviewed acoustic tables live in `build_acoustic_profiles.py`, called by
`python3 instruments/build_profiles.py`. Runtime uses only the flat JSON profiles;
no PDFs, browser access or extra Python packages are required. All remain
`verified: false`; no unsupported notes or octave fingerings are invented.

## Yamaha YDS-120 and YDS-150

```sh
tono song.mp3 --instrument yds120
tono song.mp3 --instrument yds150 --tempo-scale 0.75 --metronome both
```

`yds-120` and `yds-150` are aliases; filenames use `yds120` and `yds150`.
Both profiles use Yamaha's factory fingering chart, covering A3–F#6 (MIDI 57–90)
**with total voice transposition set to 0**. The diagram includes the rear `Oct`
and `Low A` keys and Yamaha's front controls. Charted alternatives are retained,
including side/bis Bb and front-key high E/F. The first charted pattern is primary.
There is no speculative octave expansion or custom-fingering support.

Yamaha's chart uses **written pitches**, while Tono uses sounding pitches.
Factory sax voices transpose: alto −9, soprano −2, tenor −14 and baritone −21
semitones. Those unchanged presets will not match this profile. Use the documented
zero-transposition C.01 voice (Harmonica), or configure a user voice with total
transposition 0 in YDS Controller. Some other C voices also shift octaves, so the
letter C alone is not enough. Keep the analog pitch controller neutral. Tono
shows the required setup but cannot configure or detect the instrument's settings.

Sources, checked independently for both models:

- [YDS-120 Owner's Manual](https://usa.yamaha.com/files/download/other_assets/8/1628388/yds-120_en_om_c0-w.pdf): printed pp. 8–9 (controls), 19 (voice transpositions), 20–21 (fingerings).
- [YDS-150 Owner's Manual](https://usa.yamaha.com/files/download/other_assets/2/1361052/yds-150_en_om_i0w.pdf): printed pp. 8–9, 19–21.
- [YDS Controller: User Voice settings](https://manual.yamaha.com/mi/bo/yds-150/en/yds_controller_en_d0_002.html): per-voice transposition.

`build_profiles.py` keeps reviewed Yamaha patterns separate from Roland data;
only schematic drawing positions are shared. `verified` stays `false` until
the corresponding physical instrument has been checked with the required setup.

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

## Violin

```sh
tono song.mp3 --instrument violin --tempo-scale 0.75 --metronome both
tono demo --instrument violin
```

The four-string profile covers **G3–B5 (MIDI 55–83)** in first position, with
standard G3 D4 A4 E5 tuning. This is beginner profile coverage, not the full
violin range. Acoustic and electric four-string violins use the same profile
when tuned this way; five-string instruments and alternate tunings are excluded.

The fretless diagram shows E, A, D, G from top to bottom, nut on the left.
Finger 0 means an open string; 1–4 mean index, middle, ring and little finger.
LOW/HIGH cues distinguish nearby placements. Dots are approximate pitch guides,
not frets or physical fingerboard markings: the player must listen and adjust
intonation. Only the sounding finger is prescribed, not all supporting fingers.
NOW and NEXT show finger changes and string changes. No bow direction, slur,
vibrato, double stop or higher-position fingering is inferred from note events.

Sources: [Yamaha's first-position guide](https://www.yamaha.com/en/musical_instrument_guide/violin/play/play003.html)
and [Violin Online's first-position chart](https://www.violinonline.com/fingeringchart.html).
The charted +6-semitone placement uses low fourth finger on E/A, high third on
D/G. All documented string alternatives in this subset are retained. The primary
chooses the smallest semitone distance from open, preferring open strings over
fourth fingers; it is not a phrase-optimized fingering arrangement.

`build_acoustic_profiles.py` generates `violin.json`. Position IDs encode
` s<string>_p<semitones above open>_n<finger> ` (without surrounding spaces);
`p` is not a fret number. The loader checks tuning, sounding pitch and the
charted finger assignment for primaries and alternatives. Range fitting preserves
intervals through a whole-melody octave shift or fails; individual folding stays
disabled. `verified` remains false pending physical checks.

## Video title

```sh
tono song.mp3 --instrument ae20 --title "Careless Whisper"
tono "Poove Sempoove.mp3" --instrument ae01
```

`--title` sets the video heading, for example **Practice: Careless Whisper**.
Without it, the input filename without its final extension becomes the title
(**Poove Sempoove** in the second example). The full title is saved in
`project.json`; long video headings are shortened to fit. This also works with
`tono prep`. Titles do not change output filenames. No online recognition or API
key is needed.

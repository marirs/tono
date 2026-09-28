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
| Yamaha YVS-120 (`yvs120`) | Vertical Alto Venova | MIDI 53–77 (F3–F5) | German fingering; sounding pitch; thumb-hole and octave-key cues |
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

Vertical wind diagrams default to the manuals' left-hand-upper, right-hand-lower
grip, including AE-01. `--upper-hand right|left` changes labels
only; it never mirrors controls or alters fingerings. It does not affect guitar.

## Hand-position references

The labels describe the player's hands, not the viewer's left/right. All vertical
wind profiles use left hand nearest the mouthpiece and right hand below it.
The rear octave/vent controls belong to the left thumb; the right thumb supports
the instrument. Explicit `--upper-hand right` is a label override, not a verified
alternate grip or a mirrored fingering profile.

- AE-01: [manual pp. 9–11](https://static.roland.com/assets/media/pdf/AE-01_eng03_W.pdf).
- AE-05: [manual p. 6](https://static.roland.com/assets/media/pdf/AE-05_eng05_W.pdf).
- AE-10: [manual p. 4](https://static.roland.com/assets/media/pdf/AE-10_eng03_W.pdf).
- AE-20: [manual p. 6](https://static.roland.com/assets/media/pdf/AE-20_eng01_W.pdf).
- Yamaha YDS-120 / YDS-150: controls and rear thumb-hook diagrams in their
  linked profile manuals; right thumb supports the lower hook, octave controls
  are at the upper hand.
- Yamaha YVS-120: [Let's Play Venova, printed p. 17](https://data.yamaha.com/files/download/other_assets/0/1259560/venova_yvs-120_en_started_e0.pdf).
- Brisa and concert flute stay horizontal: left hand nearest the blowing end,
  right hand farther along the body. Brisa retains its mode-specific controls;
  [Roland's performance-key diagram](https://static.roland.com/manuals/ae-brisa_reference/en-US/334327947338651915.html)
  is the reference, not an Aerophone sax drawing.
- Recorder uses left upper/right lower. Guitar, bass, ukulele and violin retain
  their string/fingerboard orientation; piano shows low to high keys without
  assigning a hand to each note.

AE-30 is not currently a supported profile; no AE-20 alias is assumed. These
checks concern diagram orientation, not physical verification of every fingering.

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
`song_ae-brisa_flute_YYYYMMDD/practice.mp4`, to keep the two arrangements separate.

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

## Yamaha YVS-120 Alto Venova

```bash
tono song.mp3 --instrument yvs120 --tempo-scale 0.75
tono demo --instrument yvs120 --out /tmp/tono-venova-demo
```

Aliases: `yvs-120`, `alto-venova`. This acoustic instrument is distinct from the
YDS digital saxophones. Its profile uses Yamaha’s **concert-pitch** chart, F3–F5
(MIDI 53–77), so the melody and backing stay in the same pitch system. The staff
shows sounding pitch, not transposed Venova-in-F notation.

The vertical diagram uses Yamaha’s numbered controls, mouthpiece at the top:
left-hand 7/6/5, right-hand 4/3/2/1, rear left-thumb 8. Paired lower keys and the
separate hole and 4A/4B controls at 4 are shown individually. At thumb 8,
partly covering the hole (3/4, key released) differs from sealing it and pressing
the octave key. Charted alternatives are retained. Breath and embouchure still
matter, especially for chromatic notes; the diagram cannot guarantee intonation.

Source: [Yamaha YVS-120 Let’s Play Venova](https://data.yamaha.com/files/download/other_assets/0/1259560/venova_yvs-120_en_started_e0.pdf),
printed pp. 102–103 (concert-pitch chart), 17 and 92–93 (controls). The JSON is a
manual chart transcription, not generated from a recorder or sax table.
`verified: false` remains until checked on a physical YVS-120.

## Easy Fingering

```bash
tono song.mp3 --instrument ae01 --easy-fingering --tempo-scale 0.60
```

AE-01 Easy Fingering allows only the six main controls, in combinations from the
existing chart. It excludes sharp/flat, the seventh front key, and octave keys.
It currently covers D4, E4, F4, G4, A4, B4, C5 and the open C♯5 fingering.
All twelve keys and MIDI-valid octave placements are considered. Original pitch
is preferred, then the smallest whole-melody pitch shift (upward wins ties).
The backing in the video is shifted by the same number of semitones. No notes
are individually folded, removed or replaced; rhythm is preserved. Pitch shifting
can change the backing’s timbre. `backing.wav` remains the original separated bed.

If the whole selected passage cannot fit, generation stops with compatible note
spans when available. Those times are **relative to the selected region**, before
slowing; add the region’s source start to use them with `--from` / `--to`.
Spans are suggestions, not automatically selected phrases. Fast passages can
still be hard: use `--tempo-scale` to slow them down.

Easy Fingering is also supported for `ae05`, `ae10`, `ae20`, `yds120`,
`yds150`, and `ae-brisa --fingering-mode brisa`. These profiles allow only main
keys 1–6, with no octave, side, palm or pinky controls. Their charted available
notes are D4, E4, F4, F♯4, G4, A4, B4, C5 and C♯5 (including open fingering).
The Yamaha YDS profiles additionally allow B♭4 through their charted main-key
alternatives (1+4 or 1+5). F♯ is allowed because its charted fingering uses the main keys; this is a
control restriction, not a ban on accidentals. Keep each profile’s documented
instrument/voice transposition settings so the sounding pitches match.

```bash
tono song.mp3 --instrument ae20 --easy-fingering
tono song.mp3 --instrument yds120 --easy-fingering --tempo-scale 0.60
tono song.mp3 --instrument ae-brisa --fingering-mode brisa --easy-fingering
```

Brisa’s `flute` mode is not supported in Easy Fingering: its chart uses rear
performance keys and breath-register cues, requiring a separate beginner policy.
YVS-120 Alto Venova is acoustic and is also not yet supported in Easy Fingering.
These and other unsupported profiles print a warning and continue normally;
Tono never silently changes the selected instrument or fingering mode.
`project.json` and `fingering.json` record `easy_fingering` as `applied`,
`unsupported_fallback` or `not_requested`, plus the total
`backing_transpose_semitones`. `octave_shift` and `semitone_offset` describe its
whole-octave and remaining-semitone components. Successful easy-mode videos are
labelled on screen. Profiles remain unverified until checked on the instrument.

## Pitched pans and piano accordion

These profiles are available in the source build after v0.1.1. The new profiles
remain `verified: false` until checked on the corresponding physical setup.

| CLI profile | Supported setup |
| --- | --- |
| `handpan-d-kurd` / `handpan` | Nine notes: D3 / A3 Bb3 C4 D4 E4 F4 G4 A4. Central D3; A3 near-right, Bb3 near-left; ascending notes alternate around the rim toward the far edge. Other arrangements are not covered. |
| `moodpan` / `mn-10` | MN-10, Handpan tone, factory pitch (A440), pitch effects off. `--pan-style` is mandatory. |
| `accordion-piano-41` / `accordion` | 41-key F3–A6 piano accordion, 8-foot register. Right-hand melody only; low keys toward chin, high keys toward knee. |
| `taiko-1` / `taiko` | Recognized but rejected: percussion generation is not implemented. |
| `spd-20-pro` / `octapad` | Recognized but rejected: percussion generation and melodic kit mappings are not implemented. |

Mood Pan styles: `major`, `minor`, `celtic`, `arabic`, `relax`, `indian`,
`meditation`, `japanese`, `equinox`, `romantic`, `dreamy`, `aegean`.
The JSON stores exact pitches per pad per style, not a guessed generic scale.
Source: [Roland MN-10 manual](https://static.roland.com/assets/media/pdf/MN-10_eng02_W.pdf),
pages 3/9 (layout) and 15 (pad pitches). Special pad, side slaps, user tones,
custom app tuning and pitch-changing effects are outside this profile.

Acoustic handpan pitch source: [Saraz D minor scales](https://www.sarazhandpans.com/handpan-scales/d-minor/),
nine-note Kurd. The drawing is an explicit supported layout, not a universal
manufacturer layout; compare it with your instrument before practising.
Accordion source: [HOHNER keyboard chart, page 1](https://hohner.de/fileadmin/documents/instruments/accordions/chromatic/bravo/bravo-iii-120/hohner-accordions-bravo-iii-120-fingering-chart.pdf).
Bass, chords, bellows direction and button accordions are not represented.

```bash
tono song.mp3 --instrument moodpan --pan-style minor --tempo-scale 0.5
tono melody.musicxml --instrument handpan-d-kurd --backing backing.mp3
tono melody.mid --instrument accordion-piano-41
```

Both pan profiles test every pitch against the actual tuning. They may transpose
the entire melody and backing by one consistent number of semitones (smallest
change first, upward on ties); they fail if no such shift fits. No individual
notes are folded or approximated. Setup and selected style are recorded with
the output; automatic Mood Pan folder names also include the style.

Pan diagrams mean **strike once, then let ring**, not hold a pad throughout the
note. Repeated notes mean strike again. The duration shown is musical timing;
this first version does not prescribe damping or hand/stroke technique.
`--easy-fingering` is not defined for these new profiles and uses the existing
explicit warning followed by normal creation.

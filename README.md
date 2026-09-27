# tono

**Play what you love.**

Tono is a local-first music practice engine that turns music you already have into instrument-specific fingering guides, backing tracks, and guided practice videos.

Bring a song. Pick the part you want to play. Choose an instrument. Tono helps you play it.

**Website:** https://tono.love

---

## Why Tono?

A lot of music-learning software starts with lessons, exercises, scales, and a fixed song catalogue.

Tono starts somewhere else:

> **I love this song. Help me play it.**

The project began with a Roland Aerophone AE-01 and a simple frustration: the official learning experience is useful, but the available music is limited.

Tono makes the *tool* independent of the catalogue.

You bring the music. Tono analyzes it, prepares a backing track, maps the melody to your instrument, and creates a visual practice experience you can follow even if you do not read music yet.

---

## What it does

Tono works toward a pipeline like this:

```text
your audio
    │
    ▼
find the actual song region
    │
    ▼
separate lead / vocal from accompaniment
    │
    ├──────────────► backing track / BGM
    │
    ▼
transcribe the melody
    │
    ▼
clean and time the notes
    │
    ▼
load an instrument profile
    │
    ▼
map notes to fingering / position
    │
    ▼
render a guided practice video
    │
    ▼
practice.mp4 + BGM
```

The generated practice video is meant to be directly usable:

**press play → hear the backing → follow the fingering → play.**

---

## Beginner-first practice

Tono does not assume you already know note names.

The primary view can be the physical action:

```text
              NEXT
       [next fingering]

               ↓

               NOW

      [current fingering]

         PRESS / LIFT

      ━━━━━━━━━━━━━━━
          progress

            ● ○ ○ ○
           metronome
```

Notation and note names can still be shown, but they are not required to get started.

What the NOW panel does so you can follow it without looking elsewhere:

- **Arrival, then preparation.** Right after a note starts, the rings on NOW
  confirm the change you just made (green = pressed, red = lifted). For the
  rest of the note (and any rest before the next one) wind-key rings switch to
  the *next* change, and the **NEXT CHANGE** box names it: `LIFT Eb · PRESS C`
  on winds (AE-01 uses its printed 1-7, #, b), `STRING 2 - FRET 3` on
  fretted instruments, `D STRING - LOW 1` on violin, `KEY D#4` on keyboards.
  Fretted and violin diagrams retain the current placement; their NEXT text
  provides the advance cue.
- **Countdown to the change.** The bar under NOW fills from one note onset to
  the next, rests included, and turns green in the last 0.3 s: move now.
- **Repeated notes.** Same fingering twice gets a `REPEAT - SAME KEYS -
  RE-TONGUE` cue (keyboards: release and play again), and every onset flashes
  a frame around NOW so back-to-back repeats are visible events.
- **Preparation time.** A count-in (default 4 beats, `--count-in 0..4`) shows
  the first fingering before the song; long intros show `GET READY` with the
  first fingering and a countdown to the first sung note. Use
  `--tempo-scale 0.5..2.0` for more time per change; the slowed backing is
  re-aligned (see below).

The idea is to let muscle memory and musical familiarity provide the motivation first. Theory can come later.

---

## Instrument profiles

Tono does not hard-code one instrument.

Each supported instrument has a deterministic runtime profile describing things such as:

- fingering or playing position;
- alternative fingerings / positions;
- playable profile range;
- required instrument settings;
- physical control IDs;
- renderer type;
- source references;
- verification status.

Current profiles (select with `--instrument`):

| Instrument | Renderer | Current profile coverage | Required setup |
| --- | --- | ---: | --- |
| Roland AE-01 (`ae01`) | Recorder-style | MIDI 47–85 | Recorder fingering, transpose 0 |
| Roland AE-05 (`ae05`) | Sax-style | MIDI 46–85 | Sax fingering, transpose 0, tone octave shift 0 |
| Roland AE-10 (`ae10`) | Sax-style | MIDI 34–97 | Sax fingering, transpose 0, tone octave shift 0, Oct Key OCT2 |
| Roland AE-20 (`ae20`) | Sax-style | MIDI 34–97 | Sax fingering, transpose 0, tone octave shift 0, Octave Key Oct2 |
| Yamaha YDS-120 (`yds120`) | Sax-style | MIDI 57–90 | Factory fingering, voice transposition 0 (e.g. C.01), no added pitch/octave shift |
| Yamaha YDS-150 (`yds150`) | Sax-style | MIDI 57–90 | Factory fingering, voice transposition 0 (e.g. C.01), no added pitch/octave shift |
| Roland AE-BRISA (`ae-brisa`), Brisa mode | Horizontal flute | MIDI 60–97 | `--fingering-mode brisa`; matching instrument mode, transpose/tone octave 0, factory key/breath mapping |
| Roland AE-BRISA (`ae-brisa`), Flute mode | Horizontal flute | MIDI 60–96 | `--fingering-mode flute`; matching instrument mode, transpose/tone octave 0, factory key/breath mapping |
| Six-string guitar (`guitar`) | Fretboard | MIDI 40–83 | Standard E2 A2 D3 G3 B3 E4, no capo, frets 0–19 |
| Four-string bass (`guitar-bass`) | Fretboard | MIDI 28–62 | Standard E1 A1 D2 G2, no capo, frets 0–19 |
| Five-string bass (`guitar-bass-5string`) | Fretboard | MIDI 23–62 | Standard B0 E1 A1 D2 G2, no capo, frets 0–19 |
| Full-size piano (`piano` / `piano-88`) | 88-key keyboard | MIDI 21–108 | Transpose 0, middle C = C4 |
| 76-key keyboard (`keyboard-76`) | 76-key keyboard | MIDI 28–103 | Transpose 0, middle C = C4 |
| 61-key keyboard (`keyboard-61`) | 61-key keyboard | MIDI 36–96 | Transpose 0, middle C = C4 |
| Ukulele (`ukulele` / `ukulele-high-g`) | Fretboard | MIDI 60–81 | G4 C4 E4 A4, no capo, frets 0–12 |
| Low-G ukulele (`ukulele-low-g`) | Fretboard | MIDI 55–81 | G3 C4 E4 A4, no capo, frets 0–12 |
| Baritone ukulele (`ukulele-baritone`) | Fretboard | MIDI 50–76 | D3 G3 B3 E4, no capo, frets 0–12 |
| Soprano recorder (`recorder-baroque`) | Vertical holes | MIDI 72–98 | Baroque system, double holes 6/7 |
| Soprano recorder (`recorder-german`) | Vertical holes | MIDI 72–98 | German system, double holes 6/7 |
| Concert flute (`flute` / `flute-cfoot`) | Horizontal keys | MIDI 60–96 | Standard Boehm system, C footjoint |
| Concert flute (`flute-bfoot`) | Horizontal keys | MIDI 59–96 | Standard Boehm system, B footjoint |
| Violin (`violin`) | Fretless fingerboard | MIDI 55–83 | G3 D4 A4 E5, first position |

These are **profile coverage ranges**, not claims about the absolute limits of the physical instruments.

`--piano` is shorthand for `--instrument piano`. Guitar profiles show a single-note
melody; selecting bass does not extract the song's bass part. Keyboard profiles
show which keys to play, without finger numbers or a two-hand arrangement.

For Yamaha YDS profiles, the factory alto/tenor/soprano/baritone voices transpose.
Use a voice with total transposition 0 to match the video and backing;
[the instrument guide](instruments/README.md#yamaha-yds-120-and-yds-150) explains the setup.

See [the instrument guide](instruments/README.md) for aliases, sources and verification details.

The long-term goal is simple: adding an instrument should mostly mean adding a reliable profile and a suitable renderer, not rewriting the transcription engine.

---

## Backing tracks

For a vocal-melody practice session, the basic idea is:

```text
Original
   │
   ▼
Stem separation
   ├── vocal / lead ─────► transcription
   ├── drums
   ├── bass
   └── other
          │
          ▼
        BGM
```

The selected lead is reduced or removed from the accompaniment where the separation model allows it.

The same BGM is muxed into the generated practice video so that `practice.mp4` can be played by itself.

---

## Melody cleanup

Transcription is polyphonic and level-blind: it turns faint separation bleed,
vibrato, slides and consonants into notes. Tono measures a monophonic pitch
track (pYIN) on the separated lead and uses it as evidence, deterministically
in Rust:

- **Bleed** — notes where the lead stem is more than 35 dB below its loud
  sung passages are dropped (on a real test song the whole instrumental intro
  produced 60+ such "notes" around -70 dB, including a bogus low E♭2 that
  forced an octave transposition).
- **Unpitched noise** — notes up to 120 ms with under 20 % voiced frames.
- **One wavering tone split in two** — neighbours with the same sung pitch
  centre and no new attack are joined, at the transcribed pitch nearest that
  centre.
- **Slides** — a short pitch sweep without its own attack is absorbed into the
  note it slides into.
- **Clear wrong pitch** — a steady, well-voiced note sung ≥0.8 semitones away
  is re-pitched to the sung semitone (octave disagreements are left alone).

Short notes are **not** deleted for being short: a short note with steady
pitch or its own attack is kept as an ornament, and repeated notes with a real
attack stay separate. `--keep-work` writes every decision with its reason to
`work/cleanup_decisions.json`; `project.json` records the counts.

---

## Song-region detection

Reels, shorts, and downloaded clips may contain:

- spoken introductions;
- commentary;
- silence;
- applause;
- spoken endings.

Tono should find the actual musical region before transcription rather than attempting to convert speech into melody.

Manual start/end boundaries can override automatic detection.

---

## Range handling

Tono does **not** individually octave-fold notes just to force them into an instrument profile.

That would change the melody.

The intended behavior is:

1. clean obvious transcription errors;
2. determine the real melody range;
3. check it against the selected instrument profile;
4. try a whole-phrase octave shift / transposition when appropriate;
5. shift the backing consistently if the whole phrase is transposed;
6. otherwise report unsupported pitches.

The aim is to keep generated practice material musically honest.

Individual octave folding is not implemented anywhere: the CLI and the library
both reject `--range-policy fold`. A whole-melody octave shift transposes the
practice backing by the same amount (ffmpeg `asetrate` + `atempo`), and the
video says so.

Slowed or transposed backing is **re-aligned automatically**: ffmpeg's
time-stretch moves audio slightly early (measured about 15 ms at 0.75x and
35 ms at 0.5x), so Tono runs the same filter chain on a click train, measures
the offset and compensates it (`backing_stretch_offset_ms` in `project.json`).

---

## Usage

Give Tono the input file and instrument; the output filename is automatic.
Videos go to `./tono-practices/<song>_<instrument>_<YYYYMMDD>.mp4`.
Use `--practice-dir DIRECTORY` (or `-d DIRECTORY`) to choose another folder.

AE-01:

```sh
tono song.mp3 --instrument ae01
```

Slow it down:

```sh
tono song.mp3 \
  --instrument ae01 \
  --tempo-scale 0.70
```

AE-20 with metronome:

```sh
tono song.mp3 \
  --instrument ae20 \
  --tempo-scale 0.75 \
  --metronome both
```

Guitar:

```sh
tono song.mp3 \
  --instrument guitar \
  --tempo-scale 0.50
```

Built-in melody demo (no input song; writes to `./tono-demo/`):

```sh
tono demo --instrument ae05
```

Help:

```sh
tono --help
```

---

## Examples

Each run starts with a colored title, the installed version and the resolved paths:

```text
Tono - Play what you love.
v0.1.0
Input: /path/to/song.mp3
Output: /path/to/tono-practices/song_ae01_YYYYMMDD.mp4
```

The version comes from Cargo at build time. Redirected output stays plain text;
set `NO_COLOR=1` to disable the title color in a terminal.

Create an AE-01 practice video with an automatic filename:

```sh
tono ~/Downloads/song.mp3 --instrument ae01
```

The video goes to `./tono-practices/song_ae01_YYYYMMDD.mp4`.
Use `-d` (or `--practice-dir`) to choose another folder:

```sh
tono ~/Downloads/song.mp3 --instrument ae20 -d ~/Movies/Practices
```

Practice a short section from a video at half speed, with a four-count start and an audible/visual metronome:

```sh
tono ~/Downloads/reel.mp4 --instrument ae01 \
  --from 00:12 --to 00:30 \
  --tempo-scale 0.5 --count-in 4 --metronome both
```

Choose a keyboard or bass guitar:

```sh
tono song.mp3 --piano                       # Full-size 88-key piano
tono song.mp3 --instrument keyboard-61      # 61-key keyboard
tono song.mp3 --instrument guitar-bass      # Four-string bass
tono song.mp3 --instrument guitar-bass-5string
```

Practice with a Yamaha digital saxophone (set up the voice as described above):

```sh
tono song.mp3 --instrument yds120 --tempo-scale 0.75
tono song.mp3 --instrument yds150 --metronome both
```

Ukulele, recorder or acoustic flute:

```sh
tono song.mp3 --instrument ukulele --tempo-scale 0.75
tono song.mp3 --instrument ukulele-low-g
tono song.mp3 --instrument ukulele-baritone
tono song.mp3 --instrument recorder-baroque   # Choose your recorder's system
tono song.mp3 --instrument recorder-german
tono song.mp3 --instrument flute              # C footjoint
tono song.mp3 --instrument flute-bfoot        # B footjoint
```

Ukulele shows individual melody notes, not chords or strumming. Recorder cues
include split holes and thumb venting; flute cues include air/embouchure register.

Violin, with first-position string and finger guidance:

```sh
tono song.mp3 --instrument violin --tempo-scale 0.75 --metronome both
```

Shows open strings, finger numbers and low/high placements. Bow directions and
slurs are not prescribed; see [the violin guide](instruments/README.md#violin).

For Aerophone Brisa, specify the fingering mode set on your instrument:

```sh
tono song.mp3 --instrument ae-brisa --fingering-mode brisa
tono song.mp3 --instrument ae-brisa --fingering-mode flute
```

---

## Architecture

Tono is intentionally local-first.

### Rust

Rust owns the deterministic application/core work:

- CLI;
- orchestration;
- instrument-profile loading and validation;
- timeline logic;
- range handling;
- fingering / position mapping;
- practice rendering;
- FFmpeg orchestration.

### Python

Python is kept mainly at the music-ML boundary, where the strongest existing ecosystem currently lives:

- source / stem separation;
- melody and pitch transcription;
- audio analysis.

ML output is normalized into deterministic data before it reaches an instrument renderer.

### FFmpeg

FFmpeg handles:

- audio extraction;
- encoding;
- backing-track mixing;
- final MP4 muxing.

### macOS

A native macOS application is planned once the core practice pipeline is reliable.

The Mac app can eventually add:

- drag-and-drop import;
- waveform section selection;
- instrument selection;
- Practice mode;
- Perform mode;
- live MIDI;
- metronome controls;
- audio/video recording;
- local library management.

---

## Verification matters

Instrument profiles can be sourced from official manufacturer documentation while still remaining:

```json
"verified": false
```

until somebody checks them on the physical instrument.

Please do not mark a profile verified by assumption and do not guess missing mappings.

For new profiles, include sources whenever possible.

---

## Adding another instrument

Conceptually, Tono wants this boundary:

```text
transcribed note
      │
      ▼
instrument profile
      │
      ├── AE-01 fingering
      ├── AE-05 fingering
      ├── AE-10 fingering
      ├── AE-20 fingering
      ├── guitar string / fret
      ├── piano key
      └── ...
```

Contributions for additional instruments, verified fingering data, renderers, transcription improvements, and practice UX are welcome.

---

## Music and copyright

Tono is a **tool**, not a music catalogue.

The repository does not need to contain copyrighted songs, commercial backing tracks, or a hosted song library.

Users bring their own practice material and are responsible for using that material in ways permitted by applicable rights and licenses.

---

## Project status

Tono is experimental and under active development.

Current priorities include:

- better melody transcription (real-song tuning of the cleanup thresholds);
- better BGM separation;
- physical verification of instrument profiles;
- easier fingering transitions;
- additional instruments;
- a native macOS application.

Expect interfaces and file formats to change while the core workflow is being proven.

---

## Contributing

Issues, profile corrections, new instrument profiles, renderer improvements, and code contributions are welcome.

When contributing instrument data:

- cite the source;
- do not guess fingerings;
- retain alternative fingerings when useful;
- keep `verified: false` until physically checked;
- run the test suite before submitting changes.

---

## License

Tono source code is licensed under the **Mozilla Public License 2.0 (MPL-2.0)**.

MPL-2.0 uses **file-level copyleft**: if someone distributes modified MPL-covered source files, those files remain under MPL-2.0, while separate files in a larger work can use other licenses, including proprietary licenses.

See [`LICENSE.txt`](LICENSE.txt) for the complete license text.

The **Tono** name, `tono.love`, logos, and other branding are not licensed for use by the MPL-2.0 software license.

---

## Tono

**Play what you love.**

https://tono.love

### Aerophone Brisa

AE-BRISA requires an explicit mode matching the physical instrument:

```sh
tono song.mp3 --instrument ae-brisa --fingering-mode brisa
tono song.mp3 --instrument ae-brisa --fingering-mode flute
```

Omitting `--fingering-mode` fails before processing. The horizontal diagram
shows the mode-specific rear keys and, for Flute mode, breath-hole instructions.
See [instrument profiles](instruments/README.md) for coverage, setup and sources.

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

## Development status

The source-to-video CLI is implemented and has been exercised on real music.
The remaining v0 acceptance work is a human play-through and physical fingering
verification; see [the M4 checklist](docs/M4_TEST_LOG.md).

Pitch evidence is checked at the ML boundary before cleanup. Malformed evidence
fails clearly; older analysis data without a pitch track still uses duration
rules. Practice cues distinguish physical keys from flute air/register changes,
Brisa breath holes and recorder thumb venting.

Known limits: cleanup thresholds need more songs and musician feedback;
`--part lead` assumes Demucs's `other` stem and stays low-confidence. macOS is the
tested platform; Windows/Linux packaging and full-pipeline validation remain
future work. A GUI, automatic song recognition and a Rust ML rewrite are separate
projects, not prerequisites for this CLI's v0 play-through.

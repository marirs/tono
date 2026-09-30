# tono

**Play what you love.**

[![Check](https://github.com/marirs/tono/actions/workflows/ci.yml/badge.svg)](https://github.com/marirs/tono/actions/workflows/ci.yml)
[![Release packages](https://github.com/marirs/tono/actions/workflows/release.yml/badge.svg)](https://github.com/marirs/tono/actions/workflows/release.yml)


Tono is a local-first music practice engine that turns music you already have into instrument-specific fingering guides, backing tracks, and guided practice videos.

Bring a song. Pick the part you want to play. Choose an instrument. Tono helps you play it.

**Website:** https://tono.love

---

## Downloads

Portable release packaging is in development: a small executable that sets up
its audio/ML tools automatically, plus an offline ZIP with everything included.
Users will not need to install Python or FFmpeg themselves. The same setup API
will serve a future GUI. No public portable release is published yet.

See [download options and platform status](docs/DISTRIBUTION.md). Existing source
installation and CLI options continue to work.

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
| Yamaha YVS-120 (`yvs120`) | Vertical Alto Venova | MIDI 53–77 (F3–F5) | German fingering; sounding pitch; thumb-hole and octave-key cues |
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
| Handpan (`handpan-d-kurd`, alias `handpan`) | Top-down numbered tone fields | D3 / A3 Bb3 C4 D4 E4 F4 G4 A4 | Exact 9-note tuning and depicted layout only |
| Roland Mood Pan (`moodpan`, alias `mn-10`) | Top-down nine pads | Depends on selected style | Required `--pan-style`; Handpan tone, factory pitch |
| Piano accordion (`accordion-piano-41`, alias `accordion`) | Upright right-hand keyboard | F3–A6 (MIDI 53–93) | 41 keys, 8-foot register; melody only |


These are **profile coverage ranges**, not claims about the absolute limits of the physical instruments.

`--piano` is shorthand for `--instrument piano`. Guitar profiles show a single-note
melody; selecting bass does not extract the song's bass part. Keyboard profiles
show which keys to play, without finger numbers or a two-hand arrangement.

For Yamaha YDS profiles, the factory alto/tenor/soprano/baritone voices transpose.
Use a voice with total transposition 0 to match the video and backing;
[the instrument guide](instruments/README.md#yamaha-yds-120-and-yds-150) explains the setup.

See [the instrument guide](instruments/README.md) for aliases, sources and verification details.

The long-term goal is simple: adding an instrument should mostly mean adding a reliable profile and a suitable renderer, not rewriting the transcription engine.


### Aerophone Brisa

AE-BRISA requires an explicit mode matching the physical instrument:

```sh
tono song.mp3 --instrument ae-brisa --fingering-mode brisa
tono song.mp3 --instrument ae-brisa --fingering-mode flute
```

Omitting `--fingering-mode` fails before processing. The horizontal diagram
shows the mode-specific rear keys and, for Flute mode, breath-hole instructions.
See [instrument profiles](instruments/README.md) for coverage, setup and sources.

### Yamaha YVS-120 Alto Venova

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

### Handpan, Mood Pan and accordion

```bash
tono song.mp3 --instrument moodpan --pan-style minor --tempo-scale 0.5
tono song.mp3 --instrument handpan-d-kurd --part lead
tono melody.mid --instrument accordion --backing backing.mp3
```

Mood Pan requires a style matching its knob: `major`, `minor`, `celtic`, `arabic`,
`relax`, `indian`, `meditation`, `japanese`, `equinox`, `romantic`, `dreamy`, or
`aegean`. Use **Handpan tone, factory pitch and no pitch-shifting effects**.
Other tones, app-customized tunings and special-pad effects are not covered.

Acoustic handpan support is specifically the nine-note D Kurd tuning above,
with the player-view layout shown in the sheet. It does not cover every handpan.
Pan fitting tries a single transposition for the entire melody and backing. If
no transposition fits every available tone field, generation stops; no notes
are folded, replaced or dropped. The sheet and video show strike cues.

Accordion support covers the **right-hand melody only** on a 41-key piano
accordion. Button/diatonic accordions, bass/chord parts and bellows direction
are not supported.

`--instrument taiko-1` and `--instrument spd-20-pro` are recognized but stop
with an explicit error before processing. Percussion practice and OCTAPAD
melodic kit mappings are not implemented. There is no silent fallback to another
part. All new profiles remain unverified on physical instruments.

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
- **Overlapping candidates** — where transcription candidates overlap, the one
  the lead stem measurably carries wins over a more confident one (≥ ~70 ms,
  steady voiced pitch within 0.25 semitone; octave-apart choices need ≥85 %
  voiced frames). Only existing candidates can win.
- **Register outliers** — a short (≤150 ms), unattacked note an octave or more
  from both neighbours, unconfirmed by pitch evidence, is dropped so one stray
  event cannot force a whole-song octave transposition.

Pitch evidence is not used for events shorter than about three pitch-track hops
(~70 ms): the tracker's ~93 ms window cannot resolve them.

After noise filtering, short notes with steady matching pitch or a strong attack
survive the duration cutoff. A short semitone change is not automatically
vibrato: when pitch evidence is available, it must support that interpretation.
Repeated notes stay separate when they have a strong attack or a measured silent
gap. `--keep-work` writes evidence corrections, vibrato absorption and short-note
removals to `work/cleanup_decisions.json`; `project.json` records the counts.

See [melody quality checks](docs/MELODY_QUALITY.md) for the real-clip results and
remaining limitations.

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


### Easy Fingering

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

---

## Usage

Give Tono the input file and instrument; the output filename is automatic.
Videos go to `./tono-practices/<song>_<instrument>_<YYYYMMDD>/practice.mp4`.
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


### Video title

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

---

AE-01 and other vertical wind diagrams use **left hand above right hand** by
default, with the left thumb on the rear octave/vent controls. Brisa and flute
keep their horizontal layouts. See [hand-position references](instruments/README.md#hand-position-references).

## Practice output folder

Each run creates one folder, such as `tono-practices/song_ae01_YYYYMMDD/`:

```text
practice.html       # open this to review and play
practice.mp4
backing.mp3         # imported-melody workflows
backing.wav
notes.json
fingering.json
project.json
```

`--practice-dir` / `-d` chooses the parent folder. An explicit output name such as
`tono song.mp3 custom.mp4 --instrument ae01` selects `custom/`, containing
`practice.html` and `practice.mp4`. Reruns replace that practice folder with a
recovery backup. Older MP4 + `.tono` outputs are left untouched.

## Review fingerings before playing

Every song and melody import also creates `practice.html` beside `practice.mp4` in the same practice folder. Open it in your browser: it shows the fingerings
for every note in your piece, in playing order, including repeated notes, rests
and hold durations. Read left to right, then continue on the next row.
Review them at your own pace, then press Play in the embedded video. Nothing
autoplays. Your chosen count-in still runs when playback starts.

Move or share the whole practice folder; its HTML links to the MP4 locally.
The MP4 itself is still a regular video; opening it directly skips this overview.

## Use a melody you already have

MIDI, MusicXML and timed note JSON can go straight to a practice video. Tono
preserves the imported notes and skips melody transcription and cleanup.

```sh
# Known melody + your backing track
tono melody.mid --instrument ae01 --backing backing.mp3

# Known melody + best-effort backing extraction from the original recording
tono melody.musicxml --instrument ae01 --audio performance.mp4 --make-bgm \
  --part lead --separation-model htdemucs-6s

# No backing: play synthesized reference tones with the fingering video
tono melody.json --instrument ae01 --tempo-scale 0.75
```

The usual `--title`, `--from`, `--to`, `--count-in`, `--metronome`,
`--easy-fingering`, instrument and output options still apply. `--from` and `--to`
refer to the **melody timeline** for these inputs.

Use `--audio-offset 7.68` if melody time zero occurs at 7.68 seconds in the
backing/original recording. A negative offset inserts silence before the audio.
Offsets printed from JSON metadata are suggestions, not automatically applied.
Audio must cover the selected passage; Tono never stretches it to hide a mismatch.
`--tempo-scale` still slows both notes and backing together.

`--separation-model htdemucs-6s` adds guitar and piano stems. With `--part lead`,
it excludes the whole **other** stem and keeps the remaining stems. This can
remove accompaniment too, and the retained stems can still contain lead leakage.
Listen to `backing.mp3` before practicing; no separation quality score is claimed.
With the default 4-stem model an instrumental lead usually shares **other** with
its guitar/piano accompaniment; if the backing is then too quiet Tono refuses the
run and suggests `--separation-model htdemucs-6s`.
The model also works with ordinary audio/video input. The default remains
`htdemucs`. Run `python ml/analyze.py --download-models` inside the ML venv to
update an existing source checkout; release runtimes include both models.

**PDF, PNG and JPG sheet music are not direct imports yet.** Convert the score
with music-recognition software to MusicXML or MIDI, check the recognized notes,
rhythm and tempo, then use the commands above. Renaming an image/PDF does not
convert it. Compressed `.mxl` must be exported as uncompressed `.musicxml` first.
These import and practice-sheet features require **Tono 0.1.1 or later**.

For multiple MIDI tracks or MusicXML parts, choose `--melody-track N` (one-based;
MIDI track numbering includes the conductor track). Import currently supports a
single monophonic melody, MIDI tempo maps, and uncompressed partwise MusicXML
with explicit tempo, rests, ties and instrument transposition. Export repeats,
ornaments, multiple voices, sustain and pitch bends as explicit linear notes
first. Invalid/overfull bars fail rather than silently changing timing. Short
pickup/final bars retain their explicit duration with a warning.

JSON accepts `notes` containing `start`, `end` (seconds), `midi` (0–127), and
optional `confidence`. Optional `duration_seconds` preserves trailing silence.
Supplied confidence is recorded, not used to discard notes. Imported scores and
their synchronization remain unverified until checked by listening.

---

## Examples

Each run starts with a colored title, the installed version and the resolved paths:

```text
Tono - Play what you love.
v0.1.1
Input: /path/to/song.mp3
Output: /path/to/tono-practices/song_ae01_YYYYMMDD/practice.mp4
```

The version comes from Cargo at build time. Redirected output stays plain text;
set `NO_COLOR=1` to disable the title color in a terminal.

Create an AE-01 practice video with an automatic filename:

```sh
tono ~/Downloads/song.mp3 --instrument ae01
```

The video goes to `./tono-practices/song_ae01_YYYYMMDD/practice.mp4`.
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

For Aerophone Brisa, use the [mode-specific examples](#aerophone-brisa) above.

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


### Development status

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

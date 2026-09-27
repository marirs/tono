# Tono (Play what you love)

A CLI that turns a song into an instrument practice video.
Spec: [TONO_V0_BUILD_SPEC.md](TONO_V0_BUILD_SPEC.md) (v0.1). Implement milestone-by-milestone; no GUI until M4.

## Status

| Milestone | State |
|-----------|-------|
| M0 environment (`tono doctor`) | done |
| M1 hard-coded fingering video (`tono demo`) | done; fingering table awaits on-instrument check |
| M2 song region + BGM + transcription (`tono prep`) | implemented; automated + synthetic/TTS checks pass. **Real-song validation pending** |
| M3 end-to-end `tono prep` with practice.mp4 | accepted after review; **real-song validation pending** |
| M4 human test | pending test media + AE-01; log in [M4_TEST_LOG.md](M4_TEST_LOG.md) |

## Quick start

```bash
tono ~/Downloads/song.mp3 --instrument ae01
```

Select `--instrument <profile>` or use `--piano` for the full 88-key piano.
All processing options remain available.

One command runs song detection, vocal separation, transcription, octave fitting,
and the vertical AE-01 video render, then validates the MP4. A spoken four-count
and matching on-screen numbers prepare you before the song begins. MP4/MOV video inputs
work too. Defaults: vocal melody, original tempo, visual metronome, automatic
whole-melody octave shifting with preserved intervals (`--range-policy strict`).
If no octave shift fits, processing fails with the out-of-range notes. Individual
folding is disabled; legacy `--range-policy fold` fails before processing.
No interactive questions.

Output defaults to `./tono-practices/song_ae01_YYYYMMDD.mp4`, using the input
filename, selected instrument and local date. Use `--practice-dir DIRECTORY`
(or `-d DIRECTORY`) to change that folder. The matching `.tono/` folder contains
`backing.wav`, `lead.wav`, notes, fingerings and project metadata.

Reruns replace the matching video and supporting folder automatically after the
new video passes validation. Failed preparation leaves previous outputs untouched;
diagnostics stay in the printed temporary directory. Previous outputs are backed
up under `$CODEX_HOME/artifacts/tono/` (default `~/.codex/artifacts/tono/`).
An optional second positional argument still sets an exact MP4 filename; use it
instead of `-d`. Source files cannot be overwritten. Quote paths containing spaces.

Optional overrides:

```bash
tono song.mp3 --instrument ae20 -d ~/Music/Practices
tono song.mp3 --instrument ae01 --tempo-scale 0.75 --metronome both
tono song.mp3 slower.mp4 --tempo-scale 0.75 --metronome both --instrument ae01
tono song.mp3 exact.mp4 --range-policy strict --instrument ae01
tono instrumental.mp3 practice.mp4 --part lead --instrument ae01
tono reel.mp4 excerpt.mp4 --from 00:12 --to 00:40 --keep-work --instrument ae01
```

Install/update the command once with `cargo install --path . --offline` from this
checkout (after ML setup below). Keep the checkout: the installed command uses
its ML environment and fingering data. The older `tono prep ... --out DIRECTORY`
form still works and retains its strict range-policy default.

## Commands

```bash
# M0: environment check. ML items are warnings; `--ml` makes them fatal (M2+).
cargo run -- doctor
cargo run -- doctor --ml

# M1: 12 hard-coded notes -> practice.mp4 (synthesized melody guide as audio)
cargo run --release -- demo --out ./tono-demo
open ./tono-demo/practice.mp4

# options
cargo run --release -- demo --out ./tono-demo --metronome both   # off|visual|audio|both
cargo run --release -- demo --out ./tono-demo --bpm 80 --keep-work
cargo run --release -- demo --out ./tono-demo --backing ~/Music/some.wav

# M2: one-time ML setup (Python 3.11 venv + model weights, ~700 MB)
brew install python@3.11
ml/setup_venv.sh
cargo run -- doctor --ml

# M2+M3: song region -> BGM + lead -> transcription -> fingerings -> practice.mp4
cargo run --release -- prep ~/Music/clip.mp4 --instrument ae01 --part vocal --out ./tono-out
cargo run --release -- prep ~/Music/clip.mp4 --from 00:12 --to 00:48 --out ./tono-out --instrument ae01   # manual region
cargo run --release -- prep ~/Music/clip.mp4 --part lead --keep-work --out ./tono-out --instrument ae01   # instrumental melody
cargo run --release -- prep ~/Music/clip.mp4 --tempo-scale 0.75 --metronome both --out ./tono-slow --instrument ae01

cargo test
```

## Real-song validation (pending)

No real song has been run yet. To validate, run on a real clip and inspect
the outputs by ear and eye:

```bash
cargo run --release -- prep /path/to/reel.mp4 --instrument ae01 --part vocal --out ./tono-real --keep-work
afplay ./tono-real/backing.wav   # vocal absent or clearly reduced, accompaniment intact
afplay ./tono-real/lead.wav      # the sung melody, little accompaniment
jq '.selected_region, .region_detection.excluded, .separation, .warnings' ./tono-real/project.json
jq -c '.notes[]' ./tono-real/notes.json | head -30   # compare against the melody you hear
```

Thresholds tuned only on synthetic clips (re-check here): region speech/music
levels, the 0.75 repeat-attack threshold, lead/BGM level limits.

## M2 pipeline

1. ffmpeg normalizes the source (audio or video) to 44.1 kHz stereo float WAV.
2. Song region: PANNs (AudioSet) reports per-0.25 s speech/music/singing
   probabilities; Rust labels frames, bridges short gaps and picks the
   longest/most confident musical span. `--from/--to` always override;
   `--auto-song-region false` uses the whole file. No music found: fails.
3. Demucs htdemucs separates the region. `backing.wav` = all non-lead stems,
   peak-normalized to -1 dBFS, 24-bit stereo, exactly the region's length.
   `--part vocal` leads with `vocals`; `--part lead` with `other` (DEFERRED:
   no finer lead-instrument detection, so pads/chords in `other` leak in).
   Lead more than 35 dB below the mix: fails; below 25 dB: low-confidence.
   BGM (measured before normalization) more than 18 dB below the mix: fails
   (unaccompanied voice or failed separation); below 12 dB: low-confidence.
   BGM gain is capped at +12 dB so residue is never amplified. `--part lead`
   is always low-confidence because the `other` stem is assumed, not detected.
   These levels show stem presence only; separation quality is not measured.
4. Basic Pitch transcribes `lead.wav`; Rust cleanup (confidence, 60 ms
   minimum, 80 ms merge unless the note has a real attack, vibrato
   absorption, monophonic reduction) writes `notes.json` (region timebase).
5. `project.json` records source, selected region, excluded spans, stems,
   BGM mix, separation level, transcription parameters, BPM and warnings.

## M3: practice video

6. Range fit: charted front-key patterns plus documented octave controls
   cover B2-C#6 (MIDI 47-85). `--range-policy strict`
   (default) shifts the whole melody by the smallest whole octave that fits
   every note (intervals preserved), or fails listing the out-of-range notes.
   Legacy `--range-policy fold` is rejected: individual pitch folding is disabled.
7. `--tempo-scale` (0.5-2.0) stretches note times and time-scales the BGM
   with ffmpeg `atempo` (pitch kept). `backing.wav` itself stays unscaled.
8. Metronome clicks and flashes are the beat tracker's detected timestamps
   (divided by `--tempo-scale`), so tempo changes are followed. Bar position
   is not detected, so there are no downbeat accents and a single pulsing
   dot instead of a bar counter. Fewer than 4 beats: metronome disabled with
   a warning. `audio`/`both` mix a separate click track at render time.
9. `practice.mp4` = fingering animation + BGM, validated with ffprobe; the
   run fails otherwise. `notes.json` keeps transcribed pitches;
   `fingering.json` holds what is played (practice-video timebase).

Runs are deterministic (seeded Demucs) and never download models
(`HF_HUB_OFFLINE=1`); `ml/setup_venv.sh` is the only download step.

The AE-01 is drawn upright with the mouthpiece at the top. Hand labels default
to right hand above / left hand below to match the requested playing grip;
`--upper-hand left` selects the opposite labels. Physical key IDs and fingerings
do not change. Rear thumb keys remain in a separate inset. The current and NEXT
fingerings sit on the left; a live treble-staff pitch guide sits on the right.
The staff shows the played notes, including disclosed octave adjustments. It is
a pitch guide, not a quantized rhythmic score or inferred time signature.

The default `--count-in 4` adds spoken numbers and preparation pulses before the
song; `--count-in 0` disables it. The count uses the detected median tempo (or
100 bpm if no tempo is available, with a warning). Audio, fingering timestamps,
and detected beats all receive the same sample-aligned delay. `backing.wav`,
`lead.wav` and `notes.json` keep their original selected-region timebase;
`fingering.json` uses video time, with `count_in_seconds` recorded explicitly.
Speech is generated locally by macOS `say`; if unavailable, audible count-in
clicks still play and a warning is recorded. Count-in audio is independent of
`--metronome`, which controls cues during the song.

Every rendered MP4 is validated with ffprobe (video + AAC audio streams, stream
durations and timeline length within 100 ms); the command fails otherwise.
Backing audio shorter than the video is padded with silence, with a warning.
The metronome click is a separate track (`work/metronome.wav`), mixed only at render time.

Executables: `ffmpeg`/`ffprobe` are found on `PATH`, then `/opt/homebrew/bin`,
`/usr/local/bin`. Overrides: `TONO_FFMPEG`, `TONO_FFPROBE`, `TONO_PYTHON`, `TONO_ROOT`.

## Fingering data

`instruments/ae01.json` is transcribed from Roland's AE-01 Fingering Chart
("Recorder" section) and Owner's Manual pp. 20-21; the key layout in
`src/instruments/ae01.rs` follows manual pp. 2, 6 and 11. It covers only the
documented front-key range B3-C#5, extended by octave controls to B2-C#6 (MIDI 47-85).

`verified` stays `false`, and videos show a caution banner, until all 39
entries (MIDI 47-85) have been played on a real AE-01. The demo melody only
exercises 4 of them (C4, D4, E4, G4), so playing the demo is not enough.
Open questions for that check:

- Is Recorder mode the factory default? (The manual only teaches this mode.)
- Do the octave UP/DOWN keys act while held, or latch?
- For notes with two charted fingerings, the first diagram is used; confirm it plays.

The large current diagram shows PRESS (green ring) and LIFT (red dashed ring)
for the change from the previous note. SAME AGAIN means repeat without changing
keys. NEXT remains a preview of the following change. Start at `--tempo-scale 0.5`
and use `--from`/`--to` for a short phrase; `tono --help` lists every control.

Octave-aware range: the bundled table covers MIDI 47–85 (B2–C#6), derived
from Roland's front-key chart and documented +/-12-semitone octave controls.
Normal fingerings are preferred in the overlap; documented alternatives are
retained in the table and output fingering metadata. `verified` remains false.
The canonical `instruments/ae01.json` was corrected against Roland's chart:
C5 uses key 2; C#5 is all-open (alternative: key 2 + sharp), including octave variants.
Strict fitting preserves every interval. If the whole melody shifts by octaves,
the rendered backing shifts by the same amount; original `backing.wav` stays in
the source register. The semitone shift is recorded in project/fingering JSON and
shown on video. No notes are individually folded in strict mode.

## Instrument profiles

The Rust module structure and extension points are documented in
[ARCHITECTURE.md](ARCHITECTURE.md).

`--instrument` supports `ae01`, `ae05`, `ae10`, `ae20`, `guitar`, `guitar-bass`,
`guitar-bass-5string`, `piano` (alias `piano-88`), `keyboard-76`, and `keyboard-61`.
`--piano` is shorthand for `--instrument piano`. See the instrument guide for
additional guitar and keyboard aliases.
All tempo, metronome, region and confidence options are shared. Instrument
selection changes the fingering lookup, available range, diagram and metadata.
Profiles and their source references live in [instrument guide](INSTRUMENTS.md).

```sh
tono song.mp3 ae05.mp4 --instrument ae05
tono song.mp3 ae10.mp4 --instrument ae10
tono song.mp3 ae20.mp4 --instrument ae20
tono song.mp3 guitar.mp4 --instrument guitar --tempo-scale 0.5
tono demo --instrument guitar --out /tmp/tono-guitar-demo
```

New wind profiles use Sax mode, zero instrument transposition/scene octave
shift, and OCT2/Oct2 on AE-10/20. Set these on your instrument; Tono displays
the requirements but cannot configure the hardware. Diagrams are vertical,
with the model's numbered performance controls and rear octave keys.
Guitar uses a horizontal fretboard for a single-note melody: standard six-string
tuning, no capo, frets 0–19. It does not generate chord accompaniments.
All new instrument mappings still require on-instrument verification.

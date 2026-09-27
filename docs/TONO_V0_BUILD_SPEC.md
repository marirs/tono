# Tono v0.1 — Build Specification

## Goal

Build a local-first CLI prototype that proves the core Tono idea:

> Give Tono a local audio/video file, automatically find the actual song portion, choose the Roland AE-01, and produce:
>
> 1. a clean backing track/BGM with the selected lead/vocal reduced or removed;
> 2. a time-aligned melody note sequence;
> 3. an AE-01 fingering timeline;
> 4. an animated practice MP4 that is already muxed with the generated BGM and can be followed visually without knowing note names.

The generated `practice.mp4` must be directly usable: press play, hear the BGM, and follow the AE-01 fingering animation.

This is a **private macOS prototype**. Do not build accounts, cloud sync, subscriptions, a server, web UI, iOS, Android, or a catalogue.

## Success criterion

A beginner who does not know note names should be able to put `practice.mp4` on screen, follow the animated AE-01 key diagram, and recognizably play the selected melody over `backing.wav`.

If that works for one real 20–60 second clip, v0 is successful.

---


## 0. Song-region detection

Reels, shorts and social clips often contain spoken intros/outros, commentary, silence, applause, or unrelated audio.

Tono must **not** blindly transcribe the entire file.

Before stem separation/transcription, detect the actual musical region(s).

### Required behavior

By default:

1. detect speech-only intro sections;
2. detect speech-only outro sections;
3. detect silence / near-silence;
4. detect the main contiguous musical section;
5. prefer the longest/highest-confidence music region when multiple regions exist;
6. trim analysis to the selected music region;
7. preserve manual override with `--from` / `--to`.

### CLI

Add:

```bash
--auto-song-region true|false
```

Default: `true`.

Manual boundaries always override auto-detection.

Examples:

```bash
tono prep reel.mp3 \
  --instrument ae01 \
  --part vocal \
  --auto-song-region true
```

or:

```bash
tono prep reel.mp3 \
  --instrument ae01 \
  --part vocal \
  --from 00:12 \
  --to 00:48
```

### Output metadata

`project.json` must record:

```json
{
  "source_duration": 59.8,
  "selected_region": {
    "start": 8.42,
    "end": 52.10,
    "method": "auto-song-region",
    "confidence": 0.93
  }
}
```

Do not treat spoken words like melody notes.

---

# Architecture

```text
local audio/video
      |
      v
+-------------------+
| Rust `tono` CLI   |
+-------------------+
      |
      +--> ffmpeg: normalize/extract audio
      |
      +--> song-region detector
      |      |
      |      +--> remove speech-only intro/outro
      |      +--> remove silence/non-song sections
      |
      +--> Python ML worker
      |      |
      |      +--> stem separation
      |      +--> selected lead/vocal stem
      |      +--> BGM/accompaniment stem mix
      |      +--> melody transcription
      |      +--> beat / tempo hints
      |
      +--> Rust cleanup / timeline normalization
      |
      +--> deterministic AE-01 fingering mapper
      |
      +--> practice-video renderer
      |
      +--> ffmpeg final mux
              |
              +--> practice.mp4  # fingering animation + BGM audio
              +--> backing.wav   # BGM only
              +--> lead.wav
              +--> notes.json
              +--> fingering.json
              +--> project.json
```

## Language split

### Rust

Rust owns:

- CLI
- project directory creation
- calling ffmpeg
- invoking the Python ML worker
- parsing ML JSON output
- note cleanup
- AE-01 fingering mapping
- normalized Tono timeline
- rendering instructions
- MP4 orchestration
- deterministic behavior
- tests

### Python

Python is **only the ML adapter**.

It owns:

- source separation
- pitch / note transcription
- optional beat analysis

It must output plain JSON. The rest of Tono must not depend on Python-specific types or state.

### FFmpeg

Use FFmpeg for:

- extracting audio from video
- resampling / normalization
- final audio mixing
- muxing frames + backing audio into MP4

No OpenAI API is required for v0.
No paid music API is required for v0.

---

# CLI

Primary command:

```bash
tono prep INPUT \
  --instrument ae01 \
  --from 00:20 \
  --to 01:04 \
  --part vocal \
  --out ./output
```

Minimum options:

```text
INPUT                   local mp3/m4a/wav/flac/mov/mp4
--instrument ae01       only ae01 required in v0
--from HH:MM:SS         optional
--to HH:MM:SS           optional
--part vocal|lead       default vocal
--auto-song-region BOOL  default true
--tempo-scale FLOAT      default 1.0
--metronome MODE         off|visual|audio|both, default visual
--out PATH               output directory
--keep-work              preserve intermediate files
```

Also support:

```bash
tono doctor
```

It should report:

- ffmpeg installed/version
- python executable
- virtualenv/model availability
- ML worker callable

---

# Output

```text
output/
├── practice.mp4
├── backing.wav
├── lead.wav
├── notes.json
├── fingering.json
├── project.json
└── work/              # only with --keep-work
```

`practice.mp4` is the central artifact.

It must contain:

- synchronized AE-01 fingering animation;
- the generated backing track/BGM as its audio track;
- no selected lead/vocal in the final BGM, to the extent the separator can remove it;
- optional metronome according to CLI settings;
- timing aligned to the same selected song region used for transcription.

Opening `practice.mp4` alone must be sufficient to practice.

---

# ML interface

Rust calls:

```bash
python3 ml/analyze.py \
  --input normalized.wav \
  --part vocal \
  --output analysis.json
```

Python writes:

```json
{
  "version": 1,
  "sample_rate": 44100,
  "duration": 43.92,
  "bpm": 94.0,
  "lead_file": "lead.wav",
  "backing_file": "backing.wav",
  "stems": {
    "vocals": "vocals.wav",
    "drums": "drums.wav",
    "bass": "bass.wav",
    "other": "other.wav"
  },
  "notes": [
    {
      "start": 0.420,
      "end": 0.880,
      "midi": 66,
      "confidence": 0.91
    }
  ]
}
```

This JSON contract is stable even if the ML model changes.

---

# ML v0 implementation

Do not invent a model.

Prefer existing local models/tools.

## Stem separation

First implementation:

- Demucs / HTDemucs

Goal for `--part vocal`:

- produce an isolated vocal/lead track for transcription;
- produce `backing.wav` by mixing all non-selected stems;
- keep all stems time-aligned with the selected song region;
- normalize the final backing track so it is immediately usable.

For `--part lead`, isolate the best available detected lead/instrument stem and remove/reduce that stem from the BGM.

Do not spend time making perfect karaoke separation in Phase 0, but **a usable BGM is mandatory**. If separation confidence is poor, fail with a clear warning or mark the result as low-confidence rather than pretending it is clean.

## Melody transcription

Start with a real automatic-music-transcription model such as Basic Pitch.

Input should preferably be the isolated lead/vocal stem rather than the full mix.

If Basic Pitch performs badly on the test clip, keep the adapter interface and evaluate a second transcription model. Do not redesign the entire application around one model.

---


# Backing-track / BGM generation

BGM generation is a required v0 feature, not optional.

For `--part vocal`:

```text
BGM = drums + bass + other (+ any non-vocal stems)
```

For `--part lead`:

```text
BGM = all stems except selected lead stem
```

Requirements:

1. BGM length exactly matches the selected song region.
2. BGM starts at t=0 in the prepared project.
3. BGM is normalized to avoid clipping.
4. Preserve stereo where available.
5. Export lossless `backing.wav`.
6. Use the same BGM as the audio bed in `practice.mp4`.
7. Do not accidentally include the original full-mix audio in the final practice MP4.
8. Keep `lead.wav` separately for debugging/transcription.
9. If a metronome audio track is enabled, mix it separately at render time so it can be disabled later.
10. Store source/stem/mix metadata in `project.json`.

Example metadata:

```json
{
  "backing": {
    "source": "stem_mix",
    "excluded_part": "vocals",
    "file": "backing.wav",
    "sample_rate": 44100,
    "channels": 2
  }
}
```

---

# Note cleanup

Raw singing contains vibrato, slides and extremely short artifacts.

Rust must normalize note events before fingering generation.

Initial deterministic rules:

1. discard events below configurable confidence threshold;
2. discard events shorter than 60 ms unless surrounded by consistent evidence;
3. merge adjacent identical semitone notes when the gap is <= 80 ms;
4. merge tiny one-semitone oscillations caused by vibrato where appropriate;
5. preserve real repeated notes when a meaningful gap/attack exists;
6. retain timing in seconds;
7. never let an LLM silently alter the final pitch sequence.

Store both:

```text
notes_raw.json
notes.json
```

during development if `--keep-work` is enabled.

---

# AE-01 fingering

Fingering is deterministic data, not AI.

Create:

```text
assets/ae01/fingering.json
```

Schema:

```json
{
  "instrument": "roland-ae01",
  "version": 1,
  "fingerings": {
    "60": {
      "octave": "normal",
      "keys": ["..."]
    }
  }
}
```

Key identifiers must correspond to stable SVG regions, not pixel coordinates.

The complete fingering table must be verified against Roland documentation / the actual instrument before calling v0 complete.

---

# AE-01 visual

Create one vector diagram of the AE-01 control surface.

Use SVG.

Every pressable area has an ID:

```text
octave_up
octave_down
left_1
left_2
left_3
right_1
right_2
right_3
sharp
flat
...
```

Renderer takes:

```json
{
  "keys": ["left_1", "left_2", "left_3"],
  "octave": "normal"
}
```

and fills only those keys.

Do not embed a separate bitmap for every fingering.

---

# Practice-video UX

The user does not need to know music theory.

Default visual hierarchy:

```text
             NEXT
        [small fingering]

              ↓

             NOW
      [large AE-01 diagram]

        [progress bar]

          ● ○ ○ ○
        metronome beat
```

Optional note names may exist internally, but do not make them required for the default beginner view.

## Current + next

Always show:

- current fingering large
- next fingering smaller
- progress through current note
- timeline progress

Later we may add previous note.

## Transition hints

If only one or two keys change between current and next fingering:

- visually emphasize the changing key(s)

Examples:

```text
LIFT
PRESS
```

This is optional for the first rendering milestone but the model/data structure should allow it.

---

# MP4 rendering

For v0, prioritize correctness over fancy GPU rendering.

The MP4 must be the **ready-to-practice deliverable**.

Acceptable implementation:

1. Rust generates frame descriptions from the cleaned note/fingering timeline.
2. Render SVG/PNG frames.
3. FFmpeg creates the H.264 video stream.
4. FFmpeg muxes `backing.wav` as the primary audio track.
5. If requested, mix metronome audio into the practice MP4 at render time.
6. Verify final audio/video duration and sync before reporting success.

The final MP4 must never be silent unless the user explicitly requests silent output.

Target:

```text
1920x1080
30 fps
H.264
AAC
```

A lower render cadence with duplicated frames is acceptable because the graphics change mainly at note boundaries.

Do not render 30 entirely new vector frames every second if nothing changes.

After rendering, validate with `ffprobe`:

- video stream exists;
- audio stream exists;
- audio codec is AAC or another intentional supported codec;
- durations are within 100 ms of each other;
- final duration matches the prepared region;
- file is non-empty.

If validation fails, the command must fail instead of printing success.

---

# Metronome

BPM comes from analysis or is user-overridable.

Practice video should support:

```text
--metronome off|visual|audio|both
```

Default: `visual`.

For audio metronome, keep it on a separate generated track so it can later be excluded from performance exports.

---

# Phase 0 milestones

## M0 — environment

`tono doctor` passes.

## M1 — hard-coded fingering video

Before any ML:

- hard-code 8–12 note events
- render AE-01 fingering animation
- mux any backing audio

This proves rendering/timing.

## M2 — song region + BGM + transcription

- normalize uploaded audio/video;
- detect and trim to the actual song region;
- exclude spoken intro/outro where present;
- separate vocal/lead;
- create usable `backing.wav`;
- run pitch transcription on the isolated lead;
- output `notes.json`;
- output region/stem metadata in `project.json`.

No practice video yet.

Manually inspect both `backing.wav` and `notes.json`.

## M3 — end-to-end

```bash
tono prep short.mp3 --instrument ae01
```

creates:

- backing.wav
- lead.wav
- notes.json
- fingering.json
- project.json
- practice.mp4

`practice.mp4` must already contain the BGM audio and must pass ffprobe A/V validation.

## M4 — test with a human

Play the clip on AE-01 using only the visual fingering.

Record:

- which note transitions were wrong
- timing that felt too fast
- transcription errors
- fingering errors

Fix those before adding features.

---

# Non-goals for v0

Do NOT build:

- SwiftUI app
- OpenAI integration
- cloud processing
- YouTube downloading
- login/account system
- database server
- mobile apps
- subscription
- public song catalogue
- automatic social posting
- generalized multi-instrument support

These come after the core experience works.

---

# Later macOS app

Once M4 succeeds:

```text
SwiftUI
  |
  +--> native file picker / drag drop
  +--> waveform + clip selection
  +--> AE-01 MIDI connection
  +--> practice player
  +--> performance mode
  +--> recording
  |
  +--> Tono core
```

The Mac app should use live vector UI instead of relying on rendered practice MP4.

The MP4 generator remains useful for:

- standalone practice
- sharing
- debugging
- future mobile/lightweight usage

---

# Engineering rules for Codex / Claude

1. Read this entire spec before editing.
2. Do not expand scope.
3. Do not add an LLM or external API.
4. Do not implement a GUI until M4 succeeds.
5. Do not replace Rust with Python-only.
6. Python is an isolated ML worker.
7. All ML outputs cross the boundary as versioned JSON.
8. Fingering is deterministic verified data.
9. Never fabricate missing fingering data.
10. Fail loudly when prerequisites or analysis fail.
11. Never claim a generated transcription is correct merely because the model completed.
12. Keep intermediate artifacts with `--keep-work`.
13. Add tests around timestamp parsing, event normalization, fingering lookup and renderer timing.
14. Commit milestone-by-milestone.
15. At the end of every milestone, print exact commands for the user to run.
16. Never report `practice.mp4` complete if it has no BGM audio stream.
17. Never transcribe speech-only intro/outro as melody.
18. Manual `--from` / `--to` always override auto song-region selection.
19. Preserve the generated BGM as a standalone artifact in addition to muxing it into the MP4.
20. Validate the final MP4 with ffprobe before success.

---

# Acceptance test

Input:

```bash
tono prep ~/Music/test-short.mp3 \
  --instrument ae01 \
  --part vocal \
  --out ~/Music/Tono/test
```

Expected:

```text
✓ source normalized
✓ song region detected
✓ speech-only intro/outro excluded
✓ vocal/lead separated
✓ backing/BGM created
✓ melody transcribed
✓ melody cleaned
✓ AE-01 fingerings mapped
✓ practice video rendered with BGM
✓ final audio/video sync validated

~/Music/Tono/test/practice.mp4
~/Music/Tono/test/backing.wav
```

A user with no note-reading knowledge must be able to open `practice.mp4`, hear the generated BGM immediately, follow the highlighted AE-01 keys, and attempt the melody without needing any other file.

Additional acceptance conditions:

- spoken intro/outro must not appear in the fingering timeline unless manually included;
- the selected lead/vocal must be absent or materially reduced in the BGM;
- fingering changes must remain synchronized with the BGM;
- the MP4 must contain both video and audio streams;
- `backing.wav` must also be usable independently.

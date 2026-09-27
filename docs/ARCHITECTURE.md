# Rust source layout

`src/lib.rs` exposes the reusable `tono` engine. Frontends share this library;
they do not need to invoke the CLI or construct command-line arguments.
`src/main.rs` only calls `cli::run()`. `src/cli.rs` owns argument parsing,
validation of CLI combinations, and command dispatch, importing the library.

| Directory | Responsibility |
| --- | --- |
| `src/instruments/` | Instrument registry, fingering profiles, transitions and instrument diagrams |
| `src/pipeline/` | Source-to-practice orchestration, practice assembly and safe output publication |
| `src/analysis/` | Python ML worker contract, song-region selection, pitch-track evidence (`evidence.rs`) and note cleanup |
| `src/music/` | Notes, range fitting, musical timeline and timecode parsing |
| `src/media/` | Source audio conversion, guide/click synthesis and ffprobe validation |
| `src/render/` | Video composition, NOW-panel cue text (`cues.rs`), staff drawing, rasterization and ffmpeg streaming |
| `src/commands/` | Standalone `demo` and `doctor` commands |

`src/paths.rs` locates runtime data, the Python environment and external tools.
Tests remain next to the implementation they exercise.

## Instrument ownership

- `mod.rs`: instrument identifiers, profile paths, layout schema and wind dispatch.
- `ae01.rs`: AE-01 control geometry and drawing.
- `brisa.rs`: AE-BRISA mode validation, horizontal keys and breath/register cues.
- `ae05.rs`, `ae10.rs`, `ae20.rs`: model entry points using the shared sax renderer.
- `yamaha.rs`: YDS-120/150 entry point with charted Oct and Low A controls.
- `recorder.rs`: acoustic soprano recorder holes, split holes and thumb venting.
- `flute.rs`: acoustic concert flute keys, footjoint controls and register cues.
- `violin.rs`: fretless first-position placements, finger numbers and string-change cues.
- `sax.rs`: common sax drawing, parameterized by each profile's key layout.
- `guitar.rs`: horizontal guitar/bass/ukulele fretboards, string/fret decoding and playing cues.
- `piano.rs`: full keyboard overview, active-key detail and press/release cues.
- `diagram.rs`: shared drawing state, grip labels and display primitives.
- `fingering.rs`: profile loading/validation, note mapping and key transitions.

The sax entry points intentionally reuse code. Their different keys, octave modes
and fingerings live in the corresponding JSON profiles; do not duplicate the sax
renderer to represent data differences.

The root `instruments/*.json` files remain canonical runtime data. They are
separate from `src/instruments/*.rs`, which implements their behavior. Keep
musical mappings and source provenance in JSON rather than embedding them in
rendering code.

## Processing flow

`cli` → `pipeline::output` → `pipeline::prep` → analysis and note cleanup →
`pipeline::practice` → range fitting/fingerings/timeline → `render` → media
validation → output publication.

The worker also returns a pYIN pitch track (`pitch_track` in analysis.json:
f0, voicing probability and level per 23 ms frame on the lead stem). Rust
turns it into per-note evidence and applies the cleanup decisions; Python
only measures. The boundary checks finite values, probability ranges, aligned
arrays and region coverage. Present-but-malformed tracks fail; older analysis
files without a track fall back to duration rules.

`demo` constructs a known melody and uses the same instrument and rendering
modules without ML. `doctor` checks the environment independently.

For structural changes, run `cargo fmt --check`, `cargo test` and
`cargo build --release`. Instrument or rendering changes should also be checked
with `tono demo --instrument <id>` for each affected model.

## Frontend boundary

A GUI can construct `tono::pipeline::prep::PrepOptions`, then call
`tono::pipeline::output::run(&options)` to prepare, validate and publish an MP4
safely. Set `output_video` to the MP4 path and `output_directory` to its matching
`.tono` sidecar path (`video.with_extension("tono")`).
`tono::pipeline::prep::run_prep(&options)` retains the advanced
project-directory workflow. `tono::instruments::Instrument` selects a profile;
`tono::instruments::fingering::FingeringTable::load_instrument` reads its data.

The library returns `anyhow::Result`; it does not parse arguments, prompt for
input or exit the process. Processing is currently synchronous, invokes external
ML/media tools and reports progress to stdout/stderr. A GUI should run it off its
UI thread. Structured progress events and cancellation are future API work.

CLI tests live in the binary's `cli.rs`; engine tests live in the library modules.

The main [README](../README.md) stays at the repository root; the
[instrument guide](../instruments/README.md) lives with its profiles. The build
specification and architecture/testing documents live in `docs/`.

For AE-BRISA, frontends must set `PrepOptions::fingering_mode` to `Brisa` or
`Flute`; other instruments must leave it `None`. Profiles are selected with
`FingeringTable::load_for_mode`. Mode selection is validated before ML work.

## Private release runtime

`runtime::ensure_ready(callback)` installs a checksum-pinned runtime into OS user
app data. The CLI calls it automatically before rendering; a GUI can call it on
its background worker with structured setup progress. `tono setup` is optional;
`doctor` stays read-only. Source builds retain the developer environment.

Release builds embed their scripts/profiles and runtime manifest. The installed
runtime supplies Python, FFmpeg, fonts and offline model caches. Python worker
subprocesses get isolated environment variables; the host process is unchanged.
See [distribution](DISTRIBUTION.md) for packaging, checks and platform limits.

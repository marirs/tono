# Rust source layout

`src/lib.rs` exposes the reusable `tono` engine. Frontends share this library;
they do not need to invoke the CLI or construct command-line arguments.
`src/main.rs` only calls `cli::run()`. `src/cli.rs` owns argument parsing,
validation of CLI combinations, and command dispatch, importing the library.

| Directory | Responsibility |
| --- | --- |
| `src/instruments/` | Instrument registry, fingering profiles, transitions and instrument diagrams |
| `src/pipeline/` | Source-to-practice orchestration, practice assembly and safe output publication |
| `src/analysis/` | Python ML worker contract, song-region selection and note cleanup |
| `src/music/` | Notes, range fitting, musical timeline and timecode parsing |
| `src/media/` | Source audio conversion, guide/click synthesis and ffprobe validation |
| `src/render/` | Video composition, staff drawing, rasterization and ffmpeg streaming |
| `src/commands/` | Standalone `demo` and `doctor` commands |

`src/paths.rs` locates runtime data, the Python environment and external tools.
Tests remain next to the implementation they exercise.

## Instrument ownership

- `mod.rs`: instrument identifiers, profile paths, layout schema and wind dispatch.
- `ae01.rs`: AE-01 control geometry and drawing.
- `ae05.rs`, `ae10.rs`, `ae20.rs`: model entry points using the shared sax renderer.
- `sax.rs`: common sax drawing, parameterized by each profile's key layout.
- `guitar.rs`: horizontal fretboard, string/fret decoding and playing cues.
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

All project Markdown lives in `docs/`; start with [README.md](README.md).

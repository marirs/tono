# Downloadable Tono

Release packaging is being validated. No public portable release has been
published by this change. Source builds still use the existing developer setup.

## For users

Each platform has two download options:

- **Small executable:** download the executable for your OS and processor, then
  run your usual Tono command. On first render, Tono downloads and checks its
  private audio/ML runtime automatically. Internet access and several GB of free
  disk space are needed for this initial setup. Later processing runs locally.
- **Offline ZIP:** download and extract the ZIP. Keep `tono` (Windows:
  `tono.exe`) beside `tono-runtime-<platform>.tar.gz`. The same first-use setup
  uses that archive without downloading anything.

```sh
./tono song.mp3 --instrument ae20 --title "My song"
```

For the small download, rename the executable to `tono` (`tono.exe` on Windows).
On macOS/Linux, a directly downloaded executable may need `chmod +x tono`.
On Windows, use `.\tono.exe` in PowerShell.
The CLI does not yet provide a double-click graphical interface. A GUI can use
the same installer and engine without asking users to manage Python.

`tono setup` optionally prepares everything before choosing a song.
`tono doctor --ml` checks the installed environment without downloading.
No separate Python, FFmpeg, package manager, or administrator access is needed
for the release runtime. All existing instrument and practice flags remain.

Install locations follow the OS user-data convention: macOS Application Support,
Windows LocalAppData, Linux XDG data home. `TONO_DATA_DIR` overrides that location.
Runtime versions coexist; replacing the executable does not delete old runtimes
or practice videos. Failed/interrupted setup is retried on the next run.

## Platform status

The release workflow attempts macOS Intel/ARM64, Linux x86_64/ARM64, and Windows
x86_64. Each package must pass tests on its own runner. These targets are not
certified until that workflow and clean-machine testing succeed. Linux packages
target glibc distributions, not Alpine/musl. Minimum OS requirements depend on
the resolved native packages and runner; do not claim older OS support from a
successful build alone.

Windows ARM64 has a native Rust CI test job, but no ML runtime release job yet.
Conda-forge's Windows ARM support is experimental; the full Python/ML/native
dependency set must be validated before adding that package. Do not advertise
an x86_64 runtime as a native ARM64 build.

Packages are not yet Apple-notarized or Windows code-signed. Signing requires
the project's release credentials; it is a remaining public-distribution task.

## Release maintainers

`.github/workflows/ci.yml` tests Rust on all six OS/architecture combinations.
`.github/workflows/release.yml` builds the five candidate runtime packages.
Manual dispatch uploads test artifacts only. A `v<version>` tag matching
Cargo.toml creates a **draft** GitHub release only after every packaging job
passes. Review the packages before publishing the draft.

The release sequence is:

1. Create a fresh conda-forge environment using `scripts/release_environment.yml`.
2. Install CPU PyTorch and `ml/requirements.txt`; run `pip check`.
3. Run `scripts/package_runtime.py --target <Rust target> --out <temporary dir>`
   using that environment's Python. It downloads models and labels, executes all
   ML engines, and packs the environment using conda-pack.
4. Run `scripts/release_artifacts.py` with the same arguments. It compiles the
   runtime SHA-256, size, version and download URL into Tono, then installs from
   the adjacent archive with an empty home and restricted PATH. It runs doctor,
   a validated demo render and all ML engines again after relocation.
5. Upload the small executable, runtime archive, offline ZIP, checksums and
   validation output. Do not replace an archive under an existing version: the
   executable trusts that exact hash. Use a new version to change dependencies.

The installer verifies the entire compressed archive before extraction, rejects
escaping paths/links, bounds the unpacked size, serializes setup with an OS lock,
relocates at the final path and checks the runtime before writing a ready marker.
Failed installation never becomes the active runtime. Embedded Python scripts
and instrument JSON replace the packaged copies to match the executable.

The Rust library exposes `runtime::ensure_ready(callback)` with structured setup
progress. GUI frontends should run this blocking operation on a background
thread, then call the existing practice engine. No process-wide Python/cache
environment variables are changed; only worker subprocesses get private paths.

Dependencies are resolved per platform. The archive retains exact conda package
metadata (URLs and hashes), pip freeze output, Python package metadata and
installed license files; package-cache license files are copied when available.
The current developer `ml/requirements.lock` is macOS-specific and is not used
as a universal platform lock.

Before a public release, review redistribution obligations for the exact FFmpeg
build (including x264), Python packages and model weights. Preserve applicable
licenses/notices and provide any required corresponding source. The draft gate
does not replace that review. Model origins: Demucs `adefossez/HTDemucs`, PANNs
Cnn14 DecisionLevelMax on Zenodo, Basic Pitch's bundled ICASSP-2022 model, and
Google AudioSet class labels.

References: [conda-pack](https://conda.github.io/conda-pack/),
[GitHub runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners),
[Windows ARM dependency status](https://conda-forge.org/blog/2026/02/09/win-arm64/).

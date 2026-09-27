"""Compile against the runtime checksum; test first use outside the checkout."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

REPO = Path(__file__).resolve().parents[1]


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--target", required=True)
    p.add_argument("--out", required=True, type=Path)
    args = p.parse_args()
    out = args.out.resolve()
    manifest_path = out / f"runtime-{args.target}.json"
    manifest = json.loads(manifest_path.read_text())
    tag = os.environ.get("GITHUB_REF", "")
    if tag.startswith("refs/tags/") and tag != f"refs/tags/v{manifest['version']}":
        raise SystemExit("release tag must equal Cargo.toml's version")
    env = os.environ.copy()
    env["TONO_RELEASE_MANIFEST"] = str(manifest_path)
    if "apple-darwin" in args.target:
        env["MACOSX_DEPLOYMENT_TARGET"] = "14.0"
    subprocess.run(["cargo", "build", "--release", "--locked", "--target", args.target],
                   cwd=REPO, env=env, check=True)
    suffix = ".exe" if "windows" in args.target else ""
    exe = out / f"tono-{args.target}{suffix}"
    shutil.copy2(REPO / "target" / args.target / "release" / f"tono{suffix}", exe)
    with tempfile.TemporaryDirectory(prefix="tono-relocated-") as tmp:
        tmp = Path(tmp)
        env = {k: v for k, v in os.environ.items()
               if not k.startswith("TONO_") and k not in ("PYTHONHOME", "PYTHONPATH")}
        env["TONO_DATA_DIR"] = str(tmp / "private-data")
        env["HF_HUB_OFFLINE"] = "1"
        # The first render automatically uses the adjacent archive. This must not require an existing
        # Python, model cache, or ffmpeg on PATH.
        env["PATH"] = str(Path(os.environ.get("SystemRoot", "/usr")) / ("System32" if suffix else "bin"))
        env["HOME"] = str(tmp / "empty-home")
        env["USERPROFILE"] = env["HOME"]
        Path(env["HOME"]).mkdir()
        for command in [
            ["demo", "--instrument", "ae20", "--out", str(tmp / "demo")],
            ["doctor", "--ml"], ["setup"],
        ]:
            subprocess.run([str(exe), *command], cwd=tmp, env=env, check=True)
        roots = list((tmp / "private-data/runtimes").glob("*/.tono-ready"))
        assert len(roots) == 1, "setup did not install exactly one runtime"
        root = roots[0].parent
        python = root / ("python.exe" if suffix else "bin/python")
        child_env = env.copy()
        child_env.update(HOME=str(root / "home"), USERPROFILE=str(root / "home"),
                         HF_HOME=str(root / "models/huggingface"), TONO_PANNS_DIR=str(root / "models/panns"))
        child_env["PATH"] = os.pathsep.join([str(root / "Library/bin"), str(root / "bin"), str(root), env["PATH"]])
        subprocess.run([str(python), str(REPO / "scripts/smoke_ml.py"), str(root / "ml")],
                       cwd=tmp, env=child_env, check=True)
        shutil.copy2(tmp / "demo/practice.mp4", out / f"smoke-{args.target}.mp4")
    # One offline download, containing the executable and its matching archive.
    offline = out / f"tono-offline-{args.target}.zip"
    with zipfile.ZipFile(offline, "w", compression=zipfile.ZIP_STORED) as bundle:
        bundle.write(exe, f"tono{suffix}")
        bundle.write(out / f"tono-runtime-{args.target}.tar.gz", f"tono-runtime-{args.target}.tar.gz")
        bundle.write(REPO / "docs/DISTRIBUTION.md", "README.md")
    checksums = []
    for path in sorted(out.iterdir()):
        if path.is_file() and path.suffix != ".sha256":
            with path.open("rb") as f:
                checksums.append(f"{hashlib.file_digest(f, 'sha256').hexdigest()}  {path.name}")
    (out / f"{args.target}.sha256").write_text("\n".join(checksums) + "\n")


if __name__ == "__main__":
    main()

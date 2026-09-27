#!/usr/bin/env python3
"""Build a relocatable, CPU-only runtime from a fresh conda-forge environment.

Run with that environment's Python. No user Python/cache is packaged. Every
resolved dependency and its metadata is retained alongside the runtime.
"""
import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import urllib.request

REPO = Path(__file__).resolve().parents[1]


def run(*args, env=None):
    subprocess.run([str(a) for a in args], check=True, env=env)


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--target", required=True)
    p.add_argument("--out", required=True, type=Path)
    p.add_argument("--repository", default="marirs/tono")
    args = p.parse_args()
    root = Path(sys.prefix).resolve()
    if not (root / "conda-meta").is_dir():
        p.error("use a fresh conda-forge Python environment, not system Python or ml/.venv")
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    import tomllib
    version = tomllib.loads((REPO / "Cargo.toml").read_text())["package"]["version"]
    env = os.environ.copy()
    env.update(HF_HOME=str(root / "models/huggingface"),
               TONO_PANNS_DIR=str(root / "models/panns"),
               HOME=str(root / "home"), USERPROFILE=str(root / "home"),
               PYTHONNOUSERSITE="1")
    for key in ("HF_HUB_OFFLINE", "PYTHONPATH", "PYTHONHOME"):
        env.pop(key, None)
    # panns_inference imports this file before loading the supplied checkpoint.
    # Supply it now so import never shells out to wget on the user's machine.
    labels = root / "home/panns_data/class_labels_indices.csv"
    labels.parent.mkdir(parents=True, exist_ok=True)
    urllib.request.urlretrieve(
        "https://storage.googleapis.com/us_audioset/youtube_corpus/v1/csv/class_labels_indices.csv", labels)
    fonts = root / "share/fonts/tono"
    fonts.mkdir(parents=True, exist_ok=True)
    for font in json.loads((REPO / "scripts/music_font.json").read_text()):
        data = urllib.request.urlopen(font["url"]).read()
        if hashlib.sha256(data).hexdigest() != font["sha256"]:
            raise SystemExit("music font checksum mismatch")
        (fonts / font["name"]).write_bytes(data)
    for name in ["ml", "instruments"]:
        shutil.copytree(REPO / name, root / name, dirs_exist_ok=True,
                        ignore=shutil.ignore_patterns(".venv", "__pycache__", "*.pyc", "requirements.lock"))
    run(sys.executable, root / "ml/analyze.py", "--download-models", env=env)
    env["HF_HUB_OFFLINE"] = "1"
    result = subprocess.run([sys.executable, str(root / "ml/analyze.py"), "--self-check"],
                            env=env, check=True, capture_output=True, text=True)
    report = json.loads(result.stdout)
    if any(not item["ok"] for group in ("modules", "models") for item in report[group].values()):
        raise SystemExit(f"Runtime self-check failed: {report}")
    # Import success is insufficient: execute all three ML models on a short
    # synthetic signal. Musical quality remains covered by real-song review.
    run(sys.executable, REPO / "scripts/smoke_ml.py", root / "ml", env=env)
    inventory = root / "release-info"
    inventory.mkdir(exist_ok=True)
    (inventory / "self-check.json").write_text(json.dumps(report, indent=2))
    with (inventory / "pip-freeze.txt").open("w") as f:
        subprocess.run([sys.executable, "-m", "pip", "freeze", "--all"], stdout=f, check=True)
    distributions = []
    for dist in importlib.metadata.distributions():
        distributions.append({"name": dist.metadata["Name"], "version": dist.version,
                              "license": dist.metadata.get("License"),
                              "project_urls": dist.metadata.get_all("Project-URL", [])})
    (inventory / "python-packages.json").write_text(json.dumps(distributions, indent=2))
    # Conda metadata preserves exact package URLs/hashes. Keep license files
    # from the package cache as well (pip license files remain in dist-info).
    cache = os.environ.get("CONDA_PKGS_DIRS")
    if cache:
        for cache_dir in cache.split(os.pathsep):
            for licenses in Path(cache_dir).glob("*/info/licenses"):
                shutil.copytree(licenses, inventory / "licenses" / licenses.parent.parent.name,
                                dirs_exist_ok=True)
    shutil.copy2(REPO / "docs/DISTRIBUTION.md", inventory / "DISTRIBUTION.md")
    # Model cache symlinks need privileges on Windows; make model data regular
    # files everywhere. Conda-pack handles its own library symlinks separately.
    for path in (root / "models").rglob("*"):
        if path.is_symlink() and path.is_file():
            data = path.read_bytes()
            path.unlink()
            path.write_bytes(data)
    import conda_pack
    archive = out / f"tono-runtime-{args.target}.tar.gz"
    environment = conda_pack.CondaEnv.from_prefix(str(root))
    environment = environment.exclude("**/__pycache__/*").exclude("**/*.pyc")
    environment.pack(output=str(archive), force=True, compress_level=3)
    with tarfile.open(archive) as tar:
        unpacked = sum(item.size for item in tar)
    if archive.stat().st_size >= 2_000_000_000:
        raise SystemExit("runtime exceeds release asset limit; split before publishing")
    manifest = {"version": version, "target": args.target,
                "url": f"https://github.com/{args.repository}/releases/download/v{version}/{archive.name}",
                "sha256": hashlib.file_digest(archive.open("rb"), "sha256").hexdigest(),
                "bytes": archive.stat().st_size, "unpacked_bytes": unpacked}
    (out / f"runtime-{args.target}.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest, indent=2))


if __name__ == "__main__":
    main()

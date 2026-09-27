#!/usr/bin/env python3
"""
Tono ML worker. Isolated adapter: measures audio, writes versioned JSON.
Do not put Tono business logic or AE-01 fingering in this worker.

Modes:
  --self-check                       JSON availability report on stdout (tono doctor)
  --download-models                  fetch model weights (setup only)
  --detect-regions --input --output  per-frame speech/music/singing probabilities
  --input --part --output            separation + transcription (spec "ML interface")

File paths inside output JSON are absolute.
"""
from __future__ import annotations

import argparse
import contextlib
import importlib
import json
import os
import platform
import sys
from pathlib import Path

CONTRACT_VERSION = 1

# Actually imported (not just found) so broken installs are reported.
ML_MODULES = ("torch", "demucs", "basic_pitch", "resampy", "panns_inference", "soundfile", "librosa")

sys.path.insert(0, str(Path(__file__).resolve().parent))


def check_import(module_name: str) -> dict:
    try:
        # Some packages print on import; stdout must stay pure JSON.
        with contextlib.redirect_stdout(sys.stderr):
            importlib.import_module(module_name)
        return {"ok": True}
    except Exception as error:  # any failure means "not usable"
        return {"ok": False, "error": f"{type(error).__name__}: {error}"}


def self_check() -> None:
    report = {
        "version": CONTRACT_VERSION,
        "python": platform.python_version(),
        "modules": {name: check_import(name) for name in ML_MODULES},
        "models": {},
    }
    if report["modules"]["torch"]["ok"]:
        from tono_ml import models

        report["models"] = {
            "demucs htdemucs weights": models.demucs_cached_files(),
            "demucs htdemucs_6s weights": models.demucs_cached_files("htdemucs_6s"),
            "panns Cnn14_DecisionLevelMax": models.panns_checkpoint_status(),
            "basic_pitch ICASSP-2022 model": models.basic_pitch_model_status(),
        }
    else:
        report["models"] = {"ml models": {"ok": False, "detail": "ML packages not installed"}}
    print(json.dumps(report))


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")


def run_detect_regions(input_path: Path, output_path: Path) -> None:
    from tono_ml.regions import detect_region_frames

    write_json(output_path, detect_region_frames(input_path))


def run_analysis(input_path: Path, part: str, output_path: Path, work_dir: Path, out_dir: Path, model_name: str = "htdemucs", separate_only: bool = False) -> None:
    from tono_ml.audio_io import read_wav
    from tono_ml.separation import separate
    from tono_ml.tempo import estimate_beats
    from tono_ml.transcription import BASIC_PITCH_PARAMETERS, transcribe
    from tono_ml.pitch_track import measure_pitch_track
    from tono_ml import models

    samples, sample_rate = read_wav(input_path)
    if samples.shape[0] == 1:
        samples = samples.repeat(2, axis=0)  # Demucs expects stereo
    separation = separate(samples, sample_rate, part, work_dir, out_dir, model_name)

    backing_samples, _ = read_wav(separation.backing_file)
    bpm, beat_times = estimate_beats(backing_samples, sample_rate)
    notes = [] if separate_only else transcribe(separation.lead_file)
    # Evidence for Rust note cleanup (vibrato, slides, bleed).
    pitch_track = None if separate_only else measure_pitch_track(separation.lead_file)

    write_json(output_path, {
        "version": CONTRACT_VERSION,
        "sample_rate": sample_rate,
        "duration": round(samples.shape[1] / sample_rate, 6),
        "bpm": bpm,
        "beat_times": beat_times,
        "lead_file": str(separation.lead_file),
        "backing_file": str(separation.backing_file),
        "stems": {name: str(path) for name, path in separation.stems.items()},
        "notes": notes,
        "pitch_track": pitch_track,
        "separation": {
            "model": model_name,
            "device": models.torch_device(),
            "lead_stem": separation.lead_stem,
            "backing_stems": separation.backing_stems,
            "channels": separation.channels,
            "lead_to_mix_db": separation.lead_to_mix_db,
            "backing_to_mix_db": separation.backing_to_mix_db,
            "backing_gain_db": separation.backing_gain_db,
            "backing_gain_limited": separation.backing_gain_limited,
            "lead_selection": separation.lead_selection,
        },
        "transcription": {"model": "skipped-import" if separate_only else "basic-pitch ICASSP-2022", "parameters": {} if separate_only else BASIC_PITCH_PARAMETERS},
        "warnings": [],
    })


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--self-check", action="store_true")
    p.add_argument("--download-models", action="store_true")
    p.add_argument("--detect-regions", action="store_true")
    p.add_argument("--separate-only", action="store_true")
    p.add_argument("--separation-model", choices=["htdemucs", "htdemucs_6s"], default="htdemucs")
    p.add_argument("--input")
    p.add_argument("--part", default="vocal", choices=["vocal", "lead"])
    p.add_argument("--output")
    p.add_argument("--work-dir", help="stems and intermediates (default: next to --output)")
    p.add_argument("--out-dir", help="lead.wav / backing.wav destination (default: next to --output)")
    args = p.parse_args()

    if args.self_check:
        self_check()
        return
    if args.download_models:
        # Downloads are allowed only here.
        os.environ.pop("HF_HUB_OFFLINE", None)
        from tono_ml.models import download_models

        download_models()
        print("models ready", file=sys.stderr)
        return
    if not args.input or not args.output:
        p.error("--input and --output are required")

    input_path = Path(args.input).resolve()
    if not input_path.exists():
        raise SystemExit(f"input does not exist: {input_path}")
    output_path = Path(args.output).resolve()

    if args.detect_regions:
        run_detect_regions(input_path, output_path)
        return
    work_dir = Path(args.work_dir).resolve() if args.work_dir else output_path.parent
    out_dir = Path(args.out_dir).resolve() if args.out_dir else output_path.parent
    run_analysis(input_path, args.part, output_path, work_dir, out_dir, args.separation_model, args.separate_only)


if __name__ == "__main__":
    main()

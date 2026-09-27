"""Demucs stem separation and backing-track (BGM) mixing."""
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

import numpy as np

from . import models
from .audio_io import TARGET_PEAK_DBFS, energy_db, fit_length, normalize_peak, peak_dbfs, write_wav_24bit

# Lead stem per --part. htdemucs has 4 stems: vocals, drums, bass, other.
# DEFERRED: --part lead ASSUMES the "other" stem (all melodic instruments,
# pads and chords together) is the lead; nothing checks that it is. The
# assumption is reported as `lead_selection` in the output JSON.
LEAD_STEM_FOR_PART = {"vocal": "vocals", "lead": "other"}
LEAD_SELECTION_FOR_PART = {"vocal": "model-vocals-stem", "lead": "assumed-other-stem"}

# A real accompaniment rarely needs more than this to reach -1 dBFS. More
# gain than this can only mean the BGM is mostly residue; it stays quiet
# and Rust decides whether it is usable (see backing_to_mix_db).
MAX_BACKING_GAIN_DB = 12.0

SEPARATION_SEED = 0


@dataclass
class SeparationResult:
    stems: dict[str, Path]
    lead_file: Path
    backing_file: Path
    lead_stem: str
    backing_stems: list[str]
    sample_rate: int
    channels: int
    lead_to_mix_db: float
    backing_to_mix_db: float
    backing_gain_db: float
    backing_gain_limited: bool
    lead_selection: str


def separate(samples: np.ndarray, sample_rate: int, part: str, work_dir: Path, out_dir: Path) -> SeparationResult:
    import torch
    from demucs.api import Separator

    if part not in LEAD_STEM_FOR_PART:
        raise SystemExit(f"unsupported --part {part}")
    status = models.demucs_cached_files()
    if not status["ok"]:
        raise SystemExit(f"Demucs model unavailable: {status['detail']}")

    # Demucs' shift trick picks a random time offset (python `random`), and
    # the model may use torch RNG; seed both so a rerun gives identical stems.
    import random

    random.seed(SEPARATION_SEED)
    torch.manual_seed(SEPARATION_SEED)
    separator = Separator(model=models.DEMUCS_MODEL_NAME, device=models.torch_device())
    if separator.samplerate != sample_rate:
        raise SystemExit(f"expected {separator.samplerate} Hz input, got {sample_rate} Hz")
    _, stem_tensors = separator.separate_tensor(torch.from_numpy(samples), sample_rate)

    sample_count = samples.shape[1]
    stem_arrays = {name: fit_length(tensor.numpy().astype(np.float32), sample_count) for name, tensor in stem_tensors.items()}

    stems_dir = work_dir / "stems"
    stems_dir.mkdir(parents=True, exist_ok=True)
    stem_files = {}
    for name, stem in stem_arrays.items():
        stem_files[name] = stems_dir / f"{name}.wav"
        write_wav_24bit(stem_files[name], stem, sample_rate)

    lead_stem = LEAD_STEM_FOR_PART[part]
    backing_stems = [name for name in stem_arrays if name != lead_stem]
    # BGM = every non-selected stem (spec "Backing-track / BGM generation").
    backing = np.sum([stem_arrays[name] for name in backing_stems], axis=0)
    # Measured BEFORE normalization: this is what tells real accompaniment
    # apart from separation residue.
    backing_to_mix_db = energy_db(backing) - energy_db(samples)
    backing, backing_gain_db, backing_gain_limited = normalize_peak(backing, max_gain_db=MAX_BACKING_GAIN_DB)
    # The lead is NOT normalized: boosting a near-silent stem would turn
    # separation bleed into "melody". Only guard against clipping.
    lead = stem_arrays[lead_stem]
    if peak_dbfs(lead) > TARGET_PEAK_DBFS:
        lead, _, _ = normalize_peak(lead)

    # Reported, not judged: Rust decides what counts as low confidence.
    lead_to_mix_db = energy_db(stem_arrays[lead_stem]) - energy_db(samples)

    lead_file = out_dir / "lead.wav"
    backing_file = out_dir / "backing.wav"
    write_wav_24bit(lead_file, lead, sample_rate)
    write_wav_24bit(backing_file, backing, sample_rate)

    return SeparationResult(
        stems=stem_files,
        lead_file=lead_file,
        backing_file=backing_file,
        lead_stem=lead_stem,
        backing_stems=backing_stems,
        sample_rate=sample_rate,
        channels=int(backing.shape[0]),
        lead_to_mix_db=round(float(lead_to_mix_db), 2) if np.isfinite(lead_to_mix_db) else -120.0,
        backing_to_mix_db=round(float(backing_to_mix_db), 2) if np.isfinite(backing_to_mix_db) else -120.0,
        backing_gain_db=round(float(backing_gain_db), 2),
        backing_gain_limited=bool(backing_gain_limited),
        lead_selection=LEAD_SELECTION_FOR_PART[part],
    )

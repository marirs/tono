"""WAV reading/writing and level helpers (numpy arrays shaped [channels, samples])."""
from __future__ import annotations

from pathlib import Path

import numpy as np
import soundfile

# Headroom for exported stems so they never clip after AAC encoding.
TARGET_PEAK_DBFS = -1.0


def read_wav(path: Path) -> tuple[np.ndarray, int]:
    samples, sample_rate = soundfile.read(str(path), dtype="float32", always_2d=True)
    return samples.T.copy(), sample_rate


def write_wav_24bit(path: Path, samples: np.ndarray, sample_rate: int) -> None:
    """Lossless 24-bit PCM (spec: backing.wav is exported lossless)."""
    soundfile.write(str(path), samples.T, sample_rate, subtype="PCM_24")


def peak_dbfs(samples: np.ndarray) -> float:
    peak = float(np.max(np.abs(samples))) if samples.size else 0.0
    return 20.0 * np.log10(peak) if peak > 0 else float("-inf")


def energy_db(samples: np.ndarray) -> float:
    mean_square = float(np.mean(np.square(samples, dtype=np.float64))) if samples.size else 0.0
    return 10.0 * np.log10(mean_square) if mean_square > 0 else float("-inf")


def normalize_peak(
    samples: np.ndarray,
    target_dbfs: float = TARGET_PEAK_DBFS,
    max_gain_db: float = float("inf"),
) -> tuple[np.ndarray, float, bool]:
    """Scales toward the target peak, never boosting by more than max_gain_db.

    Returns (scaled, applied_gain_db, gain_was_limited). Silent input is
    returned unchanged. The cap exists so a near-silent residual (failed
    separation, unaccompanied vocal) is never blown up into loud noise.
    """
    current = peak_dbfs(samples)
    if not np.isfinite(current):
        return samples, 0.0, False
    wanted_gain_db = target_dbfs - current
    gain_db = min(wanted_gain_db, max_gain_db)
    scaled = (samples * (10.0 ** (gain_db / 20.0))).astype(np.float32)
    return scaled, gain_db, gain_db < wanted_gain_db


def fit_length(samples: np.ndarray, sample_count: int) -> np.ndarray:
    """Trims or zero-pads so every exported track matches the region exactly."""
    if samples.shape[1] >= sample_count:
        return samples[:, :sample_count]
    padding = np.zeros((samples.shape[0], sample_count - samples.shape[1]), dtype=samples.dtype)
    return np.concatenate([samples, padding], axis=1)

"""Tempo and beat hints from the backing track (librosa beat tracker)."""
from __future__ import annotations

import numpy as np


def estimate_beats(samples: np.ndarray, sample_rate: int) -> tuple[float | None, list[float]]:
    """Returns (bpm, beat_times_seconds); (None, []) when no steady beat is found."""
    import librosa

    mono = samples.mean(axis=0)
    if not np.any(mono):
        return None, []
    tempo, beat_frames = librosa.beat.beat_track(y=mono, sr=sample_rate)
    bpm = float(np.atleast_1d(tempo)[0])
    if len(beat_frames) < 4 or not np.isfinite(bpm) or bpm <= 0:
        return None, []
    beat_times = librosa.frames_to_time(beat_frames, sr=sample_rate)
    return round(bpm, 2), [round(float(t), 4) for t in beat_times]

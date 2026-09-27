"""Monophonic pitch evidence for note cleanup (librosa pYIN on the lead stem).

Measurement only: per frame, the fundamental (as fractional MIDI), the
probability that the frame is voiced, and the frame level. Rust uses these
to tell sung notes from separation bleed, vibrato and slides; no decision
is made here.
"""
from __future__ import annotations

from pathlib import Path

import numpy as np

from .audio_io import read_wav

ANALYSIS_SAMPLE_RATE = 22050
HOP_LENGTH = 512  # 23.2 ms: enough frames for the shortest kept notes (60 ms)
FRAME_LENGTH = 2048
# Same melody range as transcription (65-1400 Hz).
MIN_F0_HZ = 65.0
MAX_F0_HZ = 1400.0


def measure_pitch_track(lead_file: Path) -> dict:
    import librosa

    samples, sample_rate = read_wav(lead_file)
    mono = samples.mean(axis=0)
    mono = librosa.resample(mono, orig_sr=sample_rate, target_sr=ANALYSIS_SAMPLE_RATE)
    f0_hz, _, voiced_probability = librosa.pyin(
        mono,
        fmin=MIN_F0_HZ,
        fmax=MAX_F0_HZ,
        sr=ANALYSIS_SAMPLE_RATE,
        frame_length=FRAME_LENGTH,
        hop_length=HOP_LENGTH,
    )
    rms = librosa.feature.rms(y=mono, frame_length=FRAME_LENGTH, hop_length=HOP_LENGTH)[0]
    frame_count = min(len(f0_hz), len(rms))
    f0_midi = librosa.hz_to_midi(f0_hz[:frame_count])
    level_db = 20.0 * np.log10(np.maximum(rms[:frame_count], 1e-9))
    return {
        "method": "librosa pyin",
        "hop_seconds": HOP_LENGTH / ANALYSIS_SAMPLE_RATE,
        # Frame i is centred at i * hop_seconds (librosa's centred frames).
        "f0_midi": [None if not np.isfinite(v) else round(float(v), 3) for v in f0_midi],
        "voiced_probability": [round(float(v), 3) for v in voiced_probability[:frame_count]],
        "level_db": [round(float(v), 2) for v in level_db],
    }

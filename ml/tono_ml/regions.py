"""Frame-level speech / music / singing measurements for song-region detection.

Uses PANNs Cnn14_DecisionLevelMax (AudioSet sound event detection). This
module only reports probabilities per frame; Rust decides the region.
"""
from __future__ import annotations

import contextlib
import sys
from pathlib import Path

import numpy as np

from . import models
from .audio_io import read_wav

PANNS_SAMPLE_RATE = 32000
FRAME_SECONDS = 0.25

# AudioSet classes grouped into the three signals Rust needs. Speech covers
# talking styles; singing covers sung vocals; music covers the generic class.
SPEECH_CLASSES = (
    "Speech", "Male speech, man speaking", "Female speech, woman speaking",
    "Child speech, kid speaking", "Conversation", "Narration, monologue",
)
SINGING_CLASSES = ("Singing", "Male singing", "Female singing", "Child singing", "Choir")
MUSIC_CLASSES = ("Music", "Musical instrument")


def _load_detector():
    status = models.panns_checkpoint_status()
    if not status["ok"]:
        raise SystemExit(f"PANNs checkpoint unavailable: {status['detail']}")
    # panns_inference prints progress to stdout; keep stdout clean for callers.
    with contextlib.redirect_stdout(sys.stderr):
        from panns_inference import SoundEventDetection

        return SoundEventDetection(checkpoint_path=str(models.PANNS_SED_CHECKPOINT), device=models.torch_device())


def _class_indices(names: tuple[str, ...]) -> list[int]:
    from panns_inference import labels

    missing = [name for name in names if name not in labels]
    if missing:
        raise SystemExit(f"AudioSet labels not found: {missing}")
    return [labels.index(name) for name in names]


def detect_region_frames(input_path: Path) -> dict:
    import librosa

    samples, sample_rate = read_wav(input_path)
    mono = samples.mean(axis=0)
    mono_32k = librosa.resample(mono, orig_sr=sample_rate, target_sr=PANNS_SAMPLE_RATE).astype(np.float32)

    detector = _load_detector()
    with contextlib.redirect_stdout(sys.stderr):
        framewise = detector.inference(mono_32k[None, :])[0]  # (time_steps, 527)

    return region_frames_document(
        mono,
        sample_rate,
        framewise,
        speech_index=_class_indices(SPEECH_CLASSES),
        singing_index=_class_indices(SINGING_CLASSES),
        music_index=_class_indices(MUSIC_CLASSES),
    )


def region_frames_document(mono, sample_rate, framewise, speech_index, singing_index, music_index) -> dict:
    """The region-report JSON contract (see src/analysis/region.rs).

    Pure function of the mono signal and the detector's (time_steps, classes)
    probabilities, so the contract is testable without PANNs or audio files:
    contiguous 0.25 s frames covering the duration, RMS level in dBFS
    (-120 for silence) and, per group, the strongest class averaged per frame.
    """
    duration = mono.shape[0] / sample_rate
    seconds_per_step = duration / framewise.shape[0]
    frames = []
    frame_count = int(np.ceil(duration / FRAME_SECONDS))
    for frame_number in range(frame_count):
        start = frame_number * FRAME_SECONDS
        end = min(start + FRAME_SECONDS, duration)
        step_slice = slice(int(start / seconds_per_step), max(int(start / seconds_per_step) + 1, int(end / seconds_per_step)))
        steps = framewise[step_slice]
        sample_slice = mono[int(start * sample_rate):int(end * sample_rate)]
        mean_square = float(np.mean(np.square(sample_slice, dtype=np.float64))) if sample_slice.size else 0.0
        frames.append({
            "start": round(start, 4),
            "end": round(end, 4),
            "rms_db": round(10 * np.log10(mean_square), 2) if mean_square > 0 else -120.0,
            # Within each group take the strongest class, averaged over the frame.
            "speech": round(float(steps[:, speech_index].max(axis=1).mean()), 4),
            "singing": round(float(steps[:, singing_index].max(axis=1).mean()), 4),
            "music": round(float(steps[:, music_index].max(axis=1).mean()), 4),
        })

    return {
        "version": 1,
        "duration": round(duration, 4),
        "model": "panns Cnn14_DecisionLevelMax (AudioSet)",
        "frames": frames,
    }

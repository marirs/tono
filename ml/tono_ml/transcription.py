"""Basic Pitch melody transcription of the isolated lead stem."""
from __future__ import annotations

import contextlib
import sys
from pathlib import Path

# Parameters are reported in analysis.json so results are reproducible.
BASIC_PITCH_PARAMETERS = {
    "onset_threshold": 0.5,
    "frame_threshold": 0.3,
    "minimum_note_length_ms": 58.0,
    # Sung/lead melody range; excludes sub-bass rumble and sibilance.
    "minimum_frequency_hz": 65.0,
    "maximum_frequency_hz": 1400.0,
}


# Frames either side of a note start searched for its onset activation.
ATTACK_SEARCH_FRAMES = 2


def attack_strength(onsets, frame_times, start: float, midi: int) -> float:
    """Peak onset activation for this pitch around the note start.

    Rust uses it for spec rule 5: a same-pitch neighbour with a real attack
    is a repeated note, not a transcription break to merge.
    """
    import numpy as np

    from basic_pitch.note_creation import MIDI_OFFSET

    pitch_bin = midi - MIDI_OFFSET
    if not 0 <= pitch_bin < onsets.shape[1]:
        return 0.0
    centre = int(np.argmin(np.abs(frame_times - start)))
    window = onsets[max(0, centre - ATTACK_SEARCH_FRAMES):centre + ATTACK_SEARCH_FRAMES + 1, pitch_bin]
    return float(window.max()) if window.size else 0.0


def transcribe(lead_file: Path) -> list[dict]:
    """Returns raw (unclean, possibly overlapping) note events in seconds."""
    with contextlib.redirect_stdout(sys.stderr):
        from basic_pitch import ICASSP_2022_MODEL_PATH
        from basic_pitch.inference import predict

        model_output, _, note_events = predict(
            str(lead_file),
            model_or_model_path=ICASSP_2022_MODEL_PATH,
            onset_threshold=BASIC_PITCH_PARAMETERS["onset_threshold"],
            frame_threshold=BASIC_PITCH_PARAMETERS["frame_threshold"],
            minimum_note_length=BASIC_PITCH_PARAMETERS["minimum_note_length_ms"],
            minimum_frequency=BASIC_PITCH_PARAMETERS["minimum_frequency_hz"],
            maximum_frequency=BASIC_PITCH_PARAMETERS["maximum_frequency_hz"],
            multiple_pitch_bends=False,
            melodia_trick=True,
        )
    from basic_pitch.note_creation import model_frames_to_time

    onsets = model_output["onset"]
    frame_times = model_frames_to_time(onsets.shape[0])
    notes = [
        {
            "start": round(float(start), 4),
            "end": round(float(end), 4),
            "midi": int(pitch),
            # Basic Pitch's mean note activation, used as confidence.
            "confidence": round(float(amplitude), 4),
            "attack": round(attack_strength(onsets, frame_times, float(start), int(pitch)), 4),
        }
        for start, end, pitch, amplitude, _bends in note_events
    ]
    return sorted(notes, key=lambda note: (note["start"], note["midi"]))

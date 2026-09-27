"""Release gate: execute detection, separation, transcription and pitch evidence."""
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import numpy as np
import soundfile as sf
from tono_ml import models
from tono_ml.regions import detect_region_frames
from tono_ml.transcription import transcribe
from tono_ml.pitch_track import measure_pitch_track
from demucs.pretrained import get_model
from demucs.apply import apply_model
import torch

with tempfile.TemporaryDirectory(prefix="tono-ml-smoke-") as tmp:
    path = Path(tmp) / "signal.wav"
    t = np.arange(44100 * 4) / 44100
    signal = (0.2 * np.sin(2 * np.pi * 440 * t)).astype(np.float32)
    sf.write(path, signal, 44100)
    frames = detect_region_frames(path)
    assert frames["frames"], "region detector returned no frames"
    model = get_model(models.DEMUCS_MODEL_NAME).cpu().eval()
    with torch.no_grad():
        stems = apply_model(model, torch.from_numpy(np.stack([signal, signal]))[None],
                            device="cpu", shifts=0, num_workers=0)
    assert torch.isfinite(stems).all(), "nonfinite separated samples"
    assert transcribe(path), "Basic Pitch failed to detect a sustained A4"
    assert measure_pitch_track(path)["f0_midi"], "pitch evidence absent"
print("All ML engines executed successfully")

"""Model identities, local-availability checks and downloads.

Runs never download: `tono prep` sets HF_HUB_OFFLINE and every loader
checks the local file first, so a missing model fails loudly instead of
stalling a run on the network. `analyze.py --download-models` is the only
code path allowed to fetch weights.
"""
from __future__ import annotations

import os
from pathlib import Path

DEMUCS_MODEL_NAME = "htdemucs"
DEMUCS_HF_REPO = "adefossez/HTDemucs"  # demucs>=4.1 loads bags from the HF hub

# PANNs Cnn14 with frame-level output (AudioSet sound event detection).
PANNS_SED_CHECKPOINT = Path(os.environ.get("TONO_PANNS_DIR", Path.home() / "panns_data")) / "Cnn14_DecisionLevelMax.pth"
PANNS_SED_URL = "https://zenodo.org/record/3987831/files/Cnn14_DecisionLevelMax_mAP%3D0.385.pth?download=1"
PANNS_SED_MIN_BYTES = 3e8  # same sanity bound panns_inference uses

TORCH_DEVICE_ENV = "TONO_TORCH_DEVICE"


def torch_device() -> str:
    """CPU by default for reproducible output; override with TONO_TORCH_DEVICE=mps."""
    return os.environ.get(TORCH_DEVICE_ENV, "cpu")


def demucs_cached_files() -> dict:
    """Returns {"ok": bool, "detail": str} without touching the network."""
    try:
        import yaml
        from huggingface_hub import try_to_load_from_cache

        bag_yaml = try_to_load_from_cache(DEMUCS_HF_REPO, f"{DEMUCS_MODEL_NAME}.yaml")
        if not isinstance(bag_yaml, str):
            return {"ok": False, "detail": f"{DEMUCS_MODEL_NAME}.yaml not cached (run ml/setup_venv.sh)"}
        signatures = yaml.safe_load(Path(bag_yaml).read_text())["models"]
        missing = [
            signature
            for signature in signatures
            if not isinstance(try_to_load_from_cache(DEMUCS_HF_REPO, f"{signature}.safetensors"), str)
        ]
        if missing:
            return {"ok": False, "detail": f"weights {missing} not cached (run ml/setup_venv.sh)"}
        return {"ok": True, "detail": f"{DEMUCS_HF_REPO} {signatures}"}
    except Exception as error:
        return {"ok": False, "detail": f"{type(error).__name__}: {error}"}


def panns_checkpoint_status() -> dict:
    checkpoint = PANNS_SED_CHECKPOINT
    if not checkpoint.exists() or checkpoint.stat().st_size < PANNS_SED_MIN_BYTES:
        return {"ok": False, "detail": f"{checkpoint} missing or incomplete (run ml/setup_venv.sh)"}
    return {"ok": True, "detail": str(checkpoint)}


def basic_pitch_model_status() -> dict:
    try:
        from basic_pitch import ICASSP_2022_MODEL_PATH

        model_path = Path(ICASSP_2022_MODEL_PATH)
        if not model_path.exists():
            return {"ok": False, "detail": f"model missing at {model_path}"}
        return {"ok": True, "detail": str(model_path)}
    except Exception as error:
        return {"ok": False, "detail": f"{type(error).__name__}: {error}"}


def download_models() -> None:
    """Fetch every model once. Only called by setup, never during a run."""
    import urllib.request

    from demucs.pretrained import get_model

    get_model(DEMUCS_MODEL_NAME)
    if not panns_checkpoint_status()["ok"]:
        PANNS_SED_CHECKPOINT.parent.mkdir(parents=True, exist_ok=True)
        partial = PANNS_SED_CHECKPOINT.with_suffix(".partial")
        urllib.request.urlretrieve(PANNS_SED_URL, partial)
        partial.rename(PANNS_SED_CHECKPOINT)
    for status in (demucs_cached_files(), panns_checkpoint_status(), basic_pitch_model_status()):
        if not status["ok"]:
            raise SystemExit(f"model download incomplete: {status['detail']}")

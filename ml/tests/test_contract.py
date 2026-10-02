"""Lightweight ML worker contract tests: no models, no inference, no audio.

The Rust side parses the same fixture (src/analysis/worker.rs), so a change to
the JSON the worker writes must update `fixtures/analysis_contract_v1.json`
and pass both test suites.

Run: python -m unittest discover ml/tests   (needs numpy and soundfile only)
Regenerate the fixtures after an intentional contract change:
    TONO_UPDATE_CONTRACT_FIXTURE=1 python -m unittest ml/tests/test_contract.py
"""
import io
import json
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import numpy as np

ML_DIR = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ML_DIR))
import analyze  # noqa: E402
from tono_ml.audio_io import fit_length, normalize_peak, peak_dbfs  # noqa: E402
from tono_ml.pitch_track import pitch_track_document  # noqa: E402

FIXTURE = Path(__file__).resolve().parent / "fixtures" / "analysis_contract_v1.json"
REGIONS_FIXTURE = Path(__file__).resolve().parent / "fixtures" / "regions_contract_v1.json"


def shape(value):
    """Structure of a JSON value: dict keys recursively, list element shape, type names."""
    if isinstance(value, dict):
        return {key: shape(item) for key, item in sorted(value.items())}
    if isinstance(value, list):
        kinds = {json.dumps(shape(item), sort_keys=True) for item in value}
        return ["list", sorted(kinds)]
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, (int, float)):
        return "number"
    return type(value).__name__


def analysis_document(root: Path) -> dict:
    """Runs the real `run_analysis` with every model replaced by fixed values."""
    separated = SimpleNamespace(
        backing_file=root / "backing.wav", lead_file=root / "lead.wav",
        stems={"vocals": root / "stems/vocals.wav", "drums": root / "stems/drums.wav"},
        lead_stem="vocals", backing_stems=["drums"], channels=2,
        lead_to_mix_db=-3.0, backing_to_mix_db=-5.0, backing_gain_db=2.5,
        backing_gain_limited=False, lead_selection="model-vocals-stem",
    )
    notes = [
        {"start": 0.5, "end": 1.0, "midi": 62, "confidence": 0.8, "attack": 0.9},
        {"start": 1.0, "end": 1.5, "midi": 64, "confidence": 0.7, "attack": 0.4},
    ]
    # A pitch track covering the 2 s region, as the Rust validator requires.
    frames = 86  # 86 x 23.2 ms hop = 1.997 s
    f0 = np.where(np.arange(frames) < 43, 62.0, 64.0); f0[20] = np.nan
    track = pitch_track_document(f0, np.full(frames, 0.8), np.full(frames, -12.0))
    with patch("tono_ml.audio_io.read_wav", return_value=(np.zeros((2, 44100 * 2)), 44100)), \
         patch("tono_ml.separation.separate", return_value=separated), \
         patch("tono_ml.tempo.estimate_beats", return_value=(96.0, [0.3, 0.925, 1.55])), \
         patch("tono_ml.transcription.transcribe", return_value=notes), \
         patch("tono_ml.pitch_track.measure_pitch_track", return_value=track):
        analyze.run_analysis(root / "region.wav", "vocal", root / "analysis.json", root, root, "htdemucs", False)
    return json.loads((root / "analysis.json").read_text())


def portable(document: dict, root: Path) -> dict:
    """Absolute temp paths -> file names, so the fixture is machine-independent."""
    text = json.dumps(document).replace(str(root) + os.sep, "").replace(str(root), ".")
    return json.loads(text)


class AnalysisContractTests(unittest.TestCase):
    def test_worker_output_matches_the_shared_contract_fixture(self):
        with tempfile.TemporaryDirectory() as tmp:
            document = portable(analysis_document(Path(tmp)), Path(tmp))
        if os.environ.get("TONO_UPDATE_CONTRACT_FIXTURE"):
            FIXTURE.parent.mkdir(exist_ok=True)
            FIXTURE.write_text(json.dumps(document, indent=2) + "\n")
        fixture = json.loads(FIXTURE.read_text())
        self.assertEqual(shape(document), shape(fixture),
                         "worker JSON changed shape: update the fixture and the Rust parser together")
        self.assertEqual(document["version"], analyze.CONTRACT_VERSION)

    def test_pitch_track_document_is_aligned_and_json_safe(self):
        doc = pitch_track_document(np.array([60.0, np.nan, np.inf, 61.5]),
                                   np.array([0.9, 0.2, 0.3]), np.array([-10.0, -40.0, -45.0, -9.0]))
        lengths = {len(doc[key]) for key in ("f0_midi", "voiced_probability", "level_db")}
        self.assertEqual(lengths, {3}, "frame arrays must be equally long")
        self.assertEqual(doc["f0_midi"], [60.0, None, None])
        self.assertGreater(doc["hop_seconds"], 0.0)
        json.dumps(doc, allow_nan=False)  # no NaN/Infinity may reach the JSON


def region_report() -> dict:
    """Runs the real `detect_region_frames` with PANNs, its labels and librosa
    mocked: 8 s of audio, 2 s quiet talking then 6 s of louder music."""
    from tono_ml import regions

    sample_rate, seconds = 8000, 8.0
    t = np.arange(int(sample_rate * seconds)) / sample_rate
    mono = np.where(t < 2.0, 0.05, 0.4) * np.sin(2 * np.pi * 220 * t)
    steps = 800  # detector time steps (10 ms)
    framewise = np.zeros((steps, 6))
    framewise[:200, 0] = 0.8   # speech group, class 0
    framewise[:200, 1] = 0.3   # speech group, class 1 (weaker)
    framewise[200:, 4] = 0.7   # music group
    framewise[200:, 2] = 0.2   # singing group
    detector = SimpleNamespace(inference=lambda audio: np.array([framewise]))
    fake_librosa = SimpleNamespace(resample=lambda y, orig_sr, target_sr: y)
    indices = {regions.SPEECH_CLASSES: [0, 1], regions.SINGING_CLASSES: [2, 3], regions.MUSIC_CLASSES: [4, 5]}
    with patch.dict(sys.modules, {"librosa": fake_librosa}), \
         patch.object(regions, "read_wav", return_value=(np.stack([mono, mono]), sample_rate)), \
         patch.object(regions, "_load_detector", return_value=detector), \
         patch.object(regions, "_class_indices", side_effect=lambda names: indices[names]):
        return regions.detect_region_frames(Path("source.wav"))


class RegionContractTests(unittest.TestCase):
    def test_region_report_matches_the_shared_contract_fixture(self):
        report = region_report()
        if os.environ.get("TONO_UPDATE_CONTRACT_FIXTURE"):
            REGIONS_FIXTURE.write_text(json.dumps(report, indent=2) + "\n")
        fixture = json.loads(REGIONS_FIXTURE.read_text())
        self.assertEqual(shape(report), shape(fixture),
                         "region JSON changed shape: update the fixture and the Rust parser together")
        self.assertEqual(report["version"], 1)
        json.dumps(report, allow_nan=False)

    def test_region_frames_are_contiguous_and_take_each_groups_strongest_class(self):
        report = region_report()
        frames = report["frames"]
        self.assertEqual(len(frames), 32)  # 8 s / 0.25 s
        self.assertEqual(frames[0]["start"], 0.0)
        self.assertEqual(frames[-1]["end"], report["duration"])
        self.assertTrue(all(a["end"] == b["start"] for a, b in zip(frames, frames[1:])))
        for frame in frames:
            for key in ("speech", "singing", "music"):
                self.assertTrue(0.0 <= frame[key] <= 1.0)
        talk, music = frames[2], frames[20]
        self.assertEqual((talk["speech"], talk["music"]), (0.8, 0.0))  # max of 0.8 and 0.3
        self.assertEqual((music["speech"], music["music"], music["singing"]), (0.0, 0.7, 0.2))
        self.assertLess(talk["rms_db"], music["rms_db"])

    def test_silence_is_reported_as_minus_120_db(self):
        from tono_ml.regions import region_frames_document

        doc = region_frames_document(np.zeros(4000), 8000, np.zeros((50, 2)), [0], [1], [1])
        self.assertTrue(all(frame["rms_db"] == -120.0 for frame in doc["frames"]))


class SelfCheckContractTests(unittest.TestCase):
    def test_self_check_reports_missing_ml_packages_as_structured_json(self):
        out = io.StringIO()
        with patch.object(analyze, "check_import", return_value={"ok": False, "error": "ModuleNotFoundError: x"}), \
             redirect_stdout(out):
            analyze.self_check()
        report = json.loads(out.getvalue())
        self.assertEqual(report["version"], analyze.CONTRACT_VERSION)
        self.assertEqual(set(report["modules"]), set(analyze.ML_MODULES))
        self.assertTrue(all(status["ok"] is False for status in report["modules"].values()))
        self.assertTrue(all("ok" in status for status in report["models"].values()))


class AudioHelperTests(unittest.TestCase):
    def test_normalize_peak_caps_gain_on_near_silent_input(self):
        quiet = np.full((2, 100), 10 ** (-90 / 20), dtype=np.float32)
        scaled, gain, limited = normalize_peak(quiet, max_gain_db=12.0)
        self.assertTrue(limited)
        self.assertAlmostEqual(gain, 12.0)
        self.assertLess(peak_dbfs(scaled), -70.0)
        loud = np.full((2, 100), 0.5, dtype=np.float32)
        _, gain, limited = normalize_peak(loud, max_gain_db=12.0)
        self.assertFalse(limited)
        self.assertAlmostEqual(gain, -1.0 - 20 * np.log10(0.5), places=4)

    def test_fit_length_trims_and_pads(self):
        samples = np.ones((2, 5), dtype=np.float32)
        self.assertEqual(fit_length(samples, 3).shape, (2, 3))
        padded = fit_length(samples, 8)
        self.assertEqual(padded.shape, (2, 8))
        self.assertEqual(float(padded[:, 5:].sum()), 0.0)


if __name__ == "__main__":
    unittest.main()

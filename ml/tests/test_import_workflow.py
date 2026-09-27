"""Import mode must separate without invoking either transcription engine."""
import json
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import analyze


class ImportWorkflowTests(unittest.TestCase):
    def test_separation_only_skips_pitch_models_and_reports_selected_model(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            separated = SimpleNamespace(
                backing_file=root / "backing.wav", lead_file=root / "lead.wav",
                stems={}, lead_stem="other", backing_stems=["drums", "bass", "guitar", "piano", "vocals"],
                channels=2, lead_to_mix_db=-1.0, backing_to_mix_db=-10.0,
                backing_gain_db=6.0, backing_gain_limited=True,
                lead_selection="assumed-other-stem",
            )
            with patch("tono_ml.audio_io.read_wav", return_value=(np.zeros((2, 44100)), 44100)), \
                 patch("tono_ml.separation.separate", return_value=separated) as separate, \
                 patch("tono_ml.tempo.estimate_beats", return_value=(None, [])), \
                 patch("tono_ml.transcription.transcribe", side_effect=AssertionError("ML transcription called")), \
                 patch("tono_ml.pitch_track.measure_pitch_track", side_effect=AssertionError("pitch tracker called")):
                analyze.run_analysis(root / "source.wav", "lead", root / "analysis.json", root, root, "htdemucs_6s", True)
            report = json.loads((root / "analysis.json").read_text())
            self.assertEqual(separate.call_args.args[-1], "htdemucs_6s")
            self.assertEqual(report["separation"]["model"], "htdemucs_6s")
            self.assertEqual(report["transcription"]["model"], "skipped-import")
            self.assertEqual(report["notes"], [])
            self.assertIsNone(report["pitch_track"])

    def test_six_source_cache_check_uses_its_own_repository(self):
        from tono_ml import models
        with tempfile.TemporaryDirectory() as tmp:
            bag = Path(tmp) / "htdemucs_6s.yaml"
            bag.write_text("models: ['5c90dfd2']")
            def cache(repo, filename):
                self.assertEqual(repo, "adefossez/HTDemucs-6s")
                return str(bag) if filename.endswith("yaml") else str(Path(tmp) / filename)
            with patch("huggingface_hub.try_to_load_from_cache", side_effect=cache):
                self.assertTrue(models.demucs_cached_files("htdemucs_6s")["ok"])
            with patch("huggingface_hub.try_to_load_from_cache", return_value=None):
                self.assertFalse(models.demucs_cached_files("htdemucs_6s")["ok"])


if __name__ == "__main__":
    unittest.main()

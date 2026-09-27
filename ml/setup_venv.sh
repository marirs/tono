#!/usr/bin/env bash
# Creates ml/.venv with Python 3.11 and installs the ML worker dependencies.
#
# Python 3.11 is required, not merely preferred: basic-pitch 0.4.0 on macOS
# depends on tensorflow-macos<2.15.1 for Python > 3.11, which has no wheels
# for 3.12+, while on <= 3.11 it uses the CoreML backend via coremltools.
set -euo pipefail

ML_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYTHON_BIN="${TONO_BOOTSTRAP_PYTHON:-/opt/homebrew/bin/python3.11}"

if ! "$PYTHON_BIN" -c 'import sys; assert sys.version_info[:2] == (3, 11)' 2>/dev/null; then
  echo "error: need Python 3.11 at $PYTHON_BIN (brew install python@3.11, or set TONO_BOOTSTRAP_PYTHON)" >&2
  exit 1
fi

"$PYTHON_BIN" -m venv "$ML_DIR/.venv"
"$ML_DIR/.venv/bin/python" -m pip install --upgrade pip
"$ML_DIR/.venv/bin/python" -m pip install -r "$ML_DIR/requirements.txt"
"$ML_DIR/.venv/bin/python" -m pip freeze --all > "$ML_DIR/requirements.lock"

# Download model weights now so `tono prep` never downloads mid-run.
"$ML_DIR/.venv/bin/python" "$ML_DIR/analyze.py" --download-models

echo "ML environment ready: $ML_DIR/.venv"

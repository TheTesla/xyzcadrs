#!/usr/bin/env bash
# One-command installer for the xyzcadrs Python bindings.
#
# Usage (from any directory):
#   bash /path/to/xyzcadrs/tools/install_python_binding.sh [target-venv]
#
# - Without argument: creates/uses ./xyzcadrs-venv in the current directory.
# - With argument:    uses the given existing virtualenv.
#
# Installs the compiled Python package "xyzcadrs" (which bundles the Rust
# renderer as a native module). Requires: python3, pip. Rust (cargo) is
# installed automatically if missing — it is only a build tool, you never
# touch it again afterwards.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="${1:-$PWD/xyzcadrs-venv}"

if [ "${1:-}" = "" ]; then
    if [ ! -d "$VENV_DIR" ]; then
        # PyO3 supports Python 3.9-3.13; pick the best available interpreter
        # explicitly (plain python3 may be too new).
        PYBIN=""
        for cand in python3.13 python3.12 python3.11 python3.10 python3.9 python3; do
            if command -v "$cand" >/dev/null 2>&1; then
                ver=$("$cand" -c 'import sys; print(f"{sys.version_info[0]}.{sys.version_info[1]}")')
                ok=$("$cand" -c 'import sys; print(0 if (3, 9) <= sys.version_info[:2] <= (3, 13) else 1)')
                if [ "$ok" = "0" ]; then
                    PYBIN="$cand"
                    echo "==> Using $cand (Python $ver)"
                    break
                fi
            fi
        done
        if [ "$PYBIN" = "" ]; then
            echo "error: no Python 3.9-3.13 found (python3 is $(python3 -V 2>&1)); \
PyO3 does not support 3.14+ yet" >&2
            exit 1
        fi
        echo "==> Creating virtualenv at $VENV_DIR"
        "$PYBIN" -m venv "$VENV_DIR"
    fi
fi

if [ ! -x "$VENV_DIR/bin/python" ]; then
    echo "error: $VENV_DIR is not a virtualenv (no bin/python)" >&2
    exit 1
fi

PIP="$VENV_DIR/bin/pip"
PYTHON="$VENV_DIR/bin/python"

echo "==> Installing build dependencies (maturin, numba, numpy)"
"$PIP" install --quiet --upgrade pip
"$PIP" install --quiet maturin numba numpy

if ! command -v cargo >/dev/null 2>&1; then
    echo "==> Rust is not installed (only needed to compile, once)"
    if command -v rustup >/dev/null 2>&1; then
        rustup default stable
    else
        echo "    Installing rustup to ~/.cargo and ~/.rustup (no sudo needed)"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        export PATH="$HOME/.cargo/bin:$PATH"
    fi
fi

echo "==> Building and installing xyzcadrs into $VENV_DIR (compiles ~1-2 min)"
"$PIP" install --quiet "$REPO_DIR/python"

echo "==> Self-test"
export VENV_DIR
"$PYTHON" - <<'EOF'
import os

from numba import njit

import xyzcadrs

@njit
def sphere(p):
    x, y, z = p[:3]
    return 1 if 100.0 > x * x + y * y + z * z else 0

summary = xyzcadrs.render_and_save(sphere, "/tmp/xyzcadrs_selftest.stl", 2.0)
assert summary["cubes"] == 452, summary
stats = xyzcadrs.validate("/tmp/xyzcadrs_selftest.stl")
assert stats["watertight"], stats
print(f"xyzcadrs {xyzcadrs.__version__} OK "
      f"(self-test: sphere 452 cubes, watertight={stats['watertight']})")
print("Activate the environment with:  source "
      f"{os.path.abspath(os.path.join(os.environ['VENV_DIR'], 'bin/activate'))}")
EOF

echo "==> Done. Use it in your scripts with:  import xyzcadrs"

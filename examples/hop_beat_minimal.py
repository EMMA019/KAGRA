"""Hop Beat: a one-button rhythm hopper. Spec: kagra/hop_beat.py (Rust sim in kagra-shared/src/hop_beat.rs).

The game runs in the browser (``scripts/preview_site/play.html?game=hop_beat``) and in the
Rust shells. From Python this file only checks and bakes the spec:

    python examples/hop_beat_minimal.py --check   # fixture still matches the defaults
    python examples/hop_beat_minimal.py --bake    # rewrite kagra-shared/tests/fixtures/hop_beat_world.json
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from kagra.hop_beat import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:] or ["--check"]))

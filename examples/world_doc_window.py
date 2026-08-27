#!/usr/bin/env python3
"""Desktop wgpu 30 window for a World.dump JSON.

Thin alias for ``python -m kagra.play_world`` / ``python kagra/play_world.py``.
Does not import ``kagra`` (no kagra-core / RendererV2). Crest Isle VRM play
stays on the old window; this is the shared-runtime wedge (capsules / boxes).
"""
from __future__ import annotations

import runpy
from pathlib import Path

if __name__ == "__main__":
    play = Path(__file__).resolve().parents[1] / "kagra" / "play_world.py"
    runpy.run_path(str(play), run_name="__main__")

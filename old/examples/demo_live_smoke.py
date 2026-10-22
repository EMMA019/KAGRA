"""Archived RendererV2 live-demo smoke.

The scenario lives next to the other old smokes
(``old/examples/verify_scenarios/demo_live_smoke.json``). The mainline
``examples/verify_scenarios/`` set does not reference this script.

    python old/examples/demo_live_smoke.py
    # → scratch/demo_live.png
"""
from __future__ import annotations

import os
import sys

# old/examples/<file>.py → repository root
_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path.insert(0, _ROOT)

from kagra.demo import main

raise SystemExit(
    main(
        [
            "--offline",
            "--hidden",
            "--no-orbit",
            "--width",
            "640",
            "--height",
            "360",
            "--max-frames",
            "120",
            "--screenshot",
            os.path.join("scratch", "demo_live.png"),
        ]
    )
)

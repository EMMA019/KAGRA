"""python -m kagra ライブデモの自己検証。

    python examples/demo_live_smoke.py
    # → scratch/demo_live.png
"""
from __future__ import annotations

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

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

"""Generate tests/fixtures/synthetic_walk.bvh — simple biped walk cycle."""
from __future__ import annotations

import math
from pathlib import Path

# Reuse hierarchy from dance generator
from gen_synthetic_bvh import HIER, N, I_SPINE, I_LARM, I_LFORE, I_RARM, I_RFORE

OUT = Path(__file__).resolve().parents[1] / "tests" / "fixtures" / "synthetic_walk.bvh"

FRAMES = 32
FRAME_TIME = 1.0 / 30.0

# legs / feet channel starts
I_LUP = 42
I_LLEG = 45
I_LFOOT = 48
I_RUP = 51
I_RLEG = 54
I_RFOOT = 57


def main() -> None:
    lines = [HIER.rstrip(), "MOTION", f"Frames: {FRAMES}", f"Frame Time: {FRAME_TIME:.6f}"]
    for i in range(FRAMES):
        f = [0.0] * N
        f[0], f[1], f[2] = 0.0, 100.0, 0.0
        t = i / FRAMES * 2 * math.pi

        # Hips: slight bob (Y) + roll/yaw
        f[1] = 100.0 + 1.2 * abs(math.sin(t * 2))
        f[3] = 4.0 * math.sin(t)          # Zrot roll
        f[4] = 3.0 + 2.0 * math.sin(t * 2)  # Xrot lean
        f[5] = 3.0 * math.sin(t)          # Yrot sway

        # Spine counter-rotate
        f[I_SPINE + 0] = -2.0 * math.sin(t)
        f[I_SPINE + 1] = 4.0

        # Legs (Xrot = pitch). Left forward when sin>0
        leg = 28.0
        f[I_LUP + 1] = leg * math.sin(t)
        f[I_RUP + 1] = -leg * math.sin(t)
        # Knees bend on swing
        f[I_LLEG + 1] = max(0.0, -math.cos(t)) * 45.0
        f[I_RLEG + 1] = max(0.0, math.cos(t)) * 45.0
        f[I_LFOOT + 1] = -f[I_LLEG + 1] * 0.35
        f[I_RFOOT + 1] = -f[I_RLEG + 1] * 0.35

        # Arms opposite to legs
        arm = 22.0
        f[I_LARM + 1] = -arm * math.sin(t) - 10.0
        f[I_RARM + 1] = arm * math.sin(t) - 10.0
        f[I_LFORE + 1] = max(5.0, -f[I_LARM + 1]) * 0.4
        f[I_RFORE + 1] = max(5.0, -f[I_RARM + 1]) * 0.4

        lines.append(" ".join(f"{v:.4f}" for v in f))

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()

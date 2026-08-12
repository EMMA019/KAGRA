"""Generate tests/fixtures/synthetic_dance.bvh (Y-up Mixamo-like arm dance)."""
from __future__ import annotations

import math
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / "tests" / "fixtures" / "synthetic_dance.bvh"

HIER = """HIERARCHY
ROOT Hips
{
  OFFSET 0.00 100.00 0.00
  CHANNELS 6 Xposition Yposition Zposition Zrotation Xrotation Yrotation
  JOINT Spine
  {
    OFFSET 0.00 10.00 0.00
    CHANNELS 3 Zrotation Xrotation Yrotation
    JOINT Spine1
    {
      OFFSET 0.00 12.00 0.00
      CHANNELS 3 Zrotation Xrotation Yrotation
      JOINT Neck
      {
        OFFSET 0.00 14.00 0.00
        CHANNELS 3 Zrotation Xrotation Yrotation
        JOINT Head
        {
          OFFSET 0.00 8.00 0.00
          CHANNELS 3 Zrotation Xrotation Yrotation
          End Site
          {
            OFFSET 0.00 8.00 0.00
          }
        }
      }
      JOINT LeftShoulder
      {
        OFFSET 4.00 10.00 0.00
        CHANNELS 3 Zrotation Xrotation Yrotation
        JOINT LeftArm
        {
          OFFSET 12.00 0.00 0.00
          CHANNELS 3 Zrotation Xrotation Yrotation
          JOINT LeftForeArm
          {
            OFFSET 25.00 0.00 0.00
            CHANNELS 3 Zrotation Xrotation Yrotation
            JOINT LeftHand
            {
              OFFSET 22.00 0.00 0.00
              CHANNELS 3 Zrotation Xrotation Yrotation
              End Site
              {
                OFFSET 8.00 0.00 0.00
              }
            }
          }
        }
      }
      JOINT RightShoulder
      {
        OFFSET -4.00 10.00 0.00
        CHANNELS 3 Zrotation Xrotation Yrotation
        JOINT RightArm
        {
          OFFSET -12.00 0.00 0.00
          CHANNELS 3 Zrotation Xrotation Yrotation
          JOINT RightForeArm
          {
            OFFSET -25.00 0.00 0.00
            CHANNELS 3 Zrotation Xrotation Yrotation
            JOINT RightHand
            {
              OFFSET -22.00 0.00 0.00
              CHANNELS 3 Zrotation Xrotation Yrotation
              End Site
              {
                OFFSET -8.00 0.00 0.00
              }
            }
          }
        }
      }
    }
  }
  JOINT LeftUpLeg
  {
    OFFSET 8.00 -5.00 0.00
    CHANNELS 3 Zrotation Xrotation Yrotation
    JOINT LeftLeg
    {
      OFFSET 0.00 -40.00 0.00
      CHANNELS 3 Zrotation Xrotation Yrotation
      JOINT LeftFoot
      {
        OFFSET 0.00 -40.00 0.00
        CHANNELS 3 Zrotation Xrotation Yrotation
        End Site
        {
          OFFSET 0.00 -5.00 10.00
        }
      }
    }
  }
  JOINT RightUpLeg
  {
    OFFSET -8.00 -5.00 0.00
    CHANNELS 3 Zrotation Xrotation Yrotation
    JOINT RightLeg
    {
      OFFSET 0.00 -40.00 0.00
      CHANNELS 3 Zrotation Xrotation Yrotation
      JOINT RightFoot
      {
        OFFSET 0.00 -40.00 0.00
        CHANNELS 3 Zrotation Xrotation Yrotation
        End Site
        {
          OFFSET 0.00 -5.00 10.00
        }
      }
    }
  }
}
"""

# Channel layout (depth-first):
# Hips(6), Spine, Spine1, Neck, Head, LShoulder, LArm, LForeArm, LHand,
# RShoulder, RArm, RForeArm, RHand, LUpLeg, LLeg, LFoot, RUpLeg, RLeg, RFoot
N = 6 + 18 * 3
FRAMES = 48
FRAME_TIME = 1.0 / 30.0

# indices into flat channel vector
I_SPINE = 6
I_LARM = 21
I_LFORE = 24
I_RARM = 33
I_RFORE = 36


def main() -> None:
    lines = [HIER.rstrip(), "MOTION", f"Frames: {FRAMES}", f"Frame Time: {FRAME_TIME:.6f}"]
    for i in range(FRAMES):
        f = [0.0] * N
        f[0], f[1], f[2] = 0.0, 100.0, 0.0
        t = i / FRAMES * 2 * math.pi
        f[I_SPINE + 0] = 15.0 * math.sin(t)
        arm = 80.0 * (0.5 + 0.5 * math.sin(t))
        f[I_LARM + 1] = -arm
        f[I_LFORE + 1] = -30.0 * math.sin(t)
        f[I_RARM + 1] = -arm
        f[I_RFORE + 1] = -30.0 * math.sin(t + math.pi)
        f[3] = 10.0 * math.sin(t * 0.5)
        lines.append(" ".join(f"{v:.4f}" for v in f))

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()

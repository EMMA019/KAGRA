"""Generate synthetic_dance.bvh (Y-up Mixamo-like full-body dance).

Writes both the test fixture and the packaged copy used by
``python -m kagra`` (kagra/data/synthetic_dance.bvh).
"""
from __future__ import annotations

import math
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[1]
OUTS = (
    _ROOT / "tests" / "fixtures" / "synthetic_dance.bvh",
    _ROOT / "kagra" / "data" / "synthetic_dance.bvh",
)

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

# rotation channel offsets into the flat vector (each joint is rz, rx, ry)
I_HIPS = 3  # rz rx ry of the 6-channel root
I_SPINE = 6
I_SPINE1 = 9
I_NECK = 12
I_HEAD = 15
I_LSHOULDER = 18
I_LARM = 21
I_LFORE = 24
I_LHAND = 27
I_RSHOULDER = 30
I_RARM = 33
I_RFORE = 36
I_RHAND = 39
I_LUPLEG = 42
I_LLEG = 45
I_LFOOT = 48
I_RUPLEG = 51
I_RLEG = 54
I_RFOOT = 57

RZ, RX, RY = 0, 1, 2


def main() -> None:
    lines = [HIER.rstrip(), "MOTION", f"Frames: {FRAMES}", f"Frame Time: {FRAME_TIME:.6f}"]
    for i in range(FRAMES):
        f = [0.0] * N
        f[0], f[1], f[2] = 0.0, 100.0, 0.0
        t = i / FRAMES * 2 * math.pi
        # フレーム0 (t=0) は sin 項が全て 0 になり、to_clip のデルタ基準になる。

        # 体幹: 横スウェイ + ひねり
        f[I_HIPS + RZ] = 6.0 * math.sin(t)
        f[I_HIPS + RY] = 8.0 * math.sin(t)
        f[I_SPINE + RZ] = 10.0 * math.sin(t)
        f[I_SPINE + RY] = 6.0 * math.sin(t)
        f[I_SPINE1 + RZ] = 5.0 * math.sin(t)

        # 首・頭: スウェイに逆らって傾げ、ビートで頷く
        f[I_NECK + RZ] = -5.0 * math.sin(t)
        f[I_NECK + RX] = 3.0 * math.sin(2 * t)
        f[I_HEAD + RZ] = -4.0 * math.sin(t)
        f[I_HEAD + RX] = 5.0 * math.sin(2 * t)
        f[I_HEAD + RY] = 6.0 * math.sin(t)

        # 肩: ビートで小さくシュラッグ
        f[I_LSHOULDER + RZ] = 4.0 * math.sin(2 * t)
        f[I_RSHOULDER + RZ] = -4.0 * math.sin(2 * t)

        # 腕: 左右交互に前へ振り、横方向にも上下させる（正面からも動きが見える）
        arm_l = 80.0 * (0.5 + 0.5 * math.sin(t))
        arm_r = 80.0 * (0.5 + 0.5 * math.sin(t + math.pi))
        f[I_LARM + RX] = -arm_l
        f[I_RARM + RX] = -arm_r
        f[I_LARM + RZ] = 15.0 * math.sin(t)
        f[I_RARM + RZ] = -15.0 * math.sin(t + math.pi)
        f[I_LFORE + RX] = -30.0 * math.sin(t)
        f[I_RFORE + RX] = -30.0 * math.sin(t + math.pi)

        # 手首: ビートでフリック
        f[I_LHAND + RZ] = 14.0 * math.sin(2 * t)
        f[I_RHAND + RZ] = -14.0 * math.sin(2 * t)

        # 脚: 骨盤の傾きを打ち消しつつ、左右交互のステップ（その場）
        f[I_LUPLEG + RZ] = -4.0 * math.sin(t)
        f[I_RUPLEG + RZ] = -4.0 * math.sin(t)
        step_l = max(0.0, math.sin(t))
        step_r = max(0.0, math.sin(t + math.pi))
        f[I_LUPLEG + RX] = -8.0 * step_l
        f[I_LLEG + RX] = 16.0 * step_l
        f[I_LFOOT + RX] = -8.0 * step_l
        f[I_RUPLEG + RX] = -8.0 * step_r
        f[I_RLEG + RX] = 16.0 * step_r
        f[I_RFOOT + RX] = -8.0 * step_r

        lines.append(" ".join(f"{v:.4f}" for v in f))

    text = "\n".join(lines) + "\n"
    for out in OUTS:
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
        print(f"wrote {out} ({out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()

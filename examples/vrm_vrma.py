"""VRM Animation (.vrma) を載せて踊る。

    python examples/vrm_vrma.py
    python examples/vrm_vrma.py path/to/your.vrma

手元に .vrma が無ければ、テスト用の合成ウェーブをその場で書いて再生する。
BOOTH / VRoid Hub の公式モーションはそのままパスを渡せば動く。
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra
from kagra.camera3d import Camera3D
from kagra.vrma_player import write_synthetic_vrma

kagra.init(title="KAGRA — VRMA", width=1280, height=720)
cam = Camera3D()
cam.use_orbit(radius=2.6, phi=0.1, target=(0, 0.9, 0))

av = kagra.avatar(str(kagra.ensure_vrm()))

src = sys.argv[1] if len(sys.argv) > 1 else None
if src is None:
    src = "scratch/synthetic_wave.vrma"
    write_synthetic_vrma(src, frames=24, duration=1.2)
    print(f"[example] no .vrma given — wrote {src}")

av.dance(src)
av.sing()

def update(dt):
    av.update(dt)
    cam.orbit_by(dt * 0.25, 0)
    cam.update(kagra.get_engine())


def draw():
    kagra.cls(16, 12, 32)
    kagra.draw_vrm(av.vrm_id)


kagra.run(update, draw)

"""VRM が数行で歌って踊る。

    python examples/vrm_sing_dance.py
    python -m kagra

VRM が無ければサンプル（Alicia Solid）を 1 回だけダウンロードする。
歌はその場で合成、ダンスは同梱 BVH。

Windows では ``avatar()`` を ``run(on_ready=...)`` の中で呼ぶ。
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra
from kagra.camera3d import Camera3D

kagra.init(title="KAGRA — VRM Live", width=1280, height=720)
cam = Camera3D(1280, 720, fov_deg=32.0)
cam.use_showcase()
av = None

def ready():
    global av
    kagra.apply_live_look()
    av = kagra.avatar(str(kagra.ensure_vrm()))
    av.dance(); av.sing()

def update(dt):
    av.update(dt)
    cam.update(kagra.get_engine(), dt)

def draw():
    kagra.cls(8, 6, 18)
    kagra.draw_vrm(av.vrm_id)
    kagra.draw_vignette()

kagra.run(update, draw, on_ready=ready)

"""VRM が数行で歌って踊る。

    python examples/vrm_sing_dance.py
    python -m kagra

VRM が無ければサンプル（Alicia Solid）を 1 回だけダウンロードする。
歌はその場で合成、ダンスは同梱 BVH。
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import kagra
from kagra.camera3d import Camera3D

kagra.init(title="KAGRA — VRM Live", width=1280, height=720)
cam = Camera3D(); cam.use_orbit(radius=2.6, phi=0.1, target=(0, 0.9, 0))

av = kagra.avatar(str(kagra.ensure_vrm()))  # assets/ または初回ダウンロード
av.dance()                  # 同梱のダンスモーションを再生
av.sing()                   # 歌をその場で合成してリップシンク

def update(dt):
    av.update(dt)
    cam.orbit_by(dt * 0.25, 0)
    cam.update(kagra.get_engine())

def draw():
    kagra.cls(16, 12, 32)
    kagra.draw_vrm(av.vrm_id)

kagra.run(update, draw)

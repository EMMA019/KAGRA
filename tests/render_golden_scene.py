"""ゴールデン画像用の単発レンダラ。pytest から子プロセスで呼ぶ。

使い方:
    python tests/render_golden_scene.py shapes2d scratch/golden_actual/shapes2d.png
    python tests/render_golden_scene.py mesh3d scratch/golden_actual/mesh3d.png
"""
from __future__ import annotations

import os
import struct
import sys
import tempfile
import zlib
from pathlib import Path

import kagra

SW, SH = 320, 180


def solid_png(r: int, g: int, b: int, a: int = 255) -> str:
    raw = b"\x00" + bytes([r, g, b, a])
    compressed = zlib.compress(raw)

    def chunk(tag: bytes, payload: bytes) -> bytes:
        crc = zlib.crc32(tag + payload) & 0xFFFFFFFF
        return struct.pack(">I", len(payload)) + tag + payload + struct.pack(">I", crc)

    path = os.path.join(tempfile.gettempdir(), f"kagra_golden_{r}_{g}_{b}_{a}.png")
    with open(path, "wb") as f:
        f.write(
            b"\x89PNG\r\n\x1a\n"
            + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", compressed)
            + chunk(b"IEND", b"")
        )
    return path


class Shapes2D(kagra.Scene):
    def update(self, dt):
        pass

    def draw(self):
        kagra.cls(40, 50, 70)
        kagra.fill(20, 20, 120, 80, (220, 60, 60))
        kagra.fill(160, 40, 100, 100, (60, 200, 90), alpha=200)
        kagra.circle_fill(240, 90, 35, (80, 140, 255))
        kagra.circle_outline(80, 130, 25, (255, 220, 80), width=3)


class Mesh3D(kagra.Scene):
    def on_enter(self):
        self.tex = kagra.load(solid_png(230, 230, 240))
        self.cam = kagra.Camera3D(SW, SH, fov_deg=40.0)
        self.cam.use_orbit(radius=3.2, theta=0.6, phi=0.35, target=(0.0, 0.0, 0.0))
        self.verts = [
            [-1.2, 0.0, -1.2, 0, 1, 0, 0, 0],
            [1.2, 0.0, -1.2, 0, 1, 0, 1, 0],
            [1.2, 0.0, 1.2, 0, 1, 0, 1, 1],
            [-1.2, 0.0, 1.2, 0, 1, 0, 0, 1],
            [-1.0, 0.0, -1.2, 0, 0, 1, 0, 0],
            [1.0, 0.0, -1.2, 0, 0, 1, 1, 0],
            [1.0, 1.5, -1.2, 0, 0, 1, 1, 1],
            [-1.0, 1.5, -1.2, 0, 0, 1, 0, 1],
        ]
        self.indices = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7]

    def update(self, dt):
        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

    def draw(self):
        kagra.cls(25, 30, 40)
        kagra.draw_mesh_3d(self.tex, self.verts, self.indices)


SCENES = {
    "shapes2d": (Shapes2D, 8),
    "mesh3d": (Mesh3D, 10),
}


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <scene> <out.png>", file=sys.stderr)
        print(f"scenes: {', '.join(SCENES)}", file=sys.stderr)
        return 2

    name, out_path = sys.argv[1], Path(sys.argv[2])
    if name not in SCENES:
        print(f"unknown scene: {name}", file=sys.stderr)
        return 2

    scene_cls, frames = SCENES[name]
    out_path.parent.mkdir(parents=True, exist_ok=True)

    class Wrap(scene_cls):
        def update(self, dt):
            super().update(dt)
            t = kagra.tick_count()
            if t == frames - 2:
                kagra.screenshot(str(out_path))
            if t >= frames - 1:
                kagra.quit()

    kagra.init(width=SW, height=SH, title="golden", fps=60, visible=False)
    kagra.run(start_scene=Wrap(), max_frames=frames, fixed_dt=1.0 / 60.0)
    if not out_path.exists():
        print(f"screenshot missing: {out_path}", file=sys.stderr)
        return 1
    print(f"WROTE {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

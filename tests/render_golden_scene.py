"""ゴールデン画像用の単発レンダラ。pytest から子プロセスで呼ぶ。

使い方:
    python tests/render_golden_scene.py shapes2d scratch/golden_actual/shapes2d.png
    python tests/render_golden_scene.py mesh3d scratch/golden_actual/mesh3d.png
    python tests/render_golden_scene.py indoor_spot scratch/golden_actual/indoor_spot.png
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


def _bind_cam(scene, radius, theta, phi, target, fov=42.0):
    scene.cam = kagra.Camera3D(SW, SH, fov_deg=fov)
    scene.cam.use_orbit(radius=radius, theta=theta, phi=phi, target=target)


class IndoorSpot(kagra.Scene):
    """天井スポット + 箱。平行光は床をほぼ照らさない（Lambert の 0.2 床だけ）。"""

    shadows = True

    def on_enter(self):
        self.tex = kagra.load(solid_png(220, 208, 188))
        _bind_cam(self, 4.4, 0.62, 0.78, (0.0, 0.12, 0.0), fov=48.0)
        self.floor_v, self.floor_i = kagra.quad_y_mesh(0.0, 0.0, 0.0, 2.4)
        self.box_v, self.box_i = kagra.box_mesh(0.0, 0.7, 0.0, 0.5, 1.4, 0.5)
        kagra.set_shadow_enabled(self.shadows)
        kagra.set_tonemap(False)
        kagra.set_bloom(enabled=False)
        kagra.set_ambient(0.04, 0.04, 0.04, 0.06)
        kagra.set_light_dir(0.1, -1.0, 0.05)
        kagra.set_hdri(None, strength=0.0)
        kagra.set_exposure(1.0)
        kagra.set_spot_light(
            0.0, 2.8, 0.0, 0.0, -1.0, 0.0,
            angle=0.9, penumbra=0.2, intensity=3.2, radius=10.0,
            r=1.0, g=0.94, b=0.82,
        )

    def update(self, dt):
        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

    def draw(self):
        kagra.cls(6, 6, 8)
        kagra.draw_mesh_3d(self.tex, self.floor_v, self.floor_i)
        kagra.draw_mesh_3d(self.tex, self.box_v, self.box_i)


class IndoorSpotOff(IndoorSpot):
    shadows = False


class TonemapScene(kagra.Scene):
    """高露出 + クロム球。ACES の有無でハイライトが変わる。"""

    tonemap = True

    def on_enter(self):
        self.tex = kagra.load(solid_png(235, 235, 240))
        _bind_cam(self, 3.4, 0.55, 0.42, (0.0, 0.45, 0.0), fov=40.0)
        fv, fi = kagra.quad_y_mesh(0.0, 0.0, 0.0, 2.0)
        sv, si = kagra.sphere_mesh(0.0, 0.55, 0.0, 0.55, segs=20)
        self.floor = kagra.upload_mesh_3d(self.tex, fv, fi)
        self.ball = kagra.upload_mesh_3d(
            self.tex, sv, si, metallic=1.0, roughness=0.12,
        )
        kagra.set_shadow_enabled(False)
        kagra.set_bloom(enabled=False)
        kagra.set_ambient(0.08, 0.08, 0.1, 0.12)
        kagra.set_light_dir(0.45, 0.9, 0.25)
        kagra.set_hdri("studio", strength=1.15)
        kagra.set_exposure(3.2)
        kagra.set_tonemap(self.tonemap)

    def update(self, dt):
        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

    def draw(self):
        kagra.cls(18, 16, 22)
        kagra.draw_mesh_id(self.floor)
        kagra.draw_mesh_id(self.ball)


class TonemapOff(TonemapScene):
    tonemap = False


class IblMetal(kagra.Scene):
    """同じ HDRI で金属 vs プラスチック。mip スペキュラが画素に出るか。"""

    metallic = 1.0
    roughness = 0.14

    def on_enter(self):
        self.tex = kagra.load(solid_png(210, 210, 216))
        _bind_cam(self, 3.2, 0.5, 0.4, (0.0, 0.5, 0.0), fov=40.0)
        sv, si = kagra.sphere_mesh(0.0, 0.55, 0.0, 0.55, segs=20)
        self.ball = kagra.upload_mesh_3d(
            self.tex, sv, si, metallic=self.metallic, roughness=self.roughness,
        )
        kagra.set_shadow_enabled(False)
        kagra.set_bloom(enabled=False)
        kagra.set_tonemap(False)
        kagra.set_ambient(0.05, 0.05, 0.06, 0.08)
        kagra.set_light_dir(0.35, 0.85, 0.4)
        kagra.set_hdri("studio", strength=1.0)
        kagra.set_exposure(1.15)
        kagra.set_point_light(1.4, 1.6, 1.1, intensity=0.35, radius=8.0)

    def update(self, dt):
        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

    def draw(self):
        kagra.cls(16, 16, 20)
        kagra.draw_mesh_id(self.ball)


class IblPlastic(IblMetal):
    metallic = 0.0
    roughness = 0.14


SCENES = {
    "shapes2d": (Shapes2D, 8),
    "mesh3d": (Mesh3D, 10),
    "indoor_spot": (IndoorSpot, 12),
    "indoor_spot_off": (IndoorSpotOff, 12),
    "tonemap_on": (TonemapScene, 12),
    "tonemap_off": (TonemapOff, 12),
    "ibl_metal": (IblMetal, 12),
    "ibl_plastic": (IblPlastic, 12),
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

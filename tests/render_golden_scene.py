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
    """横からの天井スポット + 箱。床は大きくて影を書かない（受けだけ）。"""

    shadows = True

    def on_enter(self):
        self.tex = kagra.load(solid_png(220, 208, 188))
        _bind_cam(self, 5.2, 1.05, 0.52, (1.4, 0.08, 0.0), fov=42.0)
        # 半辺 13 → 辺 26 > SHADOW_SKIP_EXTENT 24。床は受けだけでキャストしない。
        self.floor_v, self.floor_i = kagra.quad_y_mesh(0.0, 0.0, 0.0, 13.0)
        self.box_v, self.box_i = kagra.box_mesh(0.0, 0.95, 0.0, 0.55, 1.9, 0.55)
        kagra.set_shadow_enabled(self.shadows)
        kagra.set_tonemap(False)
        kagra.set_bloom(enabled=False)
        kagra.set_ambient(0.03, 0.03, 0.03, 0.05)
        kagra.set_light_dir(0.1, -1.0, 0.05)
        kagra.set_hdri(None, strength=0.0)
        kagra.set_exposure(1.0)
        kagra.set_spot_light(
            -2.4, 2.7, 0.15, 0.82, -0.55, 0.0,
            angle=0.72, penumbra=0.12, intensity=4.2, radius=12.0,
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


class NormalBump(kagra.Scene):
    """接空間法線の有無。サイドライトでバンプが見える。"""

    use_normal = True

    def on_enter(self):
        albedo = kagra.load(solid_png(190, 150, 120))

        def nrm(x, y):
            tile = ((x // 8) + (y // 8)) % 2
            if tile:
                return (220, 90, 255, 255)
            return (40, 180, 255, 255)

        ntex = kagra.texture_from_fn(64, 64, nrm, srgb=False, name="golden_bump")
        _bind_cam(self, 3.0, 0.7, 0.35, (0.0, 0.55, 0.0), fov=40.0)
        bv, bi = kagra.box_mesh(0.0, 0.55, 0.0, 1.1, 1.1, 1.1)
        nid = ntex if self.use_normal else 0
        self.box = kagra.upload_mesh_3d(
            albedo, bv, bi, metallic=0.0, roughness=0.55, normal_texture_id=nid,
        )
        kagra.set_shadow_enabled(False)
        kagra.set_bloom(enabled=False)
        kagra.set_tonemap(False)
        kagra.set_hdri(None, strength=0.0)
        kagra.set_ambient(0.12, 0.12, 0.14, 0.18)
        kagra.set_light_dir(0.65, 0.4, 0.35)
        kagra.set_exposure(1.0)

    def update(self, dt):
        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

    def draw(self):
        kagra.cls(20, 18, 22)
        kagra.draw_mesh_id(self.box)


class NormalFlat(NormalBump):
    use_normal = False


class LocalFour(kagra.Scene):
    """キー 1 + 色付き埋め 3。スロット 0 だけが影を持てる（このシーンは影オフ）。"""

    extras = True

    def on_enter(self):
        albedo = kagra.load(solid_png(210, 200, 188))
        _bind_cam(self, 3.4, 0.85, 0.42, (0.0, 0.5, 0.0), fov=40.0)
        bv, bi = kagra.box_mesh(0.0, 0.5, 0.0, 1.0, 1.0, 1.0)
        self.box = kagra.upload_mesh_3d(albedo, bv, bi, metallic=0.0, roughness=0.6)
        kagra.set_shadow_enabled(False)
        kagra.set_bloom(enabled=False)
        kagra.set_tonemap(False)
        kagra.set_hdri(None, strength=0.0)
        kagra.set_ambient(0.04, 0.04, 0.05, 0.06)
        kagra.set_light_dir(0.1, 1.0, 0.05)
        kagra.set_exposure(1.0)
        kagra.set_point_light(0.0, 2.2, 0.0, intensity=1.6, radius=8.0, slot=0)
        if self.extras:
            kagra.set_point_light(-1.6, 1.1, 0.4, r=1.0, g=0.15, b=0.1, intensity=2.2, radius=5.0, slot=1)
            kagra.set_point_light(1.6, 1.1, 0.4, r=0.1, g=1.0, b=0.2, intensity=2.2, radius=5.0, slot=2)
            kagra.set_point_light(0.0, 0.8, -1.6, r=0.15, g=0.35, b=1.0, intensity=2.2, radius=5.0, slot=3)

    def update(self, dt):
        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

    def draw(self):
        kagra.cls(8, 8, 10)
        kagra.draw_mesh_id(self.box)


class LocalOne(LocalFour):
    extras = False


# 近段 half=12・マップ 2048 のテクセル。render と shadow_fit のテストで同じ値。
# CI: 寄りの look() はウンブラの外で mean_abs=0。室内と同じオービット + 横日。
_OUTDOOR_CRAWL_TEXEL = 24.0 / 2048.0
_OUTDOOR_CRAWL_ORBIT = (3.8, 1.15, 0.72)  # radius, theta, phi
_OUTDOOR_CRAWL_TARGET = (1.6, 0.05, 0.0)


class OutdoorCrawl(kagra.Scene):
    """屋外 2 段。床は受けだけ。視点を 0.2 texel ずらしてもスナップは同じ。"""

    shadows = True
    nudge = 0.0

    def on_enter(self):
        self.tex = kagra.load(solid_png(228, 222, 210))
        dx = _OUTDOOR_CRAWL_TEXEL * 0.2 * self.nudge
        tx, ty, tz = _OUTDOOR_CRAWL_TARGET
        r, th, ph = _OUTDOOR_CRAWL_ORBIT
        _bind_cam(self, r, th, ph, (tx + dx, ty, tz + dx), fov=42.0)
        # 半辺 13 → 辺 26 > SHADOW_SKIP_EXTENT 24。床は受けだけ。
        self.floor_v, self.floor_i = kagra.quad_y_mesh(0.0, 0.0, 0.0, 13.0)
        self.box_v, self.box_i = kagra.box_mesh(0.0, 1.3, 0.0, 0.7, 2.6, 0.7)
        # 和 AABB を広げて近段 half=12（テクセル 24/2048）。画角の外。
        self.post_a, self.post_ai = kagra.box_mesh(9.2, 0.5, 9.2, 0.8, 1.0, 0.8)
        self.post_b, self.post_bi = kagra.box_mesh(-9.2, 0.5, -9.2, 0.8, 1.0, 0.8)
        kagra.set_shadow_enabled(self.shadows)
        kagra.set_shadow_cascades(2)
        kagra.set_tonemap(False)
        kagra.set_bloom(enabled=False)
        kagra.set_ambient(0.02, 0.02, 0.03, 0.04)
        # 光源は -X。ウンブラは箱の +X（オービットの注視点側）。
        kagra.set_light_dir(-0.88, 0.32, 0.08)
        kagra.set_hdri(None, strength=0.0)
        kagra.set_exposure(1.15)

    def update(self, dt):
        engine = kagra.get_engine()
        if engine:
            self.cam.update(engine)

    def draw(self):
        kagra.cls(28, 42, 58)
        kagra.draw_mesh_3d(self.tex, self.floor_v, self.floor_i)
        kagra.draw_mesh_3d(self.tex, self.box_v, self.box_i)
        kagra.draw_mesh_3d(self.tex, self.post_a, self.post_ai)
        kagra.draw_mesh_3d(self.tex, self.post_b, self.post_bi)


class OutdoorCrawlNudge(OutdoorCrawl):
    nudge = 1.0


class OutdoorCrawlOff(OutdoorCrawl):
    shadows = False


SCENES = {
    "shapes2d": (Shapes2D, 8),
    "mesh3d": (Mesh3D, 10),
    "indoor_spot": (IndoorSpot, 14),
    "indoor_spot_off": (IndoorSpotOff, 14),
    "tonemap_on": (TonemapScene, 12),
    "tonemap_off": (TonemapOff, 12),
    "ibl_metal": (IblMetal, 12),
    "ibl_plastic": (IblPlastic, 12),
    "normal_bump": (NormalBump, 12),
    "normal_flat": (NormalFlat, 12),
    "local_four": (LocalFour, 12),
    "local_one": (LocalOne, 12),
    "outdoor_crawl": (OutdoorCrawl, 14),
    "outdoor_crawl_nudge": (OutdoorCrawlNudge, 14),
    "outdoor_crawl_off": (OutdoorCrawlOff, 14),
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

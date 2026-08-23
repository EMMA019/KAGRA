"""Default live look — procedural sky, vignette, showcase cuts, foot lift.

GPU は ``apply_live_look`` / ``draw_vignette`` / ``load_default_sky`` だけ。
色・カット・接地の計算は純 Python（テストは拡張なしで回る）。
テクスチャは同梱せず、初回に小さな PNG を tempfile へ書く（wheel ~5MB 死守）。
"""
from __future__ import annotations

import math
import struct
import tempfile
import zlib
from pathlib import Path
from typing import Iterable


# 3/4 キー（右前上）。顔が沈まない位置。リムはシェーダ側の fresnel + 逆光。
LIVE_LIGHT_DIR = (0.55, 0.95, 0.32)
LIVE_TOON = (0.48, 0.18, 0.60, 1.10)
LIVE_BLOOM = (0.78, 0.38)
LIVE_RIM = 0.55
LIVE_FOG = (8.0, 18.0, (14, 10, 28))


def _clamp(x: float, lo: float = 0.0, hi: float = 1.0) -> float:
    return lo if x < lo else hi if x > hi else x


def _smoothstep(t: float) -> float:
    t = _clamp(t)
    return t * t * (3.0 - 2.0 * t)


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def _mix_rgb(a: tuple[int, int, int], b: tuple[int, int, int], t: float) -> tuple[int, int, int]:
    t = _clamp(t)
    return (
        int(_lerp(a[0], b[0], t)),
        int(_lerp(a[1], b[1], t)),
        int(_lerp(a[2], b[2], t)),
    )


def encode_png_rgba(width: int, height: int, pixels: bytes) -> bytes:
    """未圧縮 RGBA 列（上から、1 画素 4 バイト）を PNG にする。"""
    if width < 1 or height < 1:
        raise ValueError("png size must be >= 1")
    expect = width * height * 4
    if len(pixels) != expect:
        raise ValueError(f"expected {expect} bytes, got {len(pixels)}")
    rows = b""
    stride = width * 4
    for y in range(height):
        rows += b"\x00" + pixels[y * stride : (y + 1) * stride]
    raw = zlib.compress(rows, 9)

    def chunk(tag: bytes, data: bytes) -> bytes:
        crc = zlib.crc32(tag + data) & 0xFFFFFFFF
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", raw)
        + chunk(b"IEND", b"")
    )


def sky_rgba(u: float, v: float) -> tuple[int, int, int, int]:
    """グラデーション空。v=0 が下（足元）、v=1 が天頂。"""
    zenith = (8, 6, 22)
    horizon = (92, 48, 78)
    nadir = (10, 8, 16)
    if v >= 0.48:
        t = _smoothstep((v - 0.48) / 0.52)
        r, g, b = _mix_rgb(horizon, zenith, t)
    else:
        t = _smoothstep(v / 0.48)
        r, g, b = _mix_rgb(nadir, horizon, t)
    # ごく弱い横方向の明るさ（単色に見えないように）
    glow = 1.0 + 0.06 * math.sin(u * math.pi * 2.0)
    r = int(_clamp(r * glow, 0, 255))
    g = int(_clamp(g * glow, 0, 255))
    b = int(_clamp(b * glow, 0, 255))
    return r, g, b, 255


def vignette_alpha(u: float, v: float, strength: float = 1.0) -> int:
    """中心 0、四隅に近いほど不透明な黒。"""
    dx = u - 0.5
    dy = v - 0.5
    r = math.sqrt(dx * dx + dy * dy) / 0.72
    a = _smoothstep(_clamp((r - 0.28) / 0.72)) * _clamp(strength)
    return int(a * 255)


def gradient_sky_png(width: int = 64, height: int = 32) -> bytes:
    pix = bytearray()
    for y in range(height):
        v = 1.0 - y / max(1, height - 1)
        for x in range(width):
            u = x / max(1, width - 1)
            pix.extend(sky_rgba(u, v))
    return encode_png_rgba(width, height, bytes(pix))


def vignette_png(width: int = 64, height: int = 64, strength: float = 1.0) -> bytes:
    pix = bytearray()
    for y in range(height):
        v = y / max(1, height - 1)
        for x in range(width):
            u = x / max(1, width - 1)
            pix.extend((0, 0, 0, vignette_alpha(u, v, strength)))
    return encode_png_rgba(width, height, bytes(pix))


def _write_png(name: str, data: bytes) -> Path:
    path = Path(tempfile.gettempdir()) / name
    path.write_bytes(data)
    return path


def write_gradient_sky(name: str = "kagra_live_sky.png") -> Path:
    return _write_png(name, gradient_sky_png())


def write_vignette(name: str = "kagra_live_vignette.png") -> Path:
    return _write_png(name, vignette_png())


def showcase_blend(
    t: float,
    *,
    period: float = 6.5,
    blend: float = 1.35,
) -> float:
    """0=全身、1=顔寄り。``period`` 秒ごとにカットし、``blend`` 秒で交差する。"""
    period = max(0.5, float(period))
    blend = min(max(0.05, float(blend)), period - 0.05)
    cycle = period * 2.0
    u = float(t) % cycle
    if u < 0:
        u += cycle
    hold = period - blend
    if u < hold:
        return 0.0
    if u < period:
        return _smoothstep((u - hold) / blend)
    if u < period + hold:
        return 1.0
    return 1.0 - _smoothstep((u - period - hold) / blend)


def showcase_params(
    blend: float,
    *,
    body_radius: float = 3.35,
    face_radius: float = 1.62,
    body_target_y: float = 0.84,
    face_target_y: float = 1.30,
    body_fov: float = 32.0,
    face_fov: float = 26.0,
) -> dict[str, float]:
    """``showcase_blend`` の 0〜1 をカメラパラメータへ。"""
    u = _clamp(float(blend))
    return {
        "radius": _lerp(body_radius, face_radius, u),
        "target_y": _lerp(body_target_y, face_target_y, u),
        "fov": _lerp(body_fov, face_fov, u),
        "blend": u,
    }


def grounding_lift(
    foot_ys: Iterable[float],
    *,
    floor_y: float = 0.0,
    sole: float = 0.03,
    current: float = 0.0,
    follow: float = 0.4,
) -> float:
    """一番低い足が床を割っていたら持ち上げる量（ワールド Y）。

    押し下げはしない（浮き足のダンスを潰さない）。``follow`` でスムーズ。
    """
    ys = [float(y) for y in foot_ys]
    follow = _clamp(float(follow))
    if not ys:
        return current * (1.0 - follow)
    need = (float(floor_y) + float(sole)) - min(ys)
    target = max(0.0, need)
    return current + (target - current) * follow


def apply_live_look(*, mascot: bool = False) -> None:
    """デモ既定の光・トゥーン・ブルーム・リム・フォグ。ゴールデンは触らない。"""
    import kagra

    if mascot:
        kagra.set_light_dir(*LIVE_LIGHT_DIR)
        kagra.set_rim(0.35)
        kagra.set_shadow_enabled(False)
        kagra.set_bloom(threshold=LIVE_BLOOM[0], intensity=0.22)
        return
    kagra.set_light_dir(*LIVE_LIGHT_DIR)
    kagra.set_toon_params(*LIVE_TOON)
    kagra.set_bloom(threshold=LIVE_BLOOM[0], intensity=LIVE_BLOOM[1])
    kagra.set_rim(LIVE_RIM)
    kagra.set_fog(start=LIVE_FOG[0], end=LIVE_FOG[1], color=LIVE_FOG[2], enabled=True)
    kagra.set_shadow_enabled(True)


def load_default_sky(*, radius: float = 18.0):
    """プロシージャル空を読み、``(tex_id, verts, indices)`` を返す。"""
    import kagra
    from kagra.stage import backdrop_sphere

    path = write_gradient_sky()
    tex = kagra.load(str(path))
    verts, indices = backdrop_sphere(radius)
    return tex, verts, indices


_vignette_tex: int | None = None


def draw_vignette(sw: int, sh: int, strength: float = 0.42) -> None:
    """画面端を落とす。3D のあと、HUD の前。"""
    global _vignette_tex
    import kagra

    if strength <= 0.0:
        return
    if _vignette_tex is None:
        _vignette_tex = kagra.load(str(write_vignette()))
    kagra.image(_vignette_tex, 0, 0, sw, sh, alpha=float(strength))


def make_live_floor():
    """チェッカーをやめた暗い円盤 + 暖色スポット。``demo`` が描く。"""
    from kagra.vrm_stage import make_png

    def floor_px(x, y):
        dx = x / 128.0 - 0.5
        dy = y / 128.0 - 0.5
        r = math.sqrt(dx * dx + dy * dy) * 2.0
        edge = _clamp((r - 0.82) / 0.18)
        base = _mix_rgb((28, 20, 36), (12, 8, 18), _clamp(r))
        ring = _mix_rgb(base, (62, 42, 28), _smoothstep(1.0 - abs(r - 0.92) / 0.08) * (1.0 - edge))
        a = 255 if r < 0.98 else int((1.0 - edge) * 255)
        return (ring[0], ring[1], ring[2], a)

    def spot_px(x, y):
        d = math.sqrt((x - 32) ** 2 + (y - 32) ** 2) / 32.0
        a = max(0, int((1.0 - _clamp(d)) ** 1.35 * 200))
        return (255, 214, 150, a)

    floor_tex = make_png(128, 128, floor_px)
    spot_tex = make_png(64, 64, spot_px)
    meshes = []
    for tex, radius, y in ((floor_tex, 2.6, 0.0), (spot_tex, 0.95, 0.012)):
        segs = 40
        verts, idx = [], []
        for i in range(segs):
            a0 = math.radians(i * 360 / segs)
            a1 = math.radians((i + 1) * 360 / segs)
            base = len(verts)
            verts += [
                [0.0, y, 0.0, 0.0, 1.0, 0.0, 0.5, 0.5],
                [
                    math.cos(a0) * radius,
                    y,
                    math.sin(a0) * radius,
                    0.0,
                    1.0,
                    0.0,
                    0.5 + math.cos(a0) * 0.5,
                    0.5 + math.sin(a0) * 0.5,
                ],
                [
                    math.cos(a1) * radius,
                    y,
                    math.sin(a1) * radius,
                    0.0,
                    1.0,
                    0.0,
                    0.5 + math.cos(a1) * 0.5,
                    0.5 + math.sin(a1) * 0.5,
                ],
            ]
            idx += [base, base + 1, base + 2]
        meshes.append((tex, verts, idx))
    return meshes

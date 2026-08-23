"""Ursina 級の短さで 3D を置く。2D ECS の ``Entity`` とは別物。

GPU 不要な計算（色・歩行ベクトル・当たり）は純関数。描画は ``bake`` 後。
"""
from __future__ import annotations

import math
from typing import Optional

from kagra.color_utils import clamp_u8
from kagra.gamekit import box_mesh, cylinder_mesh, quad_y_mesh, sphere_mesh
from kagra.world3d import World3D

COLORS = {
    "white": (230, 230, 235),
    "gray": (120, 120, 128),
    "red": (220, 70, 60),
    "orange": (240, 140, 50),
    "gold": (240, 200, 70),
    "green": (70, 180, 90),
    "teal": (50, 170, 170),
    "blue": (70, 130, 220),
    "purple": (150, 90, 200),
    "pink": (230, 120, 170),
}


def resolve_color(color) -> tuple[int, int, int]:
    """名前（``orange``）または ``(r,g,b)``。"""
    if isinstance(color, str):
        key = color.strip().lower()
        if key not in COLORS:
            raise ValueError(f"unknown color {color!r}")
        return COLORS[key]
    r, g, b = color[0], color[1], color[2]
    return clamp_u8(r), clamp_u8(g), clamp_u8(b)


def walk_wish(forward: float, right: float, yaw: float, speed: float = 3.2) -> tuple[float, float]:
    """カメラ ``yaw`` 基準の歩行速度。forward=+1 は視線方向。"""
    mag = math.hypot(forward, right)
    if mag < 1e-6:
        return 0.0, 0.0
    forward /= mag
    right /= mag
    fx = math.sin(yaw) * forward + math.cos(yaw) * right
    fz = math.cos(yaw) * forward - math.sin(yaw) * right
    return fx * speed, fz * speed


def look_yaw(yaw: float, dx: float, *, sens: float = 0.004) -> float:
    """マウス X 増分から yaw を更新する。"""
    return float(yaw) - float(dx) * float(sens)


def _unit_mesh(model: str):
    m = str(model).lower()
    if m == "box":
        return box_mesh(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)
    if m == "sphere":
        return sphere_mesh(0.0, 0.0, 0.0, 0.5, 14)
    if m == "cylinder":
        return cylinder_mesh(0.0, 0.0, 0.0, 0.5, 1.0, 14)
    if m in ("plane", "floor"):
        return quad_y_mesh(0.0, 0.0, 0.0, 0.5)
    raise ValueError(f"unknown model {model!r} (box/sphere/cylinder/plane)")


_solid_cache: dict[tuple[int, int, int], int] = {}
_unit_cache: dict[tuple[str, int], int] = {}
_sky_cache = None


def solid_tex(color) -> int:
    """1 色テクスチャ。同じ色は使い回す。エンジンが要る。"""
    import kagra

    rgb = resolve_color(color)
    hit = _solid_cache.get(rgb)
    if hit:
        return hit
    r, g, b = rgb

    def px(_x, _y):
        return (r, g, b, 255)

    tid = kagra.texture_from_fn(4, 4, px, name=f"solid_{r}_{g}_{b}")
    _solid_cache[rgb] = int(tid)
    return int(tid)


def sky(*, radius: float = 18.0, look: bool = True):
    """プロシージャル空を描く。初回だけ ``apply_live_look``。"""
    global _sky_cache
    import kagra
    from kagra.look import apply_live_look, load_default_sky

    if look and _sky_cache is None:
        apply_live_look()
    if _sky_cache is None:
        _sky_cache = load_default_sky(radius=radius)
    tex, verts, idx = _sky_cache
    kagra.draw_mesh_3d(tex, verts, idx)


class Prop:
    """色付きプリミティブ。位置は中心。``World3D`` があれば AABB 衝突。

    2D の ``kagra.Entity`` とは別。エージェントはこっちを使う。
    """

    _all: list["Prop"] = []

    def __init__(
        self,
        model: str = "box",
        *,
        x: float = 0.0,
        y: float = 0.5,
        z: float = 0.0,
        scale: float | tuple = 1.0,
        color="white",
        collision: bool = True,
        world: Optional[World3D] = None,
        yaw: float = 0.0,
    ):
        self.model = str(model).lower()
        if self.model not in ("box", "sphere", "cylinder", "plane"):
            raise ValueError(f"unknown model {model!r}")
        self.x = float(x)
        self.y = float(y)
        self.z = float(z)
        self.yaw = float(yaw)
        if isinstance(scale, (int, float)):
            self.sx = self.sy = self.sz = float(scale)
        else:
            self.sx, self.sy, self.sz = (float(scale[0]), float(scale[1]), float(scale[2]))
        self.color = resolve_color(color)
        self.collision = bool(collision)
        self.world = world
        self.tex_id = 0
        self.mesh_id = 0
        self.body = None
        if world is not None and self.collision and self.model != "plane":
            bottom = self.y - self.sy * 0.5
            self.body = world.add_box(
                self.x, bottom, self.z, self.sx, self.sy, self.sz,
                draw=False,
            )
        Prop._all.append(self)

    def instance(self) -> list[float]:
        return [self.x, self.y, self.z, self.sx, self.sy, self.sz, self.yaw]

    def bake(self) -> int:
        """色テクスチャと単位メッシュを載せる。エンジン未初期化なら 0。"""
        try:
            import kagra
            self.tex_id = solid_tex(self.color)
            key = (self.model, self.tex_id)
            mid = _unit_cache.get(key)
            if not mid:
                verts, idx = _unit_mesh(self.model)
                mid = int(kagra.upload_mesh_3d(self.tex_id, verts, idx))
                if mid:
                    _unit_cache[key] = mid
            self.mesh_id = mid or 0
            return self.mesh_id
        except Exception:
            self.mesh_id = 0
            return 0

    @classmethod
    def bake_all(cls) -> list[int]:
        return [p.bake() for p in cls._all]

    @classmethod
    def draw_all(cls) -> None:
        try:
            import kagra
        except Exception:
            return
        batches: dict[int, list[list[float]]] = {}
        for p in cls._all:
            if not p.mesh_id:
                p.bake()
            if not p.mesh_id:
                continue
            batches.setdefault(p.mesh_id, []).append(p.instance())
        for mid, inst in batches.items():
            kagra.draw_mesh_instances(mid, inst)

    @classmethod
    def clear(cls) -> None:
        cls._all.clear()


class Walk:
    """WASD + マウス左右で視点。``World3D`` のカプセルと ``Camera3D.follow``。"""

    def __init__(
        self,
        world: World3D,
        cam,
        *,
        speed: float = 3.2,
        mouse_sens: float = 0.004,
        distance: float = 4.6,
        height: float = 2.2,
        yaw: float = 0.0,
    ):
        self.world = world
        self.cam = cam
        self.speed = float(speed)
        self.mouse_sens = float(mouse_sens)
        self.distance = float(distance)
        self.height = float(height)
        self.yaw = float(yaw)
        self._last_mouse: Optional[tuple[float, float]] = None

    def step(self, dt: float) -> None:
        """``update`` の別名。"""
        self.update(dt)

    def update(self, dt: float) -> None:
        import kagra

        mx, my = kagra.mouse_pos()
        if self._last_mouse is not None:
            self.yaw = look_yaw(self.yaw, mx - self._last_mouse[0], sens=self.mouse_sens)
        self._last_mouse = (float(mx), float(my))

        fwd = (1.0 if kagra.key("W") or kagra.key("UP") else 0.0) - (
            1.0 if kagra.key("S") or kagra.key("DOWN") else 0.0
        )
        right = (1.0 if kagra.key("D") or kagra.key("RIGHT") else 0.0) - (
            1.0 if kagra.key("A") or kagra.key("LEFT") else 0.0
        )
        vx, vz = walk_wish(fwd, right, self.yaw, self.speed)
        self.world.move_player(vx, vz)
        self.world.update(dt)
        p = self.world.player
        if p is None:
            return
        self.cam.follow(
            p.x, p.y, p.z,
            yaw=self.yaw,
            distance=self.distance,
            height=self.height,
            lerp=0.22,
        )
        eng = kagra.get_engine()
        if eng:
            self.cam.update(eng)

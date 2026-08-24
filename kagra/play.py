"""Ursina 級の短さで 3D を置く。2D ECS の ``Entity`` とは別物。

GPU 不要な計算（色・歩行ベクトル・当たり）は純関数。描画は ``bake`` 後。
"""
from __future__ import annotations

import math
from typing import Optional

from kagra.color_utils import clamp_u8
from kagra.gamekit import box_mesh, cylinder_mesh, quad_y_mesh, sphere_mesh
from kagra.gltf_mesh import FlatMesh, flatten_gltf, is_gltf_name, resolve_gltf_path
from kagra.pad import axis as pad_axis, poll_pad, stick_move
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


def color_name(rgb) -> str | None:
    """``resolve_color`` の逆。名前が無ければ None。"""
    want = resolve_color(rgb)
    for name, value in COLORS.items():
        if value == want:
            return name
    return None


def jump_vy(
    on_ground: bool,
    in_water: bool,
    jump: float,
    *,
    coyote: bool = False,
) -> float | None:
    """ジャンプ／泳ぎの鉛直速度。しないなら None。``coyote`` は接地猶予。"""
    jump = float(jump)
    if jump <= 0.0:
        return None
    if in_water:
        return jump * 0.42
    if on_ground or coyote:
        return jump
    return None


def walk_wish(forward: float, right: float, yaw: float, speed: float = 3.2) -> tuple[float, float]:
    """カメラ ``yaw`` 基準の歩行速度。

    ``forward=+1`` は視線方向（``yaw=0`` で +Z）。``right=+1`` は画面右。
    ``Camera3D`` の right は ``forward × up`` なので、視線 +Z のとき画面右は −X。
    """
    mag = math.hypot(forward, right)
    if mag < 1e-6:
        return 0.0, 0.0
    forward /= mag
    right /= mag
    fx = math.sin(yaw) * forward - math.cos(yaw) * right
    fz = math.cos(yaw) * forward + math.sin(yaw) * right
    return fx * speed, fz * speed


def walk_key_axes(down) -> tuple[float, float]:
    """WASD + arrows → ``(forward, right)``. ``down(name) -> bool``（``kagra.key``）。

    両方離したら ``(0, 0)``。S と ↓ は同じ後退（片方が残っていれば歩き続ける）。
    """
    fwd = (1.0 if down("W") or down("UP") else 0.0) - (
        1.0 if down("S") or down("DOWN") else 0.0
    )
    right = (1.0 if down("D") or down("RIGHT") else 0.0) - (
        1.0 if down("A") or down("LEFT") else 0.0
    )
    return fwd, right


def walk_axes(
    pad_lx: float,
    pad_ly: float,
    key_forward: float = 0.0,
    key_right: float = 0.0,
    *,
    deadzone: float = 0.2,
) -> tuple[float, float]:
    """左スティック + キーボード → ``(forward, right)``。

    デッドゾーン内（0 軸を含む）のスティックは離した扱い。キーボードとパッドは
    足すので、残り軸がキーを無視して歩き続けない。どちらも休みなら ``(0, 0)``。
    """
    pf, pr = stick_move(pad_lx, pad_ly, deadzone=deadzone)
    fwd = float(pf) + float(key_forward)
    right = float(pr) + float(key_right)
    if abs(fwd) < 1e-9:
        fwd = 0.0
    if abs(right) < 1e-9:
        right = 0.0
    return fwd, right


def look_yaw(yaw: float, dx: float, *, sens: float = 0.004) -> float:
    """マウス X 増分から yaw を更新する。"""
    return float(yaw) - float(dx) * float(sens)


def look_pitch(pitch: float, dy: float, *, sens: float = 0.004, lo: float = -1.2, hi: float = 1.2) -> float:
    """マウス Y 増分から pitch を更新する。上向きが正。``lo`` / ``hi`` でクランプ。"""
    p = float(pitch) - float(dy) * float(sens)
    return max(float(lo), min(float(hi), p))


def facing_yaw(dx: float, dz: float, fallback: float = 0.0) -> float:
    """移動ベクトル ``(dx, dz)`` の向き。``atan2(dx, dz)``（``yaw=0`` が +Z）。

    停止中（長さがほぼ 0）は ``fallback``。三人称の VRM はカメラ ``yaw`` ではなく
    こちらを ``set_yaw`` する。後退（カメラへ歩く）は ``±π`` で振り返る
    （``atan2(0, -speed)`` は ``-π``。``π`` と同じ向き）。
    """
    if dx * dx + dz * dz < 1e-8:
        return float(fallback)
    return math.atan2(float(dx), float(dz))


def pointer_look_delta(
    engine_delta: tuple[float, float] | None,
    mouse_pos: tuple[float, float] | None,
    last_mouse: tuple[float, float] | None,
) -> tuple[float, float, tuple[float, float] | None]:
    """視点用の相対マウス。``engine_delta`` があればそれを使う。

    ``(0, 0)`` は「動いていない」であり、``mouse_pos`` 差分へのフォールバックでは
    ない。ポインタロックでカーソルが中央へ飛ぶと、絶対座標差分は急な下向き
    pitch になる。``engine_delta is None`` のときだけ ``mouse_pos`` を使う。
    戻りは ``(dx, dy, new_last_mouse)``。
    """
    if engine_delta is not None:
        return float(engine_delta[0]), float(engine_delta[1]), None
    if mouse_pos is None:
        return 0.0, 0.0, last_mouse
    mx, my = float(mouse_pos[0]), float(mouse_pos[1])
    if last_mouse is not None:
        dx = mx - last_mouse[0]
        dy = my - last_mouse[1]
    else:
        dx = dy = 0.0
    return dx, dy, (mx, my)


def first_person_eye(
    x: float,
    y: float,
    z: float,
    yaw: float,
    pitch: float = 0.0,
    *,
    eye_height: float = 1.55,
) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
    """カプセル底面 ``(x,y,z)`` から一人称の ``(position, target)``。"""
    cp = math.cos(float(pitch))
    fx = math.sin(float(yaw)) * cp
    fy = math.sin(float(pitch))
    fz = math.cos(float(yaw)) * cp
    ex, ey, ez = float(x), float(y) + float(eye_height), float(z)
    return (ex, ey, ez), (ex + fx, ey + fy, ez + fz)


def _offset_xz(px: float, pz: float, yaw: float, lx: float, lz: float) -> tuple[float, float]:
    """親の yaw でローカル XZ を回す（``world_verts`` と同じ）。"""
    c, s = math.cos(yaw), math.sin(yaw)
    return px + c * lx + s * lz, pz - s * lx + c * lz


def prop_hit_extents(prop: "Prop") -> tuple[float, float, float]:
    """ホバー / 衝突の幅・高さ・奥行き。glTF はメッシュ AABB × スケール。"""
    mx = float(getattr(prop, "_mesh_sx", 1.0) or 1.0)
    my = float(getattr(prop, "_mesh_sy", 1.0) or 1.0)
    mz = float(getattr(prop, "_mesh_sz", 1.0) or 1.0)
    return abs(float(prop.sx)) * mx, abs(float(prop.sy)) * my, abs(float(prop.sz)) * mz


def prop_aabb(prop: "Prop") -> tuple[float, float, float, float, float, float]:
    """Prop 中心とスケールから AABB ``(min x,y,z, max x,y,z)``。世界座標。"""
    wx, wy, wz, _ = prop_world_pose(prop)
    w, h, d = prop_hit_extents(prop)
    hx, hy, hz = w * 0.5, h * 0.5, d * 0.5
    return (
        wx - hx, wy - hy, wz - hz,
        wx + hx, wy + hy, wz + hz,
    )


def prop_world_pose(prop: "Prop") -> tuple[float, float, float, float]:
    """世界の ``(x, y, z, yaw)``。親子が無ければローカルと同じ。"""
    fn = getattr(prop, "world_pose", None)
    if fn is not None:
        return fn()
    return float(prop.x), float(prop.y), float(prop.z), float(getattr(prop, "yaw", 0.0))


def ray_aabb(
    ox: float, oy: float, oz: float,
    dx: float, dy: float, dz: float,
    bounds,
    *,
    max_dist: float = 80.0,
) -> Optional[float]:
    """スラブ法。ヒット距離、外れは None。``bounds`` は min/max 6 要素。"""
    tmin = 0.0
    tmax = float(max_dist)
    xmin, ymin, zmin, xmax, ymax, zmax = bounds
    for o, d, lo, hi in (
        (ox, dx, xmin, xmax),
        (oy, dy, ymin, ymax),
        (oz, dz, zmin, zmax),
    ):
        if abs(d) < 1e-8:
            if o < lo or o > hi:
                return None
            continue
        inv = 1.0 / d
        t1 = (lo - o) * inv
        t2 = (hi - o) * inv
        if t1 > t2:
            t1, t2 = t2, t1
        tmin = max(tmin, t1)
        tmax = min(tmax, t2)
        if tmin > tmax:
            return None
    return tmin


def ray_sphere(
    ox: float, oy: float, oz: float,
    dx: float, dy: float, dz: float,
    cx: float, cy: float, cz: float, r: float,
    *,
    max_dist: float = 80.0,
) -> Optional[float]:
    """単位方向前提。外れは None。"""
    lx, ly, lz = ox - cx, oy - cy, oz - cz
    b = lx * dx + ly * dy + lz * dz
    c = lx * lx + ly * ly + lz * lz - r * r
    disc = b * b - c
    if disc < 0:
        return None
    s = math.sqrt(disc)
    for t in (-b - s, -b + s):
        if 0.0 <= t < max_dist:
            return t
    return None


def ray_disk_y(
    ox: float, oy: float, oz: float,
    dx: float, dy: float, dz: float,
    cx: float, cy: float, cz: float, r: float,
    *,
    max_dist: float = 80.0,
) -> Optional[float]:
    if abs(dy) < 1e-8:
        return None
    t = (cy - oy) / dy
    if t < 0.0 or t >= max_dist:
        return None
    px = ox + dx * t - cx
    pz = oz + dz * t - cz
    if px * px + pz * pz <= r * r:
        return t
    return None


def ray_cylinder(
    ox: float, oy: float, oz: float,
    dx: float, dy: float, dz: float,
    cx: float, cz: float, r: float, y0: float, y1: float,
    *,
    max_dist: float = 80.0,
) -> Optional[float]:
    """Y 軸円柱 + 上下の円盤。"""
    best = None
    a = dx * dx + dz * dz
    if a >= 1e-12:
        fx, fz = ox - cx, oz - cz
        b = 2.0 * (fx * dx + fz * dz)
        c = fx * fx + fz * fz - r * r
        disc = b * b - 4.0 * a * c
        if disc >= 0:
            s = math.sqrt(disc)
            for t in ((-b - s) / (2.0 * a), (-b + s) / (2.0 * a)):
                if 0.0 <= t < (best if best is not None else max_dist):
                    y = oy + dy * t
                    if y0 <= y <= y1:
                        best = t
    for y in (y0, y1):
        t = ray_disk_y(ox, oy, oz, dx, dy, dz, cx, y, cz, r, max_dist=max_dist)
        if t is not None and (best is None or t < best):
            best = t
    return best


def prop_hit_t(
    ox: float, oy: float, oz: float,
    dx: float, dy: float, dz: float,
    prop: "Prop",
    *,
    max_dist: float = 80.0,
) -> Optional[float]:
    """Prop の見た目の形に対するレイ距離。"""
    wx, wy, wz, _ = prop_world_pose(prop)
    model = getattr(prop, "model", "box")
    if model == "sphere":
        r = 0.5 * max(abs(prop.sx), abs(prop.sy), abs(prop.sz))
        return ray_sphere(ox, oy, oz, dx, dy, dz, wx, wy, wz, r, max_dist=max_dist)
    if model == "cylinder":
        r = 0.5 * max(abs(prop.sx), abs(prop.sz))
        hy = abs(prop.sy) * 0.5
        return ray_cylinder(
            ox, oy, oz, dx, dy, dz,
            wx, wz, r, wy - hy, wy + hy,
            max_dist=max_dist,
        )
    return ray_aabb(ox, oy, oz, dx, dy, dz, prop_aabb(prop), max_dist=max_dist)


def hovered_prop(
    ox: float,
    oy: float,
    oz: float,
    dx: float,
    dy: float,
    dz: float,
    props=None,
    *,
    max_dist: float = 80.0,
):
    """レイに最も近い ``Prop``。``plane`` は床扱いなので除外。"""
    length = math.sqrt(dx * dx + dy * dy + dz * dz)
    if length < 1e-8:
        return None
    dx, dy, dz = dx / length, dy / length, dz / length
    best_t = float(max_dist)
    best = None
    for p in (Prop._all if props is None else props):
        if not getattr(p, "enabled", True):
            continue
        if getattr(p, "model", "") == "plane":
            continue
        t = prop_hit_t(ox, oy, oz, dx, dy, dz, p, max_dist=best_t)
        if t is not None and 0.0 <= t < best_t:
            best_t = t
            best = p
    return best


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
_unit_cache: dict[tuple, int] = {}
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
        try:
            apply_live_look()
        except Exception:
            pass
    if _sky_cache is None:
        _sky_cache = load_default_sky(radius=radius)
    tex, verts, idx = _sky_cache
    kagra.draw_mesh_3d(tex, verts, idx)


_water_cache = None


def water(y: float = 0.0, *, half: float = 24.0, world: Optional[World3D] = None):
    """水面を描く。``world`` があれば物理の水面も同じ高さにする。"""
    global _water_cache
    import kagra
    from kagra.gamekit import quad_y_mesh

    if world is not None:
        world.set_water_y(float(y))
    key = (round(float(y), 3), round(float(half), 3))
    if _water_cache is None or _water_cache[0] != key:
        def px(_x, _y):
            return (36, 110, 140, 200)

        tex = kagra.texture_from_fn(8, 8, px, name="water_plane")
        verts, idx = quad_y_mesh(0.0, float(y) + 0.04, 0.0, float(half) + 0.4)
        _water_cache = (key, int(tex), verts, idx)
    _tex, verts, idx = _water_cache[1], _water_cache[2], _water_cache[3]
    kagra.draw_mesh_3d(_tex, verts, idx)


def room_layout(half: float = 6.0, height: float = 3.2, thick: float = 0.18) -> list[dict]:
    """床・天井・4 壁の配置。GPU 不要。"""
    half = float(half)
    height = float(height)
    thick = max(0.04, float(thick))
    if half < 0.5:
        raise ValueError("room half must be >= 0.5")
    if height < 0.8:
        raise ValueError("room height must be >= 0.8")
    span = half * 2.0
    mid_y = height * 0.5
    return [
        {
            "kind": "floor", "model": "plane",
            "x": 0.0, "y": 0.0, "z": 0.0,
            "sx": span, "sy": 1.0, "sz": span,
            "collision": False, "color": (118, 78, 48),
        },
        {
            "kind": "ceiling", "model": "box",
            "x": 0.0, "y": height, "z": 0.0,
            "sx": span, "sy": thick, "sz": span,
            "collision": False, "color": (232, 224, 212),
        },
        {
            "kind": "wall", "model": "box",
            "x": 0.0, "y": mid_y, "z": -half,
            "sx": span, "sy": height, "sz": thick,
            "collision": True, "color": (214, 204, 190),
        },
        {
            "kind": "wall", "model": "box",
            "x": 0.0, "y": mid_y, "z": half,
            "sx": span, "sy": height, "sz": thick,
            "collision": True, "color": (214, 204, 190),
        },
        {
            "kind": "wall", "model": "box",
            "x": -half, "y": mid_y, "z": 0.0,
            "sx": thick, "sy": height, "sz": span,
            "collision": True, "color": (214, 204, 190),
        },
        {
            "kind": "wall", "model": "box",
            "x": half, "y": mid_y, "z": 0.0,
            "sx": thick, "sy": height, "sz": span,
            "collision": True, "color": (214, 204, 190),
        },
    ]


def point_in_room(
    x: float,
    y: float,
    z: float,
    half: float = 6.0,
    height: float = 3.2,
    thick: float = 0.18,
) -> bool:
    """壁の内側（厚みの半分を除く）に点が入るか。"""
    inner = float(half) - max(0.04, float(thick)) * 0.5
    return (
        abs(float(x)) < inner
        and abs(float(z)) < inner
        and 0.0 < float(y) < float(height) - max(0.04, float(thick)) * 0.5
    )


def room(
    half: float = 6.0,
    height: float = 3.2,
    *,
    thick: float = 0.18,
    world: Optional[World3D] = None,
    look: bool = True,
    textured: bool = True,
) -> list[Prop]:
    """閉じた部屋を置く。初回だけ ``apply_room_look``。``sky()`` の室内版。"""
    if look:
        try:
            from kagra.look import apply_room_look
            apply_room_look()
        except Exception:
            pass
    floor_tex = wall_tex = ceil_tex = 0
    if textured:
        try:
            from kagra.look import room_ceiling_texture, room_floor_texture, room_wall_texture
            floor_tex = room_floor_texture()
            wall_tex = room_wall_texture()
            ceil_tex = room_ceiling_texture()
        except Exception:
            floor_tex = wall_tex = ceil_tex = 0
    props: list[Prop] = []
    for spec in room_layout(half, height, thick):
        kind = spec["kind"]
        if kind == "floor":
            tex = floor_tex
        elif kind == "ceiling":
            tex = ceil_tex
        else:
            tex = wall_tex
        props.append(
            Prop(
                spec["model"],
                x=spec["x"], y=spec["y"], z=spec["z"],
                scale=(spec["sx"], spec["sy"], spec["sz"]),
                color=spec["color"],
                collision=spec["collision"],
                world=world,
                texture=tex,
            )
        )
    return props


def destroy(prop) -> None:
    """``Prop.destroy()`` の別名。既に消えていても落ちない。"""
    if prop is None:
        return
    fn = getattr(prop, "destroy", None)
    if fn is not None:
        fn()


PARENT_MAX_LEVELS = 4


class Prop:
    """色付きプリミティブ、または静的 glTF 部品。位置は中心。

    ``texture`` は ``texture_from_fn`` / ``load`` の ID。0 なら ``color``。
    ``normal`` は接空間法線テクスチャ ID（``srgb=False``）。glTF は ``normalTexture``。
    ``model`` が ``.glb`` / ``.gltf`` ならファイルを畳んで置く（``stage()`` ではない）。
    ``metallic`` / ``roughness`` は汎用メッシュだけ。省略時は glTF の因子、無ければ 0 / 1。
    親子は 4 段まで（``set_parent``、玄孫可）。子の ``x,y,z,yaw`` は親からのローカル。
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
        texture: int = 0,
        parent: Optional["Prop"] = None,
        metallic: float | None = None,
        roughness: float | None = None,
        mesh_hit: bool = False,
        normal: int = 0,
    ):
        self.gltf_path = None
        self._gltf_flat: Optional[FlatMesh] = None
        self._mesh_sx = self._mesh_sy = self._mesh_sz = 1.0
        raw = str(model)
        if is_gltf_name(raw):
            self.model = "gltf"
            self.gltf_path = resolve_gltf_path(raw)
            flat = flatten_gltf(self.gltf_path)
            self._gltf_flat = flat
            minx, miny, minz, maxx, maxy, maxz = flat.aabb
            self._mesh_sx = max(maxx - minx, 1e-6)
            self._mesh_sy = max(maxy - miny, 1e-6)
            self._mesh_sz = max(maxz - minz, 1e-6)
        else:
            self.model = raw.lower()
            if self.model not in ("box", "sphere", "cylinder", "plane"):
                raise ValueError(f"unknown model {model!r} (box/sphere/cylinder/plane or .glb/.gltf)")
        self._x = float(x)
        self._y = float(y)
        self._z = float(z)
        self._yaw = float(yaw)
        self._enabled = True
        self._destroyed = False
        self.vx = 0.0
        self.vy = 0.0
        self.vz = 0.0
        if isinstance(scale, (int, float)):
            self.sx = self.sy = self.sz = float(scale)
        else:
            self.sx, self.sy, self.sz = (float(scale[0]), float(scale[1]), float(scale[2]))
        self.color = resolve_color(color)
        self.collision = bool(collision)
        self.mesh_hit = bool(mesh_hit)
        self.world = world
        self.texture = int(texture or 0)
        self.normal = int(normal or 0)
        if metallic is None and self._gltf_flat is not None:
            self.metallic = float(self._gltf_flat.metallic)
        else:
            self.metallic = 0.0 if metallic is None else float(metallic)
        if roughness is None and self._gltf_flat is not None:
            self.roughness = float(self._gltf_flat.roughness)
        else:
            self.roughness = 1.0 if roughness is None else float(roughness)
        self.base_color = (
            tuple(self._gltf_flat.base_color) if self._gltf_flat is not None
            else (1.0, 1.0, 1.0)
        )
        self.tex_id = 0
        self.normal_tex_id = 0
        self.mesh_id = 0
        self.body = None
        self._parent: Optional[Prop] = None
        self._children: list[Prop] = []
        if world is not None and self.collision and self.model != "plane":
            self.body = self._make_body(world)
            self._sync_body()
        Prop._all.append(self)
        if parent is not None:
            # constructor x,y,z,yaw are local to that parent
            self.set_parent(parent, keep_world=False)

    def _hit_radius(self) -> float:
        """球は外接球、円柱は XZ 半径。"""
        if self.model == "sphere":
            return 0.5 * max(abs(self.sx), abs(self.sy), abs(self.sz))
        return 0.5 * max(abs(self.sx), abs(self.sz))

    def _world_mesh_verts(self) -> tuple[list, list]:
        """glTF を世界座標の三角形にする。"""
        flat = self._gltf_flat
        if flat is None:
            return [], []
        x, y, z, yaw = self.world_pose()
        c, s = math.cos(yaw), math.sin(yaw)
        out = []
        for v in flat.verts:
            px, py, pz = v[0] * self.sx, v[1] * self.sy, v[2] * self.sz
            wx = c * px + s * pz + x
            wy = py + y
            wz = -s * px + c * pz + z
            out.append((wx, wy, wz))
        return out, list(flat.indices)

    def _make_body(self, world: World3D):
        if self.mesh_hit and self._gltf_flat is not None:
            verts, idx = self._world_mesh_verts()
            return world.add_trimesh(verts, idx)
        if self.model == "sphere":
            r = self._hit_radius()
            return world.add_sphere(self._x, self._y - r, self._z, r)
        if self.model == "cylinder":
            r = self._hit_radius()
            return world.add_cylinder(
                self._x, self._y - self.sy * 0.5, self._z, r, abs(self.sy),
            )
        w, h, d = prop_hit_extents(self)
        bottom = self._y - h * 0.5
        return world.add_box(
            self._x, bottom, self._z, w, h, d,
            draw=False,
        )

    @property
    def x(self) -> float:
        return self._x

    @x.setter
    def x(self, v: float) -> None:
        self._x = float(v)
        self._sync_body()

    @property
    def y(self) -> float:
        return self._y

    @y.setter
    def y(self, v: float) -> None:
        self._y = float(v)
        self._sync_body()

    @property
    def z(self) -> float:
        return self._z

    @z.setter
    def z(self, v: float) -> None:
        self._z = float(v)
        self._sync_body()

    @property
    def yaw(self) -> float:
        return self._yaw

    @yaw.setter
    def yaw(self, v: float) -> None:
        self._yaw = float(v)
        self._sync_body()

    @property
    def enabled(self) -> bool:
        if self._destroyed or not self._enabled:
            return False
        if self._parent is not None and not self._parent.enabled:
            return False
        return True

    @enabled.setter
    def enabled(self, v: bool) -> None:
        self._enabled = bool(v) and not self._destroyed
        self._sync_body()

    @property
    def parent(self) -> Optional["Prop"]:
        return self._parent

    @property
    def world_x(self) -> float:
        return self.world_pose()[0]

    @property
    def world_y(self) -> float:
        return self.world_pose()[1]

    @property
    def world_z(self) -> float:
        return self.world_pose()[2]

    @property
    def world_yaw(self) -> float:
        return self.world_pose()[3]

    def world_pose(self) -> tuple[float, float, float, float]:
        """世界の ``(x, y, z, yaw)``。"""
        if self._parent is None:
            return self._x, self._y, self._z, self._yaw
        px, py, pz, pyaw = self._parent.world_pose()
        wx, wz = _offset_xz(px, pz, pyaw, self._x, self._z)
        return wx, py + self._y, wz, pyaw + self._yaw

    def set_parent(self, parent: Optional["Prop"], *, keep_world: bool = True) -> None:
        """親を最大 4 段（玄孫まで）。``keep_world`` なら今の世界位置を保つ。"""
        if parent is self:
            raise ValueError("prop cannot parent itself")
        if parent is not None:
            if parent._destroyed:
                raise ValueError("parent is destroyed")

            def _depth(p: "Prop") -> int:
                n = 0
                cur: Optional[Prop] = p
                while cur is not None:
                    n += 1
                    cur = cur._parent
                    if n > 8:
                        break
                return n

            def _height(p: "Prop") -> int:
                if not p._children:
                    return 0
                return 1 + max(_height(ch) for ch in p._children)

            if _depth(parent) + 1 + _height(self) > PARENT_MAX_LEVELS + 1:
                raise ValueError(f"parent is {PARENT_MAX_LEVELS} levels only")
        if keep_world:
            wx, wy, wz, wyaw = self.world_pose()
            if parent is not None:
                px, py, pz, pyaw = parent.world_pose()
                c, s = math.cos(-pyaw), math.sin(-pyaw)
                dx, dz = wx - px, wz - pz
                self._x = c * dx + s * dz
                self._z = -s * dx + c * dz
                self._y = wy - py
                self._yaw = wyaw - pyaw
            else:
                self._x, self._y, self._z, self._yaw = wx, wy, wz, wyaw
        if self._parent is not None and self in self._parent._children:
            self._parent._children.remove(self)
        self._parent = parent
        if parent is not None and self not in parent._children:
            parent._children.append(self)
        self._sync_body()

    def _sync_body(self) -> None:
        wx, wy, wz, wyaw = self.world_pose()
        if self.body is not None:
            if self.mesh_hit and self.body.shape == "trimesh":
                verts, idx = self._world_mesh_verts()
                self.body.set_trimesh(verts, idx)
            elif self.model == "sphere":
                r = self._hit_radius()
                self.body.x = wx
                self.body.y = wy - r
                self.body.z = wz
                self.body.radius = r
                self.body.h = r * 2.0
                self.body.w = self.body.d = r * 2.0
            else:
                w, h, d = prop_hit_extents(self)
                self.body.x = wx
                self.body.y = wy - h * 0.5
                self.body.z = wz
                self.body.w = w
                self.body.h = h
                self.body.d = d
                if self.model == "cylinder":
                    r = self._hit_radius()
                    self.body.radius = r
                    self.body.h = abs(self.sy)
                    self.body.w = self.body.d = r * 2.0
            self.body.yaw = wyaw
            self.body.active = self.enabled
        for ch in list(self._children):
            ch._sync_body()

    def set_position(self, x: float, y: float, z: float) -> None:
        """中心を置く。当たりも一緒に動く。"""
        self._x, self._y, self._z = float(x), float(y), float(z)
        self._sync_body()

    def move(self, dx: float = 0.0, dy: float = 0.0, dz: float = 0.0) -> None:
        """相対移動。"""
        self.set_position(self._x + float(dx), self._y + float(dy), self._z + float(dz))

    def destroy(self) -> None:
        """描画・ホバー・衝突から外す。子も消す。二度呼んでも落ちない。"""
        if self._destroyed:
            return
        for ch in list(self._children):
            ch.destroy()
        self._destroyed = True
        self._enabled = False
        if self._parent is not None and self in self._parent._children:
            self._parent._children.remove(self)
        self._parent = None
        self._sync_body()
        try:
            Prop._all.remove(self)
        except ValueError:
            pass

    def update(self, dt: float) -> None:
        """``vx`` / ``vy`` / ``vz`` を積分する。"""
        if not self.enabled:
            return
        if self.vx == 0.0 and self.vy == 0.0 and self.vz == 0.0:
            return
        dt = float(dt)
        self.set_position(self._x + self.vx * dt, self._y + self.vy * dt, self._z + self.vz * dt)

    def instance(self) -> list[float]:
        wx, wy, wz, wyaw = self.world_pose()
        return [wx, wy, wz, self.sx, self.sy, self.sz, wyaw]

    def world_verts(self, verts) -> list[list[float]]:
        """単位メッシュを位置・スケール・ yaw で変形する（``draw_mesh_instances`` と同じ）。"""
        wx, wy, wz, wyaw = self.world_pose()
        c = math.cos(wyaw)
        s = math.sin(wyaw)
        out: list[list[float]] = []
        for v in verts:
            px, py, pz = v[0] * self.sx, v[1] * self.sy, v[2] * self.sz
            nx, ny, nz = v[3], v[4], v[5]
            out.append([
                c * px + s * pz + wx,
                py + wy,
                -s * px + c * pz + wz,
                c * nx + s * nz,
                ny,
                -s * nx + c * nz,
                v[6], v[7],
            ])
        return out

    def _mesh_data(self):
        if self.model == "gltf" and self._gltf_flat is not None:
            return self._gltf_flat.verts, self._gltf_flat.indices
        return _unit_mesh(self.model)

    def _bake_texture(self) -> int:
        import kagra
        if self.texture:
            return int(self.texture)
        img = getattr(self._gltf_flat, "image", None) if self._gltf_flat is not None else None
        if img:
            import tempfile
            from pathlib import Path

            suffix = ".jpg" if img[:2] == b"\xff\xd8" else ".png"
            path = Path(tempfile.gettempdir()) / f"kagra_prop_gltf{suffix}"
            path.write_bytes(img)
            return int(kagra.load(str(path)))
        return solid_tex(self.color)

    def _bake_normal(self) -> int:
        import kagra
        if self.normal:
            return int(self.normal)
        img = getattr(self._gltf_flat, "normal_image", None) if self._gltf_flat is not None else None
        if img:
            import tempfile
            from pathlib import Path

            suffix = ".jpg" if img[:2] == b"\xff\xd8" else ".png"
            path = Path(tempfile.gettempdir()) / f"kagra_prop_gltf_n{suffix}"
            path.write_bytes(img)
            return int(kagra.load(str(path), srgb=False))
        return 0

    def _draw_immediate(self) -> None:
        import kagra
        tex = self.tex_id or self.texture or solid_tex(self.color)
        verts, idx = self._mesh_data()
        kagra.draw_mesh_3d(int(tex), self.world_verts(verts), idx)

    def bake(self) -> int:
        """テクスチャと単位メッシュを載せる。エンジン未初期化なら 0。"""
        try:
            import kagra
            self.tex_id = self._bake_texture()
            self.normal_tex_id = self._bake_normal()
            key = (
                self.model, str(self.gltf_path or ""), self.tex_id, self.normal_tex_id,
                round(self.metallic, 4), round(self.roughness, 4),
            )
            mid = _unit_cache.get(key)
            if not mid:
                verts, idx = self._mesh_data()
                mid = int(kagra.upload_mesh_3d(
                    self.tex_id, verts, idx,
                    metallic=self.metallic, roughness=self.roughness,
                    base_color=self.base_color,
                    normal_texture_id=self.normal_tex_id,
                ))
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
    def update_all(cls, dt: float) -> None:
        try:
            from kagra.motion import tick_animations
            tick_animations(dt)
        except Exception:
            pass
        for p in list(cls._all):
            p.update(dt)

    @classmethod
    def draw_all(cls) -> None:
        try:
            import kagra
        except Exception:
            return
        batches: dict[int, list["Prop"]] = {}
        fallback: list[Prop] = []
        for p in cls._all:
            if not p.enabled:
                continue
            if not p.mesh_id:
                p.bake()
            if p.mesh_id:
                batches.setdefault(p.mesh_id, []).append(p)
            else:
                fallback.append(p)
        for mid, props in batches.items():
            try:
                kagra.draw_mesh_instances(mid, [p.instance() for p in props])
            except Exception:
                fallback.extend(props)
        for p in fallback:
            try:
                p._draw_immediate()
            except Exception:
                continue

    @classmethod
    def clear(cls) -> None:
        cls._all.clear()


class Walk:
    """WASD / 左スティック + マウス / 右スティック。``jump>0`` なら SPACE / A。

    ``yaw`` は視点（カメラが後ろに付く向き）。``face`` は移動方向で、停止中は
    直前の向きを保つ。三人称の VRM は ``avatar.set_yaw(walk.face)``。
    ``walk.yaw`` をそのまま向きにすると、カメラへ歩く（S）ときに振り返らない。
    """

    def __init__(
        self,
        world: World3D,
        cam,
        *,
        speed: float = 3.2,
        mouse_sens: float = 0.004,
        distance: float = 4.6,
        height: float = 2.2,
        look_y: float = 1.0,
        yaw: float = 0.0,
        first_person: bool = False,
        eye_height: float = 1.55,
        pitch: float = 0.0,
        stick_sens: float = 2.2,
        stick_deadzone: float = 0.2,
        jump: float = 0.0,
        coyote: float = 0.12,
        jump_buffer: float = 0.12,
        lock_cursor: bool | None = None,
    ):
        self.world = world
        self.cam = cam
        self.speed = float(speed)
        self.mouse_sens = float(mouse_sens)
        self.distance = float(distance)
        self.height = float(height)
        self.look_y = float(look_y)
        self.yaw = float(yaw)
        self.face = float(yaw)
        self.pitch = float(pitch)
        self.first_person = bool(first_person)
        self.eye_height = float(eye_height)
        self.stick_sens = float(stick_sens)
        self.stick_deadzone = float(stick_deadzone)
        self.jump = float(jump)
        self.coyote = float(coyote)
        self.jump_buffer = float(jump_buffer)
        self.lock_cursor = lock_cursor
        self.held = None
        self._coyote_left = 0.0
        self._buffer_left = 0.0
        self._locked = False
        self._last_mouse: Optional[tuple[float, float]] = None

    def step(self, dt: float) -> None:
        """``update`` の別名。"""
        self.update(dt)

    def update(self, dt: float) -> None:
        import kagra

        poll_pad()
        if self.lock_cursor is None:
            want_lock = bool(self.first_person)
        else:
            want_lock = bool(self.lock_cursor) and bool(self.first_person)
        if want_lock != self._locked:
            try:
                kagra.set_cursor_locked(want_lock)
                self._locked = want_lock
            except Exception:
                self._locked = False
            self._last_mouse = None
        engine_delta = None
        try:
            eng = kagra.get_engine()
            if eng is not None and getattr(eng, "mouse_delta", None) is not None:
                engine_delta = tuple(kagra.mouse_delta())
        except Exception:
            engine_delta = None
        mouse_pos = None
        if engine_delta is None:
            try:
                mouse_pos = kagra.mouse_pos()
            except Exception:
                mouse_pos = None
        dx, dy, self._last_mouse = pointer_look_delta(
            engine_delta, mouse_pos, self._last_mouse,
        )
        if dx or dy:
            self.yaw = look_yaw(self.yaw, dx, sens=self.mouse_sens)
            if self.first_person:
                self.pitch = look_pitch(self.pitch, dy, sens=self.mouse_sens)
        rx, ry = pad_axis("right")
        if math.hypot(rx, ry) >= self.stick_deadzone:
            dt_look = float(dt)
            self.yaw -= rx * self.stick_sens * dt_look
            if self.first_person:
                self.pitch = look_pitch(self.pitch, ry * self.stick_sens * dt_look, sens=1.0)

        try:
            key_fwd, key_right = walk_key_axes(kagra.key)
        except Exception:
            key_fwd, key_right = 0.0, 0.0
        lx, ly = pad_axis("left")
        fwd, right = walk_axes(lx, ly, key_fwd, key_right, deadzone=self.stick_deadzone)
        vx, vz = walk_wish(fwd, right, self.yaw, self.speed)
        if self.world.in_water():
            vx *= 0.55
            vz *= 0.55
        self.face = facing_yaw(vx, vz, self.face)
        self.world.move_player(vx, vz)
        p = self.world.player
        dt = float(dt)
        if p is not None and self.jump > 0.0:
            grounded = bool(p.on_ground)
            if grounded:
                self._coyote_left = self.coyote
            else:
                self._coyote_left = max(0.0, self._coyote_left - dt)
            if kagra.pressed("SPACE") or kagra.pad_pressed("a"):
                self._buffer_left = self.jump_buffer
            else:
                self._buffer_left = max(0.0, self._buffer_left - dt)
            if self._buffer_left > 0.0:
                vy = jump_vy(
                    grounded,
                    self.world.in_water(p),
                    self.jump,
                    coyote=self._coyote_left > 0.0,
                )
                if vy is not None:
                    p.vy = float(vy)
                    self._buffer_left = 0.0
                    self._coyote_left = 0.0
        if self.held is not None:
            h = self.held
            if getattr(h, "enabled", False) is False or getattr(h, "_destroyed", False):
                self.held = None
            elif p is not None:
                fx = math.sin(self.face)
                fz = math.cos(self.face)
                h.set_parent(None, keep_world=False)
                h.set_position(p.x + fx * 0.85, p.y + 1.15, p.z + fz * 0.85)
        self.world.update(dt)
        p = self.world.player
        if p is None:
            return
        if self.first_person:
            eye, tgt = first_person_eye(
                p.x, p.y, p.z, self.yaw, self.pitch, eye_height=self.eye_height,
            )
            self.cam.look(*eye, *tgt)
        else:
            self.cam.follow(
                p.x, p.y, p.z,
                yaw=self.yaw,
                distance=self.distance,
                height=self.height,
                look_y=self.look_y,
                lerp=0.22,
            )
        eng = kagra.get_engine()
        if eng:
            self.cam.update(eng)

    def carry(self, prop=None) -> None:
        """``prop`` を持つ。``None`` で下ろす。クリック拾いは呼び出し側。"""
        if prop is None:
            self.held = None
            return
        if getattr(prop, "_destroyed", False) or not getattr(prop, "enabled", True):
            return
        try:
            prop.set_parent(None, keep_world=True)
        except Exception:
            pass
        self.held = prop

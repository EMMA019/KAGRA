# kagra/physics3d.py
"""
3D 物理（ゲーム用キャラクターコントローラ。Rapier は使わない）。

2D 物理（physics.py）とは独立。回転積分はせず、Y-up カプセルと
静的 AABB / yaw OBB の押し出し、レイヤー、トリガー、VRM 同期。

Example::
    physics = kagra.Physics3D(gravity=9.8)
    player = physics.add_capsule(0, 1.0, 0, radius=0.25, height=1.7)
    wall = physics.add_obb(3, 0, 0, 0.4, 2.0, 2.0, yaw=0.4, is_static=True)
    zone = physics.add_body(0, 0, 2, 2, 2, 2, trigger=True, is_static=True)

    def update(dt):
        player.vx = speed_x
        player.vz = speed_z
        physics.update(dt)
        physics.sync_vrm(player, avatar)
"""
from __future__ import annotations
import math
from typing import Callable, Optional, Union


# ── AABB 3D ───────────────────────────────────────────────────────

class AABB:
    """軸平行バウンディングボックス（3D）。"""
    __slots__ = ('x','y','z','w','h','d')

    def __init__(self, x: float, y: float, z: float,
                 w: float, h: float, d: float):
        self.x = x  # min X
        self.y = y  # min Y（底面）
        self.z = z  # min Z
        self.w = w  # 幅 X
        self.h = h  # 高さ Y
        self.d = d  # 奥行き Z

    @property
    def cx(self): return self.x + self.w * 0.5
    @property
    def cy(self): return self.y + self.h * 0.5
    @property
    def cz(self): return self.z + self.d * 0.5
    @property
    def max_x(self): return self.x + self.w
    @property
    def max_y(self): return self.y + self.h
    @property
    def max_z(self): return self.z + self.d

    def overlaps(self, other: 'AABB') -> Optional[tuple]:
        """重なりを (dx, dy, dz) で返す。重なりなければ None。"""
        ox = min(self.max_x, other.max_x) - max(self.x, other.x)
        oy = min(self.max_y, other.max_y) - max(self.y, other.y)
        oz = min(self.max_z, other.max_z) - max(self.z, other.z)
        if ox > 0 and oy > 0 and oz > 0:
            return ox, oy, oz
        return None


# ── RigidBody3D ───────────────────────────────────────────────────

class RigidBody3D:
    """3D 剛体。AABB / Y-up カプセル / yaw OBB + 速度積分 + 重力。

    Example::
        body = physics.add_body(x=0, y=0, z=0, w=0.5, h=1.8, d=0.5)
        body.vx = 2.0   # 右に移動
        body.vy = 5.0   # 上にジャンプ
    """

    def __init__(self,
                 x: float, y: float, z: float,
                 w: float, h: float, d: float,
                 is_static: bool = False,
                 restitution: float = 0.0,
                 friction: float = 0.8,
                 shape: str = "aabb",
                 radius: Optional[float] = None,
                 yaw: float = 0.0,
                 layer: int = 1,
                 mask: int = 0xFFFFFFFF,
                 trigger: bool = False):
        # 位置（AABB / カプセルの底面中心）
        self.x = x
        self.y = y
        self.z = z

        # サイズ
        self.w = w
        self.h = h
        self.d = d

        # 速度
        self.vx = 0.0
        self.vy = 0.0
        self.vz = 0.0

        # 物理パラメータ
        self.is_static   = is_static    # True = 動かない壁・床
        self.restitution = restitution  # 反発係数 0.0〜1.0
        self.friction    = friction     # 摩擦係数 0.0〜1.0
        self.use_gravity = True         # 重力を受けるか
        self.on_ground   = False        # 地面に接触中か
        self.active      = True         # False にすると更新スキップ

        # 形状: "aabb" | "capsule" | "obb" | "sphere" | "cylinder"
        # カプセルは常に Y-up。radius 未指定なら min(w, d)*0.5。
        # OBB は Y 軸回り yaw（ラジアン）だけ回る静的向き。
        self.shape = shape
        self.radius = float(radius) if radius is not None else min(w, d) * 0.5
        self.yaw = float(yaw)

        # レイヤー: 衝突は (a.layer & b.mask) かつ (b.layer & a.mask)
        self.layer = int(layer)
        self.mask = int(mask)
        # True = 重なり検出のみ。押し出し・速度反転はしない
        self.trigger = bool(trigger)

        # コールバック
        self.on_collide: Optional[Callable[['RigidBody3D', str], None]] = None

        # ユーザーデータ（Entity や VRM ID を紐づけるのに使う）
        self.user_data = None

    @property
    def aabb(self) -> AABB:
        """現在の AABB を返す（底面左前が原点）。カプセルは外接箱。"""
        if self.shape in ("capsule", "sphere", "cylinder"):
            r = self.radius
            h = self.radius * 2.0 if self.shape == "sphere" else self.h
            return AABB(self.x - r, self.y, self.z - r, r * 2.0, h, r * 2.0)
        return AABB(
            self.x - self.w * 0.5,
            self.y,
            self.z - self.d * 0.5,
            self.w, self.h, self.d,
        )

    def capsule_segment(self) -> tuple:
        """Y-up カプセルの軸端点 (ax,ay,az, bx,by,bz)。球は中心の点。"""
        r = self.radius
        if self.shape == "sphere":
            cy = self.y + r
            return (self.x, cy, self.z, self.x, cy, self.z)
        half = max(0.0, self.h * 0.5 - r)
        cy = self.y + self.h * 0.5
        return (self.x, cy - half, self.z, self.x, cy + half, self.z)

    def sphere_center(self) -> tuple[float, float, float]:
        """球の中心。``y`` は底面。"""
        r = self.radius
        return (self.x, self.y + r, self.z)

    def add_force(self, fx: float, fy: float, fz: float):
        """速度に力を加える（質量1と仮定）。"""
        self.vx += fx
        self.vy += fy
        self.vz += fz

    def teleport(self, x: float, y: float, z: float):
        """位置を瞬間移動（速度はリセットしない）。"""
        self.x, self.y, self.z = x, y, z


# ── Physics3D ─────────────────────────────────────────────────────

class Physics3D:
    """3D 物理システム。

    Example::
        physics = kagra.Physics3D(gravity=9.8)

        player = physics.add_capsule(0, 1.0, 0, 0.25, 1.7)
        wall   = physics.add_body(3, 0.5, 0, 1.0, 1.0, 1.0, is_static=True)

        physics.set_height_fn(lambda x, z: 0.0)

        physics.update(dt)
        physics.sync_vrm(player, avatar)
    """

    def __init__(self, gravity: float = 9.8):
        self.gravity  = gravity     # m/s² (正の値 → 下向き)
        self.bodies:  list[RigidBody3D] = []
        self._height_fn: Optional[Callable[[float,float], float]] = None
        self._ground_y  = 0.0      # デフォルト地面の高さ

    # ── セットアップ ──────────────────────────────────────────────

    def add_body(self,
                 x: float, y: float, z: float,
                 w: float, h: float, d: float,
                 is_static: bool = False,
                 restitution: float = 0.0,
                 friction: float = 0.85,
                 shape: str = "aabb",
                 radius: Optional[float] = None,
                 yaw: float = 0.0,
                 layer: int = 1,
                 mask: int = 0xFFFFFFFF,
                 trigger: bool = False) -> RigidBody3D:
        """剛体を追加して返す。

        Args:
            x, y, z:    初期位置（Y は底面の高さ）
            w, h, d:    サイズ（幅・高さ・奥行き）
            is_static:  True = 壁や床など動かない物体
            restitution: 反発係数 0.0（跳ねない）〜1.0（完全弾性）
            friction:   摩擦係数
            shape:      ``"aabb"`` / ``"capsule"`` / ``"obb"`` / ``"sphere"`` / ``"cylinder"``
            radius:     カプセル半径（省略時は min(w,d)*0.5）
            yaw:        OBB の Y 軸回り（ラジアン）
            layer/mask: ビットマスク。両方の AND が非ゼロなら衝突
            trigger:    True なら重なり通知のみ（押し出さない）
        """
        body = RigidBody3D(x, y, z, w, h, d,
                           is_static=is_static,
                           restitution=restitution,
                           friction=friction,
                           shape=shape,
                           radius=radius,
                           yaw=yaw,
                           layer=layer,
                           mask=mask,
                           trigger=trigger)
        self.bodies.append(body)
        return body

    def add_capsule(self,
                    x: float, y: float, z: float,
                    radius: float, height: float,
                    is_static: bool = False,
                    restitution: float = 0.0,
                    friction: float = 0.85,
                    layer: int = 1,
                    mask: int = 0xFFFFFFFF,
                    trigger: bool = False) -> RigidBody3D:
        """Y-up カプセルを追加する。height は半球を含む全高。"""
        r = float(radius)
        return self.add_body(
            x, y, z, r * 2.0, float(height), r * 2.0,
            is_static=is_static, restitution=restitution, friction=friction,
            shape="capsule", radius=r,
            layer=layer, mask=mask, trigger=trigger,
        )

    def add_sphere(self,
                   x: float, y: float, z: float,
                   radius: float,
                   is_static: bool = True,
                   restitution: float = 0.0,
                   friction: float = 0.85,
                   layer: int = 1,
                   mask: int = 0xFFFFFFFF,
                   trigger: bool = False) -> RigidBody3D:
        """球。``y`` は底面。半径 ``radius``。"""
        r = float(radius)
        return self.add_body(
            x, y, z, r * 2.0, r * 2.0, r * 2.0,
            is_static=is_static, restitution=restitution, friction=friction,
            shape="sphere", radius=r,
            layer=layer, mask=mask, trigger=trigger,
        )

    def add_cylinder(self,
                     x: float, y: float, z: float,
                     radius: float, height: float,
                     is_static: bool = True,
                     restitution: float = 0.0,
                     friction: float = 0.85,
                     layer: int = 1,
                     mask: int = 0xFFFFFFFF,
                     trigger: bool = False) -> RigidBody3D:
        """Y 軸円柱。``y`` は底面。"""
        r = float(radius)
        return self.add_body(
            x, y, z, r * 2.0, float(height), r * 2.0,
            is_static=is_static, restitution=restitution, friction=friction,
            shape="cylinder", radius=r,
            layer=layer, mask=mask, trigger=trigger,
        )

    def add_obb(self,
                x: float, y: float, z: float,
                w: float, h: float, d: float,
                yaw: float = 0.0,
                is_static: bool = True,
                restitution: float = 0.0,
                friction: float = 0.85,
                layer: int = 1,
                mask: int = 0xFFFFFFFF,
                trigger: bool = False) -> RigidBody3D:
        """Y 軸回り yaw の向き付き箱。既定は静的（壁・段差）。"""
        return self.add_body(
            x, y, z, w, h, d,
            is_static=is_static, restitution=restitution, friction=friction,
            shape="obb", yaw=yaw,
            layer=layer, mask=mask, trigger=trigger,
        )

    def remove_body(self, body: RigidBody3D):
        """剛体を削除する。"""
        if body in self.bodies:
            self.bodies.remove(body)

    def set_ground_y(self, y: float = 0.0):
        """地面の高さを設定する（デフォルト 0.0）。"""
        self._ground_y = y
        self._height_fn = None

    def set_height_fn(self, fn: Callable[[float, float], float]):
        """地形高さ関数を設定する。

        Args:
            fn: (x, z) → y を返す関数
        """
        self._height_fn = fn

    def sync_vrm(self, body: RigidBody3D, vrm: Union[int, object]):
        """body の底面位置を VRM ルートオフセットへ書く。

        ``vrm`` は ``vrm_id`` または ``avatar.vrm_id`` を持つオブジェクト。
        エンジン未初期化なら何もしない（テスト可）。
        """
        vid = getattr(vrm, "vrm_id", vrm)
        try:
            vid = int(vid)
        except (TypeError, ValueError):
            return
        try:
            import kagra
            eng = getattr(kagra, "_engine", None)
            if eng is None:
                return
            eng.set_vrm_offset(vid, float(body.x), float(body.y), float(body.z))
        except Exception:
            return

    # ── 毎フレーム更新 ────────────────────────────────────────────

    def update(self, dt: float, max_dt: float = 0.05):
        """物理を更新する。毎フレーム呼ぶ。

        Args:
            dt:     フレーム時間（秒）
            max_dt: 最大ステップ時間（フレームレート低下時の爆発防止）
        """
        dt = min(dt, max_dt)

        for body in self.bodies:
            if body.is_static or not body.active:
                continue
            self._integrate(body, dt)
            body.on_ground = False
            self._ground_collision(body)

        self._solve_collisions()

    def _integrate(self, body: RigidBody3D, dt: float):
        """速度積分と重力適用。"""
        if body.use_gravity:
            body.vy -= self.gravity * dt

        body.x += body.vx * dt
        body.y += body.vy * dt
        body.z += body.vz * dt

        if body.on_ground:
            damp = max(0.0, 1.0 - body.friction * dt * 10.0)
            body.vx *= damp
            body.vz *= damp

    def _ground_collision(self, body: RigidBody3D):
        """地面との衝突を解決する。"""
        if self._height_fn is not None:
            gy = self._height_fn(body.x, body.z)
        else:
            gy = self._ground_y

        if body.y < gy:
            body.y = gy
            if body.vy < 0:
                body.vy = -body.vy * body.restitution
                if abs(body.vy) < 0.1:
                    body.vy = 0.0
            body.on_ground = True
        elif (body.y - gy) < 0.01:
            body.on_ground = True

    def _solves(self, a: RigidBody3D, b: RigidBody3D) -> bool:
        if not a.active or not b.active:
            return False
        if a.is_static and b.is_static:
            return False
        return (a.layer & b.mask) != 0 and (b.layer & a.mask) != 0

    def _solve_collisions(self):
        """形状に応じた衝突。トリガーは通知のみ。"""
        n = len(self.bodies)
        for i in range(n):
            a = self.bodies[i]
            for j in range(i + 1, n):
                b = self.bodies[j]
                if not self._solves(a, b):
                    continue
                hit = _collide_pair(a, b)
                if hit is None:
                    continue
                nx, ny, nz, pen = hit
                kind = "trigger" if (a.trigger or b.trigger) else "hit"
                if kind == "hit":
                    self._resolve_normal(a, b, nx, ny, nz, pen)
                if a.on_collide:
                    a.on_collide(b, kind)
                if b.on_collide:
                    b.on_collide(a, kind)

    def _resolve_normal(self, a: RigidBody3D, b: RigidBody3D,
                        nx: float, ny: float, nz: float, pen: float):
        """法線 (a→b) と侵入量で押し戻す。"""
        rest = (a.restitution + b.restitution) * 0.5
        if a.is_static:
            b.x += nx * pen
            b.y += ny * pen
            b.z += nz * pen
            dv = b.vx * nx + b.vy * ny + b.vz * nz
            if dv < 0:
                b.vx -= (1 + rest) * dv * nx
                b.vy -= (1 + rest) * dv * ny
                b.vz -= (1 + rest) * dv * nz
            # 静的 a から動的 b を +Y へ押したら接地
            if ny > 0.5:
                b.on_ground = True
        elif b.is_static:
            a.x -= nx * pen
            a.y -= ny * pen
            a.z -= nz * pen
            dv = a.vx * nx + a.vy * ny + a.vz * nz
            if dv > 0:
                a.vx -= (1 + rest) * dv * nx
                a.vy -= (1 + rest) * dv * ny
                a.vz -= (1 + rest) * dv * nz
            # 法線は a→b。箱が下なら ny<0 で a を上へ戻す
            if ny < -0.5:
                a.on_ground = True
        else:
            a.x -= nx * pen * 0.5
            a.y -= ny * pen * 0.5
            a.z -= nz * pen * 0.5
            b.x += nx * pen * 0.5
            b.y += ny * pen * 0.5
            b.z += nz * pen * 0.5
            dvx = a.vx - b.vx
            dvy = a.vy - b.vy
            dvz = a.vz - b.vz
            dv = dvx * nx + dvy * ny + dvz * nz
            if dv > 0:
                imp = (1 + rest) * dv * 0.5
                a.vx -= imp * nx
                a.vy -= imp * ny
                a.vz -= imp * nz
                b.vx += imp * nx
                b.vy += imp * ny
                b.vz += imp * nz

    # ── ユーティリティ ────────────────────────────────────────────

    def raycast(self, ox: float, oy: float, oz: float,
                dx: float, dy: float, dz: float,
                max_dist: float = 100.0) -> Optional[tuple]:
        """レイキャスト。AABB / カプセル / OBB。

        Returns:
            (body, distance, hit_x, hit_y, hit_z) または None
        """
        length = math.sqrt(dx * dx + dy * dy + dz * dz)
        if length < 1e-8:
            return None
        dx /= length
        dy /= length
        dz /= length

        best_t = max_dist
        best_body = None

        for body in self.bodies:
            if not body.active:
                continue
            t = _ray_body(ox, oy, oz, dx, dy, dz, body)
            if t is not None and 0 < t < best_t:
                best_t = t
                best_body = body

        if best_body is None:
            return None

        return (best_body, best_t,
                ox + dx * best_t, oy + dy * best_t, oz + dz * best_t)


# ── 衝突ヘルパ ────────────────────────────────────────────────────

def _is_round(shape: str) -> bool:
    return shape in ("capsule", "sphere")


def _collide_pair(a: RigidBody3D, b: RigidBody3D) -> Optional[tuple]:
    """(nx, ny, nz, pen)。法線は a→b。重ならなければ None。"""
    sa, sb = a.shape, b.shape
    if _is_round(sa) and _is_round(sb):
        return _round_round(a, b)
    if _is_round(sa):
        return _capsule_solid(a, b)
    if _is_round(sb):
        hit = _capsule_solid(b, a)
        if hit is None:
            return None
        nx, ny, nz, pen = hit
        return (-nx, -ny, -nz, pen)
    if sa == "obb" or sb == "obb":
        return _obb_pair(a, b)
    return _aabb_aabb(a, b)


def _aabb_aabb(a: RigidBody3D, b: RigidBody3D) -> Optional[tuple]:
    overlap = a.aabb.overlaps(b.aabb)
    if overlap is None:
        return None
    ox, oy, oz = overlap
    if ox <= oy and ox <= oz:
        nx = 1.0 if a.x < b.x else -1.0
        return (nx, 0.0, 0.0, ox)
    if oy <= ox and oy <= oz:
        ny = 1.0 if a.y < b.y else -1.0
        return (0.0, ny, 0.0, oy)
    nz = 1.0 if a.z < b.z else -1.0
    return (0.0, 0.0, nz, oz)


def _capsule_solid(cap: RigidBody3D, solid: RigidBody3D) -> Optional[tuple]:
    """Y-up カプセル vs AABB / yaw OBB / 円柱。法線は capsule→solid。"""
    if solid.shape == "cylinder":
        return _capsule_cylinder(cap, solid)
    if solid.shape == "obb":
        lx, ly, lz = _world_to_obb_local(
            cap.x, cap.y + cap.h * 0.5, cap.z, solid)
        hit = _capsule_vs_aabb_centered(lx, ly, lz, cap.radius, cap.h,
                                        _obb_local_aabb(solid))
        if hit is None:
            return None
        lnx, lny, lnz, pen = hit
        nx, ny, nz = _obb_local_dir_to_world(lnx, lny, lnz, solid)
        return (nx, ny, nz, pen)
    cy = cap.y + cap.h * 0.5
    return _capsule_vs_aabb_centered(cap.x, cy, cap.z, cap.radius, cap.h, solid.aabb)


def _capsule_vs_aabb_centered(cx, cy, cz, radius, height, aabb: AABB) -> Optional[tuple]:
    """中心 (cx,cy,cz) の Y-up カプセル vs AABB。法線はカプセル→箱。"""
    half = max(0.0, height * 0.5 - radius)
    sy0, sy1 = cy - half, cy + half
    qx = min(max(cx, aabb.x), aabb.max_x)
    qz = min(max(cz, aabb.z), aabb.max_z)
    dx, dz = qx - cx, qz - cz
    dist_xz = math.sqrt(dx * dx + dz * dz)

    y_overlap = min(sy1, aabb.max_y) - max(sy0, aabb.y)
    if y_overlap > 0:
        if dist_xz >= radius:
            return None
        if dist_xz > 1e-8:
            return (dx / dist_xz, 0.0, dz / dist_xz, radius - dist_xz)
        # 軸が箱の中。深い Y 重なり（壁）は XZ 優先。床・天井は Y。
        xz_faces = (
            (cx - aabb.x + radius, 1.0, 0.0, 0.0),
            (aabb.max_x - cx + radius, -1.0, 0.0, 0.0),
            (cz - aabb.z + radius, 0.0, 0.0, 1.0),
            (aabb.max_z - cz + radius, 0.0, 0.0, -1.0),
        )
        y_faces = (
            (sy0 - aabb.y, 0.0, 1.0, 0.0),
            (aabb.max_y - sy1, 0.0, -1.0, 0.0),
        )
        if y_overlap > radius * 2.0:
            pen, nx, ny, nz = min(xz_faces, key=lambda f: f[0])
        else:
            pen, nx, ny, nz = min(xz_faces + y_faces, key=lambda f: f[0])
        return (nx, ny, nz, max(pen, 1e-4))

    if sy0 >= aabb.max_y:
        y_dist = sy0 - aabb.max_y
        if dist_xz > 1e-8:
            dist = math.sqrt(dist_xz * dist_xz + y_dist * y_dist)
            if dist >= radius:
                return None
            return (dx / dist, -y_dist / dist, dz / dist, radius - dist)
        if y_dist >= radius:
            return None
        return (0.0, -1.0, 0.0, radius - y_dist)

    if sy1 <= aabb.y:
        y_dist = aabb.y - sy1
        if dist_xz > 1e-8:
            dist = math.sqrt(dist_xz * dist_xz + y_dist * y_dist)
            if dist >= radius:
                return None
            return (dx / dist, y_dist / dist, dz / dist, radius - dist)
        if y_dist >= radius:
            return None
        return (0.0, 1.0, 0.0, radius - y_dist)
    return None


def _capsule_cylinder(cap: RigidBody3D, cyl: RigidBody3D) -> Optional[tuple]:
    """Y-up カプセル vs Y 軸円柱（平らな蓋）。法線は capsule→cylinder。"""
    ra = cap.radius
    rc = cyl.radius
    cx, cz = cyl.x, cyl.z
    y0, y1 = cyl.y, cyl.y + cyl.h
    cy = cap.y + cap.h * 0.5
    half = max(0.0, cap.h * 0.5 - ra)
    sy0, sy1 = cy - half, cy + half

    dx, dz = cx - cap.x, cz - cap.z
    dist_xz = math.sqrt(dx * dx + dz * dz)
    need = ra + rc
    y_overlap = min(sy1, y1) - max(sy0, y0)

    if y_overlap > 0:
        if dist_xz >= need:
            return None
        if dist_xz > 1e-8:
            return (dx / dist_xz, 0.0, dz / dist_xz, need - dist_xz)
        return (1.0, 0.0, 0.0, need)

    if sy0 >= y1:
        y_dist = sy0 - y1
        if dist_xz <= rc:
            if y_dist >= ra:
                return None
            return (0.0, -1.0, 0.0, ra - y_dist)
        rx = dist_xz - rc
        dist = math.sqrt(rx * rx + y_dist * y_dist)
        if dist >= ra or dist_xz < 1e-8:
            return None
        return (dx / dist_xz * (rx / dist), -y_dist / dist, dz / dist_xz * (rx / dist), ra - dist)

    if sy1 <= y0:
        y_dist = y0 - sy1
        if dist_xz <= rc:
            if y_dist >= ra:
                return None
            return (0.0, 1.0, 0.0, ra - y_dist)
        rx = dist_xz - rc
        dist = math.sqrt(rx * rx + y_dist * y_dist)
        if dist >= ra or dist_xz < 1e-8:
            return None
        return (dx / dist_xz * (rx / dist), y_dist / dist, dz / dist_xz * (rx / dist), ra - dist)
    return None


def _round_round(a: RigidBody3D, b: RigidBody3D) -> Optional[tuple]:
    """カプセル / 球同士。球はゼロ長線分にしない。"""
    if a.shape == "sphere" and b.shape == "sphere":
        return _sphere_sphere(a, b)
    if a.shape == "sphere":
        hit = _capsule_sphere(b, a)
        if hit is None:
            return None
        nx, ny, nz, pen = hit
        return (-nx, -ny, -nz, pen)
    if b.shape == "sphere":
        return _capsule_sphere(a, b)
    return _capsule_capsule(a, b)


def _sphere_sphere(a: RigidBody3D, b: RigidBody3D) -> Optional[tuple]:
    ax, ay, az = a.sphere_center()
    bx, by, bz = b.sphere_center()
    dx, dy, dz = bx - ax, by - ay, bz - az
    dist2 = dx * dx + dy * dy + dz * dz
    need = a.radius + b.radius
    if dist2 > 1e-12:
        dist = math.sqrt(dist2)
        if dist >= need:
            return None
        return (dx / dist, dy / dist, dz / dist, need - dist)
    return (1.0, 0.0, 0.0, need)


def _capsule_sphere(cap: RigidBody3D, sph: RigidBody3D) -> Optional[tuple]:
    """法線は capsule→sphere。"""
    sx, sy, sz = sph.sphere_center()
    ax, ay, az, bx, by, bz = cap.capsule_segment()
    px, py, pz = _closest_point_segment(sx, sy, sz, ax, ay, az, bx, by, bz)
    dx, dy, dz = sx - px, sy - py, sz - pz
    dist2 = dx * dx + dy * dy + dz * dz
    need = cap.radius + sph.radius
    if dist2 > 1e-12:
        dist = math.sqrt(dist2)
        if dist >= need:
            return None
        return (dx / dist, dy / dist, dz / dist, need - dist)
    return (1.0, 0.0, 0.0, need)


def _capsule_capsule(a: RigidBody3D, b: RigidBody3D) -> Optional[tuple]:
    a0 = a.capsule_segment()
    b0 = b.capsule_segment()
    p, q = _closest_points_segments(a0, b0)
    dx, dy, dz = q[0] - p[0], q[1] - p[1], q[2] - p[2]
    dist2 = dx * dx + dy * dy + dz * dz
    min_d = a.radius + b.radius
    if dist2 > 1e-12:
        dist = math.sqrt(dist2)
        if dist >= min_d:
            return None
        return (dx / dist, dy / dist, dz / dist, min_d - dist)
    # 軸が重なったら Y で分ける
    ny = 1.0 if a.y < b.y else -1.0
    return (0.0, ny, 0.0, min_d)


def _obb_pair(a: RigidBody3D, b: RigidBody3D) -> Optional[tuple]:
    """AABB または yaw OBB 同士。XZ は SAT、Y は区間。"""
    ay0, ay1 = a.y, a.y + a.h
    by0, by1 = b.y, b.y + b.h
    oy = min(ay1, by1) - max(ay0, by0)
    if oy <= 0:
        return None
    hit = _sat_xz(a, b)
    if hit is None:
        return None
    nx, nz, pen_xz = hit
    if oy <= pen_xz:
        ny = 1.0 if a.y < b.y else -1.0
        return (0.0, ny, 0.0, oy)
    return (nx, 0.0, nz, pen_xz)


def _sat_xz(a: RigidBody3D, b: RigidBody3D) -> Optional[tuple]:
    """XZ 平面の OBB SAT。(nx, nz, pen) は a→b。"""
    axes = _xz_axes(a.yaw if a.shape == "obb" else 0.0)
    axes += _xz_axes(b.yaw if b.shape == "obb" else 0.0)
    best_pen = float("inf")
    best_n = (1.0, 0.0)
    for ax, az in axes:
        al, ah = _project_xz(a, ax, az)
        bl, bh = _project_xz(b, ax, az)
        overlap = min(ah, bh) - max(al, bl)
        if overlap <= 0:
            return None
        if overlap < best_pen:
            best_pen = overlap
            # a の中心が軸の負側なら法線を反転
            ac = a.x * ax + a.z * az
            bc = b.x * ax + b.z * az
            if ac > bc:
                best_n = (-ax, -az)
            else:
                best_n = (ax, az)
    return (best_n[0], best_n[1], best_pen)


def _xz_axes(yaw: float) -> list:
    c, s = math.cos(yaw), math.sin(yaw)
    return [(c, s), (-s, c)]


def _project_xz(body: RigidBody3D, ax: float, az: float) -> tuple:
    hw, hd = body.w * 0.5, body.d * 0.5
    if body.shape == "obb":
        c, s = math.cos(body.yaw), math.sin(body.yaw)
        # ローカル (±hw, ±hd) をワールド XZ へ
        corners = (
            (c * hw - s * hd, s * hw + c * hd),
            (c * hw + s * hd, s * hw - c * hd),
            (-c * hw - s * hd, -s * hw + c * hd),
            (-c * hw + s * hd, -s * hw - c * hd),
        )
        dots = [body.x * ax + body.z * az + cx * ax + cz * az for cx, cz in corners]
        return min(dots), max(dots)
    ext = hw * abs(ax) + hd * abs(az)
    c = body.x * ax + body.z * az
    return c - ext, c + ext


def _obb_local_aabb(body: RigidBody3D) -> AABB:
    return AABB(-body.w * 0.5, -body.h * 0.5, -body.d * 0.5,
                body.w, body.h, body.d)


def _world_to_obb_local(x: float, y: float, z: float, body: RigidBody3D) -> tuple:
    cy = body.y + body.h * 0.5
    dx, dy, dz = x - body.x, y - cy, z - body.z
    c, s = math.cos(-body.yaw), math.sin(-body.yaw)
    return (c * dx - s * dz, dy, s * dx + c * dz)


def _obb_local_dir_to_world(nx: float, ny: float, nz: float, body: RigidBody3D) -> tuple:
    c, s = math.cos(body.yaw), math.sin(body.yaw)
    return (c * nx + s * nz, ny, -s * nx + c * nz)


def _closest_point_aabb(px: float, py: float, pz: float, aabb: AABB) -> tuple:
    return (
        min(max(px, aabb.x), aabb.max_x),
        min(max(py, aabb.y), aabb.max_y),
        min(max(pz, aabb.z), aabb.max_z),
    )


def _closest_point_segment(px, py, pz, ax, ay, az, bx, by, bz) -> tuple:
    abx, aby, abz = bx - ax, by - ay, bz - az
    ab2 = abx * abx + aby * aby + abz * abz
    if ab2 < 1e-12:
        return ax, ay, az
    t = ((px - ax) * abx + (py - ay) * aby + (pz - az) * abz) / ab2
    t = max(0.0, min(1.0, t))
    return ax + t * abx, ay + t * aby, az + t * abz


def _closest_points_segments(a, b) -> tuple:
    """2 線分の最近接点 (Pa, Pb)。"""
    ax, ay, az, bx, by, bz = a
    cx, cy, cz, dx, dy, dz = b
    ux, uy, uz = bx - ax, by - ay, bz - az
    vx, vy, vz = dx - cx, dy - cy, dz - cz
    wx, wy, wz = ax - cx, ay - cy, az - cz
    uu = ux * ux + uy * uy + uz * uz
    vv = vx * vx + vy * vy + vz * vz
    uv = ux * vx + uy * vy + uz * vz
    uw = ux * wx + uy * wy + uz * wz
    vw = vx * wx + vy * wy + vz * wz
    den = uu * vv - uv * uv
    if den < 1e-12:
        s = 0.0
    else:
        s = max(0.0, min(1.0, (uv * vw - vv * uw) / den))
    if vv < 1e-12:
        t = 0.0
    else:
        t = (uv * s + vw) / vv
        t = max(0.0, min(1.0, t))
        if uu > 1e-12:
            s = max(0.0, min(1.0, (uv * t - uw) / uu))
    return (
        (ax + ux * s, ay + uy * s, az + uz * s),
        (cx + vx * t, cy + vy * t, cz + vz * t),
    )


def _segment_aabb_hit(ax, ay, az, bx, by, bz, radius: float,
                      aabb: AABB) -> Optional[tuple]:
    """線分+半径 vs AABB。法線は線分側→箱。"""
    mx, my, mz = (ax + bx) * 0.5, (ay + by) * 0.5, (az + bz) * 0.5
    qx, qy, qz = _closest_point_aabb(mx, my, mz, aabb)
    px, py, pz = _closest_point_segment(qx, qy, qz, ax, ay, az, bx, by, bz)
    qx, qy, qz = _closest_point_aabb(px, py, pz, aabb)
    px, py, pz = _closest_point_segment(qx, qy, qz, ax, ay, az, bx, by, bz)
    dx, dy, dz = qx - px, qy - py, qz - pz
    dist2 = dx * dx + dy * dy + dz * dz
    if dist2 > 1e-12:
        dist = math.sqrt(dist2)
        if dist >= radius:
            return None
        return (dx / dist, dy / dist, dz / dist, radius - dist)
    # 軸が箱の中。最小面で押し出す
    cx, cy, cz = (ax + bx) * 0.5, (ay + by) * 0.5, (az + bz) * 0.5
    left = (cx - aabb.x) + radius
    right = (aabb.max_x - cx) + radius
    down = (cy - aabb.y) + radius
    up = (aabb.max_y - cy) + radius
    back = (cz - aabb.z) + radius
    fwd = (aabb.max_z - cz) + radius
    faces = (
        (left, -1.0, 0.0, 0.0),
        (right, 1.0, 0.0, 0.0),
        (down, 0.0, -1.0, 0.0),
        (up, 0.0, 1.0, 0.0),
        (back, 0.0, 0.0, -1.0),
        (fwd, 0.0, 0.0, 1.0),
    )
    pen, nx, ny, nz = min(faces, key=lambda f: f[0])
    return (nx, ny, nz, max(pen, radius * 0.01))


def _ray_body(ox, oy, oz, dx, dy, dz, body: RigidBody3D) -> Optional[float]:
    if body.shape == "sphere":
        cx, cy, cz = body.sphere_center()
        return _ray_sphere(ox, oy, oz, dx, dy, dz, cx, cy, cz, body.radius)
    if body.shape == "cylinder":
        return _ray_y_cylinder_capped(
            ox, oy, oz, dx, dy, dz,
            body.x, body.z, body.radius, body.y, body.y + body.h,
        )
    if body.shape == "capsule":
        ax, ay, az, bx, by, bz = body.capsule_segment()
        return _ray_capsule(ox, oy, oz, dx, dy, dz, ax, ay, az, bx, by, bz, body.radius)
    if body.shape == "obb":
        lox, loy, loz = _world_to_obb_local(ox, oy, oz, body)
        # 方向も回す（並進なし）
        c, s = math.cos(-body.yaw), math.sin(-body.yaw)
        ldx = c * dx - s * dz
        ldy = dy
        ldz = s * dx + c * dz
        return _ray_aabb(lox, loy, loz, ldx, ldy, ldz, _obb_local_aabb(body))
    return _ray_aabb(ox, oy, oz, dx, dy, dz, body.aabb)


def _ray_aabb(ox, oy, oz, dx, dy, dz, aabb: AABB) -> Optional[float]:
    """スラブ法による Ray vs AABB 交差判定。交差距離 t を返す。"""
    INF = float("inf")

    def slab(o, d, lo, hi):
        if abs(d) < 1e-8:
            return (-INF, INF) if lo <= o <= hi else (INF, -INF)
        t0 = (lo - o) / d
        t1 = (hi - o) / d
        return (min(t0, t1), max(t0, t1))

    tx0, tx1 = slab(ox, dx, aabb.x, aabb.max_x)
    ty0, ty1 = slab(oy, dy, aabb.y, aabb.max_y)
    tz0, tz1 = slab(oz, dz, aabb.z, aabb.max_z)

    t_enter = max(tx0, ty0, tz0)
    t_exit = min(tx1, ty1, tz1)

    if t_enter > t_exit or t_exit < 0:
        return None
    return t_enter if t_enter >= 0 else t_exit


def _ray_sphere(ox, oy, oz, dx, dy, dz, cx, cy, cz, r) -> Optional[float]:
    lx, ly, lz = ox - cx, oy - cy, oz - cz
    b = lx * dx + ly * dy + lz * dz
    c = lx * lx + ly * ly + lz * lz - r * r
    disc = b * b - c
    if disc < 0:
        return None
    s = math.sqrt(disc)
    t = -b - s
    if t >= 0:
        return t
    t = -b + s
    return t if t >= 0 else None


def _ray_y_cylinder(ox, oy, oz, dx, dy, dz, cx, cz, r, y0, y1) -> Optional[float]:
    """Y 軸平行の有限円柱。"""
    fx, fz = ox - cx, oz - cz
    a = dx * dx + dz * dz
    if a < 1e-12:
        return None
    b = 2.0 * (fx * dx + fz * dz)
    c = fx * fx + fz * fz - r * r
    disc = b * b - 4.0 * a * c
    if disc < 0:
        return None
    s = math.sqrt(disc)
    t0 = (-b - s) / (2.0 * a)
    t1 = (-b + s) / (2.0 * a)
    best = None
    for t in (t0, t1):
        if t < 0:
            continue
        y = oy + dy * t
        if y0 <= y <= y1:
            if best is None or t < best:
                best = t
    return best


def _ray_disk_y(ox, oy, oz, dx, dy, dz, cx: float, cy: float, cz: float, r: float) -> Optional[float]:
    if abs(dy) < 1e-8:
        return None
    t = (cy - oy) / dy
    if t < 0:
        return None
    px = ox + dx * t - cx
    pz = oz + dz * t - cz
    if px * px + pz * pz <= r * r:
        return t
    return None


def _ray_y_cylinder_capped(ox, oy, oz, dx, dy, dz, cx, cz, r, y0, y1) -> Optional[float]:
    """有限円柱 + 上下の円盤。"""
    ts = []
    t = _ray_y_cylinder(ox, oy, oz, dx, dy, dz, cx, cz, r, y0, y1)
    if t is not None:
        ts.append(t)
    for y in (y0, y1):
        t = _ray_disk_y(ox, oy, oz, dx, dy, dz, cx, y, cz, r)
        if t is not None:
            ts.append(t)
    return min(ts) if ts else None


def _ray_capsule(ox, oy, oz, dx, dy, dz,
                 ax, ay, az, bx, by, bz, r) -> Optional[float]:
    """Y-up 前提のカプセル（軸がほぼ +Y）。一般軸でも球2つは正しい。"""
    ts = []
    t = _ray_sphere(ox, oy, oz, dx, dy, dz, ax, ay, az, r)
    if t is not None:
        ts.append(t)
    t = _ray_sphere(ox, oy, oz, dx, dy, dz, bx, by, bz, r)
    if t is not None:
        ts.append(t)
    y0, y1 = (ay, by) if ay <= by else (by, ay)
    t = _ray_y_cylinder(ox, oy, oz, dx, dy, dz, ax, az, r, y0, y1)
    if t is not None:
        ts.append(t)
    return min(ts) if ts else None

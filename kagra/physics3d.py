# kagra/physics3d.py
"""
3D 物理エンジン（Python 実装）

2D 物理（physics.py）とは独立した 3D 専用モジュール。
ECS は使わず、シンプルな手続き型 API で設計。

Example::
    # セットアップ
    physics = kagra.Physics3D(gravity=9.8)

    # 剛体を追加
    player = physics.add_body(x=0, y=1, z=0,
                              w=0.4, h=1.8, d=0.4)  # XYZ の AABB
    box = physics.add_body(x=2, y=0.5, z=0,
                           w=1.0, h=1.0, d=1.0,
                           is_static=True)  # 静的オブジェクト

    # 毎フレーム
    player.vx = speed_x
    player.vz = speed_z
    physics.update(dt)

    # 位置を取得して描画
    kagra.draw_vrm(vrm_id)   # VRM は別途 set_vrm_offset で同期
    x, y, z = player.x, player.y, player.z
"""
from __future__ import annotations
import math
from typing import Callable, Optional


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
    """3D 剛体。AABB 衝突 + 速度積分 + 重力。

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
                 friction: float = 0.8):
        # 位置（AABB の底面中心）
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

        # コールバック
        self.on_collide: Optional[Callable[['RigidBody3D', str], None]] = None

        # ユーザーデータ（Entity や VRM ID を紐づけるのに使う）
        self.user_data = None

    @property
    def aabb(self) -> AABB:
        """現在の AABB を返す（底面左前が原点）。"""
        return AABB(
            self.x - self.w * 0.5,
            self.y,
            self.z - self.d * 0.5,
            self.w, self.h, self.d,
        )

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

        # 剛体を追加
        player = physics.add_body(0, 1.0, 0, 0.4, 1.8, 0.4)
        wall   = physics.add_body(3, 0.5, 0, 1.0, 1.0, 1.0, is_static=True)

        # 地形（高さマップ関数）
        physics.set_height_fn(lambda x, z: 0.0)  # 平らな地面

        # 毎フレーム
        physics.update(dt)
        x, y, z = player.x, player.y, player.z
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
                 friction: float = 0.85) -> RigidBody3D:
        """剛体を追加して返す。

        Args:
            x, y, z:    初期位置（Y は底面の高さ）
            w, h, d:    サイズ（幅・高さ・奥行き）
            is_static:  True = 壁や床など動かない物体
            restitution: 反発係数 0.0（跳ねない）〜1.0（完全弾性）
            friction:   摩擦係数

        Example::
            player = physics.add_body(0, 0, 0, 0.4, 1.8, 0.4)
            floor  = physics.add_body(-10, -0.1, -10,
                                      20, 0.1, 20, is_static=True)
        """
        body = RigidBody3D(x, y, z, w, h, d,
                           is_static=is_static,
                           restitution=restitution,
                           friction=friction)
        self.bodies.append(body)
        return body

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

        Example::
            # 波型の地形
            physics.set_height_fn(
                lambda x, z: math.sin(x) * 0.5
            )
        """
        self._height_fn = fn

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
            self._ground_collision(body)

        # AABB vs AABB 衝突解決
        self._solve_collisions()

    def _integrate(self, body: RigidBody3D, dt: float):
        """速度積分と重力適用。"""
        if body.use_gravity:
            body.vy -= self.gravity * dt

        body.x += body.vx * dt
        body.y += body.vy * dt
        body.z += body.vz * dt

        # 摩擦（地面接触中のみ XZ 方向に適用）
        if body.on_ground:
            damp = max(0.0, 1.0 - body.friction * dt * 10.0)
            body.vx *= damp
            body.vz *= damp

    def _ground_collision(self, body: RigidBody3D):
        """地面との衝突を解決する。"""
        # 地形高さ関数があれば使う
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
        else:
            body.on_ground = (body.y - gy) < 0.01

    def _solve_collisions(self):
        """AABB vs AABB 衝突を解決する（最小軸押し戻し法）。"""
        n = len(self.bodies)
        for i in range(n):
            a = self.bodies[i]
            if not a.active: continue
            for j in range(i + 1, n):
                b = self.bodies[j]
                if not b.active: continue
                if a.is_static and b.is_static: continue

                overlap = a.aabb.overlaps(b.aabb)
                if overlap is None:
                    continue

                ox, oy, oz = overlap
                self._resolve(a, b, ox, oy, oz)

    def _resolve(self, a: RigidBody3D, b: RigidBody3D,
                 ox: float, oy: float, oz: float):
        """最小重なり軸で押し戻す。"""
        # 最小重なり軸を求める
        min_ov = min(ox, oy, oz)

        # 押し戻し方向（a→b の方向）
        if min_ov == ox:
            nx = 1.0 if a.x < b.x else -1.0
            ny = nz = 0.0
            pen = ox
        elif min_ov == oy:
            ny = 1.0 if a.y < b.y else -1.0
            nx = nz = 0.0
            pen = oy
        else:
            nz = 1.0 if a.z < b.z else -1.0
            nx = ny = 0.0
            pen = oz

        # 反発係数（平均）
        rest = (a.restitution + b.restitution) * 0.5

        if a.is_static:
            b.x += nx * pen;  b.y += ny * pen;  b.z += nz * pen
            # 速度反転
            dv = (b.vx*nx + b.vy*ny + b.vz*nz)
            if dv < 0:
                b.vx -= (1+rest)*dv*nx
                b.vy -= (1+rest)*dv*ny
                b.vz -= (1+rest)*dv*nz
        elif b.is_static:
            a.x -= nx * pen;  a.y -= ny * pen;  a.z -= nz * pen
            dv = (a.vx*nx + a.vy*ny + a.vz*nz)
            if dv > 0:
                a.vx -= (1+rest)*dv*nx
                a.vy -= (1+rest)*dv*ny
                a.vz -= (1+rest)*dv*nz
        else:
            # 両方動く → 半分ずつ押し戻す
            a.x -= nx*pen*0.5;  a.y -= ny*pen*0.5;  a.z -= nz*pen*0.5
            b.x += nx*pen*0.5;  b.y += ny*pen*0.5;  b.z += nz*pen*0.5
            # 相対速度を反転
            dvx = a.vx - b.vx
            dvy = a.vy - b.vy
            dvz = a.vz - b.vz
            dv  = dvx*nx + dvy*ny + dvz*nz
            if dv > 0:
                imp = (1+rest)*dv*0.5
                a.vx -= imp*nx;  a.vy -= imp*ny;  a.vz -= imp*nz
                b.vx += imp*nx;  b.vy += imp*ny;  b.vz += imp*nz

        # コールバック
        if a.on_collide: a.on_collide(b, 'hit')
        if b.on_collide: b.on_collide(a, 'hit')

    # ── ユーティリティ ────────────────────────────────────────────

    def raycast(self, ox: float, oy: float, oz: float,
                dx: float, dy: float, dz: float,
                max_dist: float = 100.0) -> Optional[tuple]:
        """レイキャスト（スラブ法）。

        Args:
            ox,oy,oz:  レイの始点
            dx,dy,dz:  レイの方向（正規化不要）
            max_dist:  最大距離

        Returns:
            (body, distance, hit_x, hit_y, hit_z) または None

        Example::
            result = physics.raycast(
                cam_x, cam_y, cam_z,
                look_x, look_y, look_z,
                max_dist=50.0
            )
            if result:
                body, dist, hx, hy, hz = result
        """
        # 方向を正規化
        length = math.sqrt(dx*dx + dy*dy + dz*dz)
        if length < 1e-8:
            return None
        dx /= length; dy /= length; dz /= length

        best_t   = max_dist
        best_body = None

        for body in self.bodies:
            if not body.active:
                continue
            aabb = body.aabb
            t = _ray_aabb(ox, oy, oz, dx, dy, dz, aabb)
            if t is not None and 0 < t < best_t:
                best_t    = t
                best_body = body

        if best_body is None:
            return None

        hx = ox + dx * best_t
        hy = oy + dy * best_t
        hz = oz + dz * best_t
        return (best_body, best_t, hx, hy, hz)


def _ray_aabb(ox, oy, oz, dx, dy, dz, aabb: AABB) -> Optional[float]:
    """スラブ法による Ray vs AABB 交差判定。交差距離 t を返す。"""
    INF = float('inf')

    def slab(o, d, lo, hi):
        if abs(d) < 1e-8:
            return (-INF, INF) if lo <= o <= hi else (INF, -INF)
        t0 = (lo - o) / d
        t1 = (hi - o) / d
        return (min(t0,t1), max(t0,t1))

    tx0, tx1 = slab(ox, dx, aabb.x, aabb.max_x)
    ty0, ty1 = slab(oy, dy, aabb.y, aabb.max_y)
    tz0, tz1 = slab(oz, dz, aabb.z, aabb.max_z)

    t_enter = max(tx0, ty0, tz0)
    t_exit  = min(tx1, ty1, tz1)

    if t_enter > t_exit or t_exit < 0:
        return None
    return t_enter if t_enter >= 0 else t_exit

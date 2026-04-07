# kagra/physics.py
# 物理エンジン（Python実装）
#
# ── 使い方 ──────────────────────────────────────────────────
#
# 1) Rigidbody を Entity に追加:
#   rb = entity.add(Rigidbody(gravity=980.0))
#   rb.vx = 200.0   # 初速
#
# 2) PhysicsSystem を World に登録:
#   physics = PhysicsSystem(gravity=980.0)
#   physics.set_tilemap(tilemap)   # タイルマップ衝突
#
# 3) 毎フレーム更新:
#   physics.update(dt, world)
#
# ── コンポーネント構成 ─────────────────────────────────────
#
#   Rigidbody   速度・重力・力の積分
#   BoxCollider 衝突判定ボックス（Transform からオフセット）
#
# ── 衝突イベント（Event Bus 連携） ────────────────────────
#
#   kagra.emit("collision", {
#       "entity_a": entity_a,
#       "entity_b": entity_b,
#       "overlap_x": ox,
#       "overlap_y": oy,
#   })

from __future__ import annotations
from typing import TYPE_CHECKING, Optional

from kagra.entity import Component

if TYPE_CHECKING:
    from kagra.entity import Entity, World
    from kagra.tilemap import TileMap


# ════════════════════════════════════════════════════════
#  BoxCollider コンポーネント
# ════════════════════════════════════════════════════════

class BoxCollider(Component):
    """AABB（軸平行バウンディングボックス）衝突判定コンポーネント。

    Entity の Transform 座標 + offset で判定矩形を決定する。

    Example::
        col = entity.add(BoxCollider(w=32, h=48, offset_x=0, offset_y=0))
        col.layer = "player"
        col.mask  = ["enemy", "wall"]   # この layer とだけ衝突判定する
    """

    def __init__(
        self,
        w: float,
        h: float,
        offset_x: float = 0.0,
        offset_y: float = 0.0,
        trigger: bool = False,
    ):
        super().__init__()

        self.w        = w
        self.h        = h
        self.offset_x = offset_x
        self.offset_y = offset_y
        self.trigger  = trigger   # True なら押し戻しなし（センサー）

        self.layer: str       = "default"
        self.mask:  list[str] = ["default"]

    @property
    def rect(self) -> tuple[float, float, float, float]:
        """現在の (x, y, w, h) を返す。"""
        t = self.entity.transform
        return (
            t.world_x + self.offset_x,
            t.world_y + self.offset_y,
            self.w,
            self.h,
        )

    def overlaps(self, other: "BoxCollider") -> Optional[tuple[float, float]]:
        """他の BoxCollider との重なりを返す。なければ None。"""
        ax, ay, aw, ah = self.rect
        bx, by, bw, bh = other.rect
        ox = min(ax + aw, bx + bw) - max(ax, bx)
        oy = min(ay + ah, by + bh) - max(ay, by)
        if ox > 0 and oy > 0:
            return ox, oy
        return None

    def contains_point(self, px: float, py: float) -> bool:
        ax, ay, aw, ah = self.rect
        return ax <= px <= ax + aw and ay <= py <= ay + ah


# ════════════════════════════════════════════════════════
#  Rigidbody コンポーネント
# ════════════════════════════════════════════════════════

class Rigidbody(Component):
    """物理挙動コンポーネント。速度・重力・力を積分して Transform を更新する。

    Example::
        rb = entity.add(Rigidbody(gravity=980.0, mass=1.0))

        # ジャンプ
        if on_ground and kagra.key_pressed(kagra.KEY_Z):
            rb.vy = -600.0

        # 横移動
        rb.vx = 200.0 * direction
    """

    def __init__(
        self,
        gravity:   float = 980.0,
        mass:      float = 1.0,
        drag:      float = 0.0,
        bounce:    float = 0.0,
        kinematic: bool  = False,
        max_speed: float = 2000.0,
    ):
        super().__init__()

        self.gravity   = gravity
        self.mass      = max(mass, 0.001)
        self.drag      = drag
        self.bounce    = bounce
        self.kinematic = kinematic
        self.max_speed = max_speed

        self.vx: float = 0.0
        self.vy: float = 0.0

        self._force_x: float = 0.0
        self._force_y: float = 0.0

        self.on_ground: bool  = False
        self.on_wall:   bool  = False
        self.on_ceiling: bool = False

        # 重力を無効化するフラグ（水中・はしごなど）
        self.use_gravity: bool = True

    def add_force(self, fx: float, fy: float):
        """力を加える（F = ma より v += F/m * dt で積分）。"""
        self._force_x += fx
        self._force_y += fy

    def add_impulse(self, ix: float, iy: float):
        """即時速度変化（質量無視）。ジャンプなどに。"""
        self.vx += ix
        self.vy += iy

    def stop(self):
        """速度をゼロにリセット。"""
        self.vx = 0.0
        self.vy = 0.0

    def _integrate(self, dt: float):
        """速度・位置を積分する（PhysicsSystem から呼ばれる）。"""
        if self.kinematic:
            return

        # 力 → 加速度 → 速度
        self.vx += (self._force_x / self.mass) * dt
        self.vy += (self._force_y / self.mass) * dt
        self._force_x = 0.0
        self._force_y = 0.0

        # 重力
        if self.use_gravity:
            self.vy += self.gravity * dt

        # 空気抵抗
        if self.drag > 0:
            factor = max(0.0, 1.0 - self.drag * dt)
            self.vx *= factor
            self.vy *= factor

        # 速度制限
        speed = (self.vx ** 2 + self.vy ** 2) ** 0.5
        if speed > self.max_speed:
            scale = self.max_speed / speed
            self.vx *= scale
            self.vy *= scale

        # Transform に適用
        t = self.entity.transform
        t.x += self.vx * dt
        t.y += self.vy * dt

        # 状態リセット（毎フレーム更新）
        self.on_ground  = False
        self.on_wall    = False
        self.on_ceiling = False


# ════════════════════════════════════════════════════════
#  PhysicsSystem (横スクロールアクション用)
# ════════════════════════════════════════════════════════

class PhysicsSystem:
    """横スクロール用物理システム。毎フレーム update() を呼ぶ。

    処理順:
    1. Rigidbody の積分（速度 → 位置）
    2. タイルマップとの衝突・押し戻し（Y軸を優先）
    3. Entity 間の衝突・押し戻し
    4. 衝突イベントの発火
    """

    def __init__(self, gravity: float = 980.0):
        self.gravity  = gravity
        self._tilemap: Optional["TileMap"] = None
        self.use_events: bool = True   # Event Bus に衝突イベントを発火するか

    def set_tilemap(self, tilemap: "TileMap"):
        """タイルマップ衝突を有効にする。"""
        self._tilemap = tilemap

    def update(self, dt: float, world: "World"):
        """物理を1フレーム進める。"""
        dt = min(dt, 0.05)  # ワープ防止: ウィンドウ最小化後などの大きな dt を抑制

        # Rigidbody を持つ Entity を収集
        rbs = []
        for entity in world.entities:
            if not entity.active or entity.is_destroyed:
                continue
            rb = self._get_component(entity, Rigidbody)
            if rb and rb.enabled:
                rbs.append((entity, rb))

        # 1. 積分
        for entity, rb in rbs:
            rb._integrate(dt)

        # 2. タイルマップ衝突
        if self._tilemap:
            for entity, rb in rbs:
                col = self._get_component(entity, BoxCollider)
                if col and col.enabled:
                    self._resolve_tilemap(entity, rb, col)

        # 3. Entity 間の衝突
        cols = []
        for entity in world.entities:
            if not entity.active or entity.is_destroyed:
                continue
            col = self._get_component(entity, BoxCollider)
            if col and col.enabled:
                cols.append((entity, col))

        checked = set()
        for i, (ea, ca) in enumerate(cols):
            for j, (eb, cb) in enumerate(cols):
                if i >= j:
                    continue
                pair = (id(ea), id(eb))
                if pair in checked:
                    continue
                checked.add(pair)

                # レイヤーマスクチェック
                if cb.layer not in ca.mask and ca.layer not in cb.mask:
                    continue

                overlap = ca.overlaps(cb)
                if overlap is None:
                    continue

                ox, oy = overlap
                self._resolve_entity_collision(ea, ca, eb, cb, ox, oy)

                # Event Bus に通知
                if self.use_events:
                    try:
                        import kagra
                        kagra.emit("collision", {
                            "entity_a": ea,
                            "entity_b": eb,
                            "overlap_x": ox,
                            "overlap_y": oy,
                        })
                    except Exception:
                        pass

    # ── タイルマップ衝突 ──────────────────────────────────

    def _resolve_tilemap(self, entity: "Entity", rb: Rigidbody, col: BoxCollider):
        """タイルマップの SOLID タイルと衝突・押し戻しを行う。"""
        from kagra.tilemap import TILE_SOLID, TILE_WATER, TILE_DAMAGE
        tm = self._tilemap
        t  = entity.transform

        cx, cy, cw, ch = col.rect

        # ── 縦方向（Y）解決 ──────────────────────────────
        if rb.vy >= 0:  # 落下中
            # 足元チェック（左端・中央・右端）
            foot_y = cy + ch
            for check_x in [cx + 2, cx + cw / 2, cx + cw - 2]:
                attrs = tm.get_tile_attrs_at(check_x, foot_y)
                if attrs & TILE_SOLID:
                    tile_top = (int(foot_y) // tm.tile_h) * tm.tile_h
                    penetration = foot_y - tile_top
                    if 0 < penetration <= tm.tile_h:
                        t.y -= penetration
                        if rb.bounce > 0:
                            rb.vy = -abs(rb.vy) * rb.bounce
                        else:
                            rb.vy = 0.0
                        rb.on_ground = True
                        break
        else:  # 上昇中
            # 頭上チェック
            head_y = cy
            for check_x in [cx + 2, cx + cw / 2, cx + cw - 2]:
                attrs = tm.get_tile_attrs_at(check_x, head_y - 1)
                if attrs & TILE_SOLID:
                    tile_bottom = (int(head_y) // tm.tile_h + 1) * tm.tile_h
                    penetration = tile_bottom - head_y
                    if 0 < penetration <= tm.tile_h:
                        t.y += penetration
                        rb.vy = abs(rb.vy) * rb.bounce
                        rb.on_ceiling = True
                        break

        # ── 横方向（X）解決 ──────────────────────────────
        cx, cy, cw, ch = col.rect  # Y 解決後の座標で再計算
        if rb.vx > 0:  # 右移動
            right_x = cx + cw
            for check_y in [cy + 2, cy + ch / 2, cy + ch - 2]:
                attrs = tm.get_tile_attrs_at(right_x, check_y)
                if attrs & TILE_SOLID:
                    tile_left = (int(right_x) // tm.tile_w) * tm.tile_w
                    penetration = right_x - tile_left
                    if 0 < penetration <= tm.tile_w:
                        t.x -= penetration
                        rb.vx = -abs(rb.vx) * rb.bounce
                        rb.on_wall = True
                        break
        elif rb.vx < 0:  # 左移動
            left_x = cx
            for check_y in [cy + 2, cy + ch / 2, cy + ch - 2]:
                attrs = tm.get_tile_attrs_at(left_x - 1, check_y)
                if attrs & TILE_SOLID:
                    tile_right = (int(left_x) // tm.tile_w + 1) * tm.tile_w
                    penetration = tile_right - left_x
                    if 0 < penetration <= tm.tile_w:
                        t.x += penetration
                        rb.vx = abs(rb.vx) * rb.bounce
                        rb.on_wall = True
                        break

    # ── Entity 間衝突解決 ─────────────────────────────────

    def _resolve_entity_collision(
        self,
        ea: "Entity", ca: BoxCollider,
        eb: "Entity", cb: BoxCollider,
        ox: float, oy: float,
    ):
        """2 Entity 間の押し戻しを行う。"""
        # どちらかが trigger なら押し戻しなし
        if ca.trigger or cb.trigger:
            return

        rba = self._get_component(ea, Rigidbody)
        rbb = self._get_component(eb, Rigidbody)

        # 軸の小さい方向に押し戻し
        if ox < oy:
            # X 方向に押し戻し
            push = ox / 2
            ax, *_ = ca.rect
            bx, *_ = cb.rect
            if ax < bx:
                if rba and not rba.kinematic:
                    ea.transform.x -= push
                    rba.vx = min(0.0, rba.vx)
                    rba.on_wall = True
                if rbb and not rbb.kinematic:
                    eb.transform.x += push
                    rbb.vx = max(0.0, rbb.vx)
                    rbb.on_wall = True
            else:
                if rba and not rba.kinematic:
                    ea.transform.x += push
                    rba.vx = max(0.0, rba.vx)
                    rba.on_wall = True
                if rbb and not rbb.kinematic:
                    eb.transform.x -= push
                    rbb.vx = min(0.0, rbb.vx)
                    rbb.on_wall = True
        else:
            # Y 方向に押し戻し
            push = oy / 2
            _, ay, *_ = ca.rect
            _, by, *_ = cb.rect
            if ay < by:
                if rba and not rba.kinematic:
                    ea.transform.y -= push
                    if rba.vy > 0:
                        rba.vy = -rba.vy * rba.bounce
                    rba.on_ground = True
                if rbb and not rbb.kinematic:
                    eb.transform.y += push
                    if rbb.vy < 0:
                        rbb.vy = -rbb.vy * rbb.bounce
                    rbb.on_ceiling = True
            else:
                if rba and not rba.kinematic:
                    ea.transform.y += push
                    if rba.vy < 0:
                        rba.vy = -rba.vy * rba.bounce
                    rba.on_ceiling = True
                if rbb and not rbb.kinematic:
                    eb.transform.y -= push
                    if rbb.vy > 0:
                        rbb.vy = -rbb.vy * rbb.bounce
                    rbb.on_ground = True

    # ── ユーティリティ ────────────────────────────────────

    @staticmethod
    def _get_component(entity: "Entity", comp_class):
        for c in entity.components:
            if isinstance(c, comp_class):
                return c
        return None

    # ── レイキャスト ──────────────────────────────────────

    def raycast(
        self,
        world: "World",
        ox: float, oy: float,
        dx: float, dy: float,
        length: float,
        layer: str = "default",
    ) -> Optional[tuple["Entity", float]]:
        """レイを飛ばして最初に当たった Entity と距離を返す。"""
        mag = (dx ** 2 + dy ** 2) ** 0.5
        if mag == 0:
            return None
        ndx, ndy = dx / mag, dy / mag

        best_t: float = float("inf")
        best_entity   = None

        for entity in world.entities:
            if not entity.active or entity.is_destroyed:
                continue
            col = self._get_component(entity, BoxCollider)
            if not col or not col.enabled:
                continue
            if col.layer != layer:
                continue

            cx, cy, cw, ch = col.rect
            # AABB vs レイ
            t = self._ray_aabb(ox, oy, ndx, ndy, cx, cy, cw, ch)
            if t is not None and 0 <= t <= length and t < best_t:
                best_t      = t
                best_entity = entity

        if best_entity:
            return best_entity, best_t
        return None

    @staticmethod
    def _ray_aabb(
        ox, oy, dx, dy,
        bx, by, bw, bh,
    ) -> Optional[float]:
        """レイと AABB の交差距離 t を返す。交差しなければ None。"""
        t_min = float("-inf")
        t_max = float("inf")

        for o, d, b, bsize in [(ox, dx, bx, bw), (oy, dy, by, bh)]:
            if abs(d) < 1e-8:
                if o < b or o > b + bsize:
                    return None
            else:
                t1 = (b          - o) / d
                t2 = (b + bsize  - o) / d
                if t1 > t2:
                    t1, t2 = t2, t1
                t_min = max(t_min, t1)
                t_max = min(t_max, t2)
                if t_min > t_max:
                    return None

        return t_min if t_min >= 0 else (t_max if t_max >= 0 else None)


# ════════════════════════════════════════════════════════
#  TopDownPhysicsSystem (見下ろし型/トップダウン用)
# ════════════════════════════════════════════════════════

class TopDownPhysicsSystem:
    """見下ろし型（トップダウン）特化の物理システム。
    
    重力を無視し、X軸とY軸の積分・衝突判定を独立して処理することで、
    角で引っかからない滑らかな壁滑り（スライディング）を実現する。
    
    Example::
        physics = TopDownPhysicsSystem()
        physics.set_tilemap(tilemap)

        # 毎フレーム
        physics.update(dt, world)
    """

    def __init__(self):
        self._tilemap: Optional["TileMap"] = None
        self.use_events: bool = True

    def set_tilemap(self, tilemap: "TileMap"):
        self._tilemap = tilemap

    def update(self, dt: float, world: "World"):
        dt = min(dt, 0.05) # ワープ防止の安全装置

        cols = []
        rbs = []
        for entity in world.entities:
            if not entity.active or entity.is_destroyed:
                continue
            col = self._get_component(entity, BoxCollider)
            if col and col.enabled:
                cols.append((entity, col))
                rb = self._get_component(entity, Rigidbody)
                if rb and rb.enabled:
                    rb.on_wall = False
                    rbs.append((entity, rb, col))

        # 1. 速度と位置の更新（XとYを独立して処理）
        for entity, rb, col in rbs:
            if rb.kinematic: continue
            
            rb.vx += (rb._force_x / rb.mass) * dt
            rb.vy += (rb._force_y / rb.mass) * dt
            rb._force_x = 0.0
            rb._force_y = 0.0

            if rb.drag > 0:
                factor = max(0.0, 1.0 - rb.drag * dt)
                rb.vx *= factor
                rb.vy *= factor

            speed = (rb.vx ** 2 + rb.vy ** 2) ** 0.5
            if speed > rb.max_speed:
                scale = rb.max_speed / speed
                rb.vx *= scale
                rb.vy *= scale

            # --- X軸の移動と衝突 ---
            entity.transform.x += rb.vx * dt
            if self._tilemap:
                self._resolve_tilemap_x(entity, rb, col)

            # --- Y軸の移動と衝突 ---
            entity.transform.y += rb.vy * dt
            if self._tilemap:
                self._resolve_tilemap_y(entity, rb, col)

        # 2. Entity同士の衝突解決
        checked = set()
        for i, (ea, ca) in enumerate(cols):
            for j, (eb, cb) in enumerate(cols):
                if i >= j: continue
                pair = (id(ea), id(eb))
                if pair in checked: continue
                checked.add(pair)

                if cb.layer not in ca.mask and ca.layer not in cb.mask:
                    continue

                overlap = ca.overlaps(cb)
                if overlap is None: continue
                ox, oy = overlap

                self._resolve_entity_collision(ea, ca, eb, cb, ox, oy)

                if self.use_events:
                    try:
                        import kagra
                        kagra.emit("collision", {
                            "entity_a": ea,
                            "entity_b": eb,
                            "overlap_x": ox,
                            "overlap_y": oy,
                        })
                    except Exception:
                        pass

    def _resolve_tilemap_x(self, entity: "Entity", rb: Rigidbody, col: BoxCollider):
        from kagra.tilemap import TILE_SOLID
        tm = self._tilemap
        cx, cy, cw, ch = col.rect
        
        if rb.vx > 0:
            right_x = cx + cw
            for check_y in [cy + 2, cy + ch / 2, cy + ch - 2]:
                if tm.get_tile_attrs_at(right_x, check_y) & TILE_SOLID:
                    tile_left = (int(right_x) // tm.tile_w) * tm.tile_w
                    pen = right_x - tile_left
                    if 0 < pen <= tm.tile_w:
                        entity.transform.x -= pen
                        rb.vx = -abs(rb.vx) * rb.bounce
                        rb.on_wall = True
                        break
        elif rb.vx < 0:
            left_x = cx
            for check_y in [cy + 2, cy + ch / 2, cy + ch - 2]:
                if tm.get_tile_attrs_at(left_x - 1, check_y) & TILE_SOLID:
                    tile_right = (int(left_x) // tm.tile_w + 1) * tm.tile_w
                    pen = tile_right - left_x
                    if 0 < pen <= tm.tile_w:
                        entity.transform.x += pen
                        rb.vx = abs(rb.vx) * rb.bounce
                        rb.on_wall = True
                        break

    def _resolve_tilemap_y(self, entity: "Entity", rb: Rigidbody, col: BoxCollider):
        from kagra.tilemap import TILE_SOLID
        tm = self._tilemap
        cx, cy, cw, ch = col.rect
        
        if rb.vy > 0:
            bottom_y = cy + ch
            for check_x in [cx + 2, cx + cw / 2, cx + cw - 2]:
                if tm.get_tile_attrs_at(check_x, bottom_y) & TILE_SOLID:
                    tile_top = (int(bottom_y) // tm.tile_h) * tm.tile_h
                    pen = bottom_y - tile_top
                    if 0 < pen <= tm.tile_h:
                        entity.transform.y -= pen
                        rb.vy = -abs(rb.vy) * rb.bounce
                        rb.on_wall = True
                        break
        elif rb.vy < 0:
            top_y = cy
            for check_x in [cx + 2, cx + cw / 2, cx + cw - 2]:
                if tm.get_tile_attrs_at(check_x, top_y - 1) & TILE_SOLID:
                    tile_bottom = (int(top_y) // tm.tile_h + 1) * tm.tile_h
                    pen = tile_bottom - top_y
                    if 0 < pen <= tm.tile_h:
                        entity.transform.y += pen
                        rb.vy = abs(rb.vy) * rb.bounce
                        rb.on_wall = True
                        break

    def _resolve_entity_collision(self, ea: "Entity", ca: BoxCollider, eb: "Entity", cb: BoxCollider, ox: float, oy: float):
        if ca.trigger or cb.trigger: return
        rba = self._get_component(ea, Rigidbody)
        rbb = self._get_component(eb, Rigidbody)
        
        if ox < oy:
            push = ox / 2
            ax, *_ = ca.rect
            bx, *_ = cb.rect
            if ax < bx:
                if rba and not rba.kinematic:
                    ea.transform.x -= push
                    rba.vx = min(0.0, rba.vx)
                if rbb and not rbb.kinematic:
                    eb.transform.x += push
                    rbb.vx = max(0.0, rbb.vx)
            else:
                if rba and not rba.kinematic:
                    ea.transform.x += push
                    rba.vx = max(0.0, rba.vx)
                if rbb and not rbb.kinematic:
                    eb.transform.x -= push
                    rbb.vx = min(0.0, rbb.vx)
        else:
            push = oy / 2
            _, ay, *_ = ca.rect
            _, by, *_ = cb.rect
            if ay < by:
                if rba and not rba.kinematic:
                    ea.transform.y -= push
                    rba.vy = min(0.0, rba.vy)
                if rbb and not rbb.kinematic:
                    eb.transform.y += push
                    rbb.vy = max(0.0, rbb.vy)
            else:
                if rba and not rba.kinematic:
                    ea.transform.y += push
                    rba.vy = max(0.0, rba.vy)
                if rbb and not rbb.kinematic:
                    eb.transform.y -= push
                    rbb.vy = min(0.0, rbb.vy)

    @staticmethod
    def _get_component(entity: "Entity", comp_class):
        for c in entity.components:
            if isinstance(c, comp_class):
                return c
        return None
# kagra/components.py
# よく使うゲームコンポーネント集
#
# TopDownMovement  : 4方向移動 + TileMap衝突統合
# FourDirAnimator  : 4方向アニメーション（stand/walk×4方向）
# SimpleCamera     : プレイヤー追従カメラコンポーネント

from __future__ import annotations
import math
from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.tilemap import TileMap
    from kagra.camera  import Camera


# ── TopDownMovement ───────────────────────────────────────────
class TopDownMovement:
    """4方向移動 + TileMap衝突判定をまとめたコンポーネント。

    Entityに依存せず単独で使えるよう設計。
    ECSのComponentを継承しないのは意図的（EntitySceneなしでも使えるように）。

    Attributes:
        x, y    : ワールド座標（更新される）
        w, h    : キャラクターの衝突サイズ
        speed   : 移動速度（px/秒）
        vx, vy  : 現在の速度ベクトル
        dir     : 向き "front"/"back"/"left"/"right"
        is_moving: 移動中か
        on_ground : Y衝突があったか（プラットフォーマー用）

    Example::
        self.mover = TopDownMovement(
            x=100, y=100, w=16, h=24,
            speed=120, tilemap=self.tilemap
        )

        def update(self, dt):
            self.mover.update(dt)
            # 座標はmoverが管理
    """

    def __init__(
        self,
        x: float, y: float,
        w: float, h: float,
        speed: float = 120.0,
        tilemap: Optional["TileMap"] = None,
        diagonal: bool = True,
    ):
        self.x = x
        self.y = y
        self.w = w
        self.h = h
        self.speed   = speed
        self.tilemap = tilemap
        self.diagonal = diagonal

        self.vx: float = 0.0
        self.vy: float = 0.0
        self.dir: str  = "front"
        self.is_moving: bool = False
        self.hit_x: bool = False
        self.hit_y: bool = False

    def update(self, dt: float):
        """毎フレーム呼ぶ。入力は update 前に set_input() で渡す。"""
        if not self.is_moving:
            self.vx = 0.0
            self.vy = 0.0
            return

        move_x = self.vx * self.speed * dt
        move_y = self.vy * self.speed * dt

        if self.tilemap is not None:
            new_x, new_y, _, _, self.hit_x, self.hit_y = self.tilemap.collide_move(
                self.x, self.y, self.w, self.h, move_x, move_y
            )
            self.x = new_x
            self.y = new_y
        else:
            self.x += move_x
            self.y += move_y
            self.hit_x = self.hit_y = False

    def set_input(self, dx: float, dy: float):
        """移動方向を設定する。dx/dy は -1〜1 の正規化前の値でOK。

        Args:
            dx: 横方向（-1=左, 1=右, 0=静止）
            dy: 縦方向（-1=上, 1=下, 0=静止）
        """
        self.is_moving = (dx != 0.0 or dy != 0.0)

        if not self.is_moving:
            self.vx = self.vy = 0.0
            return

        # 正規化（斜め移動のスピードを一定に）
        if self.diagonal and dx != 0 and dy != 0:
            length = math.sqrt(dx * dx + dy * dy)
            self.vx = dx / length
            self.vy = dy / length
        else:
            self.vx = dx
            self.vy = dy

        # 向き更新（上下優先→最後に押したキー優先）
        if dy < 0:
            self.dir = "back"
        elif dy > 0:
            self.dir = "front"
        elif dx < 0:
            self.dir = "left"
        elif dx > 0:
            self.dir = "right"

    def read_kagra_input(self):
        """kagra のキー入力を読み取って set_input() を呼ぶ。

        up=KEY_UP/down=KEY_DOWN 等の標準マッピングを使う。
        カスタムキーを使いたい場合は set_input() を直接呼ぶこと。
        """
        import kagra
        dx = dy = 0.0
        if kagra.key_down(kagra.KEY_LEFT):  dx -= 1.0
        if kagra.key_down(kagra.KEY_RIGHT): dx += 1.0
        if kagra.key_down(kagra.KEY_UP):    dy -= 1.0
        if kagra.key_down(kagra.KEY_DOWN):  dy += 1.0
        self.set_input(dx, dy)

    @property
    def center_x(self) -> float:
        return self.x + self.w / 2

    @property
    def center_y(self) -> float:
        return self.y + self.h / 2

    @property
    def rect(self) -> tuple[float, float, float, float]:
        return (self.x, self.y, self.w, self.h)


# ── FourDirAnimator ───────────────────────────────────────────
class FourDirAnimator:
    """4方向 + 歩きアニメーションを管理するコンポーネント。

    テクスチャIDのdict（またはリスト）から適切なフレームを選ぶ。

    テクスチャキー命名規則:
        "front"        : 正面立ちポーズ
        "front_walk1"  : 正面歩き1
        "front_walk2"  : 正面歩き2
        "back"         : 背面立ちポーズ
        ... (left / right も同様)

    Example::
        textures = kagra.assets.preload(
            "player/front", "player/front_walk1", "player/front_walk2",
            "player/back",  "player/back_walk1",  "player/back_walk2",
            "player/left",  "player/left_walk1",  "player/left_walk2",
            "player/right", "player/right_walk1", "player/right_walk2",
        )
        self.anim = FourDirAnimator(textures, frame_rate=8.0)

        def update(self, dt):
            self.anim.update(dt, self.mover.dir, self.mover.is_moving)

        def draw(self):
            tex_id = self.anim.current_texture
            kagra.draw_texture(tex_id, x, y + self.anim.bob_offset, w, h)
    """

    # フレームシーケンス: 0=stand, 1=walk1, 2=stand, 3=walk2
    WALK_CYCLE = [0, 1, 0, 2]

    def __init__(
        self,
        textures: dict[str, int],
        frame_rate: float = 8.0,
        bob_amount: float = 3.0,
    ):
        self.textures    = textures
        self.frame_rate  = frame_rate   # フレーム/秒
        self.bob_amount  = bob_amount   # 歩きボブの振れ幅(px)

        self._timer      = 0.0
        self._walk_frame = 0            # 0〜3 のサイクルインデックス

    def update(self, dt: float, direction: str, is_moving: bool):
        """アニメーションを更新する。毎フレーム呼ぶ。"""
        if is_moving:
            self._timer += dt
            frame_dur = 1.0 / self.frame_rate
            while self._timer >= frame_dur:
                self._timer -= frame_dur
                self._walk_frame = (self._walk_frame + 1) % 4
        else:
            self._walk_frame = 0
            self._timer = 0.0

        self._current_dir = direction
        self._is_moving   = is_moving

    @property
    def current_texture(self) -> Optional[int]:
        """現在描画すべきテクスチャIDを返す。"""
        dir_ = getattr(self, '_current_dir', 'front')
        moving = getattr(self, '_is_moving', False)

        if not moving or self.WALK_CYCLE[self._walk_frame] == 0:
            key = dir_
        elif self.WALK_CYCLE[self._walk_frame] == 1:
            key = f"{dir_}_walk1"
        else:
            key = f"{dir_}_walk2"

        # キーが存在しなければ方向の立ちポーズにフォールバック
        return self.textures.get(key) or self.textures.get(dir_)

    @property
    def bob_offset(self) -> float:
        """歩き時の上下ボブオフセット(px)を返す。"""
        if not getattr(self, '_is_moving', False):
            return 0.0
        bob_table = [0, -self.bob_amount, 0, self.bob_amount]
        return bob_table[self._walk_frame]


# ── CameraFollower ────────────────────────────────────────────
class CameraFollower:
    """カメラをターゲットに追従させるユーティリティ。

    毎フレーム follow() を呼ぶだけでカメラが追従する。

    Example::
        self.cam_follower = CameraFollower(
            camera=self.camera,
            lerp=0.12,          # 0.0=静止 1.0=即時追従
            dead_zone=8.0,      # この範囲内では追従しない(px)
        )

        def update(self, dt):
            self.cam_follower.follow(self.mover.center_x, self.mover.center_y,
                                     self.mover.w, self.mover.h)
    """

    def __init__(
        self,
        camera: "Camera",
        lerp:      float = 0.12,
        dead_zone: float = 0.0,
    ):
        self.camera    = camera
        self.lerp      = lerp
        self.dead_zone = dead_zone

    def follow(self, wx: float, wy: float, obj_w: float = 0, obj_h: float = 0):
        """カメラをターゲットに追従させる。"""
        if self.dead_zone > 0:
            # デッドゾーン: 中心からの距離が dead_zone 以内なら追従しない
            cx = self.camera.x + self.camera.screen_w / 2
            cy = self.camera.y + self.camera.screen_h / 2
            tx = wx + obj_w / 2
            ty = wy + obj_h / 2
            if abs(tx - cx) < self.dead_zone and abs(ty - cy) < self.dead_zone:
                return

        self.camera.follow(wx, wy, obj_w, obj_h, lerp=self.lerp)

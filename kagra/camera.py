# kagra/camera.py
# 2Dカメラ - ワールド座標 ↔ スクリーン座標変換 + 追従 + ズーム + シェイク

from __future__ import annotations
import math
import random


class Camera:
    """2Dカメラ。ワールド座標→スクリーン座標の変換・追従・ズーム・シェイクを担う。

    座標系:
        ワールド座標  : マップ全体の絶対座標
        スクリーン座標: ウィンドウ上のピクセル座標

    Example::
        cam = Camera(screen_w=1280, screen_h=720)
        kagra.set_camera(cam)           # グローバル登録

        # 毎フレーム
        cam.follow(player.x, player.y, player.w, player.h, lerp=0.1)
        cam.update(dt)                  # シェイク減衰を処理

        # ワールド描画（カメラ自動適用）
        kagra.draw_texture_world(tex, wx, wy, w, h)
        kagra.rect_world(wx, wy, w, h, r, g, b)

        # 座標変換が必要な場合
        sx, sy = cam.to_screen(world_x, world_y)
    """

    def __init__(
        self,
        screen_w: float,
        screen_h: float,
        world_w:  float = float("inf"),
        world_h:  float = float("inf"),
        zoom:     float = 1.0,
    ):
        self.screen_w = screen_w
        self.screen_h = screen_h
        self.world_w  = world_w
        self.world_h  = world_h

        # カメラ左上のワールド座標
        self.x = 0.0
        self.y = 0.0

        # ズーム（1.0=等倍, 2.0=2倍拡大, 0.5=縮小）
        self._zoom      = zoom
        self._zoom_min  = 0.1
        self._zoom_max  = 10.0

        # シェイク
        self._shake_amount  = 0.0   # 現在のシェイク量（px）
        self._shake_decay   = 8.0   # 毎秒どれだけ減衰するか
        self._shake_offset_x = 0.0
        self._shake_offset_y = 0.0

    # ── ズーム ───────────────────────────────────────────────────

    @property
    def zoom(self) -> float:
        return self._zoom

    @zoom.setter
    def zoom(self, value: float):
        self._zoom = max(self._zoom_min, min(self._zoom_max, value))

    def zoom_to(self, target: float, lerp: float = 1.0):
        """ズームをスムーズに変化させる。毎フレーム呼ぶ。"""
        target = max(self._zoom_min, min(self._zoom_max, target))
        self._zoom += (target - self._zoom) * lerp

    def set_zoom_limits(self, min_zoom: float, max_zoom: float):
        self._zoom_min = min_zoom
        self._zoom_max = max_zoom

    # ── 座標変換 ────────────────────────────────────────────────

    def to_screen(self, wx: float, wy: float) -> tuple[float, float]:
        """ワールド座標 → スクリーン座標（ズーム・シェイク適用）

        self.x/y はカメラ左上のワールド座標。
        sx = (wx - self.x) * zoom で正しくズーム座標変換される。
        """
        sx = (wx - self.x) * self._zoom + self._shake_offset_x
        sy = (wy - self.y) * self._zoom + self._shake_offset_y
        return sx, sy

    def to_world(self, sx: float, sy: float) -> tuple[float, float]:
        """スクリーン座標 → ワールド座標（ズーム考慮）"""
        wx = (sx - self._shake_offset_x) / self._zoom + self.x
        wy = (sy - self._shake_offset_y) / self._zoom + self.y
        return wx, wy

    def scale_to_screen(self, world_size: float) -> float:
        """ワールド上のサイズをスクリーンサイズに変換（ズーム適用）"""
        return world_size * self._zoom

    def is_visible(self, wx: float, wy: float, ww: float, wh: float) -> bool:
        """ワールド矩形がカメラ範囲内か（フラスタムカリング）"""
        margin = max(ww, wh) * (1.0 / self._zoom)
        sx, sy = self.to_screen(wx, wy)
        return (
            sx + ww * self._zoom > -margin and sx < self.screen_w + margin and
            sy + wh * self._zoom > -margin and sy < self.screen_h + margin
        )

    # ── 追従 ────────────────────────────────────────────────────

    def follow(
        self,
        wx: float, wy: float,
        obj_w: float = 0, obj_h: float = 0,
        lerp: float = 1.0,
    ):
        """対象ワールド座標にカメラを追従させる。

        Args:
            wx, wy  : 追従するワールド座標（対象左上）
            obj_w/h : 対象サイズ（中心に合わせる）
            lerp    : 追従の滑らかさ 0.0〜1.0（1.0=即時）
        """
        # ズームを考慮した表示領域サイズ
        view_w = self.screen_w / self._zoom
        view_h = self.screen_h / self._zoom

        target_x = wx + obj_w / 2 - view_w / 2
        target_y = wy + obj_h / 2 - view_h / 2

        # マップ端クランプ
        max_x = max(0.0, self.world_w - view_w)
        max_y = max(0.0, self.world_h - view_h)
        target_x = max(0.0, min(target_x, max_x))
        target_y = max(0.0, min(target_y, max_y))

        self.x += (target_x - self.x) * lerp
        self.y += (target_y - self.y) * lerp

    def set_pos(self, wx: float, wy: float):
        """カメラ位置を即時セット"""
        view_w = self.screen_w / self._zoom
        view_h = self.screen_h / self._zoom
        self.x = max(0.0, min(wx, self.world_w - view_w))
        self.y = max(0.0, min(wy, self.world_h - view_h))

    def center_on(self, wx: float, wy: float):
        """指定ワールド座標をカメラ中心に即時セット"""
        self.set_pos(wx - self.screen_w / self._zoom / 2,
                     wy - self.screen_h / self._zoom / 2)

    # ── シェイク ────────────────────────────────────────────────

    def shake(self, amount: float, decay: float = 8.0):
        """カメラシェイクを開始する。

        Args:
            amount : シェイク強度（px）
            decay  : 毎秒の減衰量（大きいほど早く止まる）
        """
        self._shake_amount = max(self._shake_amount, amount)
        self._shake_decay  = decay

    def update(self, dt: float):
        """毎フレーム呼ぶ。シェイクの減衰を処理する。"""
        if self._shake_amount > 0:
            self._shake_offset_x = random.uniform(
                -self._shake_amount, self._shake_amount
            )
            self._shake_offset_y = random.uniform(
                -self._shake_amount, self._shake_amount
            )
            self._shake_amount = max(
                0.0, self._shake_amount - self._shake_decay * dt
            )
        else:
            self._shake_offset_x = 0.0
            self._shake_offset_y = 0.0

    # ── シリアライズ ─────────────────────────────────────────────

    def to_dict(self) -> dict:
        return {
            "x": self.x, "y": self.y,
            "zoom": self._zoom,
            "screen_w": self.screen_w, "screen_h": self.screen_h,
            "world_w":  self.world_w,  "world_h":  self.world_h,
        }

    @classmethod
    def from_dict(cls, d: dict) -> "Camera":
        cam = cls(
            screen_w = d["screen_w"], screen_h = d["screen_h"],
            world_w  = d.get("world_w", float("inf")),
            world_h  = d.get("world_h", float("inf")),
            zoom     = d.get("zoom", 1.0),
        )
        cam.x = d.get("x", 0.0)
        cam.y = d.get("y", 0.0)
        return cam

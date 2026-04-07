# kagra/anim_state.py
# アニメーションステートマシン
#
# 使い方:
#   anim = AnimStateMachine(textures, tile_w=16, tile_h=16)
#   anim.add_state("idle",   frames=[0,1],       fps=4,  loop=True)
#   anim.add_state("walk",   frames=[4,5,6,7],   fps=10, loop=True)
#   anim.add_state("attack", frames=[8,9,10],    fps=12, loop=False, next="idle")
#   anim.add_state("hurt",   frames=[12],        fps=4,  loop=False, next="idle")
#
#   # 毎フレーム
#   anim.update(dt)
#   anim.draw(x, y, w, h, flip_x=facing_left)
#
#   # 状態遷移
#   anim.transition("attack")     # 現在のアニメが終わったら idle に戻る
#   if anim.finished("attack"):   # ワンショットが終わったか
#       ...

from __future__ import annotations
import json
import os
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class AnimState:
    name:     str
    frames:   list[int]    # タイルセットのタイルID列
    fps:      float = 8.0
    loop:     bool  = True
    next:     Optional[str] = None   # loop=False のとき終了後に遷移

    # ── シリアライズ ──────────────────────────────────────────

    def to_dict(self) -> dict:
        return {
            "name":   self.name,
            "frames": self.frames,
            "fps":    self.fps,
            "loop":   self.loop,
            "next":   self.next,
        }

    @classmethod
    def from_dict(cls, d: dict) -> "AnimState":
        return cls(
            name   = d["name"],
            frames = d["frames"],
            fps    = d.get("fps", 8.0),
            loop   = d.get("loop", True),
            next   = d.get("next"),
        )


class AnimStateMachine:
    """4方向アニメ対応のステートマシン。

    textures は TileSet か、{frame_id: texture_id} の dict。
    TileSet を使う場合は tile_w/tile_h を指定する。

    Example (TileSet モード)::
        ts   = kagra.assets.tileset("tiles/char", 16, 24)
        anim = AnimStateMachine.from_tileset(ts)
        anim.add_state("idle_front",  [0, 1],       fps=4)
        anim.add_state("walk_front",  [2, 3, 4, 5], fps=10)
        anim.add_state("attack_front",[6, 7, 8],    fps=14, loop=False, next="idle_front")
        anim.play("idle_front")

    Example (texture dict モード)::
        textures = kagra.assets.preload("char/front", "char/front_walk1", ...)
        anim = AnimStateMachine.from_textures(textures)
        anim.add_state("idle_front",  ["char/front"],               fps=2)
        anim.add_state("walk_front",  ["char/front_walk1", "char/front_walk2"], fps=8)
    """

    def __init__(self):
        self._states:  dict[str, AnimState] = {}
        self._current: Optional[str] = None
        self._frame_idx = 0
        self._timer     = 0.0
        self._finished  = False

        # 描画モード
        self._tileset   = None
        self._tex_dict: dict = {}  # {name: texture_id}

    # ── ファクトリ ────────────────────────────────────────────

    @classmethod
    def from_tileset(cls, tileset) -> "AnimStateMachine":
        m = cls()
        m._tileset = tileset
        return m

    @classmethod
    def from_textures(cls, tex_dict: dict) -> "AnimStateMachine":
        m = cls()
        m._tex_dict = tex_dict
        return m

    # ── 状態登録 ──────────────────────────────────────────────

    def add_state(
        self,
        name:   str,
        frames: list,
        fps:    float = 8.0,
        loop:   bool  = True,
        next:   Optional[str] = None,
    ) -> "AnimStateMachine":
        """ステートを追加。chainable。"""
        self._states[name] = AnimState(name, frames, fps, loop, next)
        return self

    def play(self, name: str, force: bool = False) -> bool:
        """ステートを切り替える。すでに同じなら何もしない（force=True で強制）。
        切り替えが起きた場合 True を返す。"""
        if not force and self._current == name:
            return False
        if name not in self._states:
            return False
        self._current   = name
        self._frame_idx = 0
        self._timer     = 0.0
        self._finished  = False
        return True

    def transition(self, name: str):
        """現在が指定ステートでなければ切り替える（play と同義、読みやすさのため）。"""
        self.play(name)

    # ── 毎フレーム ────────────────────────────────────────────

    def update(self, dt: float):
        if not self._current or self._current not in self._states:
            return
        state = self._states[self._current]

        if self._finished:
            return  # loop=False で終端に達した

        self._timer += dt
        frame_dur = 1.0 / max(0.1, state.fps)

        while self._timer >= frame_dur:
            self._timer -= frame_dur
            self._frame_idx += 1
            if self._frame_idx >= len(state.frames):
                if state.loop:
                    self._frame_idx = 0
                else:
                    self._frame_idx = len(state.frames) - 1
                    self._finished  = True
                    # 次ステートへ自動遷移
                    if state.next and state.next in self._states:
                        self.play(state.next)
                    return

    # ── クエリ ────────────────────────────────────────────────

    def finished(self, state_name: Optional[str] = None) -> bool:
        """ワンショットアニメが終わったか。state_name を指定するとそのステートのとき限定。"""
        if state_name and self._current != state_name:
            return False
        return self._finished

    @property
    def current_state(self) -> Optional[str]:
        return self._current

    @property
    def current_frame(self):
        """現在フレームのタイルID（または texture key）。"""
        if not self._current or self._current not in self._states:
            return None
        state = self._states[self._current]
        return state.frames[self._frame_idx]

    # ── 描画 ──────────────────────────────────────────────────

    def draw(
        self,
        x: float, y: float,
        w: float, h: float,
        flip_x: bool = False,
        alpha: float = 1.0,
    ):
        """現在フレームを描画する。"""
        import kagra
        frame = self.current_frame
        if frame is None:
            return

        if self._tileset:
            # TileSet モード: frame は tile_id (int)
            sx, sy, sw, sh = self._tileset.get_uv(frame)
            kagra.draw_texture(
                self._tileset.texture_id,
                x, y, w, h,
                sx, sy, sw, sh,
                alpha=alpha, flip_x=flip_x,
            )
        elif frame in self._tex_dict:
            # texture dict モード: frame は key (str)
            tex_id = self._tex_dict[frame]
            kagra.draw_texture(tex_id, x, y, w, h, alpha=alpha, flip_x=flip_x)

    # ── 便利メソッド ──────────────────────────────────────────

    def setup_4dir(
        self,
        prefix: str,
        idle_frames:   dict[str, list],
        walk_frames:   dict[str, list],
        attack_frames: dict[str, list] = None,
        hurt_frames:   dict[str, list] = None,
        idle_fps: float = 4.0,
        walk_fps: float = 10.0,
        atk_fps:  float = 14.0,
        hurt_fps: float = 8.0,
    ):
        """4方向 × idle/walk/attack/hurt を一括登録するショートカット。"""
        dirs = ["front", "back", "left", "right"]
        for d in dirs:
            self.add_state(f"{prefix}_idle_{d}",
                           idle_frames.get(d, [0]), fps=idle_fps)
            self.add_state(f"{prefix}_walk_{d}",
                           walk_frames.get(d, [0]), fps=walk_fps)
            if attack_frames:
                self.add_state(f"{prefix}_attack_{d}",
                               attack_frames.get(d, [0]), fps=atk_fps,
                               loop=False, next=f"{prefix}_idle_{d}")
            if hurt_frames:
                self.add_state(f"{prefix}_hurt_{d}",
                               hurt_frames.get(d, [0]), fps=hurt_fps,
                               loop=False, next=f"{prefix}_idle_{d}")

    def sync_with_mover(self, mover, prefix: str = "player"):
        """TopDownMovement の状態に合わせて自動的にステートを切り替える。"""
        d = mover.dir
        if mover.is_moving:
            target = f"{prefix}_walk_{d}"
        else:
            target = f"{prefix}_idle_{d}"
        cur = self._current or ""
        if "attack" in cur or "hurt" in cur:
            return
        self.play(target)

    # ── シリアライズ ──────────────────────────────────────────

    def to_dict(self) -> dict:
        """ステート定義のみを保存（tileset/tex_dict は含まない）。"""
        return {
            "version": 1,
            "states":  [s.to_dict() for s in self._states.values()],
            "current": self._current,
        }

    @classmethod
    def from_dict(cls, d: dict) -> "AnimStateMachine":
        """to_dict() の出力から復元する。tileset/tex_dict は別途設定すること。"""
        machine = cls()
        for sd in d.get("states", []):
            state = AnimState.from_dict(sd)
            machine._states[state.name] = state
        initial = d.get("current")
        if initial and initial in machine._states:
            machine._current = initial
        return machine

    def save(self, path: str):
        """JSON ファイルに保存する。"""
        os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(self.to_dict(), f, ensure_ascii=False, indent=2)

    @classmethod
    def load(cls, path: str) -> "AnimStateMachine":
        """JSON ファイルから読み込む。"""
        with open(path, "r", encoding="utf-8") as f:
            return cls.from_dict(json.load(f))

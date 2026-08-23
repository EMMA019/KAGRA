"""タッチ／仮想パッド抽象（モバイル入口）。

デスクトップではマウス＋キーにマップし、将来の Wasm/Android では
同じイベントをネイティブから流し込む。

Example::
    pad = VirtualPad()
    pad.set_stick(0.8, -0.2)   # 右やや上
    for name, down in pad.key_events():
        kagra.inject_key(name, down)

    # 画面タップ → マウス
    inject_pointer(400, 300, down=True)
"""
from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Iterator


class PointerPhase(str, Enum):
    BEGIN = "begin"
    MOVE = "move"
    END = "end"
    CANCEL = "cancel"


@dataclass
class PointerEvent:
    """正規化ポインタ（タッチ／マウス共通）。

    x,y はピクセル。id は指 ID（マウスは 0）。
    """

    id: int
    x: float
    y: float
    phase: PointerPhase
    pressure: float = 1.0


@dataclass
class VirtualPad:
    """左スティック → WASD に量子化。モバイル UI 用。"""

    deadzone: float = 0.25
    _lx: float = 0.0
    _ly: float = 0.0
    _held: set[str] = field(default_factory=set)

    def set_stick(self, x: float, y: float) -> None:
        """x,y ∈ [-1, 1]。y は上が負でも正でも可（screen Y 下向きなら + が下）。"""
        self._lx = max(-1.0, min(1.0, x))
        self._ly = max(-1.0, min(1.0, y))

    def stick(self) -> tuple[float, float]:
        """デッドゾーン後の ``(x, y)``。上が負。"""
        lx = self._lx if abs(self._lx) >= self.deadzone else 0.0
        ly = self._ly if abs(self._ly) >= self.deadzone else 0.0
        return lx, ly

    def clear(self) -> None:
        self._lx = 0.0
        self._ly = 0.0

    def desired_keys(self) -> set[str]:
        keys: set[str] = set()
        if abs(self._lx) >= self.deadzone:
            keys.add("D" if self._lx > 0 else "A")
        if abs(self._ly) >= self.deadzone:
            # 画面座標: +y 下 → S、ゲーム的には上が W なので -y を W に
            keys.add("S" if self._ly > 0 else "W")
        return keys

    def key_events(self) -> Iterator[tuple[str, bool]]:
        """前フレームとの差分を (key_name, down) で返す。"""
        want = self.desired_keys()
        for k in sorted(want - self._held):
            yield k, True
        for k in sorted(self._held - want):
            yield k, False
        self._held = want


def inject_pointer(x: float, y: float, *, down: bool | None = None, button: int = 0) -> None:
    """ポインタを既存の inject_mouse にマップ。"""
    import kagra

    kagra.inject_mouse(x=x, y=y, button=button, down=down)


def apply_pad(pad: VirtualPad) -> None:
    """VirtualPad の差分を inject_key に流す。"""
    import kagra

    for name, down in pad.key_events():
        kagra.inject_key(name, down=down)


def pointers_to_json(events: list[PointerEvent]) -> list[dict]:
    return [
        {
            "id": e.id,
            "x": e.x,
            "y": e.y,
            "phase": e.phase.value,
            "pressure": e.pressure,
        }
        for e in events
    ]

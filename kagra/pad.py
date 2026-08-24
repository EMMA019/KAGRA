"""Gamepad state. GPU 不要。``inject_pad``（テスト / スモーク）が実機より優先。

エンジンに ``poll_pad`` / ``pad_axis`` / ``pad_down`` があれば ``poll_pad()`` が読む。
実機 USB/XInput はウィンドウの EventLoop が gilrs で読む（Windows はループ 1 本）。
スティックは [-1, 1]。Y は下が正（``VirtualPad`` / ``Walk`` と同じ。上で前進）。
"""
from __future__ import annotations

from typing import Iterable

_BUTTON_ALIASES = {
    "a": "a", "south": "a", "cross": "a",
    "b": "b", "east": "b", "circle": "b",
    "x": "x", "west": "x", "square": "x",
    "y": "y", "north": "y", "triangle": "y",
    "lb": "lb", "l1": "lb", "leftshoulder": "lb",
    "rb": "rb", "r1": "rb", "rightshoulder": "rb",
    "lt": "lt", "l2": "lt", "lefttrigger": "lt",
    "rt": "rt", "r2": "rt", "righttrigger": "rt",
    "select": "select", "back": "select", "view": "select", "minus": "select",
    "start": "start", "menu": "start", "plus": "start", "options": "start",
    "up": "up", "dpadup": "up",
    "down": "down", "dpaddown": "down",
    "left": "left", "dpadleft": "left",
    "right": "right", "dpadright": "right",
    "ls": "ls", "l3": "ls", "leftstick": "ls",
    "rs": "rs", "r3": "rs", "rightstick": "rs",
}

BUTTON_NAMES = (
    "a", "b", "x", "y", "lb", "rb", "lt", "rt",
    "select", "start", "up", "down", "left", "right", "ls", "rs",
)


def normalize_button(name: str) -> str:
    return _BUTTON_ALIASES.get(str(name).strip().lower(), "")


def _clamp(v: float) -> float:
    return max(-1.0, min(1.0, float(v)))


class PadState:
    """1 台分。``inject_*`` が立っている間は実機より優先。"""

    def __init__(self) -> None:
        self.lx = 0.0
        self.ly = 0.0
        self.rx = 0.0
        self.ry = 0.0
        self.held: set[str] = set()
        self.pressed: set[str] = set()
        self.released: set[str] = set()
        self._inject = False

    def clear(self) -> None:
        self.lx = self.ly = self.rx = self.ry = 0.0
        self.held.clear()
        self.pressed.clear()
        self.released.clear()
        self._inject = False

    def inject(
        self,
        *,
        lx: float | None = None,
        ly: float | None = None,
        rx: float | None = None,
        ry: float | None = None,
        buttons: dict[str, bool] | None = None,
        clear: bool = False,
    ) -> None:
        if clear:
            self.clear()
            return
        self._inject = True
        if lx is not None:
            self.lx = _clamp(lx)
        if ly is not None:
            self.ly = _clamp(ly)
        if rx is not None:
            self.rx = _clamp(rx)
        if ry is not None:
            self.ry = _clamp(ry)
        if buttons:
            self.pressed.clear()
            self.released.clear()
            for raw, down in buttons.items():
                name = normalize_button(raw)
                if not name:
                    continue
                if down and name not in self.held:
                    self.pressed.add(name)
                    self.held.add(name)
                elif not down and name in self.held:
                    self.released.add(name)
                    self.held.discard(name)

    def apply_hardware(
        self,
        lx: float,
        ly: float,
        rx: float,
        ry: float,
        held: Iterable[str],
    ) -> None:
        if self._inject:
            return
        want = {normalize_button(n) for n in held}
        want.discard("")
        self.pressed = want - self.held
        self.released = self.held - want
        self.held = want
        self.lx, self.ly, self.rx, self.ry = _clamp(lx), _clamp(ly), _clamp(rx), _clamp(ry)

    def axis(self, side: str = "left") -> tuple[float, float]:
        key = str(side).strip().lower()
        if key in ("right", "r", "1", "look"):
            return self.rx, self.ry
        return self.lx, self.ly


_STATE = PadState()


def inject_pad(
    *,
    lx: float | None = None,
    ly: float | None = None,
    rx: float | None = None,
    ry: float | None = None,
    buttons: dict[str, bool] | None = None,
    clear: bool = False,
) -> None:
    """テスト / スモーク用。OS を通さない。次の ``axis`` / ``pad`` から見える。"""
    _STATE.inject(lx=lx, ly=ly, rx=rx, ry=ry, buttons=buttons, clear=clear)


def poll_pad() -> None:
    """実機を読む。``inject_pad`` 中は上書きしない。エンジンが無ければ何もしない。"""
    if _STATE._inject:
        return
    try:
        import kagra

        eng = kagra.get_engine()
        if eng is None:
            return
        poll = getattr(eng, "poll_pad", None)
        if poll is not None:
            poll()
        axis_fn = getattr(eng, "pad_axis", None)
        down_fn = getattr(eng, "pad_down", None)
        if axis_fn is None or down_fn is None:
            return
        (lx, ly) = axis_fn(0)
        (rx, ry) = axis_fn(1)
        held = [name for name in BUTTON_NAMES if down_fn(name)]
        _STATE.apply_hardware(lx, ly, rx, ry, held)
    except Exception:
        return


def axis(side: str = "left") -> tuple[float, float]:
    """アナログスティック ``(x, y)``。``left`` / ``right``。"""
    return _STATE.axis(side)


def pad(name: str) -> bool:
    """ボタンが押し続けられているか。``a`` / ``south`` / ``start`` など。"""
    key = normalize_button(name)
    return bool(key) and key in _STATE.held


def pad_pressed(name: str) -> bool:
    """この更新で押されたか。"""
    key = normalize_button(name)
    return bool(key) and key in _STATE.pressed


def pad_released(name: str) -> bool:
    """この更新で離されたか。"""
    key = normalize_button(name)
    return bool(key) and key in _STATE.released


def stick_move(lx: float, ly: float, *, deadzone: float = 0.2) -> tuple[float, float]:
    """左スティック → ``(forward, right)``。上が前進。0 軸 / デッドゾーンは離した扱い。"""
    import math

    lx = float(lx)
    ly = float(ly)
    if not math.isfinite(lx) or not math.isfinite(ly):
        return 0.0, 0.0
    if math.hypot(lx, ly) < float(deadzone):
        return 0.0, 0.0
    return -ly, lx

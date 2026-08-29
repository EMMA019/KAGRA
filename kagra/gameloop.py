"""Python ゲームマスター: Scene + run の shared(wgpu 30)版。

0.19 の ``kagra.run(start_scene)`` の形を ``kagra_shared`` の上で復活させる。
ゲームロジック（敵 AI・ターン・識別）は全部 Python で書く。Rust は世界の
tick（WorldPlay）と描画（render_world_doc）を担う。

使い方（エージェント向けの最小形）::

    import json
    from kagra.gameloop import Scene, run, draw_world

    class MyGame(Scene):
        def __init__(self):
            super().__init__()
            self.play = kagra.WorldPlay.from_json(open("world.json").read())

        def update(self, dt):
            # 入力を見て世界を進める（ゲームロジックはここ）
            if pressed("w"):
                self.play.set_input(0.0, 1.0, False, False, False)
            else:
                self.play.set_input(0.0, 0.0, False, False, False)
            self.play.tick(dt)
            self.world = json.loads(self.play.dump())

        def draw(self):
            # shared がこの dump を描画する
            self.canvas = draw_world(self.world, self.width, self.height)

    run(MyGame())

tkinter の窓（標準ライブラリのみ）。追加依存なし。ヘッドレス環境では
``run`` を呼ばず、``draw_world`` で PNG を得て保存する。
"""
from __future__ import annotations

import json
import struct
import zlib
from typing import Any

try:
    import kagra_shared as _ks
except ImportError:  # pragma: no cover
    _ks = None

__all__ = ["Scene", "run", "draw_world", "pressed", "was_pressed", "rgba_to_png"]


def rgba_to_png(rgba: bytes, width: int, height: int) -> bytes:
    """RGBA8 → PNG（標準ライブラリのみ。tkinter の PhotoImage が読める）。"""
    w, h = int(width), int(height)

    def chunk(typ: bytes, data: bytes) -> bytes:
        c = struct.pack(">I", len(data)) + typ + data
        return c + struct.pack(">I", zlib.crc32(typ + data) & 0xFFFFFFFF)

    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)  # 8-bit RGBA
    raw = b"".join(b"\x00" + rgba[y * w * 4 : (y + 1) * w * 4] for y in range(h))
    return (
        sig
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(raw, 6))
        + chunk(b"IEND", b"")
    )


def draw_world(world: dict[str, Any], width: int = 320, height: int = 180) -> bytes:
    """dump dict → shared wgpu 30 オフスクリーン描画 → PNG bytes。

    Scene.draw() 内で呼び、結果を run() の窓が表示する（または保存する）。
    """
    if _ks is None:
        raise ImportError("kagra_shared not installed: cd kagra-shared && maturin develop --release")
    rgba = _ks.render_world_doc(json.dumps(world), int(width), int(height))
    return rgba_to_png(rgba, int(width), int(height))


class Scene:
    """ゲーム 1 本。update(dt) で世界を進め、draw() で描画指示する。

    ``self.world`` は dump dict。``self.quit()`` でループを終了。
    """

    def __init__(self, world: dict[str, Any] | None = None):
        self.world: dict[str, Any] = world if world is not None else {}
        self.width = 320
        self.height = 180
        self.running = True
        self.clock = 0.0
        self._canvas_png: bytes | None = None

    def update(self, dt: float) -> None:
        """毎フレーム呼ばれる。ゲームロジックはここに書く。"""

    def draw(self) -> None:
        """毎フレーム呼ばれる。draw_world(self.world, w, h) で描画指示。"""

    def quit(self) -> None:
        self.running = False


# ── 入力（tkinter の KeyPress / KeyRelease から記録） ──────────────────────

_down: set[str] = set()        # 押されているキー
_just: set[str] = set()        # このフレーム押されたキー

_KEYMAP = {
    "w": "w", "a": "a", "s": "s", "d": "d",
    "space": "space", "j": "j", "z": "z", "f": "f",
    "escape": "escape", "return": "enter",
    "up": "up", "down": "down", "left": "left", "right": "right",
    "shift_l": "shift", "shift_r": "shift",
}


def _on_key_down(event) -> None:
    name = _KEYMAP.get(str(getattr(event, "keysym", "")).lower())
    if name:
        _down.add(name)
        _just.add(name)


def _on_key_up(event) -> None:
    name = _KEYMAP.get(str(getattr(event, "keysym", "")).lower())
    if name:
        _down.discard(name)


def pressed(name: str) -> bool:
    """今押されているか（ホールド）。update() 内で使う。"""
    return name.lower() in _down


def was_pressed(name: str) -> bool:
    """このフレーム押された瞬間か。"""
    return name.lower() in _just


def run(
    scene: Scene,
    *,
    width: int = 320,
    height: int = 180,
    title: str = "KAGRA (shared wgpu 30)",
    fps: float = 60.0,
) -> None:
    """tkinter 窓でゲームループを回す。ESC または scene.quit() で終了。

    update(dt) → draw() の順に毎フレーム呼ぶ。draw() が draw_world() で
    作った PNG を窓に表示する。ヘッドレス環境では動かない（CI では
    draw_world を直接使う）。
    """
    import tkinter as tk

    scene.width = width
    scene.height = height
    root = tk.Tk()
    root.title(title)
    label = tk.Label(root)
    label.pack()
    img = tk.PhotoImage(width=1, height=1)
    label.config(image=img)
    label.image = img

    root.bind("<KeyPress>", _on_key_down)
    root.bind("<KeyRelease>", _on_key_up)
    root.focus_set()

    frame_ms = max(1, int(1000.0 / fps))
    last = time_monotonic()

    def _tick() -> None:
        nonlocal last
        if not scene.running:
            root.destroy()
            return
        now = time_monotonic()
        dt = min((now - last) / 1000.0, 0.1)
        last = now
        scene.clock += dt
        _just.clear()
        scene.update(dt)
        if not scene.running:
            root.destroy()
            return
        scene.draw()
        if scene._canvas_png:
            img.configure(data=scene._canvas_png)
        root.after(frame_ms, _tick)

    root.after(frame_ms, _tick)
    root.mainloop()


def time_monotonic() -> float:
    import time

    return time.monotonic() * 1000.0

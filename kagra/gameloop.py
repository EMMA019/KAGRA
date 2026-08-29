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

__all__ = [
    "Scene",
    "run",
    "draw_world",
    "pressed",
    "was_pressed",
    "mouse_pos",
    "mouse_down",
    "mouse_clicked",
    "rgba_to_png",
]


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


def draw_world(
    world: dict[str, Any],
    width: int = 320,
    height: int = 180,
    hud: dict[str, Any] | None = None,
) -> bytes:
    """dump dict → shared wgpu 30 オフスクリーン描画 → PNG bytes。

    `hud`（省略可）: 世界の上に重ねる HUD。テキスト付きなので、文字の無い
    shared HUD を卒業できる。形式::

        {"quads": [{"x":..,"y":..,"w":..,"h":..,"color":[r,g,b,a]}],
         "texts": [{"text":"こんにちは","x":..,"y":..,"size":16,
                    "color":[r,g,b,a],"align":"left|center|right"}]}

    Scene.draw() 内で呼び、結果を run() の窓が表示する（または保存する）。
    """
    if _ks is None:
        raise ImportError(_missing_shared_message())
    hud_json = json.dumps(hud) if hud else None
    rgba = _ks.render_world_doc(
        json.dumps(world), int(width), int(height), hud_json
    )
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

# ── マウス（Motion / Button / ButtonRelease から記録） ────────────────────

_mouse: dict[str, Any] = {
    "x": 0,
    "y": 0,
    "buttons": set(),   # 押されているボタン（1=左, 2=中, 3=右）
    "just": set(),      # このフレーム押されたボタン
}


def _on_mouse_motion(event) -> None:
    _mouse["x"] = int(getattr(event, "x", 0))
    _mouse["y"] = int(getattr(event, "y", 0))


def _on_mouse_down(event) -> None:
    btn = int(getattr(event, "num", 1))
    if btn in (1, 2, 3):
        _mouse["buttons"].add(btn)
        _mouse["just"].add(btn)


def _on_mouse_up(event) -> None:
    btn = int(getattr(event, "num", 1))
    _mouse["buttons"].discard(btn)


def mouse_pos() -> tuple[int, int]:
    """窓内カーソル位置（左上原点、ピクセル）。"""
    return _mouse["x"], _mouse["y"]


def mouse_down(button: int = 1) -> bool:
    """いま押されているか（ホールド）。"""
    return button in _mouse["buttons"]


def mouse_clicked(button: int = 1) -> bool:
    """このフレーム押された瞬間か。"""
    return button in _mouse["just"]

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


def _missing_shared_message() -> str:
    return (
        "kagra_shared（shared wgpu 30 バインディング）が見つかりません。\n"
        "実行には .venv の Python を使ってください:\n"
        "    .venv\\Scripts\\python.exe examples\\bunny_garden_minimal.py\n"
        "または kagra_shared をビルドしてインストール:\n"
        "    cd kagra-shared && maturin develop --release"
    )


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

    if _ks is None:
        # 窓を開く前に止める: tkinter コールバック内の生トレースバックを避ける
        print(_missing_shared_message())
        return

    scene.width = width
    scene.height = height
    root = tk.Tk()
    root.title(title)
    label = tk.Label(root)
    label.pack()
    # 窓サイズを先に確定させる透明プレースホルダ。PhotoImage の
    # configure(data=) はサイズを変えない（白画面の原因）ので、
    # 毎フレーム新規 PhotoImage を作って差し替える。
    img = tk.PhotoImage(width=width, height=height)
    label.config(image=img)
    label.image = img

    root.bind_all("<KeyPress>", _on_key_down)
    root.bind_all("<KeyRelease>", _on_key_up)
    root.bind_all("<Motion>", _on_mouse_motion)
    root.bind_all("<Button-1>", _on_mouse_down)
    root.bind_all("<Button-2>", _on_mouse_down)
    root.bind_all("<Button-3>", _on_mouse_down)
    root.bind_all("<ButtonRelease-1>", _on_mouse_up)
    root.bind_all("<ButtonRelease-2>", _on_mouse_up)
    root.bind_all("<ButtonRelease-3>", _on_mouse_up)
    # Windows ではコンソールにフォーカスが残ることが多い。前面化 + 強制
    # フォーカスを数回リトライし、クリックでもフォーカスを奪い返す。
    root.bind_all(
        "<Button-1>",
        lambda _e: (root.focus_set(), root.focus_force()),
        add="+",
    )

    def _grab_focus() -> None:
        root.lift()
        root.focus_force()
        root.update_idletasks()

    root.update_idletasks()
    _grab_focus()
    root.after(120, _grab_focus)
    root.after(400, _grab_focus)
    # 最前面フラッシュ（0.6 秒後解除）で視認性も上げる
    try:
        root.attributes("-topmost", True)
        root.after(600, lambda: root.attributes("-topmost", False))
    except Exception:
        pass

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
        _mouse["just"].clear()
        scene.update(dt)
        if not scene.running:
            root.destroy()
            return
        scene.draw()
        if scene._canvas_png:
            img = tk.PhotoImage(data=scene._canvas_png)
            label.config(image=img)
            label.image = img  # 参照を保持しないと GC で消える
        root.after(frame_ms, _tick)

    root.after(frame_ms, _tick)
    root.mainloop()


def time_monotonic() -> float:
    import time

    return time.monotonic() * 1000.0

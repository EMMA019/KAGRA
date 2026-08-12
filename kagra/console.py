# kagra/console.py
"""
Phase 9b: インゲーム Python コンソール（Quake スタイル）

~ キーでゲーム実行中に Python REPL を開く。
変数の確認・変更・関数呼び出しが全部できる。

【使い方】
    import kagra
    from kagra.console import DevConsole

    class GameScene(kagra.Scene):
        def on_enter(self):
            self.console = DevConsole(self)   # self を渡すとシーンの変数にアクセスできる
            self.score = 0

        def update(self, dt):
            self.console.update(dt)   # これだけで ~ キー対応が有効になる

        def draw(self):
            kagra.cls(20, 20, 50)
            kagra.text(f"score={self.score}", 20, 20)
            self.console.draw()       # コンソールを最前面に描画

    # ゲーム実行中に ~ を押してコンソールで:
    #   scene.score = 999
    #   kagra.bgm("assets/test.ogg")
    #   import math; math.pi

【グローバルコンソール（シングルトン）】
    # どのシーンからでも使えるグローバルインスタンス
    from kagra.console import get_console
    console = get_console()
    console.update(dt)
    console.draw()
"""
from __future__ import annotations

import code
import sys
import traceback
from collections import deque
from typing import Any, Optional

# 画面に描画する最大行数
_MAX_LINES   = 20
# コマンド履歴の最大件数
_MAX_HISTORY = 100


class DevConsole:
    """インゲーム Python REPL コンソール。

    Args:
        scene:     現在の Scene インスタンス（``scene.xxx`` でアクセス可能）
        toggle_key: 開閉キー名（デフォルト: "BACKQUOTE"。なければ "F1"）
        height_ratio: 画面の何割をコンソールが占めるか（0.0〜1.0）
        font_size:  コンソールのフォントサイズ

    Example::
        console = DevConsole(self)
        console.update(dt)
        console.draw()
    """

    def __init__(
        self,
        scene: Any = None,
        toggle_key:   str   = "BACKQUOTE",
        height_ratio: float = 0.45,
        font_size:    int   = 16,
    ):
        self.scene        = scene
        self.toggle_key   = toggle_key
        self.height_ratio = height_ratio
        self.font_size    = font_size
        self.enabled      = False

        # 表示用ログ
        self._lines: deque[tuple[str, tuple]] = deque(maxlen=_MAX_LINES * 3)

        # 入力バッファ
        self._input:    str  = ""
        self._cursor:   int  = 0
        self._history:  deque[str] = deque(maxlen=_MAX_HISTORY)
        self._hist_idx: int  = -1

        # アニメーション
        self._open_t:   float = 0.0   # 0.0=閉 1.0=完全に開
        self._blink_t:  float = 0.0
        self._cursor_visible: bool = True

        # Python 実行環境
        self._locals: dict = self._build_locals()
        self._interp = code.InteractiveConsole(locals=self._locals)

        # 出力キャプチャ
        self._stdout_cap = _OutputCapture(self._append_output)
        self._stderr_cap = _OutputCapture(self._append_error)

        # 利用可能キーのフォールバック確認
        self._actual_key: Optional[str] = None

        self._append_log(
            "━━━ KAGRA Dev Console ━━━  ~ で開閉",
            (180, 200, 255)
        )
        self._append_log("scene / kagra / import が使えます", (150, 150, 180))

    def _build_locals(self) -> dict:
        """REPL の実行環境を構築する。"""
        import kagra
        env = {
            "kagra":  kagra,
            "scene":  self.scene,
            "engine": kagra.get_engine(),
            "__name__": "__console__",
        }
        # よく使うものを直接登録
        for name in ["fill", "text", "image", "load", "bgm", "se",
                     "pressed", "key", "mouse", "go", "push", "pop",
                     "emit", "on", "avatar"]:
            fn = getattr(kagra, name, None)
            if fn:
                env[name] = fn
        return env

    def update_scene_ref(self, scene: Any):
        """シーンが切り替わったら呼ぶ。"""
        self.scene = scene
        self._locals["scene"] = scene

    # ── 毎フレーム ────────────────────────────────────────────

    def update(self, dt: float):
        """毎フレーム呼ぶ。"""
        import kagra

        # キーの確認（初回のみ）
        if self._actual_key is None:
            self._actual_key = self._detect_key()

        # 開閉
        key_name = self._actual_key
        if key_name and self._is_pressed(key_name):
            self.enabled = not self.enabled
            if self.enabled:
                self._locals["scene"] = self.scene  # 最新シーンを更新

        # アニメーション
        target = 1.0 if self.enabled else 0.0
        self._open_t += (target - self._open_t) * min(1.0, dt * 12)
        if self._open_t < 0.01:
            return

        # カーソル点滅
        self._blink_t += dt
        if self._blink_t >= 0.5:
            self._blink_t = 0.0
            self._cursor_visible = not self._cursor_visible

        if not self.enabled:
            return

        # テキスト入力
        chars = kagra.get_typed_chars()
        for c in chars:
            if c == '\x08':   # Backspace
                if self._cursor > 0:
                    self._input = (self._input[:self._cursor - 1] +
                                   self._input[self._cursor:])
                    self._cursor -= 1
            elif c == '\r' or c == '\n':
                self._execute()
            elif ord(c) >= 32:
                self._input = (self._input[:self._cursor] + c +
                               self._input[self._cursor:])
                self._cursor += 1

        # 矢印キーで履歴
        if self._is_pressed("UP"):
            self._history_up()
        if self._is_pressed("DOWN"):
            self._history_down()
        if self._is_pressed("LEFT") and self._cursor > 0:
            self._cursor -= 1
        if self._is_pressed("RIGHT") and self._cursor < len(self._input):
            self._cursor += 1

    def draw(self):
        """コンソールを描画する。draw() の末尾で呼ぶ。"""
        if self._open_t < 0.01:
            return

        import kagra
        sw = kagra.screen_w()
        sh = kagra.screen_h()
        t  = self._open_t   # 0〜1 の開き具合

        # コンソール高さ
        con_h = int(sh * self.height_ratio * t)
        if con_h < 10:
            return

        fs = self.font_size
        line_h = fs + 4

        # 背景
        kagra.fill(0, 0, sw, con_h, (10, 12, 20, 220))
        kagra.fill(0, con_h, sw, 2, (80, 120, 200, 180))

        # ログ表示（下から積み上げ）
        visible_lines = max(1, (con_h - line_h - 12) // line_h)
        log_list = list(self._lines)[-visible_lines:]
        for i, (line, color) in enumerate(log_list):
            y = con_h - line_h * (len(log_list) - i) - line_h - 6
            if y > 0:
                kagra.text(line, 8, y, fs, color=color)

        # 入力行
        input_y = con_h - line_h - 4
        kagra.fill(0, input_y - 2, sw, line_h + 4, (20, 25, 40, 200))
        prompt = ">>> "
        prompt_w, _ = kagra.measure(prompt, fs)

        kagra.text(prompt, 8, input_y, fs, color=(100, 180, 255))

        # カーソル前後のテキスト
        before = self._input[:self._cursor]
        after  = self._input[self._cursor:]
        kagra.text(before, 8 + prompt_w, input_y, fs, color=(220, 220, 220))

        # カーソル
        if self._cursor_visible:
            before_w, _ = kagra.measure(before, fs)
            cur_x = 8 + prompt_w + before_w
            kagra.fill(cur_x, input_y, 2, fs, (180, 200, 255))

        if after:
            before_w2, _ = kagra.measure(before + " ", fs)
            kagra.text(after, 8 + prompt_w + before_w2 - (fs // 3),
                       input_y, fs, color=(220, 220, 220))

        # キー表示
        key_hint = f"[{self._actual_key or '~'}] 閉じる  [↑↓] 履歴"
        hw, _ = kagra.measure(key_hint, 12)
        kagra.text(key_hint, sw - hw - 8, 4, 12, color=(80, 100, 120))

    # ── コマンド実行 ──────────────────────────────────────────

    def _execute(self):
        """入力コマンドを実行する。"""
        cmd = self._input.strip()
        if not cmd:
            return

        # 履歴に追加
        if not self._history or self._history[-1] != cmd:
            self._history.append(cmd)
        self._hist_idx = -1

        self._append_log(f">>> {cmd}", (120, 160, 220))
        self._input  = ""
        self._cursor = 0

        # 出力をキャプチャして実行
        old_out = sys.stdout
        old_err = sys.stderr
        sys.stdout = self._stdout_cap
        sys.stderr = self._stderr_cap
        try:
            # 式として評価を試みる（結果を表示）
            try:
                result = eval(compile(cmd, "<console>", "eval"),
                              self._locals)
                if result is not None:
                    self._append_log(repr(result), (180, 220, 180))
            except SyntaxError:
                # 文として実行
                exec(compile(cmd, "<console>", "exec"),  # noqa
                     self._locals)
        except Exception:
            tb = traceback.format_exc()
            for line in tb.strip().split("\n"):
                self._append_error(line)
        finally:
            sys.stdout = old_out
            sys.stderr = old_err

    def run(self, cmd: str):
        """外部からコマンドを実行する（テスト・スクリプト用）。"""
        self._input  = cmd
        self._cursor = len(cmd)
        self._execute()

    # ── ログ ─────────────────────────────────────────────────

    def _append_log(self, text: str, color: tuple = (200, 200, 200)):
        for line in text.split("\n"):
            if line:
                self._lines.append((line, color))

    def _append_output(self, text: str):
        self._append_log(text, (200, 220, 200))

    def _append_error(self, text: str):
        self._append_log(text, (255, 120, 100))

    def log(self, text: str, color: tuple = (200, 200, 200)):
        """コンソールにログを追加する（外部から呼べる）。"""
        self._append_log(str(text), color)

    # ── 履歴 ─────────────────────────────────────────────────

    def _history_up(self):
        if not self._history:
            return
        if self._hist_idx == -1:
            self._hist_idx = len(self._history) - 1
        elif self._hist_idx > 0:
            self._hist_idx -= 1
        self._input  = self._history[self._hist_idx]
        self._cursor = len(self._input)

    def _history_down(self):
        if self._hist_idx == -1:
            return
        self._hist_idx += 1
        if self._hist_idx >= len(self._history):
            self._hist_idx = -1
            self._input    = ""
        else:
            self._input = self._history[self._hist_idx]
        self._cursor = len(self._input)

    # ── ユーティリティ ────────────────────────────────────────

    def _detect_key(self) -> Optional[str]:
        """利用可能なトグルキーを検出する。"""
        import kagra
        for name in [self.toggle_key, "BACKQUOTE", "F1", "F12"]:
            try:
                kagra._key_code(name)
                return name
            except (ValueError, AttributeError):
                pass
        return None

    def _is_pressed(self, name: str) -> bool:
        """キーが押されたか（例外を無視）。"""
        import kagra
        try:
            return kagra.pressed(name)
        except Exception:
            return False


# ── 出力キャプチャ ────────────────────────────────────────────

class _OutputCapture:
    def __init__(self, callback):
        self._cb  = callback
        self._buf = ""

    def write(self, text: str):
        self._buf += text
        while "\n" in self._buf:
            line, self._buf = self._buf.split("\n", 1)
            self._cb(line)

    def flush(self):
        if self._buf:
            self._cb(self._buf)
            self._buf = ""


# ── グローバルシングルトン ────────────────────────────────────

_global_console: Optional[DevConsole] = None

def get_console(scene: Any = None) -> DevConsole:
    """グローバルコンソールを取得（なければ生成）する。

    Example::
        from kagra.console import get_console

        def update(dt):
            get_console(self).update(dt)

        def draw():
            get_console().draw()
    """
    global _global_console
    if _global_console is None:
        _global_console = DevConsole(scene)
    elif scene is not None:
        _global_console.update_scene_ref(scene)
    return _global_console


# kagra/timeline_editor.py
# タイムラインエディタウィジェット（Tkinter）
# 新しい Timeline / Track / Keyframe API に対応

import tkinter as tk
from kagra.timeline import Timeline, Track, Keyframe


class TimelineEditor(tk.Canvas):
    """Timeline のキーフレームを GUI で編集するウィジェット。

    editor_app.py の下部パネルに埋め込んで使う。

    Example::
        tl = Timeline(name="Main")
        editor = TimelineEditor(parent_frame, tl)
        editor.pack(fill="x")

        # 毎フレーム
        editor.redraw()
    """

    TRACK_H  = 22    # トラック1行の高さ（px）
    HEADER_W = 120   # 左のラベル幅
    RULER_H  = 24    # ルーラー高さ

    def __init__(self, master, timeline: Timeline):
        super().__init__(master, height=140, bg="#111")
        self.timeline = timeline
        self.scale    = 80   # px / 秒

        self.bind("<Button-1>",        self._on_click)
        self.bind("<B1-Motion>",       self._on_drag)
        self.bind("<Button-3>",        self._on_right_click)
        self.bind("<MouseWheel>",      self._on_wheel)

        self._drag_key:   Keyframe | None = None
        self._drag_track: Track | None    = None

    # ── 描画 ─────────────────────────────────────────────────

    def redraw(self):
        self.delete("all")
        w = self.winfo_width()
        h = self.winfo_height()

        self._draw_ruler(w)
        self._draw_playhead(h)

        for i, track in enumerate(self.timeline.tracks):
            y = self.RULER_H + i * self.TRACK_H
            self._draw_track(track, i, y, w)

    def _draw_ruler(self, w: int):
        """秒数の目盛りを描画する。"""
        self.create_rectangle(0, 0, w, self.RULER_H, fill="#1e1e2e", outline="")
        self.create_line(0, self.RULER_H, w, self.RULER_H, fill="#444")

        step = max(1, int(60 / self.scale))   # 目盛り間隔（秒）
        t = 0
        while t * self.scale < w:
            x = self.HEADER_W + t * self.scale
            self.create_line(x, 14, x, self.RULER_H, fill="#666")
            self.create_text(x, 8, text=f"{t}s", fill="#888",
                             font=("Arial", 8))
            t += step

    def _draw_playhead(self, h: int):
        x = self.HEADER_W + self.timeline.time * self.scale
        self.create_line(x, 0, x, h, fill="#ff4444", width=2)
        # 三角マーカー
        self.create_polygon(x-5, 0, x+5, 0, x, 10,
                            fill="#ff4444", outline="")

    def _draw_track(self, track: Track, idx: int, y: int, w: int):
        # 背景
        bg = "#1a1a2a" if idx % 2 == 0 else "#161622"
        self.create_rectangle(0, y, w, y + self.TRACK_H, fill=bg, outline="")

        # ラベル
        label = f"{track.target_name}.{track.prop}" if track.target_name else track.prop
        self.create_text(4, y + self.TRACK_H // 2, text=label,
                         anchor="w", fill="#aaa", font=("Arial", 9))

        # キーフレーム
        for key in track.keys:
            kx = self.HEADER_W + key.time * self.scale
            ky = y + self.TRACK_H // 2
            self.create_oval(kx - 5, ky - 5, kx + 5, ky + 5,
                             fill="#44aaff", outline="#88ccff", width=1,
                             tags=("key", f"key_{id(key)}"))

    # ── インタラクション ──────────────────────────────────────

    def _time_from_x(self, x: int) -> float:
        return max(0.0, (x - self.HEADER_W) / self.scale)

    def _track_from_y(self, y: int) -> "Track | None":
        idx = (y - self.RULER_H) // self.TRACK_H
        if 0 <= idx < len(self.timeline.tracks):
            return self.timeline.tracks[idx]
        return None

    def _nearest_key(self, track: Track, t: float,
                     tolerance: float = 0.15) -> "Keyframe | None":
        for key in track.keys:
            if abs(key.time - t) < tolerance:
                return key
        return None

    def _on_click(self, event):
        t     = self._time_from_x(event.x)
        track = self._track_from_y(event.y)

        # ルーラークリック → スクラブ
        if event.y < self.RULER_H:
            self.timeline.seek(t)
            self.timeline.play()   # シーク後は再生再開
            self.redraw()
            return

        if track is None:
            return

        # 既存キーのドラッグ開始
        key = self._nearest_key(track, t)
        if key:
            self._drag_key   = key
            self._drag_track = track
            return

        # 新しいキーを追加（現在の評価値を使う）
        current_val = track.evaluate(t)
        if current_val is None:
            current_val = 0.0
        track.add_key(t, current_val)
        self.redraw()

    def _on_drag(self, event):
        t = self._time_from_x(event.x)

        # ルーラードラッグ → スクラブ
        if event.y < self.RULER_H:
            self.timeline.seek(t)
            self.timeline.play()   # スクラブ後は再生再開
            self.redraw()
            return

        # キードラッグ
        if self._drag_key is not None:
            self._drag_key.time = max(0.0, t)
            if self._drag_track:
                self._drag_track.keys.sort(key=lambda k: k.time)
            self.redraw()

    def _on_right_click(self, event):
        """右クリック → 最近傍のキーを削除。"""
        t     = self._time_from_x(event.x)
        track = self._track_from_y(event.y)
        if track:
            track.remove_key(t, tolerance=0.15)
            self.redraw()

    def _on_wheel(self, event):
        """マウスホイール → ズーム。"""
        factor = 1.1 if event.delta > 0 else 0.9
        self.scale = max(10, min(400, self.scale * factor))
        self.redraw()

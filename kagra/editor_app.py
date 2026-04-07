
# kagra/editor_app.py
# KAGRA 簡易エディタ（Tkinter ベース）
# 新しい Entity / Timeline / SceneGraph API に対応
#
# 起動:
#   python -m kagra.editor_app
#   または launcher.py から起動

import tkinter as tk
from tkinter import ttk, filedialog, messagebox
import json
import os

from kagra.entity import Entity, Transform
from kagra.scenegraph import SceneGraph
from kagra.scene_io import serialize_entity, save_scene
from kagra.scene_loader import load_scene
from kagra.prefab import Prefab
from kagra.timeline import Timeline, Track
from kagra.scene_runtime import SceneRuntime


class KagraEditorApp(tk.Tk):
    """KAGRA 簡易エディタ。

    機能:
    - Hierarchy パネル（Entity ツリー表示）
    - Timeline エディタ（キーフレーム編集）
    - シーン保存 / 読み込み
    - ランタイム起動 / 停止
    """

    def __init__(
        self,
        runtime_executable: str = "python",
        runtime_args_prefix: list = None,
        working_dir: str = None,
    ):
        super().__init__()
        self.title("KAGRA Editor")
        self.geometry("1200x800")

        self.scenegraph = SceneGraph()
        self.timeline   = Timeline(name="Editor")
        self.runtime    = SceneRuntime(
            runtime_executable=runtime_executable,
            runtime_args_prefix=runtime_args_prefix or [],
            working_dir=working_dir,
        )
        self._selected_entity: Entity | None = None
        self._current_file: str | None = None
        self._insp_vars: dict = {}

        self._build_menu()
        self._build_layout()
        self._build_demo()
        # 初回は何も選択していない状態で Inspector を初期化
        self.after(100, lambda: self._refresh_inspector())

        self.after(16, self._loop)

    # ── UI構築 ────────────────────────────────────────────────

    def _build_menu(self):
        menubar = tk.Menu(self)

        # File
        file_menu = tk.Menu(menubar, tearoff=0)
        file_menu.add_command(label="新規シーン",       command=self._new_scene)
        file_menu.add_command(label="シーン保存",       command=self._save_scene)
        file_menu.add_command(label="シーン保存（名前）", command=self._save_scene_as)
        file_menu.add_command(label="シーン読み込み",   command=self._load_scene)
        file_menu.add_separator()
        file_menu.add_command(label="終了",             command=self.quit)
        menubar.add_cascade(label="ファイル", menu=file_menu)

        # Runtime
        run_menu = tk.Menu(menubar, tearoff=0)
        run_menu.add_command(label="ランタイム起動", command=self._start_runtime)
        run_menu.add_command(label="ランタイム停止", command=self._stop_runtime)
        run_menu.add_command(label="再起動",         command=self._restart_runtime)
        menubar.add_cascade(label="ランタイム", menu=run_menu)

        # Edit
        edit_menu = tk.Menu(menubar, tearoff=0)
        edit_menu.add_command(label="Entity 追加", command=self._add_entity)
        edit_menu.add_command(label="Prefab 保存", command=self._save_prefab)
        menubar.add_cascade(label="編集", menu=edit_menu)

        self.config(menu=menubar)

    def _build_layout(self):
        # 上下分割
        paned_v = ttk.Panedwindow(self, orient="vertical")
        paned_v.pack(fill="both", expand=True)

        top_frame    = ttk.Frame(paned_v)
        bottom_frame = ttk.Frame(paned_v)
        paned_v.add(top_frame,    weight=3)
        paned_v.add(bottom_frame, weight=1)

        # 上部：左右分割
        paned_h = ttk.Panedwindow(top_frame, orient="horizontal")
        paned_h.pack(fill="both", expand=True)

        # 左：Hierarchy
        hier_frame = ttk.Frame(paned_h, width=220)
        paned_h.add(hier_frame, weight=1)
        self._build_hierarchy(hier_frame)

        # 中央：Scene View（Canvas）
        view_frame = ttk.Frame(paned_h)
        paned_h.add(view_frame, weight=4)
        self._build_scene_view(view_frame)

        # 右：Inspector
        insp_frame = ttk.Frame(paned_h, width=240)
        paned_h.add(insp_frame, weight=1)
        self._build_inspector(insp_frame)

        # 下部：Timeline
        self._build_timeline(bottom_frame)

        # ステータスバー
        self._status_var = tk.StringVar(value="準備完了")
        status = ttk.Label(self, textvariable=self._status_var,
                           relief="sunken", anchor="w")
        status.pack(fill="x", side="bottom")

    def _build_hierarchy(self, parent):
        ttk.Label(parent, text="Hierarchy").pack(anchor="w", padx=6, pady=(6,2))
        self._hier_tree = ttk.Treeview(parent, show="tree", selectmode="browse")
        self._hier_tree.pack(fill="both", expand=True, padx=6, pady=(0,6))
        self._hier_tree.bind("<<TreeviewSelect>>", self._on_select_entity)
        self._hier_items: dict[str, Entity] = {}

    def _build_scene_view(self, parent):
        ttk.Label(parent, text="Scene View").pack(anchor="w", padx=6, pady=(6,2))
        self._canvas = tk.Canvas(parent, bg="#1a1a2e", cursor="crosshair")
        self._canvas.pack(fill="both", expand=True, padx=6, pady=(0,6))

        # Scene View 操作パラメータ
        self._sv_scale  = 60.0   # px/unit
        self._sv_pan_x  = 0.0   # ワールド原点のオフセット（px）
        self._sv_pan_y  = 0.0
        self._sv_drag_start: tuple | None = None   # (mouse_x, mouse_y, wx, wy)
        self._sv_panning = False

        # イベントバインド
        self._canvas.bind("<Button-1>",        self._sv_on_click)
        self._canvas.bind("<B1-Motion>",       self._sv_on_drag)
        self._canvas.bind("<ButtonRelease-1>", self._sv_on_release)
        self._canvas.bind("<Button-2>",        self._sv_pan_start)
        self._canvas.bind("<B2-Motion>",       self._sv_pan_move)
        self._canvas.bind("<ButtonRelease-2>", self._sv_pan_end)
        self._canvas.bind("<MouseWheel>",      self._sv_zoom)

    def _build_inspector(self, parent):
        """Inspector パネル：選択した Entity のプロパティを表示・編集する。"""
        ttk.Label(parent, text="Inspector").pack(anchor="w", padx=6, pady=(6, 2))

        # スクロール可能なフレーム
        canvas = tk.Canvas(parent, highlightthickness=0)
        scrollbar = ttk.Scrollbar(parent, orient="vertical", command=canvas.yview)
        canvas.configure(yscrollcommand=scrollbar.set)
        scrollbar.pack(side="right", fill="y")
        canvas.pack(side="left", fill="both", expand=True)

        self._insp_inner = ttk.Frame(canvas)
        self._insp_window = canvas.create_window((0, 0), window=self._insp_inner, anchor="nw")

        def _on_frame_configure(e):
            canvas.configure(scrollregion=canvas.bbox("all"))
        def _on_canvas_configure(e):
            canvas.itemconfig(self._insp_window, width=e.width)

        self._insp_inner.bind("<Configure>", _on_frame_configure)
        canvas.bind("<Configure>", _on_canvas_configure)
        canvas.bind("<MouseWheel>", lambda e: canvas.yview_scroll(-1 if e.delta > 0 else 1, "units"))

        # 入力フィールド保持用
        self._insp_vars: dict[str, tk.Variable] = {}
        self._insp_canvas = canvas

        # 初期メッセージ
        ttk.Label(self._insp_inner, text="Entity を選択してください",
                  foreground="#888").pack(padx=8, pady=8)

    def _refresh_inspector(self):
        """選択中 Entity の内容を Inspector に反映する。"""
        # 既存ウィジェットをクリア
        for w in self._insp_inner.winfo_children():
            w.destroy()
        self._insp_vars.clear()

        e = self._selected_entity
        if e is None:
            ttk.Label(self._insp_inner, text="Entity を選択してください",
                      foreground="#888").pack(padx=8, pady=8)
            return

        # ── Entity 名 ──────────────────────────────────────
        self._insp_section("Entity")
        self._insp_field("name", "名前", e.name, str,
                         lambda v: setattr(e, "name", v) or self._refresh_hierarchy())

        # ── Transform ──────────────────────────────────────
        self._insp_section("Transform")
        t = e.transform
        self._insp_field("x",        "X",       t.x,        float, lambda v: setattr(t, "x", v))
        self._insp_field("y",        "Y",       t.y,        float, lambda v: setattr(t, "y", v))
        self._insp_field("rotation", "回転",    t.rotation, float, lambda v: setattr(t, "rotation", v))
        self._insp_field("scale_x",  "スケールX", t.scale_x, float, lambda v: setattr(t, "scale_x", v))
        self._insp_field("scale_y",  "スケールY", t.scale_y, float, lambda v: setattr(t, "scale_y", v))

        # ── Components ─────────────────────────────────────
        for comp in e.components:
            cname = type(comp).__name__
            if cname == "Transform":
                continue   # Transform は上で表示済み
            self._insp_section(cname)
            for attr, val in vars(comp).items():
                if attr.startswith("_") or attr == "entity":
                    continue
                if isinstance(val, (int, float)):
                    self._insp_field(
                        f"{cname}.{attr}", attr, val, type(val),
                        lambda v, c=comp, a=attr: setattr(c, a, v)
                    )
                elif isinstance(val, str):
                    self._insp_field(
                        f"{cname}.{attr}", attr, val, str,
                        lambda v, c=comp, a=attr: setattr(c, a, v)
                    )
                elif isinstance(val, bool):
                    self._insp_bool(f"{cname}.{attr}", attr, val,
                                    lambda v, c=comp, a=attr: setattr(c, a, v))
                else:
                    # 編集不可の値はラベル表示
                    row = ttk.Frame(self._insp_inner)
                    row.pack(fill="x", padx=8, pady=1)
                    ttk.Label(row, text=attr, width=12, foreground="#888",
                              font=("Arial", 9)).pack(side="left")
                    ttk.Label(row, text=str(val)[:30], foreground="#aaa",
                              font=("Arial", 9)).pack(side="left", padx=4)

    def _insp_section(self, title: str):
        """セクションヘッダーを追加する。"""
        f = ttk.Frame(self._insp_inner)
        f.pack(fill="x", pady=(6, 2))
        ttk.Separator(f, orient="horizontal").pack(fill="x", padx=4)
        ttk.Label(f, text=title, font=("Arial", 9, "bold"),
                  foreground="#ccc").pack(anchor="w", padx=8)

    def _insp_field(self, key: str, label: str, value, vtype, on_change):
        """数値・文字列の編集フィールドを追加する。"""
        row = ttk.Frame(self._insp_inner)
        row.pack(fill="x", padx=8, pady=2)

        ttk.Label(row, text=label, width=10, anchor="w",
                  font=("Arial", 9)).pack(side="left")

        var = tk.StringVar(value=str(round(value, 4) if isinstance(value, float) else value))
        self._insp_vars[key] = var

        entry = ttk.Entry(row, textvariable=var, width=14, font=("Arial", 9))
        entry.pack(side="left", fill="x", expand=True)

        def _apply(event=None):
            try:
                new_val = vtype(var.get())
                on_change(new_val)
                entry.configure(style="")
                # ステータスバー更新
                self._status_var.set(f"{label} = {new_val}")
            except ValueError:
                entry.configure(style="")  # 不正入力は無視

        entry.bind("<Return>",    _apply)
        entry.bind("<FocusOut>",  _apply)

    def _insp_bool(self, key: str, label: str, value: bool, on_change):
        """Bool の編集フィールド（チェックボックス）を追加する。"""
        row = ttk.Frame(self._insp_inner)
        row.pack(fill="x", padx=8, pady=2)

        var = tk.BooleanVar(value=value)
        self._insp_vars[key] = var

        ttk.Label(row, text=label, width=10, anchor="w",
                  font=("Arial", 9)).pack(side="left")
        chk = ttk.Checkbutton(row, variable=var,
                               command=lambda: on_change(var.get()))
        chk.pack(side="left")

    def _build_timeline(self, parent):
        ttk.Label(parent, text="Timeline").pack(anchor="w", padx=6, pady=(4,2))
        self._tl_canvas = tk.Canvas(parent, height=100, bg="#111")
        self._tl_canvas.pack(fill="x", padx=6, pady=(0,4))
        self._tl_canvas.bind("<Button-1>", self._tl_click)
        self._tl_canvas.bind("<B1-Motion>", self._tl_scrub)
        self._tl_scale = 80  # px/秒

    # ── デモシーン ────────────────────────────────────────────

    def _build_demo(self):
        """起動時のデモ用シーンを構築する。"""
        root = Entity("Root")
        player = Entity("Player")
        player.transform.set_parent(root.transform)
        player.transform.x = 2.0
        player.transform.y = 1.0

        enemy = Entity("Enemy")
        enemy.transform.set_parent(root.transform)
        enemy.transform.x = -3.0

        self.scenegraph.add(root)
        self._refresh_hierarchy()

        # Timeline デモ（Player の x を補間）
        t = Track(target=player, prop="x", target_name="Player")
        t.add_key(0.0, 2.0)
        t.add_key(2.0, 5.0)
        t.add_key(4.0, 2.0)
        self.timeline.add_track(t)

    # ── Hierarchy ────────────────────────────────────────────

    def _refresh_hierarchy(self):
        self._hier_tree.delete(*self._hier_tree.get_children())
        self._hier_items.clear()

        def add_node(entity: Entity, parent_id: str = ""):
            uid = id(entity)
            node_id = str(uid)
            self._hier_items[node_id] = entity
            self._hier_tree.insert(parent_id, "end", iid=node_id,
                                   text=entity.name)
            for tf_child in entity.transform.children:
                if hasattr(tf_child, "entity") and tf_child.entity:
                    add_node(tf_child.entity, node_id)

        for root in self.scenegraph.roots:
            add_node(root)

    def _on_select_entity(self, _event=None):
        sel = self._hier_tree.selection()
        if not sel:
            return
        entity = self._hier_items.get(sel[0])
        self._selected_entity = entity
        if entity:
            self._status_var.set(
                f"選択: {entity.name}  "
                f"x={entity.transform.x:.2f}  y={entity.transform.y:.2f}"
            )
        try:
            self._refresh_inspector()
        except Exception as e:
            import traceback
            traceback.print_exc()

    def _add_entity(self):
        name = f"Entity_{len(self._hier_items)+1}"
        entity = Entity(name)
        # 選択中の Entity の子として追加。なければシーンルート
        if self._selected_entity:
            entity.transform.set_parent(self._selected_entity.transform)
        else:
            self.scenegraph.add(entity)
        self._refresh_hierarchy()

    # ── Scene View ───────────────────────────────────────────

    # ── Scene View 座標変換 ──────────────────────────────────

    def _world_to_sv(self, wx: float, wy: float) -> tuple:
        """ワールド座標 → Scene View スクリーン座標。"""
        cw = self._canvas.winfo_width()
        ch = self._canvas.winfo_height()
        sx = cw / 2 + (wx + self._sv_pan_x) * self._sv_scale
        sy = ch / 2 - (wy + self._sv_pan_y) * self._sv_scale
        return sx, sy

    def _sv_to_world(self, sx: float, sy: float) -> tuple:
        """Scene View スクリーン座標 → ワールド座標。"""
        cw = self._canvas.winfo_width()
        ch = self._canvas.winfo_height()
        wx = (sx - cw / 2) / self._sv_scale - self._sv_pan_x
        wy = -((sy - ch / 2) / self._sv_scale) - self._sv_pan_y
        return wx, wy

    def _entity_at(self, sx: float, sy: float, radius: float = 10.0):
        """スクリーン座標に最も近い Entity を返す。"""
        best, best_d = None, radius
        for entity in self.scenegraph.traverse():
            ex, ey = self._world_to_sv(entity.transform.world_x, entity.transform.world_y)
            d = ((sx - ex) ** 2 + (sy - ey) ** 2) ** 0.5
            if d < best_d:
                best, best_d = entity, d
        return best

    # ── Scene View 描画 ───────────────────────────────────────

    def _redraw_scene(self):
        self._canvas.delete("all")

        # グリッド
        self._sv_draw_grid()

        for entity in self.scenegraph.traverse():
            wx, wy = entity.transform.world_x, entity.transform.world_y
            sx, sy = self._world_to_sv(wx, wy)
            is_sel = (entity is self._selected_entity)

            # Entity 本体
            color = "#5599ff" if is_sel else "#aaccff"
            r = 9 if is_sel else 7
            self._canvas.create_rectangle(sx-r, sy-r, sx+r, sy+r,
                                          fill=color, outline="white",
                                          width=2 if is_sel else 1)
            self._canvas.create_text(sx, sy - r - 6, text=entity.name,
                                     fill="white", font=("Arial", 9))

            # Gizmo（選択中のみ）
            if is_sel:
                self._sv_draw_gizmo(sx, sy)

    def _sv_draw_grid(self):
        """薄いグリッドを描画する。"""
        c  = self._canvas
        cw = c.winfo_width()
        ch = c.winfo_height()
        if cw <= 1 or ch <= 1:
            return
        s  = self._sv_scale

        # 原点
        ox, oy = self._world_to_sv(0, 0)
        c.create_line(ox, 0, ox, ch, fill="#333", width=1)
        c.create_line(0, oy, cw, oy, fill="#333", width=1)

        # グリッド線（1unit間隔）
        import math
        x0_w = math.floor(-cw / 2 / s - self._sv_pan_x)
        x1_w = math.ceil(cw  / 2 / s - self._sv_pan_x)
        y0_w = math.floor(-ch / 2 / s - self._sv_pan_y)
        y1_w = math.ceil(ch  / 2 / s - self._sv_pan_y)

        for gx in range(x0_w, x1_w + 1):
            px, _ = self._world_to_sv(gx, 0)
            c.create_line(px, 0, px, ch, fill="#222", dash=(2, 6))
        for gy in range(y0_w, y1_w + 1):
            _, py = self._world_to_sv(0, gy)
            c.create_line(0, py, cw, py, fill="#222", dash=(2, 6))

    def _sv_draw_gizmo(self, sx: float, sy: float):
        """選択中 Entity の移動ハンドルを描画する。"""
        c = self._canvas
        arm = 28   # 矢印の長さ

        # X 軸（赤）
        c.create_line(sx, sy, sx + arm, sy, fill="#ff4444", width=3)
        c.create_polygon(sx+arm, sy-5, sx+arm+10, sy, sx+arm, sy+5,
                         fill="#ff4444", outline="")
        c.create_text(sx + arm + 14, sy, text="X",
                      fill="#ff4444", font=("Arial", 9, "bold"))

        # Y 軸（緑）
        c.create_line(sx, sy, sx, sy - arm, fill="#44ff44", width=3)
        c.create_polygon(sx-5, sy-arm, sx, sy-arm-10, sx+5, sy-arm,
                         fill="#44ff44", outline="")
        c.create_text(sx, sy - arm - 14, text="Y",
                      fill="#44ff44", font=("Arial", 9, "bold"))

        # 中心（白）
        c.create_oval(sx-5, sy-5, sx+5, sy+5,
                      fill="white", outline="#aaa")

    # ── Scene View インタラクション ──────────────────────────────

    def _sv_on_click(self, event):
        """クリック：Entity 選択 または Gizmo ドラッグ開始。"""
        sx, sy = event.x, event.y
        entity = self._entity_at(sx, sy)

        if entity:
            # Entity を選択
            self._selected_entity = entity
            # Hierarchy の選択も連動
            for node_id, e in self._hier_items.items():
                if e is entity:
                    self._hier_tree.selection_set(node_id)
                    self._hier_tree.see(node_id)
                    break
            try:
                self._refresh_inspector()
            except Exception:
                pass

            # ドラッグ開始位置を記録
            wx, wy = self._sv_to_world(sx, sy)
            self._sv_drag_start = (sx, sy,
                                   entity.transform.x,
                                   entity.transform.y)
        else:
            self._sv_drag_start = None

    def _sv_on_drag(self, event):
        """ドラッグ：選択中 Entity を移動する。"""
        if self._sv_drag_start is None or self._selected_entity is None:
            return

        start_sx, start_sy, orig_x, orig_y = self._sv_drag_start
        dx = (event.x - start_sx) / self._sv_scale
        dy = -(event.y - start_sy) / self._sv_scale

        t = self._selected_entity.transform
        t.x = round(orig_x + dx, 3)
        t.y = round(orig_y + dy, 3)

        self._status_var.set(
            f"{self._selected_entity.name}  "
            f"x={t.x:.3f}  y={t.y:.3f}"
        )

    def _sv_on_release(self, event):
        self._sv_drag_start = None

    def _sv_pan_start(self, event):
        """中ボタン押下：パン開始。"""
        self._sv_panning = True
        self._sv_pan_mouse = (event.x, event.y,
                              self._sv_pan_x, self._sv_pan_y)

    def _sv_pan_move(self, event):
        """中ボタンドラッグ：パン。"""
        if not self._sv_panning:
            return
        mx, my, px, py = self._sv_pan_mouse
        self._sv_pan_x = px + (event.x - mx) / self._sv_scale
        self._sv_pan_y = py - (event.y - my) / self._sv_scale

    def _sv_pan_end(self, event):
        self._sv_panning = False

    def _sv_zoom(self, event):
        """マウスホイール：ズーム。"""
        factor = 1.1 if event.delta > 0 else 0.9
        self._sv_scale = max(10, min(300, self._sv_scale * factor))

    # ── Timeline ─────────────────────────────────────────────

    def _redraw_timeline(self):
        c = self._tl_canvas
        c.delete("all")
        w = c.winfo_width()
        h = c.winfo_height()

        # 目盛り線
        c.create_line(0, 20, w, 20, fill="#555")

        # キーフレームを描画
        for i, track in enumerate(self.timeline.tracks):
            y = 40 + i * 18
            label = f"{track.target_name}.{track.prop}"
            c.create_text(4, y, text=label, anchor="w",
                          fill="#aaa", font=("Arial", 9))
            for key in track.keys:
                x = key.time * self._tl_scale
                c.create_oval(x-4, y-4, x+4, y+4, fill="#55ccff", outline="")

        # プレイヘッド
        ph_x = self.timeline.time * self._tl_scale
        c.create_line(ph_x, 0, ph_x, h, fill="#ff5555", width=2)

    def _tl_click(self, event):
        t = event.x / self._tl_scale
        self.timeline.seek(t)
        self.timeline.play()

    def _tl_scrub(self, event):
        t = max(0.0, event.x / self._tl_scale)
        self.timeline.seek(t)
        self.timeline.play()

    # ── ファイル操作 ──────────────────────────────────────────

    def _new_scene(self):
        self.scenegraph = SceneGraph()
        self.timeline   = Timeline(name="Editor")
        self._selected_entity = None
        self._current_file    = None
        self._refresh_hierarchy()
        self._status_var.set("新規シーン")

    def _save_scene(self):
        if self._current_file:
            save_scene(self.scenegraph, self._current_file)
            self._status_var.set(f"保存: {self._current_file}")
        else:
            self._save_scene_as()

    def _save_scene_as(self):
        path = filedialog.asksaveasfilename(
            defaultextension=".json",
            filetypes=[("KAGRA Scene", "*.json"), ("All", "*.*")],
        )
        if path:
            self._current_file = path
            save_scene(self.scenegraph, path)
            self._status_var.set(f"保存: {path}")

    def _load_scene(self):
        path = filedialog.askopenfilename(
            filetypes=[("KAGRA Scene", "*.json"), ("All", "*.*")]
        )
        if path:
            self.scenegraph = SceneGraph()
            load_scene(self.scenegraph, path, {})
            self._current_file = path
            self._refresh_hierarchy()
            self._status_var.set(f"読み込み: {path}")

    def _save_prefab(self):
        if not self._selected_entity:
            messagebox.showwarning("Prefab", "保存する Entity を選択してください。")
            return
        path = filedialog.asksaveasfilename(
            defaultextension=".prefab.json",
            filetypes=[("KAGRA Prefab", "*.prefab.json"), ("All", "*.*")],
        )
        if path:
            Prefab.from_entity(self._selected_entity).save(path)
            self._status_var.set(f"Prefab 保存: {path}")

    # ── ランタイム ────────────────────────────────────────────

    def _start_runtime(self):
        if self._current_file:
            self.runtime.start(self._current_file)
            self._status_var.set(self.runtime.status_text())
        else:
            messagebox.showwarning("ランタイム",
                                   "先にシーンを保存してください。")

    def _stop_runtime(self):
        if not self.runtime.is_running:
            self._status_var.set("ランタイムは起動していません")
            return
        self.runtime.stop()
        self._status_var.set(self.runtime.status_text())

    def _restart_runtime(self):
        if not self.runtime.current_scene:
            messagebox.showwarning("ランタイム", "先にシーンを保存してください。")
            return
        self.runtime.restart()
        self._status_var.set(self.runtime.status_text())

    # ── メインループ ──────────────────────────────────────────

    def _loop(self):
        self.timeline.update(0.016)
        self._redraw_scene()
        self._redraw_timeline()
        self._sync_inspector()
        self.after(16, self._loop)

    def _sync_inspector(self):
        """Timeline 再生中に Transform の値をリアルタイムで Inspector に反映する。"""
        e = self._selected_entity
        if e is None:
            return
        t = e.transform
        pairs = [
            ("x",        t.x),
            ("y",        t.y),
            ("rotation", t.rotation),
            ("scale_x",  t.scale_x),
            ("scale_y",  t.scale_y),
        ]
        for key, val in pairs:
            var = self._insp_vars.get(key)
            if var:
                # フォーカスが当たっているフィールドは上書きしない
                focused = self.focus_get()
                if focused and hasattr(focused, "cget"):
                    try:
                        if focused.cget("textvariable") == str(var):
                            continue
                    except Exception:
                        pass
                var.set(str(round(val, 4)))


if __name__ == "__main__":
    app = KagraEditorApp()
    app.mainloop()

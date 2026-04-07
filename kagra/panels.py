from __future__ import annotations
import tkinter as tk
from tkinter import ttk
from typing import Callable


class AssetBrowserPanel(ttk.Frame):
    def __init__(self, master, on_select: Callable | None = None):
        super().__init__(master)
        self.on_select = on_select
        self.kind_var = tk.StringVar(value="all")
        self.search_var = tk.StringVar(value="")

        top = ttk.Frame(self)
        top.pack(fill="x", padx=6, pady=6)

        ttk.Label(top, text="Type").pack(side="left")
        ttk.Combobox(
            top,
            textvariable=self.kind_var,
            values=["all", "image", "font", "audio", "map", "rig", "scene", "prefab", "json", "file"],
            width=12,
            state="readonly",
        ).pack(side="left", padx=4)

        ttk.Label(top, text="Search").pack(side="left", padx=(8, 0))
        ent = ttk.Entry(top, textvariable=self.search_var)
        ent.pack(side="left", fill="x", expand=True, padx=4)

        self.tree = ttk.Treeview(self, columns=("kind", "path"), show="headings", height=18)
        self.tree.heading("kind", text="Kind")
        self.tree.heading("path", text="Path")
        self.tree.column("kind", width=90, anchor="w")
        self.tree.column("path", width=420, anchor="w")
        self.tree.pack(fill="both", expand=True, padx=6, pady=(0, 6))

        self.tree.bind("<<TreeviewSelect>>", self._handle_select)
        self.kind_var.trace_add("write", lambda *_: self._apply_filter())
        self.search_var.trace_add("write", lambda *_: self._apply_filter())

        self._all_assets = []

    def set_assets(self, assets):
        self._all_assets = list(assets)
        self._apply_filter()

    def _apply_filter(self):
        kind = self.kind_var.get().strip().lower()
        text = self.search_var.get().strip().lower()

        for iid in self.tree.get_children():
            self.tree.delete(iid)

        for a in self._all_assets:
            if kind != "all" and a.kind != kind:
                continue
            hay = f"{a.key} {a.path} {a.kind}".lower()
            if text and text not in hay:
                continue
            self.tree.insert("", "end", iid=a.key, values=(a.kind, a.path))

    def _handle_select(self, _event=None):
        sel = self.tree.selection()
        if not sel:
            return
        key = sel[0]
        found = next((a for a in self._all_assets if a.key == key), None)
        if found and self.on_select:
            self.on_select(found)


class SimpleListPanel(ttk.Frame):
    def __init__(self, master, title: str, on_select: Callable | None = None):
        super().__init__(master)
        self.on_select = on_select
        ttk.Label(self, text=title).pack(anchor="w", padx=6, pady=(6, 2))
        self.listbox = tk.Listbox(self, height=10)
        self.listbox.pack(fill="both", expand=True, padx=6, pady=(0, 6))
        self.listbox.bind("<<ListboxSelect>>", self._handle_select)
        self._items = []

    def set_items(self, items):
        self._items = list(items)
        self.listbox.delete(0, "end")
        for item in self._items:
            self.listbox.insert("end", item)

    def _handle_select(self, _event=None):
        idxs = self.listbox.curselection()
        if not idxs:
            return
        item = self._items[idxs[0]]
        if self.on_select:
            self.on_select(item)


class InspectorPanel(ttk.Frame):
    def __init__(self, master):
        super().__init__(master)
        ttk.Label(self, text="Inspector").pack(anchor="w", padx=6, pady=(6, 2))
        self.text = tk.Text(self, height=24, wrap="word")
        self.text.pack(fill="both", expand=True, padx=6, pady=(0, 6))
        self.text.configure(state="disabled")

    def show_object(self, title: str, data: dict):
        self.text.configure(state="normal")
        self.text.delete("1.0", "end")
        self.text.insert("end", f"{title}\n")
        self.text.insert("end", "=" * len(title) + "\n\n")
        for k, v in data.items():
            self.text.insert("end", f"{k}: {v}\n")
        self.text.configure(state="disabled")

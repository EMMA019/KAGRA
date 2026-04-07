from __future__ import annotations
import tkinter as tk
from tkinter import ttk
from typing import Callable


class HierarchyPanel(ttk.Frame):
    """
    SceneGraph / Entity ツリーを TreeView に表示するパネル。

    想定 Entity:
        entity.name
        entity.children -> list[entity]
        entity.components -> dict or list or any
    """

    def __init__(self, master, on_select: Callable | None = None):
        super().__init__(master)
        self.on_select = on_select
        self.entities = {}

        ttk.Label(self, text="Hierarchy").pack(anchor="w", padx=6, pady=(6, 2))

        self.tree = ttk.Treeview(self, show="tree")
        self.tree.pack(fill="both", expand=True, padx=6, pady=(0, 6))

        self.tree.bind("<<TreeviewSelect>>", self._select)

    def clear(self):
        self.tree.delete(*self.tree.get_children())
        self.entities.clear()

    def set_scene_root(self, root_entity):
        self.clear()
        if root_entity is None:
            return

        def add_entity(entity, parent=""):
            base_id = getattr(entity, "name", "Entity")
            node_id = self._unique_id(base_id)

            self.entities[node_id] = entity
            self.tree.insert(parent, "end", iid=node_id, text=base_id)

            children = getattr(entity, "children", []) or []
            for child in children:
                add_entity(child, node_id)

        add_entity(root_entity)

    def _unique_id(self, base: str) -> str:
        if base not in self.entities:
            return base
        i = 2
        while f"{base}#{i}" in self.entities:
            i += 1
        return f"{base}#{i}"

    def _select(self, _event=None):
        sel = self.tree.selection()
        if not sel:
            return

        node_id = sel[0]
        entity = self.entities.get(node_id)

        if self.on_select and entity is not None:
            self.on_select(entity)

"""Minimal 3D world: floor + static boxes + a walking capsule.

Collision is GPU-free (``Physics3D``). Mesh retain happens in ``bake()``
once the renderer exists — tests can skip that and still walk the room.
"""
from __future__ import annotations

from typing import Optional

from kagra.gamekit import box_mesh, quad_y_mesh
from kagra.physics3d import Physics3D, RigidBody3D


class World3D:
    """床と箱のある部屋。カメラ追従は ``Camera3D.follow``。

    Example::
        world = World3D(half=6.0)
        world.add_floor()
        world.add_box(2, 0, -1, 1.2, 1.0, 1.2)
        player = world.add_player(0, 3)
        world.bake(floor_tex, box_tex)   # on_ready
        world.move_player(vx, vz)
        world.update(dt)
        world.draw()
    """

    def __init__(self, *, floor_y: float = 0.0, half: float = 6.0, gravity: float = 9.8):
        self.physics = Physics3D(gravity)
        self.physics.set_ground_y(floor_y)
        self.floor_y = float(floor_y)
        self.half = float(half)
        self.boxes: list[RigidBody3D] = []
        self.player: Optional[RigidBody3D] = None
        self.mesh_ids: list[int] = []
        self._pending: list[tuple] = []
        self.box_mesh_id: int = 0
        self.box_xforms: list[list[float]] = []

    def add_floor(self, size: float | None = None):
        """Y = ``floor_y`` の正方形床を予約する。半辺は ``size`` または ``half``。"""
        self._pending.append(("floor", float(self.half if size is None else size)))

    def add_box(
        self,
        x: float,
        y: float,
        z: float,
        w: float,
        h: float,
        d: float,
        *,
        trigger: bool = False,
    ) -> RigidBody3D:
        """静的 AABB。``y`` は底面。"""
        body = self.physics.add_body(
            float(x), float(y), float(z),
            float(w), float(h), float(d),
            is_static=True,
            trigger=trigger,
        )
        self.boxes.append(body)
        if not trigger:
            self._pending.append(("box", float(x), float(y), float(z), float(w), float(h), float(d)))
            self.box_xforms.append([
                float(x), float(y) + float(h) * 0.5, float(z),
                float(w), float(h), float(d), 0.0,
            ])
        return body

    def add_player(
        self,
        x: float = 0.0,
        z: float = 2.0,
        *,
        radius: float = 0.28,
        height: float = 1.7,
    ) -> RigidBody3D:
        """歩くカプセル。底面は床。"""
        self.player = self.physics.add_capsule(
            float(x), self.floor_y, float(z),
            float(radius), float(height),
        )
        return self.player

    def move_player(self, vx: float, vz: float):
        if self.player is None:
            return
        self.player.vx = float(vx)
        self.player.vz = float(vz)

    def update(self, dt: float):
        self.physics.update(dt)

    def bake(self, floor_tex: int, box_tex: int) -> list[int]:
        """予約した床・箱を GPU に一度載せる。エンジン未初期化なら空。"""
        try:
            import kagra
            upload = kagra.upload_mesh_3d
        except Exception:
            return []
        ids: list[int] = []
        need_box = False
        try:
            for item in self._pending:
                if item[0] == "floor":
                    size = item[1]
                    verts, idx = quad_y_mesh(0.0, self.floor_y, 0.0, size)
                    mid = upload(int(floor_tex), verts, idx)
                    if mid:
                        ids.append(int(mid))
                else:
                    need_box = True
            if need_box or self.box_xforms:
                verts, idx = box_mesh(0.0, 0.0, 0.0, 1.0, 1.0, 1.0)
                mid = upload(int(box_tex), verts, idx)
                if mid:
                    ids.append(int(mid))
                    self.box_mesh_id = int(mid)
        except Exception:
            return ids
        self._pending.clear()
        self.mesh_ids.extend(ids)
        return ids

    def draw(self):
        """保持メッシュを描く。箱はインスタンス。"""
        try:
            import kagra
        except Exception:
            return
        for mid in self.mesh_ids:
            if mid == self.box_mesh_id:
                continue
            kagra.draw_mesh_id(mid)
        if self.box_mesh_id and self.box_xforms:
            kagra.draw_mesh_instances(self.box_mesh_id, self.box_xforms)

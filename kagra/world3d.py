"""Minimal 3D world: floor + static boxes + a walking capsule.

Collision is GPU-free (``Physics3D``). Mesh retain happens in ``bake()``
once the renderer exists — tests can skip that and still walk the room.

高さ場は既定でタイル分割する（1 枚だと影 AABB が 24 を超えて空扱いになる）。
``stream_radius`` を付けると歩きながらタイルを載せる / 外す。
"""
from __future__ import annotations

import math
from typing import Callable, Optional

from kagra.gamekit import box_mesh, heightfield_mesh, heightfield_tile, quad_y_mesh
from kagra.land import tile_keys, tile_origin
from kagra.physics3d import Physics3D, RigidBody3D


class World3D:
    """床と箱のある部屋。高さ関数を付けると島になる。カメラは ``Camera3D.follow``。

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
        self.terrain_mesh_id: int = 0
        self.box_xforms: list[list[float]] = []
        self._height_fn = None
        self._height_cells = 40
        self._water_y: Optional[float] = None
        self._tile: Optional[float] = None
        self._stream_radius: Optional[float] = None
        self._lod_radius: Optional[float] = None
        self._lod_cells: Optional[int] = None
        self._terrain_tex: int = 0
        self._tile_meshes: dict[tuple[int, int], int] = {}
        self._tile_lod: dict[tuple[int, int], int] = {}
        self._loaded_tiles: set[tuple[int, int]] = set()
        self._filled_chunks: set[tuple[int, int]] = set()
        self._chunk_fill: Optional[Callable[[int, int], None]] = None
        self._city = None
        self._drawn_dynamic: list[tuple[RigidBody3D, int]] = []
        self._stream_warm = False

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
        draw: bool = True,
        is_static: bool = True,
    ) -> RigidBody3D:
        """AABB。``y`` は底面。``is_static=False`` で積み木。``draw=False`` なら物理だけ。"""
        body = self.physics.add_body(
            float(x), float(y), float(z),
            float(w), float(h), float(d),
            is_static=bool(is_static),
            trigger=trigger,
        )
        self.boxes.append(body)
        if draw and not trigger:
            self._pending.append(("box", float(x), float(y), float(z), float(w), float(h), float(d)))
            self.box_xforms.append([
                float(x), float(y) + float(h) * 0.5, float(z),
                float(w), float(h), float(d), 0.0,
            ])
            if not is_static:
                self._drawn_dynamic.append((body, len(self.box_xforms) - 1))
        return body

    def add_sphere(
        self,
        x: float,
        y: float,
        z: float,
        radius: float,
        *,
        trigger: bool = False,
    ) -> RigidBody3D:
        """静的な球。``y`` は底面。描画はしない（``Prop`` 用）。"""
        body = self.physics.add_sphere(
            float(x), float(y), float(z), float(radius),
            is_static=True,
            trigger=trigger,
        )
        self.boxes.append(body)
        return body

    def add_cylinder(
        self,
        x: float,
        y: float,
        z: float,
        radius: float,
        height: float,
        *,
        trigger: bool = False,
    ) -> RigidBody3D:
        """静的な Y 軸円柱。``y`` は底面。描画はしない（``Prop`` 用）。"""
        body = self.physics.add_cylinder(
            float(x), float(y), float(z), float(radius), float(height),
            is_static=True,
            trigger=trigger,
        )
        self.boxes.append(body)
        return body

    def set_height_fn(
        self,
        fn,
        *,
        cells: int | None = None,
        tile: float | None = 10.0,
        stream_radius: float | None = None,
        lod_radius: float | None = None,
        lod_cells: int | None = None,
    ) -> None:
        """地形 ``(x, z) → y``。平面の床より優先。

        ``tile`` で格子に切る（影の AABB が 24 を超えない）。``None`` で旧来の 1 枚。
        ``stream_radius`` を付けると ``update`` で近くだけ載せる。省略時は半辺全体。
        ``lod_radius`` / ``lod_cells`` で遠いタイルを粗い格子にする（大きい野外）。
        """
        self._height_fn = fn
        self._tile = None if tile is None or float(tile) <= 0 else float(tile)
        self._stream_radius = None if stream_radius is None else float(stream_radius)
        self._lod_radius = None if lod_radius is None else float(lod_radius)
        self._lod_cells = None if lod_cells is None else max(2, int(lod_cells))
        if cells is None:
            self._height_cells = 8 if self._tile else 40
        else:
            self._height_cells = max(2, int(cells))
        self.physics.set_height_fn(fn)

    def _cells_for(self, key: tuple[int, int], x: float, z: float) -> int:
        """プレイヤーからの距離で LOD 格子を選ぶ。"""
        if (
            self._tile is None
            or self._lod_radius is None
            or self._lod_cells is None
        ):
            return self._height_cells
        ox, oz = tile_origin(key[0], key[1], self._tile)
        cx = ox + self._tile * 0.5
        cz = oz + self._tile * 0.5
        if math.hypot(cx - float(x), cz - float(z)) > self._lod_radius:
            return int(self._lod_cells)
        return self._height_cells

    def add_trimesh(self, verts, indices, *, is_static: bool = True) -> RigidBody3D:
        """静的な三角形当たり。描画は呼び出し側で ``upload_mesh_3d``。"""
        body = self.physics.add_trimesh(verts, indices, is_static=is_static)
        self.boxes.append(body)
        return body

    def load_city(self, path: str) -> dict:
        """街 JSON を読む。タイル初回ロードで箱を置く。OSM ではない。"""
        from kagra.city import load_city

        self._city = load_city(path)
        return self._city

    def set_chunk_fill(self, fn: Callable[[int, int], None] | None) -> None:
        """タイルを初めて載せるとき ``fn(ix, iz)``。箱街区など。外しても箱は残る。"""
        self._chunk_fill = fn

    def loaded_tiles(self) -> frozenset:
        """今ほしい／載せているタイルキー。GPU が無くても更新される。"""
        return frozenset(self._loaded_tiles)

    def wanted_tiles(self, x: float, z: float) -> list[tuple[int, int]]:
        """プレイヤー位置から載せるタイル。"""
        if self._height_fn is None:
            return []
        if self._tile is None:
            return [(0, 0)]
        if self._stream_radius is None:
            radius = self.half * 1.42 + self._tile
            return tile_keys(0.0, 0.0, tile=self._tile, radius=radius, half=self.half)
        return tile_keys(
            float(x), float(z),
            tile=self._tile, radius=self._stream_radius, half=self.half,
        )

    def set_water_y(self, y: float | None) -> None:
        """水面。``None`` で消す。"""
        self._water_y = None if y is None else float(y)
        self.physics.set_water_y(self._water_y)

    def in_water(self, body: Optional[RigidBody3D] = None) -> bool:
        b = body if body is not None else self.player
        if b is None:
            return False
        return self.physics.in_water(b)

    def ground_y(self, x: float, z: float) -> float:
        """その XZ の地面の高さ。"""
        if self._height_fn is not None:
            return float(self._height_fn(float(x), float(z)))
        return float(self.floor_y)

    def add_player(
        self,
        x: float = 0.0,
        z: float = 2.0,
        *,
        radius: float = 0.28,
        height: float = 1.7,
    ) -> RigidBody3D:
        """歩くカプセル。底面は ``ground_y(x, z)``（高さ場があればその上）。"""
        gy = self.ground_y(x, z)
        self.player = self.physics.add_capsule(
            float(x), gy, float(z),
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
        self._sync_dynamic_xforms()
        p = self.player
        if p is None:
            return
        lim = self.half - 0.2
        if p.x > lim:
            p.x = lim
            p.vx = 0.0
        elif p.x < -lim:
            p.x = -lim
            p.vx = 0.0
        if p.z > lim:
            p.z = lim
            p.vz = 0.0
        elif p.z < -lim:
            p.z = -lim
            p.vz = 0.0
        if self._tile is not None and self._stream_radius is not None:
            max_new = 1 if self._stream_warm else None
            self.stream_tiles(p.x, p.z, max_new=max_new)
            self._stream_warm = True
        self._trace_player()

    def _trace_player(self) -> None:
        """Feed ``debug_trace`` when an agent has started a tracer. GPU-free.

        Quiet unless ``kagra.trace._ACTIVE`` is set (or someone called
        ``kagra.debug_trace(..., reset=True)``). Crest Isle / Relic Run /
        Overworld all go through ``World3D.update``.
        """
        p = self.player
        if p is None:
            return
        try:
            from kagra.trace import _ACTIVE, debug_trace
        except Exception:
            return
        if _ACTIVE is None:
            return
        debug_trace(
            foot_y=p.y, x=p.x, z=p.z,
            world=self,
            vx=p.vx, vz=p.vz,
            on_ground=p.on_ground,
            persist=_ACTIVE.persist,
            threshold=_ACTIVE.threshold,
            path=_ACTIVE.path,
        )

    def stream_tiles(self, x: float, z: float, *, max_new: int | None = None) -> int:
        """近くのタイルを載せ、遠いタイルを外す。エンジン無しでもキーは更新する。

        ``max_new`` は新規タイル数の上限。``None`` は無制限（最初のリング /
        テスト）。歩き中は ``update`` が 1 枚/フレームに絞る。
        """
        if self._height_fn is None:
            return 0
        want = set(self.wanted_tiles(x, z))
        for key in list(self._loaded_tiles):
            if key not in want:
                self._unload_tile(key)
                self._loaded_tiles.discard(key)
                continue
            if self._tile_lod.get(key) != self._cells_for(key, x, z):
                self._unload_tile(key)
                self._loaded_tiles.discard(key)
        added = 0
        for key in want:
            if key in self._loaded_tiles:
                continue
            if max_new is not None and added >= max_new:
                break
            self._loaded_tiles.add(key)
            if key not in self._filled_chunks:
                if self._chunk_fill is not None:
                    self._chunk_fill(key[0], key[1])
                self._place_city_chunk(key[0], key[1])
                self._filled_chunks.add(key)
            self._upload_tile(key, viewer_x=x, viewer_z=z)
            added += 1
        return len(self._loaded_tiles)

    def _place_city_chunk(self, ix: int, iz: int) -> None:
        if not self._city:
            return
        from kagra.city import city_chunk

        tile = float(self._city.get("tile", self._tile or 10.0))
        for x, y, z, w, h, d in city_chunk(self._city, ix, iz, tile=tile):
            gy = self.ground_y(x, z)
            self.add_box(x, gy if abs(y) < 1e-9 else float(y), z, w, h, d)

    def _sync_dynamic_xforms(self) -> None:
        for body, i in self._drawn_dynamic:
            if i < 0 or i >= len(self.box_xforms):
                continue
            xf = self.box_xforms[i]
            xf[0] = float(body.x)
            xf[1] = float(body.y) + float(body.h) * 0.5
            xf[2] = float(body.z)

    def _unload_tile(self, key: tuple[int, int]) -> None:
        self._tile_lod.pop(key, None)
        mid = self._tile_meshes.pop(key, None)
        if not mid:
            return
        try:
            import kagra
            kagra.unload_mesh_3d(int(mid))
        except Exception:
            pass
        if mid in self.mesh_ids:
            self.mesh_ids.remove(mid)
        if self.terrain_mesh_id == mid:
            self.terrain_mesh_id = 0

    def _upload_tile(
        self, key: tuple[int, int], *, viewer_x: float = 0.0, viewer_z: float = 0.0,
    ) -> int:
        cells = self._cells_for(key, viewer_x, viewer_z)
        self._tile_lod[key] = cells
        if self._height_fn is None or self._terrain_tex <= 0:
            return 0
        try:
            import kagra
            if self._tile is None:
                verts, idx = heightfield_mesh(
                    self._height_fn, self.half, cells,
                )
            else:
                ox, oz = tile_origin(key[0], key[1], self._tile)
                verts, idx = heightfield_tile(
                    self._height_fn, ox, oz, self._tile, cells,
                    uv_half=self.half,
                )
            mid = kagra.upload_mesh_3d(int(self._terrain_tex), verts, idx)
        except Exception:
            return 0
        if not mid:
            return 0
        mid = int(mid)
        self._tile_meshes[key] = mid
        self.terrain_mesh_id = mid
        if mid not in self.mesh_ids:
            self.mesh_ids.append(mid)
        return mid

    def bake_terrain(self, tex: int) -> int:
        """高さ場メッシュを GPU に載せる。関数未設定またはエンジン無しなら 0。"""
        if self._height_fn is None:
            return 0
        self._terrain_tex = int(tex)
        x, z = 0.0, 0.0
        if self.player is not None:
            x, z = float(self.player.x), float(self.player.z)
        self.stream_tiles(x, z)
        self._stream_warm = True
        return int(self.terrain_mesh_id)

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
            if need_box or self.box_xforms or self._chunk_fill is not None:
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

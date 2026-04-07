from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Dict, List, Optional

def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def _normalize_angle_deg(a: float) -> float:
    while a > 180.0:
        a -= 360.0
    while a < -180.0:
        a += 360.0
    return a


def _lerp_angle_deg(a: float, b: float, t: float) -> float:
    d = _normalize_angle_deg(b - a)
    return a + d * t


@dataclass
class Transform2D:
    x: float = 0.0
    y: float = 0.0
    rotation: float = 0.0
    scale_x: float = 1.0
    scale_y: float = 1.0

    def copy(self) -> "Transform2D":
        return Transform2D(
            x=self.x,
            y=self.y,
            rotation=self.rotation,
            scale_x=self.scale_x,
            scale_y=self.scale_y,
        )


def combine_transform(parent: Transform2D, local: Transform2D) -> Transform2D:
    rad = math.radians(parent.rotation)
    cos_r = math.cos(rad)
    sin_r = math.sin(rad)

    lx = local.x * parent.scale_x
    ly = local.y * parent.scale_y

    rx = lx * cos_r - ly * sin_r
    ry = lx * sin_r + ly * cos_r

    return Transform2D(
        x=parent.x + rx,
        y=parent.y + ry,
        rotation=parent.rotation + local.rotation,
        scale_x=parent.scale_x * local.scale_x,
        scale_y=parent.scale_y * local.scale_y,
    )


@dataclass
class Attachment:
    texture_id: int
    width: float
    height: float
    offset_x: float = 0.0
    offset_y: float = 0.0
    pivot_x: float = 0.5
    pivot_y: float = 0.5
    sx: float = 0.0
    sy: float = 0.0
    sw: float = 0.0
    sh: float = 0.0
    alpha: float = 1.0
    visible: bool = True


@dataclass
class MeshVertex:
    """メッシュの1頂点。ローカル座標・UV・ボーンウェイトを持つ。"""
    x: float = 0.0          # ローカル座標
    y: float = 0.0
    u: float = 0.0          # UV
    v: float = 0.0
    bones: list = field(default_factory=list)   # [(bone_name, weight), ...]


@dataclass
class MeshAttachment:
    """ボーンウェイト付きメッシュアタッチメント。
    
    頂点ごとに複数ボーンの影響を受けて変形する。
    Live2D 的なぐにゃっとした変形に使う。

    Example::
        mesh = MeshAttachment(
            texture_id=tex,
            vertices=[
                MeshVertex(0,   0,   0.0, 0.0, [("hip", 1.0)]),
                MeshVertex(64,  0,   1.0, 0.0, [("hip", 1.0)]),
                MeshVertex(64, 128,  1.0, 1.0, [("hip", 0.3), ("chest", 0.7)]),
                MeshVertex(0,  128,  0.0, 1.0, [("hip", 0.3), ("chest", 0.7)]),
            ],
            triangles=[(0,1,2), (0,2,3)],
        )
        bone.mesh = mesh
    """
    texture_id: int = 0
    vertices: list = field(default_factory=list)   # list[MeshVertex]
    triangles: list = field(default_factory=list)  # [(i0,i1,i2), ...]
    alpha: float = 1.0
    visible: bool = True

    def deform(self, bone_worlds: dict) -> list:
        """ボーンのワールド変換を適用して変形後の頂点リストを返す。
        
        Args:
            bone_worlds: {bone_name: Transform2D} のワールド座標辞書
        
        Returns:
            [(wx, wy, u, v), ...] 変形後の頂点リスト
        """
        result = []
        for vert in self.vertices:
            wx, wy = 0.0, 0.0
            total_w = 0.0

            for bone_name, weight in vert.bones:
                if bone_name not in bone_worlds:
                    continue
                bt = bone_worlds[bone_name]
                rad = math.radians(bt.rotation)
                cos_r = math.cos(rad)
                sin_r = math.sin(rad)

                # ローカル座標をボーンのワールド変換で変換
                lx = vert.x * bt.scale_x
                ly = vert.y * bt.scale_y
                bx = bt.x + lx * cos_r - ly * sin_r
                by = bt.y + lx * sin_r + ly * cos_r

                wx += bx * weight
                wy += by * weight
                total_w += weight

            # ウェイト合計が 1.0 になるよう正規化
            if total_w > 0 and abs(total_w - 1.0) > 0.001:
                wx /= total_w
                wy /= total_w

            result.append((wx, wy, vert.u, vert.v))

        return result

    def triangulated_verts(self, bone_worlds: dict) -> list:
        """三角形リストに展開した変形後頂点を返す。
        
        Returns:
            [(wx, wy, u, v), ...] 三角形ごとに3頂点を展開したリスト
        """
        deformed = self.deform(bone_worlds)
        out = []
        for i0, i1, i2 in self.triangles:
            out.append(deformed[i0])
            out.append(deformed[i1])
            out.append(deformed[i2])
        return out


@dataclass
class Bone:
    name: str
    parent: Optional[str] = None
    local: Transform2D = field(default_factory=Transform2D)
    world: Transform2D = field(default_factory=Transform2D)
    attachment: Optional[Attachment] = None
    mesh: Optional["MeshAttachment"] = None


@dataclass
class Keyframe:
    time: float
    x: Optional[float] = None
    y: Optional[float] = None
    rotation: Optional[float] = None
    scale_x: Optional[float] = None
    scale_y: Optional[float] = None


class AnimationTrack:
    def __init__(self, bone_name: str):
        self.bone_name = bone_name
        self.frames: List[Keyframe] = []

    def add(
        self,
        time: float,
        x: Optional[float] = None,
        y: Optional[float] = None,
        rotation: Optional[float] = None,
        scale_x: Optional[float] = None,
        scale_y: Optional[float] = None,
    ) -> "AnimationTrack":
        self.frames.append(
            Keyframe(
                time=time,
                x=x,
                y=y,
                rotation=rotation,
                scale_x=scale_x,
                scale_y=scale_y,
            )
        )
        self.frames.sort(key=lambda f: f.time)
        return self

    def sample(self, t: float, fallback: Transform2D) -> Transform2D:
        if not self.frames:
            return fallback.copy()

        if t <= self.frames[0].time:
            return self._apply_single(self.frames[0], fallback)

        if t >= self.frames[-1].time:
            return self._apply_single(self.frames[-1], fallback)

        for i in range(len(self.frames) - 1):
            a = self.frames[i]
            b = self.frames[i + 1]
            if a.time <= t <= b.time:
                span = b.time - a.time
                if span <= 0:
                    return self._apply_single(a, fallback)
                k = (t - a.time) / span
                return self._apply_pair(a, b, k, fallback)

        return fallback.copy()

    def _apply_single(self, frame: Keyframe, fallback: Transform2D) -> Transform2D:
        out = fallback.copy()
        if frame.x is not None:
            out.x = frame.x
        if frame.y is not None:
            out.y = frame.y
        if frame.rotation is not None:
            out.rotation = frame.rotation
        if frame.scale_x is not None:
            out.scale_x = frame.scale_x
        if frame.scale_y is not None:
            out.scale_y = frame.scale_y
        return out

    def _apply_pair(
        self, a: Keyframe, b: Keyframe, t: float, fallback: Transform2D
    ) -> Transform2D:
        out = fallback.copy()

        ax = fallback.x if a.x is None else a.x
        bx = fallback.x if b.x is None else b.x
        ay = fallback.y if a.y is None else a.y
        by = fallback.y if b.y is None else b.y
        ar = fallback.rotation if a.rotation is None else a.rotation
        br = fallback.rotation if b.rotation is None else b.rotation
        asx = fallback.scale_x if a.scale_x is None else a.scale_x
        bsx = fallback.scale_x if b.scale_x is None else b.scale_x
        asy = fallback.scale_y if a.scale_y is None else a.scale_y
        bsy = fallback.scale_y if b.scale_y is None else b.scale_y

        out.x = _lerp(ax, bx, t)
        out.y = _lerp(ay, by, t)
        out.rotation = _lerp_angle_deg(ar, br, t)
        out.scale_x = _lerp(asx, bsx, t)
        out.scale_y = _lerp(asy, bsy, t)
        return out


class AnimationClip:
    def __init__(self, name: str, length: float, loop: bool = True):
        self.name = name
        self.length = max(0.0001, length)
        self.loop = loop
        self.tracks: Dict[str, AnimationTrack] = {}

    def track(self, bone_name: str) -> AnimationTrack:
        if bone_name not in self.tracks:
            self.tracks[bone_name] = AnimationTrack(bone_name)
        return self.tracks[bone_name]

    def sample_pose(
        self,
        time_sec: float,
        base_pose: Dict[str, Transform2D],
    ) -> Dict[str, Transform2D]:
        t = (time_sec % self.length) if self.loop else min(time_sec, self.length)
        pose = {name: tr.copy() for name, tr in base_pose.items()}
        for bone_name, track in self.tracks.items():
            if bone_name in pose:
                pose[bone_name] = track.sample(t, pose[bone_name])
        return pose


class Skeleton:
    def __init__(self):
        self.bones: Dict[str, Bone] = {}
        self.draw_order: List[str] = []
        self._topo_order: List[str] = []   # update_world 用トポロジカル順（親が先）
        self.root_position = Transform2D()

    def _rebuild_topo_order(self) -> None:
        """親子関係に基づく BFS トポロジカル順を再計算する。
        add_bone / set_draw_order のたびに呼ばれる。"""
        # ルートボーン（parent=None）からキューで展開
        order: List[str] = []
        children: Dict[str, List[str]] = {n: [] for n in self.bones}
        for name, bone in self.bones.items():
            if bone.parent is not None and bone.parent in children:
                children[bone.parent].append(name)
        from collections import deque
        queue: deque = deque(n for n, b in self.bones.items() if b.parent is None)
        visited: set = set()
        while queue:
            name = queue.popleft()
            if name in visited:
                continue
            visited.add(name)
            order.append(name)
            for child in children.get(name, []):
                queue.append(child)
        # 万一孤立ボーンがあれば末尾追加
        for name in self.bones:
            if name not in visited:
                order.append(name)
        self._topo_order = order

    def add_bone(
        self,
        name: str,
        parent: Optional[str] = None,
        x: float = 0.0,
        y: float = 0.0,
        rotation: float = 0.0,
        scale_x: float = 1.0,
        scale_y: float = 1.0,
    ) -> Bone:
        if name in self.bones:
            raise ValueError(f"Bone already exists: {name}")
        if parent is not None and parent not in self.bones:
            raise KeyError(f"Parent bone not found: {parent}")

        bone = Bone(
            name=name,
            parent=parent,
            local=Transform2D(x, y, rotation, scale_x, scale_y),
            world=Transform2D(x, y, rotation, scale_x, scale_y),
        )
        self.bones[name] = bone
        self.draw_order.append(name)
        self._rebuild_topo_order()
        return bone

    def set_draw_order(self, names: List[str]) -> None:
        if set(names) != set(self.bones.keys()):
            raise ValueError("set_draw_order names must match all bones exactly")
        self.draw_order = list(names)
        self._rebuild_topo_order()   # draw_order 変更後も topo_order は独立して正しく保つ

    def set_attachment(
        self,
        bone_name: str,
        texture_id: int,
        width: float,
        height: float,
        offset_x: float = 0.0,
        offset_y: float = 0.0,
        pivot_x: float = 0.5,
        pivot_y: float = 0.5,
        sx: float = 0.0,
        sy: float = 0.0,
        sw: float = 0.0,
        sh: float = 0.0,
        alpha: float = 1.0,
    ) -> None:
        if bone_name not in self.bones:
            raise KeyError(f"Bone not found: {bone_name}")

        self.bones[bone_name].attachment = Attachment(
            texture_id=texture_id,
            width=width,
            height=height,
            offset_x=offset_x,
            offset_y=offset_y,
            pivot_x=pivot_x,
            pivot_y=pivot_y,
            sx=sx,
            sy=sy,
            sw=sw,
            sh=sh,
            alpha=alpha,
        )

    def base_pose(self) -> Dict[str, Transform2D]:
        return {name: bone.local.copy() for name, bone in self.bones.items()}

    def apply_pose(self, pose: Dict[str, Transform2D]) -> None:
        for name, tr in pose.items():
            if name in self.bones:
                self.bones[name].local = tr.copy()

    def set_mesh(self, bone_name: str, mesh: "MeshAttachment") -> None:
        """ボーンにメッシュアタッチメントをセットする。

        Example::
            skel.set_mesh("spine", MeshAttachment(
                texture_id=tex,
                vertices=[
                    MeshVertex(0,   0,   0.0, 0.0, [("spine", 1.0)]),
                    MeshVertex(64,  0,   1.0, 0.0, [("spine", 1.0)]),
                    MeshVertex(64, 128,  1.0, 1.0, [("spine", 0.2), ("chest", 0.8)]),
                    MeshVertex(0,  128,  0.0, 1.0, [("spine", 0.2), ("chest", 0.8)]),
                ],
                triangles=[(0,1,2),(0,2,3)],
            ))
        """
        if bone_name not in self.bones:
            raise KeyError(f"Bone not found: {bone_name}")
        self.bones[bone_name].mesh = mesh

    def clear_mesh(self, bone_name: str) -> None:
        """ボーンのメッシュアタッチメントを削除する。"""
        if bone_name in self.bones:
            self.bones[bone_name].mesh = None

    def update_world(self, root_x: float = 0.0, root_y: float = 0.0) -> None:
        root = Transform2D(x=root_x, y=root_y, rotation=0.0, scale_x=1.0, scale_y=1.0)

        # トポロジカル順（親が先）でワールド行列を計算する。
        # draw_order とは独立しており、set_draw_order で順番を変えても正しく動作する。
        for name in self._topo_order:
            bone = self.bones[name]
            if bone.parent is None:
                bone.world = combine_transform(root, bone.local)
            else:
                bone.world = combine_transform(self.bones[bone.parent].world, bone.local)

    def _bone_worlds(self) -> dict:
        """全ボーンのワールド座標辞書を返す。"""
        return {name: bone.world for name, bone in self.bones.items()}

    def draw(self, kagra_module) -> None:
        bone_worlds = self._bone_worlds()

        for name in self.draw_order:
            bone = self.bones[name]

            # ── メッシュアタッチメント ──────────────────────
            mesh = bone.mesh
            if mesh and mesh.visible:
                verts = mesh.triangulated_verts(bone_worlds)
                if verts:
                    self._draw_mesh(kagra_module, mesh.texture_id, verts, mesh.alpha)
                continue   # mesh があれば通常 attachment はスキップ

            # ── 通常アタッチメント ──────────────────────────
            att = bone.attachment
            if not att or not att.visible:
                continue

            kagra_module.draw_texture(
                att.texture_id,
                bone.world.x + att.offset_x,
                bone.world.y + att.offset_y,
                att.width,
                att.height,
                att.sx,
                att.sy,
                att.sw if att.sw > 0 else None,
                att.sh if att.sh > 0 else None,
                att.alpha,
                bone.world.rotation,
                att.pivot_x,
                att.pivot_y,
            )

    def _draw_mesh(self, kagra_module, texture_id: int,
                   verts: list, alpha: float,
                   shader_id: int = 0,
                   shader_params: list = None) -> None:
        """変形後メッシュを GPU で描画する。

        verts: [(wx, wy, u, v), ...] 三角形リスト（3の倍数）

        kagra_module.draw_mesh() に頂点を直接渡して
        SpriteVertex バッファとして GPU 描画する。
        """
        if len(verts) < 3:
            return

        # (wx, wy, u, v) → (wx, wy, u, v, alpha) に変換
        gpu_verts = [[v[0], v[1], v[2], v[3], alpha] for v in verts]
        kagra_module.draw_mesh(texture_id, gpu_verts,
                               shader_id, shader_params)


class SkeletonAnimator:
    def __init__(self, skeleton: Skeleton):
        self.skeleton = skeleton
        self.clips: Dict[str, AnimationClip] = {}
        self.current: Optional[AnimationClip] = None
        self.current_name: Optional[str] = None
        self.time = 0.0
        self._base_pose = self.skeleton.base_pose()

    def add_clip(self, clip: AnimationClip) -> None:
        self.clips[clip.name] = clip

    def play(self, name: str, reset: bool = False) -> None:
        if name not in self.clips:
            raise KeyError(f"Animation clip not found: {name}")
        if self.current_name == name and self.current is not None:
            if reset:
                self.time = 0.0
            return
        self.current = self.clips[name]
        self.current_name = name
        if reset:
            self.time = 0.0

    def update(self, dt: float) -> None:
        if self.current is None:
            return
        self.time += dt
        pose = self.current.sample_pose(self.time, self._base_pose)
        self.skeleton.apply_pose(pose)

    def draw(self, kagra_module, root_x: float, root_y: float) -> None:
        self.skeleton.update_world(root_x, root_y)
        self.skeleton.draw(kagra_module)
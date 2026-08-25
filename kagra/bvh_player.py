# kagra/bvh_player.py
"""
BVH モーションプレイヤー
Mesh2Motion / Mixamo / DeepMotion 等の BVH を VrmAvatar で再生する。

Example::
    avatar = kagra.avatar("assets/MyModel.vrm")

    # 1行でロード＋登録
    avatar.load_motion("dance", "assets/dance.bvh")
    avatar.play("dance", loop=True)

    # または細かく制御したい場合
    motion = kagra.load_bvh("assets/walk.bvh")
    print(f"{motion.fps}fps / {motion.duration:.1f}sec")
    avatar.add_motion("walk", motion)
"""
from __future__ import annotations
import math
import re
from dataclasses import dataclass, field
from typing import Optional


# ── ボーン名正規化 ──────────────────────────────────────────────

# プレフィックスを除去するパターン
# "mixamorig:LeftArm" / "mixamorig1:LeftArm" → "LeftArm"
_PREFIX_RE = re.compile(r'^[A-Za-z0-9_]+:', )

def _normalize(name: str) -> str:
    """ボーン名からプレフィックスを除去して正規化する。"""
    return _PREFIX_RE.sub('', name).strip()


# ── VRM J_Bip_* マッピング ─────────────────────────────────────
# 正規化後のボーン名 → VRM 骨名
_VRM_MAP: dict[str, str] = {
    # 体幹
    "Hips":        "J_Bip_C_Hips",
    "Hip":         "J_Bip_C_Hips",
    "Pelvis":      "J_Bip_C_Hips",
    "Spine":       "J_Bip_C_Spine",
    "Spine1":      "J_Bip_C_Chest",
    "Spine2":      "J_Bip_C_UpperChest",
    "Chest":       "J_Bip_C_Chest",
    "UpperChest":  "J_Bip_C_UpperChest",
    "Neck":        "J_Bip_C_Neck",
    "Head":        "J_Bip_C_Head",
    # 左腕（標準 / Bandai Namco）
    "LeftShoulder":  "J_Bip_L_Shoulder",
    "Shoulder_L":    "J_Bip_L_Shoulder",
    "LeftArm":       "J_Bip_L_UpperArm",
    "UpperArm_L":    "J_Bip_L_UpperArm",
    "LeftForeArm":   "J_Bip_L_LowerArm",
    "LowerArm_L":    "J_Bip_L_LowerArm",
    "LeftHand":      "J_Bip_L_Hand",
    "Hand_L":        "J_Bip_L_Hand",
    # 右腕（標準 / Bandai Namco）
    "RightShoulder": "J_Bip_R_Shoulder",
    "Shoulder_R":    "J_Bip_R_Shoulder",
    "RightArm":      "J_Bip_R_UpperArm",
    "UpperArm_R":    "J_Bip_R_UpperArm",
    "RightForeArm":  "J_Bip_R_LowerArm",
    "LowerArm_R":    "J_Bip_R_LowerArm",
    "RightHand":     "J_Bip_R_Hand",
    "Hand_R":        "J_Bip_R_Hand",
    # 左脚（標準 / Bandai Namco）
    "LeftUpLeg":     "J_Bip_L_UpperLeg",
    "UpperLeg_L":    "J_Bip_L_UpperLeg",
    "LeftLeg":       "J_Bip_L_LowerLeg",
    "LowerLeg_L":    "J_Bip_L_LowerLeg",
    "LeftFoot":      "J_Bip_L_Foot",
    "Foot_L":        "J_Bip_L_Foot",
    "LeftToeBase":   "J_Bip_L_ToeBase",
    "Toes_L":        "J_Bip_L_ToeBase",
    # 右脚（標準 / Bandai Namco）
    "RightUpLeg":    "J_Bip_R_UpperLeg",
    "UpperLeg_R":    "J_Bip_R_UpperLeg",
    "RightLeg":      "J_Bip_R_LowerLeg",
    "LowerLeg_R":    "J_Bip_R_LowerLeg",
    "RightFoot":     "J_Bip_R_Foot",
    "Foot_R":        "J_Bip_R_Foot",
    "RightToeBase":  "J_Bip_R_ToeBase",
    "Toes_R":        "J_Bip_R_ToeBase",
}


def _to_vrm(raw_name: str, extra: dict) -> Optional[str]:
    """BVH ボーン名 → VRM 骨名（None = 対象外）"""
    # 既に J_Bip_* 形式ならそのまま使う（VRoid由来BVH）
    if raw_name.startswith("J_Bip_"):
        return raw_name
    norm = _normalize(raw_name)
    return extra.get(norm) or extra.get(raw_name) \
        or _VRM_MAP.get(norm) or _VRM_MAP.get(raw_name)


# ── クォータニオン ──────────────────────────────────────────────

def _axis_quat(ax: float, ay: float, az: float, angle: float) -> list:
    s = math.sin(angle / 2)
    return [ax*s, ay*s, az*s, math.cos(angle/2)]

def _qmul(a: list, b: list) -> list:
    ax,ay,az,aw = a; bx,by,bz,bw = b
    return [aw*bx+ax*bw+ay*bz-az*by,
            aw*by-ax*bz+ay*bw+az*bx,
            aw*bz+ax*by-ay*bx+az*bw,
            aw*bw-ax*bx-ay*by-az*bz]

_AXES = {'X':(1,0,0), 'Y':(0,1,0), 'Z':(0,0,1)}

def euler_to_quat(angles_deg: list, order: str) -> list:
    """任意回転順序のオイラー角（度） → クォータニオン [x,y,z,w]。

    BVH の CHANNELS に書いてある順序をそのまま渡す。
    例: order="ZXY", angles_deg=[rz, rx, ry]
    """
    q = [0., 0., 0., 1.]
    # 最後の軸から順に右から掛ける（外因的回転）
    for axis, deg in zip(reversed(order), reversed(angles_deg)):
        ax = _AXES[axis.upper()]
        q = _qmul(q, _axis_quat(*ax, math.radians(deg)))
    return q


# ── BVH データ構造 ─────────────────────────────────────────────

@dataclass
class _Joint:
    name:     str
    channels: list[str]
    vrm_name: Optional[str]
    parent:   Optional[int] = None

def _qinv(q: list) -> list:
    """単位クォータニオンの逆。"""
    return [-q[0], -q[1], -q[2], q[3]]


def _quat_normalize(q: list) -> list:
    n = math.sqrt(q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3]) or 1.0
    return [q[0]/n, q[1]/n, q[2]/n, q[3]/n]


@dataclass
class BvhMotion:
    """パース済み BVH モーションデータ。

    Attributes:
        frame_time: 1フレームの秒数
        frames:     フラットなチャンネル値リスト per フレーム
        fps:        フレームレート
        duration:   総再生時間（秒）
        up_axis:    'y' | 'z' — ルート位置の軸変換に使う
    """
    _joints:    list[_Joint]
    frame_time: float
    frames:     list[list[float]]
    up_axis:    str = "y"
    _cache:     Optional[list] = field(default=None, repr=False)
    src_worlds: dict = field(default_factory=dict)

    @property
    def fps(self) -> float:
        return 1.0 / self.frame_time if self.frame_time > 0 else 30.0

    @property
    def duration(self) -> float:
        return len(self.frames) * self.frame_time

    def _remap_pos(self, x: float, y: float, z: float) -> tuple[float, float, float]:
        """BVH 位置 → VRM (Y-up)。"""
        if self.up_axis == "z":
            # Z-up → Y-up: (x, y, z) → (x, z, -y)
            return (x, z, -y)
        return (x, y, z)

    def _remap_hips_quat(self, q: list) -> list:
        """Hips 回転の軸合わせ。Y-up はそのまま。"""
        if self.up_axis != "z":
            return q
        # Z-up → Y-up: X 軸 -90° で共役
        q_rot = [-0.7071067811865475, 0.0, 0.0, 0.7071067811865476]
        q_rot_inv = [0.7071067811865475, 0.0, 0.0, 0.7071067811865476]
        return _qmul(_qmul(q_rot, q), q_rot_inv)

    def _frame_abs(self, frame_vals: list[float]) -> tuple[dict, tuple[float, float, float]]:
        """1 フレーム分の絶対ローカル回転（と Hips 位置）を返す。"""
        bones: dict = {}
        hips_pos = (0.0, 0.0, 0.0)
        offset = 0
        for joint in self._joints:
            n = len(joint.channels)
            vals = frame_vals[offset:offset + n]
            offset += n

            if joint.name in ("Root", "root") and joint.vrm_name is None:
                px = py = pz = None
                for ch, v in zip(joint.channels, vals):
                    chu = ch.upper()
                    if "XPOS" in chu: px = v
                    elif "YPOS" in chu: py = v
                    elif "ZPOS" in chu: pz = v
                if None not in (px, py, pz):
                    hips_pos = self._remap_pos(px, py, pz)

            if joint.vrm_name is None:
                continue

            pos = {}
            for ch, v in zip(joint.channels, vals):
                chu = ch.upper()
                if "XPOS" in chu: pos["x"] = v
                elif "YPOS" in chu: pos["y"] = v
                elif "ZPOS" in chu: pos["z"] = v

            ro, rv = [], []
            for ch, v in zip(joint.channels, vals):
                if "ROTATION" in ch.upper():
                    ro.append(ch[0].upper())
                    rv.append(v)

            if len(ro) != 3:
                continue

            q = euler_to_quat(rv, "".join(ro))
            if joint.vrm_name == "J_Bip_C_Hips":
                q = self._remap_hips_quat(q)
                if len(pos) == 3:
                    hips_pos = self._remap_pos(pos["x"], pos["y"], pos["z"])

            bones[joint.vrm_name] = _quat_normalize(q)

        return bones, hips_pos

    def _frame_locals(self, frame_vals: list[float]) -> list:
        """Per-joint rest/anim local quats (identity if the joint has no rotation)."""
        locals_q = [[0.0, 0.0, 0.0, 1.0] for _ in self._joints]
        offset = 0
        for i, joint in enumerate(self._joints):
            n = len(joint.channels)
            vals = frame_vals[offset:offset + n]
            offset += n
            ro, rv = [], []
            for ch, v in zip(joint.channels, vals):
                if "ROTATION" in ch.upper():
                    ro.append(ch[0].upper())
                    rv.append(v)
            if len(ro) != 3:
                continue
            q = euler_to_quat(rv, "".join(ro))
            if joint.vrm_name == "J_Bip_C_Hips":
                q = self._remap_hips_quat(q)
            locals_q[i] = _quat_normalize(q)
        return locals_q

    def rest_worlds(self) -> dict:
        """Frame-0 world rest (same rest ``to_clip`` deltas are relative to)."""
        if not self.frames:
            return {}
        if self.src_worlds:
            return self.src_worlds
        locals_q = self._frame_locals(self.frames[0])
        worlds_i = [[0.0, 0.0, 0.0, 1.0] for _ in self._joints]
        out = {}
        for i, joint in enumerate(self._joints):
            local = locals_q[i]
            if joint.parent is None:
                worlds_i[i] = local
            else:
                worlds_i[i] = _quat_normalize(_qmul(worlds_i[joint.parent], local))
            if joint.vrm_name:
                out[joint.vrm_name] = tuple(worlds_i[i])
        self.src_worlds = out
        return out

    def to_clip(self) -> list:
        """VrmAvatar 向け [(bones_dict, duration, root_pos), ...] に変換する。

        各ボーン回転は **フレーム0（レスト）に対するデルタ**。
        `_Animator` が `bind_q * delta` で適用するため、FBX パスと同じ意味になる。
        """
        if self._cache is not None:
            return self._cache
        if not self.frames:
            self._cache = []
            return self._cache

        rest_bones, rest_pos = self._frame_abs(self.frames[0])
        rest_inv = {n: _qinv(q) for n, q in rest_bones.items()}

        clip = []
        for frame_vals in self.frames:
            abs_bones, abs_pos = self._frame_abs(frame_vals)
            deltas = {}
            for name, q in abs_bones.items():
                inv = rest_inv.get(name, [0.0, 0.0, 0.0, 1.0])
                deltas[name] = _quat_normalize(_qmul(inv, q))

            # ルート位置はフレーム0からの相対（モデルを原点付近に保つ）
            root_pos = (
                abs_pos[0] - rest_pos[0],
                abs_pos[1] - rest_pos[1],
                abs_pos[2] - rest_pos[2],
            )
            clip.append((deltas, self.frame_time, root_pos))

        self._cache = clip
        self.rest_worlds()
        return clip


# ── BVH パーサー ───────────────────────────────────────────────

def _skip_block(lines: list[str], idx: int) -> int:
    """{ } ブロックを深さ追跡しながらスキップする。"""
    depth = 0
    while idx < len(lines):
        line = lines[idx].strip()
        if   line == "{": depth += 1
        elif line == "}":
            depth -= 1
            idx += 1
            if depth <= 0: break
            continue
        idx += 1
    return idx

def _parse_joint(lines: list[str], idx: int,
                 joints: list[_Joint],
                 extra_map: dict,
                 parent: int | None = None) -> int:
    """ROOT / JOINT ブロックを再帰的にパースする。"""
    raw_name = lines[idx].split()[-1]
    joint = _Joint(
        name=raw_name,
        channels=[],
        vrm_name=_to_vrm(raw_name, extra_map),
        parent=parent,
    )
    my_idx = len(joints)
    joints.append(joint)
    idx += 1  # { を読み飛ばす
    if idx < len(lines) and lines[idx].strip() == "{":
        idx += 1

    while idx < len(lines):
        tok = lines[idx].strip().split()
        if not tok: idx += 1; continue
        kw = tok[0].upper()

        if kw == "CHANNELS":
            n = int(tok[1])
            joint.channels = tok[2:2+n]
            idx += 1
        elif kw == "JOINT":
            idx = _parse_joint(lines, idx, joints, extra_map, parent=my_idx)
        elif kw == "END":          # End Site
            idx += 1               # "End Site" 行
            idx = _skip_block(lines, idx)
        elif kw in ("OFFSET", "HIERARCHY", "MOTION"):
            idx += 1
        elif kw == "}":
            idx += 1; break
        else:
            idx += 1

    return idx


def _detect_up_axis(joints: list[_Joint], frames: list[list[float]]) -> str:
    """Hips の位置チャンネルから up 軸を推定する。

    Y の分散/平均絶対値が Z より大きければ Y-up（Mixamo 等）、
    そうでなければ Z-up（古典 BVH）とみなす。
    """
    hips = next((j for j in joints if j.vrm_name == "J_Bip_C_Hips"), None)
    if hips is None or not frames:
        return "y"

    # Hips チャンネルの先頭オフセットを求める
    offset = 0
    for j in joints:
        if j is hips:
            break
        offset += len(j.channels)

    ix = iy = iz = None
    for i, ch in enumerate(hips.channels):
        chu = ch.upper()
        if "XPOS" in chu: ix = offset + i
        elif "YPOS" in chu: iy = offset + i
        elif "ZPOS" in chu: iz = offset + i
    if None in (iy, iz):
        return "y"

    sample = frames[: min(30, len(frames))]
    ys = [abs(f[iy]) for f in sample if len(f) > max(iy, iz)]
    zs = [abs(f[iz]) for f in sample if len(f) > max(iy, iz)]
    if not ys:
        return "y"
    mean_y = sum(ys) / len(ys)
    mean_z = sum(zs) / len(zs)
    return "y" if mean_y >= mean_z else "z"


def load_bvh(path: str, extra_map: dict = None, up_axis: str = "auto") -> BvhMotion:
    """BVH ファイルを読み込む。

    Args:
        path:      BVH ファイルのパス
        extra_map: 追加ボーン名マッピング {"BVH名": "J_Bip_*"}
        up_axis:   'auto' | 'y' | 'z' — 座標軸。auto は Hips 位置から推定

    Returns:
        BvhMotion
    """
    extra = extra_map or {}
    text  = open(path, encoding="utf-8", errors="replace").read()
    lines = [l.rstrip() for l in text.splitlines()]

    joints:     list[_Joint] = []
    frames:     list[list[float]] = []
    frame_time  = 1 / 30.0
    in_motion   = False
    i = 0

    while i < len(lines):
        tok = lines[i].strip().split()
        if not tok: i += 1; continue
        kw = tok[0].upper()

        if kw == "ROOT":
            i = _parse_joint(lines, i, joints, extra)
        elif kw == "MOTION":
            in_motion = True; i += 1
        elif in_motion and kw == "FRAMES:":
            i += 1
        elif in_motion and kw == "FRAME" and "TIME" in lines[i].upper():
            frame_time = float(tok[-1]); i += 1
        elif in_motion:
            try:
                vals = list(map(float, lines[i].split()))
                if vals: frames.append(vals)
            except ValueError:
                pass
            i += 1
        else:
            i += 1

    mapped   = sum(1 for j in joints if j.vrm_name)
    unmapped = [j.name for j in joints
                if j.vrm_name is None and "end" not in j.name.lower()]

    axis = up_axis.lower() if up_axis != "auto" else _detect_up_axis(joints, frames)
    if axis not in ("y", "z"):
        axis = "y"

    print(f"[BVH] {path}")
    print(f"  joints  : {len(joints)} ({mapped} mapped)")
    print(f"  frames  : {len(frames)}  {1/frame_time:.1f}fps  "
          f"{len(frames)*frame_time:.1f}sec")
    print(f"  up_axis : {axis}")
    if unmapped:
        print(f"  unmapped: {unmapped}")

    return BvhMotion(
        _joints=joints,
        frame_time=frame_time,
        frames=frames,
        up_axis=axis,
    )

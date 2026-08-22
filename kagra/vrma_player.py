# kagra/vrma_player.py
"""VRM Animation (.vrma) プレイヤー。

`.vrma` は glTF 2.0 + `VRMC_vrm_animation` のアニメ専用ファイル。
ヒューマノイド骨の回転（と hips の平行移動）を、どの VRM にも載せられる。

Example::
    avatar = kagra.avatar("assets/me.vrm")
    avatar.load_motion("wave", "assets/wave.vrma")
    avatar.play("wave", loop=True)

    # または
    motion = kagra.load_vrma("assets/wave.vrma")
    print(motion.duration, motion.bones)
    avatar.add_motion("wave", motion)
"""
from __future__ import annotations

import base64
import json
import math
import struct
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional
from urllib.parse import unquote


# VRM 1.0 humanoid 名 → VRoid / UniVRM 0.x の J_Bip_* 名
_HUMANOID_TO_VRM: dict[str, str] = {
    "hips": "J_Bip_C_Hips",
    "spine": "J_Bip_C_Spine",
    "chest": "J_Bip_C_Chest",
    "upperChest": "J_Bip_C_UpperChest",
    "neck": "J_Bip_C_Neck",
    "head": "J_Bip_C_Head",
    "leftShoulder": "J_Bip_L_Shoulder",
    "leftUpperArm": "J_Bip_L_UpperArm",
    "leftLowerArm": "J_Bip_L_LowerArm",
    "leftHand": "J_Bip_L_Hand",
    "rightShoulder": "J_Bip_R_Shoulder",
    "rightUpperArm": "J_Bip_R_UpperArm",
    "rightLowerArm": "J_Bip_R_LowerArm",
    "rightHand": "J_Bip_R_Hand",
    "leftUpperLeg": "J_Bip_L_UpperLeg",
    "leftLowerLeg": "J_Bip_L_LowerLeg",
    "leftFoot": "J_Bip_L_Foot",
    "leftToes": "J_Bip_L_ToeBase",
    "rightUpperLeg": "J_Bip_R_UpperLeg",
    "rightLowerLeg": "J_Bip_R_LowerLeg",
    "rightFoot": "J_Bip_R_Foot",
    "rightToes": "J_Bip_R_ToeBase",
    "leftThumbProximal": "J_Bip_L_Thumb1",
    "leftThumbIntermediate": "J_Bip_L_Thumb2",
    "leftThumbDistal": "J_Bip_L_Thumb3",
    "leftIndexProximal": "J_Bip_L_Index1",
    "leftIndexIntermediate": "J_Bip_L_Index2",
    "leftIndexDistal": "J_Bip_L_Index3",
    "leftMiddleProximal": "J_Bip_L_Middle1",
    "leftMiddleIntermediate": "J_Bip_L_Middle2",
    "leftMiddleDistal": "J_Bip_L_Middle3",
    "leftRingProximal": "J_Bip_L_Ring1",
    "leftRingIntermediate": "J_Bip_L_Ring2",
    "leftRingDistal": "J_Bip_L_Ring3",
    "leftLittleProximal": "J_Bip_L_Little1",
    "leftLittleIntermediate": "J_Bip_L_Little2",
    "leftLittleDistal": "J_Bip_L_Little3",
    "rightThumbProximal": "J_Bip_R_Thumb1",
    "rightThumbIntermediate": "J_Bip_R_Thumb2",
    "rightThumbDistal": "J_Bip_R_Thumb3",
    "rightIndexProximal": "J_Bip_R_Index1",
    "rightIndexIntermediate": "J_Bip_R_Index2",
    "rightIndexDistal": "J_Bip_R_Index3",
    "rightMiddleProximal": "J_Bip_R_Middle1",
    "rightMiddleIntermediate": "J_Bip_R_Middle2",
    "rightMiddleDistal": "J_Bip_R_Middle3",
    "rightRingProximal": "J_Bip_R_Ring1",
    "rightRingIntermediate": "J_Bip_R_Ring2",
    "rightRingDistal": "J_Bip_R_Ring3",
    "rightLittleProximal": "J_Bip_R_Little1",
    "rightLittleIntermediate": "J_Bip_R_Little2",
    "rightLittleDistal": "J_Bip_R_Little3",
}

# VRM 1.0 プリセット表情 → VRoid / Alicia 系の候補
_EXPR_CANDIDATES: dict[str, tuple[str, ...]] = {
    "aa": ("aa", "A", "Fcl_MTH_A", "a"),
    "ih": ("ih", "I", "Fcl_MTH_I", "i"),
    "ou": ("ou", "U", "Fcl_MTH_U", "u"),
    "ee": ("ee", "E", "Fcl_MTH_E", "e"),
    "oh": ("oh", "O", "Fcl_MTH_O", "o"),
    "blink": ("blink", "Blink", "Fcl_EYE_Close"),
    "blinkLeft": ("blinkLeft", "Blink_L", "blink_l", "Fcl_EYE_Close_L"),
    "blinkRight": ("blinkRight", "Blink_R", "blink_r", "Fcl_EYE_Close_R"),
    "happy": ("happy", "Joy", "Fcl_ALL_Joy"),
    "angry": ("angry", "Angry", "Fcl_ALL_Angry"),
    "sad": ("sad", "Sorrow", "Fcl_ALL_Sorrow"),
    "relaxed": ("relaxed", "Neutral", "Fcl_ALL_Neutral"),
    "surprised": ("surprised", "Fun", "Fcl_ALL_Fun"),
    "neutral": ("neutral",),
}


def resolve_expression_name(name: str, available: set[str]) -> Optional[str]:
    """VRMA 表情名を、モデルが持つブレンドシェイプ名に解決する。"""
    if name in available:
        return name
    for cand in _EXPR_CANDIDATES.get(name, (name,)):
        if cand in available:
            return cand
    lower = {s.lower(): s for s in available}
    if name.lower() in lower:
        return lower[name.lower()]
    for cand in _EXPR_CANDIDATES.get(name, ()):
        if cand.lower() in lower:
            return lower[cand.lower()]
    return None


def _qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def _qinv(q):
    return (-q[0], -q[1], -q[2], q[3])


def _qnorm(q):
    n = math.sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]) or 1.0
    return (q[0] / n, q[1] / n, q[2] / n, q[3] / n)


def _qdot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]


def _slerp(a, b, t):
    a = _qnorm(a)
    b = _qnorm(b)
    d = _qdot(a, b)
    if d < 0.0:
        b = (-b[0], -b[1], -b[2], -b[3])
        d = -d
    if d > 0.9995:
        return _qnorm((
            a[0] + t * (b[0] - a[0]),
            a[1] + t * (b[1] - a[1]),
            a[2] + t * (b[2] - a[2]),
            a[3] + t * (b[3] - a[3]),
        ))
    d = max(-1.0, min(1.0, d))
    th = math.acos(d)
    s = math.sin(th)
    w0 = math.sin((1.0 - t) * th) / s
    w1 = math.sin(t * th) / s
    return (
        w0 * a[0] + w1 * b[0],
        w0 * a[1] + w1 * b[1],
        w0 * a[2] + w1 * b[2],
        w0 * a[3] + w1 * b[3],
    )


def _lerp(a, b, t):
    return a + (b - a) * t


def _node_index(entry) -> Optional[int]:
    """`{"node": 3}` または草案の素の整数を受け付ける。"""
    if entry is None:
        return None
    if isinstance(entry, int):
        return entry
    if isinstance(entry, dict) and "node" in entry:
        return int(entry["node"])
    return None


# ── glTF I/O ──────────────────────────────────────────────────

_COMP_BYTES = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
_COMP_COUNT = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def _pad4(n: int) -> int:
    return (4 - (n % 4)) % 4


def _read_glb_or_gltf(path: str | Path) -> tuple[dict, bytes]:
    path = Path(path)
    data = path.read_bytes()
    if data[:4] == b"glTF":
        return _parse_glb(data)
    gltf = json.loads(data.decode("utf-8"))
    blob = _load_gltf_buffers(gltf, path.parent)
    return gltf, blob


def _parse_glb(data: bytes) -> tuple[dict, bytes]:
    if len(data) < 12:
        raise ValueError("truncated GLB")
    magic, version, _length = struct.unpack_from("<4sII", data, 0)
    if magic != b"glTF" or version != 2:
        raise ValueError(f"unsupported GLB header {magic!r} v{version}")
    offset = 12
    gltf = None
    blob = b""
    while offset + 8 <= len(data):
        cl, ct = struct.unpack_from("<II", data, offset)
        chunk = data[offset + 8: offset + 8 + cl]
        offset += 8 + cl
        if ct == 0x4E4F534A:
            gltf = json.loads(chunk.decode("utf-8").rstrip("\x00"))
        elif ct == 0x004E4942:
            blob = chunk
    if gltf is None:
        raise ValueError("GLB has no JSON chunk")
    return gltf, blob


def _load_gltf_buffers(gltf: dict, base: Path) -> bytes:
    """最初の buffer を 1 本の blob として返す（アニメ用途で十分）。"""
    buffers = gltf.get("buffers") or []
    if not buffers:
        return b""
    parts: list[bytes] = []
    for buf in buffers:
        uri = buf.get("uri")
        if not uri:
            parts.append(b"")
            continue
        if uri.startswith("data:"):
            _, _, b64 = uri.partition(",")
            parts.append(base64.b64decode(b64))
            continue
        parts.append((base / unquote(uri)).read_bytes())
    return b"".join(parts)


def read_vrma_gltf(path: str | Path) -> tuple[dict, bytes]:
    """VRMA / glTF / GLB を (json, bin) で返す。"""
    return _read_glb_or_gltf(path)


def is_vrma(path: str | Path) -> bool:
    """拡張子または `VRMC_vrm_animation` の有無で判定する。"""
    p = Path(path)
    if p.suffix.lower() == ".vrma":
        return True
    try:
        gltf, _ = _read_glb_or_gltf(p)
    except Exception:
        return False
    return "VRMC_vrm_animation" in (gltf.get("extensions") or {})


def _vrma_ext(gltf: dict) -> dict:
    ext = (gltf.get("extensions") or {}).get("VRMC_vrm_animation")
    if not isinstance(ext, dict):
        raise ValueError("not a VRMA file (missing extensions.VRMC_vrm_animation)")
    return ext


def _read_accessor(gltf: dict, blob: bytes, acc_idx: int) -> list:
    acc = gltf["accessors"][acc_idx]
    if "sparse" in acc:
        raise ValueError("sparse accessors are not supported in VRMA")
    ctype = acc["componentType"]
    typ = acc["type"]
    count = acc["count"]
    ncomp = _COMP_COUNT[typ]
    csize = _COMP_BYTES[ctype]
    bv = gltf["bufferViews"][acc["bufferView"]]
    offset = bv.get("byteOffset", 0) + acc.get("byteOffset", 0)
    stride = bv.get("byteStride") or (csize * ncomp)
    fmt = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}[ctype]
    out = []
    for i in range(count):
        o = offset + i * stride
        vals = struct.unpack_from("<" + fmt * ncomp, blob, o)
        out.append(vals[0] if ncomp == 1 else list(vals))
    return out


def _sample_scalar(times: list[float], values: list, t: float, interp: str):
    if not times:
        return values[0] if values else 0.0
    if t <= times[0]:
        return values[0]
    if t >= times[-1]:
        return values[-1]
    i = 1
    while i < len(times) and times[i] < t:
        i += 1
    t0, t1 = times[i - 1], times[i]
    u = 0.0 if t1 <= t0 else (t - t0) / (t1 - t0)
    if interp == "STEP":
        return values[i - 1]
    if interp == "CUBICSPLINE":
        # values: [in, v, out] per key. VEC は線形で十分。
        v0 = values[(i - 1) * 3 + 1]
        v1 = values[i * 3 + 1]
        if isinstance(v0, list):
            return [_lerp(v0[j], v1[j], u) for j in range(len(v0))]
        b0 = values[(i - 1) * 3 + 2]
        a1 = values[i * 3 + 0]
        dt = t1 - t0
        s = u
        s2 = s * s
        s3 = s2 * s
        return (
            (2 * s3 - 3 * s2 + 1) * v0
            + (s3 - 2 * s2 + s) * (b0 * dt)
            + (-2 * s3 + 3 * s2) * v1
            + (s3 - s2) * (a1 * dt)
        )
    a, b = values[i - 1], values[i]
    if isinstance(a, list):
        return [_lerp(a[j], b[j], u) for j in range(len(a))]
    return _lerp(a, b, u)


def _sample_quat(times: list[float], values: list, t: float, interp: str):
    if t <= times[0]:
        q = values[0] if interp != "CUBICSPLINE" else values[1]
        return _qnorm(q)
    if t >= times[-1]:
        q = values[-1] if interp != "CUBICSPLINE" else values[-2]
        return _qnorm(q)
    i = 1
    while i < len(times) and times[i] < t:
        i += 1
    t0, t1 = times[i - 1], times[i]
    u = 0.0 if t1 <= t0 else (t - t0) / (t1 - t0)
    if interp == "STEP":
        return _qnorm(values[i - 1])
    if interp == "CUBICSPLINE":
        # 接線を無視して値だけ slerp（十分）
        a = values[(i - 1) * 3 + 1]
        b = values[i * 3 + 1]
        return _slerp(a, b, u)
    return _slerp(values[i - 1], values[i], u)


@dataclass
class _Track:
    node: int
    path: str  # rotation | translation
    times: list[float]
    values: list
    interp: str


@dataclass
class VrmaMotion:
    """パース済み VRM Animation。

    ``to_clip()`` は BvhMotion / FbxMotion と同じ
    ``[(bones, dt, root_pos, expressions), ...]`` を返す。
    表情 dict は任意。``_Animator`` は先頭 3 要素しか見ない。
    """

    spec_version: str
    bones: dict[str, int]          # J_Bip_* → node
    expressions: dict[str, int]    # preset/custom 名 → node
    look_at_node: Optional[int]
    _rest_rot: dict[int, tuple]
    _rest_pos: dict[int, tuple]
    _tracks: list[_Track]
    _times: list[float]
    _cache: Optional[list] = field(default=None, repr=False)
    source: str = ""

    @property
    def duration(self) -> float:
        return float(self._times[-1]) if self._times else 0.0

    @property
    def fps(self) -> float:
        if len(self._times) < 2:
            return 30.0
        dt = self._times[1] - self._times[0]
        return 1.0 / dt if dt > 1e-8 else 30.0

    @property
    def frame_time(self) -> float:
        return 1.0 / self.fps if self.fps else 1.0 / 30.0

    def _sample_at(self, t: float) -> tuple[dict, tuple, dict]:
        rots: dict[int, tuple] = dict(self._rest_rot)
        poss: dict[int, tuple] = dict(self._rest_pos)
        for tr in self._tracks:
            if tr.path == "rotation":
                rots[tr.node] = _sample_quat(tr.times, tr.values, t, tr.interp)
            elif tr.path == "translation":
                v = _sample_scalar(tr.times, tr.values, t, tr.interp)
                if isinstance(v, list) and len(v) >= 3:
                    poss[tr.node] = (float(v[0]), float(v[1]), float(v[2]))
        bones = {}
        for vrm_name, node in self.bones.items():
            rest = self._rest_rot.get(node, (0.0, 0.0, 0.0, 1.0))
            anim = rots.get(node, rest)
            # NormalizedLocalRotation: inv(TPose_anim) * Pose_anim
            bones[vrm_name] = list(_qnorm(_qmul(_qinv(rest), anim)))
        hips_node = self.bones.get("J_Bip_C_Hips")
        root = (0.0, 0.0, 0.0)
        if hips_node is not None:
            rest_p = self._rest_pos.get(hips_node, (0.0, 0.0, 0.0))
            pos = poss.get(hips_node, rest_p)
            root = (pos[0] - rest_p[0], pos[1] - rest_p[1], pos[2] - rest_p[2])
        exprs = {}
        for name, node in self.expressions.items():
            p = poss.get(node, self._rest_pos.get(node, (0.0, 0.0, 0.0)))
            exprs[name] = max(0.0, min(1.0, float(p[0])))
        return bones, root, exprs

    def to_clip(self) -> list:
        if self._cache is not None:
            return self._cache
        if not self._times:
            self._cache = []
            return self._cache
        clip = []
        for i, t in enumerate(self._times):
            if i + 1 < len(self._times):
                dt = max(1e-4, self._times[i + 1] - t)
            else:
                dt = self.frame_time
            bones, root, exprs = self._sample_at(t)
            clip.append((bones, dt, root, exprs))
        self._cache = clip
        return self._cache


def _collect_times(tracks: list[_Track], *, sample_fps: float) -> list[float]:
    raw: set[float] = set()
    for tr in tracks:
        raw.update(float(x) for x in tr.times)
    if not raw:
        return [0.0]
    lo, hi = min(raw), max(raw)
    if hi <= lo:
        return [lo]
    step = 1.0 / max(1.0, sample_fps)
    n = int(round((hi - lo) / step)) + 1
    n = max(2, min(n, 6000))
    return [lo + (hi - lo) * i / (n - 1) for i in range(n)]


def _node_rest_rot(node: dict) -> tuple:
    r = node.get("rotation") or [0.0, 0.0, 0.0, 1.0]
    return _qnorm((float(r[0]), float(r[1]), float(r[2]), float(r[3])))


def _node_rest_pos(node: dict) -> tuple:
    t = node.get("translation") or [0.0, 0.0, 0.0]
    return (float(t[0]), float(t[1]), float(t[2]))


def load_vrma(path: str | Path, *, sample_fps: float = 30.0) -> VrmaMotion:
    """`.vrma` / glTF / GLB を読み、ヒューマノイド向けクリップにする。"""
    path = Path(path)
    gltf, blob = _read_glb_or_gltf(path)
    ext = _vrma_ext(gltf)
    spec = str(ext.get("specVersion") or "1.0")

    bones: dict[str, int] = {}
    human = ((ext.get("humanoid") or {}).get("humanBones") or {})
    for hum_name, entry in human.items():
        idx = _node_index(entry)
        if idx is None:
            continue
        vrm = _HUMANOID_TO_VRM.get(hum_name)
        if vrm:
            bones[vrm] = idx

    expressions: dict[str, int] = {}
    expr_root = ext.get("expressions") or {}
    for group in ("preset", "custom"):
        block = expr_root.get(group) or {}
        for name, entry in block.items():
            idx = _node_index(entry)
            if idx is not None:
                expressions[name] = idx

    look_at_node = _node_index(ext.get("lookAt"))

    nodes = gltf.get("nodes") or []
    rest_rot = {i: _node_rest_rot(n) for i, n in enumerate(nodes)}
    rest_pos = {i: _node_rest_pos(n) for i, n in enumerate(nodes)}

    tracks: list[_Track] = []
    animations = gltf.get("animations") or []
    if animations:
        anim = animations[0]
        samplers = anim.get("samplers") or []
        for ch in anim.get("channels") or []:
            target = ch.get("target") or {}
            node = target.get("node")
            path = target.get("path")
            if node is None or path not in ("rotation", "translation"):
                continue
            samp = samplers[ch["sampler"]]
            times = [float(x) if not isinstance(x, list) else float(x[0])
                     for x in _read_accessor(gltf, blob, samp["input"])]
            values = _read_accessor(gltf, blob, samp["output"])
            interp = str(samp.get("interpolation") or "LINEAR")
            tracks.append(_Track(int(node), path, times, values, interp))

    times = _collect_times(tracks, sample_fps=sample_fps)
    mapped = len(bones)
    print(f"[VRMA] {path}")
    print(f"  spec    : {spec}")
    print(f"  bones   : {mapped}")
    print(f"  exprs   : {list(expressions)}")
    print(f"  frames  : {len(times)}  {sample_fps:.0f}fps  {times[-1]:.2f}sec")

    return VrmaMotion(
        spec_version=spec,
        bones=bones,
        expressions=expressions,
        look_at_node=look_at_node,
        _rest_rot=rest_rot,
        _rest_pos=rest_pos,
        _tracks=tracks,
        _times=times,
        source=str(path),
    )


def write_synthetic_vrma(
    path: str | Path,
    *,
    frames: int = 16,
    duration: float = 1.0,
) -> Path:
    """テスト用の最小 VRMA (GLB) を書く。腕振り + `aa` 口形。"""
    path = Path(path)
    frames = max(2, int(frames))
    duration = max(1e-3, float(duration))
    times = [duration * i / (frames - 1) for i in range(frames)]

    def zrot(rad: float) -> tuple:
        h = rad * 0.5
        return (0.0, 0.0, math.sin(h), math.cos(h))

    hips_t, left_r, right_r, aa_t = [], [], [], []
    for t in times:
        ph = t / duration * 2.0 * math.pi
        hips_t.extend((0.0, 1.0 + 0.02 * math.sin(ph), 0.0))
        left_r.extend(zrot(0.55 * math.sin(ph)))
        right_r.extend(zrot(-0.55 * math.sin(ph)))
        w = max(0.0, math.sin(ph))
        aa_t.extend((w, 0.0, 0.0))

    blob = b"".join((
        struct.pack(f"<{len(times)}f", *times),
        struct.pack(f"<{len(hips_t)}f", *hips_t),
        struct.pack(f"<{len(left_r)}f", *left_r),
        struct.pack(f"<{len(right_r)}f", *right_r),
        struct.pack(f"<{len(aa_t)}f", *aa_t),
    ))

    def acc(offset, count, typ, mn=None, mx=None):
        a = {
            "bufferView": 0,
            "byteOffset": offset,
            "componentType": 5126,
            "count": count,
            "type": typ,
        }
        if mn is not None:
            a["min"] = mn
            a["max"] = mx
        return a

    n = frames
    o_time, o_hips = 0, n * 4
    o_left = o_hips + n * 12
    o_right = o_left + n * 16
    o_aa = o_right + n * 16
    gltf = {
        "asset": {"version": "2.0", "generator": "kagra.write_synthetic_vrma"},
        "extensionsUsed": ["VRMC_vrm_animation"],
        "extensions": {
            "VRMC_vrm_animation": {
                "specVersion": "1.0",
                "humanoid": {
                    "humanBones": {
                        "hips": {"node": 0},
                        "spine": {"node": 1},
                        "leftUpperArm": {"node": 2},
                        "rightUpperArm": {"node": 3},
                    }
                },
                "expressions": {"preset": {"aa": {"node": 4}}},
            }
        },
        "nodes": [
            {"name": "hips", "translation": [0.0, 1.0, 0.0], "children": [1, 2, 3]},
            {"name": "spine"},
            {"name": "leftUpperArm"},
            {"name": "rightUpperArm"},
            {"name": "aa"},
        ],
        "animations": [{
            "name": "synthetic_wave",
            "channels": [
                {"sampler": 0, "target": {"node": 0, "path": "translation"}},
                {"sampler": 1, "target": {"node": 2, "path": "rotation"}},
                {"sampler": 2, "target": {"node": 3, "path": "rotation"}},
                {"sampler": 3, "target": {"node": 4, "path": "translation"}},
            ],
            "samplers": [
                {"input": 0, "output": 1, "interpolation": "LINEAR"},
                {"input": 0, "output": 2, "interpolation": "LINEAR"},
                {"input": 0, "output": 3, "interpolation": "LINEAR"},
                {"input": 0, "output": 4, "interpolation": "LINEAR"},
            ],
        }],
        "accessors": [
            acc(o_time, n, "SCALAR", [times[0]], [times[-1]]),
            acc(o_hips, n, "VEC3"),
            acc(o_left, n, "VEC4"),
            acc(o_right, n, "VEC4"),
            acc(o_aa, n, "VEC3"),
        ],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": len(blob)}],
        "buffers": [{"byteLength": len(blob)}],
    }

    json_bytes = json.dumps(gltf, separators=(",", ":")).encode("utf-8")
    json_bytes += b" " * _pad4(len(json_bytes))
    bin_bytes = blob + b"\x00" * _pad4(len(blob))
    total = 12 + 8 + len(json_bytes) + 8 + len(bin_bytes)
    header = struct.pack("<4sII", b"glTF", 2, total)
    json_chunk = struct.pack("<II", len(json_bytes), 0x4E4F534A) + json_bytes
    bin_chunk = struct.pack("<II", len(bin_bytes), 0x004E4942) + bin_bytes
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(header + json_chunk + bin_chunk)
    return path

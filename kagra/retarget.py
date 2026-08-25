"""Mixamo / BVH local deltas → VRoid ``bind * delta`` space.

Mixamo rest is T-pose with Y-along-bone joint orients. VRoid rest is T or A
and uses a different bone roll (local X often lifts forward, local Z hangs).
Copying Mixamo local deltas onto ``J_Bip_*`` therefore folds the arms into a
carry pose even when both skeletons are T-pose.

World-space transfer (same as VRMA NormalizedLocalRotation, but with a
**source** rest world, not dest-only conjugate):

    N = W_src * delta_src * inv(W_src)
    delta_dst = inv(W_dst) * N * W_dst

Identity Mixamo delta stays dest rest (A-pose stays A-pose). Mixamo hang
becomes a world hang, expressed in dest local axes.
"""
from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Iterable

_ID = (0.0, 0.0, 0.0, 1.0)


def qnorm(q) -> tuple:
    n = math.sqrt(q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]) or 1.0
    return (q[0] / n, q[1] / n, q[2] / n, q[3] / n)


def qinv(q) -> tuple:
    q = qnorm(q)
    return (-q[0], -q[1], -q[2], q[3])


def qmul(a, b) -> tuple:
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return qnorm((
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ))


def q_axis_angle(ax: float, ay: float, az: float, radians: float) -> tuple:
    n = math.sqrt(ax * ax + ay * ay + az * az) or 1.0
    s = math.sin(radians * 0.5)
    return qnorm((ax / n * s, ay / n * s, az / n * s, math.cos(radians * 0.5)))


def rotate_vec(q, v) -> tuple:
    """Rotate vector ``v`` by unit quaternion ``q`` (xyzw)."""
    q = qnorm(q)
    x, y, z = v
    qx, qy, qz, qw = q
    tx = 2.0 * (qy * z - qz * y)
    ty = 2.0 * (qz * x - qx * z)
    tz = 2.0 * (qx * y - qy * x)
    return (
        x + qw * tx + (qy * tz - qz * ty),
        y + qw * ty + (qz * tx - qx * tz),
        z + qw * tz + (qx * ty - qy * tx),
    )


def quat_from_axes(x_axis, y_axis, z_axis) -> tuple:
    """Rotation that maps local XYZ to the given world axes (columns)."""
    m00, m10, m20 = x_axis
    m01, m11, m21 = y_axis
    m02, m12, m22 = z_axis
    trace = m00 + m11 + m22
    if trace > 0.0:
        s = math.sqrt(trace + 1.0) * 2.0
        return qnorm(((m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s))
    if m00 > m11 and m00 > m22:
        s = math.sqrt(1.0 + m00 - m11 - m22) * 2.0
        return qnorm((0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s))
    if m11 > m22:
        s = math.sqrt(1.0 + m11 - m00 - m22) * 2.0
        return qnorm(((m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s))
    s = math.sqrt(1.0 + m22 - m00 - m11) * 2.0
    return qnorm(((m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s))


def mixamo_tpose_worlds() -> dict[str, tuple]:
    """Canonical Mixamo T-pose rest worlds (Y-along-bone, local X = world +Z).

    Left upper arm points +X, right −X. Hang around Mixamo local X is a world
    Z rotation (drop in the coronal plane). Used when an FBX did not export
    bind worlds (tests / old wheels).
    """
    # Left: local X = +Z (fwd), local Y = +X (along), local Z = +Y
    left = quat_from_axes((0.0, 0.0, 1.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0))
    # Right: local X = +Z, local Y = −X (along), local Z = −Y
    right = quat_from_axes((0.0, 0.0, 1.0), (-1.0, 0.0, 0.0), (0.0, -1.0, 0.0))
    hips = _ID
    return {
        "J_Bip_C_Hips": hips,
        "J_Bip_C_Spine": hips,
        "J_Bip_C_Chest": hips,
        "J_Bip_C_UpperChest": hips,
        "J_Bip_C_Neck": hips,
        "J_Bip_C_Head": hips,
        "J_Bip_L_Shoulder": left,
        "J_Bip_L_UpperArm": left,
        "J_Bip_L_LowerArm": left,
        "J_Bip_L_Hand": left,
        "J_Bip_R_Shoulder": right,
        "J_Bip_R_UpperArm": right,
        "J_Bip_R_LowerArm": right,
        "J_Bip_R_Hand": right,
    }


def vroid_tpose_worlds(*, rolled: bool = True) -> dict[str, tuple]:
    """VRoid T-pose rest worlds for GPU-free tests.

    Bone direction is still ±X (true T-pose). ``rolled=True`` matches the
    documented VRoid arm axes: local Y along the bone, local X = world +Y
    (forward-lift if copied from Mixamo local X). That roll is why raw
    Mixamo hang folds Emma's arms forward.
    """
    if rolled:
        # local X = +Y, local Y = +X (along), local Z = −Z
        left = quat_from_axes((0.0, 1.0, 0.0), (1.0, 0.0, 0.0), (0.0, 0.0, -1.0))
        right = quat_from_axes((0.0, 1.0, 0.0), (-1.0, 0.0, 0.0), (0.0, 0.0, 1.0))
    else:
        left = quat_from_axes((1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0))
        right = quat_from_axes((-1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, -1.0))
    hips = _ID
    return {
        "J_Bip_C_Hips": hips,
        "J_Bip_L_UpperArm": left,
        "J_Bip_R_UpperArm": right,
        "J_Bip_L_LowerArm": left,
        "J_Bip_R_LowerArm": right,
    }


def vroid_apose_worlds(tpose: dict | None = None, drop: float = 0.7) -> dict[str, tuple]:
    """A-pose VRoid: T-pose worlds with an extra world-Z drop on the arms."""
    base = dict(tpose or vroid_tpose_worlds())
    hang_l = q_axis_angle(0.0, 0.0, 1.0, -abs(drop))
    hang_r = q_axis_angle(0.0, 0.0, 1.0, abs(drop))
    out = dict(base)
    for name, extra in (
        ("J_Bip_L_UpperArm", hang_l),
        ("J_Bip_L_LowerArm", hang_l),
        ("J_Bip_R_UpperArm", hang_r),
        ("J_Bip_R_LowerArm", hang_r),
    ):
        if name in base:
            out[name] = qmul(extra, base[name])
    return out


def retarget_delta(delta_src, world_src, world_dst) -> list:
    """Mixamo/BVH local delta → dest local delta for ``bind * delta``."""
    ws = qnorm(world_src if world_src is not None else _ID)
    wd = qnorm(world_dst if world_dst is not None else _ID)
    d = qnorm(delta_src)
    # NormalizedLocalRotation N = W_src * delta * inv(W_src)
    n = qmul(qmul(ws, d), qinv(ws))
    # dest_delta_from_normalized: inv(W_dst) * N * W_dst
    return list(qmul(qmul(qinv(wd), n), wd))


def retarget_clip(clip: list, src_worlds: dict, dst_worlds: dict) -> list:
    """Rewrite every bone quat in a ``to_clip()`` list."""
    src_worlds = src_worlds or {}
    dst_worlds = dst_worlds or {}
    if not dst_worlds:
        return clip
    out = []
    for frame in clip:
        bones = frame[0]
        if not isinstance(bones, dict):
            out.append(frame)
            continue
        retargeted = {}
        for name, q in bones.items():
            ws = src_worlds.get(name)
            wd = dst_worlds.get(name)
            if wd is None:
                retargeted[name] = list(q) if not isinstance(q, list) else q
                continue
            retargeted[name] = retarget_delta(q, ws or _ID, wd)
        out.append((retargeted,) + tuple(frame[1:]))
    return out


def bone_dir(world_q, local_axis=(0.0, 1.0, 0.0)) -> tuple:
    """World direction of a rest bone axis (Mixamo/VRoid Y-along default)."""
    return rotate_vec(world_q, local_axis)


def animated_bone_dir(world_rest, delta_local, local_axis=(0.0, 1.0, 0.0)) -> tuple:
    """World direction after ``bind * delta`` (parent at rest)."""
    world_anim = qmul(world_rest, delta_local)
    return rotate_vec(world_anim, local_axis)


def folded_forward(rest_dir, anim_dir, *, forward_axis=(0.0, 0.0, 1.0), thresh: float = 0.55) -> bool:
    """True when the arm points ~90° into world +Z instead of hanging/resting."""
    fwd = abs(anim_dir[0] * forward_axis[0] + anim_dir[1] * forward_axis[1] + anim_dir[2] * forward_axis[2])
    along = abs(rest_dir[0] * anim_dir[0] + rest_dir[1] * anim_dir[1] + rest_dir[2] * anim_dir[2])
    return fwd >= thresh and along < 0.55


def mixamo_hang_delta(radians: float = math.pi / 2, *, side: str = "L") -> tuple:
    """90° Mixamo local-X hang (world Z drop for the canonical T-pose)."""
    sign = 1.0 if side.upper().startswith("L") else -1.0
    return q_axis_angle(sign, 0.0, 0.0, radians)


def write_synthetic_mixamo_hang(path: str | Path) -> Path:
    """Tiny Mixamo-layout hang clip (JSON). No FBX binary."""
    path = Path(path)
    src = mixamo_tpose_worlds()
    hang_l = mixamo_hang_delta(math.pi / 2, side="L")
    hang_r = mixamo_hang_delta(math.pi / 2, side="R")
    ident = list(_ID)
    data = {
        "format": "kagra.synthetic_mixamo_clip/v1",
        "comment": (
            "Mixamo T-pose Y-along-bone. Frame 1 hangs 90deg around local X "
            "(world Z). Raw bind*delta on rolled VRoid T-pose folds +Z."
        ),
        "src_worlds": {k: list(v) for k, v in src.items() if "UpperArm" in k or "LowerArm" in k},
        "frame_time": 0.033333,
        "frames": [
            {
                "J_Bip_L_UpperArm": ident,
                "J_Bip_R_UpperArm": ident,
                "J_Bip_L_LowerArm": ident,
                "J_Bip_R_LowerArm": ident,
            },
            {
                "J_Bip_L_UpperArm": list(hang_l),
                "J_Bip_R_UpperArm": list(hang_r),
                "J_Bip_L_LowerArm": ident,
                "J_Bip_R_LowerArm": ident,
            },
        ],
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    return path


def load_synthetic_mixamo_clip(path: str | Path) -> tuple[list, dict]:
    """``(clip, src_worlds)`` in ``to_clip()`` shape. GPU-free."""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    dt = float(data.get("frame_time") or 1.0 / 30.0)
    src = {k: tuple(v) for k, v in (data.get("src_worlds") or {}).items()}
    clip = []
    for bones in data.get("frames") or []:
        mapped = {n: tuple(q) for n, q in bones.items()}
        clip.append((mapped, dt, (0.0, 0.0, 0.0)))
    return clip, src


def clip_from_frames(frames: Iterable[dict], frame_time: float = 1.0 / 30.0) -> list:
    return [({k: tuple(v) for k, v in bones.items()}, frame_time, (0.0, 0.0, 0.0)) for bones in frames]

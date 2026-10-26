#!/usr/bin/env python3
"""Bake one frame of a glTF animation clip into the mesh (CPU skinning, once, offline).

The survivor swarm draws static props, and a Kenney mini-character in its rest pose
has its arms stretched out sideways (T-pose), which reads as "holding something
sideways". `pack_glb.py --clip holding-both-shoot --time 0.2` calls `bake_pose` to
turn the clip's frame into plain POSITION / NORMAL data before the skin is dropped.

Pure python + numpy. Handles one skin, linear/step sampler, TRS channels.
"""
import struct

import numpy as np

COMP_FMT = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}
NCOMP = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def read_acc(j, bin_, ai):
    a = j["accessors"][ai]
    v = j["bufferViews"][a["bufferView"]]
    n = NCOMP[a["type"]]
    fmt = COMP_FMT[a["componentType"]]
    size = struct.calcsize(fmt)
    stride = v.get("byteStride", size * n)
    start = v.get("byteOffset", 0) + a.get("byteOffset", 0)
    out = np.empty((a["count"], n), dtype=np.float64)
    for i in range(a["count"]):
        out[i] = struct.unpack_from("<" + fmt * n, bin_, start + i * stride)
    return out


def trs_matrix(t, r, s):
    x, y, z, w = r
    m = np.eye(4)
    m[:3, :3] = np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
        ]
    ) * np.array(s)[None, :]
    m[:3, 3] = t
    return m


def slerp(a, b, t):
    a = np.asarray(a, float)
    b = np.asarray(b, float)
    d = float(np.dot(a, b))
    if d < 0:
        b, d = -b, -d
    if d > 0.9995:
        q = a + (b - a) * t
    else:
        th = np.arccos(d)
        q = (np.sin((1 - t) * th) * a + np.sin(t * th) * b) / np.sin(th)
    return q / np.linalg.norm(q)


def sample(j, bin_, anim, t):
    """node index -> {path: value} at time t."""
    out = {}
    for ch in anim["channels"]:
        smp = anim["samplers"][ch["sampler"]]
        times = read_acc(j, bin_, smp["input"])[:, 0]
        vals = read_acc(j, bin_, smp["output"])
        path = ch["target"]["path"]
        tt = min(max(t, times[0]), times[-1])
        k = int(np.searchsorted(times, tt, side="right")) - 1
        k = min(max(k, 0), len(times) - 2) if len(times) > 1 else 0
        if len(times) == 1:
            v = vals[0]
        else:
            f = (tt - times[k]) / max(times[k + 1] - times[k], 1e-9)
            if smp.get("interpolation") == "STEP":
                f = 0.0
            if path == "rotation":
                v = slerp(vals[k], vals[k + 1], f)
            else:
                v = vals[k] * (1 - f) + vals[k + 1] * f
        out.setdefault(ch["target"]["node"], {})[path] = v
    return out


def bake_pose(j, bin_, clip, t):
    """Return {accessor_index: float32 bytes} for POSITION and NORMAL, skinned at clip@t."""
    anim = next(a for a in j["animations"] if a["name"] == clip)
    nodes = j["nodes"]
    pose = sample(j, bin_, anim, t)
    parent = {}
    for i, n in enumerate(nodes):
        for c in n.get("children", []):
            parent[c] = i
    local = []
    for i, n in enumerate(nodes):
        tr = n.get("translation", [0, 0, 0])
        ro = n.get("rotation", [0, 0, 0, 1])
        sc = n.get("scale", [1, 1, 1])
        p = pose.get(i, {})
        local.append(trs_matrix(p.get("translation", tr), p.get("rotation", ro), p.get("scale", sc)))
    glob = {}

    def g(i):
        if i not in glob:
            glob[i] = g(parent[i]) @ local[i] if i in parent else local[i]
        return glob[i]

    # rest-pose globals for the mesh node (the renderer applies this one itself)
    def rest_global(i):
        n = nodes[i]
        m = trs_matrix(n.get("translation", [0, 0, 0]), n.get("rotation", [0, 0, 0, 1]), n.get("scale", [1, 1, 1]))
        return rest_global(parent[i]) @ m if i in parent else m

    out = {}
    for ni, n in enumerate(nodes):
        if "mesh" not in n:
            continue
        skin = j["skins"][n["skin"]] if "skin" in n else None
        if skin is None:
            continue
        ibm = read_acc(j, bin_, skin["inverseBindMatrices"]).reshape(-1, 4, 4).transpose(0, 2, 1)
        jm = [g(joint) @ ibm[k] for k, joint in enumerate(skin["joints"])]
        inv_mesh = np.linalg.inv(rest_global(ni))
        for p in j["meshes"][n["mesh"]]["primitives"]:
            at = p["attributes"]
            pos = read_acc(j, bin_, at["POSITION"])
            nor = read_acc(j, bin_, at["NORMAL"])
            jo = read_acc(j, bin_, at["JOINTS_0"]).astype(int)
            we = read_acc(j, bin_, at["WEIGHTS_0"])
            P = np.zeros_like(pos)
            N = np.zeros_like(nor)
            for vi in range(len(pos)):
                m = sum(we[vi, k] * jm[jo[vi, k]] for k in range(4) if we[vi, k] > 0)
                m = inv_mesh @ m
                P[vi] = (m @ np.append(pos[vi], 1.0))[:3]
                nn = m[:3, :3] @ nor[vi]
                N[vi] = nn / max(np.linalg.norm(nn), 1e-9)
            out[at["POSITION"]] = P.astype("<f4")
            out[at["NORMAL"]] = N.astype("<f4")
    return out

# kagra/vrm_loader.py
"""VRM ローダー + CPU スキニング - KAGRA Phase 11 (修正版: rig_id 保持)"""

from __future__ import annotations
import struct, json, os, tempfile, math
from dataclasses import dataclass, field
from typing import Optional


# ── 行列ユーティリティ（列優先 4x4）─────────────────────────────

def _mat4_identity():
    return [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]

def _mat4_mul(a, b):
    out = [0.0]*16
    for row in range(4):
        for col in range(4):
            s = 0.0
            for k in range(4):
                s += a[row + k*4] * b[k + col*4]
            out[row + col*4] = s
    return out

def _mat4_from_trs(t, r, s):
    tx,ty,tz = t
    rx,ry,rz,rw = r
    sx,sy,sz = s
    x2,y2,z2 = rx*2,ry*2,rz*2
    xx,yy,zz = rx*x2, ry*y2, rz*z2
    xy,xz,yz = rx*y2, rx*z2, ry*z2
    wx,wy,wz = rw*x2, rw*y2, rw*z2
    return [
        (1-(yy+zz))*sx,  (xy+wz)*sx,     (xz-wy)*sx,     0,
        (xy-wz)*sy,      (1-(xx+zz))*sy, (yz+wx)*sy,     0,
        (xz+wy)*sz,      (yz-wx)*sz,     (1-(xx+yy))*sz, 0,
        tx, ty, tz, 1,
    ]

def _transform_point(m, x, y, z):
    return (
        m[0]*x + m[4]*y + m[8]*z  + m[12],
        m[1]*x + m[5]*y + m[9]*z  + m[13],
        m[2]*x + m[6]*y + m[10]*z + m[14],
    )

def _transform_normal(m, x, y, z):
    nx = m[0]*x + m[4]*y + m[8]*z
    ny = m[1]*x + m[5]*y + m[9]*z
    nz = m[2]*x + m[6]*y + m[10]*z
    l = math.sqrt(nx*nx + ny*ny + nz*nz)
    if l > 1e-8:
        nx /= l; ny /= l; nz /= l
    return nx, ny, nz


# ── データクラス ─────────────────────────────────────────────────

@dataclass
class VrmBone:
    name:     str
    index:    int
    parent:   Optional[int] = None
    children: list = field(default_factory=list)
    t:        list = field(default_factory=lambda: [0,0,0])
    r:        list = field(default_factory=lambda: [0,0,0,1])
    s:        list = field(default_factory=lambda: [1,1,1])
    world_mat: list = field(default_factory=_mat4_identity)
    local_rot: list = field(default_factory=lambda: [0,0,0,1])


@dataclass
class VrmPrimitive:
    name:        str
    texture_id:  int
    positions:   list
    normals:     list
    uvs:         list
    joints:      list
    weights:     list
    indices:     list
    skin_index:  int
    _cached_verts: list = field(default_factory=list)


@dataclass
class VrmSkin:
    joints: list[int]
    inv_bind: list[list[float]]


class VrmModel:
    def __init__(self):
        self.primitives:  list[VrmPrimitive] = []
        self.bones:       dict[str, VrmBone] = {}
        self.bone_list:   list[VrmBone]      = []
        self.skins:       list[VrmSkin]      = []
        self.path = ""
        self._dirty = True
        self.rig_id = None          # ← 追加: ロードしたリグのIDを保存

    @classmethod
    def load(cls, vrm_path: str, kagra_engine, debug: bool = False) -> "VrmModel":
        model = cls()
        model.path = vrm_path

        with open(vrm_path, "rb") as f:
            data = f.read()

        if data[:4] != b'glTF':
            raise ValueError(f"glTF ではありません: {vrm_path}")

        offset = 12
        gltf = bin_data = None
        while offset < len(data):
            cl = struct.unpack_from("<I", data, offset)[0]
            ct = struct.unpack_from("<I", data, offset+4)[0]
            cd = data[offset+8:offset+8+cl]
            if ct == 0x4E4F534A:
                gltf = json.loads(cd.decode("utf-8").rstrip("\x00"))
            elif ct == 0x004E4942:
                bin_data = cd
            offset += 8 + cl

        if not gltf or not bin_data:
            raise ValueError("JSON/BIN チャンクが見つかりません")

        images        = gltf.get("images", [])
        textures_gltf = gltf.get("textures", [])
        buffer_views  = gltf.get("bufferViews", [])
        accessors     = gltf.get("accessors", [])
        materials     = gltf.get("materials", [])
        meshes_gltf   = gltf.get("meshes", [])
        nodes         = gltf.get("nodes", [])
        skins_gltf    = gltf.get("skins", [])

        def read_acc(acc_idx):
            acc    = accessors[acc_idx]
            bv_idx = acc.get("bufferView")
            if bv_idx is None: return []
            bv     = buffer_views[bv_idx]
            off    = bv.get("byteOffset",0) + acc.get("byteOffset",0)
            count  = acc["count"]
            ctype  = acc["componentType"]
            atype  = acc["type"]
            tc     = {"SCALAR":1,"VEC2":2,"VEC3":3,"VEC4":4,
                      "MAT4":16}.get(atype,1)
            fmt    = {5120:"b",5121:"B",5122:"h",5123:"H",
                      5125:"I",5126:"f"}.get(ctype,"f")
            sz     = struct.calcsize(fmt)
            stride = bv.get("byteStride", sz*tc)
            result = []
            for i in range(count):
                row = [struct.unpack_from(f"<{fmt}", bin_data,
                                          off+i*stride+j*sz)[0]
                       for j in range(tc)]
                result.append(row if tc>1 else row[0])
            return result

        tex_id_map: dict[int,int] = {}
        with tempfile.TemporaryDirectory() as tmp_dir:
            for ti, tex in enumerate(textures_gltf):
                src = tex.get("source")
                if src is None: continue
                img = images[src]
                bv_idx = img.get("bufferView")
                if bv_idx is None: continue
                bv = buffer_views[bv_idx]
                sz = bv["byteLength"]
                if sz < 16: continue
                mime = img.get("mimeType","image/png")
                ext  = ".jpg" if "jpeg" in mime else ".png"
                img_data = bin_data[bv["byteOffset"]:bv["byteOffset"]+sz]
                p = os.path.join(tmp_dir, f"tex_{ti}{ext}")
                with open(p,"wb") as f: f.write(img_data)
                try:
                    kagra_id = kagra_engine.load_texture(p)
                    tex_id_map[ti] = kagra_id
                except Exception as e:
                    if debug:
                        print(f"テクスチャ {ti} 読み込み失敗: {e}")

        for ni, node in enumerate(nodes):
            t = node.get("translation", [0,0,0])
            r = node.get("rotation",    [0,0,0,1])
            s = node.get("scale",       [1,1,1])
            bone = VrmBone(
                name=node.get("name",""),
                index=ni,
                children=node.get("children",[]),
                t=t, r=r, s=s,
                local_rot=list(r),
            )
            model.bone_list.append(bone)
            if bone.name:
                model.bones[bone.name] = bone

        for bone in model.bone_list:
            for child_idx in bone.children:
                model.bone_list[child_idx].parent = bone.index

        for skin in skins_gltf:
            joints = skin.get("joints", [])
            inv_bind_acc = skin.get("inverseBindMatrices")
            inv_bind = []
            if inv_bind_acc is not None:
                mats = read_acc(inv_bind_acc)
                inv_bind = mats
            model.skins.append(VrmSkin(joints=joints, inv_bind=inv_bind))

        mesh_to_skin: dict[str, int] = {}
        for ni, node in enumerate(nodes):
            mesh_idx = node.get("mesh")
            if mesh_idx is not None:
                mesh_name = meshes_gltf[mesh_idx].get("name", f"mesh_{mesh_idx}")
                skin_idx = node.get("skin", -1)
                mesh_to_skin[mesh_name] = skin_idx

        for mesh_gltf in meshes_gltf:
            mesh_name = mesh_gltf.get("name", "")
            skin_idx = mesh_to_skin.get(mesh_name, -1)

            for pi, prim in enumerate(mesh_gltf.get("primitives",[])):
                attrs   = prim.get("attributes",{})
                idx_acc = prim.get("indices")
                mat_idx = prim.get("material")

                kagra_tex_id = 0
                if mat_idx is not None:
                    mat = materials[mat_idx]
                    ti  = mat.get("pbrMetallicRoughness",{}).get(
                              "baseColorTexture",{}).get("index")
                    if ti is not None:
                        kagra_tex_id = tex_id_map.get(ti, 0)

                positions = read_acc(attrs["POSITION"])   if "POSITION"   in attrs else []
                normals   = read_acc(attrs["NORMAL"])     if "NORMAL"     in attrs else []
                uvs       = read_acc(attrs["TEXCOORD_0"]) if "TEXCOORD_0" in attrs else []
                joints_   = read_acc(attrs["JOINTS_0"])   if "JOINTS_0"   in attrs else []
                weights_  = read_acc(attrs["WEIGHTS_0"])  if "WEIGHTS_0"  in attrs else []
                indices   = read_acc(idx_acc)             if idx_acc is not None   else []

                if not positions: continue

                flat = []
                if indices and isinstance(indices[0], list):
                    for tri in indices: flat.extend(tri)
                else:
                    flat = [int(x) for x in indices]

                model.primitives.append(VrmPrimitive(
                    name=mesh_name,
                    texture_id=kagra_tex_id,
                    positions=positions,
                    normals=normals,
                    uvs=uvs,
                    joints=joints_,
                    weights=weights_,
                    indices=flat,
                    skin_index=skin_idx,
                ))

        print(f"VrmModel: {len(model.primitives)} prims, "
              f"{len(model.bone_list)} bones, "
              f"{len(model.skins)} skins")

        # リグをロードしてIDを取得（KAGRAエンジンの機能）
        # ここでは仮のパスを使う（実際のリグファイルパスは別途設定が必要）
        # 注意: このデモではリグファイルが存在しない可能性があるため、IKを使う場合は別途リグを用意する必要がある。
        # とりあえずダミーIDを設定（実際にはリグをロードするコードが必要）
        # 本来は vrm_path からリグJSONのパスを推測するなど。
        # 今回はIKの動作確認のため、手動でリグIDを0と仮定し、外部から設定できるようにする。
        model.rig_id = 0   # 仮のID。実際のリグがロードされていないとIKは動作しない。
        # もしリグファイルがあれば以下のようにロードする:
        # rig_path = vrm_path.replace(".vrm", ".rig.json")
        # if os.path.exists(rig_path):
        #     model.rig_id = kagra_engine.load_rig(rig_path)

        model._recompute_world()
        model._rebuild_cache()
        model._dirty = False
        return model

    def set_bone_rotation(self, bone_name: str, rx: float, ry: float, rz: float):
        if bone_name not in self.bones:
            return
        cx,sx = math.cos(rx/2), math.sin(rx/2)
        cy,sy = math.cos(ry/2), math.sin(ry/2)
        cz,sz = math.cos(rz/2), math.sin(rz/2)
        qx = sx*cy*cz + cx*sy*sz
        qy = cx*sy*cz - sx*cy*sz
        qz = cx*cy*sz + sx*sy*cz
        qw = cx*cy*cz - sx*sy*sz
        norm = math.sqrt(qx*qx + qy*qy + qz*qz + qw*qw)
        if norm > 1e-8:
            qx /= norm; qy /= norm; qz /= norm; qw /= norm
        self.bones[bone_name].local_rot = [qx, qy, qz, qw]
        self._dirty = True

    def reset_pose(self):
        for bone in self.bone_list:
            bone.local_rot = list(bone.r)
        self._dirty = True

    def update_pose(self):
        self._dirty = True

    def _build_skin_matrices(self, skin_idx: int) -> list:
        if skin_idx < 0 or skin_idx >= len(self.skins):
            return []
        skin = self.skins[skin_idx]
        mats = []
        for ji, node_idx in enumerate(skin.joints):
            if node_idx >= len(self.bone_list):
                mats.append(_mat4_identity())
                continue
            wm = self.bone_list[node_idx].world_mat
            if ji < len(skin.inv_bind):
                mats.append(_mat4_mul(wm, skin.inv_bind[ji]))
            else:
                mats.append(wm)
        return mats

    def _skin_primitive(self, prim: VrmPrimitive, skin_mats: list) -> list:
        n = len(prim.positions)
        uvs = prim.uvs
        out = []

        for vi in range(n):
            px,py,pz = prim.positions[vi]
            nx,ny,nz = prim.normals[vi] if vi < len(prim.normals) else (0,1,0)
            u,v = uvs[vi] if vi < len(uvs) else (0,0)

            if not prim.joints or not prim.weights or not skin_mats:
                out.append([px,py,pz, nx,ny,nz, u,v])
                continue

            j4 = prim.joints[vi]  if vi < len(prim.joints)  else [0,0,0,0]
            w4 = prim.weights[vi] if vi < len(prim.weights) else [1,0,0,0]

            wpx=wpy=wpz=wnx=wny=wnz=w_total = 0.0

            for k in range(4):
                w = w4[k]
                if w < 1e-6: continue
                ji = int(j4[k])
                if ji >= len(skin_mats): continue
                sm = skin_mats[ji]

                bx = sm[0]*px + sm[4]*py + sm[8]*pz  + sm[12]
                by = sm[1]*px + sm[5]*py + sm[9]*pz  + sm[13]
                bz = sm[2]*px + sm[6]*py + sm[10]*pz + sm[14]

                bnx = sm[0]*nx + sm[4]*ny + sm[8]*nz
                bny = sm[1]*nx + sm[5]*ny + sm[9]*nz
                bnz = sm[2]*nx + sm[6]*ny + sm[10]*nz

                wpx += bx * w
                wpy += by * w
                wpz += bz * w
                wnx += bnx * w
                wny += bny * w
                wnz += bnz * w
                w_total += w

            if w_total > 1e-8:
                inv = 1.0 / w_total
                fx = wpx * inv
                fy = wpy * inv
                fz = wpz * inv
                nl = math.sqrt(wnx*wnx + wny*wny + wnz*wnz)
                if nl > 1e-8:
                    fnx = wnx / nl
                    fny = wny / nl
                    fnz = wnz / nl
                else:
                    fnx, fny, fnz = 0, 1, 0
                out.append([fx, fy, fz, fnx, fny, fnz, u, v])
            else:
                out.append([px,py,pz, nx,ny,nz, u,v])
        return out

    def draw(self, kagra_engine):
        if self._dirty:
            self._recompute_world()
            self._rebuild_cache()
            self._dirty = False

        for prim in self.primitives:
            if not prim._cached_verts or not prim.indices:
                continue
            kagra_engine.draw_mesh_3d(
                prim.texture_id, prim._cached_verts, prim.indices)

    def _compute_world_order(self) -> list[VrmBone]:
        roots = [b for b in self.bone_list if b.parent is None]
        order = []
        def add_children(bone):
            order.append(bone)
            for child_idx in bone.children:
                child = self.bone_list[child_idx]
                add_children(child)
        for root in roots:
            add_children(root)
        for b in self.bone_list:
            if b not in order:
                order.append(b)
        return order

    def _recompute_world(self):
        order = self._compute_world_order()
        for bone in order:
            local_mat = _mat4_from_trs(bone.t, bone.local_rot, bone.s)
            if bone.parent is None:
                bone.world_mat = local_mat
            else:
                parent_mat = self.bone_list[bone.parent].world_mat
                bone.world_mat = _mat4_mul(parent_mat, local_mat)

    def _rebuild_cache(self):
        skin_mats_cache: dict[int, list] = {}
        for prim in self.primitives:
            si = prim.skin_index
            if si not in skin_mats_cache:
                skin_mats_cache[si] = self._build_skin_matrices(si)
            if prim.positions and prim.indices:
                prim._cached_verts = self._skin_primitive(
                    prim, skin_mats_cache[si])
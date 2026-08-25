# kagra/vrm_spring.py
"""
VRM スプリングボーンシミュレーター (修正版)
 
【主な修正点】
1. ポーズ適用後のワールド行列で「目標位置」を計算 → シミュレーション結果と差分を取る
2. 回転の合成順序を正しく修正: ポーズローカル回転 × 揺れデルタ回転（ローカル空間）
3. クリップ切替時の速度爆発防止: reset() で prev=curr にクランプ
4. 初回のアニメ未反映フレームをスキップ（bind pose を rest として誤学習しない）
 
VRM 0.x secondaryAnimation.boneGroups 対応
VRM 1.0 VRMC_springBone               対応
"""
from __future__ import annotations
import math, json
from collections import deque
from dataclasses import dataclass, field
from typing import Optional
 
# ── ベクトル / クォータニオン ユーティリティ ─────────────────
 
def _add(a,b):  return [a[0]+b[0],a[1]+b[1],a[2]+b[2]]
def _sub(a,b):  return [a[0]-b[0],a[1]-b[1],a[2]-b[2]]
def _sc(v,s):   return [v[0]*s,v[1]*s,v[2]*s]
def _dot(a,b):  return a[0]*b[0]+a[1]*b[1]+a[2]*b[2]
def _len(v):    return math.sqrt(v[0]**2+v[1]**2+v[2]**2)
def _norm(v):   l=_len(v); return [v[0]/l,v[1]/l,v[2]/l] if l>1e-8 else [0,1,0]
def _cross(a,b):return [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]
 
def _qmul(a,b):
    ax,ay,az,aw=a; bx,by,bz,bw=b
    return [aw*bx+ax*bw+ay*bz-az*by, aw*by-ax*bz+ay*bw+az*bx,
            aw*bz+ax*by-ay*bx+az*bw, aw*bw-ax*bx-ay*by-az*bz]
 
def _qconj(q):  return [-q[0],-q[1],-q[2],q[3]]
 
def _qnorm(q):
    l=math.sqrt(sum(x*x for x in q))
    return [x/l for x in q] if l>1e-8 else [0,0,0,1]
 
def _q_from_to(f,t):
    """2ベクトル間の最短回転クォータニオン"""
    f=_norm(f); t=_norm(t); d=_dot(f,t)
    if d> 0.9999: return [0,0,0,1]
    if d<-0.9999:
        perp=_cross(f,[1,0,0])
        if _len(perp)<0.001: perp=_cross(f,[0,1,0])
        p=_norm(perp); return [p[0],p[1],p[2],0.0]
    ax=_norm(_cross(f,t)); ang=math.acos(max(-1.0,min(1.0,d))); s=math.sin(ang/2)
    return _qnorm([ax[0]*s,ax[1]*s,ax[2]*s,math.cos(ang/2)])
 
def _qrotate(q, v):
    """クォータニオンでベクトルを回転する: q * v * q^-1"""
    qx,qy,qz,qw = q
    vx,vy,vz = v
    # 最適化された公式: v + 2 * cross(q.xyz, cross(q.xyz, v) + q.w * v)
    tx = 2*(qy*vz - qz*vy)
    ty = 2*(qz*vx - qx*vz)
    tz = 2*(qx*vy - qy*vx)
    return [vx + qw*tx + (qy*tz - qz*ty),
            vy + qw*ty + (qz*tx - qx*tz),
            vz + qw*tz + (qx*ty - qy*tx)]
 
def _q_from_mat3(m):
    """列優先9要素 3x3 回転行列 → クォータニオン"""
    m00,m10,m20,m01,m11,m21,m02,m12,m22=m
    tr=m00+m11+m22
    if tr>0:
        s=0.5/math.sqrt(tr+1); return _qnorm([(m21-m12)*s,(m02-m20)*s,(m10-m01)*s,0.25/s])
    if m00>m11 and m00>m22:
        s=2*math.sqrt(1+m00-m11-m22); return _qnorm([0.25*s,(m01+m10)/s,(m02+m20)/s,(m21-m12)/s])
    if m11>m22:
        s=2*math.sqrt(1+m11-m00-m22); return _qnorm([(m01+m10)/s,0.25*s,(m12+m21)/s,(m02-m20)/s])
    s=2*math.sqrt(1+m22-m00-m11); return _qnorm([(m02+m20)/s,(m12+m21)/s,0.25*s,(m10-m01)/s])
 
def _mid(): return [1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1]
 
def _mmul(a,b):
    out=[0.0]*16
    for col in range(4):
        for row in range(4):
            out[row+col*4]=sum(a[row+k*4]*b[k+col*4] for k in range(4))
    return out
 
def _mtrs(t,r,s):
    rx,ry,rz,rw=r; sx,sy,sz=s
    x2,y2,z2=rx*2,ry*2,rz*2
    xx,yy,zz=rx*x2,ry*y2,rz*z2; xy,xz,yz=rx*y2,rx*z2,ry*z2
    wx,wy,wz=rw*x2,rw*y2,rw*z2
    return [(1-(yy+zz))*sx,(xy+wz)*sx,(xz-wy)*sx,0,
            (xy-wz)*sy,(1-(xx+zz))*sy,(yz+wx)*sy,0,
            (xz+wy)*sz,(yz-wx)*sz,(1-(xx+yy))*sz,0,
            t[0],t[1],t[2],1]
 
def _mpos(m): return [m[12],m[13],m[14]]
 
def _mrot(m):
    """行列から回転クォータニオンを抽出（スケール除去済み）"""
    sx=_len([m[0],m[1],m[2]]); sy=_len([m[4],m[5],m[6]]); sz=_len([m[8],m[9],m[10]])
    if sx<1e-8: sx=1.0
    if sy<1e-8: sy=1.0
    if sz<1e-8: sz=1.0
    return _q_from_mat3([m[0]/sx,m[1]/sx,m[2]/sx,
                          m[4]/sy,m[5]/sy,m[6]/sy,
                          m[8]/sz,m[9]/sz,m[10]/sz])
 
# ── データ構造 ────────────────────────────────────────────────
 
@dataclass
class _Joint:
    node_idx:    int
    stiffness:   float       # VRM はだいたい 0〜4。UniVRM は stiffness * dt²
    drag:        float       # 空気抵抗 (0.0〜1.0)
    gravity:     list        # 重力ベクトル [x,y,z]
    radius:      float       # コライダー半径
    bone_length: float = 0.07
    # ワールド空間のシミュレーション位置
    curr: list = field(default_factory=lambda:[0.,0.,0.])
    prev: list = field(default_factory=lambda:[0.,0.,0.])
    # ポーズ後の目標位置（bind + pose 適用後）
    target: list = field(default_factory=lambda:[0.,0.,0.])
    # ポーズ後の親ワールド回転（ローカル化に使う）
    parent_world_rot: list = field(default_factory=lambda:[0.,0.,0.,1.])
    # bind pose での親 → 子方向（親のローカル空間）
    rest_dir_local: list = field(default_factory=lambda:[0.,1.,0.])
    virtual_tail: bool = False

@dataclass
class _Collider:
    node_idx: int
    offset: list           # ノードローカル
    radius: float
    tail: Optional[list] = None  # あればカプセル（ローカル終点）

@dataclass
class _Chain:
    joints: list  # list[_Joint]  ルート→末端
    collider_ids: list = field(default_factory=list)


def _mtransform_point(m, p):
    x, y, z = p
    return [
        m[0] * x + m[4] * y + m[8] * z + m[12],
        m[1] * x + m[5] * y + m[9] * z + m[13],
        m[2] * x + m[6] * y + m[10] * z + m[14],
    ]


def _as_vec3(v, default=(0.0, 0.0, 0.0)):
    if v is None:
        return list(default)
    if isinstance(v, dict):
        return [float(v.get("x", default[0])), float(v.get("y", default[1])), float(v.get("z", default[2]))]
    if isinstance(v, (list, tuple)) and len(v) >= 3:
        return [float(v[0]), float(v[1]), float(v[2])]
    return list(default)


VIRTUAL_TAIL_LEN = 0.07
SLEEVE_TRANSFER = 0.82


def is_sleeve_bone_name(name: str) -> bool:
    lower = name.lower()
    return "sleeve" in lower or "sode" in lower or "袖" in name or "ソデ" in name


def sleeve_follow(radius: float) -> float:
    a, b = 0.022, 0.038
    t = max(0.0, min(1.0, (radius - a) / (b - a)))
    return t * t * (3.0 - 2.0 * t)


def transfer_sleeve_weights(joints, weights, arm_palette, helper_palette, follow):
    """arm_palette のウェイトのうち follow を helper へ移す。"""
    if follow <= 1e-5:
        return list(joints), list(weights)
    j = list(joints)
    w = list(weights)
    arm_w = sum(w[i] for i in range(4) if j[i] == arm_palette)
    move_w = arm_w * max(0.0, min(1.0, follow))
    if move_w <= 1e-6:
        return j, w
    if arm_w > 1e-8:
        keep = 1.0 - move_w / arm_w
        for i in range(4):
            if j[i] == arm_palette:
                w[i] *= keep
    if helper_palette in j:
        w[j.index(helper_palette)] += move_w
    elif any(x <= 1e-6 for x in w):
        i = next(i for i in range(4) if w[i] <= 1e-6)
        j[i] = helper_palette
        w[i] = move_w
    else:
        best = min((i for i in range(4) if j[i] != arm_palette), key=lambda i: w[i], default=0)
        if j[best] != arm_palette:
            j[best] = helper_palette
            w[best] = move_w
    s = sum(w)
    if s > 1e-8:
        w = [x / s for x in w]
    return j, w


def collide_sphere(point, center, radius, fallback_dir=None):
    """点が球の内側なら表面へ押し出す。外側ならそのまま。"""
    to = _sub(point, center)
    dist = _len(to)
    if dist >= radius:
        return list(point)
    if dist < 1e-8:
        n = _norm(fallback_dir) if fallback_dir is not None else [0.0, 1.0, 0.0]
        return _add(center, _sc(n, radius))
    return _add(center, _sc(to, radius / dist))


def collide_capsule(point, a, b, radius, fallback_dir=None):
    """点がカプセルの内側なら表面へ押し出す。"""
    ab = _sub(b, a)
    ab_len2 = _dot(ab, ab)
    if ab_len2 < 1e-12:
        return collide_sphere(point, a, radius, fallback_dir)
    t = _dot(_sub(point, a), ab) / ab_len2
    t = max(0.0, min(1.0, t))
    closest = _add(a, _sc(ab, t))
    return collide_sphere(point, closest, radius, fallback_dir)
 
# ── メインクラス ──────────────────────────────────────────────
 
class SpringBone:
    """VRM スプリングボーンシミュレーター。
 
    Example::
        spring = kagra.SpringBone("assets/Emma.vrm", vrm_id)
        spring.update(dt, animator.current_rots)
        spring.set_wind(0.3, direction=(1,0,0))
    """
 
    def __init__(self, vrm_path: str, vrm_id: int):
        self.vrm_id  = vrm_id
        self.chains: list[_Chain] = []
        self.colliders: list[_Collider] = []
        self._enabled: bool = True
        self._wind   = [0.,0.,0.]
        self._native = False
        self._nodes: list[dict] = []       # gltf ノード (bind pose)
        self._wmats: list[list] = []       # ポーズ適用後のワールド行列
        self._topo:  list[int]  = []
        self._initialized = False          # 初回アニメ更新後まで rest の確定を遅らせる
        try:
            import kagra
            chains, joints, cols = kagra.vrm_spring_info(vrm_id)
            if chains > 0:
                self._native = True
                self.chains = [None] * int(chains)
                self.colliders = [None] * int(cols)
                print(f"[SpringBone] rust {chains} chains / {joints} joints / {cols} colliders")
                return
        except Exception:
            pass
        self._parse(vrm_path)
        self._rebuild({})                  # bind pose でまずワールド計算
        self._compute_rest_dirs()          # bind ポーズの親→子方向を保存
        self._init_joints_to_rest()

    @property
    def enabled(self) -> bool:
        return self._enabled

    @enabled.setter
    def enabled(self, value: bool):
        self._enabled = bool(value)
        if self._native:
            try:
                import kagra
                kagra.set_vrm_spring_enabled(self.vrm_id, self._enabled)
            except Exception:
                pass
 
    # ── 公開 API ──────────────────────────────────────────────
 
    def set_wind(self, strength: float = 0.0, direction: tuple = (1.,0.,0.)):
        self._wind = _sc(_norm(list(direction)), strength)
        if self._native:
            try:
                import kagra
                kagra.set_vrm_spring_wind(self.vrm_id, *self._wind)
            except Exception:
                pass
 
    def reset(self):
        """全ジョイントを現在のポーズ位置にリセット（速度ゼロ）。
        クリップ切替時にこれを呼ぶと揺れ暴走を防げる。
        """
        if self._native:
            try:
                import kagra
                kagra.reset_vrm_spring(self.vrm_id)
            except Exception:
                pass
            return
        # 現在の _wmats を使ってジョイント位置を強制同期
        for chain in self.chains:
            for i, j in enumerate(chain.joints):
                if j.virtual_tail or j.node_idx < 0:
                    pj = chain.joints[i - 1]
                    parent_q = _mrot(self._wmats[pj.node_idx])
                    pos = _add(_mpos(self._wmats[pj.node_idx]),
                               _sc(_norm(_qrotate(parent_q, j.rest_dir_local)), j.bone_length))
                else:
                    pos = _mpos(self._wmats[j.node_idx])
                j.curr = list(pos)
                j.prev = list(pos)     # 速度ゼロ
                j.target = list(pos)
 
    def update(self, dt: float, pose_rots: dict = None):
        """スプリングボーンを1ステップ進める。
 
        Args:
            dt:        デルタ時間（秒）
            pose_rots: {bone_name: [qx,qy,qz,qw]} - VrmAnimator.current_rots
        """
        if not self.enabled: return
        if not self.chains:  return
        if self._native:
            try:
                import kagra
                kagra.step_vrm_spring(self.vrm_id, min(dt, 1.0 / 30.0))
            except Exception:
                pass
            return
        dt = min(dt, 1.0/30.0)   # フレーム落ち時の暴走防止
 
        # ポーズ後のワールド行列を計算
        name2rot = pose_rots or {}
        idx_rots = {i: name2rot[n['name']]
                    for i,n in enumerate(self._nodes)
                    if n['name'] in name2rot}
        self._rebuild(idx_rots)
 
        # 初回のポーズ反映時に prev/curr をクランプ（bind pose 学習を回避）
        if not self._initialized and idx_rots:
            self._initialized = True
            self.reset()
            return    # 1 フレーム目はシミュレーションせず目標位置に合わせる
 
        # 物理シミュレーション & 回転適用
        for chain in self.chains:
            self._simulate(chain, dt)
            self._apply(chain)
 
    # ── VRM パーサー（元のまま） ────────────────────────────────
 
    def _parse(self, path: str):
        data = open(path,'rb').read()
        if data[:4] != b'glTF': raise ValueError(f"Not a glTF binary: {path}")
        offset=12; jbytes=None
        while offset+8<=len(data):
            clen =int.from_bytes(data[offset:offset+4],'little')
            ctype=int.from_bytes(data[offset+4:offset+8],'little')
            if ctype==0x4E4F534A: jbytes=data[offset+8:offset+8+clen].rstrip(b'\x00')
            offset+=8+clen
        gltf=json.loads(jbytes)
        nodes=gltf.get('nodes',[])
 
        for i,n in enumerate(nodes):
            self._nodes.append({
                'name':n.get('name',f'node_{i}'),
                't':n.get('translation',[0,0,0]),
                'r':n.get('rotation',[0,0,0,1]),
                's':n.get('scale',[1,1,1]),
                'children':n.get('children',[]),
                'parent':None,
            })
        for i,n in enumerate(nodes):
            for ci in n.get('children',[]):
                if ci<len(self._nodes): self._nodes[ci]['parent']=i
        self._topo  = self._calc_topo()
        self._wmats = [_mid()]*len(self._nodes)
 
        vrm0=gltf.get('extensions',{}).get('VRM',{})
        sa = vrm0.get('secondaryAnimation', {})
        v0_groups = []
        for cg in sa.get('colliderGroups', []):
            start = len(self.colliders)
            node = cg.get('node', 0)
            for col in cg.get('colliders', []):
                self.colliders.append(_Collider(
                    node_idx=int(node),
                    offset=_as_vec3(col.get('offset')),
                    radius=float(col.get('radius', 0.05)),
                ))
            v0_groups.append(list(range(start, len(self.colliders))))
        for g in sa.get('boneGroups', []):
            self._parse_v0(g, v0_groups)

        sb1=gltf.get('extensions',{}).get('VRMC_springBone',{})
        v1_groups = []
        for col in sb1.get('colliders', []):
            node = int(col.get('node', 0))
            shape = col.get('shape') or {}
            if 'capsule' in shape:
                cap = shape['capsule'] or {}
                self.colliders.append(_Collider(
                    node_idx=node,
                    offset=_as_vec3(cap.get('offset')),
                    radius=float(cap.get('radius', 0.05)),
                    tail=_as_vec3(cap.get('tail')),
                ))
            else:
                sph = shape.get('sphere') or {}
                self.colliders.append(_Collider(
                    node_idx=node,
                    offset=_as_vec3(sph.get('offset')),
                    radius=float(sph.get('radius', 0.05)),
                ))
        for cg in sb1.get('colliderGroups', []):
            ids = [int(i) for i in cg.get('colliders', []) if isinstance(i, (int, float))]
            v1_groups.append(ids)
        for sp in sb1.get('springs',[]):
            self._parse_v1(sp, v1_groups)

        print(f"[SpringBone] {len(self.chains)} chains / "
              f"{sum(len(c.joints) for c in self.chains)} joints / "
              f"{len(self.colliders)} colliders")
 
    def _parse_v0(self, g: dict, groups: list):
        stiff = g.get('stiffiness',1.0)
        drag  = g.get('dragForce',0.4)
        gp    = g.get('gravityPower',0.)
        gd    = g.get('gravityDir',{'x':0,'y':-1,'z':0})
        grav  = [gd.get('x',0)*gp, gd.get('y',-1)*gp, gd.get('z',0)*gp]
        rad   = g.get('hitRadius',0.02)
        col_ids = []
        for gi in g.get('colliderGroups', []):
            if isinstance(gi, (int, float)) and 0 <= int(gi) < len(groups):
                col_ids.extend(groups[int(gi)])
        if not col_ids:
            col_ids = list(range(len(self.colliders)))
        for ri in g.get('bones',[]):
            ch = self._build_chain(ri,stiff,drag,grav,rad)
            if ch:
                ch.collider_ids = list(col_ids)
                self.chains.append(ch)

    def _parse_v1(self, sp: dict, groups: list):
        joints=[]
        for jd in sp.get('joints',[]):
            ni=jd.get('node',-1)
            if ni<0 or ni>=len(self._nodes): continue
            grav=jd.get('gravityDir',[0,-1,0])
            gp  =jd.get('gravityPower',0.)
            if isinstance(grav,dict): grav=[grav.get('x',0),grav.get('y',-1),grav.get('z',0)]
            joints.append(_Joint(
                node_idx=ni,
                stiffness=jd.get('stiffness',1.),
                drag=jd.get('dragForce',.4),
                gravity=_sc(grav,gp),
                radius=jd.get('hitRadius',.02),
            ))
        col_ids = []
        for gi in sp.get('colliderGroups', []):
            if isinstance(gi, (int, float)) and 0 <= int(gi) < len(groups):
                col_ids.extend(groups[int(gi)])
        if not col_ids:
            col_ids = list(range(len(self.colliders)))
        if len(joints)>=2:
            self.chains.append(_Chain(joints=joints, collider_ids=col_ids))
 
    def _build_chain(self, root: int, stiff, drag, grav, rad) -> Optional[_Chain]:
        joints=[]
        def _walk(idx):
            joints.append(_Joint(node_idx=idx,stiffness=stiff,drag=drag,gravity=grav,radius=rad))
            ch=self._nodes[idx]['children']
            if ch: _walk(ch[0])
        if root<len(self._nodes): _walk(root)
        if len(joints) == 1:
            joints.append(_Joint(
                node_idx=-1, stiffness=stiff, drag=drag, gravity=grav, radius=rad,
                bone_length=VIRTUAL_TAIL_LEN, rest_dir_local=[0., 1., 0.], virtual_tail=True,
            ))
        return _Chain(joints=joints) if len(joints)>=2 else None
 
    # ── FK ────────────────────────────────────────────────────
 
    def _calc_topo(self):
        order=[]; visited=set()
        q=deque(i for i,n in enumerate(self._nodes) if n['parent'] is None)
        while q:
            idx=q.popleft()
            if idx in visited: continue
            visited.add(idx); order.append(idx)
            for c in self._nodes[idx]['children']: q.append(c)
        for i in range(len(self._nodes)):
            if i not in visited: order.append(i)
        return order
 
    def _rebuild(self, idx_rots: dict):
        """ポーズ回転でワールド行列を再計算。idx_rots が空なら bind pose。"""
        for idx in self._topo:
            n=self._nodes[idx]
            r=idx_rots.get(idx, n['r'])
            local=_mtrs(n['t'], r, n['s'])
            pi=n['parent']
            self._wmats[idx]=local if pi is None else _mmul(self._wmats[pi],local)
 
    def _compute_rest_dirs(self):
        """bind pose での「親の回転を取り除いた」親→子方向を保存する。
 
        これにより、ポーズ後の親回転を掛ければ正しい目標位置が再現される。
        """
        for chain in self.chains:
            for i in range(len(chain.joints) - 1):
                j  = chain.joints[i]
                jn = chain.joints[i+1]
                if jn.virtual_tail or jn.node_idx < 0:
                    if jn.bone_length < 0.001:
                        jn.bone_length = VIRTUAL_TAIL_LEN
                    continue
                # bind pose でのワールド位置
                p_parent = _mpos(self._wmats[j.node_idx])
                p_child  = _mpos(self._wmats[jn.node_idx])
                world_dir = _sub(p_child, p_parent)
                bone_len = _len(world_dir)
                jn.bone_length = bone_len if bone_len > 0.001 else VIRTUAL_TAIL_LEN
 
                # 親のワールド回転を除去してローカル方向に変換
                parent_q = _mrot(self._wmats[j.node_idx])
                local_dir = _qrotate(_qconj(parent_q), _norm(world_dir))
                jn.rest_dir_local = local_dir
 
    def _init_joints_to_rest(self):
        """bind ポーズ位置にジョイントを初期化する。"""
        for chain in self.chains:
            for i, j in enumerate(chain.joints):
                if j.virtual_tail or j.node_idx < 0:
                    pj = chain.joints[i - 1]
                    parent_q = _mrot(self._wmats[pj.node_idx])
                    pos = _add(_mpos(self._wmats[pj.node_idx]),
                               _sc(_norm(_qrotate(parent_q, j.rest_dir_local)), j.bone_length))
                else:
                    pos = _mpos(self._wmats[j.node_idx])
                j.curr = list(pos)
                j.prev = list(pos)
                j.target = list(pos)
 
    # ── シミュレーション ──────────────────────────────────────
 
    def _simulate(self, chain: _Chain, dt: float):
        """Verlet 統合で各ジョイント位置を更新する。"""
        for i, j in enumerate(chain.joints):
            if i == 0:
                # ルートはポーズに固定
                pos = _mpos(self._wmats[j.node_idx])
                j.curr = list(pos)
                j.prev = list(pos)
                j.target = list(pos)
                continue
 
            pj = chain.joints[i-1]
            parent_pos = pj.curr
 
            # ── 目標位置: 親のポーズ後ワールド回転 × rest_dir_local × bone_length + 親位置 ──
            parent_q = _mrot(self._wmats[pj.node_idx])
            j.parent_world_rot = parent_q
            rest_world_dir = _qrotate(parent_q, j.rest_dir_local)
            j.target = _add(parent_pos, _sc(_norm(rest_world_dir), j.bone_length))
 
            # ── Verlet: 慣性 + rest 軸へ stiffness*dt²（UniVRM） + 重力 + 風 ──
            vel_damped = _sc(_sub(j.curr, j.prev), 1.0 - j.drag)
            spring_force  = _sc(_norm(rest_world_dir), j.stiffness * dt * dt)
            external = _add(_sc(j.gravity, dt*dt),
                            _sc(self._wind, dt*dt))
 
            new_pos = _add(_add(j.curr, vel_damped),
                           _add(spring_force, external))
 
            # ── ボーン長拘束（親からの距離を bone_length に固定）──
            to_new = _sub(new_pos, parent_pos)
            dist   = _len(to_new)
            if dist > 1e-6:
                new_pos = _add(parent_pos, _sc(to_new, j.bone_length / dist))

            new_pos = self._collide(chain, new_pos, j.radius, rest_world_dir)
            to_new = _sub(new_pos, parent_pos)
            dist = _len(to_new)
            if dist > 1e-6:
                new_pos = _add(parent_pos, _sc(to_new, j.bone_length / dist))

            j.prev = list(j.curr)
            j.curr = list(new_pos)

    def _collide(self, chain: _Chain, point, hit_radius: float, fallback_dir):
        ids = chain.collider_ids if chain.collider_ids else range(len(self.colliders))
        pos = list(point)
        for ci in ids:
            if ci < 0 or ci >= len(self.colliders):
                continue
            c = self.colliders[ci]
            if c.node_idx < 0 or c.node_idx >= len(self._wmats):
                continue
            m = self._wmats[c.node_idx]
            center = _mtransform_point(m, c.offset)
            rad = c.radius + hit_radius
            if c.tail is not None:
                tail = _mtransform_point(m, c.tail)
                pos = collide_capsule(pos, center, tail, rad, fallback_dir)
            else:
                pos = collide_sphere(pos, center, rad, fallback_dir)
        return pos
 
    def _apply(self, chain: _Chain):
        """シミュレーション結果をボーン回転に変換して kagra に送る。
 
        親ボーン i の回転 = ポーズローカル回転 * 揺れデルタローカル回転
        """
        import kagra
        for i in range(len(chain.joints) - 1):
            j  = chain.joints[i]
            jn = chain.joints[i+1]
            if j.virtual_tail or j.node_idx < 0:
                continue
 
            # 目標方向とシミュレーション方向（ワールド）
            target_dir_world = _norm(_sub(jn.target, j.curr))
            curr_dir_world   = _norm(_sub(jn.curr,   j.curr))
 
            if _len(target_dir_world) < 0.001 or _len(curr_dir_world) < 0.001:
                continue
 
            # ── ワールド差分回転 ──
            delta_world = _q_from_to(target_dir_world, curr_dir_world)
 
            # ── 親のワールド回転空間（ポーズ後）に変換してローカル差分に ──
            # jn.parent_world_rot は j ノードのポーズ後ワールド回転
            pw_rot = jn.parent_world_rot
            delta_local = _qnorm(_qmul(_qconj(pw_rot),
                                        _qmul(delta_world, pw_rot)))
 
            # ── ポーズローカル回転を取得（なければ bind の回転）──
            name = self._nodes[j.node_idx]['name']
            # ポーズ側の current_rots 経由でローカル回転を知る手段がないので、
            # 代わりにワールド行列の分解結果からローカル逆算する
            parent_node_idx = self._nodes[j.node_idx]['parent']
            if parent_node_idx is not None:
                parent_world_q = _mrot(self._wmats[parent_node_idx])
                j_world_q      = _mrot(self._wmats[j.node_idx])
                pose_local_q   = _qnorm(_qmul(_qconj(parent_world_q), j_world_q))
            else:
                pose_local_q = _mrot(self._wmats[j.node_idx])
 
            # ── 最終回転 = 揺れデルタ × ポーズローカル ──
            final = _qnorm(_qmul(delta_local, pose_local_q))
 
            if name:
                kagra._engine.set_vrm_bone_rot(
                    self.vrm_id, name,
                    final[0], final[1], final[2], final[3]
                )
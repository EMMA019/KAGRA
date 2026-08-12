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
    stiffness:   float       # バネ硬さ (0.0〜1.0)
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
 
@dataclass
class _Chain:
    joints: list  # list[_Joint]  ルート→末端
 
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
        self.enabled: bool = True
        self._wind   = [0.,0.,0.]
        self._nodes: list[dict] = []       # gltf ノード (bind pose)
        self._wmats: list[list] = []       # ポーズ適用後のワールド行列
        self._topo:  list[int]  = []
        self._initialized = False          # 初回アニメ更新後まで rest の確定を遅らせる
        self._parse(vrm_path)
        self._rebuild({})                  # bind pose でまずワールド計算
        self._compute_rest_dirs()          # bind ポーズの親→子方向を保存
        self._init_joints_to_rest()
 
    # ── 公開 API ──────────────────────────────────────────────
 
    def set_wind(self, strength: float = 0.0, direction: tuple = (1.,0.,0.)):
        self._wind = _sc(_norm(list(direction)), strength)
 
    def reset(self):
        """全ジョイントを現在のポーズ位置にリセット（速度ゼロ）。
        クリップ切替時にこれを呼ぶと揺れ暴走を防げる。
        """
        # 現在の _wmats を使ってジョイント位置を強制同期
        for chain in self.chains:
            for j in chain.joints:
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
        for g in vrm0.get('secondaryAnimation',{}).get('boneGroups',[]):
            self._parse_v0(g)
 
        sb1=gltf.get('extensions',{}).get('VRMC_springBone',{})
        for sp in sb1.get('springs',[]):
            self._parse_v1(sp)
 
        print(f"[SpringBone] {len(self.chains)} chains / "
              f"{sum(len(c.joints) for c in self.chains)} joints")
 
    def _parse_v0(self, g: dict):
        stiff = g.get('stiffiness',1.0)
        drag  = g.get('dragForce',0.4)
        gp    = g.get('gravityPower',0.)
        gd    = g.get('gravityDir',{'x':0,'y':-1,'z':0})
        grav  = [gd.get('x',0)*gp, gd.get('y',-1)*gp, gd.get('z',0)*gp]
        rad   = g.get('hitRadius',0.02)
        for ri in g.get('bones',[]):
            ch = self._build_chain(ri,stiff,drag,grav,rad)
            if ch: self.chains.append(ch)
 
    def _parse_v1(self, sp: dict):
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
        if len(joints)>=2: self.chains.append(_Chain(joints=joints))
 
    def _build_chain(self, root: int, stiff, drag, grav, rad) -> Optional[_Chain]:
        joints=[]
        def _walk(idx):
            joints.append(_Joint(node_idx=idx,stiffness=stiff,drag=drag,gravity=grav,radius=rad))
            ch=self._nodes[idx]['children']
            if ch: _walk(ch[0])
        if root<len(self._nodes): _walk(root)
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
                # bind pose でのワールド位置
                p_parent = _mpos(self._wmats[j.node_idx])
                p_child  = _mpos(self._wmats[jn.node_idx])
                world_dir = _sub(p_child, p_parent)
                bone_len = _len(world_dir)
                jn.bone_length = bone_len if bone_len > 0.001 else 0.07
 
                # 親のワールド回転を除去してローカル方向に変換
                parent_q = _mrot(self._wmats[j.node_idx])
                local_dir = _qrotate(_qconj(parent_q), _norm(world_dir))
                jn.rest_dir_local = local_dir
 
    def _init_joints_to_rest(self):
        """bind ポーズ位置にジョイントを初期化する。"""
        for chain in self.chains:
            for j in chain.joints:
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
 
            # ── Verlet: 慣性 + バネ（目標へ引き戻す） + 重力 + 風 ──
            vel_damped = _sc(_sub(j.curr, j.prev), 1.0 - j.drag)
 
            # バネ力: 目標位置方向に stiffness だけ近づける
            toward_target = _sub(j.target, j.curr)
            spring_force  = _sc(toward_target, j.stiffness)
 
            external = _add(_sc(j.gravity, dt*dt),
                            _sc(self._wind, dt*dt*0.5))
 
            new_pos = _add(_add(j.curr, vel_damped),
                           _add(spring_force, external))
 
            # ── ボーン長拘束（親からの距離を bone_length に固定）──
            to_new = _sub(new_pos, parent_pos)
            dist   = _len(to_new)
            if dist > 1e-6:
                new_pos = _add(parent_pos, _sc(to_new, j.bone_length / dist))
 
            j.prev = list(j.curr)
            j.curr = list(new_pos)
 
    def _apply(self, chain: _Chain):
        """シミュレーション結果をボーン回転に変換して kagra に送る。
 
        親ボーン i の回転 = ポーズローカル回転 * 揺れデルタローカル回転
        """
        import kagra
        for i in range(len(chain.joints) - 1):
            j  = chain.joints[i]
            jn = chain.joints[i+1]
 
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
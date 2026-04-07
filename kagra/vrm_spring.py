# kagra/vrm_spring.py
"""
VRM スプリングボーンシミュレーター
VRM 0.x secondaryAnimation.boneGroups 対応
VRM 1.0 VRMC_springBone               対応（簡易）

使い方::
    spring = kagra.SpringBone("assets/Emma.vrm", vrm_id)

    # 毎フレーム（animator.update の後に呼ぶ）
    spring.update(dt, animator.current_rots)

    # 風
    spring.set_wind(0.3, direction=(1,0,0))
"""
from __future__ import annotations
import math, json
from collections import deque
from dataclasses import dataclass, field
from typing import Optional

# ── ベクトル / 行列 / クォータニオン ──────────────────────────

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
    l=math.sqrt(sum(x*x for x in q)); return [x/l for x in q] if l>1e-8 else [0,0,0,1]

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
    xx,yy,zz=rx*x2,ry*y2,rz*z2; xy,xz,yz=rx*y2,rx*z2,ry*z2; wx,wy,wz=rw*x2,rw*y2,rw*z2
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
    stiffness:   float       # バネ硬さ
    drag:        float       # 空気抵抗
    gravity:     list        # 重力ベクトル [x,y,z]
    radius:      float       # コライダー半径
    bone_length: float = 0.07
    curr: list = field(default_factory=lambda:[0.,0.,0.])
    prev: list = field(default_factory=lambda:[0.,0.,0.])

@dataclass
class _Chain:
    joints: list  # list[_Joint]  ルート→末端

# ── メインクラス ──────────────────────────────────────────────

class SpringBone:
    """VRM スプリングボーンシミュレーター。

    Example::
        spring = kagra.SpringBone("assets/Emma.vrm", vrm_id)
        # 毎フレーム animator.update の後に呼ぶ
        spring.update(dt, animator.current_rots)
        spring.set_wind(0.3, direction=(1,0,0))
    """

    def __init__(self, vrm_path: str, vrm_id: int):
        self.vrm_id  = vrm_id
        self.chains: list[_Chain] = []
        self._wind   = [0.,0.,0.]
        self._nodes: list[dict] = []
        self._wmats: list[list] = []
        self._topo:  list[int]  = []
        self._parse(vrm_path)
        self._rebuild({})
        self._init_joints()

    # ── 公開 API ──────────────────────────────────────────────

    def set_wind(self, strength: float = 0.0,
                 direction: tuple = (1.,0.,0.)):
        """風の強さと方向を設定する。"""
        self._wind = _sc(_norm(list(direction)), strength)

    def update(self, dt: float, pose_rots: dict = None):
        """スプリングボーンを1ステップ進める。

        Args:
            dt:        デルタ時間（秒）
            pose_rots: {bone_name: [qx,qy,qz,qw]}
                       VrmAnimator.current_rots をそのまま渡す
        """
        if not self.chains: return
        dt = min(dt, 0.05)
        # bone_name → node_idx の逆引き
        # pose_rots が None または空でも安全に動作する
        name2rot = pose_rots or {}
        idx_rots = {i: name2rot[n['name']]
                    for i,n in enumerate(self._nodes)
                    if n['name'] in name2rot}
        self._rebuild(idx_rots)
        for chain in self.chains:
            self._simulate(chain, dt)
            self._apply(chain)

    # ── VRM パーサー ──────────────────────────────────────────

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

        # ノード収集
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

        # VRM 0.x
        vrm0=gltf.get('extensions',{}).get('VRM',{})
        for g in vrm0.get('secondaryAnimation',{}).get('boneGroups',[]):
            self._parse_v0(g)

        # VRM 1.0
        sb1=gltf.get('extensions',{}).get('VRMC_springBone',{})
        for sp in sb1.get('springs',[]):
            self._parse_v1(sp)

        print(f"[SpringBone] {len(self.chains)} chains / "
              f"{sum(len(c.joints) for c in self.chains)} joints")

    def _parse_v0(self, g: dict):
        stiff = g.get('stiffiness',1.0)   # VRM spec の typo
        drag  = g.get('dragForce',0.4)
        gp    = g.get('gravityPower',0.)
        gd    = g.get('gravityDir',{'x':0,'y':-1,'z':0})
        grav  = [gd.get('x',0)*gp, gd.get('y',-1)*gp, gd.get('z',0)*gp]
        rad   = g.get('hitRadius',0.02)
        for ri in g.get('bones',[]):
            ch = self._build_chain(ri,stiff,drag,grav,rad)
            if ch: self.chains.append(ch)

    def _parse_v1(self, sp: dict):
        """VRM 1.0: joints 配列を1チェーンとして扱う"""
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
        """ポーズ回転でワールド行列を再計算する。"""
        for idx in self._topo:
            n=self._nodes[idx]; r=idx_rots.get(idx,n['r'])
            local=_mtrs(n['t'],r,n['s']); pi=n['parent']
            self._wmats[idx]=local if pi is None else _mmul(self._wmats[pi],local)

    def _init_joints(self):
        """ボーン長と初期位置を設定する。"""
        for chain in self.chains:
            for i,j in enumerate(chain.joints):
                pos=_mpos(self._wmats[j.node_idx])
                j.curr=list(pos); j.prev=list(pos)
                if i>0:
                    pp=_mpos(self._wmats[chain.joints[i-1].node_idx])
                    bl=_len(_sub(pos,pp)); j.bone_length=bl if bl>0.001 else 0.07

    # ── シミュレーション ──────────────────────────────────────

    def _simulate(self, chain: _Chain, dt: float):
        for i,j in enumerate(chain.joints):
            if i==0:
                pos=_mpos(self._wmats[j.node_idx]); j.curr=list(pos); j.prev=list(pos)
                continue
            pj=chain.joints[i-1]; pp=pj.curr

            # Verlet + 各種力
            vel     = _sc(_sub(j.curr,j.prev), 1.0-j.drag)
            rest_d  = _norm(_sub(_mpos(self._wmats[j.node_idx]),
                                  _mpos(self._wmats[pj.node_idx])))
            new_pos = _add(_add(j.curr, vel),
                           _add(_add(_sc(rest_d, j.stiffness*dt*dt),
                                     _sc(j.gravity, dt*dt)),
                                _sc(self._wind, dt*dt*0.5)))
            # ボーン長拘束
            new_pos = _add(pp, _sc(_norm(_sub(new_pos,pp)), j.bone_length))
            j.prev=list(j.curr); j.curr=list(new_pos)

    def _apply(self, chain: _Chain):
        """シミュレーション結果をボーン回転に変換して kagra に送る。"""
        import kagra
        for i in range(len(chain.joints)-1):
            j=chain.joints[i]; jn=chain.joints[i+1]; pidx=j.node_idx

            rest_dir = _norm(_sub(_mpos(self._wmats[jn.node_idx]),
                                   _mpos(self._wmats[pidx])))
            curr_dir = _norm(_sub(jn.curr, j.curr))
            if _len(rest_dir)<0.001: continue

            # ワールド差分回転 → 親のワールド回転空間でローカル化
            delta_w   = _q_from_to(rest_dir, curr_dir)
            pw_rot    = _mrot(self._wmats[pidx])
            delta_loc = _qnorm(_qmul(_qconj(pw_rot), delta_w))

            # レスト回転と合成
            rest_r = self._nodes[pidx]['r']
            final  = _qnorm(_qmul(rest_r, delta_loc))

            name = self._nodes[pidx]['name']
            if name:
                kagra._engine.set_vrm_bone_rot(
                    self.vrm_id, name, final[0], final[1], final[2], final[3])

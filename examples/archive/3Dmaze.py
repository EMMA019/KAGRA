"""
迷路ゲーム - リアル3D TPS版 (set_fog 完全除去)
操作:
  ↑↓ : 前進 / 後退
  ←→ : 旋回（カメラ連動）
  SPACE長押し : 上空から迷路確認
  ESC : 終了

assets/player.vrm を置くと VRM キャラで表示。
"""
import math, random, os, struct, zlib, tempfile
import kagra
from kagra.camera3d import Camera3D
from kagra.tilemap import TileSet, TileMap, TILE_SOLID
from kagra.physics import Rigidbody, BoxCollider, TopDownPhysicsSystem
from kagra.entity import World

SW, SH  = 1280, 720
TW = TH = 48

TILE_FLOOR, TILE_WALL, TILE_GOAL, TILE_ITEM = 0, 1, 2, 3
ATTRS    = {TILE_WALL: TILE_SOLID}
VRM_PATH  = "assets/player.vrm"

UNIT   = 2.0   # 1タイル = 2.0 ワールド単位
WALL_H = 2.5   # 壁の高さ
DRAW_R = 10    # 描画半径（タイル数）

# ── 迷路生成 ──────────────────────────────────────────────────

def generate_maze(w, h):
    w = w if w%2 else w+1; h = h if h%2 else h+1
    m = [[TILE_WALL]*w for _ in range(h)]
    st = [(1,1)]; m[1][1] = TILE_FLOOR
    ds = [(0,-2),(0,2),(-2,0),(2,0)]
    while st:
        x,y = st[-1]
        nb = [(x+dx,y+dy,dx//2,dy//2) for dx,dy in ds
              if 0<x+dx<w-1 and 0<y+dy<h-1 and m[y+dy][x+dx]==TILE_WALL]
        if nb:
            nx,ny,mx,my = random.choice(nb)
            m[y+my][x+mx] = m[ny][nx] = TILE_FLOOR; st.append((nx,ny))
        else: st.pop()
    m[h-2][w-2] = TILE_GOAL
    placed = 0
    while placed < max(4, w*h//25):
        x,y = random.randint(1,w-2), random.randint(1,h-2)
        if m[y][x]==TILE_FLOOR and (x,y) not in ((1,1),(w-2,h-2)):
            m[y][x] = TILE_ITEM; placed += 1
    return m, (1,1), (w-2,h-2)

# ── テクスチャ生成 ─────────────────────────────────────────────

def _make_png(w, h, px_fn):
    rows = b""
    for y in range(h):
        row = b"\x00"
        for x in range(w):
            row += bytes(px_fn(x, y))
        rows += row
    raw = zlib.compress(rows)
    def chunk(t,d):
        c = zlib.crc32(t+d)&0xFFFFFFFF
        return struct.pack(">I",len(d))+t+d+struct.pack(">I",c)
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR",struct.pack(">IIBBBBB",w,h,8,6,0,0,0))
           + chunk(b"IDAT",raw)+chunk(b"IEND",b""))
    p = os.path.join(tempfile.gettempdir(), f"3dmaze_{w}x{h}_{abs(hash(str(px_fn.__code__.co_consts)))%9999}.png")
    open(p,"wb").write(png)
    return kagra.load(p)

def _floor_tex():
    def px(x,y):
        s = (x//8+y//8)%2
        b = (160,110,60) if s else (140,90,45)
        n = ((x*7+y*13)%20)-10
        return (min(255,b[0]+n),min(255,b[1]+n//2),b[2],255)
    return _make_png(64,64,px)

def _wall_tex():
    def px(x,y):
        row=y//8; off=16 if row%2 else 0
        bx=(x+off)%16; by=y%8
        if bx==0 or by==0: return (90,80,70,255)
        s=(x+off)//16%2
        return (180,130,100,255) if s else (160,110,85,255)
    return _make_png(64,64,px)

def _item_tex():
    def px(x,y):
        d=math.sqrt((x-16)**2+(y-16)**2)/16
        a=max(0,int((1-d)*255))
        return (255,220,50,a)
    return _make_png(32,32,px)

def _goal_tex():
    def px(x,y):
        d=math.sqrt((x-32)**2+(y-32)**2)/32
        return (80,255,120,max(0,int((1-d)*200)))
    return _make_png(64,64,px)

# ── 3D ジオメトリ構築 ──────────────────────────────────────────
def _rot(rx, ry, ca, sa):
    return (rx*ca - ry*sa)*UNIT, (rx*sa + ry*ca)*UNIT

def _quad(vl, il, corners, y_bottom, y_top, nx, ny, nz):
    base = len(vl)
    uvs  = [(0,0),(1,0),(1,1),(0,1)]
    ys   = [y_bottom, y_bottom, y_top, y_top]
    for i,((wx,wz),uv,y) in enumerate(zip(corners, uvs, ys)):
        vl.append([wx, y, wz, nx, ny, nz, uv[0], uv[1]])
    il += [base,base+1,base+2, base,base+2,base+3]

def build_geometry(maze_data, px, py, mw, mh, angle, anim_t):
    ca, sa = math.cos(angle), math.sin(angle)
    ptx, pty = px/TW, py/TH

    fv,fi = [],[]
    wv,wi = [],[]
    iv,ii = [],[]
    gv,gi = [],[]

    r = DRAW_R
    for ty in range(max(0,int(pty)-r), min(mh,int(pty)+r+1)):
        for tx in range(max(0,int(ptx)-r), min(mw,int(ptx)+r+1)):
            rx0,ry0 = tx-ptx,   ty-pty
            if rx0*rx0+ry0*ry0 > (r+1.5)**2: continue

            A = _rot(rx0,   ry0,   ca, sa)
            B = _rot(rx0+1, ry0,   ca, sa)
            C = _rot(rx0+1, ry0+1, ca, sa)
            D = _rot(rx0,   ry0+1, ca, sa)

            tile = maze_data[ty][tx]

            if tile in (TILE_FLOOR, TILE_GOAL, TILE_ITEM):
                _quad(fv,fi, [A,B,C,D], 0,0, 0,1,0)

                cx = (A[0]+C[0])/2; cz = (A[1]+C[1])/2

                if tile == TILE_ITEM:
                    h = 0.7+math.sin(anim_t*3+tx+ty)*0.12
                    s = UNIT*0.28
                    base = len(iv)
                    iv += [
                        [cx-s,h,cz,   0,1,0, 0.5,0],
                        [cx,  h,cz-s, 0,1,0, 1,0.5],
                        [cx+s,h,cz,   0,1,0, 0.5,1],
                        [cx,  h,cz+s, 0,1,0, 0,0.5],
                    ]
                    ii += [base,base+1,base+2, base,base+2,base+3]

                elif tile == TILE_GOAL:
                    segs = 16; R = UNIT*0.42
                    bob = 0.05+math.sin(anim_t*2)*0.04
                    for i in range(segs):
                        a0=i/segs*2*math.pi; a1=(i+1)/segs*2*math.pi
                        x0=cx+math.cos(a0)*R; z0=cz+math.sin(a0)*R
                        x1=cx+math.cos(a1)*R; z1=cz+math.sin(a1)*R
                        base=len(gv)
                        gv += [[cx,bob,cz,0,1,0,0.5,0.5],
                                [x0,bob,z0,0,1,0,0.5+math.cos(a0)*0.5,0.5+math.sin(a0)*0.5],
                                [x1,bob,z1,0,1,0,0.5+math.cos(a1)*0.5,0.5+math.sin(a1)*0.5]]
                        gi += [base,base+1,base+2]

            elif tile == TILE_WALL:
                _quad(wv,wi, [A,B,C,D], WALL_H,WALL_H, 0,1,0)
                if ty>0    and maze_data[ty-1][tx]!=TILE_WALL:
                    _quad(wv,wi,[A,B,B,A], 0,WALL_H, 0,0,-1)
                if ty<mh-1 and maze_data[ty+1][tx]!=TILE_WALL:
                    _quad(wv,wi,[C,D,D,C], 0,WALL_H, 0,0, 1)
                if tx>0    and maze_data[ty][tx-1]!=TILE_WALL:
                    _quad(wv,wi,[D,A,A,D], 0,WALL_H,-1,0, 0)
                if tx<mw-1 and maze_data[ty][tx+1]!=TILE_WALL:
                    _quad(wv,wi,[B,C,C,B], 0,WALL_H, 1,0, 0)

    return (fv,fi),(wv,wi),(iv,ii),(gv,gi)

# ── VRM アニメーション（簡易） ─────────────────────────────────
def _eq(rx,ry,rz):
    cx,sx=math.cos(rx/2),math.sin(rx/2)
    cy,sy=math.cos(ry/2),math.sin(ry/2)
    cz,sz=math.cos(rz/2),math.sin(rz/2)
    return [sx*cy*cz+cx*sy*sz,cx*sy*cz-sx*cy*sz,
            cx*cy*sz+sx*sy*cz,cx*cy*cz-sx*sy*sz]

def _slerp(a,b,t):
    dot=sum(a[i]*b[i] for i in range(4))
    if dot<0: b=[-x for x in b]; dot=-dot
    dot=min(1.0,dot)
    if dot>0.9995:
        r=[a[i]+t*(b[i]-a[i]) for i in range(4)]
        l=math.sqrt(sum(x*x for x in r)) or 1e-8
        return [x/l for x in r]
    th0=math.acos(dot); th=th0*t
    sa_=math.sin(th0-th)/math.sin(th0); sb=math.sin(th)/math.sin(th0)
    return [sa_*a[i]+sb*b[i] for i in range(4)]

_ID=[0.,0.,0.,1.]

def _walk_frames(speed=1.0,arm=0.40,leg=0.45,lean=0.07):
    fs=[]
    for i in range(8):
        ph=i/8*2*math.pi
        ll=leg*math.sin(ph); lr=-leg*math.sin(ph)
        kl=max(0,leg*.5*math.sin(ph+.5)); kr=max(0,-leg*.5*math.sin(ph+math.pi+.5))
        az=-1.2
        al=arm*math.sin(ph+math.pi); ar=-arm*math.sin(ph+math.pi)
        el=max(0,arm*.5*math.sin(ph+math.pi+.4)); er=max(0,arm*.5*math.sin(ph+.4))
        tw=math.sin(ph)*.05*(1+speed*.3)
        bones={
            "J_Bip_L_UpperLeg":(ll,0,0),"J_Bip_R_UpperLeg":(lr,0,0),
            "J_Bip_L_LowerLeg":(-kl,0,0),"J_Bip_R_LowerLeg":(-kr,0,0),
            "J_Bip_L_UpperArm":(al,0,az),"J_Bip_R_UpperArm":(ar,0,-az),
            "J_Bip_L_LowerArm":(el,0,0),"J_Bip_R_LowerArm":(er,0,0),
            "J_Bip_C_Hips":(lean*.4,math.pi,tw*.6),"J_Bip_C_Spine":(lean*.6,0,tw),
            "J_Bip_C_Chest":(lean*.4,0,tw*.5),"J_Bip_C_Neck":(-lean*.3,0,0),
        }
        fs.append((bones,1.0/(8*speed)))
    return fs

_CLIPS={
    "idle": [({
        "J_Bip_C_Hips":    (0,math.pi,0),
        "J_Bip_L_UpperArm":(0,0,-1.2),"J_Bip_R_UpperArm":(0,0,1.2),
        "J_Bip_L_LowerArm":(.2,0,0),  "J_Bip_R_LowerArm":(.2,0,0),
    },.4)],
    "walk": _walk_frames(speed=1.2,arm=.45,leg=.45,lean=.08),
    "run":  _walk_frames(speed=2.4,arm=.75,leg=.70,lean=.22),
}

class PythonAnimator:
    def __init__(self,vid):
        self.vid=vid; self._clip=""; self._frames=[]; self._fidx=0
        self._t=0.; self._loop=False; self._from={}; self._cur={}; self._playing=False
    @property
    def clip(self): return self._clip
    @property
    def playing(self): return self._playing
    def play(self,name,loop=False):
        if name not in _CLIPS: return
        if self._clip==name and self._playing and self._loop==loop: return
        self._clip=name; self._frames=_CLIPS[name]; self._fidx=0
        self._t=0.; self._loop=loop; self._playing=True; self._from=dict(self._cur)
    def update(self,dt):
        if not self._playing or not self._frames: return
        bones,dur=self._frames[self._fidx]
        self._t=min(1.,self._t+dt/max(.01,dur))
        te=self._t*self._t*(3-2*self._t)
        if not bones:
            for n,qf in self._from.items():
                qn=_slerp(qf,_ID,te); self._cur[n]=qn
                kagra.get_engine().set_vrm_bone_rot(self.vid,n,*qn)
            if self._t>=1.:
                kagra.get_engine().reset_vrm_pose(self.vid); self._cur.clear()
        else:
            for n,rot in bones.items():
                qt=_eq(*rot); qf=self._from.get(n,_ID)
                qn=_slerp(qf,qt,te); self._cur[n]=qn
                kagra.get_engine().set_vrm_bone_rot(self.vid,n,*qn)
        if self._t>=1.:
            self._fidx+=1
            if self._fidx>=len(self._frames):
                if self._loop: self._fidx=0; self._from=dict(self._cur); self._t=0.
                else: self._playing=False
            else: self._from=dict(self._cur); self._t=0.

# ── プレイヤースクリプト ──────────────────────────────────────
class PlayerScript(kagra.Script):
    def start(self):
        self.rb=self.entity.get(Rigidbody)
        self.speed=240.; self.rot_speed=2.8; self.angle=0.
        self.moving=False; self.game=None

    def update(self,dt):
        if self.game.game_clear:
            self.rb.vx=self.rb.vy=0; return
        dt=min(dt,.05)
        turn=(1 if kagra.key("LEFT") else 0)+(-1 if kagra.key("RIGHT") else 0)
        fwd =(1 if kagra.key("UP")   else 0)+(-1 if kagra.key("DOWN")  else 0)
        self.angle+=turn*self.rot_speed*dt
        dx=-math.sin(self.angle)*fwd; dy=-math.cos(self.angle)*fwd
        self.rb.vx+=(dx*self.speed-self.rb.vx)*12*dt
        self.rb.vy+=(dy*self.speed-self.rb.vy)*12*dt
        self.moving=(fwd!=0)
        self._check_tiles()

    def _check_tiles(self):
        tx=int(self.entity.transform.x//TW)
        ty=int(self.entity.transform.y//TH)
        if 0<=tx<self.game.maze_w and 0<=ty<self.game.maze_h:
            t=self.game.maze_data[ty][tx]
            if   t==TILE_ITEM: self.game.pick_item(tx,ty)
            elif t==TILE_GOAL: self.game.reach_goal()

# ── メインシーン ──────────────────────────────────────────────
class MazeGame(kagra.Scene):
    def on_enter(self):
        kagra.font()
        self.anim_t=0.; self.level=1; self.total=0

        self.cam=Camera3D(SW,SH,fov_deg=55.)
        self.cam.use_orbit(radius=7.,theta=0.,phi=1.1,target=(0.,0.9,0.))

        self.tex_floor=_floor_tex()
        self.tex_wall =_wall_tex()
        self.tex_item =_item_tex()
        self.tex_goal =_goal_tex()

        self.vrm_id=None; self.anim=None
        if os.path.exists(VRM_PATH):
            try:
                self.vrm_id=kagra.load_vrm(VRM_PATH)
                self.anim=PythonAnimator(self.vrm_id)
                q_flip = _eq(0, math.pi, 0)
                kagra.get_engine().set_vrm_bone_rot(self.vrm_id,"J_Bip_C_Hips",*q_flip)
                self.anim._cur["J_Bip_C_Hips"] = q_flip
                self.anim.play("idle",loop=True)
                print("VRM ロード成功")
            except Exception as e: print(f"VRM ロード失敗: {e}")

        self.world=World(); self.physics=TopDownPhysicsSystem()
        self.player=self.world.create("Player")
        self.player.add(Rigidbody(gravity=0.,mass=1.,bounce=0.))
        col=self.player.add(BoxCollider(w=28,h=28,offset_x=-14,offset_y=-14))
        col.layer="player"; col.mask=[]
        self.ps=self.player.add(PlayerScript()); self.ps.game=self

        self.game_clear=False; self.clear_timer=0.
        # set_fog は完全に削除（エンジンにないため）
        self.start_new_maze()

    def start_new_maze(self):
        self.maze_w=min(81,25+(self.level-1)*8)
        self.maze_h=min(61,17+(self.level-1)*5)
        self.maze_data,start,self.goal=generate_maze(self.maze_w,self.maze_h)
        dummy=TileSet(self.tex_floor,TW,TH)
        self.tilemap=TileMap(dummy,self.maze_data,ATTRS,TW,TH)
        self.physics.set_tilemap(self.tilemap)
        px=start[0]*TW+TW/2; py=start[1]*TH+TH/2
        self.player.transform.x,self.player.transform.y=px,py
        self.ps.angle=0.
        self.items_left=sum(row.count(TILE_ITEM) for row in self.maze_data)
        self.game_clear=False
        self.cam.update(kagra.get_engine())

    def pick_item(self,tx,ty):
        self.maze_data[ty][tx]=TILE_FLOOR; self.total+=10; self.items_left-=1

    def reach_goal(self):
        if not self.game_clear:
            self.game_clear=True; self.clear_timer=0.; self.total+=100

    def update(self,dt):
        dt=min(dt,.05)
        if kagra.pressed("ESCAPE"): raise SystemExit
        self.anim_t+=dt
        self.world.update(dt); self.physics.update(dt,self.world)
        kagra.flush_events()
        if self.game_clear:
            self.clear_timer+=dt
            if self.clear_timer>1.8: self.level+=1; self.start_new_maze()
            return
        if self.anim:
            target="walk" if self.ps.moving else "idle"
            if self.anim.clip!=target or not self.anim.playing:
                self.anim.play(target,loop=True)
            self.anim.update(dt)
        if kagra.key("SPACE"):
            self.cam.orbit_phi,self.cam.orbit_r=0.15,28.
        else:
            self.cam.orbit_phi,self.cam.orbit_r=1.1,7.
        self.cam.update(kagra.get_engine())

    def draw(self):
        kagra.cls(110,180,230)
        px=self.player.transform.x; py=self.player.transform.y
        angle=self.ps.angle
        (fv,fi),(wv,wi),(iv,ii),(gv,gi)=build_geometry(
            self.maze_data,px,py,self.maze_w,self.maze_h,angle,self.anim_t)
        if fv: kagra.draw_mesh_3d(self.tex_floor,fv,fi)
        if wv: kagra.draw_mesh_3d(self.tex_wall, wv,wi)
        if iv: kagra.draw_mesh_3d(self.tex_item, iv,ii)
        if gv: kagra.draw_mesh_3d(self.tex_goal, gv,gi)
        if self.vrm_id is not None: kagra.draw_vrm(self.vrm_id)

        kagra.text(f"レベル {self.level}",  30, 20, 32,(255,240,100))
        kagra.text(f"スコア {self.total}",   30, 65, 26,(255,220, 80))
        if self.items_left:
            kagra.text(f"アイテム残り {self.items_left}",30,105,20,(200,220,255))
        kagra.fill(0,SH-40,SW,40,(0,0,0),120)
        kagra.text("↑↓:前後  ←→:旋回  SPACE:俯瞰  ESC:終了",20,SH-30,18,(180,180,180))

        if self.game_clear:
            alpha=min(200,int(200*self.clear_timer/0.6))
            kagra.fill(0,0,SW,SH,(0,0,0),alpha)
            w,_=kagra.measure("CLEAR!",80)
            kagra.text("CLEAR!",(SW-w)//2,SH//2-50,80,(255,215,0))

# ── タイトル ──────────────────────────────────────────────────
class TitleScene(kagra.Scene):
    def on_enter(self):
        kagra.font(); self.t=0.

    def update(self,dt):
        self.t+=dt
        if kagra.pressed("Z"): kagra.go(MazeGame())

    def draw(self):
        kagra.cls(25,35,60)
        w,_=kagra.measure("まいぞう たんけん 3D",64)
        kagra.text("まいぞう たんけん 3D",(SW-w)//2,160,64,(255,220,80))
        w,_=kagra.measure("床・壁・アイテムが見える 3D 迷路！",24)
        kagra.text("床・壁・アイテムが見える 3D 迷路！",(SW-w)//2,280,24,(150,200,255))
        if int(self.t*2)%2==0:
            w,_=kagra.measure("Z キーでスタート",30)
            kagra.text("Z キーでスタート",(SW-w)//2,380,30,(200,200,200))
        w,_=kagra.measure("assets/model/player.vrm を置くと 3D キャラ表示",20)
        kagra.text("assets/model/player.vrm を置くと 3D キャラ表示",(SW-w)//2,520,20,(120,120,120))

if __name__=="__main__":
    kagra.init(width=SW,height=SH,title="Maze Explorer 3D",fps=60)
    kagra.run(start_scene=TitleScene())

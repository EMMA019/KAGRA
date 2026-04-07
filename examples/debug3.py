"""
motion_analyzer_pro.py - ライブ・モーション解析ツール（安定版）
================================================
FBX の真の底（Floor）を検出し、VRM の地面（Y=0）に強制的に合わせます。
これで Emma の足やお尻が床に届かない問題を解決します。

操作:
  RIGHT : フレーム送り
  LEFT  : フレーム戻し
  H     : 再生/停止
"""
import struct
import json
import os
import kagra
from kagra.camera3d import Camera3D

SW, SH = 1280, 720
VRM_PATH = "assets/Emma.vrm"
FBX_PATH = "assets/Flair.fbx"

def get_vrm_static_info(path):
    """VRMを解析してバインドポーズの情報を取得"""
    if not os.path.exists(path): return 0.8532, 0.0
    try:
        with open(path, 'rb') as f:
            f.read(12)
            chunk_len = struct.unpack('<I', f.read(4))[0]
            f.read(4)
            data = json.loads(f.read(chunk_len).decode('utf-8'))
        nodes = data.get('nodes', [])
        parents = {c: i for i, n in enumerate(nodes) for c in n.get('children', [])}
        def world_y(idx):
            n = nodes[idx]
            y = n.get('translation', [0,0,0])[1]
            p = parents.get(idx)
            return y + world_y(p) if p is not None else y
        
        hips_y = 0.8532
        foot_y = 0.0
        for i, n in enumerate(nodes):
            name = n.get('name', '').lower()
            if 'hips' in name: hips_y = world_y(i)
            if 'foot' in name and 'l' in name: foot_y = world_y(i)
        return hips_y, foot_y
    except: return 0.8532, 0.0

class Analyzer:
    def __init__(self):
        self.vrm_id = None
        self.raw_frames = None
        self.total_frames = 0
        self.initialized = False
        self.init_wait = 60 # 初期化待ちを増強
        self.cur_frame = 0
        self.playing = False
        self.timer = 0.0
        
        self.cam = Camera3D()
        self.cam.use_orbit(radius=3.5, target=(0, 0.8, 0))
        
        # VRM基準値
        self.vrm_hips_base, _ = get_vrm_static_info(VRM_PATH)
        
        # FBX解析用
        self.fbx_f0_y = 0.0
        self.fbx_min_y = 999.0
        self.scale = 1.0

    def setup(self):
        print("\n--- Analyzer Pro: Deep Analysis ---")
        self.vrm_id = kagra.load_vrm(VRM_PATH)
        raw = kagra._engine.load_fbx_anim(FBX_PATH)
        if not raw: return
        
        _, _, self.raw_frames = raw[0]
        self.total_frames = len(self.raw_frames)
        
        # FBXの全フレームをスキャンして「一番低い位置（床）」を探す
        for frame in self.raw_frames:
            for n, tx, ty, tz, qx, qy, qz, qw, ht in frame:
                if n == 'Armature':
                    if ty < self.fbx_min_y: self.fbx_min_y = ty
                    if self.fbx_f0_y == 0: self.fbx_f0_y = ty
        
        # 【重要】スケールの再計算
        # FBX の「足の長さ」は (開始時の腰 - 最低地点の床)
        fbx_leg_len = self.fbx_f0_y - self.fbx_min_y
        vrm_leg_len = self.vrm_hips_base # Emmaの床は Y=0 なのでそのまま
        
        self.scale = vrm_leg_len / fbx_leg_len if fbx_leg_len > 0 else 1.0
        
        print(f"VRM Hips: {self.vrm_hips_base:.4f}m")
        print(f"FBX Floor Level: {self.fbx_min_y:.4f}m")
        print(f"Computed scale (Leg-based): {self.scale:.4f}")
        print("-----------------------------------\n")
        self.initialized = True

    def update(self, dt):
        if not self.initialized:
            self.init_wait -= 1
            if self.init_wait == 0: self.setup()
            return

        last_f = self.cur_frame
        if kagra.pressed("RIGHT"): self.cur_frame = (self.cur_frame + 1) % self.total_frames
        if kagra.pressed("LEFT"): self.cur_frame = (self.cur_frame - 1) % self.total_frames
        if kagra.pressed("H"): self.playing = not self.playing

        if self.playing:
            self.timer += dt
            if self.timer >= 0.033:
                self.cur_frame = (self.cur_frame + 1) % self.total_frames
                self.timer = 0

        # 解析ログ
        if last_f != self.cur_frame or self.playing:
            self.analyze_frame()

        # 適用
        frame = self.raw_frames[self.cur_frame]
        for n, tx, ty, tz, qx, qy, qz, qw, ht in frame:
            q_fix = (0.7071, 0.0, 0.0, 0.7071)
            qx,qy,qz,qw = self.qmul(q_fix, (qx,qy,qz,qw)) if n == 'Armature' else (qx,qy,qz,qw)
            
            for m in ['set_vrm_bone_rotation', 'set_vrm_bone_rot']:
                f = getattr(kagra, m, None)
                if f: f(self.vrm_id, n, qx, qy, qz, qw); break
            
            if n == 'Armature' and ht:
                # 【修正ロジック】
                # FBX の床 (min_y) を 0 とみなし、そこからの高さをスケールして適用する
                dy = ((ty - self.fbx_min_y) * self.scale) - self.vrm_hips_base
                kagra._engine.set_vrm_offset(self.vrm_id, tx*self.scale, dy, tz*self.scale)

    def qmul(self, a, b):
        ax,ay,az,aw = a; bx,by,bz,bw = b
        return (aw*bx+ax*bw+ay*bz-az*by, aw*by-ax*bz+ay*bw+az*bx, aw*bz+ax*by-ay*bx+az*bw, aw*bw-ax*bx-ay*by-az*bz)

    def analyze_frame(self):
        cy = 0.0
        for n, tx, ty, tz, qx, qy, qz, qw, ht in self.raw_frames[self.cur_frame]:
            if n == 'Armature': cy = ty; break
        
        #  Emma の腰の最終的な地上高
        # (FBXの現在値 - FBXの床) * スケール
        world_y = (cy - self.fbx_min_y) * self.scale
        
        status = "NORMAL"
        if world_y < 0.1: status = "!! FLOOR !!"
        elif world_y > self.vrm_hips_base + 0.1: status = "FLOATING"

        print(f"F:{self.cur_frame:3d} | FBX_Y:{cy:+.4f} | WorldY:{world_y:.4f} | {status}")

    def draw(self):
        # swap chainエラーを完全に無視する
        try:
            kagra.cls(20, 20, 30)
            if self.initialized:
                kagra.draw_vrm(self.vrm_id)
        except:
            pass

if __name__ == "__main__":
    kagra.init(SW, SH, "Motion Analyzer Pro")
    a = Analyzer()
    kagra.run(a.update, a.draw)
"""
motion_validator.py - 最終検証版（補正計算実装）
================================================
ログから判明した「0.8532 vs 0.6193」の比率を使い、
足の長さの差を吸収する補正（スケーリング）を実証します。

操作:
  RIGHT : フレーム送り (Next)
  LEFT  : フレーム戻し (Prev)
  H     : 自動再生切り替え
"""
import struct
import json
import os
import kagra

SW, SH = 1280, 720
VRM_PATH = "assets/Emma.vrm"
FBX_PATH = "assets/Flair.fbx"

# 座標系の補正用クォータニオン（X軸+90度：Mixamoの寝そべりを直立に直す）
# これにより、FBXの「旋回」がVRMの「旋回」として正しく伝わるようになります。
_Q_FIX_X90 = (0.7071, 0.0, 0.0, 0.7071)

def _qmul(a, b):
    ax,ay,az,aw = a; bx,by,bz,bw = b
    return (aw*bx+ax*bw+ay*bz-az*by,
            aw*by-ax*bz+ay*bw+az*bx,
            aw*bz+ax*by-ay*bx+az*bw,
            aw*bw-ax*bx-ay*by-az*bz)

def get_vrm_hips_standard_y(path):
    if not os.path.exists(path): return 0.8532
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
        for i, n in enumerate(nodes):
            if n.get('name') in ('J_Bip_C_Hips', 'Hips', 'mixamorig:Hips'):
                return world_y(i)
    except: pass
    return 0.8532

class Validator:
    def __init__(self):
        self.vrm_id = None
        self.raw_frames = None
        self.total_frames = 0
        self.initialized = False
        self.init_delay = 30 
        self.cur_frame = 0
        self.playing = False
        self.timer = 0.0
        
        # ログから得られた基準値
        self.vrm_base_y = get_vrm_hips_standard_y(VRM_PATH)
        self.f0_arm_y = 0.0
        self.f0_arm_x = 0.0
        self.f0_arm_z = 0.0
        self.scale_factor = 1.0

    def setup(self):
        print("\n--- Loading Assets ---")
        try:
            self.vrm_id = kagra.load_vrm(VRM_PATH)
            raw = kagra._engine.load_fbx_anim(FBX_PATH)
            if not raw: return
            
            _, _, self.raw_frames = raw[0]
            self.total_frames = len(self.raw_frames)
            
            # Frame 0 の基準
            for n, tx, ty, tz, qx, qy, qz, qw, ht in self.raw_frames[0]:
                if n == 'Armature':
                    self.f0_arm_x, self.f0_arm_y, self.f0_arm_z = tx, ty, tz
                    break
            
            # 【重要】プロポーション補正倍率の算出
            self.scale_factor = self.vrm_base_y / self.f0_arm_y if self.f0_arm_y > 0 else 1.0
                    
            print(f"VRM Base Hips Y: {self.vrm_base_y:.4f}")
            print(f"FBX Base Arm Y : {self.f0_arm_y:.4f}")
            print(f"Scale Factor   : {self.scale_factor:.4f} (Apply to movement)")
            print("Ready. Controls: [RIGHT] Next, [LEFT] Prev, [H] Play\n")
            self.initialized = True
            self.log_stats()
        except Exception as e:
            print(f"Setup Error: {e}")

    def update(self, dt):
        if not self.initialized:
            self.init_delay -= 1
            if self.init_delay <= 0: self.setup()
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

        if last_f != self.cur_frame or (self.playing and self.timer == 0):
            self.log_stats()

        # 適用
        frame = self.raw_frames[self.cur_frame]
        for n, tx, ty, tz, qx, qy, qz, qw, ht in frame:
            if n == 'Armature':
                # 回転：X+90度の補正を入れてから適用（これで「90度ズレ」と「おかしな回り方」を同時に直す）
                q_corrected = _qmul(_Q_FIX_X90, (qx, qy, qz, qw))
                for m in ['set_vrm_bone_rotation', 'set_vrm_bone_rot']:
                    f = getattr(kagra, m, None)
                    if f: f(self.vrm_id, n, *q_corrected); break
                
                # 移動：開始地点(F0)からの差分を算出し、身長差(scale_factor)を掛けて適用
                dx = (tx - self.f0_arm_x) * self.scale_factor
                dy = (ty - self.f0_arm_y) * self.scale_factor
                dz = (tz - self.f0_arm_z) * self.scale_factor
                kagra._engine.set_vrm_offset(self.vrm_id, dx, dy, dz)
            else:
                for m in ['set_vrm_bone_rotation', 'set_vrm_bone_rot']:
                    f = getattr(kagra, m, None)
                    if f: f(self.vrm_id, n, qx, qy, qz, qw); break

    def log_stats(self):
        cy = 0.0
        for n, tx, ty, tz, qx, qy, qz, qw, ht in self.raw_frames[self.cur_frame]:
            if n == 'Armature': cy = ty; break
        
        # 補正後の Delta Y
        dy_corrected = (cy - self.f0_arm_y) * self.scale_factor
        ry = self.vrm_base_y + dy_corrected
        
        status = "NORMAL  "
        if ry > self.vrm_base_y + 0.05: status = "FLOATING"
        elif ry < 0.15: status = "CLIPPING"
        
        print(f"F:{self.cur_frame:3d} | FBX_Y:{cy:.4f} | DeltaCorrected:{dy_corrected:+.4f} | ResultHipsY:{ry:.4f} | {status}")

    def draw(self):
        try:
            kagra.cls(10, 10, 15)
            if self.initialized: kagra.draw_vrm(self.vrm_id)
        except: pass

if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="Validator")
    v = Validator()
    kagra.run(v.update, v.draw)
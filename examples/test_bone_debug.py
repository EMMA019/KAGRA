"""
全ボーンのデルタ回転を可視化するデバッグシーン。
怪しいボーンを特定するためのツール。

操作:
  ← → : フレーム移動（1フレームずつ）
  [ ] : フレーム移動（10フレームずつ）
  SPACE : 再生/停止
  ESC : 終了
"""
import kagra, math
from kagra.camera3d import Camera3D

SW, SH = 1280, 720
FBX_PATH = "assets/Flair.fbx"

class BoneDebugScene(kagra.Scene):
    def on_enter(self):
        self.font = kagra.assets.font("meiryo")
        self.cam = Camera3D(SW, SH, fov_deg=35.0)
        self.cam.use_orbit(radius=3.5, theta=0.0, phi=0.1,
                           target=(0.0, 0.85, 0.0))
        self._drag = False
        self._lmx = self._lmy = 0

        self.av = kagra.avatar("assets/Emma.vrm")

        from kagra.fbx_player import load_fbx
        self.motion = load_fbx(FBX_PATH)
        self.clip = self.motion.to_clip()
        self.av.add_motion("dance", self.motion)

        self.fidx = 0
        self.playing = False
        self.timer = 0.0

        # ボーンごとのデルタ回転の角度（度）をフレームごとに記録
        # 大きな角度のボーンを特定する
        self._analyze_bones()

    def _analyze_bones(self):
        """全ボーンの最大回転角度を分析"""
        self.bone_max_angle = {}
        for frame_data in self.clip:
            bones = frame_data[0]
            for bname, rot in bones.items():
                if len(rot) == 7:
                    qx,qy,qz,qw = rot[3],rot[4],rot[5],rot[6]
                elif len(rot) == 4:
                    qx,qy,qz,qw = rot
                else:
                    continue
                if True:
                    w = max(-1.0, min(1.0, qw))
                    angle = math.degrees(2.0 * math.acos(w))
                    if angle > 180: angle = 360 - angle
                    prev = self.bone_max_angle.get(bname, 0)
                    self.bone_max_angle[bname] = max(prev, angle)

        # 回転が大きいボーントップ10
        print("\n=== 最大デルタ回転角度トップ15 ===")
        sorted_bones = sorted(self.bone_max_angle.items(),
                             key=lambda x: x[1], reverse=True)
        for bname, angle in sorted_bones[:15]:
            flag = " ← 要確認!" if angle > 60 else ""
            print(f"  {bname:35s}: {angle:6.1f}°{flag}")

    def _apply_frame(self, fidx):
        """指定フレームを手動適用"""
        if not self.clip or fidx >= len(self.clip):
            return
        bones, dur, root_pos = self.clip[fidx]
        # root_offset 適用
        kagra._engine.set_vrm_offset(self.av.vrm_id,
            float(root_pos[0]), float(root_pos[1]), float(root_pos[2]))
        # ポーズリセット
        kagra._engine.reset_vrm_pose(self.av.vrm_id)
        # ボーン回転適用
        for bname, rot in bones.items():
            if len(rot) == 7:
                qx,qy,qz,qw = rot[3],rot[4],rot[5],rot[6]
            elif len(rot) == 4:
                qx,qy,qz,qw = rot
            else:
                continue
            kagra._engine.set_vrm_bone_rot(self.av.vrm_id, bname,
                float(qx), float(qy), float(qz), float(qw))

    def update(self, dt):
        if kagra.pressed("ESCAPE"): raise SystemExit

        moved = False
        if kagra.pressed("RIGHT"): self.fidx = (self.fidx+1) % len(self.clip); moved=True
        if kagra.pressed("LEFT"):  self.fidx = (self.fidx-1) % len(self.clip); moved=True
        if kagra.pressed("UP"):   self.fidx = (self.fidx+10) % len(self.clip); moved=True
        if kagra.pressed("DOWN"): self.fidx = (self.fidx-10) % len(self.clip); moved=True
        if kagra.pressed("SPACE"): self.playing = not self.playing

        if self.playing:
            self.timer += dt
            fps = self.motion.fps
            if self.timer >= 1.0/fps:
                self.timer = 0.0
                self.fidx = (self.fidx+1) % len(self.clip)
                moved = True
        elif moved:
            pass

        self._apply_frame(self.fidx)

        # カメラ
        mx, my = kagra.mouse_pos()
        if kagra.mouse_pressed(kagra.MOUSE_LEFT):
            self._drag=True; self._lmx,self._lmy=mx,my
        if kagra.mouse_down(kagra.MOUSE_LEFT) and self._drag:
            self.cam.orbit_by((mx-self._lmx)*.008, -(my-self._lmy)*.008)
            self._lmx,self._lmy=mx,my
        if not kagra.mouse_down(kagra.MOUSE_LEFT): self._drag=False
        _, wy = kagra.mouse_wheel()
        if wy: self.cam.zoom(-wy*0.2)
        self.cam.update(kagra._engine)

    def draw(self):
        kagra.cls(10, 8, 20)
        kagra.draw_vrm(self.av.vrm_id)

        # 現フレームの怪しいボーンを画面に表示
        kagra.rect(0, 0, SW, 80, 0, 0, 0, 160)
        kagra.rect(0, SH-50, SW, 50, 0, 0, 0, 160)

        if self.clip:
            bones, dur, root_pos = self.clip[self.fidx]
            # 回転が大きいボーントップ5を表示
            angles = []
            for bname, rot in bones.items():
                if len(rot) == 4: qx,qy,qz,qw = rot
                elif len(rot)==7: qx,qy,qz,qw = rot[3],rot[4],rot[5],rot[6]
                else: continue
                w = max(-1.0, min(1.0, qw))
                angle = math.degrees(2.0*math.acos(w))
                if angle > 180: angle = 360-angle
                angles.append((bname, angle))
            angles.sort(key=lambda x: x[1], reverse=True)

            kagra.draw_text(self.font,
                f"Frame {self.fidx}/{len(self.clip)-1}  "
                f"{'▶ PLAY' if self.playing else '■ STOP'}  "
                f"root=({root_pos[0]:.2f},{root_pos[1]:.2f},{root_pos[2]:.2f})",
                10, 10, 18, color=(255,210,80))

            top5 = "  ".join(f"{n.replace('J_Bip_','')}: {a:.0f}°"
                             for n,a in angles[:5] if a > 5)
            kagra.draw_text(self.font, f"大きい回転: {top5}",
                10, 35, 15, color=(100,255,150))

            warn = [(n,a) for n,a in angles if a > 80]
            if warn:
                wstr = "  ".join(f"{n}: {a:.0f}°" for n,a in warn[:3])
                kagra.draw_text(self.font, f"⚠ 要確認: {wstr}",
                    10, 58, 15, color=(255,80,80))

        kagra.draw_text(self.font,
            "← →:1フレーム  ↑↓:10フレーム  SPACE:再生  ドラッグ:回転",
            10, SH-38, 15, color=(160,160,180))


if __name__ == "__main__":
    kagra.init(SW, SH, "Bone Debug", 60)
    kagra.run(start_scene=BoneDebugScene())

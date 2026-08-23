"""
test_blendshapes.py - ブレンドシェイプ（表情）テスト（Scene方式）
"""
import kagra

SW, SH = 800, 600

class BlendShapeTestScene(kagra.Scene):
    def on_enter(self):
        # VRMをロード（Rendererは既に初期化済み）
        self.vrm_id = kagra.load_vrm("assets/Emma.vrm")
        print(f"VRM ID: {self.vrm_id}")

        # 利用可能なブレンドシェイプを表示
        shapes = kagra.list_blend_shapes(self.vrm_id)
        print(f"\n利用可能なブレンドシェイプ ({len(shapes)}個):")
        for i, shape in enumerate(shapes):
            print(f"  {i}: {shape}")

        # テスト用の表情リスト
        self.test_shapes = ["Fcl_ALL_Joy", "Fcl_ALL_Angry", "Fcl_ALL_Sorrow",
                            "Fcl_ALL_Fun", "Fcl_ALL_Surprised", "Fcl_ALL_Neutral"]
        self.shape_idx = 0
        self.timer = 0.0

        # カメラ設定（必須）
        self.cam = kagra.Camera3D(SW, SH, fov_deg=45.0)
        self.cam.use_orbit(radius=2.5, theta=0.0, phi=0.2,
                           target=(0.0, 1.0, 0.0))

        # フォント
        self.font = kagra.font()

    def update(self, dt):
        self.cam.update(kagra.get_engine())
        self.timer += dt

        # 2秒ごとに表情を切り替え
        if self.timer >= 2.0:
            self.timer = 0.0
            # 現在の表情をリセット
            current = self.test_shapes[self.shape_idx]
            kagra.set_blend_shape(self.vrm_id, current, 0.0)
            # 次の表情へ
            self.shape_idx = (self.shape_idx + 1) % len(self.test_shapes)
            new_shape = self.test_shapes[self.shape_idx]
            kagra.set_blend_shape(self.vrm_id, new_shape, 1.0)
            print(f"Switch to: {new_shape}")

        if kagra.pressed("ESCAPE"):
            exit()

    def draw(self):
        kagra.cls(20, 20, 40)
        kagra.draw_vrm(self.vrm_id)

        # 現在の表情を表示
        current = self.test_shapes[self.shape_idx]
        kagra.draw_text(self.font, f"Current blend shape: {current}", 20, 40, 20, (255, 255, 255))
        kagra.draw_text(self.font, "ESC to exit", 20, 70, 16, (200, 200, 200))

if __name__ == "__main__":
    kagra.init(SW, SH, "BlendShape Test", 60)
    kagra.run(start_scene=BlendShapeTestScene())
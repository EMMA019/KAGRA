import kagra
from kagra.camera3d import Camera3D

SW, SH = 1280, 720

class TestDraw(kagra.Scene):
    def on_enter(self):
        self.cam = Camera3D(SW, SH, fov_deg=45)
        self.cam.use_orbit(radius=5, target=(0,1,0))

    def draw(self):
        kagra.cls(30,30,50)
        # テスト: 赤い線 (0,0,0) → (1,1,1)
        kagra.draw_line_3d(0,0,0, 1,1,1, (255,0,0), 2)
        # テスト: 緑の線 (0,1,0) → (0,0,1)
        kagra.draw_line_3d(0,1,0, 0,0,1, (0,255,0), 2)

if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="Draw Test")
    kagra.run(start_scene=TestDraw())
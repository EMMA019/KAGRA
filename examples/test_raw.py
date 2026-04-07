# test_debug.py - 迷路ゲームの描画機能を流用してVRMだけ表示
import math
import kagra
from kagra.camera3d import Camera3D
from kagra.tilemap import TileSet, TileMap, TILE_SOLID

SW, SH = 1280, 720
VRM_PATH = "assets/Emma.vrm"

class DebugScene(kagra.Scene):
    def on_enter(self):
        # カメラ設定 (迷路ゲームと同じように)
        self.cam = Camera3D(SW, SH, fov_deg=55.0)
        self.cam.use_orbit(radius=3.0, theta=0.0, phi=0.5, target=(0.0, 0.9, 0.0))
        
        # VRM読み込み
        self.av = kagra.avatar(VRM_PATH)
        self.av.play("idle")
        
        # デバッグ用の地面を簡易生成（迷路の床と同じテクスチャを使う）
        # ただし、地面がなくてもVRMは表示されるはず。とりあえず床を置いて位置を確認
        from kagra.tilemap import TileSet, TileMap
        # ダミーのタイルマップ（1x1の床）
        dummy_tex = kagra.load_texture("assets/floor.png")  # 無ければNoneでも可
        tile_set = TileSet(dummy_tex, 2.0, 2.0)  # 仮のサイズ
        self.tilemap = TileMap(tile_set, [[0]], {0: 0}, 2.0, 2.0)
        
        self.status = "VRM loaded. Press H to play raw FBX (if loaded)"
        print(self.status)

    def update(self, dt):
        if kagra.pressed("ESCAPE"):
            raise SystemExit
        self.av.update(dt)
        self.cam.update(kagra._engine)

    def draw(self):
        kagra.cls(30, 40, 60)
        # 地面を描画（任意）
        # self.tilemap.draw()  # コメント解除しても良い
        kagra.draw_vrm(self.av.vrm_id)
        kagra.text(self.status, 20, 20, 16, (255,255,200))

if __name__ == "__main__":
    kagra.init(width=SW, height=SH, title="VRM Debug")
    kagra.run(start_scene=DebugScene())
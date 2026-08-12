import kagra as kagra_core
from pathlib import Path
import math

def main():
    engine = kagra_core._Engine(1280, 720, "VTuber App", 60, False, True, False)

    vrm_path = Path(r"D:\program\kagra\models\Emma.vrm")
    vrm_id = None
    frame = 0

    def _make_lookat(eye, target):
        """簡易 look-at ビュー行列（行優先）"""
        ex,ey,ez = eye; tx,ty,tz = target
        fx,fy,fz = tx-ex, ty-ey, tz-ez
        fl = math.sqrt(fx*fx+fy*fy+fz*fz) or 1e-8
        fx/=fl; fy/=fl; fz/=fl
        rx,ry,rz = fy*0-fz*1, fz*0-fx*0, fx*1-fy*0  # up=(0,1,0)とのcross
        rl = math.sqrt(rx*rx+ry*ry+rz*rz) or 1e-8
        rx/=rl; ry/=rl; rz/=rl
        ux,uy,uz = ry*fz-rz*fy, rz*fx-rx*fz, rx*fy-ry*fx
        return [
            rx,  ry,  rz,  -(rx*ex+ry*ey+rz*ez),
            ux,  uy,  uz,  -(ux*ex+uy*ey+uz*ez),
            -fx,-fy,-fz,   (fx*ex+fy*ey+fz*ez),
            0,   0,   0,   1,
        ]

    def _make_proj(fov_deg, aspect, near, far):
        """wgpu 用 perspective 行列（行優先）"""
        f = 1.0 / math.tan(math.radians(fov_deg) * 0.5)
        return [
            f/aspect, 0,  0,                          0,
            0,        f,  0,                          0,
            0,        0,  far/(near-far),  (near*far)/(near-far),
            0,        0, -1,                          0,
        ]

    def update(dt):
        nonlocal frame, vrm_id
        frame += 1
        if frame == 30 and vrm_id is None:
            vrm_id = engine.load_vrm(str(vrm_path))
            print(f"Loaded vrm_id={vrm_id}")

        # ── カメラ行列を毎フレーム送る ──────────────────
        eye    = (0.0, 1.0, 3.0)   # カメラ位置
        target = (0.0, 0.9, 0.0)   # 注視点（胸付近）
        view = _make_lookat(eye, target)
        proj = _make_proj(30.0, 1280/720, 0.01, 100.0)
        engine.update_camera_3d(view, proj)

    def draw():
        engine.cls(50, 50, 70)
        if vrm_id is not None:
            engine.draw_vrm(vrm_id)
        else:
            engine.rect(100, 100, 200, 200, 255, 0, 0, 255)

    engine.run(update, draw)

if __name__ == "__main__":
    main()
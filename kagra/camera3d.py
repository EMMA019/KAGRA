# kagra/camera3d.py
"""Camera3D - KAGRA Phase 11 3D カメラ（wgpu 対応版）"""

from __future__ import annotations
import math


def _look_at(eye, target, up) -> list:
    ex, ey, ez = eye
    tx, ty, tz = target
    ux, uy, uz = up

    # forward（カメラ → ターゲット方向、右手系）
    fx = tx - ex; fy = ty - ey; fz = tz - ez
    fl = math.sqrt(fx*fx + fy*fy + fz*fz) or 1e-8
    fx /= fl; fy /= fl; fz /= fl

    # right = forward × up
    rx = fy*uz - fz*uy
    ry = fz*ux - fx*uz
    rz = fx*uy - fy*ux
    rl = math.sqrt(rx*rx + ry*ry + rz*rz) or 1e-8
    rx /= rl; ry /= rl; rz /= rl

    # true_up = right × forward
    ux2 = ry*fz - rz*fy
    uy2 = rz*fx - rx*fz
    uz2 = rx*fy - ry*fx

    # 行優先 View 行列。列優先への変換はエンジン側 (update_camera_3d) が行う。
    # [  r.x   r.y   r.z  -dot(r,eye)  ]
    # [ u2.x  u2.y  u2.z  -dot(u2,eye) ]
    # [ -f.x  -f.y  -f.z   dot(f,eye)  ]
    # [  0     0     0      1           ]
    return [
        rx,  ry,  rz,  -(rx*ex + ry*ey + rz*ez),
        ux2, uy2, uz2, -(ux2*ex + uy2*ey + uz2*ez),
        -fx, -fy, -fz,  (fx*ex + fy*ey + fz*ez),
        0,   0,   0,    1,
    ]


def _perspective_wgpu(fov_deg: float, aspect: float,
                      near: float, far: float) -> list:
    """wgpu 用 Projection 行列（深度 0〜1、行優先）。"""
    f = 1.0 / math.tan(math.radians(fov_deg) * 0.5)
    # wgpu NDC: X 右向き、Y 上向き、Z 0(near)→1(far)
    return [
        f / aspect, 0,  0,                         0,
        0,          f,  0,                         0,
        0,          0,  far / (near - far),        (near * far) / (near - far),
        0,          0, -1,                         0,
    ]


class Camera3D:
    """
    3D カメラ。

    update() を一度呼ぶとエンジン組み込みカメラは無効になり、以降このクラスが
    カメラの権威になる。engine.zoom_camera() 等の組み込み操作とは併用できない
    （併用した場合はこちらが優先され、警告がログに出る）。

    Example::
        cam = Camera3D(1280, 720)
        cam.use_orbit(radius=2.5, target=(0, 0.9, 0))
        cam.update(kagra.get_engine())   # update() 内で毎フレーム呼ぶ
    """

    def __init__(self, screen_w=1280, screen_h=720,
                 fov_deg=30.0, near=0.01, far=1000.0):
        self.screen_w  = screen_w
        self.screen_h  = screen_h
        self.fov_deg   = fov_deg
        self.near      = near
        self.far       = far

        self.position  = (0.0, 1.0, 3.0)
        self.target    = (0.0, 1.0, 0.0)
        self.up        = (0.0, 1.0, 0.0)

        # 軌道カメラ
        self._orbit    = False
        self.orbit_r   = 3.0
        self.orbit_th  = 0.0    # 水平角（rad）
        self.orbit_phi = 0.2    # 仰角（rad）
        self.orbit_tgt = (0.0, 0.9, 0.0)

    def use_orbit(self, radius=3.0, theta=0.0, phi=0.2,
                  target=(0.0, 0.9, 0.0)):
        self._orbit    = True
        self.orbit_r   = radius
        self.orbit_th  = theta
        self.orbit_phi = phi
        self.orbit_tgt = target

    def orbit_by(self, d_theta: float, d_phi: float):
        self.orbit_th  += d_theta
        self.orbit_phi  = max(-1.4, min(1.4, self.orbit_phi + d_phi))

    def zoom(self, delta: float):
        self.orbit_r = max(0.3, self.orbit_r + delta)

    def _update_orbit(self):
        r  = self.orbit_r
        th = self.orbit_th
        ph = self.orbit_phi
        tx, ty, tz = self.orbit_tgt
        x = tx + r * math.cos(ph) * math.sin(th)
        y = ty + r * math.sin(ph)
        z = tz + r * math.cos(ph) * math.cos(th)
        self.position = (x, y, z)
        self.target   = self.orbit_tgt

    def update(self, kagra_engine):
        if self._orbit:
            self._update_orbit()
        view = _look_at(self.position, self.target, self.up)
        proj = _perspective_wgpu(
            self.fov_deg,
            self.screen_w / max(1, self.screen_h),
            self.near, self.far,
        )
        kagra_engine.update_camera_3d(view, proj)

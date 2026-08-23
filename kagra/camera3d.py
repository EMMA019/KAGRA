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


def _mat4_mul(a, b) -> list:
    out = [0.0] * 16
    for r in range(4):
        for c in range(4):
            s = 0.0
            for k in range(4):
                s += a[r * 4 + k] * b[k * 4 + c]
            out[r * 4 + c] = s
    return out


def _mat4_mul_vec(m, v) -> list:
    x, y, z, w = v
    return [
        m[0] * x + m[1] * y + m[2] * z + m[3] * w,
        m[4] * x + m[5] * y + m[6] * z + m[7] * w,
        m[8] * x + m[9] * y + m[10] * z + m[11] * w,
        m[12] * x + m[13] * y + m[14] * z + m[15] * w,
    ]


def _mat4_inv(m) -> list | None:
    a = [list(m[i * 4:(i + 1) * 4]) + [1.0 if i == j else 0.0 for j in range(4)]
         for i in range(4)]
    for i in range(4):
        piv = i
        for r in range(i + 1, 4):
            if abs(a[r][i]) > abs(a[piv][i]):
                piv = r
        if abs(a[piv][i]) < 1e-12:
            return None
        a[i], a[piv] = a[piv], a[i]
        div = a[i][i]
        for c in range(8):
            a[i][c] /= div
        for r in range(4):
            if r == i:
                continue
            f = a[r][i]
            for c in range(8):
                a[r][c] -= f * a[i][c]
    out = []
    for i in range(4):
        out.extend(a[i][4:8])
    return out


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

    def view_matrix(self) -> list:
        """現在の行優先 view（16 要素）。"""
        if self._orbit:
            self._update_orbit()
        return _look_at(self.position, self.target, self.up)

    def proj_matrix(self) -> list:
        """現在の行優先 proj（16 要素、wgpu 深度 0〜1）。"""
        return _perspective_wgpu(
            self.fov_deg,
            self.screen_w / max(1, self.screen_h),
            self.near, self.far,
        )

    def ray_from_screen(self, sx: float, sy: float):
        """スクリーン座標からワールド空間のレイ (origin, direction) を返す。

        ``sx, sy`` はピクセル。原点は左上。方向は正規化済み。
        行列が特異なら None。
        """
        view = self.view_matrix()
        proj = self.proj_matrix()
        vp = _mat4_mul(proj, view)
        inv = _mat4_inv(vp)
        if inv is None:
            return None
        w = max(1.0, float(self.screen_w))
        h = max(1.0, float(self.screen_h))
        ndc_x = 2.0 * float(sx) / w - 1.0
        ndc_y = 1.0 - 2.0 * float(sy) / h
        near = _mat4_mul_vec(inv, (ndc_x, ndc_y, 0.0, 1.0))
        far = _mat4_mul_vec(inv, (ndc_x, ndc_y, 1.0, 1.0))
        if abs(near[3]) < 1e-8 or abs(far[3]) < 1e-8:
            return None
        nx, ny, nz = near[0] / near[3], near[1] / near[3], near[2] / near[3]
        fx, fy, fz = far[0] / far[3], far[1] / far[3], far[2] / far[3]
        dx, dy, dz = fx - nx, fy - ny, fz - nz
        length = math.sqrt(dx * dx + dy * dy + dz * dz)
        if length < 1e-8:
            return None
        return (nx, ny, nz), (dx / length, dy / length, dz / length)

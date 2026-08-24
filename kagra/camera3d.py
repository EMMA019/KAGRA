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
        self._showcase = False
        self._show: dict = {}
        self._show_t = 0.0
        self._follow = False

    def use_orbit(self, radius=3.0, theta=0.0, phi=0.2,
                  target=(0.0, 0.9, 0.0)):
        self._orbit    = True
        self._showcase = False
        self._follow   = False
        self.orbit_r   = radius
        self.orbit_th  = theta
        self.orbit_phi = phi
        self.orbit_tgt = target

    def use_showcase(
        self,
        *,
        body_radius: float = 3.35,
        face_radius: float = 1.62,
        body_target_y: float = 0.84,
        face_target_y: float = 1.30,
        body_fov: float = 32.0,
        face_fov: float = 26.0,
        orbit_speed: float = 0.18,
        cut_period: float = 6.5,
        blend_sec: float = 1.35,
        theta: float = 0.0,
        phi: float = 0.14,
    ):
        """ライブデモ用。低速オービット + 全身⇔顔寄りのカット割り。

        ``update(engine, dt)`` に dt を渡すと進む。``--no-orbit`` では使わない。
        """
        self._showcase = True
        self._orbit = True
        self._follow = False
        self._show_t = 0.0
        self._show = {
            "body_radius": float(body_radius),
            "face_radius": float(face_radius),
            "body_target_y": float(body_target_y),
            "face_target_y": float(face_target_y),
            "body_fov": float(body_fov),
            "face_fov": float(face_fov),
            "orbit_speed": float(orbit_speed),
            "cut_period": float(cut_period),
            "blend_sec": float(blend_sec),
        }
        self.orbit_th = float(theta)
        self.orbit_phi = float(phi)
        self._apply_showcase(0.0)

    def showcase_tick(self, dt: float) -> float:
        """カットを進めて blend（0=全身, 1=顔）を返す。"""
        if not self._showcase:
            return 0.0
        self._show_t += max(0.0, float(dt))
        self.orbit_th += self._show.get("orbit_speed", 0.18) * dt
        return self._apply_showcase(self._show_t)

    def _apply_showcase(self, t: float) -> float:
        from kagra.look import showcase_blend, showcase_params

        s = self._show
        u = showcase_blend(
            t,
            period=s.get("cut_period", 6.5),
            blend=s.get("blend_sec", 1.35),
        )
        p = showcase_params(
            u,
            body_radius=s.get("body_radius", 3.35),
            face_radius=s.get("face_radius", 1.62),
            body_target_y=s.get("body_target_y", 0.84),
            face_target_y=s.get("face_target_y", 1.30),
            body_fov=s.get("body_fov", 32.0),
            face_fov=s.get("face_fov", 26.0),
        )
        tx, _, tz = self.orbit_tgt
        self.orbit_r = p["radius"]
        self.orbit_tgt = (tx, p["target_y"], tz)
        self.fov_deg = p["fov"]
        return u

    def follow(
        self,
        x: float,
        y: float,
        z: float,
        *,
        distance: float = 4.8,
        height: float = 2.4,
        look_y: float = 1.0,
        lerp: float = 0.18,
        yaw: float = 0.0,
        bounds_half: float | None = None,
    ):
        """ワールド上の点を追うチェイスカメラ。orbit / showcase は切る。

        ``yaw`` はプレイヤーの向き（``atan2(dx, dz)``）。カメラは後ろ上。
        ``lerp=1`` で瞬間移動。毎フレーム呼んでから ``update(engine)``。
        ``bounds_half`` があれば目の XZ を箱部屋の内側にクランプする
        （既定 distance が壁の外に出る Switch / Dodge 用）。
        """
        self._orbit = False
        self._showcase = False
        self._follow = True
        tx = float(x)
        ty = float(y) + float(look_y)
        tz = float(z)
        bx = float(x) - math.sin(float(yaw)) * float(distance)
        bz = float(z) - math.cos(float(yaw)) * float(distance)
        by = float(y) + float(height)
        if bounds_half is not None:
            lim = max(0.05, float(bounds_half) - 0.15)
            bx = max(-lim, min(lim, bx))
            bz = max(-lim, min(lim, bz))
        t = max(0.0, min(1.0, float(lerp)))
        if t >= 1.0:
            self.position = (bx, by, bz)
            self.target = (tx, ty, tz)
            return
        px, py, pz = self.position
        ox, oy, oz = self.target
        self.position = (
            px + (bx - px) * t,
            py + (by - py) * t,
            pz + (bz - pz) * t,
        )
        self.target = (
            ox + (tx - ox) * t,
            oy + (ty - oy) * t,
            oz + (tz - oz) * t,
        )

    def look(
        self,
        x: float,
        y: float,
        z: float,
        tx: float,
        ty: float,
        tz: float,
    ):
        """位置と注視点を直接置く。orbit / showcase / follow は切る。"""
        self._orbit = False
        self._showcase = False
        self._follow = False
        self.position = (float(x), float(y), float(z))
        self.target = (float(tx), float(ty), float(tz))

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

    def update(self, kagra_engine, dt: float | None = None):
        if dt is not None and self._showcase:
            self.showcase_tick(dt)
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

    def world_to_screen(self, wx: float, wy: float, wz: float):
        """ワールド座標 → スクリーンピクセル (sx, sy)。左上が原点。

        カメラの後ろ、またはクリップ外なら None。
        """
        view = self.view_matrix()
        proj = self.proj_matrix()
        vp = _mat4_mul(proj, view)
        x = vp[0] * wx + vp[1] * wy + vp[2] * wz + vp[3]
        y = vp[4] * wx + vp[5] * wy + vp[6] * wz + vp[7]
        z = vp[8] * wx + vp[9] * wy + vp[10] * wz + vp[11]
        w = vp[12] * wx + vp[13] * wy + vp[14] * wz + vp[15]
        if abs(w) < 1e-5:
            return None
        ndc_x, ndc_y, ndc_z = x / w, y / w, z / w
        if ndc_z < 0.0 or ndc_z > 1.0:
            return None
        sx = (ndc_x * 0.5 + 0.5) * self.screen_w
        sy = (1.0 - (ndc_y * 0.5 + 0.5)) * self.screen_h
        return sx, sy

# kagra/vrm_lookat.py
"""
VRM 視線追従（Look-at）コントローラー

キャラクターの目・首・頭を3D空間の任意の点に向ける。
ブレンドシェイプ（LookLeft/Right/Up/Down）とボーン回転の両方に対応。
VRM LookAt メタ（range map）がある場合は inputMax / outputScale でスケールする。

Example::
    from kagra.vrm_lookat import LookAtController

    lookat = LookAtController(avatar)

    # 毎フレーム
    lookat.look_at_3d(target_x, target_y, target_z)
    lookat.update(dt)

    # スクリーン座標から（カメラの手前にターゲットを置く）
    mx, my = kagra.mouse()
    lookat.look_at_screen(mx, my, screen_w=1280, screen_h=720)

    # 無効化（前を向く）
    lookat.reset()
"""
from __future__ import annotations
import math
from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.vrm_avatar import VrmAvatar


# ── 内部ユーティリティ ────────────────────────────────────────

def _qmul(a: list, b: list) -> list:
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]


def _euler_to_quat(rx: float, ry: float, rz: float) -> list:
    cx, sx = math.cos(rx / 2), math.sin(rx / 2)
    cy, sy = math.cos(ry / 2), math.sin(ry / 2)
    cz, sz = math.cos(rz / 2), math.sin(rz / 2)
    return [
        sx * cy * cz + cx * sy * sz,
        cx * sy * cz - sx * cy * sz,
        cx * cy * sz + sx * sy * cz,
        cx * cy * cz - sx * sy * sz,
    ]

def _slerp(a: list, b: list, t: float) -> list:
    dot = sum(a[i] * b[i] for i in range(4))
    if dot < 0:
        b = [-x for x in b]
        dot = -dot
    dot = min(1.0, dot)
    if dot > 0.9995:
        r = [a[i] + t * (b[i] - a[i]) for i in range(4)]
        l = math.sqrt(sum(x * x for x in r)) or 1e-8
        return [x / l for x in r]
    th0 = math.acos(dot)
    th = th0 * t
    sa = math.sin(th0 - th) / math.sin(th0)
    sb = math.sin(th) / math.sin(th0)
    return [sa * a[i] + sb * b[i] for i in range(4)]

_ID_QUAT = [0.0, 0.0, 0.0, 1.0]


def _apply_range_map(angle_rad: float, input_max: float, output_scale: float) -> float:
    """角度(rad)を VRM range map でスケールして rad で返す。

    clamp(|deg|, 0..inputMax) / inputMax * outputScale（符号維持）。
    """
    if input_max <= 1e-6:
        return 0.0
    deg = math.degrees(angle_rad)
    sign = 1.0 if deg >= 0.0 else -1.0
    t = min(1.0, abs(deg) / input_max)
    return math.radians(sign * t * output_scale)


class LookAtController:
    """VRM キャラクターの視線・頭部追従コントローラー。

    Args:
        avatar:       VrmAvatar インスタンス
        eye_height:   頭の高さ（ワールド座標 Y）。モデルに合わせて調整。デフォルト 1.5
        head_weight:  頭部ボーンへの適用ウェイト（0.0=首のみ, 1.0=頭も最大）
        neck_weight:  首ボーンへの適用ウェイト
        smooth_speed: 視線の追従速度（ラジアン/秒）。0 で即時。

    Example::
        lookat = LookAtController(avatar, eye_height=1.55, smooth_speed=5.0)
    """

    # 可動域制限（ラジアン）— メタ未定義時のフォールバック
    MAX_YAW_HEAD   = math.radians(50)   # 左右（頭）
    MAX_PITCH_HEAD = math.radians(30)   # 上下（頭）
    MAX_YAW_NECK   = math.radians(30)   # 左右（首）
    MAX_PITCH_NECK = math.radians(20)   # 上下（首）
    MAX_EYE_YAW    = math.radians(20)   # 左右（目ブレンドシェイプ）
    MAX_EYE_PITCH  = math.radians(15)   # 上下（目ブレンドシェイプ）

    def __init__(
        self,
        avatar: "VrmAvatar",
        eye_height: float = 1.5,
        head_weight: float = 0.6,
        neck_weight: float = 0.4,
        smooth_speed: float = 6.0,
    ):
        self._avatar      = avatar
        self.eye_height   = eye_height
        self.head_weight  = head_weight
        self.neck_weight  = neck_weight
        self.smooth_speed = smooth_speed
        self.enabled      = True
        # False なら目だけ。ダンスの頭・首を潰さない。
        self.apply_bones  = True

        # 現在の目標角度
        self._target_yaw:   float = 0.0
        self._target_pitch: float = 0.0

        # 現在の補間済み角度
        self._cur_yaw:   float = 0.0
        self._cur_pitch: float = 0.0

        # VRM LookAt メタ（あれば range map でスケール）
        self._meta: Optional[dict] = None
        self.look_at_type: str = "bone"
        try:
            import kagra
            meta = kagra.get_vrm_look_at(avatar.vrm_id)
            if meta:
                self._meta = meta
                self.look_at_type = meta.get("type", "bone")
                off = meta.get("offsetFromHeadBone") or [0.0, 0.0, 0.0]
                print(
                    f"[LookAt] meta type={self.look_at_type} "
                    f"offset={off} "
                    f"HOuter={meta['rangeMapHorizontalOuter']['outputScale']}deg"
                )
        except Exception as e:
            print(f"[LookAt] meta skipped: {e}")

        # ブレンドシェイプ対応確認
        import kagra
        shapes = set(kagra.list_blend_shapes(avatar.vrm_id))
        self._bs_left  = next((s for s in ["LookLeft",  "lookLeft",  "Fcl_EYE_Direction_Left"]  if s in shapes), None)
        self._bs_right = next((s for s in ["LookRight", "lookRight", "Fcl_EYE_Direction_Right"] if s in shapes), None)
        self._bs_up    = next((s for s in ["LookUp",    "lookUp",    "Fcl_EYE_Direction_Up"]    if s in shapes), None)
        self._bs_down  = next((s for s in ["LookDown",  "lookDown", "Fcl_EYE_Direction_Down"]  if s in shapes), None)

        has_bs = any([self._bs_left, self._bs_right, self._bs_up, self._bs_down])
        print(f"[LookAt] eye_blend_shapes={'OK' if has_bs else 'なし（ボーンのみ）'}")

    def _scale_yaw_pitch(self, yaw: float, pitch: float) -> tuple[float, float]:
        """メタ range map があれば yaw/pitch をスケール。なければそのまま。"""
        if not self._meta:
            return yaw, pitch
        hi = self._meta["rangeMapHorizontalInner"]
        ho = self._meta["rangeMapHorizontalOuter"]
        vd = self._meta["rangeMapVerticalDown"]
        vu = self._meta["rangeMapVerticalUp"]
        # 右(+yaw) → Outer, 左(-yaw) → Inner（簡易）
        if yaw >= 0.0:
            yaw = _apply_range_map(yaw, ho["inputMaxValue"], ho["outputScale"])
        else:
            yaw = _apply_range_map(yaw, hi["inputMaxValue"], hi["outputScale"])
        # 下(+pitch) → Down, 上(-pitch) → Up
        if pitch >= 0.0:
            pitch = _apply_range_map(pitch, vd["inputMaxValue"], vd["outputScale"])
        else:
            pitch = _apply_range_map(pitch, vu["inputMaxValue"], vu["outputScale"])
        return yaw, pitch

    # ── ターゲット設定 ─────────────────────────────────────────

    def look_at_3d(self, tx: float, ty: float, tz: float,
                   avatar_x: float = 0.0, avatar_z: float = 0.0):
        """3D ワールド座標を見る。

        Args:
            tx, ty, tz:         ターゲット位置
            avatar_x, avatar_z: アバターのワールド XZ 座標（デフォルト原点）
        """
        if not self.enabled:
            return
        hx = avatar_x
        hy = self.eye_height
        hz = avatar_z
        if self._meta:
            off = self._meta.get("offsetFromHeadBone") or [0.0, 0.0, 0.0]
            hx += float(off[0])
            hy += float(off[1])
            hz += float(off[2])

        dx = tx - hx
        dy = ty - hy
        dz = tz - hz

        dist_xz = math.sqrt(dx * dx + dz * dz)
        self._target_yaw   = math.atan2(dx, dz)          # 横方向
        self._target_pitch = -math.atan2(dy, dist_xz)    # 縦方向（下向き正）

    def look_at_screen(self, sx: float, sy: float,
                        screen_w: float = 1280, screen_h: float = 720,
                        fov_deg: float = 45.0):
        """スクリーン座標を見る（2D 的な指定）。

        スクリーン中央が正面、端が最大角度になる。

        Args:
            sx, sy:       スクリーン座標（ピクセル）
            screen_w/h:   画面解像度
            fov_deg:      仮想 FOV（角度が大きいほど敏感）
        """
        if not self.enabled:
            return
        nx = (sx / screen_w - 0.5) * 2.0   # -1.0 〜 1.0
        ny = (sy / screen_h - 0.5) * 2.0   # -1.0 〜 1.0（下が正）

        half_fov = math.radians(fov_deg / 2)
        self._target_yaw   = nx * half_fov
        self._target_pitch = ny * half_fov * 0.6   # 縦は少し抑える

    def reset(self):
        """正面を向く（ターゲットをリセット）。"""
        self._target_yaw   = 0.0
        self._target_pitch = 0.0

    def _apply_bone_look(self, name: str, q_look: list):
        """アニメの現在回転に Look デルタを右から掛ける。"""
        cur = None
        anim = getattr(self._avatar, "_anim", None)
        if anim is not None:
            cur = anim.current_rots.get(name)
        if cur is None:
            binds = getattr(self._avatar, "_bind_rots", None) or {}
            cur = binds.get(name, _ID_QUAT)
        self._avatar._send_bone(name, _qmul(list(cur), q_look))

    # ── 毎フレーム更新 ────────────────────────────────────────

    def update(self, dt: float):
        """毎フレーム呼ぶ。ボーン・ブレンドシェイプに適用する。"""
        if not self.enabled:
            return

        # スムーズ追従
        if self.smooth_speed > 0:
            spd = self.smooth_speed * dt
            dy = self._target_yaw   - self._cur_yaw
            dp = self._target_pitch - self._cur_pitch
            self._cur_yaw   += dy * min(1.0, spd)
            self._cur_pitch += dp * min(1.0, spd)
        else:
            self._cur_yaw   = self._target_yaw
            self._cur_pitch = self._target_pitch

        yaw, pitch = self._scale_yaw_pitch(self._cur_yaw, self._cur_pitch)

        if self.apply_bones:
            # ダンスの頭を潰さず、現在ポーズに視線を足す
            neck_yaw   = max(-self.MAX_YAW_NECK,   min(self.MAX_YAW_NECK,   yaw   * self.neck_weight))
            neck_pitch = max(-self.MAX_PITCH_NECK,  min(self.MAX_PITCH_NECK, pitch * self.neck_weight))
            self._apply_bone_look("J_Bip_C_Neck", _euler_to_quat(neck_pitch, neck_yaw, 0.0))

            head_yaw   = max(-self.MAX_YAW_HEAD,   min(self.MAX_YAW_HEAD,   yaw   * self.head_weight))
            head_pitch = max(-self.MAX_PITCH_HEAD,  min(self.MAX_PITCH_HEAD, pitch * self.head_weight))
            self._apply_bone_look("J_Bip_C_Head", _euler_to_quat(head_pitch, head_yaw, 0.0))

        # ── 目ブレンドシェイプ ────────────────────────────────
        # expression タイプ、または Look* シェイプがある場合に適用
        eye_yaw   = max(-self.MAX_EYE_YAW,   min(self.MAX_EYE_YAW,   yaw))
        eye_pitch = max(-self.MAX_EYE_PITCH,  min(self.MAX_EYE_PITCH, pitch))

        left_w  = max(0.0, -eye_yaw   / self.MAX_EYE_YAW)
        right_w = max(0.0,  eye_yaw   / self.MAX_EYE_YAW)
        up_w    = max(0.0, -eye_pitch / self.MAX_EYE_PITCH)
        down_w  = max(0.0,  eye_pitch / self.MAX_EYE_PITCH)

        import kagra
        vid = self._avatar.vrm_id
        if self._bs_left:  kagra.get_engine().set_blend_shape(vid, self._bs_left,  left_w)
        if self._bs_right: kagra.get_engine().set_blend_shape(vid, self._bs_right, right_w)
        if self._bs_up:    kagra.get_engine().set_blend_shape(vid, self._bs_up,    up_w)
        if self._bs_down:  kagra.get_engine().set_blend_shape(vid, self._bs_down,  down_w)

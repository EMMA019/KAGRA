# kagra/vrm_ik.py
"""
VRM 2ボーン IK（逆運動学）システム

腕・脚などのボーンチェーンを使って、手や足を3D空間の任意の点に到達させる。
CCD (Cyclic Coordinate Descent) アルゴリズムの簡易実装。

VRM ボーン命名規則に基づいてデフォルト設定済み:
  右腕: Shoulder → UpperArm → LowerArm → Hand
  左腕: 同左対称

Example::
    from kagra.vrm_ik import ArmIK

    ik = ArmIK(avatar)

    def update(dt):
        # 右手をターゲット位置へ
        ik.reach_right(tx=0.5, ty=1.0, tz=0.3)

        # 左手を特定位置へ
        ik.reach_left(tx=-0.4, ty=0.9, tz=0.2)

        # ウェイト（0=IKなし, 1=IK全力）でアニメとブレンド
        ik.set_weight(0.8)

    # 無効化
    ik.enabled = False
"""
from __future__ import annotations
import math
from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.vrm_avatar import VrmAvatar


# ── クォータニオン・ベクトル ユーティリティ ──────────────────

def _add3(a, b): return [a[0]+b[0], a[1]+b[1], a[2]+b[2]]
def _sub3(a, b): return [a[0]-b[0], a[1]-b[1], a[2]-b[2]]
def _scale3(v, s): return [v[0]*s, v[1]*s, v[2]*s]
def _len3(v): return math.sqrt(v[0]**2 + v[1]**2 + v[2]**2)
def _norm3(v):
    l = _len3(v)
    return [v[0]/l, v[1]/l, v[2]/l] if l > 1e-8 else [0, 1, 0]
def _dot3(a, b): return a[0]*b[0] + a[1]*b[1] + a[2]*b[2]
def _cross3(a, b):
    return [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]

def _qmul(a, b):
    ax, ay, az, aw = a; bx, by, bz, bw = b
    return [aw*bx+ax*bw+ay*bz-az*by,
            aw*by-ax*bz+ay*bw+az*bx,
            aw*bz+ax*by-ay*bx+az*bw,
            aw*bw-ax*bx-ay*by-az*bz]

def _qnorm(q):
    l = math.sqrt(sum(x*x for x in q))
    return [x/l for x in q] if l > 1e-8 else [0, 0, 0, 1]

def _q_from_to(f, t):
    """2ベクトル間の最短回転クォータニオン [x,y,z,w]"""
    f = _norm3(f); t = _norm3(t)
    d = _dot3(f, t)
    if d > 0.9999: return [0, 0, 0, 1]
    if d < -0.9999:
        perp = _cross3(f, [1, 0, 0])
        if _len3(perp) < 0.001: perp = _cross3(f, [0, 1, 0])
        p = _norm3(perp)
        return _qnorm([p[0], p[1], p[2], 0.0])
    ax = _norm3(_cross3(f, t))
    ang = math.acos(max(-1.0, min(1.0, d)))
    s = math.sin(ang / 2)
    return _qnorm([ax[0]*s, ax[1]*s, ax[2]*s, math.cos(ang/2)])

def _euler_to_quat(rx, ry, rz):
    cx, sx = math.cos(rx/2), math.sin(rx/2)
    cy, sy = math.cos(ry/2), math.sin(ry/2)
    cz, sz = math.cos(rz/2), math.sin(rz/2)
    return [sx*cy*cz+cx*sy*sz, cx*sy*cz-sx*cy*sz,
            cx*cy*sz+sx*sy*cz, cx*cy*cz-sx*sy*sz]

def _slerp(a, b, t):
    dot = sum(a[i]*b[i] for i in range(4))
    if dot < 0: b = [-x for x in b]; dot = -dot
    dot = min(1.0, dot)
    if dot > 0.9995:
        r = [a[i]+t*(b[i]-a[i]) for i in range(4)]
        l = math.sqrt(sum(x*x for x in r)) or 1e-8
        return [x/l for x in r]
    th0 = math.acos(dot); th = th0*t
    sa = math.sin(th0-th)/math.sin(th0); sb = math.sin(th)/math.sin(th0)
    return [sa*a[i]+sb*b[i] for i in range(4)]

_ID = [0.0, 0.0, 0.0, 1.0]


# ── 2ボーン IK ────────────────────────────────────────────────

class TwoBoneIK:
    """2ボーン（上腕・前腕）の解析的 IK ソルバー。

    FABRIK より軽量で、腕の IK に特化した実装。
    肘の曲がり方向を pole_vector で制御できる。

    Args:
        upper_bone:  上腕ボーン名 (例: "J_Bip_R_UpperArm")
        lower_bone:  前腕ボーン名 (例: "J_Bip_R_LowerArm")
        upper_len:   上腕の長さ（ワールド単位）
        lower_len:   前腕の長さ（ワールド単位）
        origin:      上腕の付け根位置 [x,y,z]（ワールド座標、モデルに合わせる）
        pole:        肘の曲がり方向 [x,y,z] (デフォルト: 肘が後ろに曲がる)
    """

    def __init__(
        self,
        upper_bone: str,
        lower_bone: str,
        upper_len:  float = 0.28,
        lower_len:  float = 0.26,
        origin:     list  = None,
        pole:       list  = None,
    ):
        self.upper_bone = upper_bone
        self.lower_bone = lower_bone
        self.upper_len  = upper_len
        self.lower_len  = lower_len
        self.origin     = origin or [0.0, 1.2, 0.0]
        self.pole       = pole   or [0.0, 0.0, -1.0]   # 肘が前方向

    def solve(self, target: list) -> tuple[list, list]:
        """IK を解いて (上腕回転quat, 前腕回転quat) を返す。

        Args:
            target: ターゲット位置 [x, y, z]

        Returns:
            (upper_quat, lower_quat): 各ボーンに設定する回転クォータニオン
        """
        L1 = self.upper_len
        L2 = self.lower_len
        origin = self.origin

        # ターゲットまでのベクトル
        to_target = _sub3(target, origin)
        dist      = _len3(to_target)
        max_reach = L1 + L2

        # 届かない場合はまっすぐ伸ばす
        if dist < 0.001:
            return _ID, _euler_to_quat(0, 0, -1.2)  # 曲げた状態
        if dist >= max_reach * 0.999:
            dir_ = _norm3(to_target)
            q_upper = _q_from_to([0, -1, 0], dir_)
            return q_upper, _ID

        # コサイン余弦定理で肘角度を求める
        cos_angle = (L1*L1 + dist*dist - L2*L2) / (2 * L1 * dist)
        cos_angle = max(-1.0, min(1.0, cos_angle))
        angle1    = math.acos(cos_angle)   # 上腕が伸びる方向とのなす角

        cos_elbow = (L1*L1 + L2*L2 - dist*dist) / (2 * L1 * L2)
        cos_elbow = max(-1.0, min(1.0, cos_elbow))
        elbow_angle = math.pi - math.acos(cos_elbow)   # 前腕の折れ角

        # Pole vector: 肘の方向を計算
        dir_to_target = _norm3(to_target)
        pole          = _norm3(self.pole)

        # pole をターゲット軸に垂直な成分に射影
        pole_proj = _sub3(pole, _scale3(dir_to_target, _dot3(pole, dir_to_target)))
        if _len3(pole_proj) < 0.001:
            pole_proj = [0, 0, -1]
        pole_dir = _norm3(pole_proj)

        # 上腕の方向 = ターゲット方向を angle1 だけ pole_dir 方向に曲げる
        # 回転軸 = dir_to_target × pole_dir
        rot_axis = _norm3(_cross3(dir_to_target, pole_dir))

        # angle1 だけ rot_axis 周りに dir_to_target を回転
        s = math.sin(angle1)
        c = math.cos(angle1)
        d = _dot3(dir_to_target, rot_axis)
        upper_dir = [
            dir_to_target[i] * c + _cross3(rot_axis, dir_to_target)[i] * s + rot_axis[i] * d * (1 - c)
            for i in range(3)
        ]
        upper_dir = _norm3(upper_dir)

        # 上腕回転: Y 軸下向き(-Y) → upper_dir
        q_upper = _q_from_to([0, -1, 0], upper_dir)

        # 前腕回転: 上腕ローカル Y 軸下向き → 前腕の方向（ローカル座標で elbow_angle 分曲げる）
        # 簡略化: 前腕は純粋に elbow_angle だけ曲げる（Z 軸周り）
        q_lower = _euler_to_quat(elbow_angle, 0, 0)

        return q_upper, q_lower


# ── ArmIK（高レベルラッパー）─────────────────────────────────

class ArmIK:
    """VRM アバターの腕 IK コントローラー。

    右腕・左腕の IK を管理し、アニメーションとウェイトブレンドできる。

    Args:
        avatar:        VrmAvatar インスタンス
        smooth_speed:  追従スピード（ラジアン/秒）。0 で即時。

    Example::
        ik = ArmIK(avatar)

        def update(dt):
            ik.reach_right(tx=0.5, ty=1.0, tz=0.3, weight=0.8)
            ik.update(dt)
    """

    def __init__(self, avatar: "VrmAvatar", smooth_speed: float = 8.0):
        self._avatar      = avatar
        self.smooth_speed = smooth_speed
        self.enabled      = True

        # 右腕 IK
        self._ik_right = TwoBoneIK(
            upper_bone = "J_Bip_R_UpperArm",
            lower_bone = "J_Bip_R_LowerArm",
            upper_len  = 0.28,
            lower_len  = 0.26,
            origin     = [0.18, 1.35, 0.0],   # 右肩の近似位置
            pole       = [0.0, 0.0, -1.0],    # 肘は前方向
        )
        self._right_target:  Optional[list] = None
        self._right_weight:  float = 1.0
        self._right_cur_q_upper = list(_ID)
        self._right_cur_q_lower = list(_ID)

        # 左腕 IK
        self._ik_left = TwoBoneIK(
            upper_bone = "J_Bip_L_UpperArm",
            lower_bone = "J_Bip_L_LowerArm",
            upper_len  = 0.28,
            lower_len  = 0.26,
            origin     = [-0.18, 1.35, 0.0],  # 左肩の近似位置
            pole       = [0.0, 0.0, -1.0],
        )
        self._left_target:  Optional[list] = None
        self._left_weight:  float = 1.0
        self._left_cur_q_upper = list(_ID)
        self._left_cur_q_lower = list(_ID)

    def reach_right(
        self,
        tx: float, ty: float, tz: float,
        weight: float = 1.0,
    ):
        """右手をターゲット位置へ向ける。

        Args:
            tx, ty, tz: ターゲット位置（ワールド座標）
            weight:     IK の強さ（0.0=アニメのまま, 1.0=IK全力）
        """
        self._right_target = [tx, ty, tz]
        self._right_weight = max(0.0, min(1.0, weight))

    def reach_left(
        self,
        tx: float, ty: float, tz: float,
        weight: float = 1.0,
    ):
        """左手をターゲット位置へ向ける。"""
        self._left_target = [tx, ty, tz]
        self._left_weight = max(0.0, min(1.0, weight))

    def release_right(self):
        """右腕 IK を無効化する。"""
        self._right_target = None

    def release_left(self):
        """左腕 IK を無効化する。"""
        self._left_target = None

    def set_shoulder_height(self, height: float):
        """肩の高さを変更する（アバターのスケールに合わせて調整）。

        Args:
            height: 肩の Y 座標（例: 1.35）
        """
        self._ik_right.origin[1] = height
        self._ik_left.origin[1]  = height

    def set_arm_lengths(self, upper: float, lower: float):
        """腕の長さを変更する（アバターのスケールに合わせて調整）。"""
        for ik in [self._ik_right, self._ik_left]:
            ik.upper_len = upper
            ik.lower_len = lower

    def update(self, dt: float):
        """毎フレーム呼ぶ。IK を解いてボーンに適用する。"""
        if not self.enabled:
            return

        spd = self.smooth_speed * dt if self.smooth_speed > 0 else 1.0

        # 右腕
        if self._right_target is not None and self._right_weight > 0.001:
            q_u, q_l = self._ik_right.solve(self._right_target)
            # スムーズ補間
            self._right_cur_q_upper = _slerp(self._right_cur_q_upper, q_u, min(1.0, spd))
            self._right_cur_q_lower = _slerp(self._right_cur_q_lower, q_l, min(1.0, spd))
            # アニメとブレンド
            anim_q_u = self._avatar._anim.current_rots.get("J_Bip_R_UpperArm", _ID)
            anim_q_l = self._avatar._anim.current_rots.get("J_Bip_R_LowerArm", _ID)
            final_u = _slerp(anim_q_u, self._right_cur_q_upper, self._right_weight)
            final_l = _slerp(anim_q_l, self._right_cur_q_lower, self._right_weight)
            self._avatar._send_bone("J_Bip_R_UpperArm", final_u)
            self._avatar._send_bone("J_Bip_R_LowerArm", final_l)

        # 左腕
        if self._left_target is not None and self._left_weight > 0.001:
            q_u, q_l = self._ik_left.solve(self._left_target)
            self._left_cur_q_upper = _slerp(self._left_cur_q_upper, q_u, min(1.0, spd))
            self._left_cur_q_lower = _slerp(self._left_cur_q_lower, q_l, min(1.0, spd))
            anim_q_u = self._avatar._anim.current_rots.get("J_Bip_L_UpperArm", _ID)
            anim_q_l = self._avatar._anim.current_rots.get("J_Bip_L_LowerArm", _ID)
            final_u = _slerp(anim_q_u, self._left_cur_q_upper, self._left_weight)
            final_l = _slerp(anim_q_l, self._left_cur_q_lower, self._left_weight)
            self._avatar._send_bone("J_Bip_L_UpperArm", final_u)
            self._avatar._send_bone("J_Bip_L_LowerArm", final_l)

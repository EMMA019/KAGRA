# kagra/vrm_anim.py
"""
VRM アニメーションシステム - SLERP補間 + シーケンス再生
Rust GPU スキニング版に対応
"""

from __future__ import annotations
import math
from typing import Optional, Callable


def _quat_dot(a, b):
    return a[0]*b[0] + a[1]*b[1] + a[2]*b[2] + a[3]*b[3]

def _quat_normalize(q):
    l = math.sqrt(q[0]**2 + q[1]**2 + q[2]**2 + q[3]**2)
    if l < 1e-8: return [0, 0, 0, 1]
    return [q[0]/l, q[1]/l, q[2]/l, q[3]/l]

def _slerp(a, b, t):
    """クォータニオンの球面線形補間。"""
    dot = _quat_dot(a, b)
    if dot < 0:
        b = [-b[0], -b[1], -b[2], -b[3]]
        dot = -dot
    dot = min(1.0, dot)
    if dot > 0.9995:
        return _quat_normalize([a[i] + t*(b[i]-a[i]) for i in range(4)])
    theta0 = math.acos(dot)
    theta  = theta0 * t
    sin0   = math.sin(theta0)
    sa = math.sin(theta0 - theta) / sin0
    sb = math.sin(theta)          / sin0
    return [sa*a[i] + sb*b[i] for i in range(4)]

def _euler_to_quat(rx, ry, rz):
    """オイラー角（ラジアン XYZ）→ クォータニオン xyzw。"""
    cx, sx = math.cos(rx/2), math.sin(rx/2)
    cy, sy = math.cos(ry/2), math.sin(ry/2)
    cz, sz = math.cos(rz/2), math.sin(rz/2)
    return [
        sx*cy*cz + cx*sy*sz,
        cx*sy*cz - sx*cy*sz,
        cx*cy*sz + sx*sy*cz,
        cx*cy*cz - sx*sy*sz,
    ]

# ── バインドポーズ（リセット用）キャッシュ ───────────────────
# Rust 側はボーンのバインド回転を保持しているが
# Python 側では「現在の回転」だけを管理する
_IDENTITY_QUAT = [0.0, 0.0, 0.0, 1.0]


class PoseKeyframe:
    """1キーフレーム：ボーンごとの目標回転（オイラー角）と補間時間。"""
    def __init__(self, bones: dict[str, tuple], duration: float):
        """
        Args:
            bones:    {bone_name: (rx, ry, rz)} オイラー角（ラジアン）
                      空の dict を渡すとバインドポーズへ戻る
            duration: このキーフレームまでの補間時間（秒）
        """
        self.quats:    dict[str, list] = {
            name: _euler_to_quat(*rot) for name, rot in bones.items()
        }
        self.duration  = duration
        self.is_bind   = (len(bones) == 0)  # 空 = バインドポーズへ戻る


class VrmAnimator:
    """
    VRM モデルのアニメーションコントローラ。
    Rust GPU スキニング版に対応。

    Example::
        anim = VrmAnimator(vrm_id, kagra_engine)
        anim.play("kiss")

        # 毎フレーム
        anim.update(dt)
        kagra.draw_vrm(vrm_id)   # Rust GPU スキニングで描画
    """

    def __init__(self, vrm_id: int, engine):
        """
        Args:
            vrm_id: kagra.load_vrm() の戻り値
            engine: kagra._engine（Rust エンジンインスタンス）
        """
        self.vrm_id  = vrm_id
        self.engine  = engine
        self._clips: dict[str, list[PoseKeyframe]] = {}

        # 現在の各ボーンのクォータニオン（補間用）
        self._current_rots: dict[str, list] = {}

        self._playing    = False
        self._clip_name  = ""
        self._frames:    list[PoseKeyframe] = []
        self._frame_idx  = 0
        self._t          = 0.0
        self._from_rots: dict[str, list] = {}
        self._on_finish: Optional[Callable] = None
        self._loop       = False

        # ── デフォルトクリップ ──────────────────────────────
        self._register_defaults()

    def _register_defaults(self):
        """標準クリップを登録する。"""

        # バインドポーズに戻す
        self.add_clip("bind", [PoseKeyframe({}, duration=0.3)])

        # 両腕を上げる
        self.add_clip("arm_up", [
            PoseKeyframe({
                "J_Bip_L_UpperArm": (0, 0, -1.5),
                "J_Bip_R_UpperArm": (0, 0,  1.5),
                "J_Bip_L_LowerArm": (0, 0, -0.3),
                "J_Bip_R_LowerArm": (0, 0,  0.3),
            }, duration=0.4),
        ])

        # 片手投げキッス
        self.add_clip("kiss", [
            PoseKeyframe({
                "J_Bip_C_Spine":      (-0.15, 0,    0),
                "J_Bip_C_UpperChest": (-0.20, 0,  0.1),
                "J_Bip_R_UpperArm":   (-1.6,  0,  0.4),
                "J_Bip_R_LowerArm":   (-1.2,  0,    0),
                "J_Bip_R_Hand":       ( 0.3,  0,  0.2),
                "J_Bip_L_UpperArm":   (0,     0, -0.8),
                "J_Bip_L_LowerArm":   (0.3,   0,    0),
            }, duration=0.5),
            PoseKeyframe({
                "J_Bip_C_Spine":      (-0.18, 0,   0.1),
                "J_Bip_C_UpperChest": (-0.22, 0,  0.15),
                "J_Bip_R_UpperArm":   (-1.7,  0,  0.5),
                "J_Bip_R_LowerArm":   (-1.3,  0,    0),
                "J_Bip_R_Hand":       ( 0.4,  0,  0.3),
                "J_Bip_L_UpperArm":   (0,     0, -0.7),
                "J_Bip_L_LowerArm":   (0.2,   0,    0),
            }, duration=0.3),
            PoseKeyframe({
                "J_Bip_C_Spine":      (-0.20, 0,  0.15),
                "J_Bip_C_UpperChest": (-0.25, 0,  0.2),
                "J_Bip_R_UpperArm":   (-2.0,  0,  1.0),
                "J_Bip_R_LowerArm":   (-0.6,  0,    0),
                "J_Bip_R_Hand":       (-0.2,  0, -0.2),
                "J_Bip_L_UpperArm":   ( 0.1,  0, -1.0),
                "J_Bip_L_LowerArm":   ( 0.5,  0,    0),
            }, duration=0.25),
            PoseKeyframe({
                "J_Bip_C_Spine":      (-0.18, 0,  0.12),
                "J_Bip_C_UpperChest": (-0.22, 0,  0.18),
                "J_Bip_R_UpperArm":   (-1.8,  0,  0.9),
                "J_Bip_R_LowerArm":   (-0.4,  0,    0),
                "J_Bip_R_Hand":       (-0.3,  0, -0.3),
                "J_Bip_L_UpperArm":   ( 0.1,  0, -1.1),
                "J_Bip_L_LowerArm":   ( 0.4,  0,    0),
            }, duration=0.4),
            PoseKeyframe({}, duration=0.6),
        ])

        # 両手投げキッス
        self.add_clip("kiss_both", [
            PoseKeyframe({
                "J_Bip_C_Spine":      (-0.05, 0,    0),
                "J_Bip_C_UpperChest": (-0.05, 0,  0.1),
                "J_Bip_L_UpperArm":   (-0.9,  0,  0.8),
                "J_Bip_L_LowerArm":   (-1.6,  0,    0),
                "J_Bip_L_Hand":       ( 0.3,  0,  0.1),
                "J_Bip_R_UpperArm":   (-0.9,  0, -0.8),
                "J_Bip_R_LowerArm":   (-1.6,  0,    0),
                "J_Bip_R_Hand":       ( 0.3,  0, -0.1),
            }, duration=0.5),
            PoseKeyframe({
                "J_Bip_C_Spine":      (-0.12, 0,    0),
                "J_Bip_C_UpperChest": (-0.12, 0,  0.15),
                "J_Bip_L_UpperArm":   (-1.2,  0,  1.4),
                "J_Bip_L_LowerArm":   (-0.5,  0,    0),
                "J_Bip_L_Hand":       (-0.3,  0,  0.3),
                "J_Bip_R_UpperArm":   (-1.2,  0, -1.4),
                "J_Bip_R_LowerArm":   (-0.5,  0,    0),
                "J_Bip_R_Hand":       (-0.3,  0, -0.3),
            }, duration=0.25),
            PoseKeyframe({
                "J_Bip_C_Spine":      (-0.10, 0,    0),
                "J_Bip_C_UpperChest": (-0.10, 0,  0.12),
                "J_Bip_L_UpperArm":   (-1.3,  0,  1.5),
                "J_Bip_L_LowerArm":   (-0.3,  0,    0),
                "J_Bip_R_UpperArm":   (-1.3,  0, -1.5),
                "J_Bip_R_LowerArm":   (-0.3,  0,    0),
            }, duration=0.5),
            PoseKeyframe({}, duration=0.7),
        ])

        # お辞儀
        self.add_clip("bow", [
            PoseKeyframe({
                "J_Bip_C_Hips":       (0.6,  0, 0),
                "J_Bip_C_Spine":      (0.4,  0, 0),
                "J_Bip_C_Chest":      (0.3,  0, 0),
                "J_Bip_C_Neck":       (-0.3, 0, 0),
                "J_Bip_L_UpperArm":   (0,    0, -0.6),
                "J_Bip_R_UpperArm":   (0,    0,  0.6),
            }, duration=0.6),
            PoseKeyframe({
                "J_Bip_C_Hips":       (0.6,  0, 0),
                "J_Bip_C_Spine":      (0.4,  0, 0),
                "J_Bip_C_Chest":      (0.3,  0, 0),
                "J_Bip_C_Neck":       (-0.3, 0, 0),
                "J_Bip_L_UpperArm":   (0,    0, -0.6),
                "J_Bip_R_UpperArm":   (0,    0,  0.6),
            }, duration=0.8),
            PoseKeyframe({}, duration=0.5),
        ])

    # ── クリップ管理 ─────────────────────────────────────────

    def add_clip(self, name: str, frames: list[PoseKeyframe]):
        """カスタムクリップを登録する。"""
        self._clips[name] = frames

    def play(self, name: str, loop: bool = False,
             on_finish: Optional[Callable] = None):
        """クリップを再生する。"""
        if name not in self._clips:
            print(f"VrmAnimator: clip '{name}' not found")
            return
        self._clip_name  = name
        self._frames     = self._clips[name]
        self._frame_idx  = 0
        self._t          = 0.0
        self._playing    = True
        self._loop       = loop
        self._on_finish  = on_finish
        self._from_rots  = dict(self._current_rots)

    def stop(self):
        self._playing = False

    @property
    def is_playing(self) -> bool:
        return self._playing

    @property
    def clip_name(self) -> str:
        return self._clip_name

    # ── 毎フレーム ───────────────────────────────────────────

    def update(self, dt: float):
        """毎フレーム update() で呼ぶ。draw_vrm() の前に呼ぶこと。"""
        if not self._playing or not self._frames:
            return

        frame = self._frames[self._frame_idx]
        speed = 1.0 / max(0.01, frame.duration)
        self._t = min(1.0, self._t + dt * speed)

        self._apply_frame(frame, self._t)

        if self._t >= 1.0:
            self._frame_idx += 1
            if self._frame_idx >= len(self._frames):
                if self._loop:
                    self._frame_idx = 0
                    self._from_rots = dict(self._current_rots)
                    self._t = 0.0
                else:
                    self._playing = False
                    if self._on_finish:
                        self._on_finish()
            else:
                self._from_rots = dict(self._current_rots)
                self._t = 0.0

    def _apply_frame(self, frame: PoseKeyframe, t: float):
        """フレームを補間して Rust エンジンに反映する。"""
        t_ease = t * t * (3 - 2 * t)

        if frame.is_bind:
            # バインドポーズへ全ボーンを戻す
            for bone_name, q_from in self._from_rots.items():
                q_new = _slerp(q_from, _IDENTITY_QUAT, t_ease)
                self._current_rots[bone_name] = q_new
                self.engine.set_vrm_bone_rot(
                    self.vrm_id, bone_name,
                    q_new[0], q_new[1], q_new[2], q_new[3]
                )
            if t >= 1.0:
                self.engine.reset_vrm_pose(self.vrm_id)
                self._current_rots.clear()
        else:
            for bone_name, q_to in frame.quats.items():
                q_from = self._from_rots.get(bone_name, _IDENTITY_QUAT)
                q_new  = _slerp(q_from, q_to, t_ease)
                self._current_rots[bone_name] = q_new
                self.engine.set_vrm_bone_rot(
                    self.vrm_id, bone_name,
                    q_new[0], q_new[1], q_new[2], q_new[3]
                )

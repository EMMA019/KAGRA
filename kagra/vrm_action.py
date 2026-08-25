# kagra/vrm_action.py
"""
VRM ワンショット・アクションシステム (AIエージェント用)

既存のループアニメーション（idle等）の上に、
一時的なアクション（バンザイ、うなずき等）を上書きブレンドするコントローラー。

【VRM ボーン回転の基礎知識】
VRM の腕ボーン (UpperArm) は、Tポーズで体の横に伸びている。
-   X軸: 「前方向へのリフト」 — 腕を前方に上げる (ローカル空間の向きに依存)
-   Y軸: 「捻り (外旋/内旋)」 — 腕の長軸周りの回転
-   Z軸: 「外転/内転」 — 腕を横に広げる / 下ろす

【重要】
Emma.vrm では Shoulder ボーンが rz ≈ -98.6° で回転しているため、
腕を真上に上げる（バンザイ）には、UpperArm の rz を大きくマイナス（左腕）にする必要があります。
"""
from __future__ import annotations
import math
from typing import TYPE_CHECKING, Dict, List, Tuple

if TYPE_CHECKING:
    from kagra.vrm_avatar import VrmAvatar

# ── ユーティリティ（タコ化防止のため正規化を徹底） ──────────────────
def _quat_normalize(q: list[float]) -> list[float]:
    l = math.sqrt(q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3])
    if l < 1e-8: return [0.0, 0.0, 0.0, 1.0]
    return [q[0]/l, q[1]/l, q[2]/l, q[3]/l]

def _euler_to_quat(rx: float, ry: float, rz: float) -> list[float]:
    cx, sx = math.cos(rx / 2), math.sin(rx / 2)
    cy, sy = math.cos(ry / 2), math.sin(ry / 2)
    cz, sz = math.cos(rz / 2), math.sin(rz / 2)
    return _quat_normalize([
        sx * cy * cz + cx * sy * sz,
        cx * sy * cz - sx * cy * sz,
        cx * cy * sz + sx * sy * cz,
        cx * cy * cz - sx * sy * sz,
    ])

def _slerp(a: list[float], b: list[float], t: float) -> list[float]:
    dot = sum(a[i] * b[i] for i in range(4))
    if dot < 0:
        b = [-x for x in b]
        dot = -dot
    dot = min(1.0, dot)
    if dot > 0.9995:
        r = [a[i] + t * (b[i] - a[i]) for i in range(4)]
        return _quat_normalize(r)
    th0 = math.acos(dot)
    th = th0 * t
    sa = math.sin(th0 - th) / math.sin(th0)
    sb = math.sin(th) / math.sin(th0)
    r = [sa * a[i] + sb * b[i] for i in range(4)]
    return _quat_normalize(r)

_ID_QUAT = [0.0, 0.0, 0.0, 1.0]


def _overlay_bone_quat(
    pose: Dict[str, list[float]],
    bone: str,
    saved_idle: Dict[str, list[float]],
    bind_pose: Dict[str, list[float]],
    live_loco: Dict[str, list[float]] | None = None,
) -> list[float]:
    """Pick a bone quat from an overlay keyframe.

    Empty keyframes (``{}``) mean "release the overlay":
    1. live locomotion pose if the mixer/base clip still owns the bone
    2. the pose saved at play() (idle snapshot)
    3. bind

    Do not fall back to a live overlay dict — the overlay already wrote
    the action there, so blending toward it leaves clap/banzai arms stuck.
    """
    q = pose.get(bone)
    if q is not None:
        return q
    if live_loco and bone in live_loco:
        return live_loco[bone]
    rest = saved_idle.get(bone) or bind_pose.get(bone)
    return rest if rest is not None else _ID_QUAT[:]


# ── プロシージャル・アクション定義 ──────────────────────────────
# 形式: "アクション名": [(時間秒, {ボーン名: (rx, ry, rzのオイラー角ラジアン)}), ...]
#
# 注意: 各キーフレームの回転は「絶対回転」（現在のアニメ姿勢からの差分ではない）。
#       ActionController が現在の idle 姿勢からキーフレーム姿勢へ SLERP する。
# 【腕を上げる動作の正しい指定 (Emma.vrm)】
#   左腕:
#     - rx > 0 : 前に出す (Forward)
#     - rz > 0 : 真上に上げる (Banzai / Abduction)
#   右腕:
#     - rx > 0 : 前に出す (Forward)
#     - rz < 0 : 真上に上げる (Banzai / Abduction)
#   肘 (LowerArm):
#     - rx < 0 : 内側に曲げる (Bend inward)
#     - rx > 0 : 外側に曲げる / 前に曲げる
_ACTIONS = {
    # ── バンザイ ──────────────────────────────────────────────
    "banzai": [
        (0.0, {}),
        (0.25, {
            "J_Bip_L_UpperArm": (0, 0, 1.35),
            "J_Bip_R_UpperArm": (0, 0, -1.35),
            "J_Bip_C_Spine": (-0.03, 0, 0),
        }),
        (1.0, {
            "J_Bip_L_UpperArm": (0, 0, 1.35),
            "J_Bip_R_UpperArm": (0, 0, -1.35),
            "J_Bip_C_Spine": (-0.03, 0, 0),
        }),
        (1.3, {}),
    ],
    # ── うなずく ──────────────────────────────────────────────
    "nod": [
        (0.0, {}),
        (0.15, {"J_Bip_C_Head": (0.3, 0, 0), "J_Bip_C_Neck": (0.1, 0, 0)}),
        (0.3,  {}),
        (0.45, {"J_Bip_C_Head": (0.3, 0, 0), "J_Bip_C_Neck": (0.1, 0, 0)}),
        (0.6,  {}),
    ],
    # ── 首を振る ──────────────────────────────────────────────
    "shake_head": [
        (0.0, {}),
        (0.15, {"J_Bip_C_Head": (0, 0.3, 0), "J_Bip_C_Neck": (0, 0.1, 0)}),
        (0.45, {"J_Bip_C_Head": (0, -0.3, 0), "J_Bip_C_Neck": (0, -0.1, 0)}),
        (0.75, {"J_Bip_C_Head": (0, 0.3, 0), "J_Bip_C_Neck": (0, 0.1, 0)}),
        (0.9,  {}),
    ],
    # ── 首をかしげる ──────────────────────────────────────────
    "tilt_head": [
        (0.0, {}),
        (0.3, {"J_Bip_C_Head": (0, 0, 0.25), "J_Bip_C_Neck": (0, 0, 0.1)}),
        (1.5, {"J_Bip_C_Head": (0, 0, 0.25), "J_Bip_C_Neck": (0, 0, 0.1)}),
        (1.8, {}),
    ],
    # ── ジャンプ ──────────────────────────────────────────────
    "jump_joy": [
        (0.0, {}),
        (0.1, {
            "J_Bip_L_UpperLeg": (-0.2, 0, 0),
            "J_Bip_R_UpperLeg": (-0.2, 0, 0),
            "J_Bip_C_Spine": (0.05, 0, 0),
        }),
        (0.25, {
            "J_Bip_C_Hips": (0, 0, 0),
            "J_Bip_L_UpperLeg": (0.1, 0, 0),
            "J_Bip_R_UpperLeg": (0.1, 0, 0),
            "J_Bip_L_UpperArm": (0, 0, 1.35),
            "J_Bip_R_UpperArm": (0, 0, -1.35),
        }),
        (0.4, {
            "J_Bip_L_UpperLeg": (-0.1, 0, 0),
            "J_Bip_R_UpperLeg": (-0.1, 0, 0),
            "J_Bip_C_Spine": (0.05, 0, 0),
        }),
        (0.6, {}),
    ],
    # ── 手を振る ──────────────────────────────────────────────
    "wave": [
        (0.0, {}),
        (0.2, {
            "J_Bip_R_UpperArm": (0, 0.3, -1.35),
            "J_Bip_R_LowerArm": (1.0, 0, 0),       # 肘曲げ
        }),
        (0.4, {
            "J_Bip_R_UpperArm": (0, -0.3, -1.35),
            "J_Bip_R_LowerArm": (1.0, 0, 0),       # 肘伸ばし
        }),
        (0.6, {
            "J_Bip_R_UpperArm": (0, 0.3, -1.35),
            "J_Bip_R_LowerArm": (1.0, 0, 0),
        }),
        (0.8, {
            "J_Bip_R_UpperArm": (0, -0.3, -1.35),
            "J_Bip_R_LowerArm": (1.0, 0, 0),
        }),
        (1.0, {
            "J_Bip_R_UpperArm": (0, 0.3, -1.35),
            "J_Bip_R_LowerArm": (1.0, 0, 0),
        }),
        (1.3, {}),
    ],
    # ── 考える ──────────────────────────────────────────────
    "think": [
        (0.0, {}),
        (0.3, {
            "J_Bip_R_UpperArm": (1.0, 0, -0.2), # 前に出して少し内側
            "J_Bip_R_LowerArm": (-2.0, 0, 0),   # 肘を深く曲げる
            "J_Bip_C_Head": (0.1, 0, 0.1),
        }),
        (2.0, {
            "J_Bip_R_UpperArm": (1.0, 0, -0.2),
            "J_Bip_R_LowerArm": (-2.0, 0, 0),
            "J_Bip_C_Head": (0.1, 0, 0.1),
        }),
        (2.3, {}),
    ],
    # ── お辞儀 ──────────────────────────────────────────────
    "bow": [
        (0.0, {}),
        (0.4, {
            "J_Bip_C_Hips": (0.15, 0, 0),
            "J_Bip_C_Spine": (0.15, 0, 0),
            "J_Bip_C_Neck": (0.1, 0, 0),
        }),
        (1.2, {
            "J_Bip_C_Hips": (0.15, 0, 0),
            "J_Bip_C_Spine": (0.15, 0, 0),
            "J_Bip_C_Neck": (0.1, 0, 0),
        }),
        (1.6, {}),
    ],
    # ── 拍手 ───────────────────────────────────────────────
    "clap": [
        (0.0, {}),
        (0.2, {
            "J_Bip_L_UpperArm": (0.8, 0, 0.4),  # 前に出して少し内側へ
            "J_Bip_R_UpperArm": (0.8, 0, -0.4),
            "J_Bip_L_LowerArm": (-1.5, 0, 0),   # 内側に曲げる
            "J_Bip_R_LowerArm": (-1.5, 0, 0),
        }),
        (0.7, {
            "J_Bip_L_UpperArm": (0.8, 0, 0.4),
            "J_Bip_R_UpperArm": (0.8, 0, -0.4),
            "J_Bip_L_LowerArm": (-0.5, 0, 0),   # 少し開く
            "J_Bip_R_LowerArm": (-0.5, 0, 0),
        }),
        (1.2, {}),
    ],
}


# ══════════════════════════════════════════════════════════════════
#  ActionController
# ══════════════════════════════════════════════════════════════════

class ActionController:
    """ワンショットアクションコントローラー。

    VrmAvatar のループアニメーション（idle等）の上に、
    アクションのボーン回転を一時的に上書きブレンドする。

    Example::
        action = ActionController(avatar)
        print(ActionController.names())  # banzai, nod, clap, ...
        action.play("banzai")

        def update(dt):
            avatar.update(dt)
            action.update(dt)  # ← avatar.update() の後に呼ぶ
    """

    @staticmethod
    def names() -> list[str]:
        """組み込みアクション名。"""
        return list(_ACTIONS)

    def __init__(self, avatar: "VrmAvatar"):
        self._avatar = avatar
        self.vrm_id = avatar.vrm_id
        avatar._action_controller = self

        self.playing_action: str | None = None
        self._time = 0.0
        self._duration = 0.0
        self._keyframes: List[Tuple[float, Dict[str, list[float]]]] = []

        # アクション開始時に保存した idle 姿勢（終了時に復元する）
        self._saved_idle_rots: Dict[str, list[float]] = {}
        # Overlay pose written to the engine without mutating locomotion current_rots.
        self._overlay_rots: Dict[str, list[float]] = {}

        # バインドポーズのキャッシュ（初回アクション時に構築）
        self._bind_pose: Dict[str, list[float]] = {}
        self._bind_pose_loaded = False

    def _ensure_bind_pose(self):
        """バインドポーズを必要に応じて構築する。"""
        if self._bind_pose_loaded:
            return
        self._bind_pose_loaded = True

        if hasattr(self._avatar, 'get_bind_rot'):
            candidate_bones = set()
            for frames in _ACTIONS.values():
                for _, rot_dict in frames:
                    candidate_bones.update(rot_dict.keys())
            for bone in candidate_bones:
                rot = self._avatar.get_bind_rot(bone)
                if rot is not None:
                    self._bind_pose[bone] = rot
                else:
                    self._bind_pose[bone] = _ID_QUAT[:]
        else:
            # get_bind_rot がない場合、バインドポーズ取得はできない
            pass

    def play(self, action_name: str):
        """アクションを開始する。

        Args:
            action_name: "banzai" / "nod" / "shake_head" / "tilt_head" /
                         "jump_joy" / "wave" / "think" / "bow" / "clap"
        """
        if action_name not in _ACTIONS:
            print(f"[Action] 不明なアクション: {action_name}")
            return

        self._ensure_bind_pose()
        keep_idle = self.playing_action is not None and bool(self._saved_idle_rots)
        self.playing_action = action_name
        self._time = 0.0

        from kagra.vrm_avatar import _qmul
        raw_frames = _ACTIONS[action_name]
        self._keyframes = []
        for t, rot_dict in raw_frames:
            quat_dict = {}
            for bone, euler in rot_dict.items():
                qt = _euler_to_quat(*euler)
                bind_q = self._bind_pose.get(bone, _ID_QUAT)
                quat_dict[bone] = _qmul(bind_q, qt)
            self._keyframes.append((t, quat_dict))
        self._duration = self._keyframes[-1][0]

        # First play() snapshots idle. A follow-up clap→banzai must not
        # treat the overlay pose as the new rest (folded-forward arms).
        self._save_idle_poses(keep=keep_idle)

    def _save_idle_poses(self, *, keep: bool = False):
        """アクションが触るボーンの現在の idle 姿勢を保存する。"""
        if not keep:
            self._saved_idle_rots = {}
        all_bones = set()
        for _, pose in self._keyframes:
            all_bones.update(pose.keys())
        for bone in all_bones:
            if keep and bone in self._saved_idle_rots:
                continue
            current = self._avatar._anim.current_rots.get(bone)
            if current is not None:
                self._saved_idle_rots[bone] = current[:]
            elif bone in self._bind_pose:
                self._saved_idle_rots[bone] = self._bind_pose[bone][:]
            else:
                self._saved_idle_rots[bone] = _ID_QUAT[:]

    def _restore_idle_poses(self):
        """アクション終了後、idle 姿勢を復元する。"""
        import kagra
        for bone, q in self._saved_idle_rots.items():
            kagra.get_engine().set_vrm_bone_rot(self.vrm_id, bone, *q)
            self._avatar._anim.current_rots[bone] = q[:]
        self._saved_idle_rots = {}

    def update(self, dt: float):
        """毎フレーム呼ぶ（avatar.update() 内または外部呼出し）。"""
        if not self.playing_action:
            return

        frame_num = getattr(self._avatar, '_frame_count', 0)
        if getattr(self, '_last_update_frame', -1) == frame_num and frame_num > 0:
            return
        self._last_update_frame = frame_num

        import kagra
        self._time += dt

        if self._time >= self._duration:
            self.playing_action = None
            loco = getattr(self._avatar, "_loco", None)
            if loco is not None and loco.enabled:
                # Mixer already holds the live walk pose in current_rots.
                self._overlay_rots = {}
                self._saved_idle_rots = {}
                return
            self._restore_idle_poses()
            self._overlay_rots = {}
            return

        # 現在のキーフレーム区間を探す
        idx = 0
        for i in range(len(self._keyframes) - 1):
            if self._keyframes[i][0] <= self._time <= self._keyframes[i+1][0]:
                idx = i
                break

        t0, pose0 = self._keyframes[idx]
        t1, pose1 = self._keyframes[idx + 1]
        progress = (self._time - t0) / max(0.0001, (t1 - t0))
        t_ease = progress * progress * (3 - 2 * progress)

        all_bones = set(pose0.keys()) | set(pose1.keys())
        live_loco = dict(self._avatar._anim.current_rots)
        upper = getattr(self._avatar, "_upper", None)
        if upper is not None and upper.playing:
            live_loco.update(upper.current_rots)

        for bone in all_bones:
            q0 = _overlay_bone_quat(
                pose0, bone, self._saved_idle_rots, self._bind_pose, live_loco,
            )
            q1 = _overlay_bone_quat(
                pose1, bone, self._saved_idle_rots, self._bind_pose, live_loco,
            )
            q_blend = _slerp(q0, q1, t_ease)

            kagra.get_engine().set_vrm_bone_rot(self.vrm_id, bone, *q_blend)
            self._overlay_rots[bone] = q_blend

# kagra/fbx_player.py
"""
FBX アニメーションプレイヤー
ufbx（Rust）経由で FBX を直接読み込み、BvhMotion と同じ形式で返す。

Example::
    avatar = kagra.avatar("assets/Emma.vrm")

    # FBX を1行でロード
    avatar.load_motion("dance", "assets/hiphop.fbx")
    avatar.play("dance", loop=True)

    # または詳細確認
    motion = kagra.load_fbx("assets/hiphop.fbx")
    print(f"clips: {motion.clip_names}")
    print(f"{motion.fps:.0f}fps  {motion.duration:.1f}sec")
    avatar.add_motion("dance", motion)
"""
from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional


def _qconj(q):
    return (-q[0], -q[1], -q[2], q[3])

def _qmul(a, b):
    ax,ay,az,aw = a; bx,by,bz,bw = b
    return (aw*bx+ax*bw+ay*bz-az*by,
            aw*by-ax*bz+ay*bw+az*bx,
            aw*bz+ax*by-ay*bx+az*bw,
            aw*bw-ax*bx-ay*by-az*bz)

def _qnorm(q):
    l = (q[0]**2+q[1]**2+q[2]**2+q[3]**2)**0.5 or 1e-8
    return (q[0]/l, q[1]/l, q[2]/l, q[3]/l)

def _qdamp(q, factor):
    """クォータニオンの回転角度を factor 倍に抑制する（0.0〜1.0）。"""
    import math
    w = max(-1.0, min(1.0, q[3]))
    angle = 2.0 * math.acos(w)
    if angle < 1e-6:
        return q
    new_angle = angle * factor
    s_old = math.sin(angle / 2.0)
    s_new = math.sin(new_angle / 2.0)
    ratio = s_new / s_old if s_old > 1e-8 else 0.0
    return (q[0]*ratio, q[1]*ratio, q[2]*ratio, math.cos(new_angle / 2.0))

# ufbx が target_axes=right_handed_y_up で変換済みなので
# Armature の -90° X 回転はすでに吸収されている。
# 追加の軸補正は不要（二重補正になる）。

# ── Mixamo → VRM (J_Bip_*) ボーン名マッピング ──────────────────
# Mixamo FBX のボーン名を VRM のボーン名に変換する
# VRoid Studio の J_Bip_* 命名規則に対応
_BONE_MAP = {
    # 体幹
    'Hips':        'J_Bip_C_Hips',
    'Spine':       'J_Bip_C_Spine',
    'Spine1':      'J_Bip_C_Chest',
    'Spine2':      'J_Bip_C_UpperChest',
    'Neck':        'J_Bip_C_Neck',
    'Head':        'J_Bip_C_Head',
    # 左腕
    'LeftShoulder':  'J_Bip_L_Shoulder',
    'LeftArm':       'J_Bip_L_UpperArm',
    'LeftForeArm':   'J_Bip_L_LowerArm',
    'LeftHand':      'J_Bip_L_Hand',
    # 右腕
    'RightShoulder': 'J_Bip_R_Shoulder',
    'RightArm':      'J_Bip_R_UpperArm',
    'RightForeArm':  'J_Bip_R_LowerArm',
    'RightHand':     'J_Bip_R_Hand',
    # 左脚
    'LeftUpLeg':   'J_Bip_L_UpperLeg',
    'LeftLeg':     'J_Bip_L_LowerLeg',
    'LeftFoot':    'J_Bip_L_Foot',
    'LeftToeBase': 'J_Bip_L_ToeBase',
    # 右脚
    'RightUpLeg':   'J_Bip_R_UpperLeg',
    'RightLeg':     'J_Bip_R_LowerLeg',
    'RightFoot':    'J_Bip_R_Foot',
    'RightToeBase': 'J_Bip_R_ToeBase',
    # mixamorig: プレフィックス付き版（Mixamo のキャラ付き FBX）
    'mixamorig:Hips':        'J_Bip_C_Hips',
    'mixamorig:Spine':       'J_Bip_C_Spine',
    'mixamorig:Spine1':      'J_Bip_C_Chest',
    'mixamorig:Spine2':      'J_Bip_C_UpperChest',
    'mixamorig:Neck':        'J_Bip_C_Neck',
    'mixamorig:Head':        'J_Bip_C_Head',
    'mixamorig:LeftShoulder':  'J_Bip_L_Shoulder',
    'mixamorig:LeftArm':       'J_Bip_L_UpperArm',
    'mixamorig:LeftForeArm':   'J_Bip_L_LowerArm',
    'mixamorig:LeftHand':      'J_Bip_L_Hand',
    'mixamorig:RightShoulder': 'J_Bip_R_Shoulder',
    'mixamorig:RightArm':      'J_Bip_R_UpperArm',
    'mixamorig:RightForeArm':  'J_Bip_R_LowerArm',
    'mixamorig:RightHand':     'J_Bip_R_Hand',
    'mixamorig:LeftUpLeg':   'J_Bip_L_UpperLeg',
    'mixamorig:LeftLeg':     'J_Bip_L_LowerLeg',
    'mixamorig:LeftFoot':    'J_Bip_L_Foot',
    'mixamorig:LeftToeBase': 'J_Bip_L_ToeBase',
    'mixamorig:RightUpLeg':   'J_Bip_R_UpperLeg',
    'mixamorig:RightLeg':     'J_Bip_R_LowerLeg',
    'mixamorig:RightFoot':    'J_Bip_R_Foot',
    'mixamorig:RightToeBase': 'J_Bip_R_ToeBase',
}

# ── ボーン回転抑制率（1.0=そのまま, 0.0=固定）
# FBX/VRM のバインドポーズ差異による過剰回転を抑制
# 完全スキップするボーン
# 足首・つま先は FBX/VRM 間の軸規約差が大きく、ローカル空間デルタでも
# 過剰回転が発生するためスキップが必要
_SKIP_BONES = {
    'J_Bip_L_Foot',      # 足首：軸規約差が大きい
    'J_Bip_R_Foot',
    'J_Bip_L_ToeBase',   # 足先
    'J_Bip_R_ToeBase',
}

_DAMP_MAP = {
    # 頭・首は軸ズレの影響を受けやすいのでやや抑制
    'J_Bip_C_Head':       0.5,
    'J_Bip_C_Neck':       0.4,
    # 体幹はダンスの動きを活かすため強めに適用
    'J_Bip_C_UpperChest': 0.7,
    'J_Bip_C_Chest':      0.8,
    'J_Bip_C_Spine':      0.8,
}
_HEAD_DAMP = _DAMP_MAP  # 後方互換

# Mixamo は cm、VRM は m。Hips の ty を VRM 脚高として使うと scale≈97 になる。
_DEFAULT_VRM_HIPS_Y = 0.853


def root_scale(fbx_leg_h: float, vrm_hips_y: float = _DEFAULT_VRM_HIPS_Y) -> float:
    """FBX ルート移動 → VRM メートル。Mixamo の Hips ty は渡さない。"""
    if fbx_leg_h < 0.01:
        fbx_leg_h = 1.0
    if not (0.2 <= vrm_hips_y <= 2.5):
        vrm_hips_y = _DEFAULT_VRM_HIPS_Y
    return vrm_hips_y / fbx_leg_h


@dataclass
class FbxMotion:
    """ufbx で読み込んだ FBX アニメーションデータ。

    BvhMotion と同じ to_clip() インターフェースを持つ。
    """
    _raw_clips: list          # Rust から返ってきた生データ
    _clip_index: int = 0      # 使用するクリップのインデックス
    _cache: Optional[list] = field(default=None, repr=False)
    vrm_hips_y: float = _DEFAULT_VRM_HIPS_Y

    @property
    def clip_names(self) -> list[str]:
        return [c[0] for c in self._raw_clips]

    @property
    def frame_time(self) -> float:
        return self._raw_clips[self._clip_index][1]

    @property
    def fps(self) -> float:
        ft = self.frame_time
        return 1.0 / ft if ft > 0 else 60.0

    @property
    def duration(self) -> float:
        frames = self._raw_clips[self._clip_index][2]
        return len(frames) * self.frame_time

    def use_clip(self, name: str) -> 'FbxMotion':
        """使用するクリップを名前で選択する。"""
        for i, (clip_name, _, _) in enumerate(self._raw_clips):
            if clip_name == name:
                self._clip_index = i
                self._cache = None
                return self
        raise ValueError(f"Clip '{name}' not found. Available: {self.clip_names}")

    def to_clip(self) -> list:
        """VrmAvatar.add_motion() に渡せる [(bones_dict, duration, root_pos), ...] に変換する。

        デルタ回転は Rust 側（fbx_loader.rs）で計算済み。
        node.local_transform.rotation をバインドポーズとして使用するため精度が高い。
        """
        if self._cache is not None:
            return self._cache

        _, frame_time, raw_frames = self._raw_clips[self._clip_index]
        clip = []

        # ── 接地計算 ──────────────────────────────────────────────
        # max(Armature Y) = 立ち姿勢（最も高い点）
        # → この時 root_offset = 0（VRM キャラが自然な立ち位置）
        #
        # delta_y = arm_y - max_arm_y ≤ 0
        # → しゃがみ・フレア時にマイナスになり VRM が沈む
        #
        # 脚長スケール補正:
        # FBX 脚高 = max(arm_y) - min(arm_y)（立ちから最低点まで）
        # VRM 脚高 = VRM Hips Y（bind_trans_y ≈ 0.853m）
        # scale = VRM脚高 / FBX脚高

        # ── Armature XYZ を全フレーム収集 ─────────────────────────
        arm_xs, arm_ys, arm_zs = [], [], []
        for raw_frame in raw_frames:
            ax = ay = az = None
            for (name, tx, ty, tz, qx, qy, qz, qw, has_trans) in raw_frame:
                if name == 'Armature' and has_trans:
                    ax, ay, az = tx, ty, tz
                    break
            arm_xs.append(ax or 0.0)
            arm_ys.append(ay or 0.0)
            arm_zs.append(az or 0.0)

        true_ground_y = min(arm_ys) if arm_ys else 0.0
        first_arm_y   = arm_ys[0]  if arm_ys else 0.0

        # fbx_leg_h = frame[0] から床までの高さ（FBXの「立ち」基準）
        fbx_leg_h = first_arm_y - true_ground_y
        if fbx_leg_h < 0.01:
            fbx_leg_h = 1.0

        # Mixamo Hips ty（≈97cm）は VRM の bind Y ではない。アバター側の値を使う。
        vrm_hips_y = self.vrm_hips_y
        if not (0.2 <= vrm_hips_y <= 2.5):
            vrm_hips_y = _DEFAULT_VRM_HIPS_Y
        scale = root_scale(fbx_leg_h, vrm_hips_y)

        # XZ の原点（frame[0] 基準）
        base_arm_x = arm_xs[0]
        base_arm_z = arm_zs[0]

        print(f"[FBX] ground={true_ground_y:.4f} frame0={first_arm_y:.4f}")
        print(f"[FBX] fbx_leg={fbx_leg_h:.4f} vrm_hips={vrm_hips_y:.4f} scale={scale:.4f}")

        for fi, raw_frame in enumerate(raw_frames):
            bones = {}
            arm_x, arm_y, arm_z = arm_xs[fi], arm_ys[fi], arm_zs[fi]

            # Y: (arm_y - ground) をスケール → VRM 脚長に合わせる
            #    frame[0] 時 scaled_y = fbx_leg_h * scale = vrm_hips_y → offset=0
            #    最低点時 scaled_y = 0 → offset = -vrm_hips_y → Hips が床
            scaled_y = (arm_y - true_ground_y) * scale
            offset_y = scaled_y - vrm_hips_y

            # XZ: frame[0] からのデルタ（水平移動）
            offset_x = (arm_x - base_arm_x) * scale
            offset_z = (arm_z - base_arm_z) * scale

            # 足ボーンをスキップしている間は位置オフセットを無効にする
            # （足が地面から離れてしまうため）
            root_pos = (0.0, 0.0, 0.0)

            for (name, tx, ty, tz, qx, qy, qz, qw, has_trans) in raw_frame:
                if name in ('Armature', 'Root', 'root'):
                    continue

                # ボーン名マッピング（Mixamo → J_Bip_*）
                vrm_name = _BONE_MAP.get(name, name)

                # 完全スキップ
                if vrm_name in _SKIP_BONES:
                    continue

                q_delta = (qx, qy, qz, qw)
                damp = _DAMP_MAP.get(vrm_name, 1.0)
                if damp < 1.0:
                    q_delta = _qdamp(q_delta, damp)
                bones[vrm_name] = q_delta

            clip.append((bones, frame_time, root_pos))

        self._cache = clip
        return clip


def load_fbx(path: str, clip_name: str = None) -> FbxMotion:
    """FBX ファイルを読み込んで FbxMotion を返す。

    Args:
        path:      FBX ファイルのパス
        clip_name: 使用するクリップ名（省略時は最初のクリップ）

    Returns:
        FbxMotion（BvhMotion と同じインターフェース）

    Example::
        # シンプル版（推奨）
        avatar.load_motion("dance", "assets/hiphop.fbx")

        # 詳細確認
        motion = kagra.load_fbx("assets/hiphop.fbx")
        print(motion.clip_names)
        print(f"{motion.fps:.0f}fps  {motion.duration:.1f}sec")
    """
    import kagra
    raw = kagra.get_engine().load_fbx_anim(path)
    if not raw:
        raise RuntimeError(f"FBX にアニメーションが見つかりませんでした: {path}")

    motion = FbxMotion(_raw_clips=raw)

    if clip_name:
        motion.use_clip(clip_name)

    print(f"[FBX] {path}")
    print(f"  clips : {motion.clip_names}")
    print(f"  fps   : {motion.fps:.0f}  duration: {motion.duration:.1f}sec")

    return motion

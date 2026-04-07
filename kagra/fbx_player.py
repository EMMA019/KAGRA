# kagra/fbx_player.py
"""
FBX アニメーションプレイヤー (ロード時ベイク版)
- FBX読み込み時に「VRMにそのまま流せるローカル回転＆移動」に変換（ベイク）
- 再生時は計算ゼロ
- 移動補正: Yのみ脚長比スケール、XZは1.0倍（歩行時の左右過大揺れ防止）
"""
from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional, Dict, Tuple, List
import math
import warnings
import os

# ============================================================================
# クォータニオン基本演算
# ============================================================================
def _qconj(q: Tuple[float, float, float, float]) -> Tuple[float, float, float, float]:
    return (-q[0], -q[1], -q[2], q[3])

def _qmul(a: Tuple[float, float, float, float],
         b: Tuple[float, float, float, float]) -> Tuple[float, float, float, float]:
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz
    )

def _qnorm(q: Tuple[float, float, float, float]) -> Tuple[float, float, float, float]:
    l = math.sqrt(q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3])
    if l < 1e-8:
        return (0.0, 0.0, 0.0, 1.0)
    return (q[0]/l, q[1]/l, q[2]/l, q[3]/l)

def _qdamp(q: Tuple[float, float, float, float], factor: float) -> Tuple[float, float, float, float]:
    w = max(-1.0, min(1.0, q[3]))
    angle = 2.0 * math.acos(w)
    if angle < 1e-6:
        return q
    new_angle = angle * factor
    s_old = math.sin(angle / 2.0)
    s_new = math.sin(new_angle / 2.0)
    ratio = s_new / s_old if s_old > 1e-8 else 0.0
    return (q[0] * ratio, q[1] * ratio, q[2] * ratio, math.cos(new_angle / 2.0))

# ============================================================================
# Mixamo ボーン階層 (ワールド計算用)
# ============================================================================
MIXAMO_PARENTS: Dict[str, str] = {
    'Hips': 'Armature',
    'Spine': 'Hips', 'Spine1': 'Spine', 'Spine2': 'Spine1',
    'Neck': 'Spine2', 'Head': 'Neck', 'HeadTop_End': 'Head',
    'LeftShoulder': 'Spine2', 'LeftArm': 'LeftShoulder', 'LeftForeArm': 'LeftArm', 'LeftHand': 'LeftForeArm',
    'RightShoulder': 'Spine2', 'RightArm': 'RightShoulder', 'RightForeArm': 'RightArm', 'RightHand': 'RightForeArm',
    'LeftUpLeg': 'Hips', 'LeftLeg': 'LeftUpLeg', 'LeftFoot': 'LeftLeg', 'LeftToeBase': 'LeftFoot',
    'RightUpLeg': 'Hips', 'RightLeg': 'RightUpLeg', 'RightFoot': 'RightLeg', 'RightToeBase': 'RightFoot',
}
for hand in ('LeftHand', 'RightHand'):
    for finger in ('Thumb', 'Index', 'Middle', 'Ring', 'Pinky'):
        for i in range(1, 5):
            name = f'{hand}{finger}{i}'
            MIXAMO_PARENTS[name] = hand if i == 1 else f'{hand}{finger}{i-1}'

# ============================================================================
# Mixamo → VRM ボーン名マッピング
# ============================================================================
MIXAMO_TO_VRM: Dict[str, str] = {
    'Armature': 'Root',
    'Hips': 'J_Bip_C_Hips',
    'Spine': 'J_Bip_C_Spine',
    'Spine1': 'J_Bip_C_Chest',
    'Spine2': 'J_Bip_C_UpperChest',
    'Neck': 'J_Bip_C_Neck',
    'Head': 'J_Bip_C_Head',
    'LeftShoulder': 'J_Bip_L_Shoulder',
    'LeftArm': 'J_Bip_L_UpperArm',
    'LeftForeArm': 'J_Bip_L_LowerArm',
    'LeftHand': 'J_Bip_L_Hand',
    'RightShoulder': 'J_Bip_R_Shoulder',
    'RightArm': 'J_Bip_R_UpperArm',
    'RightForeArm': 'J_Bip_R_LowerArm',
    'RightHand': 'J_Bip_R_Hand',
    'LeftUpLeg': 'J_Bip_L_UpperLeg',
    'LeftLeg': 'J_Bip_L_LowerLeg',
    'LeftFoot': 'J_Bip_L_Foot',
    'LeftToeBase': 'J_Bip_L_ToeBase',
    'RightUpLeg': 'J_Bip_R_UpperLeg',
    'RightLeg': 'J_Bip_R_LowerLeg',
    'RightFoot': 'J_Bip_R_Foot',
    'RightToeBase': 'J_Bip_R_ToeBase',
}
for m_prefix, v_prefix in (('LeftHand', 'J_Bip_L_'), ('RightHand', 'J_Bip_R_')):
    for finger in ('Thumb', 'Index', 'Middle', 'Ring', 'Pinky'):
        for i in range(1, 5):
            MIXAMO_TO_VRM[f'{m_prefix}{finger}{i}'] = f'{v_prefix}{finger}{i}'

VRM_HIPS_Y_BIND = 0.8532
DAMP_MAP = {
    'J_Bip_C_Head': 0.2,
    'J_Bip_C_Neck': 0.3,
    'J_Bip_C_UpperChest': 0.6,
    'J_Bip_C_Chest': 0.7,
}

BakedClip = List[Tuple[Dict[str, Tuple[float, float, float, float]], float, Tuple[float, float, float]]]

@dataclass
class FbxMotion:
    _raw_clips: list
    _clip_index: int = 0
    _baked_cache: Dict[int, BakedClip] = field(default_factory=dict)
    _bind_frame: Optional[list] = field(default=None)
    _vrm_bind_rot: Dict[str, Tuple[float, float, float, float]] = field(default_factory=dict)

    @property
    def clip_names(self) -> List[str]:
        return [c[0] for c in self._raw_clips]

    @property
    def frame_time(self) -> float:
        return self._raw_clips[self._clip_index][1]

    @property
    def fps(self) -> float:
        return 1.0 / self.frame_time if self.frame_time > 0 else 30.0

    def use_clip(self, name: str) -> 'FbxMotion':
        for i, (clip_name, _, _) in enumerate(self._raw_clips):
            if clip_name == name:
                self._clip_index = i
                return self
        raise ValueError(f"Clip '{name}' not found")

    def set_bind_from_fbx(self, bind_fbx_path: str) -> None:
        import kagra
        raw = kagra._engine.load_fbx_anim(bind_fbx_path)
        if not raw:
            raise RuntimeError(f"T-pose FBX could not be loaded: {bind_fbx_path}")
        self._bind_frame = raw[0][2][0]
        print(f"[FBX] Bind pose loaded from: {bind_fbx_path}")

    def set_vrm_bind_rotations(self, vrm_bind_map: Dict[str, Tuple[float, float, float, float]]) -> None:
        self._vrm_bind_rot = vrm_bind_map.copy()

    def _bake_clip(self, clip_index: int) -> BakedClip:
        if clip_index in self._baked_cache:
            return self._baked_cache[clip_index]

        _, frame_time, raw_frames = self._raw_clips[clip_index]
        bind_source = self._bind_frame if self._bind_frame else raw_frames[0]
        if not self._bind_frame:
            warnings.warn("No T-pose FBX set; using first animation frame as bind pose.", UserWarning)

        # FBXバインドワールド回転
        fbx_bind_local = {}
        for item in bind_source:
            name, _, _, _, qx, qy, qz, qw, _ = item
            fbx_bind_local[name] = (qx, qy, qz, qw)
        fbx_bind_world = {}
        def calc_fbx_bind_world(node):
            if node in fbx_bind_world: return fbx_bind_world[node]
            lr = fbx_bind_local.get(node, (0,0,0,1))
            if node not in MIXAMO_PARENTS:
                wr = lr
            else:
                wr = _qmul(calc_fbx_bind_world(MIXAMO_PARENTS[node]), lr)
            fbx_bind_world[node] = wr
            return wr
        for node in fbx_bind_local:
            calc_fbx_bind_world(node)

        # VRMバインドワールド回転
        vrm_bind_local = {}
        for mix_name, vrm_name in MIXAMO_TO_VRM.items():
            vrm_bind_local[mix_name] = self._vrm_bind_rot.get(vrm_name, (0,0,0,1))
        vrm_bind_world = {}
        def calc_vrm_bind_world(node):
            if node in vrm_bind_world: return vrm_bind_world[node]
            lr = vrm_bind_local.get(node, (0,0,0,1))
            if node not in MIXAMO_PARENTS:
                wr = lr
            else:
                wr = _qmul(calc_vrm_bind_world(MIXAMO_PARENTS[node]), lr)
            vrm_bind_world[node] = wr
            return wr
        for node in fbx_bind_local:
            calc_vrm_bind_world(node)

        # 移動補正パラメータ
        x_bind_arm = y_bind_arm = z_bind_arm = 0.0
        hips_bind_y = 0.0
        for item in bind_source:
            name, tx, ty, tz, _, _, _, _, has_trans = item
            if name == 'Armature' and has_trans:
                x_bind_arm, y_bind_arm, z_bind_arm = tx, ty, tz
            if name == 'Hips' and has_trans:
                hips_bind_y = ty
        leg_len_fbx = hips_bind_y - y_bind_arm
        if leg_len_fbx <= 0:
            leg_len_fbx = 0.7
        scale_y = VRM_HIPS_Y_BIND / leg_len_fbx
        
        # ★ 千鳥足対策: XZ軸の移動量もY軸（脚の長さ）に合わせて縮小する
        # ※ もしこれでも左右の揺れが強すぎる（ガニ股すぎる）場合は、 `scale_y * 0.8` などのように
        # さらにスケールダウンさせると、アニメキャラらしい直線的な歩き方になります。
        scale_xz = scale_y * 0.2 

        baked = []
        for frame in raw_frames:
            fbx_cur_local = {}
            root_delta = (0.0, 0.0, 0.0)
            for item in frame:
                name, tx, ty, tz, qx, qy, qz, qw, has_trans = item
                fbx_cur_local[name] = (qx, qy, qz, qw)
                if name == 'Armature' and has_trans:
                    dx = (tx - x_bind_arm) * scale_xz
                    dz = (tz - z_bind_arm) * scale_xz
                    dy = (ty - y_bind_arm) * scale_y
                    root_delta = (dx, dy, dz)

            # ワールド回転とデルタ
            fbx_cur_world = {}
            delta_world = {}
            def calc_frame_world(node):
                if node in fbx_cur_world: return fbx_cur_world[node]
                lr = fbx_cur_local.get(node, (0,0,0,1))
                if node not in MIXAMO_PARENTS:
                    wr = lr
                else:
                    wr = _qmul(calc_frame_world(MIXAMO_PARENTS[node]), lr)
                fbx_cur_world[node] = wr
                bind_w = fbx_bind_world.get(node, (0,0,0,1))
                delta_world[node] = _qmul(wr, _qconj(bind_w))
                return wr
            for node in fbx_cur_local:
                calc_frame_world(node)

            # VRMターゲットワールド → ローカル回転
            target_world = {node: _qmul(delta_world[node], vrm_bind_world.get(node, (0,0,0,1)))
                            for node in delta_world}
            bones = {}
            for node, tw in target_world.items():
                if node not in MIXAMO_PARENTS:
                    local_q = tw
                else:
                    parent = MIXAMO_PARENTS[node]
                    ptw = target_world[parent]
                    local_q = _qmul(_qconj(ptw), tw)
                vrm_name = MIXAMO_TO_VRM.get(node, node)
                if vrm_name in DAMP_MAP:
                    local_q = _qdamp(local_q, DAMP_MAP[vrm_name])
                bones[vrm_name] = local_q
            if 'Armature' in target_world:
                bones['Root'] = target_world['Armature']

            baked.append((bones, frame_time, root_delta))

        self._baked_cache[clip_index] = baked
        return baked

    def to_clip(self) -> BakedClip:
        return self._bake_clip(self._clip_index)


def load_fbx(path: str, clip_name: str = None, bind_fbx_path: str = None) -> FbxMotion:
    import kagra
    raw = kagra._engine.load_fbx_anim(path)
    if not raw:
        raise RuntimeError(f"No animation found in FBX: {path}")
    motion = FbxMotion(_raw_clips=raw)
    if clip_name:
        motion.use_clip(clip_name)
    if bind_fbx_path and os.path.exists(bind_fbx_path):
        motion.set_bind_from_fbx(bind_fbx_path)
    else:
        warnings.warn(f"Bind FBX not provided: {bind_fbx_path}. Using first animation frame as bind pose.")
    _ = motion.to_clip()  # ベイク実行
    print(f"[FBX] Loaded & baked: {path} | Clips: {motion.clip_names} | FPS: {motion.fps:.1f}")
    return motion
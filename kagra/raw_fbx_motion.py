# raw_fbx_motion.py - リターゲット補正を一切行わない、生のFBXモーション
from dataclasses import dataclass, field
from typing import List, Dict, Tuple

# Mixamo → VRM ボーン名マッピング（最小限）
MIXAMO_TO_VRM = {
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
    'RightUpLeg': 'J_Bip_R_UpperLeg',
    'RightLeg': 'J_Bip_R_LowerLeg',
    'RightFoot': 'J_Bip_R_Foot',
}

class RawFbxMotion:
    """補正一切なし。FBXのローカル回転と移動をそのままVRMに適用する"""
    def __init__(self, raw_clips):
        self._raw_clips = raw_clips
        self._clip_index = 0

    @property
    def clip_names(self) -> List[str]:
        return [c[0] for c in self._raw_clips]

    @property
    def frame_time(self) -> float:
        return self._raw_clips[self._clip_index][1]

    @property
    def fps(self) -> float:
        ft = self.frame_time
        return 1.0 / ft if ft > 0 else 30.0

    def use_clip(self, name: str):
        for i, (n, _, _) in enumerate(self._raw_clips):
            if n == name:
                self._clip_index = i
                return self
        raise ValueError(f"Clip '{name}' not found")

    def to_clip(self):
        _, frame_time, raw_frames = self._raw_clips[self._clip_index]
        clip = []
        for frame in raw_frames:
            bones = {}
            root_pos = (0.0, 0.0, 0.0)
            for name, tx, ty, tz, qx, qy, qz, qw, has_trans in frame:
                if name == 'Armature':
                    root_pos = (tx, ty, tz)   # 移動はそのまま
                else:
                    vrm_name = MIXAMO_TO_VRM.get(name, name)
                    bones[vrm_name] = (qx, qy, qz, qw)   # 回転もそのまま
            clip.append((bones, frame_time, root_pos))
        return clip
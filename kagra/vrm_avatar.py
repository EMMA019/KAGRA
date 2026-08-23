# kagra/vrm_avatar.py
"""
VrmAvatar - VRM キャラクターの統合管理クラス (Phase 7+)

アニメーション・スプリングボーン・ブレンドシェイプ・まばたき・
視線追従・リップシンク・IK・感情表情を 1 オブジェクトで管理する。
タコ化（スケール爆発）防止のセーフティパッチ適用済み。

Example::
    # セットアップ（on_enter で一度だけ）
    avatar = kagra.avatar("assets/Emma.vrm")
    avatar.load_motion("dance", "assets/dance.bvh")

    # Phase 7 機能を有効化
    avatar.enable_lookat()       # 視線追従
    avatar.enable_lipsync()      # リップシンク
    avatar.enable_ik()           # 腕 IK
    avatar.enable_emotion()      # 表情スムーズブレンド

    # 毎フレーム
    avatar.play("idle")
    avatar.look_at_screen(*kagra.mouse())           # マウスを見る
    avatar.say("Fcl_MTH_A", amplitude=0.7)         # リップシンク
    avatar.feel("joy", intensity=0.8)               # 感情表情
    avatar.update(dt)
    kagra.draw_vrm(avatar.vrm_id)
"""
from __future__ import annotations
import math
import random
import logging
from typing import Optional

log = logging.getLogger("kagra.vrm_avatar")


# ── 内部ユーティリティ（タコ化防止の正規化を徹底） ────────────

def _quat_normalize(q: list) -> list:
    l = math.sqrt(q[0]**2 + q[1]**2 + q[2]**2 + q[3]**2)
    # ノルムが0に近い異常値の場合は、安全な無回転(単位クォータニオン)を返す
    if l < 1e-8: return [0.0, 0.0, 0.0, 1.0]
    return [q[0]/l, q[1]/l, q[2]/l, q[3]/l]

def _qmul(a: list, b: list) -> list:
    ax,ay,az,aw = a; bx,by,bz,bw = b
    return _quat_normalize([aw*bx+ax*bw+ay*bz-az*by,
                            aw*by-ax*bz+ay*bw+az*bx,
                            aw*bz+ax*by-ay*bx+az*bw,
                            aw*bw-ax*bx-ay*by-az*bz])

def _slerp(a: list, b: list, t: float) -> list:
    dot = sum(a[i]*b[i] for i in range(4))
    if dot < 0:
        b = [-x for x in b]
        dot = -dot
    dot = min(1.0, dot)
    if dot > 0.9995:
        r = [a[i]+t*(b[i]-a[i]) for i in range(4)]
        return _quat_normalize(r)
    
    # ゼロ除算回避
    if dot < -0.9995:
        return _quat_normalize(a)
        
    th0 = math.acos(dot); th = th0*t
    sin0 = math.sin(th0)
    if abs(sin0) < 1e-8:
        return _quat_normalize(a)
        
    sa = math.sin(th0-th) / sin0
    sb = math.sin(th) / sin0
    return _quat_normalize([sa*a[i]+sb*b[i] for i in range(4)])

def _euler_to_quat(rx: float, ry: float, rz: float) -> list:
    cx,sx = math.cos(rx/2), math.sin(rx/2)
    cy,sy = math.cos(ry/2), math.sin(ry/2)
    cz,sz = math.cos(rz/2), math.sin(rz/2)
    return _quat_normalize([sx*cy*cz+cx*sy*sz, cx*sy*cz-sx*cy*sz,
                            cx*cy*sz+sx*sy*cz, cx*cy*cz-sx*sy*sz])

_ID = [0., 0., 0., 1.]

def _send_bone_rot(vrm_id: int, name: str, q: list):
    import kagra
    kagra.get_engine().set_vrm_bone_rot(vrm_id, name, q[0], q[1], q[2], q[3])

def _send_bone_trans(vrm_id: int, name: str, t: list):
    import kagra
    kagra.get_engine().set_vrm_bone_trans(vrm_id, name, t[0], t[1], t[2])

def _reset_pose(vrm_id: int):
    import kagra
    kagra.get_engine().reset_vrm_pose(vrm_id)

def _set_shape(vrm_id: int, name: str, w: float) -> bool:
    """ブレンドシェイプを設定。未登録名は False、成功は True を返す。"""
    import kagra
    try:
        kagra.get_engine().set_blend_shape(vrm_id, name, w)
        return True
    except Exception as e:
        log.debug("set_blend_shape(%s) failed: %s", name, e)
        return False


# ── プリセットクリップ ─────────────────────────────────────────

def _make_walk(speed=1.0, arm=0.40, leg=0.45, lean=0.07) -> list:
    """対向アーム・脚、膝のスイング屈曲、骨盤の微小揺動を入れた歩行。"""
    frames = []
    n = 16
    for i in range(n):
        ph = i / n * 2 * math.pi
        # 脚: 左右逆相
        ll = leg * math.sin(ph)
        lr = -leg * math.sin(ph)
        # 膝: 脚が後ろへ引かれる相で屈曲（接地クリアランス）
        kl = max(0.0, -math.cos(ph)) * leg * 0.95
        kr = max(0.0, math.cos(ph)) * leg * 0.95
        # 足首: 膝に連動して少し戻す
        fl = -kl * 0.4 + ll * 0.12
        fr = -kr * 0.4 + lr * 0.12
        # 腕: 脚と逆相（右手前 ↔ 左足前）
        al = arm * math.sin(ph + math.pi)
        ar = -arm * math.sin(ph + math.pi)
        el = max(0.05, -al) * 0.55
        er = max(0.05, ar) * 0.55
        # 骨盤: 前後リーン + 左右ロール + 微小ヨー
        roll = math.sin(ph) * 0.045
        yaw = math.sin(ph) * 0.03
        bob = math.sin(ph * 2.0) * 0.025  # 上下はヒップ pitch で近似
        frames.append(({
            "J_Bip_L_UpperLeg": (ll, 0, 0),
            "J_Bip_R_UpperLeg": (lr, 0, 0),
            "J_Bip_L_LowerLeg": (-kl, 0, 0),
            "J_Bip_R_LowerLeg": (-kr, 0, 0),
            "J_Bip_L_Foot": (fl, 0, 0),
            "J_Bip_R_Foot": (fr, 0, 0),
            "J_Bip_L_UpperArm": (al, 0, -1.15),
            "J_Bip_R_UpperArm": (ar, 0, 1.15),
            "J_Bip_L_LowerArm": (el, 0, 0),
            "J_Bip_R_LowerArm": (er, 0, 0),
            "J_Bip_C_Hips": (lean * 0.35 + bob, yaw, roll),
            "J_Bip_C_Spine": (lean * 0.45, -yaw * 0.5, roll * 0.5),
            "J_Bip_C_Chest": (lean * 0.25, -yaw * 0.3, 0),
            "J_Bip_C_Neck": (-lean * 0.25, 0, 0),
        }, 1.0 / (n * speed)))
    return frames


PRESETS: dict[str, list] = {
    "idle": [({
        "J_Bip_L_UpperArm": (0, 0,-1.2), "J_Bip_R_UpperArm": (0, 0, 1.2),
        "J_Bip_L_LowerArm": (.2, 0, 0),  "J_Bip_R_LowerArm": (.2, 0, 0),
    }, .4)],
    "walk":  _make_walk(speed=1.2, arm=.45, leg=.45, lean=.08),
    "run":   _make_walk(speed=2.4, arm=.75, leg=.70, lean=.22),
    "sneak": _make_walk(speed=0.5, arm=.10, leg=.20, lean=.05),
    "bind":  [({}, .3)],
    "bow": [
        ({"J_Bip_C_Hips":(0.6,0,0),"J_Bip_C_Spine":(0.4,0,0),
          "J_Bip_C_Chest":(0.3,0,0),"J_Bip_C_Neck":(-0.3,0,0),
          "J_Bip_L_UpperArm":(0,0,-0.6),"J_Bip_R_UpperArm":(0,0,0.6)}, 0.6),
        ({"J_Bip_C_Hips":(0.6,0,0),"J_Bip_C_Spine":(0.4,0,0),
          "J_Bip_C_Chest":(0.3,0,0),"J_Bip_C_Neck":(-0.3,0,0)}, 0.8),
        ({}, 0.5),
    ],
    "arm_up": [({
        "J_Bip_L_UpperArm":(0,0,-1.5),"J_Bip_R_UpperArm":(0,0,1.5),
        "J_Bip_L_LowerArm":(0,0,-0.3),"J_Bip_R_LowerArm":(0,0,0.3),
    }, .4)],
    "kiss": [
        ({"J_Bip_C_Spine":(-0.15,0,0),
          "J_Bip_R_UpperArm":(-1.6,0,0.4),"J_Bip_R_LowerArm":(-1.2,0,0),
          "J_Bip_R_Hand":(0.3,0,0.2),
          "J_Bip_L_UpperArm":(0,0,-0.8),"J_Bip_L_LowerArm":(0.3,0,0)}, 0.5),
        ({"J_Bip_C_Spine":(-0.20,0,0.15),
          "J_Bip_R_UpperArm":(-2.0,0,1.0),"J_Bip_R_LowerArm":(-0.6,0,0),
          "J_Bip_L_UpperArm":(0.1,0,-1.0),"J_Bip_L_LowerArm":(0.5,0,0)}, 0.25),
        ({}, 0.6),
    ],
    "kiss_both": [
        ({"J_Bip_L_UpperArm":(-1.2,0,1.4),"J_Bip_L_LowerArm":(-0.5,0,0),
          "J_Bip_R_UpperArm":(-1.2,0,-1.4),"J_Bip_R_LowerArm":(-0.5,0,0)}, 0.25),
        ({"J_Bip_L_UpperArm":(-1.3,0,1.5),"J_Bip_R_UpperArm":(-1.3,0,-1.5)}, 0.5),
        ({}, 0.7),
    ],
    "wave": [
        ({"J_Bip_R_UpperArm":(-0.5,0,-1.2),"J_Bip_R_LowerArm":(0,0,0)},   0.3),
        ({"J_Bip_R_UpperArm":(-0.5,0,-1.2),"J_Bip_R_LowerArm":(0.6,0,0)}, 0.2),
        ({"J_Bip_R_UpperArm":(-0.5,0,-1.2),"J_Bip_R_LowerArm":(0,0,0)},   0.2),
        ({"J_Bip_R_UpperArm":(-0.5,0,-1.2),"J_Bip_R_LowerArm":(0.6,0,0)}, 0.2),
        ({}, 0.4),
    ],
    # Phase 7 追加プリセット
    "nod": [
        ({"J_Bip_C_Neck":(0.3,0,0),"J_Bip_C_Head":(0.2,0,0)}, 0.25),
        ({}, 0.25),
        ({"J_Bip_C_Neck":(0.3,0,0),"J_Bip_C_Head":(0.2,0,0)}, 0.25),
        ({}, 0.3),
    ],
    "shake_head": [
        ({"J_Bip_C_Neck":(0,0.3,0),"J_Bip_C_Head":(0,0.2,0)}, 0.2),
        ({"J_Bip_C_Neck":(0,-0.3,0),"J_Bip_C_Head":(0,-0.2,0)}, 0.2),
        ({"J_Bip_C_Neck":(0,0.3,0),"J_Bip_C_Head":(0,0.2,0)}, 0.2),
        ({}, 0.25),
    ],
    "think": [({
        "J_Bip_R_UpperArm":(0,0,-1.4),"J_Bip_R_LowerArm":(1.2,0,0),
        "J_Bip_R_Hand":(0.2,0,0),
        "J_Bip_C_Neck":(-0.1,0.15,0),
    }, 0.5)],
    "point": [({
        "J_Bip_R_UpperArm":(-0.8,0,-0.8),"J_Bip_R_LowerArm":(0.3,0,0),
        "J_Bip_R_Hand":(0,0,0.2),
    }, 0.4)],
}


# 上半身レイヤー対象（名前部分一致）
_UPPER_BONE_KEYS = (
    "Spine", "Chest", "Neck", "Head",
    "Shoulder", "Arm", "Hand", "Finger",
    "Thumb", "Index", "Middle", "Ring", "Little",
)

# 指ボーン判定（クリップが指を動かすかどうかの検出に使う）
_FINGER_KEYS = ("Thumb", "Index", "Middle", "Ring", "Little")

def _is_finger_bone(name: str) -> bool:
    return any(k in name for k in _FINGER_KEYS)

def _is_upper_bone(name: str) -> bool:
    return any(k in name for k in _UPPER_BONE_KEYS)


# ── アニメーター（内部クラス）────────────────────────────────

class _Animator:
    def __init__(self, vrm_id: int, bind_rots: dict = None, bone_filter=None):
        self.vrm_id      = vrm_id
        self._clip       = ""
        self._frames:  list = []
        self._fidx       = 0
        self._t          = 0.0
        self._loop       = False
        self._playing    = False
        self._from:  dict = {}
        # 初期化時点で current_rots を安全に保護（ゼロベクトル混入防止）
        self.current_rots: dict = {}
        self._on_finish  = None
        self._clips: dict = dict(PRESETS)
        self._bind_rots: dict = bind_rots or {}
        # クリップ切替クロスフェード
        self._cross_from: dict = {}
        self._cross_t = 0.0
        self._cross_dur = 0.0
        # False = In Place（ルート移動をエンジンに書かない）
        self.root_motion: bool = False
        # None = 全ボーン / callable(name)->bool でフィルタ
        self._bone_filter = bone_filter

    @property
    def clip(self) -> str:   return self._clip
    @property
    def playing(self) -> bool: return self._playing

    def play(self, name: str, loop: bool, on_finish=None, fade: float = 0.2):
        if name not in self._clips:
            print(f"[VrmAvatar] unknown clip '{name}'. available: {sorted(self._clips)}")
            return
        if self._clip == name and self._loop and self._playing:
            return

        # 切替時: 現在ポーズから fade 秒かけて新クリップへ
        if self.current_rots and fade > 0.0 and self._clip and self._clip != name:
            self._cross_from = {k: list(v) for k, v in self.current_rots.items()}
            self._cross_t = 0.0
            self._cross_dur = float(fade)
        else:
            self._cross_from = {}
            self._cross_dur = 0.0

        self._clip      = name
        self._frames    = self._clips[name]
        self._fidx      = 0
        self._t         = 0.0
        self._loop      = loop
        self._playing   = True
        
        # モーション開始時に現在のポーズを確実にスナップショットする
        self._from      = {k: list(v) for k, v in self.current_rots.items()}
        # 初回再生時（current_rots が空）はバインドポーズから補間開始（浮き上がり防止）
        if not self._from and self._bind_rots:
            for frame in self._frames:
                bones_dict = frame[0]
                if isinstance(bones_dict, dict):
                    for bone_name in bones_dict:
                        if bone_name not in self._from and bone_name in self._bind_rots:
                            self._from[bone_name] = list(self._bind_rots[bone_name])
        self._on_finish = on_finish

    def register(self, name: str, frames: list):
        self._clips[name] = frames

    def _accept_bone(self, name: str) -> bool:
        if self._bone_filter is None:
            return True
        return bool(self._bone_filter(name))

    def _bind_quat(self, name: str) -> list:
        """バインド回転。VRMA の hips / leftUpperArm も J_Bip_* に解決する。"""
        q = self._bind_rots.get(name)
        if q:
            return q
        from kagra.vrma_player import _HUMANOID_TO_VRM
        alias = _HUMANOID_TO_VRM.get(name)
        if alias:
            q = self._bind_rots.get(alias)
            if q:
                return q
        return _ID

    def update(self, dt: float):
        import kagra
        if not self._playing or not self._frames: return
        
        frame = self._frames[self._fidx]
        bones = frame[0]
        dur   = frame[1]
        
        # ルートモーション: フラグ ON のときだけ offset を書く（In Place 既定）
        if self.root_motion and len(frame) > 2:
            rx, ry, rz = frame[2]
            kagra.get_engine().set_vrm_offset(self.vrm_id, float(rx), float(ry), float(rz))
            
        self._t = min(1.0, self._t + dt / max(0.01, dur))
        te = self._t * self._t * (3 - 2*self._t)

        target_rots: dict = {}

        if not bones:
            for n, qf in self._from.items():
                if not self._accept_bone(n):
                    continue
                bind_q = self._bind_quat(n)
                qn = _slerp(qf, bind_q, te)
                target_rots[n] = qn
            if self._t >= 1.0:
                if self._bone_filter is None:
                    _reset_pose(self.vrm_id)
                self.current_rots.clear()
                self._cross_dur = 0.0
                return
        else:
            for n, rot in bones.items():
                if not self._accept_bone(n):
                    continue
                if len(rot) == 7:
                    _send_bone_trans(self.vrm_id, n, list(rot[:3]))
                    qt = _quat_normalize(list(rot[3:7]))
                elif len(rot) == 4:
                    qt = _quat_normalize(list(rot))
                else:
                    qt = _euler_to_quat(*rot)
                
                bind_q = self._bind_quat(n)
                qt = _qmul(bind_q, qt)
                qf = self._from.get(n, bind_q)
                target_rots[n] = _slerp(qf, qt, te)

        # クロスフェード: 旧ポーズ → 新クリップ目標
        if self._cross_dur > 0.0 and self._cross_from:
            self._cross_t = min(self._cross_dur, self._cross_t + dt)
            w = self._cross_t / self._cross_dur
            w = w * w * (3 - 2 * w)
            names = set(self._cross_from) | set(target_rots)
            for n in names:
                if not self._accept_bone(n):
                    continue
                qa = self._cross_from.get(n, self._bind_quat(n))
                qb = target_rots.get(n, self._bind_quat(n))
                qn = _slerp(qa, qb, w)
                self.current_rots[n] = qn
                _send_bone_rot(self.vrm_id, n, qn)
            if self._cross_t >= self._cross_dur:
                self._cross_dur = 0.0
                self._cross_from = {}
        else:
            for n, qn in target_rots.items():
                self.current_rots[n] = qn
                _send_bone_rot(self.vrm_id, n, qn)

        if self._t >= 1.0:
            self._t    = 0.0
            self._from = {n: self.current_rots.get(n, self._bind_quat(n)) for n in (bones or {})}
            self._fidx += 1
            if self._fidx >= len(self._frames):
                if self._loop:
                    self._fidx = 0
                else:
                    self._playing = False
                    self._fidx    = 0
                    if self._on_finish:
                        self._on_finish()


# ══════════════════════════════════════════════════════════════
#  VrmAvatar (Phase 7+)
# ══════════════════════════════════════════════════════════════

class VrmAvatar:
    """VRM キャラクターの統合管理クラス (Phase 7+)。

    Phase 7 で追加された機能:
      - 視線追従 (look_at_screen / look_at_3d)
      - リップシンク (lipsync_amplitude / lipsync_wav)
      - 腕 IK (reach_right / reach_left)
      - 感情表情 (feel / feel_from_text)
      - 新プリセット: nod / shake_head / think / point

    Example::
        avatar = kagra.avatar("assets/Emma.vrm")
        avatar.enable_lookat()
        avatar.enable_lipsync()
        avatar.enable_emotion()

        def update(dt):
            avatar.play("idle")
            avatar.look_at_screen(*kagra.mouse())
            avatar.feel("joy")
            avatar.update(dt)
            kagra.draw_vrm(avatar.vrm_id)
    """

    def __init__(self, vrm_path: str):
        import kagra
        self.vrm_path = vrm_path
        self.vrm_id   = kagra.load_vrm(vrm_path)
        self._bind_rots, self._bind_worlds, self._bind_trans = self._load_bind_pose(vrm_path)
        self._anim    = _Animator(self.vrm_id, self._bind_rots)
        # 上半身レイヤー（クリップ辞書はベースと共有）
        self._upper   = _Animator(self.vrm_id, self._bind_rots, bone_filter=_is_upper_bone)
        self._upper._clips = self._anim._clips
        self._upper.root_motion = False
        self._spring  = None

        # Phase 7 サブシステム（enable_*() で初期化）
        self._lookat:  Optional[object] = None
        self._lipsync: Optional[object] = None
        self._ik:      Optional[object] = None
        self._emotion: Optional[object] = None

        # まばたき
        self._blink_enabled = True
        self._blink_t     = 0.0
        self._blink_next  = random.uniform(2.5, 5.0)
        self._blink_phase = 0.0
        self._blink_closing = True   # まばたきの方向フラグ
        self._blink_l: Optional[str] = None
        self._blink_r: Optional[str] = None
        self._expr_shapes: list[str] = []

        # SpringBone
        try:
            from kagra.vrm_spring import SpringBone
            self._spring = SpringBone(vrm_path, self.vrm_id)
            print(f"[VrmAvatar] SpringBone: {len(self._spring.chains)} chains")
        except Exception as e:
            log.warning("[VrmAvatar] SpringBone skipped: %s", e)

        # ブレンドシェイプ名を探索（VRM 1.0 / 0.x 両対応）
        try:
            shapes = set(kagra.list_blend_shapes(self.vrm_id))
            EXPR = {
                # VRM 1.0
                "happy", "angry", "sad", "relaxed", "surprised", "neutral",
                # VRM 0.x
                "Joy", "Angry", "Sorrow", "Fun", "Surprised", "Neutral",
                "Fcl_ALL_Joy", "Fcl_ALL_Angry", "Fcl_ALL_Sorrow",
                "Fcl_ALL_Fun", "Fcl_ALL_Surprised", "Fcl_ALL_Neutral",
            }
            self._expr_shapes = [s for s in shapes if s in EXPR]
            self._blink_l = next(
                (s for s in ["blinkLeft", "Blink_L", "Fcl_EYE_Blink_L", "blink_l",
                             "blink", "Blink", "Fcl_EYE_Blink"]
                 if s in shapes), None)
            self._blink_r = next(
                (s for s in ["blinkRight", "Blink_R", "Fcl_EYE_Blink_R", "blink_r"]
                 if s in shapes), self._blink_l)
        except Exception as e:
            log.warning("[VrmAvatar] blendshape discovery failed: %s", e)

        self._first_person = False
        print(f"[VrmAvatar] Loaded: {vrm_path}")

    def _load_bind_rots(self, vrm_path: str) -> dict:
        """VRM ファイルからバインドポーズの回転（クォータニオン xyzw）を読み込む。"""
        locals_, _worlds, _trans = self._load_bind_pose(vrm_path)
        return locals_

    def _hips_bind_y(self) -> float:
        """VRM Hips の bind 高さ（メートル）。Mixamo cm は使わない。"""
        t = getattr(self, "_bind_trans", None) or {}
        y = t.get("J_Bip_C_Hips") or t.get("hips")
        if y is not None and 0.2 <= float(y) <= 2.5:
            return float(y)
        return 0.853

    def _load_bind_pose(self, vrm_path: str) -> tuple[dict, dict, dict]:
        """ノード名 → ローカル / ワールド bind 回転 + ローカル Y 平移。"""
        import json, struct
        try:
            with open(vrm_path, 'rb') as f:
                data = f.read()
            if data[:4] != b'glTF':
                return {}, {}, {}
            offset = 12
            json_bytes = None
            while offset + 8 <= len(data):
                chunk_len = struct.unpack('<I', data[offset:offset+4])[0]
                chunk_type = struct.unpack('<I', data[offset+4:offset+8])[0]
                if chunk_type == 0x4E4F534A:
                    json_bytes = data[offset+8:offset+8+chunk_len]
                    break
                offset += 8 + chunk_len
            if not json_bytes:
                return {}, {}, {}
            gltf = json.loads(json_bytes.decode('utf-8').rstrip('\0'))
            from kagra.vrma_player import _world_rest_rots
            nodes = gltf.get('nodes', [])
            rest_by_i = {}
            locals_ = {}
            trans_ = {}
            for i, node in enumerate(nodes):
                r = node.get('rotation', [0.0, 0.0, 0.0, 1.0])
                q = (float(r[0]), float(r[1]), float(r[2]), float(r[3]))
                rest_by_i[i] = q
                name = node.get('name', '')
                if name:
                    locals_[name] = list(q)
                    t = node.get('translation') or [0.0, 0.0, 0.0]
                    trans_[name] = float(t[1])
            worlds_i = _world_rest_rots(nodes, rest_by_i)
            worlds = {}
            for i, node in enumerate(nodes):
                name = node.get('name', '')
                if name and i in worlds_i:
                    worlds[name] = list(worlds_i[i])
            return locals_, worlds, trans_
        except Exception as e:
            log.warning("[VrmAvatar] bind rot 読み込み失敗: %s", e)
            return {}, {}, {}

    def get_bind_rot(self, bone_name: str):
        """指定ボーンのバインドポーズ回転（クォータニオン xyzw）を返す。"""
        return self._bind_rots.get(bone_name)

    # ── Phase 7 サブシステム初期化 ────────────────────────────

    def enable_facetracking(
        self,
        camera_id: int = 0,
        use_iris: bool = False,
        smooth_speed: float = 6.0,
    ) -> "FaceTracker":
        """顔トラッキング（MediaPipe Face Mesh）を有効化して FaceTracker を返す。

        依存: pip install mediapipe opencv-python

        Args:
            camera_id:  カメラデバイスID（0=内蔵カメラ）
            use_iris:   虹彩トラッキングを使用するか（精度向上）
            smooth_speed: 頭部回転の追従速度

        Example::
            ft = avatar.enable_facetracking(camera_id=0)
            ft.enable()

            def update(dt):
                ft.update(dt)
                kagra.draw_vrm(avatar.vrm_id)

            def draw():
                ft.draw_debug()
        """
        from kagra.vrm_facetrack import FaceTracker
        self._facetrack = FaceTracker(
            self,
            camera_id=camera_id,
            use_iris=use_iris,
            smooth_speed=smooth_speed,
        )
        return self._facetrack

    def enable_lookat(
        self,
        eye_height: float = 1.5,
        smooth_speed: float = 6.0,
        head_weight: float = 0.6,
        neck_weight: float = 0.4,
    ) -> "LookAtController":

        """視線追従を有効化して LookAtController を返す。

        Args:
            eye_height:   モデルの目の高さ（ワールド Y 座標）
            smooth_speed: 追従速度（大きいほど速い）
            head_weight:  頭ボーンへの適用ウェイト
            neck_weight:  首ボーンへの適用ウェイト

        Example::
            avatar.enable_lookat(eye_height=1.55)
        """
        from kagra.vrm_lookat import LookAtController
        self._lookat = LookAtController(
            self,
            eye_height   = eye_height,
            smooth_speed = smooth_speed,
            head_weight  = head_weight,
            neck_weight  = neck_weight,
        )
        return self._lookat

    def enable_lipsync(
        self,
        smoothing: float = 0.4,
        max_open:  float = 0.9,
    ) -> "LipSyncController":
        """リップシンクを有効化して LipSyncController を返す。

        Example::
            ls = avatar.enable_lipsync()
            timeline = ls.analyze_wav("voice.wav")
            ls.play_timeline(timeline)
        """
        from kagra.vrm_lipsync import LipSyncController
        self._lipsync = LipSyncController(
            self,
            smoothing = smoothing,
            max_open  = max_open,
        )
        return self._lipsync

    def enable_ik(self, smooth_speed: float = 8.0) -> "ArmIK":
        """腕 IK を有効化して ArmIK を返す。

        Example::
            ik = avatar.enable_ik()
            ik.reach_right(tx=0.5, ty=1.0, tz=0.3)
        """
        from kagra.vrm_ik import ArmIK
        self._ik = ArmIK(self, smooth_speed=smooth_speed)
        return self._ik

    def enable_emotion(
        self,
        blend_speed: float = 2.5,
    ) -> "EmotionController":
        """感情表情スムーズブレンドを有効化して EmotionController を返す。

        Example::
            avatar.enable_emotion()
            avatar.feel("joy", 0.8)
        """
        from kagra.vrm_emotion import EmotionController
        self._emotion = EmotionController(self, blend_speed=blend_speed)
        return self._emotion

    # ── Phase 7 シンプル API ──────────────────────────────────

    def look_at_screen(
        self,
        sx: float, sy: float,
        screen_w: float = 1280, screen_h: float = 720,
    ):
        """スクリーン座標を見る。enable_lookat() が必要。

        Example::
            mx, my = kagra.mouse()
            avatar.look_at_screen(mx, my)
        """
        if self._lookat:
            self._lookat.look_at_screen(sx, sy, screen_w, screen_h)

    def look_at_3d(self, tx: float, ty: float, tz: float,
                   ax: float = 0.0, az: float = 0.0):
        """3D 空間の点を見る。enable_lookat() が必要。"""
        if self._lookat:
            self._lookat.look_at_3d(tx, ty, tz, ax, az)

    def look_reset(self):
        """正面を向く（視線リセット）。"""
        if self._lookat:
            self._lookat.reset()

    def feel(self, emotion: str, intensity: float = 1.0):
        """感情表情を設定する。enable_emotion() が必要。

        Args:
            emotion:   "joy" / "angry" / "sorrow" / "fun" / "surprised"
                       / "neutral" / "shy" / "excited" / "loving" 等
            intensity: 強度（0.0〜1.0）

        Example::
            avatar.feel("joy", 0.8)
            avatar.feel("neutral")
        """
        if self._emotion:
            self._emotion.set(emotion, intensity)

    def feel_from_text(self, text: str, intensity: float = 0.8) -> str:
        """テキストから感情を自動推定して設定する。enable_emotion() が必要。

        Returns:
            推定された感情名

        Example::
            emotion = avatar.feel_from_text("ありがとう！嬉しい！")
            print(emotion)  # → "joy"
        """
        if self._emotion:
            return self._emotion.from_text(text, intensity)
        return "neutral"

    def lipsync_amplitude(self, amplitude: float, vowel: str = "aa"):
        """リアルタイム振幅でリップシンク。enable_lipsync() が必要。

        Args:
            amplitude: 音量（0.0〜1.0）
            vowel:     母音ヒント ("aa"/"ih"/"ou"/"ee"/"oh")

        Example::
            avatar.lipsync_amplitude(mic_volume)
        """
        if self._lipsync:
            self._lipsync.set_amplitude(amplitude, vowel)

    def lipsync_wav(self, wav_path: str) -> bool:
        """WAV ファイルを解析してリップシンクを開始する。enable_lipsync() が必要。

        Args:
            wav_path: WAV ファイルパス

        Returns:
            True = 開始成功, False = 失敗

        Example::
            avatar.lipsync_wav("assets/voice/hello.wav")
        """
        if self._lipsync:
            timeline = self._lipsync.analyze_wav(wav_path)
            if len(timeline) > 0:
                self._lipsync.play_timeline(timeline)
                return True
        return False

    def lipsync_text(self, text: str, duration: float):
        """テキストから簡易リップシンクを生成して再生。enable_lipsync() が必要。

        TTS ライブラリと組み合わせる場合は lipsync_wav() を推奨。

        Example::
            avatar.lipsync_text("こんにちは！", duration=1.5)
        """
        if self._lipsync:
            self._lipsync.play_text(text, duration)

    def speak_voicevox(
        self,
        text: str,
        *,
        speaker: int = 3,
        url: str = "http://localhost:50021",
        play: bool = True,
    ) -> str:
        """VOICEVOX で発話して口を動かす。エンジンは同梱しない。

        Example::
            avatar.speak_voicevox("こんにちは")
        """
        from kagra.voicevox import speak

        if self._lipsync is None:
            self.enable_lipsync()
        return speak(self, text, speaker=speaker, url=url, play=play)

    def reach_right(self, tx: float, ty: float, tz: float, weight: float = 1.0):
        """右手を3D位置へ向ける。enable_ik() が必要。

        Example::
            avatar.reach_right(0.5, 1.0, 0.3)
        """
        if self._ik:
            self._ik.reach_right(tx, ty, tz, weight)

    def reach_left(self, tx: float, ty: float, tz: float, weight: float = 1.0):
        """左手を3D位置へ向ける。enable_ik() が必要。"""
        if self._ik:
            self._ik.reach_left(tx, ty, tz, weight)

    # ── ボーン操作（内部 + 外部から使えるように公開） ───────────

    def _send_bone(self, name: str, q: list):
        """ボーン回転を送る(サブシステムから呼ばれる)。"""
        _send_bone_rot(self.vrm_id, name, q)

    # ── アニメーション ─────────────────────────────────────────

    def play(self, clip: str, loop: bool = True, on_finish=None, fade: float = 0.2):
        """クリップを再生する（全身）。上半身レイヤーは自動では消さない。

        組み込み: idle / walk / run / sneak / bind /
                  bow / arm_up / kiss / kiss_both / wave /
                  nod / shake_head / think / point   ← Phase 7 追加

        Args:
            clip:      クリップ名
            loop:      ループ再生するか
            on_finish: 非ループ完了時コールバック
            fade:      前クリップからのクロスフェード秒（0 で即切替）
        """
        prev_clip = self._anim._clip
        self._anim.play(clip, loop=loop, on_finish=on_finish, fade=fade)
        if prev_clip != clip:
            # クリップ遷移時の SpringBone 暴発防止
            if self._spring:
                self._spring.reset()
            if not self._is_bvh_clip() and self.root_motion:
                import kagra
                kagra.get_engine().set_vrm_offset(self.vrm_id, 0.0, 0.0, 0.0)

    def play_upper(self, clip: Optional[str], loop: bool = True, fade: float = 0.15):
        """上半身のみの第2レイヤーを再生する。

        Spine / Chest / Neck / Head / Shoulder / Arm / Hand / Finger 系ボーンだけ上書き。
        ``play_upper(None)`` または ``stop_upper()`` で解除。

        Args:
            clip: クリップ名。None で解除。
            loop: ループ再生するか
            fade: クロスフェード秒
        """
        if clip is None:
            self.stop_upper()
            return
        self._upper.play(clip, loop=loop, fade=fade)

    def stop_upper(self):
        """上半身レイヤーを停止する。"""
        self._upper._playing = False
        self._upper._clip = ""
        self._upper._cross_dur = 0.0
        self._upper._cross_from = {}

    def add_clip(self, name: str, frames: list):
        """カスタムクリップを登録する(低レベル API)。"""
        self._anim.register(name, frames)

    def add_motion(self, name: str, motion):
        """BvhMotion / FbxMotion / VrmaMotion をクリップとして登録する。"""
        from kagra.fbx_player import FbxMotion
        from kagra.vrma_player import VrmaMotion
        if isinstance(motion, FbxMotion):
            motion.vrm_hips_y = self._hips_bind_y()
            motion._cache = None
        clip = motion.to_clip()
        # dest 共役は VRMA の NormalizedLocalRotation 専用。
        # Mixamo / BVH のローカルデルタに掛けると腰・袖が潰れて骨格お化けになる。
        if isinstance(motion, VrmaMotion):
            clip = self._retarget_vrma_clip(clip)
        self._anim.register(name, clip)
        print(f"[VrmAvatar] '{name}': {len(clip)} frames @ {motion.fps:.1f}fps")

    def _retarget_vrma_clip(self, clip: list) -> list:
        """NormalizedLocalRotation を VRM の rest ワールドに載せる。"""
        from kagra.vrma_player import dest_delta_from_normalized
        worlds = getattr(self, "_bind_worlds", None) or {}
        if not worlds:
            return clip
        out = []
        for frame in clip:
            bones = frame[0]
            if not isinstance(bones, dict):
                out.append(frame)
                continue
            retargeted = {}
            for bone, q in bones.items():
                w = worlds.get(bone)
                retargeted[bone] = dest_delta_from_normalized(q, w) if w else list(q)
            out.append((retargeted,) + tuple(frame[1:]))
        return out

    def load_motion(self, name: str, path: str, extra_map: dict = None):
        """BVH / FBX / VRMA ファイルを読み込んでクリップとして登録する(1行 API)。"""
        low = path.lower()
        if low.endswith(".vrma"):
            from kagra.vrma_player import load_vrma
            motion = load_vrma(path)
        elif low.endswith((".glb", ".gltf")):
            from kagra.vrma_player import is_vrma, load_vrma
            if not is_vrma(path):
                raise ValueError(
                    f"not a VRMA file (missing VRMC_vrm_animation): {path}"
                )
            motion = load_vrma(path)
        elif low.endswith(".fbx"):
            from kagra.fbx_player import load_fbx
            motion = load_fbx(path)
        else:
            from kagra.bvh_player import load_bvh
            motion = load_bvh(path, extra_map=extra_map)
        self.add_motion(name, motion)

    def relax_hands(self, curl: float = 0.35, thumb: float = 0.25):
        """指を軽く曲げて自然な手にする。

        BVH など指データを持たないモーションでは指がバインドポーズ
        （開いた手）のまま固まって見えるので、その対策。
        クリップが指ボーンを動かし始めれば上書きされる。

        Args:
            curl:  人差し指〜小指の曲げ量（ラジアン）
            thumb: 親指の曲げ量（ラジアン）
        """
        # VRM は -Z 向き: 左手指は -X、右手指は +X に伸び、手のひらは下。
        # 手のひら側へ曲げる = 左 +Z / 右 -Z 回転。
        for side, sign in (("L", 1.0), ("R", -1.0)):
            for finger in ("Index", "Middle", "Ring", "Little"):
                for seg, amount in ((1, curl), (2, curl * 1.3), (3, curl * 0.8)):
                    name = f"J_Bip_{side}_{finger}{seg}"
                    delta = _euler_to_quat(0.0, 0.0, sign * amount)
                    bind_q = self._bind_rots.get(name, _ID)
                    _send_bone_rot(self.vrm_id, name, _qmul(bind_q, delta))
            for seg in (1, 2, 3):
                name = f"J_Bip_{side}_Thumb{seg}"
                delta = _euler_to_quat(0.0, -sign * thumb, 0.0)
                bind_q = self._bind_rots.get(name, _ID)
                _send_bone_rot(self.vrm_id, name, _qmul(bind_q, delta))

    def _clip_has_fingers(self, clip: str) -> bool:
        frames = self._anim._clips.get(clip) or []
        return any(
            _is_finger_bone(n)
            for frame in frames
            for n in (frame[0] if isinstance(frame[0], dict) else {})
        )

    def dance(self, clip: str = "dance", *, fade: float = 0.3):
        """踊る（1行 API）。

        クリップが未登録なら contracts のエイリアスから自動で読み込む。
        既定の "dance" はパッケージ同梱の synthetic_dance.bvh に
        解決されるため、外部アセットなしで動く（pip インストール後も含む）。
        `.vrma`（VRM Animation）もそのまま渡せる。
        指データを持たないクリップでは relax_hands() で手を自然に曲げる。

        Example::
            av = kagra.avatar("Emma")
            av.dance()                      # 同梱ダンス
            av.dance("assets/wave.vrma")    # VRM Animation
            av.dance("assets/hiphop.fbx")   # 自分のモーション
        """
        if clip not in self._anim._clips:
            from kagra.contracts import AssetKind, resolve_asset
            path = resolve_asset(AssetKind.ANY, clip)
            self.load_motion(clip, str(path))
        self.play(clip, loop=True, fade=fade)
        if not self._clip_has_fingers(clip):
            self.relax_hands()

    def sing(self, audio: str = None, *, volume: float = 1.0, loop: bool = False) -> float:
        """歌う（1行 API）。リップシンクを自動で有効化し、音声を再生する。

        Args:
            audio:  WAV パスまたは contracts エイリアス。省略時は内蔵ソングを
                    その場で合成して歌う（外部アセット不要）。
            volume: 再生音量（0.0〜1.0）
            loop:   True なら曲と口パクを繰り返す（``kagra.bgm``）。

        Returns:
            曲の長さ（秒）

        Example::
            av.sing()                   # 内蔵ソング
            av.sing("assets/song.wav")  # 自分の曲（波形から口パク解析）
            av.sing("song.wav", loop=True)
        """
        import kagra
        from kagra.vrm_lipsync import LipSyncTimeline

        if self._lipsync is None:
            self.enable_lipsync(smoothing=0.22, max_open=0.95)
        else:
            self._lipsync.smoothing = min(self._lipsync.smoothing, 0.28)

        if audio is None:
            # 内蔵ソング: 音符列から母音タイムラインが正確に出る
            from kagra.song import generate_song
            path, entries, duration = generate_song()
            timeline = LipSyncTimeline(entries, duration)
        else:
            from kagra.contracts import AssetKind, resolve_asset
            resolved = resolve_asset(AssetKind.AUDIO, audio, required=False)
            path = str(resolved) if resolved else audio
            timeline = self._lipsync.analyze_wav(path)
            duration = timeline.duration

        self._lipsync.play_timeline(timeline, loop=loop)
        try:
            if loop:
                kagra.bgm(path, loop=True, vol=volume)
            else:
                kagra.se(path, vol=volume)
        except Exception as e:
            log.warning("[VrmAvatar] 音声再生失敗（リップシンクは継続）: %s", e)
        return duration

    @property
    def clip(self) -> str:
        return self._anim.clip

    @property
    def playing(self) -> bool:
        return self._anim.playing

    @property
    def upper_clip(self) -> str:
        """上半身レイヤーのクリップ名（未再生なら空文字）。"""
        return self._upper.clip if self._upper.playing else ""

    @property
    def clips(self) -> list[str]:
        return sorted(self._anim._clips.keys())

    @property
    def expressions(self) -> list[str]:
        return list(self._expr_shapes)

    @property
    def root_motion(self) -> bool:
        """True のときクリップのルート移動を set_vrm_offset に書く。既定 False（In Place）。"""
        return self._anim.root_motion

    @root_motion.setter
    def root_motion(self, value: bool):
        self._anim.root_motion = bool(value)

    @property
    def blink_enabled(self) -> bool:
        return self._blink_enabled

    @blink_enabled.setter
    def blink_enabled(self, v: bool):
        self._blink_enabled = v

    @property
    def first_person(self) -> bool:
        """True のとき一人称レイヤー（頭を隠す）。"""
        return self._first_person

    @first_person.setter
    def first_person(self, v: bool):
        self.set_first_person(v)

    def pick(self, sx: float, sy: float, camera=None, max_dist: float = 100.0):
        """スクリーン座標のレイが当たった humanoid ボーン名。

        返す例: ``"head"`` / ``"leftHand"``。外れは None。
        なでる・叩く認識はエンジンの外。

        ``camera`` を省略すると ``kagra.get_camera3d()``、それも無ければ
        エンジンの現在カメラ。
        """
        import kagra
        cam = camera if camera is not None else kagra.get_camera3d()
        if cam is not None and hasattr(cam, "ray_from_screen"):
            ray = cam.ray_from_screen(float(sx), float(sy))
        else:
            ray = kagra.camera_ray_from_screen(float(sx), float(sy))
        if ray is None:
            return None
        origin, direction = ray
        return kagra.pick_vrm_bone(
            self.vrm_id,
            origin[0], origin[1], origin[2],
            direction[0], direction[1], direction[2],
            max_dist=float(max_dist),
        )

    def apply_pose(self, rots: dict):
        """ライブ体入力。humanoid 名 / ノード名 → クォータニオン。

        VR・Kinect・Holistic のキャプチャはこのエンジンに置かない。
        外部が 60Hz でこの dict を流せば、収録 FBX と同じ口で乗る。

        Example::
            av.apply_pose({"head": (0, 0.1, 0, 0.99), "hips": (0, 0, 0, 1)})
        """
        import kagra
        packed = []
        for name, q in (rots or {}).items():
            if q is None or len(q) < 4:
                continue
            packed.append((str(name), float(q[0]), float(q[1]), float(q[2]), float(q[3])))
        if packed:
            kagra.set_vrm_pose(self.vrm_id, packed)

    def set_first_person(self, enabled: bool = True):
        """一人称視点。頭 / ThirdPersonOnly メッシュを隠す。"""
        self._first_person = bool(enabled)
        try:
            import kagra
            kagra.set_vrm_first_person(self.vrm_id, self._first_person)
        except Exception as e:
            log.debug("set_first_person failed: %s", e)

    # ── サブシステムへの公開アクセサ ──────────────────────────
    # 以前は `avatar._spring` / `avatar._emotion` のようなプライベート属性を
    # デモ側から直接触る必要があったが、すべて読み取り専用プロパティで公開する。
    # None が返り得る点に注意(enable_*() 未呼び出しまたは初期化失敗時)。

    @property
    def spring_bone(self):
        """SpringBone インスタンス(未初期化なら None)。"""
        return self._spring

    @property
    def facetrack(self):
        """FaceTracker インスタンス(enable_facetracking() 後のみ)。"""
        return getattr(self, '_facetrack', None)

    @property
    def emotion(self):
        """EmotionController インスタンス(enable_emotion() 後のみ)。"""
        return self._emotion

    @property
    def lookat(self):
        """LookAtController インスタンス(enable_lookat() 後のみ)。"""
        return self._lookat

    @property
    def lipsync(self):
        """LipSyncController インスタンス(enable_lipsync() 後のみ)。"""
        return self._lipsync

    @property
    def ik(self):
        """ArmIK インスタンス(enable_ik() 後のみ)。"""
        return self._ik

    # ── ポーズ／診断 ──────────────────────────────────────────

    def reset_pose(self):
        """全ボーンを VRM の rest ポーズへ戻し、SpringBone もリセットする。"""
        _reset_pose(self.vrm_id)
        self._anim.current_rots.clear()
        self._anim._from = {}
        self.stop_upper()
        self._upper.current_rots.clear()
        self._upper._from = {}
        if self._spring:
            self._spring.reset()

    def diagnostics(self) -> dict:
        """サブシステムの有効／無効・解決済みシェイプ等を一括取得する。

        Returns:
            dict: デバッグ HUD から `avatar.diagnostics()` で呼び出しやすい形式。
        """
        import kagra
        try:
            shapes = kagra.list_blend_shapes(self.vrm_id)
        except Exception:
            shapes = []
        info = {
            "vrm_path":   self.vrm_path,
            "vrm_id":     self.vrm_id,
            "blendshapes": sorted(shapes),
            "spring_bone": {
                "loaded":  self._spring is not None,
                "enabled": bool(self._spring and self._spring.enabled),
                "chains":  len(self._spring.chains) if self._spring else 0,
                "colliders": len(self._spring.colliders) if self._spring else 0,
            },
            "first_person": self._first_person,
            "lookat":  self._lookat  is not None,
            "lipsync": self._lipsync is not None,
            "ik":      self._ik      is not None,
            "emotion": None,
        }
        if self._emotion:
            info["emotion"] = self._emotion.diagnostics() \
                if hasattr(self._emotion, "diagnostics") \
                else {"resolved": dict(getattr(self._emotion, "_resolved", {}))}
        return info

    # ── 毎フレーム更新 ────────────────────────────────────────

    def update(self, dt: float):
        """毎フレーム呼ぶ。以下を順番に更新する:
        1. アニメーション
        2. スプリングボーン
        3. 視線追従 (有効時)
        4. IK (有効時)
        5. リップシンク (有効時)
        6. 感情表情 (有効時)
        7. 顔トラッキング (有効時)
        8. 自動まばたき
        """
        dt = min(dt, 0.05)
        self._frame_count = getattr(self, '_frame_count', 0) + 1

        # 1. アニメーション（ベース → 上半身レイヤーで上書き）
        self._anim.update(dt)
        self._upper.update(dt)
        self._apply_vrma_expressions()

        # 1.5. アクションコントローラー（アタッチされている場合、スプリングボーン前に適用）
        action_ctrl = getattr(self, '_action_controller', None)
        if action_ctrl:
            action_ctrl.update(dt)

        # 2. スプリングボーン(BVH再生中はスキップ)
        if self._spring and not self._is_bvh_clip():
            rots = dict(self._anim.current_rots)
            if self._upper.playing:
                rots.update(self._upper.current_rots)
            self._spring.update(dt, rots)

        # 3. 視線追従（VRMA LookAt があるフレームは目だけ後から上書き）
        if self._lookat:
            self._lookat.update(dt)
        self._apply_vrma_lookat()

        # 4. IK
        if self._ik:
            self._ik.update(dt)

        # 5. リップシンク
        if self._lipsync:
            self._lipsync.update(dt)

        # 6. 感情表情
        if self._emotion:
            self._emotion.update(dt)

        # 7. 顔トラッキング
        if hasattr(self, '_facetrack') and self._facetrack and self._facetrack.enabled:
            self._facetrack.update(dt)

        # 8. まばたき（VRMA が blink を持っている間は任せる）
        if self._blink_enabled and not getattr(self, "_vrma_has_blink", False):
            self._update_blink(dt)

    def _apply_vrma_expressions(self):
        """VRMA クリップの 4 要素目（表情ウェイト）をブレンドシェイプへ書く。"""
        frames = self._anim._frames
        prev = getattr(self, "_vrma_expr_on", set())
        exprs = None
        if self._anim.playing and frames:
            frame = frames[self._anim._fidx]
            if len(frame) > 3 and isinstance(frame[3], dict):
                exprs = frame[3]
        self._vrma_has_blink = bool(
            exprs and any(str(k).lower().startswith("blink") for k in exprs)
        )
        if not exprs:
            for name in prev:
                _set_shape(self.vrm_id, name, 0.0)
            self._vrma_expr_on = set()
            return
        import kagra
        try:
            available = set(kagra.list_blend_shapes(self.vrm_id))
        except Exception:
            available = set()
        from kagra.vrma_player import resolve_expression_name
        on: set[str] = set()
        for raw, w in exprs.items():
            name = resolve_expression_name(str(raw), available) or str(raw)
            if _set_shape(self.vrm_id, name, float(w)):
                on.add(name)
        for name in prev - on:
            _set_shape(self.vrm_id, name, 0.0)
        self._vrma_expr_on = on

    def _apply_vrma_lookat(self):
        """VRMA ``lookAt`` ノードの yaw/pitch を目ブレンドシェイプへ書く。"""
        frames = self._anim._frames
        look = None
        if self._anim.playing and frames:
            frame = frames[self._anim._fidx]
            if len(frame) > 4 and frame[4]:
                look = frame[4]
        prev = getattr(self, "_vrma_look_on", set())
        if not look:
            for name in prev:
                _set_shape(self.vrm_id, name, 0.0)
            self._vrma_look_on = set()
            return
        yaw, pitch = float(look[0]), float(look[1])
        max_yaw = math.radians(20)
        max_pitch = math.radians(15)
        weights = {
            "lookLeft": max(0.0, -yaw / max_yaw),
            "lookRight": max(0.0, yaw / max_yaw),
            "lookUp": max(0.0, -pitch / max_pitch),
            "lookDown": max(0.0, pitch / max_pitch),
        }
        import kagra
        try:
            available = set(kagra.list_blend_shapes(self.vrm_id))
        except Exception:
            available = set()
        from kagra.vrma_player import resolve_expression_name
        on: set[str] = set()
        for raw, w in weights.items():
            if w < 1e-4:
                continue
            name = resolve_expression_name(raw, available) or raw
            if _set_shape(self.vrm_id, name, min(1.0, w)):
                on.add(name)
        for name in prev - on:
            _set_shape(self.vrm_id, name, 0.0)
        self._vrma_look_on = on

    def _is_bvh_clip(self) -> bool:
        if not self._anim.playing or not self._anim._frames:
            return False
        try:
            return len(self._anim._frames[0][0]) > 20
        except Exception:
            return False

    def _update_blink(self, dt: float):
        self._blink_t += dt
        if self._blink_phase == 0.0 and self._blink_t >= self._blink_next:
            self._blink_phase  = 0.001
            self._blink_t      = 0.0
            self._blink_next   = random.uniform(2.5, 5.5)
            self._blink_closing = True   # 閉じる方向から開始

        if self._blink_phase > 0.0:
            if self._blink_closing:
                # 閉じる（0→1）
                self._blink_phase = min(1.0, self._blink_phase + dt / 0.07)
                if self._blink_phase >= 1.0:
                    self._blink_closing = False   # 開く方向へ切り替え
            else:
                # 開く（1→0）
                self._blink_phase = max(0.0, self._blink_phase - dt / 0.10)

            # phase をそのまま weight として使う（1.0=完全閉眼, 0.0=開眼）
            w = self._blink_phase
            for bs in filter(None, [self._blink_l, self._blink_r]):
                _set_shape(self.vrm_id, bs, w)

    # ── 表情(旧 API 維持)───────────────────────────────────

    def set_expression(self, name: str, weight: float = 1.0):
        """表情を直接設定する(0.0〜1.0)。スムーズブレンドなし。

        スムーズにしたい場合は enable_emotion() + feel() を使う。
        """
        _set_shape(self.vrm_id, name, float(weight))

    def reset_expressions(self):
        """全ブレンドシェイプをゼロにリセット。"""
        import kagra
        kagra.reset_blend_shapes(self.vrm_id)

    # ── スプリングボーン ──────────────────────────────────────

    def set_wind(self, strength: float = 0.0, direction: tuple = (1., 0., 0.)):
        """スプリングボーンに風を設定する。"""
        if self._spring:
            self._spring.set_wind(strength, direction)
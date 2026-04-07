# kagra/vrm_avatar.py
"""
VrmAvatar - VRM キャラクターの統合管理クラス

アニメーション・スプリングボーン・ブレンドシェイプ・まばたきを
1オブジェクトで管理する。

Example::
    # セットアップ（on_enter で一度だけ）
    avatar = kagra.avatar("assets/Emma.vrm")
    avatar.load_motion("dance", "assets/dance.bvh")   # BVH を1行で登録

    # 毎フレーム（これだけ）
    avatar.play("dance")
    avatar.update(dt)           # アニメ + スプリングボーン + まばたき
    kagra.draw_vrm(avatar.vrm_id)

    # 表情
    avatar.set_expression("Fcl_ALL_Joy", 0.8)
    avatar.reset_expressions()

    # 利用可能クリップ / 表情一覧
    print(avatar.clips)
    print(avatar.expressions)
"""
from __future__ import annotations
import math
import random
from typing import Optional


# ── 内部ユーティリティ ────────────────────────────────────────

def _qmul(a: list, b: list) -> list:
    ax,ay,az,aw = a; bx,by,bz,bw = b
    return [aw*bx+ax*bw+ay*bz-az*by,
            aw*by-ax*bz+ay*bw+az*bx,
            aw*bz+ax*by-ay*bx+az*bw,
            aw*bw-ax*bx-ay*by-az*bz]

def _slerp(a: list, b: list, t: float) -> list:
    dot = sum(a[i]*b[i] for i in range(4))
    if dot < 0: b=[-x for x in b]; dot=-dot
    dot = min(1.0, dot)
    if dot > 0.9995:
        r = [a[i]+t*(b[i]-a[i]) for i in range(4)]
        l = math.sqrt(sum(x*x for x in r)) or 1e-8
        return [x/l for x in r]
    th0 = math.acos(dot); th = th0*t
    sa = math.sin(th0-th)/math.sin(th0); sb = math.sin(th)/math.sin(th0)
    return [sa*a[i]+sb*b[i] for i in range(4)]

def _euler_to_quat(rx: float, ry: float, rz: float) -> list:
    """XYZ オイラー角（ラジアン）→ クォータニオン [x,y,z,w]"""
    cx,sx = math.cos(rx/2), math.sin(rx/2)
    cy,sy = math.cos(ry/2), math.sin(ry/2)
    cz,sz = math.cos(rz/2), math.sin(rz/2)
    return [sx*cy*cz+cx*sy*sz, cx*sy*cz-sx*cy*sz,
            cx*cy*sz+sx*sy*cz, cx*cy*cz-sx*sy*sz]

_ID = [0., 0., 0., 1.]

def _send_bone_rot(vrm_id: int, name: str, q: list):
    """ボーン回転を engine に送る（engine アクセスをここに集約）"""
    import kagra
    kagra._engine.set_vrm_bone_rot(vrm_id, name, q[0], q[1], q[2], q[3])

def _send_bone_trans(vrm_id: int, name: str, t: list):
    """ボーン並進を engine に送る"""
    import kagra
    kagra._engine.set_vrm_bone_trans(vrm_id, name, t[0], t[1], t[2])

def _reset_pose(vrm_id: int):
    import kagra
    kagra._engine.reset_vrm_pose(vrm_id)

def _set_shape(vrm_id: int, name: str, w: float):
    import kagra
    try: kagra._engine.set_blend_shape(vrm_id, name, w)
    except Exception: pass


# ── プリセットクリップ ─────────────────────────────────────────

def _make_walk(speed=1.0, arm=0.40, leg=0.45, lean=0.07) -> list:
    frames = []
    for i in range(8):
        ph = i/8 * 2*math.pi
        ll =  leg*math.sin(ph);          lr = -leg*math.sin(ph)
        kl = max(0,  leg*.5*math.sin(ph+.5))
        kr = max(0, -leg*.5*math.sin(ph+math.pi+.5))
        al =  arm*math.sin(ph+math.pi);  ar = -arm*math.sin(ph+math.pi)
        el = max(0, arm*.5*math.sin(ph+math.pi+.4))
        er = max(0, arm*.5*math.sin(ph+.4))
        tw = math.sin(ph)*.05*(1+speed*.3)
        frames.append(({
            "J_Bip_L_UpperLeg": (ll, 0, 0),   "J_Bip_R_UpperLeg": (lr, 0, 0),
            "J_Bip_L_LowerLeg": (-kl, 0, 0),  "J_Bip_R_LowerLeg": (-kr, 0, 0),
            "J_Bip_L_UpperArm": (al, 0, -1.2),"J_Bip_R_UpperArm": (ar, 0, 1.2),
            "J_Bip_L_LowerArm": (el, 0, 0),   "J_Bip_R_LowerArm": (er, 0, 0),
            "J_Bip_C_Hips":     (lean*.4, 0, tw*.6),
            "J_Bip_C_Spine":    (lean*.6, 0, tw),
            "J_Bip_C_Chest":    (lean*.4, 0, tw*.5),
            "J_Bip_C_Neck":     (-lean*.3, 0, 0),
        }, 1.0/(8*speed)))
    return frames


# グローバルプリセット辞書（add_clip でユーザーが追加可能）
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
}


# ── アニメーター（内部クラス）────────────────────────────────

class _Animator:
    def __init__(self, vrm_id: int):
        self.vrm_id      = vrm_id
        self._clip       = ""
        self._frames:  list = []
        self._fidx       = 0
        self._t          = 0.0
        self._loop       = False
        self._playing    = False
        self._from:  dict = {}
        self.current_rots: dict = {}   # SpringBone が読む
        self._on_finish  = None
        # PRESETS のコピー（アバターごとに独立した辞書）
        self._clips: dict = dict(PRESETS)

    @property
    def clip(self) -> str:   return self._clip
    @property
    def playing(self) -> bool: return self._playing

    def play(self, name: str, loop: bool, on_finish=None):
        if name not in self._clips:
            print(f"[VrmAvatar] unknown clip '{name}'. "
                  f"available: {sorted(self._clips)}")
            return
        if self._clip == name and self._loop and self._playing:
            return  # 同じループクリップが既に再生中
        self._clip      = name
        self._frames    = self._clips[name]
        self._fidx      = 0
        self._t         = 0.0
        self._loop      = loop
        self._playing   = True
        self._from      = dict(self.current_rots)
        self._on_finish = on_finish

    def register(self, name: str, frames: list):
        self._clips[name] = frames

    def update(self, dt: float):
        import kagra
        if not self._playing or not self._frames: return
        frame = self._frames[self._fidx]
        bones = frame[0]
        dur   = frame[1]
        # Root 位置オフセット（BVH/FBX の3要素目）を毎フレーム適用
        # (0,0,0) のときも明示的にリセットする（前フレームの値が残らないよう）
        if len(frame) > 2:
            rx, ry, rz = frame[2]
            kagra._engine.set_vrm_offset(self.vrm_id, float(rx), float(ry), float(rz))
        self._t = min(1.0, self._t + dt / max(0.01, dur))
        te = self._t * self._t * (3 - 2*self._t)   # smoothstep

        if not bones:
            # バインドポーズへ補間
            for n, qf in self._from.items():
                qn = _slerp(qf, _ID, te)
                self.current_rots[n] = qn
                _send_bone_rot(self.vrm_id, n, qn)
            if self._t >= 1.0:
                _reset_pose(self.vrm_id)
                self.current_rots.clear()
        else:
            for n, rot in bones.items():
                # rot が len==7 → (tx,ty,tz, qx,qy,qz,qw) 並進+回転（BVH Hips）
                # rot が len==4 → クォータニオン（BVH 通常ボーン）
                # rot が len==3 → オイラー角・ラジアン（プリセット）
                if len(rot) == 7:
                    _send_bone_trans(self.vrm_id, n, list(rot[:3]))
                    qt = list(rot[3:7])
                elif len(rot) == 4:
                    qt = list(rot)
                else:
                    qt = _euler_to_quat(*rot)
                qn = _slerp(self._from.get(n, _ID), qt, te)
                self.current_rots[n] = qn
                _send_bone_rot(self.vrm_id, n, qn)

        if self._t >= 1.0:
            self._fidx += 1
            if self._fidx >= len(self._frames):
                if self._loop:
                    self._fidx = 0
                    self._from = dict(self.current_rots)
                    self._t    = 0.0
                else:
                    self._playing = False
                    if self._on_finish:
                        cb, self._on_finish = self._on_finish, None
                        cb()
            else:
                self._from = dict(self.current_rots)
                self._t    = 0.0


# ── VrmAvatar（公開クラス）───────────────────────────────────

class VrmAvatar:
    """VRM キャラクターの統合管理クラス。

    Example::
        avatar = kagra.avatar("assets/Emma.vrm")
        avatar.load_motion("dance", "assets/dance.bvh")

        # ゲームループ
        avatar.play("dance")
        avatar.update(dt)                        # 全部まとめて更新
        kagra.draw_vrm(avatar.vrm_id)

        avatar.set_expression("Fcl_ALL_Joy", 0.8)
        avatar.reset_expressions()
    """

    def __init__(self, vrm_path: str):
        import kagra
        self.vrm_path = vrm_path
        self.vrm_id   = kagra.load_vrm(vrm_path)
        self._anim    = _Animator(self.vrm_id)
        self._spring  = None

        # まばたき状態
        self._blink_t     = 0.0
        self._blink_next  = random.uniform(2.5, 5.0)
        self._blink_phase = 0.0
        self._blink_l: Optional[str] = None
        self._blink_r: Optional[str] = None

        # 表情シェイプ
        self._expr_shapes: list[str] = []

        # SpringBone
        try:
            from kagra.vrm_spring import SpringBone
            self._spring = SpringBone(vrm_path, self.vrm_id)
            print(f"[VrmAvatar] SpringBone: {len(self._spring.chains)} chains")
        except Exception as e:
            print(f"[VrmAvatar] SpringBone skipped: {e}")

        # ブレンドシェイプ名を探索
        try:
            shapes = set(kagra.list_blend_shapes(self.vrm_id))
            EXPR = {"Joy","Angry","Sorrow","Fun","Surprised","Neutral",
                    "Fcl_ALL_Joy","Fcl_ALL_Angry","Fcl_ALL_Sorrow",
                    "Fcl_ALL_Fun","Fcl_ALL_Surprised","Fcl_ALL_Neutral"}
            self._expr_shapes = [s for s in shapes if s in EXPR]
            self._blink_l = next(
                (s for s in ["Blink_L","Fcl_EYE_Blink_L","Blink","Fcl_EYE_Blink"]
                 if s in shapes), None)
            self._blink_r = next(
                (s for s in ["Blink_R","Fcl_EYE_Blink_R"]
                 if s in shapes), self._blink_l)
        except Exception:
            pass

        print(f"[VrmAvatar] Loaded: {vrm_path}")

    # ── アニメーション ─────────────────────────────────────────

    def play(self, clip: str, loop: bool = True, on_finish=None):
        """クリップを再生する。

        組み込み: idle / walk / run / sneak / bind /
                  bow / arm_up / kiss / kiss_both / wave

        BVH は load_motion() で登録してから同じように play() で再生できる。

        Args:
            clip:      クリップ名
            loop:      ループ再生するか（デフォルト True）
            on_finish: 非ループ完了時コールバック
        """
        prev_clip = self._anim._clip
        self._anim.play(clip, loop=loop, on_finish=on_finish)
        # クリップが切り替わったとき
        if prev_clip != clip:
            # スプリングボーンをリセット（着物暴走防止）
            if self._spring:
                self._spring._init_joints()
            # BVH でないクリップに戻るときは root_offset をリセット
            if not self._is_bvh_clip():
                import kagra
                kagra._engine.set_vrm_offset(self.vrm_id, 0.0, 0.0, 0.0)

    def add_clip(self, name: str, frames: list):
        """カスタムクリップを登録する（低レベル API）。

        Args:
            name:   クリップ名
            frames: [(bones_dict, duration_sec), ...]
                    bones_dict の値は (rx,ry,rz) ラジアン or [qx,qy,qz,qw]

        Example::
            avatar.add_clip("nod", [
                ({"J_Bip_C_Neck": (0.3, 0, 0)}, 0.2),
                ({}, 0.2),
            ])
            avatar.play("nod", loop=False)
        """
        self._anim.register(name, frames)

    def add_motion(self, name: str, motion):
        """BvhMotion をクリップとして登録する。

        Args:
            name:   クリップ名
            motion: kagra.load_bvh() が返した BvhMotion
        """
        clip = motion.to_clip()
        self._anim.register(name, clip)
        print(f"[VrmAvatar] '{name}': {len(clip)} frames @ {motion.fps:.1f}fps")

    def load_motion(self, name: str, path: str, extra_map: dict = None):
        """BVH / FBX ファイルを読み込んでクリップとして登録する（1行 API）。

        拡張子で自動判別：
          .bvh → BVH プレイヤー
          .fbx → FBX プレイヤー（ufbx 経由、Blender 変換不要）

        Args:
            name:      クリップ名
            path:      BVH または FBX ファイルのパス
            extra_map: BVH の追加ボーン名マッピング（省略可）

        Example::
            avatar.load_motion("dance", "assets/hiphop.bvh")  # BVH
            avatar.load_motion("dance", "assets/hiphop.fbx")  # FBX
            avatar.play("dance")
        """
        if path.lower().endswith('.fbx'):
            from kagra.fbx_player import load_fbx
            motion = load_fbx(path)
        else:
            from kagra.bvh_player import load_bvh
            motion = load_bvh(path, extra_map=extra_map)
        self.add_motion(name, motion)

    @property
    def clip(self) -> str:
        """現在再生中のクリップ名。"""
        return self._anim.clip

    @property
    def playing(self) -> bool:
        """再生中かどうか。"""
        return self._anim.playing

    @property
    def clips(self) -> list[str]:
        """利用可能なクリップ名の一覧。"""
        return sorted(self._anim._clips.keys())

    @property
    def expressions(self) -> list[str]:
        """このモデルで使える表情名の一覧。"""
        return list(self._expr_shapes)

    # ── 更新（毎フレーム呼ぶ） ────────────────────────────────

    def update(self, dt: float):
        """毎フレーム呼ぶ。以下を順番に更新する：
        1. アニメーション
        2. スプリングボーン（BVH再生中はスキップ）
        3. 自動まばたき
        """
        dt = min(dt, 0.05)
        self._anim.update(dt)
        # BVH クリップ（J_Bip_* ボーンが64本以上ある）再生中は
        # スプリングボーンが干渉するのでスキップ
        if self._spring and not self._is_bvh_clip():
            self._spring.update(dt, self._anim.current_rots)
        self._update_blink(dt)

    def _is_bvh_clip(self) -> bool:
        """現在のクリップが BVH（モーションキャプチャ）かどうか判定する。"""
        if not self._anim.playing or not self._anim._frames:
            return False
        # フレームのボーン数が多い → BVH
        try:
            return len(self._anim._frames[0][0]) > 20
        except Exception:
            return False

    def _update_blink(self, dt: float):
        self._blink_t += dt
        if self._blink_phase == 0.0 and self._blink_t >= self._blink_next:
            self._blink_phase = 0.001
            self._blink_t     = 0.0
            self._blink_next  = random.uniform(2.5, 5.5)

        if self._blink_phase > 0.0:
            if self._blink_phase < 1.0:
                self._blink_phase = min(1.0, self._blink_phase + dt / 0.08)
            else:
                self._blink_phase = max(0.0, self._blink_phase - dt / 0.12)
            # 開閉の山型ウェイト
            w = self._blink_phase * 2 if self._blink_phase <= 0.5 \
                else (1.0 - self._blink_phase) * 2
            w = min(1.0, w)
            for bs in filter(None, [self._blink_l, self._blink_r]):
                _set_shape(self.vrm_id, bs, w)

    # ── 表情 ─────────────────────────────────────────────────

    def set_expression(self, name: str, weight: float = 1.0):
        """表情を設定する（0.0〜1.0）。

        Example::
            avatar.set_expression("Fcl_ALL_Joy", 0.8)   # 笑顔
            avatar.set_expression("Fcl_ALL_Joy", 0.0)   # 消す
        """
        _set_shape(self.vrm_id, name, float(weight))

    def reset_expressions(self):
        """全ブレンドシェイプをゼロにリセット。"""
        import kagra
        kagra.reset_blend_shapes(self.vrm_id)

    # ── スプリングボーン ──────────────────────────────────────

    def set_wind(self, strength: float = 0.0,
                 direction: tuple = (1., 0., 0.)):
        """スプリングボーンに風を設定する。

        Args:
            strength:  風の強さ（0.0=無風, 1.0=強風）
            direction: 風向き（正規化される）
        """
        if self._spring:
            self._spring.set_wind(strength, direction)

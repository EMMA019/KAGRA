# kagra/vrm_facetrack.py
"""
VRM 顔トラッキングシステム (MediaPipe Face Mesh)

カメラ映像からリアルタイムに顔のランドマーク（468点）を取得し、
VRM キャラクターのブレンドシェイプと頭部回転にマッピングする。

依存: mediapipe, opencv-python
    pip install kagra[facetrack]
    または pip install mediapipe opencv-python

Example::
    avatar = kagra.avatar("assets/Emma.vrm")
    ft = avatar.enable_facetracking(camera_id=0)
    ft.enable()

    def update(dt):
        ft.update(dt)          # カメラ→VRM 反映
        kagra.draw_vrm(avatar.vrm_id)

    def draw():
        ft.draw_debug()        # デバッグ描画（カメラ映像＋ランドマーク）
"""
from __future__ import annotations
import math
import logging
from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.vrm_avatar import VrmAvatar

log = logging.getLogger("kagra.vrm_facetrack")

# ── MediaPipe / OpenCV のオプションインポート ──────────────────
try:
    import cv2
    HAS_CV2 = True
except ImportError:
    HAS_CV2 = False

try:
    import mediapipe as mp
    # MediaPipe 0.10.x 用: python.solutions を直接インポートして AttributeError 回避
    import mediapipe.python.solutions as mp_solutions
    mp.solutions = mp_solutions
    HAS_MEDIAPIPE = True
except ImportError:
    HAS_MEDIAPIPE = False

# ── 1€ フィルタ（低遅延スムージング） ──────────────────────────

class _OneEuroFilter:
    """1€ フィルタ: カットオフ周波数を速度に応じて適応的に変化させる。"""
    def __init__(self, min_cutoff: float = 1.0, beta: float = 0.5, d_cutoff: float = 1.0):
        self.min_cutoff = min_cutoff
        self.beta       = beta
        self.d_cutoff   = d_cutoff
        self._x_prev    = 0.0
        self._dx_prev   = 0.0
        self._t_prev    = -1.0

    def reset(self, value: float = 0.0):
        self._x_prev  = value
        self._dx_prev = 0.0
        self._t_prev  = -1.0

    def update(self, x: float, t: float) -> float:
        if self._t_prev < 0:
            self.reset(x)
            return x

        dt = t - self._t_prev
        if dt <= 0:
            return self._x_prev

        # 微分信号の平滑化
        dx   = (x - self._x_prev) / dt
        edx  = self._exponential_smoothing(dx, self._dx_prev, self._alpha_for(self.d_cutoff, dt))
        self._dx_prev = edx

        # 速度に応じてカットオフを適応
        cutoff = self.min_cutoff + self.beta * abs(edx)
        alpha  = self._alpha_for(cutoff, dt)

        x_hat  = self._exponential_smoothing(x, self._x_prev, alpha)
        self._x_prev  = x_hat
        self._t_prev  = t
        return x_hat

    @staticmethod
    def _alpha_for(cutoff: float, dt: float) -> float:
        tau = 1.0 / (2 * math.pi * cutoff) if cutoff > 0 else 0
        return 1.0 / (1.0 + tau / dt) if tau > 0 else 1.0

    @staticmethod
    def _exponential_smoothing(x: float, x_prev: float, alpha: float) -> float:
        return alpha * x + (1 - alpha) * x_prev


class _FilterSet:
    """複数の 1€ フィルタを一括管理。"""
    def __init__(self, n: int, min_cutoff: float = 1.0, beta: float = 0.5):
        self.filters = [_OneEuroFilter(min_cutoff, beta) for _ in range(n)]
        self._t = 0.0

    def reset(self):
        for f in self.filters:
            f.reset()

    def update(self, values: list[float], dt: float) -> list[float]:
        self._t += dt
        return [f.update(v, self._t) for f, v in zip(self.filters, values)]


# ══════════════════════════════════════════════════════════════════
#  顔トラッキングコントローラー
# ══════════════════════════════════════════════════════════════════

class FaceTracker:
    """MediaPipe Face Mesh による顔トラッキングコントローラー。"""

    _LEFT_EYE_TOP     = 159
    _LEFT_EYE_BOTTOM  = 145
    _RIGHT_EYE_TOP    = 386
    _RIGHT_EYE_BOTTOM = 374

    _LEFT_EYE_INNER   = 133
    _LEFT_EYE_OUTER   = 33
    _RIGHT_EYE_INNER  = 362
    _RIGHT_EYE_OUTER  = 263

    _UPPER_LIP_TOP    = 13
    _LOWER_LIP_BOTTOM = 14
    _LEFT_MOUTH       = 61
    _RIGHT_MOUTH      = 291
    _UPPER_LIP_BOTTOM = 12
    _LOWER_LIP_TOP    = 17

    _LEFT_EYEBROW     = 70
    _RIGHT_EYEBROW    = 300
    _NOSE_TIP         = 1
    _NOSE_BRIDGE      = 6
    _CHIN             = 152

    def __init__(
        self,
        avatar: "VrmAvatar",
        camera_id: int = 0,
        use_iris: bool = False,
        max_faces: int = 1,
        min_detection_confidence: float = 0.7,
        smooth_speed: float = 6.0,
        blink_smooth: float = 0.3,
    ):
        self._avatar    = avatar
        self.camera_id  = camera_id
        self.use_iris   = use_iris
        self.enabled    = False

        self.smooth_speed  = smooth_speed
        self.blink_smooth  = blink_smooth

        if not HAS_MEDIAPIPE:
            raise ImportError("mediapipe がインストールされていません。")
        if not HAS_CV2:
            raise ImportError("opencv-python がインストールされていません。")

        self._mp_face_mesh = mp.solutions.face_mesh.FaceMesh(
            max_num_faces          = max_faces,
            refine_landmarks       = use_iris,
            min_detection_confidence = min_detection_confidence,
            min_tracking_confidence  = 0.6,
        )

        self._cap: Optional[cv2.VideoCapture] = None
        self._frame_w: int = 640
        self._frame_h: int = 480
        self._last_frame = None

        self._filter_head = _FilterSet(3, min_cutoff=2.0, beta=1.0)
        self._filter_mouth = _FilterSet(2, min_cutoff=1.0, beta=0.8)
        self._filter_brow = _FilterSet(2, min_cutoff=1.0, beta=0.6)

        self.head_rx: float = 0.0
        self.head_ry: float = 0.0
        self.head_rz: float = 0.0
        self.mouth_open: float = 0.0
        self.mouth_wide: float = 0.0
        self.eye_blink_l: float = 0.0
        self.eye_blink_r: float = 0.0
        self.brow_l: float = 0.0
        self.brow_r: float = 0.0

        print(f"[FaceTracker] 初期化完了 (camera_id={camera_id}, use_iris={use_iris})")

    def _open_camera(self) -> bool:
        if self._cap is not None:
            return True
        self._cap = cv2.VideoCapture(self.camera_id)
        if not self._cap.isOpened():
            self._cap = None
            print(f"[FaceTracker] カメラ {self.camera_id} を開けませんでした")
            return False
        self._cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
        self._cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
        self._frame_w = int(self._cap.get(cv2.CAP_PROP_FRAME_WIDTH))
        self._frame_h = int(self._cap.get(cv2.CAP_PROP_FRAME_HEIGHT))
        print(f"[FaceTracker] カメラ起動: {self._frame_w}x{self._frame_h}")
        return True

    def _close_camera(self):
        if self._cap is not None:
            self._cap.release()
            self._cap = None
        self._last_frame = None

    def enable(self):
        if self.enabled: return
        if self._open_camera():
            self.enabled = True
            self._filter_head.reset()
            self._filter_mouth.reset()
            self._filter_brow.reset()
            print("[FaceTracker] トラッキング開始")

    def disable(self):
        self.enabled = False
        self._close_camera()
        print("[FaceTracker] トラッキング停止")

    def _process_landmarks(self, landmarks) -> dict:
        lm = landmarks.landmark
        h, w = self._frame_h, self._frame_w

        # ════════════════════════════════════════════════════════════
        # ⚙️ キャリブレーション設定（ユーザーログ解析に基づく最適化値）
        # ════════════════════════════════════════════════════════════
        EYE_OPEN  = 0.38  # 目の比率（これ以上なら目がパッチリ開く）
        EYE_CLOSE = 0.28  # 目の比率（これ以下なら完全に目を閉じる）

        MOUTH_OPEN  = 0.35 # 口の比率（縦/横）上限（全開）
        MOUTH_CLOSE = 0.05 # 口の比率（縦/横）下限（完全に閉じる）
        # ════════════════════════════════════════════════════════════

        # ── 目の開き計算 ──
        eye_l_ratio = self._eye_openness(lm, self._LEFT_EYE_TOP, self._LEFT_EYE_BOTTOM,
                                        self._LEFT_EYE_INNER, self._LEFT_EYE_OUTER)
        eye_r_ratio = self._eye_openness(lm, self._RIGHT_EYE_TOP, self._RIGHT_EYE_BOTTOM,
                                        self._RIGHT_EYE_INNER, self._RIGHT_EYE_OUTER)
        
        # 開き度（0=開, 1=閉）に変換。比率が小さいほど閉じる。
        eye_l = 1.0 - max(0.0, min(1.0, (eye_l_ratio - EYE_CLOSE) / (EYE_OPEN - EYE_CLOSE)))
        eye_r = 1.0 - max(0.0, min(1.0, (eye_r_ratio - EYE_CLOSE) / (EYE_OPEN - EYE_CLOSE)))

        # ── 口の開き計算（距離ではなく比率で計算して距離依存バグを修正） ──
        upper_lip = lm[self._UPPER_LIP_TOP]
        lower_lip = lm[self._LOWER_LIP_BOTTOM]
        left_mouth  = lm[self._LEFT_MOUTH]
        right_mouth = lm[self._RIGHT_MOUTH]

        mouth_h = abs(upper_lip.y - lower_lip.y)
        mouth_w = abs(left_mouth.x - right_mouth.x)
        m_ratio = mouth_h / mouth_w if mouth_w > 1e-6 else 0.0

        mouth_open = max(0.0, min(1.0, (m_ratio - MOUTH_CLOSE) / (MOUTH_OPEN - MOUTH_CLOSE)))

        # 💡 デバッグ用プリント (調整が終わったら先頭に # をつけてコメントアウトしてください)
        print(f"左目比率: {eye_l_ratio:.3f} | 右目比率: {eye_r_ratio:.3f} | 口比率: {m_ratio:.3f}")

        # 口の広がり（口角間距離）
        mouth_w_px = mouth_w * w
        eye_dist_px = abs(lm[self._LEFT_EYE_OUTER].x - lm[self._RIGHT_EYE_OUTER].x) * w
        mouth_wide = min(1.0, mouth_w_px / (eye_dist_px * 0.8)) if eye_dist_px > 0 else 0.0

        # ── 眉の高さ ──
        brow_l = (lm[self._LEFT_EYEBROW].y - lm[self._NOSE_BRIDGE].y) * h
        brow_r = (lm[self._RIGHT_EYEBROW].y - lm[self._NOSE_BRIDGE].y) * h
        brow_l_norm = max(-1.0, min(1.0, brow_l / 20.0))
        brow_r_norm = max(-1.0, min(1.0, brow_r / 20.0))

        # ── 頭部回転推定 ──
        nose_tip   = lm[self._NOSE_TIP]
        nose_bridge = lm[self._NOSE_BRIDGE]
        chin       = lm[self._CHIN]
        left_eye   = lm[self._LEFT_EYE_OUTER]
        right_eye  = lm[self._RIGHT_EYE_OUTER]

        nose_dx = (nose_tip.x - nose_bridge.x) * w
        head_ry = max(-1.0, min(1.0, nose_dx / (eye_dist_px * 0.5))) if eye_dist_px > 0 else 0.0

        face_h = abs(chin.y - nose_bridge.y) * h
        nose_chin_dy = (nose_tip.y - nose_bridge.y) * h
        head_rx = max(-1.0, min(1.0, (nose_chin_dy / face_h - 0.5) * 4.0)) if face_h > 0 else 0.0

        eye_dy = left_eye.y - right_eye.y
        head_rz = max(-1.0, min(1.0, -eye_dy * 4.0))

        return {
            "head_rx":    head_rx,
            "head_ry":    head_ry,
            "head_rz":    head_rz,
            "eye_l":      eye_l,
            "eye_r":      eye_r,
            "mouth_open": mouth_open,
            "mouth_wide": mouth_wide,
            "brow_l":     brow_l_norm,
            "brow_r":     brow_r_norm,
        }

    @staticmethod
    def _eye_openness(lm, top_idx, bottom_idx, inner_idx, outer_idx) -> float:
        top    = lm[top_idx]
        bottom = lm[bottom_idx]
        inner  = lm[inner_idx]
        outer  = lm[outer_idx]
        eye_h  = abs(top.y - bottom.y)
        eye_w  = abs(inner.x - outer.x)
        if eye_w < 1e-6: return 0.0
        return eye_h / eye_w

    def _apply_to_vrm(self, data: dict, dt: float):
        avatar = self._avatar
        vrm_id = avatar.vrm_id

        # ── 頭部回転 ──
        head_rx = data["head_rx"] * math.radians(25)
        head_ry = data["head_ry"] * math.radians(30)
        head_rz = data["head_rz"] * math.radians(15)

        neck_rx = data["head_rx"] * math.radians(15)
        head_rx_extra = data["head_rx"] * math.radians(10)

        avatar._send_bone("J_Bip_C_Neck", self._euler_to_quat(head_ry, -head_rx, head_rz))
        avatar._send_bone("J_Bip_C_Head", self._euler_to_quat(0.0, -head_rx_extra, 0.0))

        # ── まばたき ──
        import kagra
        if avatar._blink_l:
            kagra.get_engine().set_blend_shape(vrm_id, avatar._blink_l, data["eye_l"])
        if avatar._blink_r:
            kagra.get_engine().set_blend_shape(vrm_id, avatar._blink_r, data["eye_r"])

        # ── 口の開き ──
        mo = data["mouth_open"]
        mw = data["mouth_wide"]

        aa_w = max(0.0, mo * (1.0 - mw * 0.5))
        oh_w = max(0.0, mo * mw * 0.5)
        ih_w = max(0.0, (1.0 - mo) * mw * 0.7)
        ee_w = max(0.0, mw * (1.0 - mo) * 0.5)
        ou_w = max(0.0, (1.0 - mw) * (1.0 - mo) * 0.3)

        shapes = { "aa": aa_w, "oh": oh_w, "ih": ih_w, "ee": ee_w, "ou": ou_w }
        for vowel, weight in shapes.items():
            if weight < 0.02: continue
            for candidate in self._vowel_candidates(vowel):
                try: kagra.get_engine().set_blend_shape(vrm_id, candidate, weight)
                except: pass

        # ── 眉 → 感情 ──
        brow_avg = (data["brow_l"] + data["brow_r"]) / 2.0
        if brow_avg > 0.3:
            for s in ["surprised", "Surprised", "Fcl_ALL_Surprised"]:
                try: kagra.get_engine().set_blend_shape(vrm_id, s, min(1.0, brow_avg * 1.5))
                except: pass
        elif brow_avg < -0.2:
            for s in ["angry", "Angry", "Fcl_ALL_Angry"]:
                try: kagra.get_engine().set_blend_shape(vrm_id, s, min(1.0, -brow_avg * 2.0))
                except: pass

        # ── 口角 → joy ──
        if mw > 0.6 and mo < 0.3:
            for s in ["happy", "Joy", "Fcl_ALL_Joy", "joy"]:
                try: kagra.get_engine().set_blend_shape(vrm_id, s, min(1.0, mw * 1.2))
                except: pass

    @staticmethod
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

    @staticmethod
    def _vowel_candidates(vowel: str) -> list[str]:
        table = {
            "aa": ["aa", "Fcl_MTH_A", "A", "MTH_A"],
            "ih": ["ih", "Fcl_MTH_I", "I", "MTH_I"],
            "ou": ["ou", "Fcl_MTH_U", "U", "MTH_U"],
            "ee": ["ee", "Fcl_MTH_E", "E", "MTH_E"],
            "oh": ["oh", "Fcl_MTH_O", "O", "MTH_O"],
        }
        return table.get(vowel, [vowel])

    def update(self, dt: float):
        if not self.enabled or self._cap is None: return
        dt = min(dt, 0.05)

        ret, frame = self._cap.read()
        if not ret:
            print("[FaceTracker] カメラ読み取り失敗")
            self.disable()
            return

        frame_rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
        frame_rgb = cv2.flip(frame_rgb, 1)
        self._last_frame = frame_rgb.copy()

        results = self._mp_face_mesh.process(frame_rgb)
        if not results.multi_face_landmarks: return

        landmarks = results.multi_face_landmarks[0]
        raw = self._process_landmarks(landmarks)

        self.head_rx, self.head_ry, self.head_rz = self._filter_head.update(
            [raw["head_rx"], raw["head_ry"], raw["head_rz"]], dt)
        self.mouth_open, self.mouth_wide = self._filter_mouth.update(
            [raw["mouth_open"], raw["mouth_wide"]], dt)
        self.brow_l, self.brow_r = self._filter_brow.update(
            [raw["brow_l"], raw["brow_r"]], dt)
        self.eye_blink_l = raw["eye_l"]
        self.eye_blink_r = raw["eye_r"]

        self._apply_to_vrm({
            "head_rx":    self.head_rx,
            "head_ry":    self.head_ry,
            "head_rz":    self.head_rz,
            "eye_l":      self.eye_blink_l,
            "eye_r":      self.eye_blink_r,
            "mouth_open": self.mouth_open,
            "mouth_wide": self.mouth_wide,
            "brow_l":     self.brow_l,
            "brow_r":     self.brow_r,
        }, dt)

    def draw_debug(self, x: int = 0, y: int = 0, scale: float = 0.3):
        if self._last_frame is None: return
        import kagra
        info = [
            f"Head: RX={self.head_rx:.2f} RY={self.head_ry:.2f} RZ={self.head_rz:.2f}",
            f"Eye: L={self.eye_blink_l:.2f} R={self.eye_blink_r:.2f}",
            f"Mouth: open={self.mouth_open:.2f} wide={self.mouth_wide:.2f}",
        ]
        h, w = self._last_frame.shape[:2]
        dw, dh = int(w * scale), int(h * scale)
        kagra.fill(x, y, dw, dh, (20, 20, 30), alpha=180)
        kagra.text(f"[FaceTracker]", x + 4, y + 2, 12, (0, 255, 0))
        kagra.text(info[0], x + 4, y + 18, 11, (200, 200, 200))
        kagra.text(info[1], x + 4, y + 32, 11, (200, 200, 200))
        kagra.text(info[2], x + 4, y + 46, 11, (200, 200, 200))


if __name__ == "__main__":
    print("=" * 60)
    print("FaceTracker 単体テスト")
    print("=" * 60)

    if not HAS_MEDIAPIPE or not HAS_CV2:
        print("依存ライブラリが不足しています: pip install mediapipe opencv-python")
        exit(1)

    cap = cv2.VideoCapture(1)
    if not cap.isOpened():
        print("カメラを開けませんでした")
        exit(1)

    mp_face = mp.solutions.face_mesh.FaceMesh(
        max_num_faces=1, refine_landmarks=False, min_detection_confidence=0.7,
    )

    print("カメラ起動中... ESC で終了")
    while True:
        ret, frame = cap.read()
        if not ret: break
        frame = cv2.flip(frame, 1)
        rgb   = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
        results = mp_face.process(rgb)

        if results.multi_face_landmarks:
            lm = results.multi_face_landmarks[0].landmark
            xs = [p.x for p in lm]; ys = [p.y for p in lm]
            h, w = frame.shape[:2]
            cv2.rectangle(frame, (int(min(xs)*w), int(min(ys)*h)), 
                                 (int(max(xs)*w), int(max(ys)*h)), (0, 255, 0), 2)
            for p in lm:
                cv2.circle(frame, (int(p.x*w), int(p.y*h)), 1, (0, 255, 0), -1)

        cv2.imshow("FaceTracker Test", frame)
        if cv2.waitKey(1) == 27: break

    cap.release()
    cv2.destroyAllWindows()
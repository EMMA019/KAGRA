# kagra/vrm_emotion.py
"""
VRM 表情スムーズブレンドシステム (Phase 7 - VRM 1.0/0.x dual support)

VRM 1.0 と VRM 0.x で表情ブレンドシェイプの命名規則が違うため、
両方の候補を持って自動で存在するものを使う。

VRM 1.0 expression presets:
    happy / angry / sad / relaxed / surprised / aa / ih / ou / ee / oh / blink / etc.

VRM 0.x blend shape names:
    Fcl_ALL_Joy / Fcl_ALL_Angry / Fcl_ALL_Sorrow / Fcl_ALL_Fun / Fcl_ALL_Surprised
    Joy / Angry / Sorrow / Fun / Surprised
"""
from __future__ import annotations
import math
from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.vrm_avatar import VrmAvatar


# ══════════════════════════════════════════════════════════════
#  感情 → ブレンドシェイプ候補リスト
# ══════════════════════════════════════════════════════════════

_EMOTION_CANDIDATES: dict[str, list[str]] = {
    "neutral":   ["neutral",   "Neutral",   "Fcl_ALL_Neutral"],
    "joy":       ["happy",     "Joy",       "Fcl_ALL_Joy",       "joy"],
    "happy":     ["happy",     "Joy",       "Fcl_ALL_Joy",       "joy"],
    "angry":     ["angry",     "Angry",     "Fcl_ALL_Angry"],
    "sorrow":    ["sad",       "sorrow",    "Sorrow",   "Fcl_ALL_Sorrow"],
    "sad":       ["sad",       "sorrow",    "Sorrow",   "Fcl_ALL_Sorrow"],
    "fun":       ["relaxed",   "fun",       "Fun",      "Fcl_ALL_Fun"],
    "surprised": ["surprised", "Surprised", "Fcl_ALL_Surprised"],
    "relaxed":   ["relaxed",   "Fun",       "Fcl_ALL_Fun"],
}

_COMPOSITE_EMOTIONS: dict[str, dict[str, float]] = {
    "shy":         {"joy": 0.4, "fun": 0.3},
    "embarrassed": {"sorrow": 0.3, "fun": 0.4},
    "excited":     {"joy": 0.7, "surprised": 0.5},
    "loving":      {"joy": 0.6, "fun": 0.5},
    "confused":    {"sorrow": 0.3, "angry": 0.2},
}

_KEYWORD_EMOTION = {
    "嬉しい":"joy","よかった":"joy","ありがとう":"joy","はい":"joy","そうです":"joy",
    "楽しい":"fun","おもしろ":"fun","笑":"fun","ふふ":"fun",
    "悲しい":"sorrow","つらい":"sorrow","泣":"sorrow","すみません":"sorrow","ごめん":"sorrow",
    "怒":"angry","むかつ":"angry","ひどい":"angry","腹立":"angry",
    "驚":"surprised","えっ":"surprised","まじ":"surprised","うそ":"surprised",
    "恥ずかしい":"shy","照れ":"shy",
    "好き":"loving","大好き":"loving","愛":"loving",
    "happy":"joy","great":"joy","thanks":"joy",
    "fun":"fun","haha":"fun","lol":"fun",
    "sad":"sorrow","sorry":"sorrow",
    "angry":"angry","mad":"angry",
    "wow":"surprised","omg":"surprised",
    "love":"loving","like":"loving",
}


class EmotionController:
    """VRM 表情スムーズブレンドコントローラー（VRM 1.0/0.x両対応）。

    Args:
        avatar:      VrmAvatar インスタンス
        blend_speed: 遷移速度（大きいほど速い）
        debug:       True で起動時に診断情報を出力
    """

    def __init__(
        self,
        avatar: "VrmAvatar",
        blend_speed: float = 2.5,
        debug:       bool  = True,
    ):
        self._avatar     = avatar
        self.blend_speed = blend_speed
        self.enabled     = True

        import kagra
        available = set(kagra.list_blend_shapes(avatar.vrm_id))
        self._available = available

        # 感情ごとに、このモデルで実際に使える名前を解決
        self._resolved: dict[str, Optional[str]] = {}
        for emo, cands in _EMOTION_CANDIDATES.items():
            found = next((c for c in cands if c in available), None)
            self._resolved[emo] = found

        if debug:
            self._print_diagnostic()

        self._targets:     dict[str, float] = {}
        self._cur_weights: dict[str, float] = {}
        self._current_emotion: str = "neutral"

    def _print_diagnostic(self):
        print("━" * 60)
        print(f"[Emotion] モデル診断 ({len(self._available)} 個のシェイプ検出)")
        print("━" * 60)

        resolved_count = sum(1 for v in self._resolved.values() if v is not None)
        print(f"  感情マッピング: {resolved_count}/{len(_EMOTION_CANDIDATES)} 解決")
        for emo, shape in self._resolved.items():
            mark = "[OK]" if shape else "[--]"
            print(f"   {mark} {emo:12s} -> {shape if shape else '(Not Found)'}")

        mapped   = {s for s in self._resolved.values() if s}
        unmapped = sorted(self._available - mapped - {"__warned__"})
        if unmapped:
            preview = unmapped[:25]
            print(f"  未マッピング ({len(unmapped)} 個):  {preview}")
        print("━" * 60)

    # ══════════════════════════════════════════════════════════════
    #  diagnostics() メソッド（デモ互換のため追加）
    # ══════════════════════════════════════════════════════════════
    def diagnostics(self) -> dict:
        """デバッグ用の診断情報を返す（デモの em.diagnostics() 呼び出しに対応）

        Returns:
            dict: {
                "resolved": 感情→シェイプ名の辞書,
                "available": モデルが持つ全ブレンドシェイプのリスト,
                "num_resolved": 解決された感情の数,
                "num_available": 利用可能なシェイプの総数,
            }
        """
        return {
            "resolved": self._resolved.copy(),
            "available": list(self._available),
            "num_resolved": sum(1 for v in self._resolved.values() if v is not None),
            "num_available": len(self._available),
        }

    def set(self, emotion: str, intensity: float = 1.0, force: bool = False):
        self._current_emotion = emotion

        if emotion in _COMPOSITE_EMOTIONS:
            merged: dict[str, float] = {}
            for sub_emo, sub_w in _COMPOSITE_EMOTIONS[emotion].items():
                shape = self._resolved.get(sub_emo)
                if shape:
                    merged[shape] = max(merged.get(shape, 0.0), sub_w * intensity)
            self._targets = merged
        else:
            shape = self._resolved.get(emotion)
            self._targets = {shape: intensity} if shape else {}
            if not shape and emotion != "neutral":
                print(f"[Emotion] '{emotion}' 対応シェイプなし")

        if force:
            self._apply_immediate()

    def blend(self, emotions: dict[str, float]):
        merged: dict[str, float] = {}
        for emo, strength in emotions.items():
            if emo in _COMPOSITE_EMOTIONS:
                for sub, sub_w in _COMPOSITE_EMOTIONS[emo].items():
                    shape = self._resolved.get(sub)
                    if shape:
                        merged[shape] = max(merged.get(shape, 0.0), sub_w * strength)
            else:
                shape = self._resolved.get(emo)
                if shape:
                    merged[shape] = max(merged.get(shape, 0.0), strength)
        self._targets = merged

    def from_text(self, text: str, intensity: float = 0.8) -> str:
        detected = "neutral"
        text_l = text.lower()
        for kw, em in _KEYWORD_EMOTION.items():
            if kw in text or kw in text_l:
                detected = em
                break
        self.set(detected, intensity)
        return detected

    def reset(self, force: bool = False):
        self.set("neutral", force=force)

    @property
    def current(self) -> str:
        return self._current_emotion

    @property
    def resolved_shapes(self) -> dict[str, Optional[str]]:
        return dict(self._resolved)

    def update(self, dt: float):
        if not self.enabled:
            return

        import kagra
        vid = self._avatar.vrm_id
        spd = self.blend_speed * dt

        all_shapes = set(self._cur_weights) | set(self._targets.keys())

        for shape in all_shapes:
            tgt = self._targets.get(shape, 0.0)
            cur = self._cur_weights.get(shape, 0.0)
            new = cur + (tgt - cur) * min(1.0, spd)
            new = max(0.0, min(1.0, new))
            self._cur_weights[shape] = new

            if new > 0.001 or cur > 0.001:
                kagra.get_engine().set_blend_shape(vid, shape, new)

    def _apply_immediate(self):
        import kagra
        vid = self._avatar.vrm_id
        for shape in list(self._cur_weights.keys()):
            kagra.get_engine().set_blend_shape(vid, shape, 0.0)
        self._cur_weights.clear()
        for shape, w in self._targets.items():
            kagra.get_engine().set_blend_shape(vid, shape, w)
            self._cur_weights[shape] = w
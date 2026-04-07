# kagra/bgm_sync.py
# BGM同期システム
#
# BGMの再生時間に合わせて振り付けキューを発火させる。
# Audio の再生位置取得が不要な設計（Python 側で時間を管理）。
#
# ── 使い方 ──────────────────────────────────────────────────
#
# 1) 振り付けデータを定義（JSON または コード）
#
#   CHOREO = [
#       {"time": 1.0, "cue": "UP",    "pose": "arm_up"},
#       {"time": 2.5, "cue": "RIGHT", "pose": "wave_right"},
#       {"time": 4.0, "cue": "DOWN",  "pose": "crouch"},
#   ]
#
# 2) BgmSync を作成して BGM と同時にスタート
#
#   self.sync = BgmSync(CHOREO, bpm=128)
#   kagra.audio.play_bgm("assets/audio/song.ogg")
#   self.sync.start()
#
# 3) 毎フレーム update() を呼ぶ
#
#   self.sync.update(dt)
#
# 4) キューを受け取る（Event Bus 経由）
#
#   kagra.on("bgm_cue", self._on_cue)
#
#   def _on_cue(self, data):
#       if data["cue"] == "UP":
#           self.animator.play("arm_up")
#           self.waiting_input = "UP"

from __future__ import annotations
import json
from typing import Callable, Optional


class BgmCue:
    """振り付けの1ステップ。"""

    def __init__(
        self,
        time: float,
        cue: str,
        pose: str = "",
        data: dict = None,
    ):
        self.time   = time    # 発火タイミング（秒）
        self.cue    = cue     # キー方向など（"UP" / "DOWN" / "LEFT" / "RIGHT" / "Z" / "X"）
        self.pose   = pose    # 対応するポーズ名
        self.data   = data or {}
        self._fired = False   # 発火済みフラグ


class BgmSync:
    """BGM同期マネージャー。

    BGM の再生時間に合わせて振り付けキューを発火する。
    Python 側で時間を管理するため、Audio API の再生位置取得が不要。

    Example::
        CHOREO = [
            {"time": 1.0,  "cue": "UP",    "pose": "arm_up"},
            {"time": 2.5,  "cue": "RIGHT", "pose": "wave_right"},
            {"time": 4.0,  "cue": "DOWN",  "pose": "crouch"},
            {"time": 5.5,  "cue": "LEFT",  "pose": "wave_left"},
            {"time": 7.0,  "cue": "UP",    "pose": "arm_up"},
        ]

        sync = BgmSync(CHOREO, bpm=128)
        kagra.audio.play_bgm("assets/audio/song.ogg")
        sync.start()

        # update() で毎フレーム
        sync.update(dt)

        # Event Bus でキューを受け取る
        kagra.on("bgm_cue", on_cue)

        def on_cue(data):
            print(data["cue"], data["pose"])
    """

    def __init__(
        self,
        choreography: list,   # list[dict] or list[BgmCue]
        bpm: float = 120.0,
        loop: bool = False,
        length: float = 0.0,  # ループ時の曲の長さ（秒）
    ):
        self.bpm    = bpm
        self.loop   = loop
        self.length = length

        self._time    = 0.0
        self._running = False
        self._cues: list[BgmCue] = []

        # dict → BgmCue に変換
        for item in choreography:
            if isinstance(item, BgmCue):
                self._cues.append(item)
            else:
                self._cues.append(BgmCue(
                    time=item.get("time", 0.0),
                    cue=item.get("cue", ""),
                    pose=item.get("pose", ""),
                    data={k: v for k, v in item.items()
                          if k not in ("time", "cue", "pose")},
                ))

        # 時間順にソート
        self._cues.sort(key=lambda c: c.time)

        # コールバック（Event Bus 以外の選択肢）
        self._callbacks: list[Callable] = []

    # ── 制御 ──────────────────────────────────────────────────

    def start(self, offset: float = 0.0):
        """BGM と同時に呼ぶ。offset で開始位置をずらせる。"""
        self._time    = offset
        self._running = True
        for cue in self._cues:
            cue._fired = cue.time < offset   # offset 前のキューは発火済みにする

    def stop(self):
        self._running = False

    def pause(self):
        self._running = False

    def resume(self):
        self._running = True

    def seek(self, t: float):
        """任意の時刻にジャンプする。"""
        self._time = t
        for cue in self._cues:
            cue._fired = (cue.time < t)

    @property
    def time(self) -> float:
        return self._time

    @property
    def running(self) -> bool:
        return self._running

    # ── BPM ユーティリティ ────────────────────────────────────

    def beat_to_sec(self, beat: float) -> float:
        """拍数 → 秒数。"""
        return beat * 60.0 / self.bpm

    def sec_to_beat(self, sec: float) -> float:
        """秒数 → 拍数。"""
        return sec * self.bpm / 60.0

    def current_beat(self) -> float:
        """現在の拍数を返す。"""
        return self.sec_to_beat(self._time)

    def beat_fraction(self) -> float:
        """現在の拍の小数部分（0.0〜1.0）。タイミング判定や点滅エフェクトに。"""
        return self.current_beat() % 1.0

    # ── 毎フレーム ────────────────────────────────────────────

    def update(self, dt: float):
        """毎フレーム update() の中で呼ぶ。"""
        if not self._running:
            return

        self._time += dt

        # ループ処理
        if self.loop and self.length > 0 and self._time >= self.length:
            self._time -= self.length
            for cue in self._cues:
                cue._fired = False

        # キューの発火チェック
        for cue in self._cues:
            if not cue._fired and self._time >= cue.time:
                cue._fired = True
                self._fire(cue)

    def _fire(self, cue: BgmCue):
        """キューを発火する（Event Bus + コールバック）。"""
        import kagra
        payload = {
            "time":  cue.time,
            "cue":   cue.cue,
            "pose":  cue.pose,
            "beat":  self.sec_to_beat(cue.time),
            **cue.data,
        }
        kagra.emit("bgm_cue", payload)

        for cb in self._callbacks:
            cb(payload)

    # ── コールバック登録 ──────────────────────────────────────

    def on_cue(self, callback: Callable):
        """Event Bus を使わずコールバックで受け取る場合。"""
        self._callbacks.append(callback)

    def off_cue(self, callback: Callable):
        self._callbacks = [c for c in self._callbacks if c is not callback]

    # ── ファイル入出力 ────────────────────────────────────────

    def save(self, path: str):
        """振り付けデータを JSON に保存する。"""
        data = {
            "bpm":    self.bpm,
            "loop":   self.loop,
            "length": self.length,
            "cues": [
                {"time": c.time, "cue": c.cue, "pose": c.pose, **c.data}
                for c in self._cues
            ],
        }
        with open(path, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2, ensure_ascii=False)

    @classmethod
    def load(cls, path: str) -> "BgmSync":
        """JSON から振り付けデータを読み込む。"""
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
        return cls(
            choreography=data.get("cues", []),
            bpm=data.get("bpm", 120.0),
            loop=data.get("loop", False),
            length=data.get("length", 0.0),
        )


# ── リズム判定 ────────────────────────────────────────────────

class RhythmJudge:
    """プレイヤーの入力タイミングを判定するクラス。

    BgmSync と組み合わせて音ゲーのノーツ判定を行う。

    Example::
        judge = RhythmJudge(perfect=0.08, good=0.15, ok=0.25)

        # bgm_cue イベントを受け取ったとき
        def _on_cue(self, data):
            self.judge.register(data["cue"], data["time"], data["pose"])

        # update() で
        self.judge.update(dt)
        if kagra.key_pressed(kagra.KEY_UP):
            result = self.judge.hit("UP", self.sync.time)
            if result:
                print(result["grade"], result["pose"])
    """

    GRADES = ["PERFECT", "GOOD", "OK", "MISS"]

    def __init__(
        self,
        perfect: float = 0.08,   # ±秒
        good:    float = 0.15,
        ok:      float = 0.25,
        miss_after: float = 0.35,  # これ以上過ぎたら自動MISS
    ):
        self.windows = {
            "PERFECT": perfect,
            "GOOD":    good,
            "OK":      ok,
        }
        self.miss_after = miss_after
        self._pending: list[dict] = []   # 未判定のキュー

    def register(self, cue: str, target_time: float, pose: str = ""):
        """判定待ちのキューを登録する（bgm_cue イベント受け取り時に呼ぶ）。"""
        self._pending.append({
            "cue":         cue,
            "target_time": target_time,
            "pose":        pose,
            "missed":      False,
        })

    def update(self, dt: float, current_time: float):
        """毎フレーム呼ぶ。タイムオーバーのキューを MISS 処理。"""
        import kagra
        for p in self._pending:
            if not p["missed"] and current_time - p["target_time"] > self.miss_after:
                p["missed"] = True
                kagra.emit("rhythm_result", {
                    "grade": "MISS",
                    "cue":   p["cue"],
                    "pose":  p["pose"],
                    "diff":  current_time - p["target_time"],
                })
        self._pending = [p for p in self._pending if not p["missed"]]

    def hit(self, cue: str, current_time: float) -> Optional[dict]:
        """キー入力時に呼ぶ。判定結果を返す（なければ None）。

        Returns:
            {"grade": "PERFECT"/"GOOD"/"OK"/"MISS", "cue": str, "pose": str, "diff": float}
            または None（対応するキューがない場合）
        """
        import kagra

        # 最も近い未判定のキューを探す
        best = None
        best_diff = float("inf")
        for p in self._pending:
            if p["missed"]:
                continue
            if p["cue"] != cue:
                continue
            diff = abs(current_time - p["target_time"])
            if diff < best_diff:
                best_diff = diff
                best = p

        if best is None:
            return None

        # グレード判定
        grade = "MISS"
        for g in ["PERFECT", "GOOD", "OK"]:
            if best_diff <= self.windows[g]:
                grade = g
                break

        result = {
            "grade": grade,
            "cue":   best["cue"],
            "pose":  best["pose"],
            "diff":  best_diff,
        }
        best["missed"] = True   # 判定済みにする

        kagra.emit("rhythm_result", result)
        return result

    def clear(self):
        self._pending.clear()


# ── スコアマネージャー ────────────────────────────────────────

class LiveScore:
    """ライブのスコアを管理するクラス。

    Example::
        score = LiveScore()
        kagra.on("rhythm_result", score.on_result)

        score.total      # 合計スコア
        score.combo      # 現在のコンボ数
        score.max_combo  # 最大コンボ数
        score.grade_counts  # {"PERFECT": 10, "GOOD": 5, ...}
        score.rank()     # "S" / "A" / "B" / "C" / "D"
    """

    SCORE_TABLE = {
        "PERFECT": 1000,
        "GOOD":    500,
        "OK":      200,
        "MISS":    0,
    }

    def __init__(self):
        self.reset()

    def reset(self):
        self.total        = 0
        self.combo        = 0
        self.max_combo    = 0
        self.grade_counts = {"PERFECT": 0, "GOOD": 0, "OK": 0, "MISS": 0}

    def on_result(self, data: dict):
        """rhythm_result イベントのコールバック。"""
        grade = data.get("grade", "MISS")
        self.grade_counts[grade] = self.grade_counts.get(grade, 0) + 1

        if grade == "MISS":
            self.combo = 0
        else:
            self.combo += 1
            self.max_combo = max(self.max_combo, self.combo)
            # コンボボーナス
            combo_bonus = 1.0 + min(self.combo * 0.02, 1.0)
            self.total += int(self.SCORE_TABLE[grade] * combo_bonus)

    def rank(self) -> str:
        """スコアに応じたランクを返す。"""
        total_notes = sum(self.grade_counts.values())
        if total_notes == 0:
            return "D"
        perfect_rate = self.grade_counts["PERFECT"] / total_notes
        miss_rate    = self.grade_counts["MISS"]    / total_notes

        if perfect_rate >= 0.95 and miss_rate == 0:
            return "S"
        elif perfect_rate >= 0.80 and miss_rate <= 0.05:
            return "A"
        elif perfect_rate >= 0.60:
            return "B"
        elif miss_rate <= 0.3:
            return "C"
        else:
            return "D"

    def summary(self) -> dict:
        return {
            "total":       self.total,
            "max_combo":   self.max_combo,
            "grade_counts": self.grade_counts,
            "rank":        self.rank(),
        }

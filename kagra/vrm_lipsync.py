# kagra/vrm_lipsync.py
"""
VRM リップシンクシステム

音声振幅・WAV ファイル・母音推定からキャラクターの口の形を制御する。

【3つのモード】
1. 振幅モード（リアルタイム）
   lipsync.set_amplitude(0.7)   → 0.0〜1.0 の値を毎フレーム渡す

2. WAV ファイル先読みモード
   timeline = lipsync.analyze_wav("voice.wav")
   lipsync.play_timeline(timeline)
   # 毎フレーム lipsync.update(dt) で自動再生

3. テキスト母音マップモード（VOICEVOX 等の無音区間情報と組み合わせ）
   lipsync.play_text("こんにちは", duration=2.0)

Example::
    from kagra.vrm_lipsync import LipSyncController

    lipsync = LipSyncController(avatar)

    # WAV 先読み
    timeline = lipsync.analyze_wav("assets/voice/hello.wav")
    lipsync.play_timeline(timeline)

    def update(dt):
        lipsync.update(dt)   # 自動で口が動く

    # リアルタイム振幅
    amplitude = get_mic_amplitude()
    lipsync.set_amplitude(amplitude)
    lipsync.update(dt)
"""
from __future__ import annotations
import math
import struct
import os
from typing import Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from kagra.vrm_avatar import VrmAvatar

# 母音ブレンドシェイプ名（VRM 0.x / 1.0 両対応）
# 末尾の小文字単体は旧モデル（AliciaSolid 等）のフォールバック
_VOWEL_SHAPES = {
    "aa": ["aa", "Fcl_MTH_A", "A", "MTH_A", "a"],
    "ih": ["ih", "Fcl_MTH_I", "I", "MTH_I", "i"],
    "ou": ["ou", "Fcl_MTH_U", "U", "MTH_U", "u"],
    "ee": ["ee", "Fcl_MTH_E", "E", "MTH_E", "e"],
    "oh": ["oh", "Fcl_MTH_O", "O", "MTH_O", "o"],
}

# 日本語テキスト → 母音マッピング
_KANA_VOWEL = {
    "あ":"aa","か":"aa","さ":"aa","た":"aa","な":"aa","は":"aa","ま":"aa","や":"aa","ら":"aa","わ":"aa","ぁ":"aa","ゃ":"aa",
    "い":"ih","き":"ih","し":"ih","ち":"ih","に":"ih","ひ":"ih","み":"ih","り":"ih","ゐ":"ih","ぃ":"ih",
    "う":"ou","く":"ou","す":"ou","つ":"ou","ぬ":"ou","ふ":"ou","む":"ou","ゆ":"ou","る":"ou","ぅ":"ou","ゅ":"ou",
    "え":"ee","け":"ee","せ":"ee","て":"ee","ね":"ee","へ":"ee","め":"ee","れ":"ee","ゑ":"ee","ぇ":"ee",
    "お":"oh","こ":"oh","そ":"oh","と":"oh","の":"oh","ほ":"oh","も":"oh","よ":"oh","ろ":"oh","を":"oh","ぉ":"oh","ょ":"oh",
    # カタカナ
    "ア":"aa","カ":"aa","サ":"aa","タ":"aa","ナ":"aa","ハ":"aa","マ":"aa","ヤ":"aa","ラ":"aa","ワ":"aa",
    "イ":"ih","キ":"ih","シ":"ih","チ":"ih","ニ":"ih","ヒ":"ih","ミ":"ih","リ":"ih",
    "ウ":"ou","ク":"ou","ス":"ou","ツ":"ou","ヌ":"ou","フ":"ou","ム":"ou","ユ":"ou","ル":"ou",
    "エ":"ee","ケ":"ee","セ":"ee","テ":"ee","ネ":"ee","ヘ":"ee","メ":"ee","レ":"ee",
    "オ":"oh","コ":"oh","ソ":"oh","ト":"oh","ノ":"oh","ホ":"oh","モ":"oh","ヨ":"oh","ロ":"oh","ヲ":"oh",
}


class LipSyncTimeline:
    """WAV 解析結果。(time, vowel, weight) のリスト。"""

    def __init__(self, entries: list, duration: float):
        self.entries  = entries   # [(time_sec, vowel_str, weight), ...]
        self.duration = duration

    def __len__(self):
        return len(self.entries)


class LipSyncController:
    """VRM リップシンクコントローラー。

    Args:
        avatar:       VrmAvatar インスタンス
        smoothing:    スムージング係数（0.0=即時, 1.0=全く動かない）。推奨: 0.3〜0.6
        max_open:     口の最大開口ウェイト（0.0〜1.0）。デフォルト: 0.9

    Example::
        lipsync = LipSyncController(avatar, smoothing=0.4)
        timeline = lipsync.analyze_wav("voice.wav")
        lipsync.play_timeline(timeline)

        def update(dt):
            lipsync.update(dt)
    """

    def __init__(
        self,
        avatar: "VrmAvatar",
        smoothing: float = 0.4,
        max_open:  float = 0.9,
    ):
        self._avatar   = avatar
        self.smoothing = smoothing
        self.max_open  = max_open
        self.enabled   = True

        # 現在の各母音ウェイト（スムーズ値）
        self._weights: dict[str, float] = {v: 0.0 for v in _VOWEL_SHAPES}

        # タイムラインモード
        self._timeline: Optional[LipSyncTimeline] = None
        self._tl_time:  float = 0.0
        self._tl_idx:   int   = 0
        self._tl_playing: bool = False

        # リアルタイム振幅モード
        self._rt_amplitude: float = 0.0
        self._rt_vowel:     str   = "aa"

        # ブレンドシェイプ名の解決
        import kagra
        shapes = set(kagra.list_blend_shapes(avatar.vrm_id))
        self._shape_map: dict[str, Optional[str]] = {}
        for vowel, candidates in _VOWEL_SHAPES.items():
            found = next((s for s in candidates if s in shapes), None)
            self._shape_map[vowel] = found

        available = [f"{v}→{s}" for v, s in self._shape_map.items() if s]
        print(f"[LipSync] 利用可能シェイプ: {available if available else 'なし'}")

    # ── WAV 解析 ──────────────────────────────────────────────

    def analyze_wav(
        self,
        wav_path: str,
        fps: float = 24.0,
        silence_threshold: float = 0.02,
    ) -> LipSyncTimeline:
        """WAV ファイルを解析してリップシンクタイムラインを生成する。

        Args:
            wav_path:           WAV ファイルパス（PCM 16bit / 32bit float）
            fps:                タイムラインのフレームレート
            silence_threshold:  無音判定閾値（振幅）

        Returns:
            LipSyncTimeline: play_timeline() に渡せるタイムライン
        """
        samples, sample_rate = _load_wav_samples(wav_path)
        if not samples:
            print(f"[LipSync] WAV 読み込み失敗: {wav_path}")
            return LipSyncTimeline([], 0.0)

        frame_size   = max(1, int(sample_rate / fps))
        total_frames = max(1, len(samples) // frame_size)
        duration     = len(samples) / sample_rate
        entries      = []

        for i in range(total_frames):
            t     = i / fps
            chunk = samples[i * frame_size: (i + 1) * frame_size]
            amp   = _rms(chunk)

            if amp < silence_threshold:
                entries.append((t, "aa", 0.0))
            else:
                # 周波数成分から母音を簡易推定
                vowel  = _estimate_vowel_simple(chunk, sample_rate)
                weight = min(1.0, amp * 8.0) * self.max_open
                entries.append((t, vowel, weight))

        print(f"[LipSync] 解析完了: {wav_path} ({duration:.1f}秒, {total_frames}フレーム)")
        return LipSyncTimeline(entries, duration)

    def play_timeline(self, timeline: LipSyncTimeline):
        """タイムラインを先頭から再生開始する。"""
        self._timeline   = timeline
        self._tl_time    = 0.0
        self._tl_idx     = 0
        self._tl_playing = True
        self._rt_amplitude = 0.0

    def stop(self):
        """リップシンクを停止して口を閉じる。"""
        self._tl_playing  = False
        self._rt_amplitude = 0.0
        self._timeline    = None

    @property
    def is_playing(self) -> bool:
        return self._tl_playing

    # ── リアルタイム振幅モード ────────────────────────────────

    def set_amplitude(self, amplitude: float, vowel: str = "aa"):
        """リアルタイムで振幅を設定する（マイク入力等に使用）。

        Args:
            amplitude: 0.0〜1.0 の音量
            vowel:     母音ヒント ("aa"/"ih"/"ou"/"ee"/"oh")
        """
        self._rt_amplitude = max(0.0, min(1.0, amplitude))
        self._rt_vowel     = vowel
        self._tl_playing   = False

    # ── 毎フレーム更新 ────────────────────────────────────────

    def update(self, dt: float):
        """毎フレーム呼ぶ。"""
        if not self.enabled:
            return

        # ターゲットウェイトを決定
        target: dict[str, float] = {v: 0.0 for v in _VOWEL_SHAPES}

        if self._tl_playing and self._timeline:
            self._tl_time += dt
            tl = self._timeline

            # 現在時刻のエントリを探す
            while (self._tl_idx + 1 < len(tl.entries) and
                   tl.entries[self._tl_idx + 1][0] <= self._tl_time):
                self._tl_idx += 1

            if self._tl_idx < len(tl.entries):
                _, vowel, weight = tl.entries[self._tl_idx]
                target[vowel] = weight
            else:
                self._tl_playing = False  # 再生終了

        elif self._rt_amplitude > 0.001:
            target[self._rt_vowel] = self._rt_amplitude * self.max_open

        # スムージング & 適用
        import kagra
        vid = self._avatar.vrm_id
        smooth = 1.0 - self.smoothing

        for vowel, tgt in target.items():
            cur = self._weights[vowel]
            new = cur + (tgt - cur) * min(1.0, smooth * 60 * dt)
            new = max(0.0, min(1.0, new))
            self._weights[vowel] = new

            shape_name = self._shape_map.get(vowel)
            if shape_name and new > 0.001:
                kagra.get_engine().set_blend_shape(vid, shape_name, new)
            elif shape_name and cur > 0.001:
                kagra.get_engine().set_blend_shape(vid, shape_name, 0.0)

    # ── テキスト母音マップ ─────────────────────────────────────

    def play_text(self, text: str, duration: float, fps: float = 12.0):
        """テキストから母音シーケンスを生成して再生する（簡易 TTS 連携用）。

        Args:
            text:     読み上げテキスト（ひらがな・カタカナが有効）
            duration: 再生秒数
            fps:      タイムラインのフレームレート

        Example::
            lipsync.play_text("こんにちは", duration=1.5)
        """
        # 母音シーケンスを抽出
        vowels = []
        for ch in text:
            if ch in _KANA_VOWEL:
                vowels.append(_KANA_VOWEL[ch])
            elif ch.isalpha():
                v = ch.lower()
                if v in "aeiou":
                    map_ = {"a":"aa","e":"ee","i":"ih","o":"oh","u":"ou"}
                    vowels.append(map_.get(v, "aa"))

        if not vowels:
            vowels = ["aa"]

        entries      = []
        total_frames = max(1, int(duration * fps))
        for i in range(total_frames):
            t       = i / fps
            idx     = int(i / total_frames * len(vowels))
            vowel   = vowels[min(idx, len(vowels) - 1)]
            # 各音節の中間を開いて前後を閉じる波形
            phase   = (i % max(1, total_frames // len(vowels))) / max(1, total_frames // len(vowels))
            weight  = math.sin(phase * math.pi) * 0.85
            entries.append((t, vowel, weight * self.max_open))

        timeline = LipSyncTimeline(entries, duration)
        self.play_timeline(timeline)


# ── 内部ユーティリティ ─────────────────────────────────────────

def _load_wav_samples(path: str) -> tuple[list[float], int]:
    """WAV ファイルを読み込んで正規化済みサンプルリストを返す。"""
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError as e:
        print(f"[LipSync] WAV 読み込みエラー: {e}")
        return [], 44100

    # RIFF ヘッダチェック
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE":
        print("[LipSync] WAV フォーマットエラー")
        return [], 44100

    # fmt チャンク解析
    offset = 12
    sample_rate = 44100
    num_channels = 1
    bits_per_sample = 16
    audio_format = 1   # 1=PCM, 3=float

    while offset + 8 <= len(data):
        chunk_id   = data[offset:offset + 4]
        chunk_size = struct.unpack_from("<I", data, offset + 4)[0]
        offset    += 8

        if chunk_id == b"fmt ":
            audio_format    = struct.unpack_from("<H", data, offset)[0]
            num_channels    = struct.unpack_from("<H", data, offset + 2)[0]
            sample_rate     = struct.unpack_from("<I", data, offset + 4)[0]
            bits_per_sample = struct.unpack_from("<H", data, offset + 14)[0]
        elif chunk_id == b"data":
            raw = data[offset: offset + chunk_size]
            break
        offset += chunk_size
    else:
        return [], sample_rate

    # PCM デコード
    samples_all = []
    if audio_format == 1 and bits_per_sample == 16:
        count = len(raw) // 2
        samples_all = list(struct.unpack_from(f"<{count}h", raw))
        samples_all = [s / 32768.0 for s in samples_all]
    elif audio_format == 1 and bits_per_sample == 8:
        samples_all = [(b / 128.0 - 1.0) for b in raw]
    elif audio_format == 3 and bits_per_sample == 32:
        count = len(raw) // 4
        samples_all = list(struct.unpack_from(f"<{count}f", raw))
    else:
        print(f"[LipSync] 非対応 WAV フォーマット: {audio_format}/{bits_per_sample}bit")
        return [], sample_rate

    # モノラル化（複数チャンネルは平均）
    if num_channels > 1:
        mono = []
        for i in range(0, len(samples_all) - num_channels + 1, num_channels):
            mono.append(sum(samples_all[i:i + num_channels]) / num_channels)
        samples_all = mono

    return samples_all, sample_rate


def _rms(samples: list[float]) -> float:
    """RMS 振幅を計算する。"""
    if not samples:
        return 0.0
    return math.sqrt(sum(s * s for s in samples) / len(samples))


def _estimate_vowel_simple(samples: list[float], sample_rate: int) -> str:
    """簡易母音推定（ゼロ交差率 + エネルギー比）。

    正確な推定は FFT / MFCC が必要だが、軽量実装として
    ゼロ交差率（ZCR）で子音/母音の大まかな分類のみ行う。
    日常的には "aa" が最も多く使われるためデフォルト。
    """
    if not samples:
        return "aa"

    # ゼロ交差率
    zc = sum(1 for i in range(1, len(samples)) if samples[i - 1] * samples[i] < 0)
    zcr = zc / len(samples) * sample_rate

    # 非常に簡易な分類
    if zcr > 3000:
        return "ih"   # 高 ZCR → イ系
    elif zcr > 1500:
        return "ee"   # 中 ZCR → エ系
    else:
        # エネルギー重心で ア/ウ/オ を区別
        mid = len(samples) // 2
        e1 = _rms(samples[:mid])
        e2 = _rms(samples[mid:])
        if e2 > e1 * 1.2:
            return "oh"
        elif e1 > e2 * 1.2:
            return "ou"
        return "aa"

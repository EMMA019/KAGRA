"""DSP（純 Python、WAV bytes → WAV bytes）。汎用エンジン化 Phase 4。

ゲームロジックは Python のみ。再生はシェル側（winsound 等）が 1 音ずつ
鳴らすだけなので、**混ぜる / 加工するのはここで WAV bytes に対して行う**:

- ``mix``        複数モノ WAV を重ねて 1 本にする（BGM+SE を先に合成）。
- ``reverb``     Schroeder リバーブ（コムフィルタ 4 + オールパス 2）。
- ``crossfade``  BGM 切替のクロスフェード（等パワークロス）。
- ``duck``       SE タイミングで BGM 音量を下げる（ダッキング）。

全て決定論的（乱数なし）。モノラル 16-bit PCM 前提（audio.py の出力形式）。
"""
from __future__ import annotations

import math
import struct
import wave
from io import BytesIO

from kagra.audio import RATE, _pcm, _wav_bytes

__all__ = ["mix", "reverb", "crossfade", "duck"]


# ── 共通 ────────────────────────────────────────────────────────────────

def _samples(wav: bytes) -> list[float]:
    """WAV bytes → float サンプル列（-1..1）。"""
    pcm = _pcm(wav)
    n = len(pcm) // 2
    return [struct.unpack_from("<h", pcm, i * 2)[0] / 32767.0 for i in range(n)]


def _wav(samples: list[float], rate: int) -> bytes:
    """float サンプル列 → 16-bit PCM WAV（クリップ防止）。"""
    pcm = bytearray()
    for v in samples:
        s = int(max(-1.0, min(1.0, v)) * 32767)
        pcm += struct.pack("<h", s)
    return _wav_bytes(bytes(pcm), rate)


def _pad(a: list[float], b: list[float]) -> tuple[list[float], list[float]]:
    n = max(len(a), len(b))
    return a + [0.0] * (n - len(a)), b + [0.0] * (n - len(b))


# ── mix ─────────────────────────────────────────────────────────────────

def mix(*wavs: bytes, gains: list[float] | None = None) -> bytes:
    """複数モノ WAV を重ねて 1 本にする（長さは最長に合わせる）。

    例: ``mix(bgm, se)`` — winsound は 1 音しか鳴らせないので、ゲームは
    BGM と SE を先にここで合成してから鳴らす。``gains`` は各入力の音量。
    """
    if not wavs:
        return _wav([], RATE)
    rate = RATE
    buf: list[float] = []
    for i, w in enumerate(wavs):
        g = gains[i] if gains is not None else 1.0
        s = [v * g for v in _samples(w)]
        buf, s = _pad(buf, s)
        buf = [a + b for a, b in zip(buf, s)]
    return _wav(buf, rate)


# ── reverb ──────────────────────────────────────────────────────────────

_COMB_MS = (29.7, 37.1, 41.1, 43.7)
_ALLPASS_MS = (5.0, 1.7)


def reverb(
    wav: bytes,
    *,
    roomsize: float = 0.6,
    damping: float = 0.5,
    wet: float = 0.35,
    rate: int = RATE,
) -> bytes:
    """Schroeder リバーブ。モノ WAV に残響を加える。

    - 4 本のコムフィルタ（feedback は roomsize でスケール）
    - 2 本のオールパスフィルタ（染色の打ち消し）
    - 出力 = dry + wet * 残響

    決定論的。``wet=0`` で入力そのまま（バイパス）。
    """
    if wet <= 0.0:
        return wav
    src = _samples(wav)
    n = len(src)
    fb = 0.84 * max(0.0, min(1.0, roomsize))
    dmp = max(0.0, min(1.0, damping))
    combs: list[list[float]] = []
    for ms in _COMB_MS:
        delay = max(1, int(ms / 1000.0 * rate))
        buf = [0.0] * delay
        out = [0.0] * n
        for i in range(n):
            j = i % delay
            x = src[i] + buf[j] * fb
            out[i] = buf[j]
            # 1 ポールローパス: 減衰を滑らかに（高域の残響を落とす）
            buf[j] = x * (1.0 - dmp) + buf[j] * dmp
        combs.append(out)
    verb = [0.0] * n
    for c in combs:
        for i in range(n):
            verb[i] += c[i]
    for ms in _ALLPASS_MS:
        delay = max(1, int(ms / 1000.0 * rate))
        buf = [0.0] * delay
        out = [0.0] * n
        for i in range(n):
            j = i % delay
            b = buf[j]
            buf[j] = verb[i] + b * 0.5
            out[i] = b - verb[i]
        verb = out
    # dry + wet。残響は comb 4 本分の和なので wet を 1/4 に正規化
    mixed = [src[i] + verb[i] * wet * 0.25 for i in range(n)]
    return _wav(mixed, rate)


# ── crossfade ───────────────────────────────────────────────────────────

def crossfade(a: bytes, b: bytes, seconds: float, *, rate: int = RATE) -> bytes:
    """BGM 切替のクロスフェード。

    ``a`` の末尾 ``seconds`` 秒と ``b`` の先頭 ``seconds`` 秒を等パワーで
    クロスフェードし、``a + b`` の長さ（先頭重複なし）の 1 本にする。
    ``seconds <= 0`` なら単純連結。入力は同 rate のモノ WAV。
    """
    sa = _samples(a)
    sb = _samples(b)
    if seconds <= 0.0:
        return _wav(sa + sb, rate)
    fade = max(1, int(seconds * rate))
    if fade >= len(sa) or fade >= len(sb):
        # 片方がフェード長より短い → 短い側いっぱいでフェード
        fade = min(len(sa), len(sb))
        if fade <= 0:
            return _wav(sa + sb, rate)
    out = list(sa) + list(sb[fade:])
    for i in range(fade):
        t = i / fade
        # 等パワークロス: cos/sin カーブ
        ga = math.cos(t * math.pi / 2.0)
        gb = math.sin(t * math.pi / 2.0)
        ai = len(sa) - fade + i
        out[ai] = sa[ai] * ga + sb[i] * gb
    return _wav(out, rate)


# ── duck ────────────────────────────────────────────────────────────────

def duck(
    bgm: bytes,
    *,
    at: float = 0.0,
    dur: float = 0.3,
    amount: float = 0.6,
    attack: float = 0.02,
    release: float = 0.2,
    rate: int = RATE,
) -> bytes:
    """SE が鳴る間、BGM を下げる（ダッキング）。

    ``at`` 秒後に ``amount`` まで下げ（``attack`` 秒）、``dur`` 秒保持して
    ``release`` 秒で戻す。ゲームは SE を鳴らす瞬間に ``duck(bgm, at=now)``
    を合成して鳴らす。``amount=0`` で無加工。
    """
    if amount <= 0.0:
        return bgm
    s = _samples(bgm)
    n = len(s)
    amount = max(0.0, min(1.0, amount))
    attack_n = max(1, int(attack * rate))
    hold_n = max(0, int(dur * rate))
    release_n = max(1, int(release * rate))
    start = max(0, min(n - 1, int(at * rate)))
    out = list(s)
    for i in range(attack_n):
        idx = start + i
        if idx >= n:
            break
        t = i / attack_n
        g = 1.0 - amount * t
        out[idx] = s[idx] * g
    for i in range(hold_n):
        idx = start + attack_n + i
        if idx >= n:
            break
        out[idx] = s[idx] * (1.0 - amount)
    for i in range(release_n):
        idx = start + attack_n + hold_n + i
        if idx >= n:
            break
        t = i / release_n
        g = 1.0 - amount + amount * t
        out[idx] = s[idx] * g
    return _wav(out, rate)

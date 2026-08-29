"""音（Python のみ）。0.19 の tone() / sound() を shared ゲームマスター用に移植。

合成は全部純 Python（WAV bytes を返す）。再生は「シェル側 = Python 側」の
原則（audio.rs と同じ）: デスクトップは標準ライブラリ winsound（Windows）
で鳴らし、無い環境は no-op。wasm / Android / iOS は各シェルの AudioTrack /
AVAudioEngine / Web Audio が担当するので、ここでは触らない。

使い方::

    from kagra.audio import se, tone, play_wav, set_listener, play_se

    se("coin")          # プリセット SE（キャッシュ済み）
    play_wav(tone(880, 0.08, wave="sine"))   # 任意トーン
    set_listener(0, 0, 0, 0, 0, 1)           # 聞き手を置く（前 = +Z）
    play_se("coin", x=5, y=0, z=0)           # 右から聞こえる（距離減衰 + パン）
"""
from __future__ import annotations

import math
import struct
import sys
import wave
from io import BytesIO

from kagra.spatial import spatial_mix

__all__ = ["tone", "sound", "se", "play_wav", "preset_names", "set_listener", "play_se"]

RATE = 22050

# ── 合成 ────────────────────────────────────────────────────────────────

def _envelope(n: int, decay: float) -> list[float]:
    """クリック防止の指数減衰。`decay` は 1 サンプルあたりの減衰率。"""
    out = [1.0] * n
    a = 1.0
    for i in range(1, n):
        a *= decay
        out[i] = a
    return out


def tone(
    freq: float = 440.0,
    dur: float = 0.1,
    vol: float = 0.5,
    wave: str = "sine",
    rate: int = RATE,
    decay: float = 0.995,
) -> bytes:
    """単音の WAV bytes（16-bit PCM mono）。

    wave: "sine" | "square" | "saw" | "noise"。freq=0 は無音（ノイズ専用
    で使う）。
    """
    n = max(1, int(rate * dur))
    env = _envelope(n, decay)
    samples = bytearray()
    for i in range(n):
        t = i / rate
        if wave == "noise":
            v = _noise(i)
        elif wave == "square":
            v = 1.0 if math.sin(2 * math.pi * freq * t) >= 0 else -1.0
        elif wave == "saw":
            v = 2.0 * ((freq * t) % 1.0) - 1.0
        else:
            v = math.sin(2 * math.pi * freq * t)
        s = int(max(-1.0, min(1.0, v * env[i])) * vol * 32767)
        samples += struct.pack("<h", s)
    return _wav_bytes(bytes(samples), rate)


def _noise(i: int) -> float:
    """決定的ノイズ（再現可能。シードは不要な単純 xorshift）。"""
    x = (i * 0x9E3779B9) & 0xFFFFFFFF
    x ^= x >> 16
    x = (x * 0x21F0AAAD) & 0xFFFFFFFF
    x ^= x >> 15
    return (x / 0xFFFFFFFF) * 2.0 - 1.0


def _wav_bytes(pcm: bytes, rate: int) -> bytes:
    buf = BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(pcm)
    return buf.getvalue()


# ── プリセット ──────────────────────────────────────────────────────────

_PRESETS: dict[str, bytes] = {}


def sound(name: str) -> bytes:
    """プリセット SE の WAV bytes（初回合成、以降キャッシュ）。"""
    cached = _PRESETS.get(name)
    if cached is not None:
        return cached
    wav = _make_preset(name)
    _PRESETS[name] = wav
    return wav


def preset_names() -> list[str]:
    return list(_PRESETS)


def _make_preset(name: str) -> bytes:
    if name == "coin":
        return _concat(
            tone(880, 0.06, 0.4, "sine", decay=0.9),
            tone(1320, 0.12, 0.4, "sine", decay=0.9),
        )
    if name == "jump":
        return tone(300, 0.12, 0.35, "square", decay=0.94)
    if name == "hit":
        return tone(0, 0.08, 0.5, "noise", decay=0.9)
    if name == "ok":
        return tone(660, 0.08, 0.35, "sine", decay=0.92)
    if name == "bite":
        return _concat(
            tone(0, 0.05, 0.5, "noise", decay=0.85),
            tone(120, 0.1, 0.4, "square", decay=0.9),
        )
    if name == "cast":
        return _concat(
            tone(440, 0.05, 0.3, "sine", decay=0.9),
            tone(660, 0.08, 0.3, "sine", decay=0.9),
        )
    if name == "hurt":
        return _concat(
            tone(180, 0.15, 0.4, "saw", decay=0.92),
            tone(90, 0.2, 0.35, "square", decay=0.94),
        )
    return tone(440, 0.1, 0.3, "sine")  # 未知名は単音


def _concat(*wavs: bytes) -> bytes:
    """複数 WAV を連結（同 rate / mono / 16bit 前提）。"""
    pcm = b"".join(_pcm(w) for w in wavs)
    return _wav_bytes(pcm, RATE)


def _pcm(wav: bytes) -> bytes:
    """WAV bytes から PCM を取り出す（このモジュールが作った形式のみ対応）。"""
    buf = BytesIO(wav)
    with wave.open(buf, "rb") as w:
        assert w.getnchannels() == 1 and w.getsampwidth() == 2
        return w.readframes(w.getnframes())


# ── 再生 ────────────────────────────────────────────────────────────────

def play_wav(wav: bytes, loop: bool = False) -> None:
    """WAV bytes を鳴らす。デスクトップは winsound（Windows）。他は no-op。

    shared の「実際の再生はシェル側」方針どおり、ここは Python（シェル）
    側の最小実装。wasm / mobile は各プラットフォームの再生経路が担う。
    """
    if sys.platform != "win32":
        return
    try:
        import winsound  # type: ignore[import-not-found]

        flags = winsound.SND_MEMORY | winsound.SND_ASYNC
        if loop:
            flags |= winsound.SND_LOOP
        winsound.PlaySound(wav, flags)
    except Exception:  # pragma: no cover - 音が出せない環境は静かに無視
        pass


# ── 3D 音響（距離減衰 + ステレオパン） ────────────────────────────────────

_listener = {
    "x": 0.0, "y": 0.0, "z": 0.0,
    "fx": 0.0, "fy": 0.0, "fz": 1.0,
    "ux": 0.0, "uy": 1.0, "uz": 0.0,
}


def set_listener(
    x: float = 0.0,
    y: float = 0.0,
    z: float = 0.0,
    fx: float = 0.0,
    fy: float = 0.0,
    fz: float = 1.0,
    ux: float = 0.0,
    uy: float = 1.0,
    uz: float = 0.0,
) -> None:
    """聞き手の位置と向き（前 = forward、上 = up）。play_se の定位基準。"""
    _listener.update(
        x=x, y=y, z=z, fx=fx, fy=fy, fz=fz, ux=ux, uy=uy, uz=uz
    )


def _spatialize(wav: bytes, left: float, right: float) -> bytes:
    """モノ WAV をステレオ WAV にし、左右ゲインを焼き込む（0..1）。"""
    pcm = _pcm(wav)
    l = max(0.0, min(1.0, left))
    r = max(0.0, min(1.0, right))
    n = len(pcm) // 2
    out = bytearray()
    for i in range(n):
        s = struct.unpack_from("<h", pcm, i * 2)[0]
        out += struct.pack("<hh", int(s * l), int(s * r))
    buf = BytesIO()
    with wave.open(buf, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(bytes(out))
    return buf.getvalue()


def play_se(
    name: str,
    x: float = 0.0,
    y: float = 0.0,
    z: float = 0.0,
    volume: float = 1.0,
    ref_distance: float = 4.0,
    max_distance: float = 48.0,
) -> None:
    """3D SE。聞き手（set_listener）から見た距離減衰 + ステレオパン。

    0.19 の ``play_se(path, x=, y=, z=, ref_distance=, max_distance=)`` 相当
    （spatial.py と同じ逆二乗減衰 + equal-power パン。HRTF ではない）。
    ``x=y=z=0`` かつリスナーが原点なら 2D のまま（パン無し）。
    """
    _, _, left, right = spatial_mix(
        _listener["x"], _listener["y"], _listener["z"],
        _listener["fx"], _listener["fy"], _listener["fz"],
        x, y, z,
        ref_distance=ref_distance,
        max_distance=max_distance,
        ux=_listener["ux"], uy=_listener["uy"], uz=_listener["uz"],
    )
    if left <= 0.0 and right <= 0.0:
        return
    v = max(0.0, min(1.0, volume))
    stereo = _spatialize(sound(name), left * v, right * v)
    play_wav(stereo)


def se(name: str) -> None:
    """プリセット SE を鳴らす（なければ合成して鳴らす）。"""
    play_wav(sound(name))

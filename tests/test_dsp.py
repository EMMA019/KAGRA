"""DSP（kagra.dsp — Phase 4 音）の純ロジックテスト。

WAV bytes → WAV bytes の加工関数。全て決定論的（乱数なし）。再生はしない。
"""
import math
import struct
import wave
from io import BytesIO

from tests.conftest import load_kagra_submodule

audio = load_kagra_submodule("audio")
dsp = load_kagra_submodule("dsp")

RATE = audio.RATE


def _is_wav(b: bytes) -> bool:
    return b[:4] == b"RIFF" and b[8:12] == b"WAVE" and b"data" in b


def _samples(wav: bytes) -> list[int]:
    with wave.open(BytesIO(wav), "rb") as w:
        assert w.getnchannels() == 1 and w.getsampwidth() == 2
        frames = w.readframes(w.getnframes())
    return list(struct.unpack("<%dh" % (len(frames) // 2), frames))


def _rms(wav: bytes) -> float:
    s = _samples(wav)
    if not s:
        return 0.0
    return math.sqrt(sum(v * v for v in s) / len(s))


def test_dsp_imports_and_presets_are_mono():
    for n in ["coin", "ok", "hurt"]:
        wav = audio.sound(n)
        assert _is_wav(dsp.reverb(wav, wet=0.0))  # バイパス
        assert _is_wav(dsp.mix(wav))
        assert _is_wav(dsp.crossfade(wav, wav, 0.05))
        assert _is_wav(dsp.duck(wav, at=0.0))


# ── mix ─────────────────────────────────────────────────────────────────

def test_mix_sums_two_wavs():
    a = audio.tone(440, 0.1, vol=0.5)
    b = audio.tone(880, 0.1, vol=0.5)
    m = dsp.mix(a, b)
    assert _is_wav(m)
    sa, sb, sm = _samples(a), _samples(b), _samples(m)
    assert len(sm) == max(len(sa), len(sb))
    # 同一位相なら和は 2 倍（合成は 2 トーンでもクリップしない）
    peak = max(abs(v) for v in sm)
    assert peak > 0, "mix に音がある"


def test_mix_gains():
    a = audio.tone(440, 0.1, vol=1.0)
    full = dsp.mix(a)
    half = dsp.mix(a, gains=[0.5])
    assert _rms(full) > _rms(half) > 0, "gains が効く"


def test_mix_pads_shorter():
    short = audio.tone(440, 0.05, vol=0.5)
    long = audio.tone(880, 0.1, vol=0.5)
    m = dsp.mix(short, long)
    assert len(_samples(m)) == len(_samples(long)), "長い方に合わせる"


def test_mix_deterministic():
    a = dsp.mix(audio.sound("coin"), audio.sound("hit"))
    b = dsp.mix(audio.sound("coin"), audio.sound("hit"))
    assert a == b, "決定論"


# ── reverb ──────────────────────────────────────────────────────────────

def test_reverb_wet_zero_is_identity():
    wav = audio.tone(440, 0.1, vol=0.5)
    assert dsp.reverb(wav, wet=0.0) == wav, "wet=0 はバイパス"


def test_reverb_adds_tail():
    # 短いトーンに残響を足すと、元より長いエネルギーが残る
    wav = audio.tone(440, 0.05, vol=0.5)
    dry = dsp.reverb(wav, wet=0.0)
    wet = dsp.reverb(wav, wet=0.5, roomsize=0.8)
    s_dry = _samples(dry)
    s_wet = _samples(wet)
    # 末尾側（元の音が既に減衰した領域）に残響成分がある
    tail_dry = max(abs(v) for v in s_dry[len(s_dry) // 2:])
    tail_wet = max(abs(v) for v in s_wet[len(s_wet) // 2:])
    assert tail_wet > tail_dry, "リバーブは音の後に尾を引く"
    assert len(s_wet) == len(s_dry), "長さは変わらない"


def test_reverb_deterministic():
    wav = audio.tone(440, 0.1, vol=0.5)
    assert dsp.reverb(wav, wet=0.4) == dsp.reverb(wav, wet=0.4)


# ── crossfade ───────────────────────────────────────────────────────────

def test_crossfade_concats_when_zero():
    a = audio.tone(440, 0.1, vol=0.5)
    b = audio.tone(880, 0.1, vol=0.5)
    c = dsp.crossfade(a, b, 0.0)
    assert len(_samples(c)) == len(_samples(a)) + len(_samples(b))
    assert c[:4] == b"RIFF"


def test_crossfade_length_is_sum_minus_fade():
    a = audio.tone(440, 0.1, vol=0.5)
    b = audio.tone(880, 0.1, vol=0.5)
    fade = 0.02
    c = dsp.crossfade(a, b, fade)
    expect = len(_samples(a)) + len(_samples(b)) - int(fade * RATE)
    assert abs(len(_samples(c)) - expect) <= 1


def test_crossfade_blends_tail_and_head():
    a = audio.tone(440, 0.1, vol=0.5)
    b = audio.tone(880, 0.1, vol=0.5)
    c = dsp.crossfade(a, b, 0.05)
    sc = _samples(c)
    # フェード領域の中央は両方の成分がある（a の末尾と b の先頭の和）
    fade_n = int(0.05 * RATE)
    mid = len(_samples(a)) - fade_n + fade_n // 2
    assert abs(sc[mid]) > 0, "クロスフェード領域に音がある"


def test_crossfade_deterministic():
    a = audio.tone(440, 0.1, vol=0.5)
    b = audio.tone(880, 0.1, vol=0.5)
    assert dsp.crossfade(a, b, 0.05) == dsp.crossfade(a, b, 0.05)


# ── duck ────────────────────────────────────────────────────────────────

def _window_rms(samples: list[int], start: int, n: int) -> float:
    win = samples[start : start + n]
    if not win:
        return 0.0
    return math.sqrt(sum(v * v for v in win) / len(win))


def test_duck_lowers_bgm_around_se_time():
    bgm = audio.tone(440, 0.3, vol=0.5, decay=0.9995)  # 減衰が遅いトーン
    at = 0.1
    d = dsp.duck(bgm, at=at, dur=0.1, amount=0.8, attack=0.005, release=0.005)
    s = _samples(bgm)
    sd = _samples(d)
    # ダッキング窓（at 直後）は音量が落ちる（窓 RMS で比較）
    win = int(0.02 * RATE)
    before = _window_rms(s, int(at * RATE), win)
    during = _window_rms(sd, int(at * RATE), win)
    assert before > 0, "比較元に音がある"
    assert during < before * 0.8, "SE 中は BGM が下がる"
    # 窓の前（at より十分前）は変わらない
    i0 = int(0.02 * RATE)
    assert sd[i0] == s[i0], "ダッキング前は無加工"


def test_duck_recovers_after_release():
    bgm = audio.tone(440, 0.3, vol=0.5)
    d = dsp.duck(bgm, at=0.05, dur=0.05, amount=0.9, attack=0.005, release=0.03)
    s = _samples(bgm)
    sd = _samples(d)
    tail = int(0.2 * RATE)  # release 後の領域
    assert sd[tail] == s[tail], "release 後は元の音量に戻る"


def test_duck_amount_zero_is_identity():
    bgm = audio.tone(440, 0.2, vol=0.5)
    assert dsp.duck(bgm, at=0.05, amount=0.0) == bgm


def test_duck_deterministic():
    bgm = audio.tone(440, 0.2, vol=0.5)
    assert dsp.duck(bgm, at=0.05) == dsp.duck(bgm, at=0.05)

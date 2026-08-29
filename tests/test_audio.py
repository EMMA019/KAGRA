"""音（kagra.audio）の純ロジックテスト。

合成は全部 Python（WAV bytes）なので拡張不要。再生はテストで実際に
鳴らさない（winsound は非同期再生のため）。play_wav は壊れたデータでも
例外を出さないことだけ確認する。
"""
from tests.conftest import load_kagra_submodule

audio = load_kagra_submodule("audio")


def _is_wav(b: bytes) -> bool:
    return b[:4] == b"RIFF" and b[8:12] == b"WAVE" and b"data" in b


def test_tone_is_wav_with_correct_length():
    wav = audio.tone(440, 0.1, rate=8000)
    assert _is_wav(wav)
    import wave
    from io import BytesIO

    with wave.open(BytesIO(wav), "rb") as w:
        assert w.getnchannels() == 1
        assert w.getsampwidth() == 2
        assert w.getframerate() == 8000
        assert w.getnframes() == 800  # 0.1s @ 8kHz


def test_tone_waveforms_all_valid():
    for wv in ("sine", "square", "saw", "noise"):
        assert _is_wav(audio.tone(440, 0.05, wave=wv, rate=8000)), wv


def test_noise_is_deterministic():
    a = audio.tone(0, 0.05, wave="noise", rate=8000)
    b = audio.tone(0, 0.05, wave="noise", rate=8000)
    assert a == b, "合成は決定的（再現可能）"


def test_presets_exist_and_differ():
    names = ["coin", "jump", "hit", "ok", "bite", "cast", "hurt"]
    wavs = {n: audio.sound(n) for n in names}
    for n, w in wavs.items():
        assert _is_wav(w), n
        assert len(w) > 100, n
    assert len(set(wavs.values())) >= 6, "プリセットは互いに異なる"


def test_sound_caches():
    a = audio.sound("coin")
    b = audio.sound("coin")
    assert a is b, "2 回目はキャッシュ"


def test_coin_is_two_tones():
    import wave
    from io import BytesIO

    wav = audio.sound("coin")
    with wave.open(BytesIO(wav), "rb") as w:
        # 880Hz 0.06s + 1320Hz 0.12s @ 22050 → 約 0.18s
        n = w.getnframes()
    assert 0.15 * 22050 < n < 0.22 * 22050, f"coin は 2 音（frames={n}）"


def test_play_wav_never_raises():
    audio.play_wav(b"garbage-not-a-wav")
    audio.play_wav(audio.tone(440, 0.05, rate=8000), loop=False)


# ── 3D 音響 ──────────────────────────────────────────────────────────────

def test_spatial_mix_distance_attenuation():
    # 近い = ゲイン大、遠い = 0
    g_near = audio.spatial_mix(0, 0, 0, 0, 0, 1, 0, 0, 0)[0]
    assert g_near == 1.0
    g_far = audio.spatial_mix(0, 0, 0, 0, 0, 1, 100, 0, 0, max_distance=48.0)[0]
    assert g_far == 0.0


def test_spatial_mix_panning_direction():
    # 前 +Z、音源 +X（右）→ pan は +1（右スピーカー）
    _, pan, left, right = audio.spatial_mix(0, 0, 0, 0, 0, 1, 5, 0, 0)
    assert pan > 0.9
    assert right > left
    # 音源 -X（左）→ pan は -1
    _, pan2, left2, right2 = audio.spatial_mix(0, 0, 0, 0, 0, 1, -5, 0, 0)
    assert pan2 < -0.9
    assert left2 > right2


def test_spatialize_makes_stereo_with_pan():
    import wave
    from io import BytesIO

    wav = audio.sound("ok")
    stereo = audio._spatialize(wav, 1.0, 0.0)
    with wave.open(BytesIO(stereo), "rb") as w:
        assert w.getnchannels() == 2
        frames = w.readframes(w.getnframes())
    # 左チャンネルにだけ音がある（右 = 0）
    import struct

    samples = struct.unpack("<%dh" % (len(frames) // 2), frames)
    left_peak = max(abs(samples[i]) for i in range(0, len(samples), 2))
    right_peak = max(abs(samples[i]) for i in range(1, len(samples), 2))
    assert left_peak > 0
    assert right_peak == 0, "右ゲイン 0 なら右チャンネルは無音"


def test_play_se_respects_listener():
    audio.set_listener(0, 0, 0, 0, 0, 1)
    audio.play_se("ok", x=3, y=0, z=0, volume=0.5)  # 右から、鳴らして例外なし
    audio.play_se("ok", x=100, y=0, z=0, max_distance=48.0)  # 遠すぎ → 無音（例外なし）

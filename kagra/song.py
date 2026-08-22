# kagra/song.py
"""歌声シンセサイザ（純 Python・外部依存なし）。

VRM デモ用に、音声アセットなしで「歌」を用意する。
歌声はフォルマント重み付きの加算合成。音符列から作るので、
リップシンク用の母音タイムラインが波形解析なしで正確に手に入る。

Example::
    from kagra.song import generate_song

    path, entries, duration = generate_song()
    # path     : 書き出した WAV（16bit PCM mono）
    # entries  : [(time_sec, vowel, weight), ...]  ← LipSyncTimeline にそのまま渡せる
    # duration : 曲の長さ（秒）

通常は VrmAvatar.sing() が内部で呼ぶので、直接使うのはカスタム曲を
作りたいときだけ。メロディは (開始拍, 長さ拍, MIDIノート, 母音) のタプル列。
"""
from __future__ import annotations

import math
import os
import struct
import tempfile
import wave

SAMPLE_RATE = 22050
BPM = 112.0
TAIL_SEC = 0.6

VOWELS = ("aa", "ih", "ou", "ee", "oh")

# 母音フォルマント（中心周波数 Hz, ゲイン）。女声寄りの近似値。
_FORMANTS: dict[str, tuple[tuple[float, float], ...]] = {
    "aa": ((850.0, 1.00), (1250.0, 0.60), (2800.0, 0.25)),
    "ih": ((350.0, 1.00), (2250.0, 0.70), (3000.0, 0.30)),
    "ou": ((380.0, 1.00), (950.0, 0.55), (2650.0, 0.20)),
    "ee": ((550.0, 1.00), (1950.0, 0.65), (2850.0, 0.28)),
    "oh": ((500.0, 1.00), (950.0, 0.60), (2750.0, 0.22)),
}
_FORMANT_BW = (90.0, 140.0, 200.0)


def _midi_hz(m: float) -> float:
    return 440.0 * 2.0 ** ((m - 69.0) / 12.0)


def default_song() -> tuple[list, list, float]:
    """内蔵ソングを返す。

    Returns:
        (melody, chords, bpm)
        melody: [(start_beat, dur_beats, midi, vowel), ...]
        chords: 1小節ごとの (bass_midi, (triad...)) のリスト
    """
    # C → G → Am → F の王道進行 8 小節（4/4）
    C, G, Am, F = (48, (60, 64, 67)), (43, (59, 62, 67)), (45, (57, 60, 64)), (41, (57, 60, 65))
    chords = [C, G, Am, F, C, G, Am, C]
    melody = [
        # bar 1 (C)
        (0.0, 0.5, 76, "aa"), (0.5, 0.5, 79, "aa"), (1.0, 1.0, 81, "oh"),
        (2.0, 1.0, 79, "ou"), (3.0, 1.0, 76, "ee"),
        # bar 2 (G)
        (4.0, 1.5, 74, "aa"), (5.5, 0.5, 76, "ih"), (6.0, 2.0, 79, "oh"),
        # bar 3 (Am)
        (8.0, 0.5, 81, "aa"), (8.5, 0.5, 84, "aa"), (9.0, 1.0, 81, "ee"),
        (10.0, 1.0, 79, "ou"), (11.0, 1.0, 76, "oh"),
        # bar 4 (F)
        (12.0, 1.5, 77, "aa"), (13.5, 0.5, 76, "ih"), (14.0, 2.0, 74, "ou"),
        # bar 5 (C)
        (16.0, 0.5, 76, "aa"), (16.5, 0.5, 79, "oh"), (17.0, 1.0, 84, "aa"),
        (18.0, 1.0, 83, "ee"), (19.0, 1.0, 79, "ou"),
        # bar 6 (G)
        (20.0, 1.5, 81, "aa"), (21.5, 0.5, 79, "ih"), (22.0, 2.0, 83, "oh"),
        # bar 7 (Am)
        (24.0, 0.5, 84, "aa"), (24.5, 0.5, 83, "ih"), (25.0, 1.0, 81, "ee"),
        (26.0, 1.0, 79, "oh"), (27.0, 1.0, 77, "ou"),
        # bar 8 (C)
        (28.0, 1.0, 74, "ou"), (29.0, 3.0, 72, "aa"),
    ]
    return melody, chords, BPM


def _harmonic_amps(f0: float, vowel: str, sr: int) -> list[tuple[int, float]]:
    """基音 f0 の各倍音に母音フォルマントの重みを掛けた振幅表を返す。"""
    formants = _FORMANTS[vowel]
    out: list[tuple[int, float]] = []
    n = 1
    while n * f0 < sr * 0.45 and n <= 14:
        f = n * f0
        gain = 0.06
        for (fc, g), bw in zip(formants, _FORMANT_BW):
            gain += g * math.exp(-0.5 * ((f - fc) / bw) ** 2)
        out.append((n, gain / n ** 0.7))
        n += 1
    total = sum(abs(a) for _, a in out) or 1.0
    return [(n, a / total) for n, a in out]


def _render_voice(buf: list[float], melody: list, spb: float, sr: int, vol: float = 0.52):
    two_pi = 2.0 * math.pi
    for start_b, dur_b, midi, vowel in melody:
        f0 = _midi_hz(midi)
        harm = _harmonic_amps(f0, vowel, sr)
        t0 = start_b * spb
        dur = dur_b * spb
        i0, i1 = int(t0 * sr), min(int((t0 + dur) * sr), len(buf))
        phase = 0.0
        for i in range(i0, i1):
            t = (i - i0) / sr
            # ビブラート（発音から少し遅れて深くなる）
            vib = math.sin(two_pi * 5.3 * t) * 0.30 * min(1.0, t / 0.25)
            phase += two_pi * f0 * 2.0 ** (vib / 12.0) / sr
            # エンベロープ: アタック → 緩い減衰 → リリース
            env = min(1.0, t / 0.04) * min(1.0, (dur - t) / 0.10)
            env *= 1.0 - 0.15 * min(1.0, t / dur)
            s = 0.0
            for n, a in harm:
                s += a * math.sin(n * phase)
            buf[i] += s * env * vol


def _render_backing(buf: list[float], chords: list, spb: float, sr: int):
    two_pi = 2.0 * math.pi
    bar = 4.0 * spb
    for bi, (bass_midi, triad) in enumerate(chords):
        t_bar = bi * bar
        # ベース: 1・3 拍目にプラック（基音 + 弱い2倍音）
        fb = _midi_hz(bass_midi)
        for beat in (0.0, 2.0):
            t0 = t_bar + beat * spb
            i0, i1 = int(t0 * sr), min(int((t0 + 2.0 * spb) * sr), len(buf))
            for i in range(i0, i1):
                t = (i - i0) / sr
                env = math.exp(-t / 0.40) * min(1.0, t / 0.01)
                ph = two_pi * fb * t
                buf[i] += (math.sin(ph) + 0.30 * math.sin(2.0 * ph)) * env * 0.20
        # パッド: 三和音をバー全体に薄く敷く
        i0, i1 = int(t_bar * sr), min(int((t_bar + bar) * sr), len(buf))
        freqs = [_midi_hz(m) for m in triad]
        for i in range(i0, i1):
            t = (i - i0) / sr
            env = min(1.0, t / 0.30) * min(1.0, (bar - t) / 0.30)
            s = sum(math.sin(two_pi * f * t) for f in freqs)
            buf[i] += s * env * 0.045
        # オフビートのプラック（コードの第5音、1オクターブ上）
        fp = _midi_hz(triad[2] + 12)
        for beat in (1.5, 3.5):
            t0 = t_bar + beat * spb
            i0, i1 = int(t0 * sr), min(int((t0 + 0.5 * spb) * sr), len(buf))
            for i in range(i0, i1):
                t = (i - i0) / sr
                env = math.exp(-t / 0.12)
                ph = two_pi * fp * t
                buf[i] += (math.sin(ph) + 0.5 * math.sin(2.0 * ph)) * env * 0.055


def render_song(
    path: str,
    melody: list | None = None,
    chords: list | None = None,
    bpm: float | None = None,
    sample_rate: int = SAMPLE_RATE,
) -> float:
    """歌を合成して WAV（16bit PCM mono）に書き出す。曲長（秒）を返す。"""
    if melody is None or chords is None or bpm is None:
        d_mel, d_cho, d_bpm = default_song()
        melody = melody if melody is not None else d_mel
        chords = chords if chords is not None else d_cho
        bpm = bpm if bpm is not None else d_bpm

    spb = 60.0 / bpm
    end_beat = max(s + d for s, d, _, _ in melody)
    end_beat = max(end_beat, 4.0 * len(chords))
    duration = end_beat * spb + TAIL_SEC
    buf = [0.0] * int(duration * sample_rate)

    _render_voice(buf, melody, spb, sample_rate)
    _render_backing(buf, chords, spb, sample_rate)

    peak = max(1e-6, max(abs(s) for s in buf))
    scale = 0.85 / peak
    pcm = struct.pack(
        f"<{len(buf)}h",
        *(int(max(-1.0, min(1.0, s * scale)) * 32767) for s in buf),
    )
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(sample_rate)
        w.writeframes(pcm)
    return duration


def lipsync_entries(
    melody: list | None = None,
    bpm: float | None = None,
    fps: float = 30.0,
) -> list[tuple[float, str, float]]:
    """音符列から (time, vowel, weight) のタイムラインを作る。

    LipSyncTimeline のエントリ形式と同一。波形解析より正確で、
    合成した歌と完全に同期する。
    """
    if melody is None or bpm is None:
        d_mel, _, d_bpm = default_song()
        melody = melody if melody is not None else d_mel
        bpm = bpm if bpm is not None else d_bpm

    spb = 60.0 / bpm
    notes = sorted(melody)
    end = max(s + d for s, d, _, _ in notes) * spb + TAIL_SEC
    entries: list[tuple[float, str, float]] = []
    total = int(end * fps)
    ni = 0
    for fi in range(total):
        t = fi / fps
        while ni < len(notes) and (notes[ni][0] + notes[ni][1]) * spb <= t:
            ni += 1
        if ni < len(notes) and notes[ni][0] * spb <= t:
            s_b, d_b, _, vowel = notes[ni]
            pos = (t - s_b * spb) / max(1e-6, d_b * spb)
            weight = 0.9 * math.sin(math.pi * min(1.0, pos)) ** 0.6
            entries.append((t, vowel, weight))
        else:
            entries.append((t, "aa", 0.0))
    return entries


def generate_song(path: str | None = None) -> tuple[str, list, float]:
    """内蔵ソングを合成する 1 行 API。

    Returns:
        (wav_path, lipsync_entries, duration_sec)

    Example::
        path, entries, duration = generate_song()
        timeline = LipSyncTimeline(entries, duration)
    """
    if path is None:
        path = os.path.join(tempfile.gettempdir(), "kagra_builtin_song.wav")
    duration = render_song(path)
    entries = lipsync_entries()
    return path, entries, duration

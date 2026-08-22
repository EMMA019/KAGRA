"""歌声シンセ（kagra/song.py）— GPU / Rust 拡張不要。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule


def test_generate_song_writes_valid_wav(tmp_path):
    song = load_kagra_submodule("song")
    lips = load_kagra_submodule("vrm_lipsync")

    out = str(tmp_path / "song.wav")
    path, entries, duration = song.generate_song(out)
    assert path == out

    samples, sr = lips._load_wav_samples(path)
    assert sr == song.SAMPLE_RATE
    assert duration > 8.0
    assert abs(len(samples) / sr - duration) < 0.2

    # 無音でもクリップ寸前でもない
    peak = max(abs(s) for s in samples)
    assert 0.5 < peak <= 1.0
    rms = (sum(s * s for s in samples) / len(samples)) ** 0.5
    assert rms > 0.02


def test_lipsync_entries_match_melody(tmp_path):
    song = load_kagra_submodule("song")
    entries = song.lipsync_entries()

    assert entries, "タイムラインが空"
    times = [t for t, _, _ in entries]
    assert times == sorted(times)
    assert all(v in song.VOWELS for _, v, _ in entries)
    assert all(0.0 <= w <= 1.0 for _, _, w in entries)

    # 歌っている時間帯は口が開き、末尾（TAIL）は閉じる
    assert any(w > 0.5 for _, _, w in entries)
    assert entries[-1][2] == 0.0

    # 音符の中央では対応する母音が開いている
    melody, _, bpm = song.default_song()
    spb = 60.0 / bpm
    start_b, dur_b, _, vowel = melody[2]  # bar1 の長め音符
    mid_t = (start_b + dur_b / 2) * spb
    near = min(entries, key=lambda e: abs(e[0] - mid_t))
    assert near[1] == vowel
    assert near[2] > 0.5


def test_render_song_duration_covers_chords(tmp_path):
    song = load_kagra_submodule("song")
    melody, chords, bpm = song.default_song()
    out = str(tmp_path / "song2.wav")
    duration = song.render_song(out, melody, chords, bpm)
    # 8小節 × 4拍 + テール
    expected = 32.0 * 60.0 / bpm + song.TAIL_SEC
    assert abs(duration - expected) < 1e-6

"""リップシンク母音推定 / タイムライン補間（GPU 不要）。"""
from __future__ import annotations

import math

from tests.conftest import load_kagra_submodule

ls = load_kagra_submodule("vrm_lipsync")


def _tone(freq: float, sr: int = 16000, n: int = 512) -> list[float]:
    return [math.sin(2.0 * math.pi * freq * i / sr) for i in range(n)]


def test_estimate_vowel_low_formant_is_aa_or_oh():
    v = ls.estimate_vowel(_tone(730.0), 16000)
    assert v in ("aa", "oh", "ou")


def test_estimate_vowel_high_f2_is_ih_or_ee():
    v = ls.estimate_vowel(_tone(2290.0), 16000)
    assert v in ("ih", "ee")


def test_sample_timeline_lerps_same_vowel():
    entries = [(0.0, "aa", 0.0), (1.0, "aa", 1.0)]
    mid = ls.sample_timeline(entries, 0.5)
    assert mid is not None
    assert abs(mid["aa"] - 0.5) < 1e-6


def test_sample_timeline_crossfades():
    entries = [(0.0, "aa", 1.0), (1.0, "ih", 1.0)]
    mid = ls.sample_timeline(entries, 0.5)
    assert mid is not None
    assert abs(mid["aa"] - 0.5) < 1e-6
    assert abs(mid["ih"] - 0.5) < 1e-6


def test_sample_timeline_past_end_is_none():
    entries = [(0.0, "aa", 0.8)]
    assert ls.sample_timeline(entries, 1.0) is None

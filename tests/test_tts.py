"""TTS（kagra.tts）の純ロジックテスト。ネットワーク不要（モーラ解析のみ）。

VOICEVOX の audio_query JSON を直接渡して、モーラタイミングが正しく
積み上がるか検証する。ネットワーク関数（tts_ping / tts_speak）は呼ばない。
"""
from tests.conftest import load_kagra_submodule

tts = load_kagra_submodule("tts")


def test_parse_moras_timings():
    query = {
        "accent_phrases": [
            {
                "moras": [
                    {"text": "こ", "consonant": "k", "consonant_length": 0.05,
                     "vowel": "o", "vowel_length": 0.12},
                    {"text": "ん", "vowel": "n", "vowel_length": 0.08},
                ],
                "pause_mora": [{"text": "、", "vowel_length": 0.2}],
            },
            {
                "moras": [
                    {"text": "に", "consonant": "n", "consonant_length": 0.04,
                     "vowel": "i", "vowel_length": 0.1},
                ],
            },
        ]
    }
    moras = tts.parse_moras(query)
    assert moras[0] == ("o", 0.05, 0.17)      # k(0.05) → o(0.12)
    assert moras[1] == ("n", 0.17, 0.25)      # ん（母音なし扱いでそのまま）
    assert moras[2] == ("i", 0.49, 0.59)      # 0.25 + pause 0.2 + n(0.04) → 0.49
    assert len(moras) == 3


def test_parse_moras_empty_and_vowel_case():
    assert tts.parse_moras({}) == []
    query = {"accent_phrases": [{"moras": [{"vowel": "A", "vowel_length": 0.1}]}]}
    assert tts.parse_moras(query) == [("a", 0.0, 0.1)], "母音は小文字正規化"


def test_vowel_to_expression_mapping():
    # VRM 表情プリセットへ写る（shared の walker.expression と一致）
    assert tts.VOWEL_TO_EXPRESSION["a"] == "aa"
    assert tts.VOWEL_TO_EXPRESSION["i"] == "ih"
    assert tts.VOWEL_TO_EXPRESSION["u"] == "ou"
    assert tts.VOWEL_TO_EXPRESSION["e"] == "ee"
    assert tts.VOWEL_TO_EXPRESSION["o"] == "oh"

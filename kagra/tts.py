"""TTS（VOICEVOX）— 音素タイミング同期。エンジンは同梱しない。

Phase 0-③。0.19 の voicevox の mora 長を、shared の walker.expression
（aa / ih / ou / ee / oh）に繋げるための純 Python モジュール。

使い方::

    from kagra.tts import tts_ping, tts_speak, tts_lipsync_timeline

    if tts_ping():
        wav, moras = tts_speak("こんにちは")   # WAV bytes + [(母音, 開始, 終了)]
        play_wav(wav)
        # moras のタイミングで walker.expression を切り替える（リップシンク）

VOICEVOX を起動してから（https://voicevox.hiroshiba.jp/）::

    # 既定 http://localhost:50021、speaker=3
"""
from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request

DEFAULT_URL = "http://localhost:50021"
DEFAULT_SPEAKER = 3

# VRM 表情プリセット: 母音 → expression 名（walkerk.expression）
VOWEL_TO_EXPRESSION = {
    "a": "aa",
    "i": "ih",
    "u": "ou",
    "e": "ee",
    "o": "oh",
    "n": "blink",  # ん（口を閉じる）
}


class TtsError(RuntimeError):
    """VOICEVOX に届かない / 合成に失敗した。"""


def tts_ping(url: str = DEFAULT_URL, timeout: float = 2.0) -> bool:
    """エンジンが応答するか。未起動なら False（同梱しない）。"""
    try:
        with urllib.request.urlopen(
            f"{url.rstrip('/')}/version", timeout=timeout
        ) as r:
            return 200 <= r.status < 300
    except Exception:
        return False


def _audio_query(text: str, speaker: int, url: str, timeout: float) -> dict:
    text = str(text or "").strip()
    if not text:
        raise TtsError("空のテキストは合成できない")
    base = url.rstrip("/")
    encoded = urllib.parse.quote(text)
    query_url = f"{base}/audio_query?text={encoded}&speaker={int(speaker)}"
    try:
        with urllib.request.urlopen(query_url, timeout=timeout) as r:
            return json.loads(r.read().decode("utf-8"))
    except urllib.error.URLError as e:
        raise TtsError(
            f"VOICEVOX に接続できない ({base})。アプリを起動してください。{e}"
        ) from e


def parse_moras(query: dict) -> list[tuple[str, float, float]]:
    """audio_query JSON → ``[(母音, 開始秒, 終了秒)]``。

    各 accent_phrase の moras（子音 + 母音）を順に積み上げてタイミングを出す。
    モーラの開始は子音の開始、母音の開始は子音の後（リップの開くタイミング）。
    """
    out: list[tuple[str, float, float]] = []
    t = 0.0
    for phrase in query.get("accent_phrases", []):
        for mora in phrase.get("moras", []):
            cons_len = float(mora.get("consonant_length") or 0.0)
            vowel = (mora.get("vowel") or "").strip().lower()
            vowel_len = float(mora.get("vowel_length") or 0.0)
            start = t + cons_len
            end = start + vowel_len
            if vowel:
                out.append((vowel, round(start, 3), round(end, 3)))
            t += cons_len + vowel_len
        # 句間の休止（pause_mora があれば）も進める
        for p in phrase.get("pause_mora", []):
            t += float(p.get("vowel_length") or 0.0)
    return out


def tts_lipsync_timeline(
    text: str, speaker: int = DEFAULT_SPEAKER, url: str = DEFAULT_URL
) -> list[tuple[str, float, float]]:
    """テキスト → 母音タイミング（VOICEVOX の audio_query から）。"""
    return parse_moras(_audio_query(text, speaker, url, timeout=10.0))


def tts_speak(
    text: str,
    speaker: int = DEFAULT_SPEAKER,
    url: str = DEFAULT_URL,
) -> tuple[bytes, list[tuple[str, float, float]]]:
    """合成して ``(WAV bytes, 母音タイミング)`` を返す。

    VOICEVOX 未起動は TtsError。WAV は winsound 等で再生できる。
    """
    query = _audio_query(text, speaker, url, timeout=30.0)
    moras = parse_moras(query)
    base = url.rstrip("/")
    synth_url = f"{base}/synthesis?speaker={int(speaker)}"
    req = urllib.request.Request(
        synth_url,
        data=json.dumps(query).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=60.0) as r:
            return r.read(), moras
    except urllib.error.URLError as e:
        raise TtsError(f"VOICEVOX 合成に失敗した: {e}") from e

"""VOICEVOX / COEIROINK HTTP。エンジンは同梱しない。

VOICEVOX を別途起動してから::

    pip install kagra
    # VOICEVOX 本体は https://voicevox.hiroshiba.jp/
    av.speak_voicevox("こんにちは")

``audio_query`` の mora 長をリップシンクに渡す。WAV フォルマントはフォールバック。
"""
from __future__ import annotations

import json
import tempfile
import urllib.error
import urllib.parse
import urllib.request
from typing import Optional

DEFAULT_URL = "http://localhost:50021"


class VoicevoxError(RuntimeError):
    """VOICEVOX に届かない / 合成に失敗した。"""


def ping(url: str = DEFAULT_URL, timeout: float = 2.0) -> bool:
    """エンジンが応答するか。同梱はしないので、未起動なら False。"""
    try:
        with urllib.request.urlopen(f"{url.rstrip('/')}/version", timeout=timeout) as r:
            return 200 <= r.status < 300
    except Exception:
        return False


def synthesize(
    text: str,
    *,
    speaker: int = 3,
    url: str = DEFAULT_URL,
    timeout: float = 30.0,
) -> tuple[str, Optional[dict]]:
    """同期合成。``(wav_path, audio_query dict|None)``。

    VOICEVOX は入れない。未起動なら VoicevoxError。
    """
    text = str(text or "").strip()
    if not text:
        raise VoicevoxError("空のテキストは合成できない")
    base = url.rstrip("/")
    encoded = urllib.parse.quote(text)
    query_url = f"{base}/audio_query?text={encoded}&speaker={int(speaker)}"
    try:
        with urllib.request.urlopen(query_url, timeout=min(10.0, timeout)) as r:
            query_bytes = r.read()
    except urllib.error.URLError as e:
        raise VoicevoxError(
            f"VOICEVOX に接続できない ({base})。アプリを起動してください。{e}"
        ) from e
    query = None
    try:
        query = json.loads(query_bytes)
    except json.JSONDecodeError:
        query = None
    synth_url = f"{base}/synthesis?speaker={int(speaker)}"
    req = urllib.request.Request(
        synth_url,
        data=query_bytes,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            wav_bytes = r.read()
    except urllib.error.URLError as e:
        raise VoicevoxError(f"VOICEVOX 合成に失敗: {e}") from e
    tmp = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
    tmp.write(wav_bytes)
    tmp.close()
    return tmp.name, query


def apply_lipsync(avatar, wav_path: str, query: Optional[dict]):
    """mora タイムライン優先。query が無ければ WAV フォルマント。"""
    if avatar is None:
        return
    lipsync = getattr(avatar, "lipsync", None) or getattr(avatar, "_lipsync", None)
    if lipsync is None and hasattr(avatar, "enable_lipsync"):
        avatar.enable_lipsync()
        lipsync = getattr(avatar, "lipsync", None) or getattr(avatar, "_lipsync", None)
    if lipsync is None:
        return
    if query:
        lipsync.play_audio_query(query)
        return
    if hasattr(avatar, "lipsync_wav"):
        avatar.lipsync_wav(wav_path)


def speak(
    avatar,
    text: str,
    *,
    speaker: int = 3,
    url: str = DEFAULT_URL,
    play: bool = True,
    timeout: float = 30.0,
) -> str:
    """合成して口を動かし、必要なら ``kagra.se`` で再生。WAV パスを返す。"""
    wav, query = synthesize(text, speaker=speaker, url=url, timeout=timeout)
    apply_lipsync(avatar, wav, query)
    if play:
        import kagra

        kagra.se(wav)
    return wav

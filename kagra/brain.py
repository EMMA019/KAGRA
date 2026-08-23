"""外部チャットバックエンドを VRM の頭脳にする薄いクライアント。

LLM をエンジンに入れない方針は変えない。ここにあるのは HTTP で
「テキスト → テキスト」を受け取る部品だけで、API キーも扱わない。

- ``KairiBrain`` — `kairi <https://github.com/EMMA019/kairi>`_
  （接地レイヤー付きローカル BYOK チャット、FastAPI + SSE）の ``/chat``。
  幻覚対策済みの返答が来るので、無人配信の頭脳に向く。
- OpenAI 互換 API は ``AiCharacter.set_llm_func`` に自前関数を渡す
  （``docs/recipes/ai-brain.md``）。

Example::

    from kagra.ai_character import AiCharacter
    from kagra.brain import KairiBrain

    brain = KairiBrain()                  # http://127.0.0.1:8000
    char = AiCharacter("me.vrm", tts="voicevox")
    char.set_llm_func(brain.ask)
    char.chat("市場どうだった？")          # → 接地済みの返答を喋る

SSE パースは純 Python（テストは GPU もサーバーも不要）。
"""
from __future__ import annotations

import json
import urllib.error
import urllib.request
import uuid
from typing import Iterable, Optional


class BrainError(RuntimeError):
    """頭脳バックエンドに届かない・返答が読めない。"""


def parse_sse_event(line: str) -> Optional[dict]:
    """SSE の 1 行を dict にする。イベント行以外・壊れた JSON は None。"""
    raw = (line or "").strip()
    if not raw.startswith("data:"):
        return None
    payload = raw[len("data:"):].strip()
    if not payload:
        return None
    try:
        obj = json.loads(payload)
    except json.JSONDecodeError:
        return None
    return obj if isinstance(obj, dict) else None


def accumulate_reply(events: Iterable[Optional[dict]]) -> str:
    """kairi の SSE 契約（status → chunk → done）から本文を組み立てる。

    - ``chunk`` の ``content`` を連結
    - ``done`` で打ち切り
    - ``error`` は :class:`BrainError`
    - 未知のイベント種は無視（前方互換）
    """
    parts: list[str] = []
    for ev in events:
        if not ev:
            continue
        kind = str(ev.get("type") or "")
        if kind == "chunk":
            parts.append(str(ev.get("content") or ""))
        elif kind == "done":
            break
        elif kind == "error":
            raise BrainError(str(ev.get("content") or "brain error"))
    return "".join(parts).strip()


class KairiBrain:
    """kairi バックエンド（``POST /chat``、SSE）を同期関数として使う。

    Args:
        url:        kairi のベース URL（``docker compose up`` の既定は 8000）
        mode:       kairi の会話モード（``chat`` / ``task`` / ``stocks`` / ``char``）
        session_id: 会話の継続単位。省略時は起動ごとにランダム
        timeout:    1 応答の最大秒数（検索が走ると長い）

    ``ask()`` はブロッキング。ゲームループから使うときは
    ``AiCharacter.set_llm_func`` に渡せば AiCharacter 側のスレッドで回る。
    """

    def __init__(
        self,
        url: str = "http://127.0.0.1:8000",
        *,
        mode: str = "chat",
        session_id: str | None = None,
        timeout: float = 120.0,
    ):
        self.url = url.rstrip("/")
        self.mode = mode
        self.session_id = session_id or f"kagra-{uuid.uuid4().hex[:12]}"
        self.timeout = float(timeout)

    def ask(self, text: str) -> str:
        """テキストを送り、SSE を最後まで読んで本文を返す。"""
        message = (text or "").strip()
        if not message:
            return ""
        payload = json.dumps(
            {
                "message": message,
                "session_id": self.session_id,
                "mode": self.mode,
            },
            ensure_ascii=False,
        ).encode("utf-8")
        req = urllib.request.Request(
            f"{self.url}/chat",
            data=payload,
            headers={
                "Content-Type": "application/json",
                "Accept": "text/event-stream",
            },
        )
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                reply = accumulate_reply(
                    parse_sse_event(raw.decode("utf-8", "replace"))
                    for raw in resp
                )
        except urllib.error.URLError as e:
            raise BrainError(
                f"kairi に接続できません（{self.url}）。"
                " `docker compose up` か `uvicorn app.main:app` で起動してください。"
                f" 詳細: {e}"
            ) from e
        return reply

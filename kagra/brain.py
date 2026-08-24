"""LLM 頭脳の公式面。モデルは wheel に入れない。

kairi（ローカル BYOK サーバー）/ OpenAI 互換（Ollama 含む）を ``ask(text) -> str`` で切替。
``AiCharacter.set_llm_func(brain.ask)`` にそのまま渡せる。
"""
from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from typing import Any, Callable
from uuid import uuid4


class BrainError(RuntimeError):
    """頭脳サーバー / API が答えを返せなかった。"""


def parse_kairi_sse(body: str) -> str:
    """kairi ``POST /api/chat`` の SSE から本文を取り出す。

    ``chunk`` は増分。``done.content`` があればそれを優先（無いときは chunk を結合）。
    demo モードは ``done`` に content が無い。
    """
    chunks: list[str] = []
    done = ""
    err = ""
    for block in body.replace("\r\n", "\n").split("\n\n"):
        data_lines = [
            line[6:] for line in block.split("\n") if line.startswith("data: ")
        ]
        if not data_lines:
            continue
        raw = "\n".join(data_lines)
        try:
            ev = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if not isinstance(ev, dict):
            continue
        kind = str(ev.get("type") or "")
        if kind == "chunk":
            c = ev.get("content")
            if isinstance(c, str) and c:
                chunks.append(c)
        elif kind == "done":
            c = ev.get("content")
            if isinstance(c, str) and c:
                done = c
        elif kind == "error":
            err = str(ev.get("message") or ev.get("detail") or "kairi error")
    if err:
        raise BrainError(err)
    text = done.strip() if done.strip() else "".join(chunks).strip()
    if not text:
        raise BrainError("kairi returned an empty reply")
    return text


def _http_post(url: str, payload: dict[str, Any], headers: dict[str, str], timeout: float) -> bytes:
    data = json.dumps(payload).encode("utf-8")
    hdrs = {"Content-Type": "application/json", "Accept": "text/event-stream", **headers}
    req = urllib.request.Request(url, data=data, headers=hdrs, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=float(timeout)) as resp:
            return resp.read()
    except urllib.error.HTTPError as e:
        detail = e.read().decode("utf-8", errors="replace")[:400]
        raise BrainError(f"HTTP {e.code} from {url}: {detail}") from e
    except urllib.error.URLError as e:
        raise BrainError(f"could not reach {url}: {e.reason}") from e


def _http_get(url: str, headers: dict[str, str], timeout: float) -> bytes:
    req = urllib.request.Request(url, headers=headers, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=float(timeout)) as resp:
            return resp.read()
    except urllib.error.HTTPError as e:
        raise BrainError(f"HTTP {e.code} from {url}") from e
    except urllib.error.URLError as e:
        raise BrainError(f"could not reach {url}: {e.reason}") from e


class Brain:
    """``ask(text) -> str``。中にモデルは持たない。"""

    def ask(self, text: str) -> str:
        raise NotImplementedError

    def ping(self) -> bool:
        return True


class KairiBrain(Brain):
    """ローカル kairi サーバー（https://github.com/EMMA019/kairi）。

    FastAPI / SQLite / グラウンディングは kairi 側。ここは ``/api/chat`` の SSE を読むだけ。
    既定 ``http://127.0.0.1:8000``。トークンは ``KAIRI_API_TOKEN``（未設定なら開発モードで通る）。
    """

    def __init__(
        self,
        url: str | None = None,
        *,
        token: str | None = None,
        session_id: str | None = None,
        mode: str = "chat",
        timeout: float = 120.0,
        post: Callable[..., bytes] | None = None,
        get: Callable[..., bytes] | None = None,
    ):
        self.url = (url or os.environ.get("KAIRI_URL") or "http://127.0.0.1:8000").rstrip("/")
        self.token = token if token is not None else os.environ.get("KAIRI_API_TOKEN", "")
        self.session_id = session_id or os.environ.get("KAIRI_SESSION") or f"kagra-{uuid4().hex[:8]}"
        self.mode = str(mode)
        self.timeout = float(timeout)
        self._post = post or _http_post
        self._get = get or _http_get

    def _headers(self) -> dict[str, str]:
        h: dict[str, str] = {}
        tok = (self.token or "").strip()
        if tok:
            h["Authorization"] = f"Bearer {tok}"
            h["X-API-Token"] = tok
        return h

    def ping(self) -> bool:
        try:
            raw = self._get(f"{self.url}/api/ping", self._headers(), min(self.timeout, 5.0))
            data = json.loads(raw.decode("utf-8", errors="replace"))
            return bool(data.get("alive") or data.get("status") == "ok")
        except Exception:
            return False

    def ask(self, text: str) -> str:
        msg = str(text or "").strip()
        if not msg:
            raise BrainError("empty message")
        raw = self._post(
            f"{self.url}/api/chat",
            {"message": msg, "session_id": self.session_id, "mode": self.mode},
            self._headers(),
            self.timeout,
        )
        return parse_kairi_sse(raw.decode("utf-8", errors="replace"))


class OpenAIBrain(Brain):
    """OpenAI 互換 ``/v1/chat/completions``（OpenAI / Groq / Ollama の v1）。"""

    def __init__(
        self,
        *,
        base_url: str | None = None,
        api_key: str | None = None,
        model: str | None = None,
        system: str = "You are a helpful assistant. Reply briefly, in the user's language.",
        timeout: float = 60.0,
        post: Callable[..., bytes] | None = None,
    ):
        self.base_url = (base_url or os.environ.get("OPENAI_BASE_URL") or "https://api.openai.com/v1").rstrip("/")
        self.api_key = api_key if api_key is not None else os.environ.get("OPENAI_API_KEY", "")
        self.model = model or os.environ.get("OPENAI_MODEL") or "gpt-4o-mini"
        self.system = system
        self.timeout = float(timeout)
        self._post = post or _http_post
        self._history: list[dict[str, str]] = []

    def ask(self, text: str) -> str:
        msg = str(text or "").strip()
        if not msg:
            raise BrainError("empty message")
        messages = [{"role": "system", "content": self.system}]
        messages.extend(self._history[-10:])
        messages.append({"role": "user", "content": msg})
        headers: dict[str, str] = {}
        key = (self.api_key or "").strip()
        if key:
            headers["Authorization"] = f"Bearer {key}"
        raw = self._post(
            f"{self.base_url}/chat/completions",
            {"model": self.model, "messages": messages, "max_tokens": 256},
            headers,
            self.timeout,
        )
        try:
            data = json.loads(raw.decode("utf-8", errors="replace"))
            reply = data["choices"][0]["message"]["content"] or ""
        except Exception as e:
            raise BrainError(f"OpenAI-compatible reply was not JSON chat: {e}") from e
        reply = str(reply).strip()
        if not reply:
            raise BrainError("OpenAI-compatible API returned an empty reply")
        self._history.append({"role": "user", "content": msg})
        self._history.append({"role": "assistant", "content": reply})
        return reply


def brain(name: str = "kairi", **kwargs) -> Brain:
    """``\"kairi\"`` / ``\"openai\"`` / ``\"ollama\"``。"""
    n = str(name or "kairi").strip().lower()
    if n in ("kairi", "kairi-brain"):
        return KairiBrain(**kwargs)
    if n in ("ollama",):
        kwargs.setdefault(
            "base_url",
            os.environ.get("OLLAMA_URL") or "http://127.0.0.1:11434/v1",
        )
        kwargs.setdefault("model", os.environ.get("OLLAMA_MODEL") or "llama3.2")
        kwargs.setdefault("api_key", os.environ.get("OLLAMA_API_KEY") or "ollama")
        return OpenAIBrain(**kwargs)
    if n in ("openai", "openai-compat", "groq"):
        if n == "groq":
            kwargs.setdefault("base_url", os.environ.get("GROQ_BASE_URL") or "https://api.groq.com/openai/v1")
            kwargs.setdefault("api_key", os.environ.get("GROQ_API_KEY") or "")
            kwargs.setdefault("model", os.environ.get("GROQ_MODEL") or "llama-3.1-8b-instant")
        return OpenAIBrain(**kwargs)
    raise BrainError(f"unknown brain {name!r}; use kairi, openai, or ollama")

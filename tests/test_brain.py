"""kagra.brain — GPU 不要。HTTP は差し替え。"""
from __future__ import annotations

import json

import pytest

from tests.conftest import load_kagra_submodule

brain = load_kagra_submodule("brain")


def test_parse_kairi_sse_joins_demo_chunks():
    body = (
        'data: {"type": "status", "content": "demo"}\n\n'
        'data: {"type": "chunk", "content": "Hel"}\n\n'
        'data: {"type": "chunk", "content": "lo"}\n\n'
        'data: {"type": "done"}\n\n'
    )
    assert brain.parse_kairi_sse(body) == "Hello"


def test_parse_kairi_sse_prefers_done_content():
    body = (
        'data: {"type": "chunk", "content": "partial"}\n\n'
        'data: {"type": "done", "content": "full reply", "ok": true}\n\n'
    )
    assert brain.parse_kairi_sse(body) == "full reply"


def test_parse_kairi_sse_error_raises():
    with pytest.raises(brain.BrainError, match="nope"):
        brain.parse_kairi_sse('data: {"type": "error", "message": "nope"}\n\n')


def test_kairi_brain_ask_posts_chat():
    seen = {}

    def post(url, payload, headers, timeout):
        seen["url"] = url
        seen["payload"] = payload
        seen["headers"] = headers
        body = (
            'data: {"type": "chunk", "content": "hi from kairi"}\n\n'
            'data: {"type": "done"}\n\n'
        )
        return body.encode()

    b = brain.KairiBrain(url="http://127.0.0.1:8000", token="s3cret", session_id="s1", post=post)
    assert b.ask("こんにちは") == "hi from kairi"
    assert seen["url"] == "http://127.0.0.1:8000/api/chat"
    assert seen["payload"]["message"] == "こんにちは"
    assert seen["payload"]["session_id"] == "s1"
    assert seen["headers"]["Authorization"] == "Bearer s3cret"


def test_kairi_brain_ping():
    def get(url, headers, timeout):
        assert url.endswith("/api/ping")
        return json.dumps({"status": "ok", "alive": True}).encode()

    b = brain.KairiBrain(get=get, post=lambda *a: b"")
    assert b.ping() is True


def test_openai_brain_ask():
    def post(url, payload, headers, timeout):
        assert url.endswith("/chat/completions")
        assert payload["messages"][-1]["content"] == "hey"
        return json.dumps({
            "choices": [{"message": {"content": "there"}}],
        }).encode()

    b = brain.OpenAIBrain(base_url="http://127.0.0.1:11434/v1", api_key="ollama", post=post)
    assert b.ask("hey") == "there"


def test_brain_factory_names():
    k = brain.brain("kairi", url="http://example.invalid")
    assert isinstance(k, brain.KairiBrain)
    o = brain.brain("ollama")
    assert isinstance(o, brain.OpenAIBrain)
    assert "11434" in o.base_url
    with pytest.raises(brain.BrainError):
        brain.brain("not-a-brain")


def test_empty_ask_raises():
    b = brain.KairiBrain(post=lambda *a: b"")
    with pytest.raises(brain.BrainError):
        b.ask("  ")

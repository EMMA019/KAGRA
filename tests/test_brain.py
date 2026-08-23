"""kairi SSE クライアントの純ロジック。GPU もサーバーも不要。"""
from __future__ import annotations

import pytest

from tests.conftest import load_kagra_submodule

brain = load_kagra_submodule("brain")


def test_parse_sse_event_ok():
    ev = brain.parse_sse_event('data: {"type":"chunk","content":"やあ"}')
    assert ev == {"type": "chunk", "content": "やあ"}


def test_parse_sse_event_skips_noise():
    assert brain.parse_sse_event("") is None
    assert brain.parse_sse_event(": keep-alive") is None
    assert brain.parse_sse_event("event: message") is None
    assert brain.parse_sse_event("data: {broken") is None
    assert brain.parse_sse_event('data: "just-a-string"') is None


def test_accumulate_reply_joins_chunks_until_done():
    events = [
        {"type": "status", "content": "searching"},
        {"type": "chunk", "content": "市場は"},
        {"type": "chunk", "content": "小幅高。"},
        {"type": "done"},
        {"type": "chunk", "content": "無視される"},
    ]
    assert brain.accumulate_reply(events) == "市場は小幅高。"


def test_accumulate_reply_ignores_unknown_and_none():
    events = [None, {"type": "future_thing"}, {"type": "chunk", "content": "ok"}]
    assert brain.accumulate_reply(events) == "ok"


def test_accumulate_reply_raises_on_error():
    with pytest.raises(brain.BrainError):
        brain.accumulate_reply([{"type": "error", "content": "boom"}])


def test_kairi_brain_defaults():
    b = brain.KairiBrain("http://127.0.0.1:8000/")
    assert b.url == "http://127.0.0.1:8000"
    assert b.mode == "chat"
    assert b.session_id.startswith("kagra-")
    assert b.ask("   ") == ""


def test_kairi_brain_connection_error_hint():
    b = brain.KairiBrain("http://127.0.0.1:9", timeout=0.2)
    with pytest.raises(brain.BrainError) as ei:
        b.ask("hi")
    assert "kairi" in str(ei.value)

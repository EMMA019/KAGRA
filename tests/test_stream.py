"""配信ヘルパ（ChatInbox / HUD / VOICEVOX パース / マイク RMS）。GPU 不要。"""
from __future__ import annotations

from tests.conftest import load_kagra_submodule

stream = load_kagra_submodule("stream")
voicevox = load_kagra_submodule("voicevox")
mic = load_kagra_submodule("mic")


def test_parse_chat_line_ok():
    msg = stream.parse_chat_line('{"user":"alice","text":"hello"}')
    assert msg is not None
    assert msg.user == "alice"
    assert msg.text == "hello"
    assert msg.as_line() == "alice: hello"


def test_parse_chat_line_skips_junk():
    assert stream.parse_chat_line("") is None
    assert stream.parse_chat_line("# comment") is None
    assert stream.parse_chat_line("{nope}") is None
    assert stream.parse_chat_line('{"user":"a"}') is None


def test_chat_inbox_jsonl_tail(tmp_path):
    path = tmp_path / "chat.jsonl"
    inbox = stream.ChatInbox(path)
    path.write_text('{"user":"a","text":"one"}\n', encoding="utf-8")
    first = inbox.poll()
    assert [m.text for m in first] == ["one"]
    with path.open("a", encoding="utf-8") as f:
        f.write('{"name":"b","message":"two"}\n')
    second = inbox.poll()
    assert [m.text for m in second] == ["two"]
    assert inbox.poll() == []


def test_chat_inbox_push_memory():
    inbox = stream.ChatInbox()
    inbox.push("mod", "hi", persist=False)
    got = inbox.poll()
    assert len(got) == 1
    assert got[0].user == "mod"
    assert inbox.poll() == []


def test_stream_hud_caps_chat():
    hud = stream.StreamHud(max_chat=3)
    for i in range(5):
        hud.push_chat(f"m{i}", user="u")
    assert [c.text for c in hud.chat] == ["m2", "m3", "m4"]


def test_stream_hud_default_brand():
    hud = stream.StreamHud(song="♪ demo")
    assert hud.brand == "KAGRA"
    assert hud.song.startswith("♪")


def test_stream_hud_ingest_sets_subtitle():
    inbox = stream.ChatInbox()
    inbox.push("a", "最新", persist=False)
    hud = stream.StreamHud()
    hud.ingest(inbox)
    assert hud.subtitle == "最新"
    assert hud.chat[-1].user == "a"


def test_voicevox_empty_text_errors():
    try:
        voicevox.synthesize("   ")
    except voicevox.VoicevoxError as e:
        assert "空" in str(e)
    else:
        raise AssertionError("expected VoicevoxError")


def test_voicevox_unreachable_errors(monkeypatch):
    def boom(*_a, **_k):
        raise voicevox.urllib.error.URLError("down")

    monkeypatch.setattr(voicevox.urllib.request, "urlopen", boom)
    try:
        voicevox.synthesize("hi", url="http://127.0.0.1:9")
    except voicevox.VoicevoxError as e:
        assert "VOICEVOX" in str(e)
    else:
        raise AssertionError("expected VoicevoxError")


def test_apply_lipsync_prefers_audio_query():
    class FakeLs:
        def __init__(self):
            self.query = None
            self.wav = None

        def play_audio_query(self, q, **_k):
            self.query = q

    class FakeAv:
        def __init__(self):
            self.lipsync = FakeLs()
            self.wav = None

        def lipsync_wav(self, path):
            self.wav = path

    av = FakeAv()
    voicevox.apply_lipsync(av, "x.wav", {"speedScale": 1.0})
    assert av.lipsync.query["speedScale"] == 1.0
    assert av.wav is None


def test_mic_rms_silence_and_peak():
    assert mic.amplitude_from_samples([0.0, 0.0, 0.0]) == 0.0
    loud = mic.amplitude_from_samples([1.0, -1.0], gain=1.0)
    assert abs(loud - 1.0) < 1e-6
    clipped = mic.amplitude_from_samples([1.0], gain=99.0)
    assert clipped == 1.0

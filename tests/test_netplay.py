"""Local versus transport and lockstep.

Frame codec and the relay do not need the Rust extension. The two-client
match shells out to ``examples/versus_online.py`` so this file does not
import ``kagra_shared``.
"""
from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

import pytest

from tests.conftest import load_kagra_submodule

ROOT = Path(__file__).resolve().parents[1]


def _relay():
    path = ROOT / "server" / "netplay_relay.py"
    spec = importlib.util.spec_from_file_location("kagra_netplay_relay_test", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_accept_key_matches_rfc_sample():
    ws = load_kagra_submodule("wsproto")
    assert ws.accept_key("dGhlIHNhbXBsZSBub25jZQ==") == "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="


def test_text_frames_roundtrip_and_wait_for_a_full_header():
    ws = load_kagra_submodule("wsproto")
    buf = bytearray()
    assert ws.try_pop_frame(buf) is None
    raw = ws.encode_text("hello", mask=True)
    buf.extend(raw[:1])
    assert ws.try_pop_frame(buf) is None
    buf.extend(raw[1:])
    opcode, payload = ws.try_pop_frame(buf)
    assert opcode == 1
    assert payload == b"hello"
    assert buf == bytearray()
    plain = bytearray(ws.encode_text('{"op":"start"}', mask=False))
    opcode, payload = ws.try_pop_frame(plain)
    assert payload == b'{"op":"start"}'


def test_screen_halves_split_two_pointers():
    net = load_kagra_submodule("netplay")
    pads = net.half_pads(960, 540)
    assert pads[0].stick[0] < 480 < pads[1].stick[0]
    cx, cy, _r = pads[0].stick
    ax, ay, aw, ah = pads[1].attack
    p1, p2 = net.halves_input(
        960,
        540,
        [
            {"x": cx + 20, "y": cy, "phase": "move"},
            {"x": ax + aw * 0.5, "y": ay + ah * 0.5, "phase": "move"},
            {"x": 10, "y": 10, "phase": "end"},
        ],
    )
    assert p1[0] > 0.2
    assert p1[2] is False
    assert p2[2] is True
    assert p2[0] == 0.0
    left = net.scripted_stick(0, 0)
    right = net.scripted_stick(1, 0)
    assert left[0] == 1.0 and right[0] == -1.0


def _read_until(conn, op: str, limit: int = 6) -> dict:
    for _ in range(limit):
        text = conn.recv_text(2.0)
        assert text, "expected a frame"
        msg = json.loads(text)
        if msg.get("op") == op:
            return msg
    raise AssertionError(op)


def test_relay_pairs_two_sockets_and_stamps_the_slot():
    ws = load_kagra_submodule("wsproto")
    relay = _relay()
    listener, port = relay.serve_background("127.0.0.1", 0)
    try:
        first = ws.connect("127.0.0.1", port, timeout=2.0)
        second = ws.connect("127.0.0.1", port, timeout=2.0)
        hello = {"op": "hello", "proto": 1, "room": "ring", "name": "a"}
        first.send_text(json.dumps(hello))
        hello["name"] = "b"
        second.send_text(json.dumps(hello))
        welcome_a = _read_until(first, "welcome")
        welcome_b = _read_until(second, "welcome")
        assert {welcome_a["slot"], welcome_b["slot"]} == {0, 1}
        assert _read_until(first, "start")["dt_den"] == 60
        assert _read_until(second, "start")["dt_num"] == 1
        owner = first if welcome_a["slot"] == 0 else second
        peer = second if owner is first else first
        expected_slot = welcome_a["slot"] if owner is first else welcome_b["slot"]
        owner.send_text(
            json.dumps(
                {
                    "op": "input",
                    "input": {
                        "slot": 9,
                        "tick": 4,
                        "lx": 1000,
                        "lz": 0,
                        "jump": False,
                        "attack": True,
                        "dodge": False,
                    },
                }
            )
        )
        forwarded = _read_until(peer, "input")
        assert forwarded["input"]["slot"] == expected_slot
        assert forwarded["input"]["tick"] == 4
        assert forwarded["input"]["lx"] == 1000
        assert forwarded["input"]["attack"] is True
    finally:
        listener.close()


def test_two_headless_clients_match_for_n_ticks():
    probe = subprocess.run(
        [
            sys.executable,
            "-c",
            "import importlib\n"
            "ok=False\n"
            "\nfor name in ('kagra.kagra_shared','kagra_shared'):\n"
            "    try:\n"
            "        importlib.import_module(name)\n"
            "        ok=True\n"
            "    except Exception:\n"
            "        pass\n"
            "raise SystemExit(0 if ok else 1)\n",
        ],
        capture_output=True,
        text=True,
    )
    if probe.returncode != 0:
        pytest.skip("kagra_shared not built")
    proc = subprocess.run(
        [sys.executable, str(ROOT / "examples" / "versus_online.py"), "--match", "--ticks", "90"],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        timeout=45,
    )
    if proc.returncode == 2:
        pytest.skip("kagra_shared not built")
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "MATCH ok" in proc.stdout

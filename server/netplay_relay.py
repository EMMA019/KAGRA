#!/usr/bin/env python3
"""Local room relay for KAGRA input sync.

Two clients join a room. The server assigns player slots, then forwards
each `input` message to the other client. It does not simulate the match.
Rooms live in memory. Nothing here is deployed.

    python server/netplay_relay.py --host 127.0.0.1 --port 8765

Free hosting notes (do not turn this process on in CI): see docs/netplay.md.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import socket
import sys
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

_ROOM_OK = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_")


def _wsproto():
    path = ROOT / "kagra" / "wsproto.py"
    spec = importlib.util.spec_from_file_location("kagra_wsproto", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"missing {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


WS = _wsproto()


def _dumps(obj: dict) -> str:
    return json.dumps(obj, separators=(",", ":"))


def _as_int(value, default: int = 0) -> int:
    if isinstance(value, bool) or value is None:
        return default
    if isinstance(value, int):
        return value
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return default


class Client:
    def __init__(self, conn) -> None:
        self.conn = conn
        self.slot: int | None = None
        self.room: "Room | None" = None
        self.lock = threading.Lock()

    def send(self, text: str) -> None:
        with self.lock:
            self.conn.send_text(text)


class Room:
    def __init__(self, name: str) -> None:
        self.name = name
        self.slots: dict[int, Client] = {}
        self.lock = threading.Lock()

    def join(self, client: Client) -> int | None:
        with self.lock:
            if len(self.slots) >= 2:
                return None
            slot = 0 if 0 not in self.slots else 1
            self.slots[slot] = client
            peers = len(self.slots)
            members = list(self.slots.values()) if peers >= 2 else []
        client.slot = slot
        client.room = self
        client.send(
            _dumps(
                {
                    "op": "welcome",
                    "slot": slot,
                    "room": self.name,
                    "seed": 1,
                    "peers": peers,
                }
            )
        )
        if members:
            start = _dumps({"op": "start", "tick0": 0, "dt_num": 1, "dt_den": 60})
            for member in members:
                member.send(start)
        return slot

    def relay(self, sender: Client, text: str) -> None:
        with self.lock:
            others = [c for c in self.slots.values() if c is not sender]
        for other in others:
            other.send(text)

    def leave(self, client: Client) -> None:
        with self.lock:
            self.slots = {slot: c for slot, c in self.slots.items() if c is not client}
            others = list(self.slots.values())
        note = _dumps({"op": "bye", "reason": "peer left"})
        for other in others:
            try:
                other.send(note)
            except OSError:
                pass


class Relay:
    def __init__(self) -> None:
        self.rooms: dict[str, Room] = {}
        self.lock = threading.Lock()

    def room(self, name: str) -> Room:
        with self.lock:
            found = self.rooms.get(name)
            if found is None:
                found = Room(name)
                self.rooms[name] = found
            return found


def _handle(client: Client, text: str, relay: Relay) -> None:
    try:
        msg = json.loads(text)
    except json.JSONDecodeError:
        client.send(_dumps({"op": "bye", "reason": "json"}))
        return
    if not isinstance(msg, dict):
        client.send(_dumps({"op": "bye", "reason": "json"}))
        return
    op = msg.get("op")
    if op == "hello":
        if client.slot is not None:
            return
        if _as_int(msg.get("proto"), 0) != 1:
            client.send(_dumps({"op": "bye", "reason": "proto"}))
            return
        room_name = str(msg.get("room") or "")[:32]
        if not room_name or any(ch not in _ROOM_OK for ch in room_name):
            client.send(_dumps({"op": "bye", "reason": "room"}))
            return
        if relay.room(room_name).join(client) is None:
            client.send(_dumps({"op": "bye", "reason": "room full"}))
        return
    if client.slot is None or client.room is None:
        return
    if op == "input":
        raw = msg.get("input")
        if not isinstance(raw, dict):
            return
        stamped = {
            "slot": int(client.slot),
            "tick": _as_int(raw.get("tick")),
            "lx": _as_int(raw.get("lx")),
            "lz": _as_int(raw.get("lz")),
            "jump": bool(raw.get("jump")),
            "attack": bool(raw.get("attack")),
            "dodge": bool(raw.get("dodge")),
        }
        client.room.relay(client, _dumps({"op": "input", "input": stamped}))
        return
    if op == "bye":
        client.room.relay(client, _dumps({"op": "bye", "reason": str(msg.get("reason") or "bye")[:64]}))


def _serve_client(sock: socket.socket, relay: Relay) -> None:
    try:
        if not WS.server_handshake(sock):
            return
        client = Client(WS.WsConn(sock, client=False))
        while True:
            text = client.conn.recv_text(None)
            if text is None:
                continue
            _handle(client, text, relay)
    except (EOFError, ConnectionError, OSError, ValueError):
        pass
    finally:
        try:
            if "client" in locals() and client.room is not None:
                client.room.leave(client)
        except Exception:
            pass
        try:
            sock.close()
        except OSError:
            pass


def serve_background(host: str = "127.0.0.1", port: int = 0) -> tuple[socket.socket, int]:
    """Bind and accept in a daemon thread. Returns ``(listener, port)``."""
    listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    listener.bind((host, int(port)))
    listener.listen(8)
    actual = int(listener.getsockname()[1])
    relay = Relay()

    def _accept() -> None:
        while True:
            try:
                conn, _addr = listener.accept()
            except OSError:
                return
            try:
                conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            except OSError:
                pass
            threading.Thread(target=_serve_client, args=(conn, relay), daemon=True).start()

    threading.Thread(target=_accept, daemon=True).start()
    return listener, actual


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="KAGRA local netplay relay")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args(argv)
    listener, actual = serve_background(args.host, args.port)
    print(f"LISTENING {args.host} {actual}", flush=True)
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        pass
    finally:
        listener.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())

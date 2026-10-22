"""Minimal WebSocket text frames (RFC 6455). Standard library only.

Client frames are masked. Server frames are not. Messages are one text
frame each; this is the transport under ``kagra.netplay`` and
``server/netplay_relay.py``.
"""
from __future__ import annotations

import base64
import hashlib
import os
import socket
import struct
import time
from typing import Tuple

GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"
_MAX_FRAME = 1_000_000


def accept_key(key: str) -> str:
    digest = hashlib.sha1((key.strip() + GUID).encode("ascii")).digest()
    return base64.b64encode(digest).decode("ascii")


def encode_frame(opcode: int, payload: bytes, *, mask: bool) -> bytes:
    if len(payload) > _MAX_FRAME:
        raise ValueError("frame too large")
    header = bytearray([0x80 | (opcode & 0x0F)])
    mask_bit = 0x80 if mask else 0
    n = len(payload)
    if n < 126:
        header.append(mask_bit | n)
    elif n < 65536:
        header.append(mask_bit | 126)
        header.extend(struct.pack("!H", n))
    else:
        header.append(mask_bit | 127)
        header.extend(struct.pack("!Q", n))
    body = payload
    if mask:
        key = os.urandom(4)
        header.extend(key)
        body = bytes(b ^ key[i % 4] for i, b in enumerate(payload))
    return bytes(header) + body


def encode_text(text: str, *, mask: bool) -> bytes:
    return encode_frame(0x1, text.encode("utf-8"), mask=mask)


def try_pop_frame(buf: bytearray) -> Tuple[int, bytes] | None:
    """Pop one complete frame. ``None`` leaves ``buf`` unchanged."""
    if len(buf) < 2:
        return None
    b1 = buf[1]
    masked = (b1 & 0x80) != 0
    length = b1 & 0x7F
    offset = 2
    if length == 126:
        if len(buf) < 4:
            return None
        length = struct.unpack_from("!H", buf, 2)[0]
        offset = 4
    elif length == 127:
        if len(buf) < 10:
            return None
        length = struct.unpack_from("!Q", buf, 2)[0]
        offset = 10
    if length > _MAX_FRAME:
        raise ValueError("frame too large")
    mask_len = 4 if masked else 0
    total = offset + mask_len + length
    if len(buf) < total:
        return None
    mask = bytes(buf[offset : offset + mask_len]) if masked else b""
    start = offset + mask_len
    payload = bytes(buf[start : start + length])
    opcode = buf[0] & 0x0F
    del buf[:total]
    if masked:
        payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
    return opcode, payload


def _read_headers(sock: socket.socket, limit: int = 8192) -> bytes:
    data = bytearray()
    while b"\r\n\r\n" not in data:
        if len(data) > limit:
            raise ConnectionError("headers too long")
        chunk = sock.recv(1024)
        if not chunk:
            raise ConnectionError("socket closed during handshake")
        data.extend(chunk)
    return bytes(data)


def server_handshake(conn: socket.socket) -> bool:
    raw = _read_headers(conn)
    header, _, _rest = raw.partition(b"\r\n\r\n")
    key = ""
    for line in header.decode("latin1").split("\r\n"):
        if line.lower().startswith("sec-websocket-key:"):
            key = line.split(":", 1)[1].strip()
    if not key:
        return False
    accept = accept_key(key)
    response = (
        "HTTP/1.1 101 Switching Protocols\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Accept: {accept}\r\n"
        "\r\n"
    )
    conn.sendall(response.encode("ascii"))
    return True


def connect(host: str, port: int, timeout: float = 5.0) -> "WsConn":
    deadline = time.monotonic() + timeout
    sock = None
    while True:
        try:
            sock = socket.create_connection((host, port), timeout=timeout)
            break
        except ConnectionRefusedError:
            if time.monotonic() > deadline:
                raise
            time.sleep(0.02)
    assert sock is not None
    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
    key = base64.b64encode(os.urandom(16)).decode("ascii")
    req = (
        f"GET / HTTP/1.1\r\n"
        f"Host: {host}:{port}\r\n"
        "Upgrade: websocket\r\n"
        "Connection: Upgrade\r\n"
        f"Sec-WebSocket-Key: {key}\r\n"
        "Sec-WebSocket-Version: 13\r\n"
        "\r\n"
    )
    sock.sendall(req.encode("ascii"))
    raw = _read_headers(sock)
    status = raw.split(b"\r\n", 1)[0]
    if b" 101 " not in status:
        sock.close()
        raise ConnectionError(status.decode("latin1", "replace"))
    if accept_key(key).encode("ascii") not in raw:
        sock.close()
        raise ConnectionError("websocket accept mismatch")
    return WsConn(sock, client=True)


class WsConn:
    """One TCP socket speaking WebSocket text frames."""

    def __init__(self, sock: socket.socket, *, client: bool) -> None:
        self.sock = sock
        self.client = client
        self._buf = bytearray()

    def send_text(self, text: str) -> None:
        self.sock.sendall(encode_text(text, mask=self.client))

    def recv_text(self, timeout: float | None) -> str | None:
        """Next text payload, or ``None`` when the wait times out.

        A close frame or a dead socket raises ``EOFError``.
        """
        if timeout is None:
            self.sock.settimeout(None)
        elif timeout <= 0:
            self.sock.setblocking(False)
        else:
            self.sock.settimeout(timeout)
        while True:
            frame = try_pop_frame(self._buf)
            if frame is not None:
                opcode, payload = frame
                if opcode == 0x8:
                    raise EOFError("websocket closed")
                if opcode == 0x9:
                    self.sock.sendall(encode_frame(0xA, payload, mask=self.client))
                    continue
                if opcode == 0x1:
                    return payload.decode("utf-8")
                continue
            try:
                chunk = self.sock.recv(4096)
            except (TimeoutError, socket.timeout):
                return None
            except BlockingIOError:
                return None
            if not chunk:
                raise EOFError("socket closed")
            self._buf.extend(chunk)

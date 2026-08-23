"""配信向けの薄い層。コアに YouTube / Twitch / NDI / RTMP は入れない。

- ``StreamHud`` — 字幕・曲名・チャットを既存の 2D テキストで重ねる
- ``ChatInbox`` — JSONL / メモリ。外部スクリプトが YouTube 等を書く
- ``VirtualCam`` — ``kagra[stream]``（pyvirtualcam）。OBS は仮想カメラを選ぶ

GPU readback は 720p 推奨。毎フレーム取るので 1080p は重い。
"""
from __future__ import annotations

import json
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


@dataclass
class ChatMessage:
    user: str
    text: str
    ts: float = field(default_factory=time.time)

    @classmethod
    def from_obj(cls, obj) -> Optional["ChatMessage"]:
        if not isinstance(obj, dict):
            return None
        user = str(obj.get("user") or obj.get("name") or "").strip()
        text = str(obj.get("text") or obj.get("message") or "").strip()
        if not user or not text:
            return None
        ts = obj.get("ts") or obj.get("timestamp")
        try:
            ts_f = float(ts) if ts is not None else time.time()
        except (TypeError, ValueError):
            ts_f = time.time()
        return cls(user=user, text=text, ts=ts_f)

    def as_line(self) -> str:
        return f"{self.user}: {self.text}"


def parse_chat_line(line: str) -> Optional[ChatMessage]:
    """1 行 JSON（``{user, text}``）を ChatMessage にする。空・壊れた行は None。"""
    raw = (line or "").strip()
    if not raw or raw.startswith("#"):
        return None
    try:
        obj = json.loads(raw)
    except json.JSONDecodeError:
        return None
    return ChatMessage.from_obj(obj)


class ChatInbox:
    """チャット受け口。エンジンは配信 API キーを持たない。

    外部が JSONL を追記する::

        echo '{"user":"alice","text":"こんにちは"}' >> kagra-chat.jsonl
        inbox = ChatInbox("kagra-chat.jsonl")
        for msg in inbox.poll():
            hud.push_chat(msg)
    """

    def __init__(self, path: str | Path | None = None):
        self.path = Path(path) if path else None
        self._mem: list[ChatMessage] = []
        self._offset = 0

    def push(self, user: str, text: str, *, persist: bool = True) -> ChatMessage:
        msg = ChatMessage(user=str(user), text=str(text))
        self._mem.append(msg)
        if persist and self.path is not None:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            with self.path.open("a", encoding="utf-8") as f:
                f.write(json.dumps({"user": msg.user, "text": msg.text, "ts": msg.ts}, ensure_ascii=False) + "\n")
            self._offset = self.path.stat().st_size
        return msg

    def poll(self) -> list[ChatMessage]:
        """前回以降の新規メッセージ。メモリ分 + JSONL 追記分。"""
        out = list(self._mem)
        self._mem.clear()
        if self.path is None or not self.path.is_file():
            return out
        try:
            size = self.path.stat().st_size
        except OSError:
            return out
        if size < self._offset:
            self._offset = 0
        if size == self._offset:
            return out
        with self.path.open("r", encoding="utf-8") as f:
            f.seek(self._offset)
            chunk = f.read()
            self._offset = f.tell()
        for line in chunk.splitlines():
            msg = parse_chat_line(line)
            if msg is not None:
                out.append(msg)
        return out


class StreamHud:
    """字幕・曲名・直近チャット。``kagra.font()`` のあと ``draw()`` する。"""

    def __init__(
        self,
        *,
        subtitle: str = "",
        song: str = "",
        credit: str = "",
        max_chat: int = 5,
    ):
        self.subtitle = subtitle
        self.song = song
        self.credit = credit
        self.max_chat = max(1, int(max_chat))
        self.chat: list[ChatMessage] = []

    def push_chat(self, msg: ChatMessage | str, user: str = ""):
        if isinstance(msg, str):
            msg = ChatMessage(user=user or "chat", text=msg)
        self.chat.append(msg)
        if len(self.chat) > self.max_chat:
            self.chat = self.chat[-self.max_chat :]

    def ingest(self, inbox: ChatInbox) -> list[ChatMessage]:
        msgs = inbox.poll()
        for m in msgs:
            self.push_chat(m)
        if msgs and not self.subtitle:
            self.subtitle = msgs[-1].text
        return msgs

    def draw(self, sw: int | None = None, sh: int | None = None):
        import kagra

        if sw is None or sh is None:
            sw, sh = kagra.get_screen_size()
        pad = 16
        if self.song:
            kagra.fill(0, 0, sw, 40, (8, 6, 16), alpha=150)
            kagra.text(self.song, pad, 8, 18, (230, 210, 150))
        if self.credit:
            tw, _ = kagra.measure(self.credit, 12)
            kagra.text(self.credit, max(pad, sw - tw - pad), 10, 12, (160, 150, 170))
        if self.chat:
            y = 52 if self.song else pad
            for msg in self.chat:
                kagra.text(msg.as_line()[:64], pad, y, 14, (200, 200, 210))
                y += 20
        if self.subtitle:
            bar_h = 56
            kagra.fill(0, sh - bar_h, sw, bar_h, (8, 6, 16), alpha=170)
            kagra.text(self.subtitle[:80], pad, sh - 42, 22, (250, 245, 235))


class VirtualCam:
    """``pyvirtualcam`` へ RGB を送る。``pip install "kagra[stream]"``。

    ``update()`` の先頭で ``send()`` する（直前フレーム。1 フレーム遅れ）。
    """

    def __init__(self, *, fps: int = 30, every: int = 1):
        self.fps = int(fps)
        self.every = max(1, int(every))
        self._cam = None
        self._n = 0
        self._wh: tuple[int, int] | None = None

    def start(self, width: int | None = None, height: int | None = None):
        try:
            import pyvirtualcam
        except ImportError as e:
            raise ImportError(
                '仮想カメラには pip install "kagra[stream]" が必要です（pyvirtualcam）。'
            ) from e
        import kagra

        if width is None or height is None:
            width, height = kagra.get_screen_size()
        kagra.set_grab_frames(True)
        self._wh = (int(width), int(height))
        self._cam = pyvirtualcam.Camera(
            width=self._wh[0],
            height=self._wh[1],
            fps=self.fps,
            fmt=pyvirtualcam.PixelFormat.RGB,
        )
        return self

    def send(self) -> bool:
        if self._cam is None:
            return False
        self._n += 1
        if self._n % self.every:
            return False
        import kagra

        frame = kagra.grab_frame()
        if frame is None:
            return False
        w, h, rgb = frame
        if self._wh and (w, h) != self._wh:
            return False
        try:
            import numpy as np
        except ImportError:
            return False
        arr = np.frombuffer(rgb, dtype=np.uint8).reshape(h, w, 3)
        self._cam.send(arr)
        return True

    def close(self):
        import kagra

        kagra.set_grab_frames(False)
        cam = self._cam
        self._cam = None
        if cam is not None:
            cam.close()

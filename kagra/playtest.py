"""「直してここ」— debug / preview playtest handoff.

Same constants and block format as ``kagra_shared::playtest``: zlib (RFC 1950,
level 6) + standard base64, header ``KAGRA-PLAYTEST-1``. No tokens and no
paid services. The GitHub new-issue URL is opened by the host; a screenshot
stays out of the URL.

Windows games attach this from ``kagra.gameloop.run`` when
``KAGRA_PLAYTEST=1`` or the process is frozen. Browser preview and the
Android debug APK use the Rust overlay instead. This module does not import
the Rust extension.
"""
from __future__ import annotations

import base64
import json
import os
import sys
import zlib
from typing import Any, Iterator

ISSUE_NEW = "https://github.com/EMMA019/KAGRA/issues/new"
BLOCK_HEADER = "KAGRA-PLAYTEST-1"
URL_LIMIT = 6000
HOLD_SEC = 0.6
SNAP_EVERY = 1.0
WINDOW_SEC = 10.5
NOTE_MAX = 400
_PASTE_HINT = (
    "状態が URL に入りきりません。画面の KAGRA-PLAYTEST-1 ブロックを"
    "再現データ欄に貼り、スクリーンショットも貼ってください。"
)


def enabled() -> bool:
    """True for a frozen Windows zip, or when ``KAGRA_PLAYTEST`` is on.

    Unset is off. ``0`` / ``false`` / ``no`` / ``off`` stay off, including
    a frozen build that explicitly opts out.
    """
    raw = os.environ.get("KAGRA_PLAYTEST")
    if raw is not None:
        return raw.strip() in {"1", "true", "TRUE", "yes", "YES", "on", "ON"}
    return bool(getattr(sys, "frozen", False))


def layout(width: int, height: int) -> dict[str, Any]:
    """Panel geometry. Keep this in lockstep with ``playtest::layout``."""
    w = float(max(1, int(width)))
    h = float(max(1, int(height)))
    scale = min(w, h) / 720.0
    scale = min(1.6, max(0.55, scale))
    pw = max(120.0, min(440.0 * scale, w - 24.0))
    ph = max(100.0, min(300.0 * scale, h - 24.0))
    x = max(0.0, (w - pw) * 0.5)
    y = max(0.0, (h - ph) * 0.5)
    pad = 14.0 * scale
    title_h = 28.0 * scale
    note_h = 72.0 * scale
    btn_h = 40.0 * scale
    gap = 8.0 * scale
    inner_w = max(40.0, pw - pad * 2.0)
    btn_w = (inner_w - gap * 2.0) / 3.0
    by = y + ph - pad - btn_h
    send = (x + pad, by, btn_w, btn_h)
    copy = (send[0] + btn_w + gap, by, btn_w, btn_h)
    close = (copy[0] + btn_w + gap, by, btn_w, btn_h)
    return {
        "scale": scale,
        "panel": (x, y, pw, ph),
        "note": (x + pad, y + pad + title_h, inner_w, note_h),
        "send": send,
        "copy": copy,
        "close": close,
    }


def _contains(rect: tuple[float, float, float, float], px: float, py: float) -> bool:
    x, y, w, h = rect
    return x <= px <= x + w and y <= py <= y + h


def percent_encode(text: str) -> str:
    out = []
    for byte in text.encode("utf-8"):
        if (
            0x41 <= byte <= 0x5A
            or 0x61 <= byte <= 0x7A
            or 0x30 <= byte <= 0x39
            or byte in (ord("-"), ord("_"), ord("."), ord("~"))
        ):
            out.append(chr(byte))
        else:
            out.append(f"%{byte:02X}")
    return "".join(out)


def _wrap76(text: str) -> str:
    return "\n".join(text[i : i + 76] for i in range(0, len(text), 76))


def encode_block(report: dict[str, Any]) -> str:
    raw = json.dumps(report, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    compressed = zlib.compress(raw, 6)
    body = _wrap76(base64.b64encode(compressed).decode("ascii"))
    return f"{BLOCK_HEADER}\n{body}\n"


def _extract_b64(text: str) -> str | None:
    found = False
    buf: list[str] = []
    for line in text.splitlines():
        line = line.strip().strip("`")
        if not found:
            if line.startswith(BLOCK_HEADER):
                found = True
            continue
        if not line or line.startswith("```"):
            break
        if line and all(c.isalnum() or c in "+/=" for c in line):
            buf.append(line)
        else:
            break
    if not buf:
        return None
    return "".join(buf)


def decode_block(text: str) -> dict[str, Any]:
    b64 = _extract_b64(text)
    if b64:
        try:
            raw = zlib.decompress(base64.b64decode(b64))
            value = json.loads(raw.decode("utf-8"))
        except Exception as exc:
            raise ValueError(str(exc)) from exc
        if not isinstance(value, dict):
            raise ValueError("playtest block is not an object")
        return value
    trimmed = text.strip()
    start = trimmed.find("{")
    end = trimmed.rfind("}")
    if start >= 0 and end > start:
        try:
            value = json.loads(trimmed[start : end + 1])
        except json.JSONDecodeError:
            value = None
        if isinstance(value, dict) and ("v" in value or "dump" in value):
            return value
    raise ValueError("no KAGRA-PLAYTEST block")


def _title_from_note(note: str) -> str:
    line = (note.splitlines() or [""])[0].strip()
    short = "".join(list(line)[:60])
    if not short:
        return "[playtest] 直してここ"
    return f"[playtest] {short}"


def _build_summary(report: dict[str, Any]) -> str:
    b = report.get("build") if isinstance(report.get("build"), dict) else {}
    d = report.get("device") if isinstance(report.get("device"), dict) else {}
    return (
        f"kagra-shared {b.get('kagra_shared', '')} git {b.get('git', '')} "
        f"{b.get('profile', '')} {b.get('os', '')}/{b.get('arch', '')}\n"
        f"{d.get('label', '')} {d.get('width', 0)}x{d.get('height', 0)} "
        f"lang {d.get('lang', '')}"
    )


def _issue_url(title: str, note: str, build: str, repro: str) -> str:
    return (
        f"{ISSUE_NEW}?template=playtest.yml&labels=playtest"
        f"&title={percent_encode(title)}&note={percent_encode(note)}"
        f"&build={percent_encode(build)}&repro={percent_encode(repro)}"
    )


def instruction_text(fits: bool, shot_path: str = "") -> str:
    if shot_path:
        shot = f"スクリーンショットを保存しました ({shot_path})。issue に貼り付けてください。"
    else:
        shot = "スクリーンショットを issue に貼り付けてください。"
    if fits:
        return f"issue の下書きを開きました。{shot}"
    return f"状態が大きいので KAGRA-PLAYTEST-1 ブロックを再現データ欄に貼ってください。{shot}"


def issue_from_report(report: dict[str, Any]) -> dict[str, Any]:
    note = report.get("note") if isinstance(report.get("note"), str) else ""
    title = _title_from_note(note)
    build = _build_summary(report)
    block = encode_block(report)
    full = _issue_url(title, note, build, block)
    if len(full) <= URL_LIMIT:
        url, fits = full, True
    else:
        url, fits = _issue_url(title, note, build, _PASTE_HINT), False
    return {
        "url": url,
        "block": block,
        "fits": fits,
        "copy": block,
        "instruction": instruction_text(fits, ""),
        "title": title,
        "action": "",
    }


def iter_replay(report: dict[str, Any]) -> Iterator[dict[str, Any]]:
    """Walk and confirm events with ``dump_t < t <= captured_t``.

    Pointer and key rows stay in the log for the agent and are not
    simulated. Time gaps are not filled: the long-press that opened the
    panel has no walk events, and inventing ticks would move the character.
    """
    dump_t = float(report.get("dump_t") or 0.0)
    captured = report.get("captured_t")
    cap = float(captured if captured is not None else 0.0)
    for ev in report.get("inputs") or []:
        if not isinstance(ev, dict):
            continue
        try:
            t = float(ev.get("t") or 0.0)
        except (TypeError, ValueError):
            continue
        if t <= dump_t + 1e-4 or t > cap + 1e-3:
            continue
        if ev.get("kind") in {"walk", "confirm"}:
            yield ev


def overlay_hud(
    hud: dict[str, Any] | None,
    rec: Playtest,
    width: int,
    height: int,
) -> dict[str, Any]:
    """A new HUD dict. The game's dict is left alone."""
    base = dict(hud or {})
    quads = []
    for item in base.get("quads") or []:
        quads.append(dict(item) if isinstance(item, dict) else item)
    texts = []
    for item in base.get("texts") or []:
        texts.append(dict(item) if isinstance(item, dict) else item)
    base["quads"] = quads
    base["texts"] = texts
    if not rec.is_open():
        return base
    lay = layout(width, height)
    scale = float(lay["scale"])

    def quad(rect: tuple[float, float, float, float], color: list[int]) -> dict[str, Any]:
        x, y, w, h = rect
        return {"x": x, "y": y, "w": w, "h": h, "color": color}

    quads.append(quad((0.0, 0.0, float(width), float(height)), [8, 10, 16, 150]))
    quads.append(quad(lay["panel"], [28, 32, 44, 238]))
    quads.append(quad(lay["note"], [14, 16, 24, 255]))
    quads.append(quad(lay["send"], [36, 122, 90, 245]))
    quads.append(quad(lay["copy"], [48, 78, 132, 245]))
    quads.append(quad(lay["close"], [112, 58, 58, 245]))
    ink = [236, 232, 220, 255]
    px, py, _pw, _ph = lay["panel"]
    texts.append(
        {
            "text": "直してここ",
            "x": px + 14.0 * scale,
            "y": py + 8.0 * scale,
            "size": 20.0 * scale,
            "color": [232, 176, 84, 255],
        }
    )
    shown = rec.display_note()
    if shown:
        note_text, note_color = shown[-72:], ink
    else:
        note_text, note_color = "メモ（日本語）", [150, 146, 136, 255]
    nx, ny, _nw, _nh = lay["note"]
    texts.append(
        {
            "text": note_text,
            "x": nx + 8.0 * scale,
            "y": ny + 8.0 * scale,
            "size": 16.0 * scale,
            "color": note_color,
        }
    )
    status = rec.display_status()
    texts.append(
        {
            "text": status,
            "x": nx,
            "y": ny + lay["note"][3] + 8.0 * scale,
            "size": 13.0 * scale,
            "color": [210, 206, 196, 255],
        }
    )
    btn = 15.0 * scale
    for label, rect in (("送る", lay["send"]), ("コピー", lay["copy"]), ("閉じる", lay["close"])):
        x, y, w, h = rect
        texts.append(
            {
                "text": label,
                "x": x + w * 0.5,
                "y": y + (h - btn) * 0.35,
                "size": btn,
                "color": ink,
                "align": "center",
            }
        )
    return base


class _Finger:
    def __init__(self, ident: int, x: float, y: float) -> None:
        self.id = ident
        self.x0 = x
        self.y0 = y
        self.x = x
        self.y = y


class Playtest:
    def __init__(self, enabled_flag: bool = True) -> None:
        self.enabled_flag = bool(enabled_flag)
        self.open = False
        self.holding = False
        self.hold_t = 0.0
        self.clock = 0.0
        self.capture_t = 0.0
        self.last_f12 = -1.0
        self.view_w = 1280
        self.view_h = 720
        self.note = ""
        self.preedit = ""
        self.status = ""
        self.device_label = ""
        self.lang = ""
        self.shot = False
        self.shot_path = ""
        self.packet_seq = 0
        self.packet_action = ""
        self._packet: dict[str, Any] | None = None
        self.fingers: list[_Finger] = []
        self.snaps: list[tuple[float, str]] = []
        self.events: list[dict[str, Any]] = []
        self.seq = 0
        self.pending = (0.0, 0.0, False, False, False)
        self.ticked = False

    def set_viewport(self, w: int, h: int) -> None:
        self.view_w = max(1, int(w))
        self.view_h = max(1, int(h))

    def set_device(self, label: str, lang: str) -> None:
        self.device_label = label
        self.lang = lang

    def is_open(self) -> bool:
        return self.open

    def blocks_sim(self) -> bool:
        return self.enabled_flag and (self.open or self.holding)

    def advance(self, dt: float) -> None:
        if not self.enabled_flag:
            return
        dt = max(0.0, float(dt))
        self.clock += dt
        if self.holding and not self.open:
            self.hold_t += dt
            if self.hold_t >= HOLD_SEC:
                self.try_open()

    def on_pointer(self, ident: int, x: float, y: float, phase: str) -> bool:
        if not self.enabled_flag:
            return False
        if self.open:
            if phase == "begin":
                self._hit(x, y)
            return True
        if phase == "begin":
            self.fingers = [f for f in self.fingers if f.id != ident]
            self.fingers.append(_Finger(ident, x, y))
        elif phase == "move":
            for finger in self.fingers:
                if finger.id == ident:
                    finger.x = x
                    finger.y = y
        elif phase in {"end", "cancel"}:
            self.fingers = [f for f in self.fingers if f.id != ident]
        slop = max(min(self.view_w, self.view_h) * 0.04, 36.0)
        stable = len(self.fingers) >= 3 and all(
            (f.x - f.x0) ** 2 + (f.y - f.y0) ** 2 <= slop * slop for f in self.fingers
        )
        if stable:
            if not self.holding:
                self.holding = True
                self.hold_t = 0.0
            return True
        self.holding = False
        self.hold_t = 0.0
        return False

    def on_key(self, name: str, down: bool) -> bool:
        if not self.enabled_flag or not name:
            return False
        if name.lower() == "f12":
            if not down:
                return True
            if self.last_f12 >= 0.0 and (self.clock - self.last_f12) < 0.25:
                return True
            self.last_f12 = self.clock
            if self.open:
                self.close()
            else:
                self.try_open()
            return True
        if self.open:
            if not down:
                return True
            if name.lower() == "escape":
                self.close()
            elif name.lower() == "backspace":
                self.backspace()
            elif name.lower() in {"enter", "return"}:
                self.push_text("\n")
            return True
        self._push_log({"t": self.clock, "kind": "key", "name": name, "down": bool(down)})
        return False

    def push_text(self, text: str) -> None:
        if not self.open:
            return
        for ch in text:
            if ch == "\u007f":
                self.backspace()
                continue
            if ord(ch) < 32 and ch != "\n":
                continue
            self.note += ch
        self._trim_note()

    def set_note(self, note: str) -> None:
        self.note = note
        self._trim_note()

    def set_preedit(self, text: str) -> None:
        self.preedit = text

    def backspace(self) -> None:
        if self.note:
            self.note = self.note[:-1]

    def note_walk(
        self, lx: float, lz: float, jump: bool, attack: bool, dodge: bool
    ) -> None:
        if not self.enabled_flag or self.blocks_sim():
            return
        self.pending = (float(lx), float(lz), bool(jump), bool(attack), bool(dodge))

    def commit_walk(self, dt: float) -> None:
        if not self.enabled_flag or self.blocks_sim() or dt <= 0.0:
            return
        lx, lz, jump, attack, dodge = self.pending
        self._push_log(
            {
                "t": self.clock,
                "dt": float(dt),
                "kind": "walk",
                "lx": lx,
                "lz": lz,
                "jump": jump,
                "attack": attack,
                "dodge": dodge,
            }
        )

    def record_confirm(self) -> None:
        if not self.enabled_flag or self.open:
            return
        self._push_log({"t": self.clock, "kind": "confirm"})

    def wants_snapshot(self) -> bool:
        if not self.enabled_flag or self.open:
            return False
        if not self.snaps:
            return True
        return self.clock - self.snaps[-1][0] >= SNAP_EVERY - 1e-3

    def snapshot_dump(self, dump: str) -> None:
        if not self.wants_snapshot() or not str(dump).strip():
            return
        self._push_snap(_compact(str(dump)))

    def try_open(self) -> bool:
        if not self.enabled_flag:
            return False
        if self.open:
            return True
        self.open = True
        self.shot = True
        self.capture_t = self.clock
        self.holding = False
        self.hold_t = 0.0
        self.fingers.clear()
        self.preedit = ""
        self.status = "一時停止。メモを書いて「送る」。"
        return True

    def close(self) -> None:
        self.open = False
        self.holding = False
        self.hold_t = 0.0
        self.fingers.clear()
        self.preedit = ""
        self.shot = False
        self.status = ""

    def action(self, name: str) -> None:
        if name in {"close", "cancel"}:
            self.close()
        elif name == "backspace" and self.open:
            self.backspace()
        elif name in {"send", "copy"} and self.open:
            self.packet_action = name
            self._build_packet()

    def set_shot_path(self, path: str) -> None:
        self.shot_path = path
        if self._packet is not None:
            self._packet["instruction"] = instruction_text(bool(self._packet["fits"]), path)
            self.status = self._packet["instruction"]

    def take_shot(self) -> bool:
        shot = self.shot
        self.shot = False
        return shot

    def packet(self) -> dict[str, Any] | None:
        return self._packet

    def display_note(self) -> str:
        if not self.preedit:
            return self.note
        return self.note + self.preedit

    def display_status(self) -> str:
        if not self.status:
            return "一時停止。メモを書いて「送る」。"
        return self.status

    def report(self) -> dict[str, Any]:
        if self.snaps:
            dump_t, dump_text = self.snaps[0]
        else:
            dump_t, dump_text = 0.0, "{}"
        try:
            dump = json.loads(dump_text)
        except json.JSONDecodeError:
            dump = None
        if not isinstance(dump, dict):
            dump = {}
        genre = dump.get("genre") if isinstance(dump.get("genre"), str) else ""
        cap = self.capture_t if self.open else self.clock
        inputs = [
            dict(ev)
            for ev in self.events
            if dump_t + 1e-4 < float(ev.get("t") or 0.0) <= cap + 1e-3
        ]
        return {
            "v": 1,
            "note": self.note,
            "captured_t": cap,
            "dump_t": dump_t,
            "build": {
                "kagra_shared": "python",
                "git": os.environ.get("KAGRA_GIT_REV", "unknown"),
                "profile": "playtest",
                "os": sys.platform,
                "arch": _arch(),
            },
            "device": {
                "label": self.device_label,
                "lang": self.lang,
                "width": self.view_w,
                "height": self.view_h,
            },
            "genre": genre,
            "dump": dump,
            "inputs": inputs,
        }

    def _hit(self, x: float, y: float) -> None:
        lay = layout(self.view_w, self.view_h)
        if _contains(lay["send"], x, y):
            self.action("send")
        elif _contains(lay["copy"], x, y):
            self.action("copy")
        elif _contains(lay["close"], x, y):
            self.action("close")

    def _trim_note(self) -> None:
        chars = list(self.note)
        if len(chars) > NOTE_MAX:
            self.note = "".join(chars[:NOTE_MAX])

    def _push_log(self, ev: dict[str, Any]) -> None:
        self.seq += 1
        ev["seq"] = self.seq
        self.events.append(ev)
        if len(self.events) > 4000:
            del self.events[: len(self.events) - 4000]

    def _push_snap(self, dump: str) -> None:
        self.snaps.append((self.clock, dump))
        cutoff = self.clock - WINDOW_SEC
        self.snaps = [s for s in self.snaps if s[0] >= cutoff - 1e-3]
        if len(self.snaps) > 12:
            self.snaps = self.snaps[-12:]
        if self.snaps:
            oldest = self.snaps[0][0]
            self.events = [e for e in self.events if float(e.get("t") or 0) >= oldest - 1e-3]
        if len(self.events) > 4000:
            self.events = self.events[-4000:]

    def _build_packet(self) -> None:
        packet = issue_from_report(self.report())
        packet["action"] = self.packet_action
        packet["instruction"] = instruction_text(bool(packet["fits"]), self.shot_path)
        self.status = packet["instruction"]
        self.packet_seq += 1
        self._packet = packet


def _compact(text: str) -> str:
    try:
        return json.dumps(json.loads(text), ensure_ascii=False, separators=(",", ":"))
    except json.JSONDecodeError:
        return text


def _arch() -> str:
    import platform

    return platform.machine()


def downloads_dir():
    from pathlib import Path

    profile = os.environ.get("USERPROFILE", "").strip()
    if profile:
        path = Path(profile) / "Downloads"
        if path.is_dir():
            return path
    home = os.environ.get("HOME", "").strip()
    if home:
        path = Path(home) / "Downloads"
        if path.is_dir():
            return path
        lower = Path(home) / "downloads"
        if lower.is_dir():
            return lower
        path.mkdir(parents=True, exist_ok=True)
        return path
    scratch = Path("scratch")
    scratch.mkdir(parents=True, exist_ok=True)
    return scratch


class SessionLog:
    """Forwards a GenreSession and records the calls that repro replays."""

    def __init__(self, inner: Any, rec: Playtest) -> None:
        self._inner = inner
        self.rec = rec

    def set_input(self, lx: float = 0.0, lz: float = 0.0, jump: bool = False, attack: bool = False, dodge: bool = False):
        self.rec.note_walk(lx, lz, jump, attack, dodge)
        if self.rec.blocks_sim():
            return None
        return self._inner.set_input(lx, lz, jump, attack, dodge)

    def tick(self, dt: float):
        self.rec.ticked = True
        self.rec.advance(dt)
        if self.rec.blocks_sim():
            return None
        result = self._inner.tick(dt)
        self.rec.commit_walk(float(dt))
        dumped = self._inner.dump()
        if isinstance(dumped, bytes):
            dumped = dumped.decode("utf-8")
        if not isinstance(dumped, str):
            dumped = json.dumps(dumped, ensure_ascii=False)
        self.rec.snapshot_dump(dumped)
        return result

    def confirm(self):
        if self.rec.blocks_sim():
            return None
        result = self._inner.confirm()
        self.rec.record_confirm()
        return result

    def dump(self):
        return self._inner.dump()

    def __getattr__(self, name: str):
        return getattr(self._inner, name)


def wrap_session(scene: Any, rec: Playtest | None = None) -> Playtest:
    """Wrap ``scene.session`` (or ``scene.play``) so the log is the real input."""
    rec = rec or Playtest(True)
    for name in ("session", "play"):
        obj = getattr(scene, name, None)
        if obj is None or isinstance(obj, SessionLog) or not hasattr(obj, "tick"):
            continue
        setattr(scene, name, SessionLog(obj, rec))
        break
    return rec


_attached = False


def attach_tk(scene: Any, root: Any, label: Any = None) -> None:
    """Pause on F12, IME note, and a GitHub issue draft. Debug / frozen only."""
    global _attached
    if _attached or not enabled():
        return
    import tkinter as tk

    from kagra import gameloop as gl

    rec = Playtest(True)
    rec.set_viewport(getattr(scene, "width", 1280) or 1280, getattr(scene, "height", 720) or 720)
    try:
        import locale
        import platform

        lang = ""
        try:
            lang = locale.getlocale()[0] or ""
        except Exception:
            lang = os.environ.get("LANG", "") or ""
        rec.set_device(platform.platform(), lang)
    except Exception:
        pass
    wrap_session(scene, rec)
    if label is not None:
        try:
            label.configure(borderwidth=0, highlightthickness=0, padx=0, pady=0)
        except Exception:
            pass

    note = tk.Text(root, wrap="word", font=("sans-serif", 12), bg="#0e1018", fg="#ece8dc", insertbackground="#ece8dc")
    status = tk.Label(root, text="", fg="#d2cec4", bg="#1c202c", anchor="w", justify="left")
    buttons = {
        "send": tk.Button(root, text="送る"),
        "copy": tk.Button(root, text="コピー"),
        "close": tk.Button(root, text="閉じる"),
    }
    widgets = [note, status, *buttons.values()]
    focused = {"on": False}
    last_open = {"on": False}

    def pull_note() -> None:
        if not rec.is_open():
            return
        try:
            rec.set_note(note.get("1.0", "end-1c"))
        except Exception:
            pass

    def place_widgets() -> None:
        if not rec.is_open():
            if last_open["on"]:
                for widget in widgets:
                    widget.place_forget()
                focused["on"] = False
                last_open["on"] = False
                gl.set_input_blocked(False)
            return
        if not last_open["on"]:
            gl.release_all_keys()
            gl.set_input_blocked(True)
            last_open["on"] = True
        lay = layout(rec.view_w, rec.view_h)
        nx, ny, nw, nh = lay["note"]
        note.place(x=nx, y=ny, width=nw, height=nh)
        sx, sy, sw, sh = lay["send"]
        buttons["send"].place(x=sx, y=sy, width=sw, height=sh)
        cx, cy, cw, ch = lay["copy"]
        buttons["copy"].place(x=cx, y=cy, width=cw, height=ch)
        kx, ky, kw, kh = lay["close"]
        buttons["close"].place(x=kx, y=ky, width=kw, height=kh)
        status.place(x=nx, y=ny + nh + 4, width=nw, height=max(18, 22 * lay["scale"]))
        status.configure(text=rec.display_status())
        if not focused["on"]:
            note.focus_set()
            focused["on"] = True

    def save_shot(target: Any) -> None:
        if not rec.take_shot():
            return
        rgba = getattr(target, "_frame_rgba", None)
        width = int(getattr(target, "width", rec.view_w) or rec.view_w)
        height = int(getattr(target, "height", rec.view_h) or rec.view_h)
        if not rgba:
            return
        try:
            from datetime import datetime, timezone

            stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
            path = downloads_dir() / f"kagra-playtest-{stamp}.png"
            path.write_bytes(gl.rgba_to_png(rgba, width, height))
            rec.set_shot_path(str(path))
        except Exception:
            pass

    def before_draw(target: Any, dt: float = 0.0) -> None:
        rec.set_viewport(getattr(target, "width", rec.view_w), getattr(target, "height", rec.view_h))
        if not rec.ticked:
            rec.advance(float(dt))
        rec.ticked = False
        save_shot(target)
        place_widgets()

    def hud_hook(hud: dict[str, Any] | None) -> dict[str, Any]:
        pull_note()
        return overlay_hud(hud, rec, rec.view_w, rec.view_h)

    def deliver(action: str) -> None:
        pull_note()
        rec.action(action)
        packet = rec.packet()
        if not packet:
            return
        try:
            root.clipboard_clear()
            root.clipboard_append(packet["copy"])
        except Exception:
            pass
        try:
            from datetime import datetime, timezone

            stamp = datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S")
            path = downloads_dir() / f"kagra-playtest-{stamp}.txt"
            path.write_text(packet["block"], encoding="utf-8")
        except Exception:
            pass
        if action == "send":
            import webbrowser

            try:
                webbrowser.open(packet["url"])
            except Exception:
                pass
        status.configure(text=packet["instruction"])
        rec.status = packet["instruction"]

    buttons["send"].configure(command=lambda: deliver("send"))
    buttons["copy"].configure(command=lambda: deliver("copy"))
    buttons["close"].configure(command=lambda: rec.action("close"))

    def on_f12(event: Any) -> str:
        rec.on_key("F12", True)
        place_widgets()
        return "break"

    root.bind_all("<F12>", on_f12)
    gl.set_hud_hook(hud_hook)
    gl.set_before_draw(before_draw)
    _attached = True
    scene._playtest = rec  # type: ignore[attr-defined]


def detach_tk() -> None:
    global _attached
    _attached = False
    try:
        from kagra import gameloop as gl

        gl.restore_hud_hook()
    except Exception:
        pass

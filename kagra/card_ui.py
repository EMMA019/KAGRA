"""Reusable card table UI. Rules stay in the game. This module only places and draws.

Pieces other card games can share:

- ``fan_slots`` / ``row_slots`` / ``duel_slots`` - screen layout
- ``CardFace`` / ``CardView`` - a face with text and a small art motif
- ``card_hud`` - one card, face or back, including a flip squeeze
- ``TableView`` - tweens cards into their slots and flips new faces
- ``DragPlay`` - mouse and touch (begin / move / end / cancel) to play or attack

HUD quads are axis-aligned, so a fan is an arc of positions with overlap.
The center sits lower. The edges lift.
"""
from __future__ import annotations

import math
from dataclasses import dataclass, field

from kagra.motion import Tween
from kagra.ui2d import merge, panel

__all__ = [
    "CardFace",
    "CardView",
    "DragPlay",
    "DragResult",
    "TableView",
    "Zone",
    "card_hud",
    "duel_slots",
    "fan_slots",
    "row_slots",
]

_INK = [42, 32, 28, 255]
_CREAM = [236, 228, 210, 245]
_FRAME = [92, 64, 48, 255]
_BACK = [28, 48, 92, 245]
_BACK_EDGE = [186, 154, 72, 255]
_DIM = [186, 178, 166, 235]


@dataclass
class CardFace:
    """What a card says. ``art`` picks a motif drawn with quads."""

    title: str = ""
    cost: int = 0
    attack: int | None = None
    health: int | None = None
    text: str = ""
    art: int = 0
    face_down: bool = False
    ready: bool = True
    kind: str = "unit"


@dataclass
class CardView:
    """Screen pose. Tweens write ``x``, ``y``, and ``flip`` (0 back, 1 face)."""

    uid: int
    face: CardFace
    x: float = 0.0
    y: float = 0.0
    w: float = 86.0
    h: float = 120.0
    flip: float = 1.0
    dragging: bool = False
    zone: str = "hand"
    index: int = 0


@dataclass
class Zone:
    name: str
    index: int
    x: float
    y: float
    w: float
    h: float

    def contains(self, px: float, py: float) -> bool:
        return self.x <= px <= self.x + self.w and self.y <= py <= self.y + self.h


@dataclass
class DragResult:
    action: str
    index: int
    target: int = -1


def fan_slots(
    n: int,
    cx: float,
    cy: float,
    card_w: float,
    card_h: float,
    arc_deg: float = 28.0,
    lift_sign: float = 1.0,
) -> list[tuple[float, float]]:
    """Top-left corners of a hand. Overlap, with the edges lifted along the arc."""
    if n <= 0:
        return []
    overlap = card_w * 0.78
    width = card_w + overlap * (n - 1)
    left = cx - width * 0.5
    arc = math.radians(arc_deg)
    slots: list[tuple[float, float]] = []
    for i in range(n):
        t = 0.5 if n == 1 else i / (n - 1)
        angle = (t - 0.5) * arc
        lift = (1.0 - math.cos(angle)) * card_h * 0.28 * lift_sign
        slots.append((left + i * overlap, cy - lift))
    return slots


def row_slots(
    n: int,
    cx: float,
    cy: float,
    card_w: float,
    gap: float = 12.0,
) -> list[tuple[float, float]]:
    if n <= 0:
        return []
    width = n * card_w + (n - 1) * gap
    left = cx - width * 0.5
    return [(left + i * (card_w + gap), cy) for i in range(n)]


def duel_slots(
    w: float,
    h: float,
    you_hand: int,
    you_board: int,
    foe_hand: int,
    foe_board: int,
    card_w: float = 78.0,
    card_h: float = 108.0,
) -> dict:
    """Slots and hit zones for a two-player table. Coordinates are pixels."""
    w = float(w)
    h = float(h)
    hand = fan_slots(you_hand, w * 0.50, h * 0.80, card_w, card_h)
    board = row_slots(you_board, w * 0.50, h * 0.46, card_w, 10.0)
    foe_b = row_slots(foe_board, w * 0.50, h * 0.30, card_w, 10.0)
    foe_h = fan_slots(
        foe_hand, w * 0.50, h * 0.04, card_w, card_h, arc_deg=18.0, lift_sign=-1.0
    )
    zones = [
        Zone("play", -1, w * 0.16, h * 0.42, w * 0.68, h * 0.20),
        Zone("face", -1, w * 0.74, h * 0.06, w * 0.20, h * 0.16),
    ]
    for i, (x, y) in enumerate(foe_b):
        zones.append(Zone("unit", i, x, y, card_w, card_h))
    return {
        "hand": hand,
        "board": board,
        "foe_board": foe_b,
        "foe_hand": foe_h,
        "zones": zones,
        "play_line": h * 0.64,
        "card_w": card_w,
        "card_h": card_h,
    }


def _art(motif: int, x: float, y: float, w: float, h: float) -> list[dict]:
    """A small picture made of quads. Motif 0..5, reused by any card game."""
    palette = [
        [176, 96, 64, 255],
        [72, 112, 168, 255],
        [64, 128, 96, 255],
        [168, 112, 48, 255],
        [120, 96, 160, 255],
        [64, 140, 120, 255],
    ]
    ink = palette[int(motif) % len(palette)]
    wash = [ink[0], ink[1], ink[2], 70]
    quads = [{"x": x, "y": y, "w": w, "h": h, "color": wash}]
    cx = x + w * 0.5
    cy = y + h * 0.5
    kind = int(motif) % 6
    if kind == 0:
        quads.append({"x": cx - 7, "y": cy - 8, "w": 14, "h": 16, "color": ink})
        quads.append({"x": cx - 12, "y": cy - 16, "w": 6, "h": 8, "color": ink})
        quads.append({"x": cx + 6, "y": cy - 16, "w": 6, "h": 8, "color": ink})
    elif kind == 1:
        quads.append({"x": cx - 2, "y": y + 4, "w": 4, "h": h - 8, "color": ink})
        quads.append({"x": cx - 10, "y": cy - 2, "w": 20, "h": 4, "color": ink})
    elif kind == 2:
        quads.append({"x": cx - 12, "y": y + 6, "w": 24, "h": h - 14, "color": ink})
        quads.append({"x": cx - 4, "y": y + 2, "w": 8, "h": 8, "color": [236, 228, 210, 255]})
    elif kind == 3:
        quads.append({"x": cx - 2, "y": y + 8, "w": 4, "h": h - 16, "color": ink})
        quads.append({"x": cx - 8, "y": y + 4, "w": 16, "h": 5, "color": ink})
    elif kind == 4:
        quads.append({"x": cx - 12, "y": cy - 8, "w": 24, "h": 16, "color": ink})
        quads.append({"x": cx - 8, "y": cy - 14, "w": 16, "h": 6, "color": ink})
    else:
        quads.append({"x": cx - 3, "y": cy - 12, "w": 6, "h": 24, "color": ink})
        quads.append({"x": cx - 12, "y": cy - 3, "w": 24, "h": 6, "color": ink})
    return quads


def _badge(x: float, y: float, text: str, fill: list[int]) -> dict:
    return merge(
        panel(x, y, 22, 16, color=tuple(fill), border=(32, 24, 20, 255), border_w=1.0, radius=4),
        {
            "texts": [
                {
                    "text": text,
                    "x": x + 11,
                    "y": y + 1,
                    "size": 11,
                    "color": [255, 248, 236, 255],
                    "align": "center",
                }
            ]
        },
    )


def card_hud(view: CardView) -> dict:
    """One card. ``flip`` 0 is the back. Mid-flip squeezes toward the center."""
    flip = 0.0 if view.flip < 0.0 else 1.0 if view.flip > 1.0 else float(view.flip)
    squeeze = max(0.08, abs(math.cos(flip * math.pi)))
    w = view.w * squeeze
    h = view.h
    x = view.x + (view.w - w) * 0.5
    y = view.y
    show_text = squeeze > 0.72
    if flip < 0.5 or view.face.face_down:
        body = panel(x, y, w, h, color=tuple(_BACK), border=tuple(_BACK_EDGE), border_w=2.0, radius=8)
        inset = 6.0 * squeeze
        if w > inset * 2 + 8:
            body["quads"].extend(
                _art(0, x + inset, y + inset, w - inset * 2, h - inset * 2)
            )
        return body
    fill = _CREAM if view.face.ready or view.face.kind == "spell" else _DIM
    body = panel(x, y, w, h, color=tuple(fill), border=tuple(_FRAME), border_w=2.0, radius=8)
    pad = 6.0
    art_y = y + 28
    art_h = max(16.0, h - 62)
    if w > 24:
        body["quads"].extend(_art(view.face.art, x + pad, art_y, max(8.0, w - pad * 2), art_h))
    if not show_text:
        return body
    texts: list[dict] = []
    if view.face.title:
        texts.append(
            {
                "text": view.face.title,
                "x": x + w * 0.58,
                "y": y + 6,
                "size": 13,
                "color": _INK,
                "align": "center",
            }
        )
    if view.face.text:
        texts.append(
            {
                "text": view.face.text,
                "x": x + w * 0.5,
                "y": y + h - 36,
                "size": 11,
                "color": [72, 56, 48, 255],
                "align": "center",
            }
        )
    parts = [body, {"texts": texts}]
    parts.append(_badge(x + 4, y + 4, str(int(view.face.cost)), [186, 132, 42, 255]))
    if view.face.kind == "spell" and (view.face.attack or 0) > 0:
        parts.append(_badge(x + 4, y + h - 22, str(int(view.face.attack or 0)), [168, 64, 48, 255]))
    elif view.face.kind == "spell":
        parts.append(_badge(x + 4, y + h - 22, str(int(view.face.health or 0)), [48, 128, 96, 255]))
    else:
        parts.append(_badge(x + 4, y + h - 22, str(int(view.face.attack or 0)), [168, 64, 48, 255]))
        parts.append(
            _badge(x + w - 26, y + h - 22, str(int(view.face.health or 0)), [48, 96, 148, 255])
        )
    return merge(*parts)


class TableView:
    """Keeps card views, tweens them toward slots, and flips faces that just arrived."""

    def __init__(self, card_w: float = 78.0, card_h: float = 108.0) -> None:
        self.card_w = float(card_w)
        self.card_h = float(card_h)
        self.origin = (36.0, 36.0)
        self.views: list[CardView] = []
        self._tweens: list[Tween] = []

    def tick(self, dt: float) -> None:
        dt = max(0.0, float(dt))
        alive: list[Tween] = []
        for tw in self._tweens:
            if not tw.step(dt):
                alive.append(tw)
        self._tweens = alive

    def tween_attr(self, view: CardView, attr: str, end: float, duration: float = 0.22) -> None:
        end = float(end)
        existing = next(
            (tw for tw in self._tweens if tw.obj is view and tw.attr == attr and not tw.done),
            None,
        )
        if existing is not None and abs(existing.end - end) < 0.5:
            return
        self._tweens = [tw for tw in self._tweens if not (tw.obj is view and tw.attr == attr)]
        if abs(float(getattr(view, attr)) - end) < 0.4:
            setattr(view, attr, end)
            return
        self._tweens.append(Tween(view, attr, end, duration))

    def sync(self, entries: list[tuple[int, CardFace, str, int]], slots: dict) -> None:
        """``entries`` are ``(uid, face, zone, index)``. ``slots[zone]`` is a list of points."""
        by_uid = {view.uid: view for view in self.views}
        nxt: list[CardView] = []
        for uid, face, zone, index in entries:
            points = slots.get(zone) or []
            if index < 0 or index >= len(points):
                continue
            sx, sy = points[index]
            view = by_uid.get(uid)
            fresh = view is None
            if view is None:
                view = CardView(
                    uid=uid,
                    face=face,
                    x=float(self.origin[0]),
                    y=float(self.origin[1]),
                    w=self.card_w,
                    h=self.card_h,
                    flip=0.0,
                    zone=zone,
                    index=index,
                )
            view.face = face
            view.zone = zone
            view.index = index
            view.w = self.card_w
            view.h = self.card_h
            if not view.dragging:
                self.tween_attr(view, "x", float(sx), 0.22)
                self.tween_attr(view, "y", float(sy), 0.22)
            if face.face_down:
                view.flip = 0.0
                self._tweens = [
                    tw for tw in self._tweens if not (tw.obj is view and tw.attr == "flip")
                ]
            elif fresh or view.flip < 0.99:
                self.tween_attr(view, "flip", 1.0, 0.28)
            nxt.append(view)
        kept = {view.uid for view in nxt}
        self.views = nxt
        self._tweens = [tw for tw in self._tweens if getattr(tw.obj, "uid", None) in kept]

    def hud(self) -> dict:
        ordered = sorted(self.views, key=lambda view: (view.dragging, view.y))
        return merge(*[card_hud(view) for view in ordered])


@dataclass
class DragPlay:
    """Pointer drag shared by mouse and touch. ``phase`` is begin, move, end, or cancel."""

    play_line: float = 0.0
    active: int | None = None
    grab_x: float = 0.0
    grab_y: float = 0.0
    _views: list[CardView] = field(default_factory=list)

    def clear(self) -> None:
        for view in self._views:
            view.dragging = False
        self.active = None

    def pointer(
        self,
        phase: str,
        x: float,
        y: float,
        views: list[CardView],
        zones: list[Zone],
    ) -> DragResult | None:
        self._views = views
        phase = str(phase)
        x = float(x)
        y = float(y)
        if phase == "begin":
            hit = _top_card(views, x, y)
            if hit is None or hit.zone not in {"hand", "board"}:
                self.active = None
                return None
            if hit.zone == "board" and not hit.face.ready:
                return None
            self.active = hit.uid
            hit.dragging = True
            self.grab_x = x - hit.x
            self.grab_y = y - hit.y
            return None
        hit = _by_uid(views, self.active)
        if hit is None:
            self.active = None
            return None
        if phase == "move":
            hit.x = x - self.grab_x
            hit.y = y - self.grab_y
            return None
        if phase not in {"end", "cancel"}:
            return None
        hit.dragging = False
        self.active = None
        if phase == "cancel":
            return DragResult("cancel", hit.index, -1)
        cx = hit.x + hit.w * 0.5
        cy = hit.y + hit.h * 0.5
        if hit.zone == "hand":
            in_play = _zone_at(zones, "play", cx, cy) is not None or cy < self.play_line
            if in_play:
                return DragResult("play", hit.index, -1)
            return DragResult("cancel", hit.index, -1)
        if hit.zone == "board":
            unit = _zone_at(zones, "unit", cx, cy)
            if unit is not None:
                return DragResult("attack", hit.index, unit.index)
            if _zone_at(zones, "face", cx, cy) is not None:
                return DragResult("attack", hit.index, -1)
            return DragResult("cancel", hit.index, -1)
        return DragResult("cancel", hit.index, -1)


def _top_card(views: list[CardView], x: float, y: float) -> CardView | None:
    for view in reversed(views):
        if view.x <= x <= view.x + view.w and view.y <= y <= view.y + view.h:
            return view
    return None


def _by_uid(views: list[CardView], uid: int | None) -> CardView | None:
    if uid is None:
        return None
    for view in views:
        if view.uid == uid:
            return view
    return None


def _zone_at(zones: list[Zone], name: str, x: float, y: float) -> Zone | None:
    for zone in zones:
        if zone.name == name and zone.contains(x, y):
            return zone
    return None

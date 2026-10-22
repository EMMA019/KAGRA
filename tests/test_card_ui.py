"""Card table UI. Pure layout, drag, and flip. No Rust extension."""
from __future__ import annotations

from pathlib import Path

from tests.conftest import ROOT, load_kagra_submodule

card_ui = load_kagra_submodule("card_ui")

ROOT_PATH = Path(ROOT)


def test_fan_lifts_the_edges_and_spreads():
    slots = card_ui.fan_slots(5, 400, 500, 80, 110)
    assert len(slots) == 5
    xs = [p[0] for p in slots]
    ys = [p[1] for p in slots]
    assert xs == sorted(xs)
    assert ys[0] < ys[2]
    assert ys[-1] < ys[2]
    assert card_ui.fan_slots(0, 0, 0, 80, 110) == []
    one = card_ui.fan_slots(1, 200, 100, 80, 110)
    assert len(one) == 1


def test_duel_slots_cover_both_sides():
    layout = card_ui.duel_slots(960, 540, 4, 2, 3, 1)
    assert len(layout["hand"]) == 4
    assert len(layout["board"]) == 2
    assert len(layout["foe_hand"]) == 3
    assert len(layout["foe_board"]) == 1
    names = [zone.name for zone in layout["zones"]]
    assert "play" in names
    assert "face" in names
    assert names.count("unit") == 1


def test_drag_play_and_attack_and_cancel():
    face = card_ui.CardFace(title="騎士", cost=3, attack=3, health=3, art=4, kind="unit", ready=True)
    view = card_ui.CardView(uid=7, face=face, x=100, y=400, w=80, h=110, zone="hand", index=2)
    drag = card_ui.DragPlay()
    drag.play_line = 300
    zones = [
        card_ui.Zone("play", -1, 200, 150, 400, 120),
        card_ui.Zone("face", -1, 400, 20, 140, 80),
        card_ui.Zone("unit", 1, 180, 20, 90, 120),
    ]
    assert drag.pointer("begin", 120, 430, [view], zones) is None
    assert view.dragging
    drag.pointer("move", 320, 180, [view], zones)
    played = drag.pointer("end", 320, 180, [view], zones)
    assert played.action == "play"
    assert played.index == 2
    assert not view.dragging

    view.zone = "hand"
    view.x, view.y = 100, 400
    drag.pointer("begin", 110, 410, [view], zones)
    cancelled = drag.pointer("cancel", 0, 0, [view], zones)
    assert cancelled.action == "cancel"

    view.zone = "board"
    view.index = 0
    view.x, view.y = 100, 200
    drag.pointer("begin", 120, 220, [view], zones)
    drag.pointer("move", 450, 40, [view], zones)
    attacked = drag.pointer("end", 450, 40, [view], zones)
    assert attacked.action == "attack"
    assert attacked.target == -1

    view.x, view.y = 100, 200
    drag.pointer("begin", 120, 220, [view], zones)
    drag.pointer("move", 200, 40, [view], zones)
    traded = drag.pointer("end", 200, 40, [view], zones)
    assert traded.action == "attack"
    assert traded.target == 1


def test_flip_tween_and_face_text():
    face = card_ui.CardFace(title="騎士", cost=3, attack=3, health=3, text="攻撃", art=4)
    table = card_ui.TableView()
    table.sync([(1, face, "hand", 0)], {"hand": [(240.0, 360.0)]})
    view = table.views[0]
    assert view.flip == 0.0
    table.tick(0.1)
    assert 0.0 < view.flip < 1.0
    table.tick(0.4)
    assert view.flip == 1.0
    hud = card_ui.card_hud(view)
    texts = [item["text"] for item in hud["texts"]]
    assert "騎士" in texts
    assert "攻撃" in texts
    back = card_ui.CardView(uid=2, face=card_ui.CardFace(face_down=True, title="騎士"), x=0, y=0, flip=0)
    hidden = [item["text"] for item in card_ui.card_hud(back).get("texts", [])]
    assert "騎士" not in hidden
    blob = "\n".join(texts)
    assert "\u2014" not in blob
    assert "\u2013" not in blob


def test_sources_avoid_em_dash():
    for rel in (
        "kagra/card_ui.py",
        "examples/card_minimal.py",
        "examples/verify_scenarios/card_smoke.json",
        "kagra-shared/tests/fixtures/card_world.json",
    ):
        text = (ROOT_PATH / rel).read_text(encoding="utf-8")
        assert "\u2014" not in text, rel
        assert "\u2013" not in text, rel

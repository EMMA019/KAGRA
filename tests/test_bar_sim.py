"""Bar 経営シムの純ロジック（拡張不要）。"""
from __future__ import annotations

from pathlib import Path

from tests.conftest import load_kagra_submodule


def test_rng_is_deterministic():
    bar = load_kagra_submodule("bar_sim")
    a = bar.Rng(1).guests_for_day(3)
    b = bar.Rng(1).guests_for_day(3)
    assert [g["id"] for g in a] == [g["id"] for g in b]


def test_serve_consumes_stock_and_pays(tmp_path: Path):
    bar = load_kagra_submodule("bar_sim")
    scene = bar.BarSim(save_path=tmp_path / "s.json", seed=1)
    scene.game["stock"]["whiskey"] = 1
    scene.guest = dict(bar.GUESTS[0])  # ren likes old_fashioned
    before = scene.game["money"]
    scene._do_serve(0)
    assert scene.game["stock"]["whiskey"] == 0
    assert scene.game["money"] == before + bar.DRINKS["old_fashioned"]["price"]


def test_broke_closes_shop(tmp_path: Path):
    bar = load_kagra_submodule("bar_sim")
    scene = bar.BarSim(save_path=tmp_path / "s.json", seed=1)
    scene.game["money"] = 10
    scene.game["sales_today"] = 0
    scene._close_day()
    assert scene.game["lost"] is True


def test_save_roundtrip(tmp_path: Path):
    bar = load_kagra_submodule("bar_sim")
    path = tmp_path / "s.json"
    scene = bar.BarSim(save_path=path, seed=2)
    scene.game["money"] = 333
    scene._save()
    again = bar.BarSim(save_path=path, seed=2)
    assert again.game["money"] == 333


def test_room_dump_has_no_genre():
    import json

    dump = Path("kagra-shared/tests/fixtures/bar_room_world.json")
    data = json.loads(dump.read_text(encoding="utf-8"))
    assert "genre" not in data or data.get("genre") in (None, "")
    names = {p["name"] for p in data["props"]}
    assert {"counter", "door", "shelf"} <= names
    assert len(data["lights"]) == 4

"""ゲームマニフェストとステージング。拡張モジュールには依存しない。"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from tests.conftest import ROOT, load_kagra_submodule

dist = load_kagra_submodule("distribute")


def test_platformer_manifest_has_required_fields():
    manifest = dist.manifest_for_game(ROOT, "examples/platformer_minimal.py")
    assert manifest.title == "Box Hop"
    assert manifest.version == "0.1.0"
    assert manifest.entry == "examples/platformer_minimal.py"
    assert manifest.icon == "assets/icons/game_placeholder.png"
    assert manifest.world == "kagra-shared/tests/fixtures/platformer_world.json"
    assert manifest.slug == "platformer"
    assert manifest.application_id == "dev.kagra.platformer"
    assert manifest.version_code() == 100
    assert (ROOT / manifest.icon).is_file()


def test_every_shipped_manifest_loads():
    paths = sorted((ROOT / "games").glob("*.json"))
    assert len(paths) >= 19
    for path in paths:
        manifest = dist.load_manifest(path, ROOT)
        assert manifest.title
        assert manifest.entry.endswith(".py")
        assert (ROOT / manifest.entry).is_file()
        assert (ROOT / manifest.icon).read_bytes().startswith(b"\x89PNG")


def test_missing_field_is_an_error(tmp_path: Path):
    icon = tmp_path / "icon.png"
    icon.write_bytes(dist.solid_icon_png("x"))
    entry = tmp_path / "game.py"
    entry.write_text("print('hi')\n", encoding="utf-8")
    bad = tmp_path / "game.json"
    bad.write_text(
        json.dumps({"icon": "icon.png", "version": "0.1.0", "entry": "game.py"}),
        encoding="utf-8",
    )
    with pytest.raises(dist.ManifestError):
        dist.load_manifest(bad, tmp_path)


def test_path_traversal_is_rejected(tmp_path: Path):
    icon = tmp_path / "icon.png"
    icon.write_bytes(dist.solid_icon_png("x"))
    entry = tmp_path / "game.py"
    entry.write_text("print('hi')\n", encoding="utf-8")
    bad = tmp_path / "game.json"
    bad.write_text(
        json.dumps(
            {
                "title": "Nope",
                "icon": "icon.png",
                "version": "1.2.3",
                "entry": "../game.py",
            }
        ),
        encoding="utf-8",
    )
    with pytest.raises(dist.ManifestError):
        dist.load_manifest(bad, tmp_path)


def test_version_code_and_ico():
    assert dist.version_code("0.2.0") == 200
    assert dist.version_code("1.2.3") == 10203
    png = dist.solid_icon_png("箱跳び")
    assert png.startswith(b"\x89PNG")
    ico = dist.png_to_ico(png)
    assert ico[:4] == b"\x00\x00\x01\x00"
    assert png in ico


def test_stage_platformer_keeps_repo_layout(tmp_path: Path):
    manifest = dist.manifest_for_game(ROOT, "examples/platformer_minimal.py")
    rels = dist.stage_game(ROOT, manifest, tmp_path)
    assert "examples/platformer_minimal.py" in rels
    assert "kagra_game.json" in rels
    assert "world.json" in rels
    assert "kagra-shared/Cargo.toml" in rels
    assert "assets/library/kaykit/adventurers/characters/Knight.glb" in rels
    staged = json.loads((tmp_path / "kagra_game.json").read_text(encoding="utf-8"))
    assert staged["entry"] == manifest.entry
    assert staged["title"] == "Box Hop"
    assert (tmp_path / "world.json").read_bytes() == (
        ROOT / manifest.world
    ).read_bytes()


def test_android_properties_are_utf8(tmp_path: Path):
    manifest = dist.manifest_for_game(ROOT, "examples/platformer_minimal.py")
    manifest.title = "箱跳び"
    path = tmp_path / "kagra.game.properties"
    dist.write_android_properties(manifest, path, icon_ref="@drawable/ic_launcher")
    text = path.read_text(encoding="utf-8")
    assert "title=箱跳び" in text
    assert "application_id=dev.kagra.platformer" in text
    assert "version_code=100" in text
    assert "icon=@drawable/ic_launcher" in text

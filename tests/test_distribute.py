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


def test_diamond_days_ships_an_adaptive_icon_and_a_japanese_name(tmp_path: Path):
    manifest = dist.manifest_for_game(ROOT, "games/diamond_days_minimal.json")
    assert manifest.title_ja == "白球デイズ"
    assert set(manifest.icon_layers) == {"foreground", "background"}
    res = tmp_path / "res"
    # A file the last build left is removed.
    stale = res / "drawable-mdpi" / "ic_launcher.png"
    stale.parent.mkdir(parents=True)
    stale.write_bytes(b"old")
    icon_ref, label_ref = dist.write_android_res(manifest, ROOT, res)
    assert icon_ref == "@mipmap/ic_launcher"
    assert label_ref == "@string/kagra_app_name"
    assert not stale.exists()
    assert (res / "drawable-xxxhdpi" / "ic_launcher.png").read_bytes() == (ROOT / manifest.icon).read_bytes()
    for key in ("foreground", "background"):
        assert (res / "drawable-xxxhdpi" / f"ic_launcher_{key}.png").is_file()
    xml = (res / "mipmap-anydpi-v26" / "ic_launcher.xml").read_text(encoding="utf-8")
    assert "@drawable/ic_launcher_foreground" in xml and "@drawable/ic_launcher_background" in xml
    assert "白球デイズ" in (res / "values-ja" / "kagra_game.xml").read_text(encoding="utf-8")
    assert ">Diamond Days<" in (res / "values" / "kagra_game.xml").read_text(encoding="utf-8")
    props = tmp_path / "kagra.game.properties"
    dist.write_android_properties(manifest, props, icon_ref=icon_ref, label_ref=label_ref)
    text = props.read_text(encoding="utf-8")
    assert "label=@string/kagra_app_name" in text and "icon=@mipmap/ic_launcher" in text


def test_a_plain_icon_stays_a_drawable_with_the_title(tmp_path: Path):
    manifest = dist.manifest_for_game(ROOT, "examples/platformer_minimal.py")
    icon_ref, label_ref = dist.write_android_res(manifest, ROOT, tmp_path)
    assert (icon_ref, label_ref) == ("@drawable/ic_launcher", "Box Hop")
    assert not (tmp_path / "mipmap-anydpi-v26").exists()
    assert sorted(p.relative_to(tmp_path).as_posix() for p in tmp_path.rglob("*.png")) == [
        "drawable-xxxhdpi/ic_launcher.png"
    ]


def test_icon_layers_must_be_two_squares_of_one_size(tmp_path: Path):
    entry = tmp_path / "game.py"
    entry.write_text("print('hi')\n", encoding="utf-8")
    (tmp_path / "icon.png").write_bytes(dist.solid_icon_png("x", 48))
    (tmp_path / "front.png").write_bytes(dist.solid_icon_png("x", 108))
    (tmp_path / "back.png").write_bytes(dist.solid_icon_png("x", 216))
    raw = {
        "title": "T",
        "icon": "icon.png",
        "version": "0.1.0",
        "entry": "game.py",
        "icon_layers": {"foreground": "front.png", "background": "back.png"},
    }
    bad = tmp_path / "game.json"
    bad.write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(dist.ManifestError):
        dist.load_manifest(bad, tmp_path)
    raw["icon_layers"] = {"foreground": "front.png"}
    bad.write_text(json.dumps(raw), encoding="utf-8")
    with pytest.raises(dist.ManifestError):
        dist.load_manifest(bad, tmp_path)


def test_android_strings_are_escaped():
    assert dist.android_string("Tom's \"A&B\"") == "Tom\\'s \\\"A&amp;B\\\""
    assert dist.android_string("@home") == "\\@home"
    assert dist.android_string("白球デイズ") == "白球デイズ"

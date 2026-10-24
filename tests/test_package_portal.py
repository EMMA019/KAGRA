"""scripts/package_portal.py: zip layout, no stray requests, size budget. Uses a fake wasm."""
from __future__ import annotations

import importlib.util
import json
import os
import stat
import zipfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("package_portal", ROOT / "scripts" / "package_portal.py")
assert _spec is not None and _spec.loader is not None
pp = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pp)


@pytest.fixture()
def fake_pkg(tmp_path):
    pkg = tmp_path / "pkg"
    pkg.mkdir()
    (pkg / "kagra_shared.js").write_text("export default function init() {}\n", encoding="utf-8")
    (pkg / "kagra_shared_bg.wasm").write_bytes(b"\0asm\1\0\0\0" + os.urandom(2048))
    return pkg


def build(tmp_path, fake_pkg, *extra):
    out = tmp_path / "out"
    code = pp.main(["--pkg", str(fake_pkg), "--out", str(out), "--keep-tree", *extra])
    return code, out


def test_zip_layout_and_no_external_requests(tmp_path, fake_pkg, capsys):
    code, out = build(tmp_path, fake_pkg)
    assert code == 0
    zips = list(out.glob("*.zip"))
    assert [z.name for z in zips] == ["neon_cat_v0.1.0.zip"]
    with zipfile.ZipFile(zips[0]) as zf:
        names = zf.namelist()
    assert names[0] == "index.html"
    assert {"i18n.js", "portal.js", "pkg/kagra_shared.js", "pkg/kagra_shared_bg.wasm",
            "games/neon_cat/bundle.json", "games/neon_cat/world.json"} <= set(names)
    assert not any(n.startswith("/") or ".." in n for n in names)
    assert not any(n.endswith(".pyc") for n in names)
    html = (out / "neon_cat" / "index.html").read_text(encoding="utf-8")
    assert 'name="kagra-portal" content="none"' in html
    assert 'data-portal="hide"' not in html          # catalog links are stripped
    assert pp.broken_paths(out / "neon_cat") == []
    assert pp.unexpected_urls(out / "neon_cat") == []
    text = capsys.readouterr().out
    assert "raw" in text and "gzip" in text and "initial" in text and "total" in text


def test_only_this_games_files(tmp_path, fake_pkg):
    _, out = build(tmp_path, fake_pkg)
    tree = out / "neon_cat"
    assert [p.name for p in (tree / "games").iterdir()] == ["neon_cat"]
    bundle = json.loads((tree / "games/neon_cat/bundle.json").read_text(encoding="utf-8"))
    assert "platform" in bundle and bundle["platform"]["ads"]["commercial_on"] == ["retry"]
    on_disk = {p.relative_to(tree).as_posix() for p in (tree / "assets").rglob("*") if p.is_file()}
    assert on_disk == {f["url"] for f in bundle["files"] if f["url"].startswith("assets/")}
    assert not (tree / "games/neon_cat/icon.png").exists()


def test_portal_flag_is_built_in(tmp_path, fake_pkg):
    code, out = build(tmp_path, fake_pkg, "--portal", "crazygames")
    assert code == 0
    assert (out / "neon_cat_crazygames_v0.1.0.zip").is_file()
    html = (out / "neon_cat" / "index.html").read_text(encoding="utf-8")
    assert 'name="kagra-portal" content="crazygames"' in html
    # The SDK address is in portal.js only, never in the page.
    assert "sdk.crazygames.com" not in html
    assert "sdk.crazygames.com" in (out / "neon_cat" / "portal.js").read_text(encoding="utf-8")


def test_same_input_same_zip(tmp_path, fake_pkg):
    _, a = build(tmp_path / "a", fake_pkg)
    _, b = build(tmp_path / "b", fake_pkg)
    za, zb = next(a.glob("*.zip")), next(b.glob("*.zip"))
    assert za.read_bytes() == zb.read_bytes()


def test_unexpected_url_fails_the_build(tmp_path, fake_pkg, monkeypatch):
    real = pp.stage

    def evil(*args, **kw):
        facts = real(*args, **kw)
        (args[3] / "index.html").write_text(
            (args[3] / "index.html").read_text(encoding="utf-8") + "<script src=\"//cdn.example/x.js\"></script>\n"
            "<img src=\"https://tracker.example/p.png\">\n",
            encoding="utf-8",
        )
        return facts

    monkeypatch.setattr(pp, "stage", evil)
    code, out = build(tmp_path, fake_pkg)
    assert code == 1
    assert not list(out.glob("*.zip"))


def test_prune_audio_keeps_named_clips_and_drops_orphan_licences():
    files = [
        {"path": "assets/audio/g/bgm.wav", "url": "assets/audio/g/bgm.wav"},
        {"path": "assets/audio/g/unused.wav", "url": "assets/audio/g/unused.wav"},
        {"path": "assets/audio/other/a.ogg", "url": "assets/audio/other/a.ogg"},
        {"path": "assets/audio/other/LICENSE.txt", "url": "assets/audio/other/LICENSE.txt"},
        {"path": "assets/audio/g/LICENSE.txt", "url": "assets/audio/g/LICENSE.txt"},
        {"path": "assets/models/m.glb", "url": "assets/models/m.glb"},
    ]
    world = {"audio": {"bgm": {"file": "assets/audio/g/bgm.wav"}}}
    kept = [f["path"] for f in pp.prune_audio(files, world)]
    assert kept == ["assets/audio/g/bgm.wav", "assets/audio/g/LICENSE.txt", "assets/models/m.glb"]


def test_portal_index_html_needs_anchors():
    src = ('<!doctype html>\n<html>\n<head>\n  <meta charset="utf-8" />\n'
           '  <meta name="robots" content="noindex" />\n</head>\n<body>\n'
           '<p data-portal="hide"><a href="index.html">all games</a></p>\n</body></html>\n')
    out = pp.portal_index_html(src, "g", "gamemonetize")
    assert 'content="gamemonetize"' in out and "noindex" not in out and "all games" not in out
    with pytest.raises(pp.PackageError):
        pp.portal_index_html(src.replace('<meta charset="utf-8" />', ""), "g", "none")
    with pytest.raises(pp.PackageError):
        pp.portal_index_html(src.replace(' data-portal="hide"', ""), "g", "none")


def measured(initial_raw, initial_gz, total_raw, total_gz):
    return {"initial": {"raw": initial_raw, "gzip": initial_gz}, "total": {"raw": total_raw, "gzip": total_gz}}


def test_budget_warns_on_either_basis():
    mb = pp.MB
    assert pp.budget_warnings(measured(4 * mb, 1 * mb, 7 * mb, 2 * mb)) == []
    w = pp.budget_warnings(measured(6 * mb, 2 * mb, 7 * mb, 2 * mb))
    assert len(w) == 1 and "initial raw" in w[0]
    w = pp.budget_warnings(measured(4 * mb, 1 * mb, 9 * mb, 9 * mb))
    assert len(w) == 2 and all(x.startswith("total") for x in w)


def test_strict_exits_nonzero_over_budget(tmp_path, fake_pkg, capsys):
    code, _ = build(tmp_path, fake_pkg, "--strict", "--initial-budget-mb", "0.01")
    assert code == 1
    assert "WARN" in capsys.readouterr().out
    code, _ = build(tmp_path / "ok", fake_pkg, "--initial-budget-mb", "0.01")
    assert code == 0                       # warns, but only --strict fails


def test_wasm_opt_is_run_when_asked(tmp_path, fake_pkg, capsys):
    tool = tmp_path / "wasm-opt"
    tool.write_text(
        "#!/bin/sh\n# fake: -Oz -o OUT IN\nhead -c 100 \"$4\" > \"$3\"\n", encoding="utf-8")
    tool.chmod(tool.stat().st_mode | stat.S_IXUSR)
    code, out = build(tmp_path, fake_pkg, "--wasm-opt", str(tool))
    assert code == 0
    assert (out / "neon_cat/pkg/kagra_shared_bg.wasm").stat().st_size == 100
    assert "wasm-opt -Oz" in capsys.readouterr().out


def test_missing_wasm_or_tool_is_a_clear_error(tmp_path, capsys):
    code = pp.main(["--pkg", str(tmp_path / "nope"), "--out", str(tmp_path / "o")])
    assert code == 2 and "wasm package missing" in capsys.readouterr().err
    code = pp.main(["--wasm-opt", "definitely-not-a-tool", "--out", str(tmp_path / "o")])
    assert code == 2


def test_unknown_game_is_an_error(tmp_path, fake_pkg, capsys):
    assert pp.main(["--game", "no_such_game", "--pkg", str(fake_pkg), "--out", str(tmp_path / "o")]) == 2

"""Browser preview staging and gh-pages tree updates. No wasm, no network."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

from tests.conftest import ROOT, load_kagra_submodule

dist = load_kagra_submodule("distribute")


def _load(name: str):
    path = ROOT / "scripts" / f"{name}.py"
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


preview = _load("build_browser_preview")
pages = _load("publish_preview_pages")


def test_pages_base_keeps_repo_case():
    assert preview.pages_base("EMMA019", "EMMA019/KAGRA") == "https://emma019.github.io/KAGRA"


def test_preview_links_every_browser_genre(tmp_path: Path):
    catalog = preview.build_preview(ROOT, tmp_path / "site", pkg=None, require_pkg=False)
    games = {g["slug"]: g for g in catalog["games"]}
    assert games["platformer"]["browser"] is True
    assert games["platformer"]["title"] == "Box Hop"
    assert games["platformer"]["href"] == "play.html?game=platformer"
    assert games["neon_run"]["browser"] is True
    assert games["neon_run"]["title"] == "Lumen"
    assert games["neon_run"]["href"] == "play.html?game=neon_run"
    assert catalog["games"][0]["slug"] == "neon_run"
    shipped = list((ROOT / "games").glob("*.json"))
    assert len(games) == len(shipped)
    for path in shipped:
        manifest = dist.load_manifest(path, ROOT)
        got = games[manifest.slug]
        assert got["browser"] is bool(manifest.world)
        if manifest.world:
            assert got["href"] == f"play.html?game={manifest.slug}"
            bundle_path = tmp_path / "site" / "games" / manifest.slug / "bundle.json"
            assert bundle_path.is_file()
            bundle = json.loads(bundle_path.read_text(encoding="utf-8"))
            assert bundle["world"] == "world.json"
            paths = {item["path"] for item in bundle["files"]}
            assert "world.json" in paths
            world = tmp_path / "site" / "games" / manifest.slug / "world.json"
            assert world.is_file()
        else:
            assert got["href"] is None
    index = (tmp_path / "site" / "index.html").read_text(encoding="utf-8")
    assert "play.html?game=platformer" in index
    assert "Box Hop" in index
    assert 'href="play.html?game=neon_run"' in index
    assert index.index("play.html?game=neon_run") < index.index("play.html?game=platformer")
    assert "play.html?game=chess" not in index
    play = (tmp_path / "site" / "play.html").read_text(encoding="utf-8")
    assert "setPlayInput" in play
    assert "data-orient" in play
    assert '"neon_run"' in play
    assert (tmp_path / "site" / ".nojekyll").is_file()
    knight = (
        tmp_path
        / "site"
        / "assets"
        / "library"
        / "kaykit"
        / "adventurers"
        / "characters"
        / "Knight.glb"
    )
    assert knight.is_file()
    staged = [p for p in (tmp_path / "site").rglob("*") if p.is_file()]
    assert len(staged) < 400
    assert sum(p.stat().st_size for p in staged) < 80_000_000


def test_root_publish_keeps_other_prs(tmp_path: Path):
    work = tmp_path / "gh-pages"
    (work / "pr" / "4").mkdir(parents=True)
    (work / "pr" / "4" / "keep.txt").write_text("stay", encoding="utf-8")
    (work / "index.html").write_text("old", encoding="utf-8")
    src = tmp_path / "src"
    src.mkdir()
    (src / "index.html").write_text("new", encoding="utf-8")
    (src / "play.html").write_text("play", encoding="utf-8")
    pages.apply_tree(work, src, "")
    assert (work / "pr" / "4" / "keep.txt").read_text(encoding="utf-8") == "stay"
    assert (work / "index.html").read_text(encoding="utf-8") == "new"
    assert (work / ".nojekyll").is_file()
    pages.apply_tree(work, src, "pr/9")
    assert (work / "pr" / "9" / "play.html").is_file()
    assert (work / "index.html").read_text(encoding="utf-8") == "new"
    pages.delete_tree(work, "pr/9")
    assert not (work / "pr" / "9").exists()
    assert (work / "pr" / "4" / "keep.txt").is_file()


def test_root_seed_skips_only_a_stable_root(tmp_path: Path):
    work = tmp_path / "gh-pages"
    work.mkdir()
    with pytest.raises(SystemExit):
        pages.skip_root_seed(work, "pr/12", True)
    assert pages.skip_root_seed(work, "", False) is False
    (work / "play.html").write_text("catalog", encoding="utf-8")
    (work / "pkg").mkdir()
    (work / "pkg" / "kagra_shared_bg.wasm").write_bytes(b"\0asm")
    assert pages.skip_root_seed(work, "", True) is False
    (work / "world.json").write_text('{"genre":"neon_run"}', encoding="utf-8")
    assert pages.skip_root_seed(work, "", True) is True


def test_pages_and_previews_share_one_tree():
    pages_yml = (ROOT / ".github/workflows/pages.yml").read_text(encoding="utf-8")
    assert "publish_preview_pages.py" in pages_yml
    assert "kagra-pages" in pages_yml
    assert "actions/deploy-pages" not in pages_yml
    assert "actions/upload-pages-artifact" not in pages_yml
    preview_yml = (ROOT / ".github/workflows/preview.yml").read_text(encoding="utf-8")
    assert "branches: [master, main]" not in preview_yml
    assert "--only-if-missing" in preview_yml
    assert "dist/web/pkg" in preview_yml
    assert "bash scripts/build_wasm.sh" in preview_yml
    assert "kagra-pages" in preview_yml
    assert "github.event.pull_request.merged == false" in preview_yml
    deploy_yml = (ROOT / ".github/workflows/pages-deploy.yml").read_text(encoding="utf-8")
    assert "site/play.html" in deploy_yml
    assert "site/pkg/kagra_shared_bg.wasm" in deploy_yml
    assert "site/assets.json" in deploy_yml


def test_dest_rejects_escape():
    assert pages.safe_dest("") == ""
    assert pages.safe_dest("pr/12") == "pr/12"
    with pytest.raises(SystemExit):
        pages.safe_dest("../secrets")
    with pytest.raises(SystemExit):
        pages.safe_dest("pr/12/extra")


def test_dev_shell_is_a_separate_debug_app():
    gradle = (ROOT / "mobile/android/dev/build.gradle.kts").read_text(encoding="utf-8")
    assert 'applicationId = "dev.kagra.devshell"' in gradle
    assert 'namespace = "dev.kagra.shell"' in gradle
    assert '?: "29.0.14206865"' in gradle
    assert "useLegacyPackaging = false" in gradle
    cmake = (ROOT / "mobile/android/dev/src/main/cpp/CMakeLists.txt").read_text(encoding="utf-8")
    assert "-Wl,-z,max-page-size=16384" in cmake
    assert "-Wl,-z,common-page-size=16384" in cmake
    manifest = (ROOT / "mobile/android/dev/src/main/AndroidManifest.xml").read_text(encoding="utf-8")
    assert "android.permission.INTERNET" in manifest
    assert "DevActivity" in manifest
    assert 'android:label="KAGRA Dev"' in manifest
    jni = (
        ROOT / "mobile/android/dev/src/main/cpp" / "../../../../app/src/main/cpp/jni_bridge.cpp"
    ).resolve()
    assert jni.is_file()
    settings = (ROOT / "mobile/android/settings.gradle.kts").read_text(encoding="utf-8")
    assert 'include(":dev")' in settings
    assert "syncSharedShell" in gradle
    assert 'include("dev/kagra/shell/KagraNative.kt")' in gradle
    assert 'include("dev/kagra/shell/AudioBus.kt")' in gradle
    assert "MainActivity.kt" not in gradle
    preview_yml = (ROOT / ".github/workflows/preview.yml").read_text(encoding="utf-8")
    assert "bash scripts/build_wasm.sh" in preview_yml
    assert "kagra-pages" in preview_yml
    assert 'data-orient="portrait"' in preview_yml
    assert "build_type" not in preview_yml
    assert "Deploy from a branch" not in preview_yml
    deploy_yml = (ROOT / ".github/workflows/pages-deploy.yml").read_text(encoding="utf-8")
    assert "actions/upload-pages-artifact" in deploy_yml
    assert "actions/deploy-pages" in deploy_yml
    assert "ref: gh-pages" in deploy_yml
    assert "name: github-pages" in deploy_yml
    assert "id-token: write" in deploy_yml
    assert "kagra-pages" in deploy_yml


def test_pages_remote_keeps_the_token_out_of_logs():
    secret = "ghs_secret"
    url = pages.authed_remote("https://github.com/EMMA019/KAGRA.git", secret)
    assert url == "https://x-access-token:ghs_secret@github.com/EMMA019/KAGRA.git"
    assert pages.redact(f"push {url} failed", secret) == (
        "push https://x-access-token:***@github.com/EMMA019/KAGRA.git failed"
    )
    with pytest.raises(SystemExit):
        pages.authed_remote("git@github.com:EMMA019/KAGRA.git", secret)

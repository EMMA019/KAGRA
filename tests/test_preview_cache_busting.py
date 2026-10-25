"""A deploy never mixes old and new files: one build id, per-asset tags, a reload hint."""

from __future__ import annotations

import importlib.util
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
_spec = importlib.util.spec_from_file_location("bbp", ROOT / "scripts" / "build_browser_preview.py")
preview = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(preview)
PLAY = ROOT / "scripts/preview_site/play.html"


def _site(tmp_path: Path, name: str = "site", wasm: bytes = b"\0asm") -> Path:
    pkg = tmp_path / "pkg"
    pkg.mkdir(exist_ok=True)
    (pkg / "kagra_shared.js").write_text("// js", encoding="utf-8")
    (pkg / "kagra_shared_bg.wasm").write_bytes(wasm)
    preview.build_preview(ROOT, tmp_path / name, pkg=pkg, require_pkg=True)
    return tmp_path / name


def test_the_build_is_stamped_everywhere_and_the_assets_are_tagged(tmp_path: Path):
    site = _site(tmp_path)
    build = json.loads((site / "build.json").read_text(encoding="utf-8"))["build"]
    assert re.fullmatch(r"[0-9a-f]{12}", build)
    play = (site / "play.html").read_text(encoding="utf-8")
    assert "__KAGRA_BUILD__" not in play
    assert f'content="{build}"' in play
    assert f'./pkg/kagra_shared.js?v={build}' in play
    assert f'./i18n.js?v={build}' in play and f'./portal.js?v={build}' in play
    bundle = json.loads((site / "games/hop_beat/bundle.json").read_text(encoding="utf-8"))
    assert bundle["build"] == build
    glbs = [f for f in bundle["files"] if f["path"].endswith(".glb")]
    wavs = [f for f in bundle["files"] if f["path"].endswith(".wav")]
    assert glbs and wavs
    for f in bundle["files"]:
        assert re.fullmatch(r"[0-9a-f]{10}", f["v"]), f
        assert f["v"] == preview.file_tag((site / f["url"]).read_bytes())


def test_the_build_id_follows_the_content_and_is_repeatable(tmp_path: Path):
    a = _site(tmp_path, "a")
    b = _site(tmp_path, "b")
    id_a = (a / "build.json").read_text(encoding="utf-8")
    assert id_a == (b / "build.json").read_text(encoding="utf-8")
    c = _site(tmp_path, "c", wasm=b"\0asm-new")
    assert (c / "build.json").read_text(encoding="utf-8") != id_a


def test_the_page_stays_free_of_external_requests_in_the_cache_block():
    html = PLAY.read_text(encoding="utf-8")
    block = html[html.index("// <kagra-build>") : html.index("// </kagra-build>")]
    assert not re.search(r"https?://", block)
    # the wasm is fetched through the stamped URL, never by import.meta.url alone
    assert 'init({ module_or_path: new URL(vq("./pkg/kagra_shared_bg.wasm")' in html


def test_the_page_reloads_once_then_offers_the_hint():
    node = shutil.which("node")
    if node is None:
        pytest.skip("node is not installed")
    html = PLAY.read_text(encoding="utf-8")
    block = html[html.index("// <kagra-build>") : html.index("// </kagra-build>")]
    script = r"""
const block = process.argv[1];
const run = (build, latest, tried) => {
  const log = { reload: [], hint: 0 };
  const store = tried ? { "kagra.build.tried": latest } : {};
  const el = { hidden: true, onclick: null };
  const doc = {
    querySelector: () => ({ content: build }),
    getElementById: () => el,
    addEventListener() {},
    hidden: false,
  };
  const fn = new Function("document", "fetch", "location", "sessionStorage", "AbortController", "URL", "setTimeout", "clearTimeout", "fileUrl",
    block + "; return { checkBuild, vq, assetUrl, buildOn };");
  const api = fn(doc, async () => ({ ok: true, json: async () => ({ build: latest }) }),
    { href: "https://x.test/pr/1/play.html?game=hop_beat", replace: (u) => log.reload.push(u) },
    { getItem: (k) => store[k] ?? null, setItem: (k, v) => { store[k] = v; } },
    AbortController, URL, setTimeout, clearTimeout, (b, r) => b + r);
  return api.checkBuild(true).then((r) => ({ r, hint: !el.hidden, reload: log.reload, vq: api.vq("a.js"),
    asset: api.assetUrl("/s/", { url: "m.glb", v: "0123456789" }) }));
};
(async () => {
  const same = await run("aaaaaaaaaaaa", "aaaaaaaaaaaa", false);
  const stale = await run("aaaaaaaaaaaa", "bbbbbbbbbbbb", false);
  const again = await run("aaaaaaaaaaaa", "bbbbbbbbbbbb", true);
  const dev = await run("__KAGRA_BUILD__", "bbbbbbbbbbbb", false);
  console.log(JSON.stringify({ same, stale, again, dev }));
})();
"""
    done = subprocess.run([node, "-e", script, block], capture_output=True, text=True, timeout=30)
    assert done.returncode == 0, done.stderr
    got = json.loads(done.stdout)
    assert got["same"]["r"] is False and not got["same"]["hint"] and not got["same"]["reload"]
    assert got["same"]["vq"] == "a.js?v=aaaaaaaaaaaa"
    assert got["same"]["asset"] == "/s/m.glb?v=0123456789"
    assert got["stale"]["r"] is True and len(got["stale"]["reload"]) == 1
    assert "_b=bbbbbbbbbbbb" in got["stale"]["reload"][0]
    assert got["again"]["r"] is False and got["again"]["hint"] and not got["again"]["reload"]
    assert got["dev"]["r"] is False and not got["dev"]["hint"] and got["dev"]["vq"] == "a.js"

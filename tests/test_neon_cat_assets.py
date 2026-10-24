"""Neon Cat asset manifest. No native extension."""
from __future__ import annotations

import json

from tests.conftest import ROOT

ALLOWED = {"CC0-1.0", "CC-BY-4.0", "CC-BY-3.0"}


def test_fetch_manifest_is_permissive_and_files_exist():
    spec = json.loads((ROOT / "tools/fetch_assets/neon_cat.json").read_text(encoding="utf-8"))
    glbs = []
    for pack in spec["packs"]:
        assert pack["license"] in ALLOWED
        assert str(pack.get("source_url", "")).startswith("https://")
        dest = ROOT / pack["dest"]
        assert (dest / "CREDITS.md").is_file()
        for item in pack["files"]:
            path = dest / item["out"]
            assert path.is_file(), path
            if path.suffix.lower() == ".glb":
                glbs.append(path)
    assert len(glbs) >= 6


def test_game_manifest_bundles_the_glbs():
    manifest = json.loads((ROOT / "games/neon_cat_minimal.json").read_text(encoding="utf-8"))
    bundled = set(manifest["bundle_assets"])
    for rel in (
        "assets/models/kenney/cube-pets/animal-cat.glb",
        "assets/models/kenney/platformer-kit/crate.glb",
        "assets/models/kenney/platformer-kit/barrel.glb",
        "assets/models/kenney/racing-kit/overheadRound.glb",
        "assets/models/kenney/racing-kit/barrierRed.glb",
        "assets/models/neon_cat/gate_arch.glb",
    ):
        assert rel in bundled
        assert (ROOT / rel).is_file()
    assert (ROOT / manifest["icon"]).is_file()
    assert (ROOT / manifest["world"]).is_file()

"""Lumen helpers that do not need the native extension."""
from __future__ import annotations

import json
import sys

from kagra.juice import apply_look, apply_post
from kagra.particles import Emitter
from tests.conftest import ROOT, load_kagra_submodule

dist = load_kagra_submodule("distribute")


def test_night_look_scales_lights_and_sets_cull():
    world = {
        "lights": [{"id": "light:key", "type": "light", "intensity": 2.0, "color": [1, 1, 1]}],
    }
    apply_look(world, "night")
    assert world["lut"] == "cool"
    assert world["env_map"] == "preset:studio"
    assert world["lights"][0]["intensity"] < 2.0
    world["cull"] = True
    world["prop_lod"] = 80.0
    assert world["cull"] is True


def test_emitter_emissive_is_optional():
    plain = Emitter(max_count=4, seed=1)
    plain.burst((0, 0, 0), count=2, life=1.0)
    plain.tick(0.01)
    assert "emissive" not in plain.props()[0]
    glow = Emitter(max_count=4, seed=1, emissive=2.5)
    glow.burst((0, 0, 0), count=1, life=1.0)
    glow.tick(0.01)
    assert glow.props()[0]["emissive"] > 1.0


def test_neon_post_preset_is_hot():
    world = apply_post({}, "neon")
    assert world["bloom"] > 0.7
    assert world["bloom_threshold"] < 0.5
    assert world["lut"] == "punch"
    assert world["fxaa"] is True
    assert world["vignette"] > 1.0


def test_package_web_stages_lumen_without_wasm(tmp_path):
    import subprocess

    out = tmp_path / "web"
    proc = subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts" / "package_web.py"),
            "--game",
            "examples/neon_run_minimal.py",
            "--out",
            str(out),
            "--skip-wasm",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    assert proc.returncode == 0, proc.stderr
    assert (out / "play.html").is_file()
    assert b"play.html" in (out / "index.html").read_bytes()
    assert b"CORRIDOR" in (out / "corridor.html").read_bytes()
    world = json.loads((out / "world.json").read_text(encoding="utf-8"))
    assert world["genre"] == "neon_run"
    assert (out / "audio" / "bgm.wav").is_file()
    assert (out / "audio" / "card.wav").is_file()
    page = (out / "play.html").read_text(encoding="utf-8")
    assert "loadGameJson" in page
    assert "setPlayInput" in page


def test_lumen_manifest_is_packaged():
    manifest = dist.manifest_for_game(ROOT, "examples/neon_run_minimal.py")
    assert manifest.title == "Lumen"
    assert manifest.entry == "examples/neon_run_minimal.py"
    assert manifest.world == "kagra-shared/tests/fixtures/neon_run_world.json"
    assert manifest.slug == "neon_run"
    assert manifest.application_id == "dev.kagra.neon_run"
    dump = json.loads((ROOT / manifest.world).read_text(encoding="utf-8"))
    assert dump["genre"] == "neon_run"
    assert dump["version"] == 1

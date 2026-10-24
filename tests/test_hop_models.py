"""tools/gen_assets/hop_models.py: deterministic, committed files match, GLB headers parse."""
from __future__ import annotations

import importlib.util
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
_spec = importlib.util.spec_from_file_location("hop_models", ROOT / "tools" / "gen_assets" / "hop_models.py")
assert _spec is not None and _spec.loader is not None
hm = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(hm)


def test_generator_is_deterministic_and_matches_the_committed_files():
    a, b = hm.build(), hm.build()
    assert a == b
    assert len(a) == 11
    for path, data in a.items():
        assert path.is_file(), path
        assert path.read_bytes() == data, f"{path.name} drifted: run tools/gen_assets/hop_models.py"
    assert hm.main(["--check"]) == 0


def test_every_glb_is_valid_and_small():
    total = 0
    for path, data in hm.build().items():
        magic, version, length = struct.unpack("<4sII", data[:12])
        assert magic == b"glTF" and version == 2 and length == len(data)
        n, tag = struct.unpack("<I4s", data[12:20])
        assert tag == b"JSON"
        doc = json.loads(data[20:20 + n])
        assert doc["asset"]["version"] == "2.0"
        assert doc["meshes"], path.name
        assert len(data) < 200_000, path.name
        total += len(data)
    assert total < 900_000


def test_the_glbs_are_in_the_manifest_bundle_and_the_ledger():
    manifest = json.loads((ROOT / "games" / "hop_beat_minimal.json").read_text())
    assert "assets/models/hop_beat" in manifest["bundle_assets"]
    ledger = json.loads((ROOT / "assets" / "LEDGER.json").read_text())
    entry = next(p for p in ledger["packs"] if p["id"] == "project-hop-beat-models")
    assert entry["license"] == "cc0" and entry["redistributable_in_public_repo"] is True
    assert "assets/models/hop_beat/*.glb" in entry["paths"]


def test_the_three_enemy_models_exist_stand_on_the_ground_and_are_small():
    built = {p.name: d for p, d in hm.build().items()}
    for name in ("hop_slime.glb", "hop_beetle.glb", "hop_bat.glb"):
        data = built[name]
        n = struct.unpack("<I", data[12:16])[0]
        doc = json.loads(data[20:20 + n])
        for acc in doc["accessors"]:
            if acc.get("type") == "VEC3" and "min" in acc and "max" in acc and acc["max"][1] > 0.1:
                lo, hi = acc["min"], acc["max"]
                assert lo[1] > -0.05, f"{name} sinks below its base"
                # a bat's wingspan is the widest; none is wider than the 3 m beat
                assert hi[1] - lo[1] < 1.4 and hi[0] - lo[0] < 2.0 and hi[2] - lo[2] < 2.0, name

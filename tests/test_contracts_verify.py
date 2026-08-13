"""contracts / verify / touch の単体テスト（GPU 不要）。"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from tests.conftest import load_kagra_submodule

# `import kagra` は Rust 拡張を要求するため、純ロジックのモジュールだけを直接ロードする。
_contracts = load_kagra_submodule("contracts")
_touch = load_kagra_submodule("touch")
_verify = load_kagra_submodule("verify")

AssetKind = _contracts.AssetKind
KagraContractError = _contracts.KagraContractError
candidate_paths = _contracts.candidate_paths
describe_environment = _contracts.describe_environment
resolve_asset = _contracts.resolve_asset

PointerEvent = _touch.PointerEvent
PointerPhase = _touch.PointerPhase
VirtualPad = _touch.VirtualPad

_load_scenario = _verify._load_scenario
load_scenario = _verify.load_scenario


ROOT = Path(__file__).resolve().parents[1]


def test_describe_environment():
    env = describe_environment(ROOT)
    assert env["root"]
    assert "aliases" in env


def test_resolve_walk_fixture():
    # synthetic_walk はリポジトリに含まれる
    p = resolve_asset(AssetKind.BVH, "walk", root=ROOT)
    assert p.exists()
    assert p.suffix.lower() == ".bvh"


def test_resolve_missing_raises_structured():
    with pytest.raises(KagraContractError) as ei:
        resolve_asset(AssetKind.VRM, "definitely_missing_xyz", root=ROOT)
    err = ei.value
    assert err.code == "ASSET_NOT_FOUND"
    assert err.hint
    d = err.to_dict()
    assert "candidates" in d


def test_candidate_paths_ordered():
    cands = candidate_paths(AssetKind.VRM, "Emma", root=ROOT)
    assert any("Emma" in str(c) for c in cands)


def test_virtual_pad_wasd():
    pad = VirtualPad(deadzone=0.2)
    pad.set_stick(0.9, 0.0)
    assert pad.desired_keys() == {"D"}
    ev = list(pad.key_events())
    assert ("D", True) in ev
    pad.set_stick(0.0, 0.0)
    ev2 = list(pad.key_events())
    assert ("D", False) in ev2


def test_pointer_json_roundtrip_shape():
    e = PointerEvent(0, 10, 20, PointerPhase.BEGIN)
    assert e.phase.value == "begin"


def test_load_blank_scenario():
    sc = load_scenario(ROOT / "examples/verify_scenarios/blank_smoke.json")
    assert sc.name == "blank_smoke"
    assert sc.inline is not None
    assert sc.expect


def test_load_scenario_dict():
    sc = _load_scenario(
        {
            "name": "x",
            "script": "scratch/smoke_orb_rush.py",
            "expect_files": ["a.png"],
            "min_file_bytes": 10,
        }
    )
    assert sc.script.endswith("smoke_orb_rush.py")
    assert sc.expect[0].min_bytes == 10

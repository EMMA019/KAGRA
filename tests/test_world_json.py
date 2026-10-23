"""World JSON checks. Skipped unless pydantic is installed.

The default python-unit job does not install it. The world-json-checks job
does. pydantic is not part of the wheel, and this test does not import the
Rust extension.
"""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest

pytest.importorskip("pydantic")

ROOT = Path(__file__).resolve().parents[1]
CREST = ROOT / "kagra-shared" / "tests" / "fixtures" / "crest_isle_world.json"


def _checker():
    path = ROOT / "tools" / "check_world_json.py"
    spec = importlib.util.spec_from_file_location("check_world_json", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_crest_isle_matches_the_schema():
    data = json.loads(CREST.read_text(encoding="utf-8"))
    assert _checker().check_world(data) == []


def test_misspelled_cascade_suggests_the_schema_name():
    errors = _checker().check_world({"version": 1, "shadow_cascade": 2})
    assert any(
        line.startswith("shadow_cascade:") and "shadow_cascades" in line for line in errors
    )


def test_prop_typo_suggests_position():
    errors = _checker().check_world(
        {
            "version": 1,
            "props": [{"id": "prop:1", "type": "prop", "positon": [0, 0, 0]}],
        }
    )
    assert any("props[0].positon: unknown field; did you mean position?" == line for line in errors)


def test_coins_string_names_integer():
    errors = _checker().check_world({"version": 1, "coins": "3"})
    assert "coins: expected integer, got str" in errors


def test_missing_version():
    errors = _checker().check_world({})
    assert "version: missing required field" in errors


def test_checker_does_not_import_the_extension():
    assert "kagra.kagra_shared" not in sys.modules
    assert _checker().check_world({"version": 1}) == []
    assert "kagra.kagra_shared" not in sys.modules

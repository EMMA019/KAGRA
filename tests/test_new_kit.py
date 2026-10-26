"""scripts/new_kit.py: a new game from the Starter template, in a scratch copy of the files."""
from __future__ import annotations

import importlib.util
import json
import shutil

import pytest

from tests.conftest import ROOT

FILES = (
    "kagra-shared/src/starter.rs",
    "kagra-shared/src/lib.rs",
    "kagra-shared/src/kit_host.rs",
    "kagra-shared/tests/fixtures/starter_world.json",
    "games/starter_minimal.json",
    "examples/starter_minimal.py",
)


def _new_kit():
    spec = importlib.util.spec_from_file_location("new_kit", ROOT / "scripts/new_kit.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


@pytest.fixture()
def scratch(tmp_path):
    for rel in FILES:
        (tmp_path / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / rel, tmp_path / rel)
    return tmp_path


def test_a_new_kit_is_renamed_registered_and_shipped(scratch):
    nk = _new_kit()
    written = nk.make(scratch, "lantern_run", "Lantern Run")
    assert len(written) == 6
    kit = (scratch / "kagra-shared/src/lantern_run.rs").read_text(encoding="utf-8")
    assert "pub struct LanternRun" in kit and "pub struct LanternRunSpec" in kit
    assert 'pub const GAME_ID: &str = "lantern_run";' in kit
    assert "pub fn is_lantern_run_dump" in kit
    assert 'title: "Lantern Run".into()' in kit
    code = kit.split("\nuse serde", 1)[1]
    assert "Starter" not in code and "starter" not in code, "no template name left in the code"
    lib = (scratch / "kagra-shared/src/lib.rs").read_text(encoding="utf-8")
    assert "pub mod lantern_run;\n" in lib and "pub mod starter;\n" in lib
    mods = [line for line in lib.splitlines() if line.startswith("pub mod ")]
    assert mods.index("pub mod lantern_run;") < mods.index("pub mod starter;")
    kh = (scratch / "kagra-shared/src/kit_host.rs").read_text(encoding="utf-8")
    assert "crate::lantern_run::LanternRun::from_json(json)?" in kh
    assert kh.index("lantern_run") < kh.index(nk.KITS_MARK)
    world = json.loads((scratch / "kagra-shared/tests/fixtures/lantern_run_world.json").read_text(encoding="utf-8"))
    assert world["genre"] == "lantern_run" and world["game"]["id"] == "lantern_run"
    assert world["game"]["title"] == "Lantern Run"
    man = json.loads((scratch / "games/lantern_run_minimal.json").read_text(encoding="utf-8"))
    assert man["entry"] == "examples/lantern_run_minimal.py" and man["page"]["fixed_step"]
    ex = (scratch / "examples/lantern_run_minimal.py").read_text(encoding="utf-8")
    assert 'GAME_ID = "lantern_run"' in ex


def test_it_refuses_a_bad_slug_or_an_existing_kit(scratch):
    nk = _new_kit()
    for bad in ("Lantern", "1run", "starter", "a"):
        with pytest.raises(SystemExit):
            nk.make(scratch, bad, "X")
    nk.make(scratch, "lantern_run", "Lantern Run")
    with pytest.raises(SystemExit):
        nk.make(scratch, "lantern_run", "Again")


def test_the_template_ships_in_the_preview():
    man = json.loads((ROOT / "games/starter_minimal.json").read_text(encoding="utf-8"))
    assert man["world"] == "kagra-shared/tests/fixtures/starter_world.json"
    assert man["page"]["blurb"] == "blurb.stick"

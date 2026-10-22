"""Runtime checks: mainline ``import kagra`` is WorldDoc / WorldPlay.

Archived RendererV2 names (Walk / Prop / stage / avatar) stay off the
public package. Submodule collisions that remain (annotate / brain) must
still be callables, not the ``kagra/*.py`` modules.
"""
from __future__ import annotations

import ast
import json
import os
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
INIT = ROOT / "kagra" / "__init__.py"
OPEN_WORLD = ROOT / "old" / "examples" / "vrm_open_world.py"
RELIC_RUN = ROOT / "old" / "examples" / "vrm_relic_run.py"

MAINLINE_NAMES = (
    "WorldDoc",
    "WorldPlay",
    "Scene",
    "annotate",
    "brain",
    "choice_menu",
    "draw_world",
    "eval_world_expect",
    "image",
    "merge",
    "play_se",
    "play_wav",
    "pressed",
    "run",
    "run_scenario",
    "tone",
)

CUT_NAMES = (
    "ActionController",
    "Label",
    "Prop",
    "Walk",
    "CharacterController",
    "World",
    "World3D",
    "avatar",
    "get_engine",
    "stage",
    "sky",
    "water",
    "texture_from_fn",
)


def _submodule_stems() -> set[str]:
    stems = {p.stem for p in (ROOT / "kagra").glob("*.py") if p.stem != "__init__"}
    stems |= {
        p.name
        for p in (ROOT / "kagra").iterdir()
        if p.is_dir() and (p / "__init__.py").is_file()
    }
    return stems


def _init_names_matching_submodules() -> list[str]:
    """``from kagra.annotate import annotate`` — names that collide."""
    tree = ast.parse(INIT.read_text(encoding="utf-8"))
    sub = _submodule_stems()
    names: list[str] = []
    seen: set[str] = set()

    def add(name: str) -> None:
        if name in sub and name not in seen and not name.startswith("_"):
            seen.add(name)
            names.append(name)

    for node in tree.body:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            add(node.name)
        elif isinstance(node, ast.ImportFrom) and node.module:
            parts = node.module.split(".")
            if len(parts) >= 2 and parts[0] == "kagra":
                sub_name = parts[1]
                for alias in node.names:
                    bind = alias.asname or alias.name
                    if bind == sub_name:
                        add(bind)
    return names


def _kagra_calls_in_method(path: Path, class_name: str, method: str) -> list[ast.Call]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    target = None
    for node in tree.body:
        if isinstance(node, ast.ClassDef) and node.name == class_name:
            for item in node.body:
                if isinstance(item, ast.FunctionDef) and item.name == method:
                    target = item
                    break
    assert target is not None, f"{path.name}: {class_name}.{method} missing"
    calls: list[ast.Call] = []

    class V(ast.NodeVisitor):
        def visit_Call(self, node: ast.Call) -> None:
            fn = node.func
            if (
                isinstance(fn, ast.Attribute)
                and isinstance(fn.value, ast.Name)
                and fn.value.id == "kagra"
            ):
                calls.append(node)
            self.generic_visit(node)

    V().visit(target)
    return calls


# Re-exported from the native module. Without that module ``__init__``
# sets them to None on purpose. Every other mainline name is pure Python.
_NATIVE_MAINLINE = ("WorldDoc", "WorldPlay")

_NATIVE_ABSENT_REASON = (
    "kagra_shared is not installed (no kagra.kagra_shared extension and no "
    "top-level kagra_shared), so kagra.WorldDoc and kagra.WorldPlay are None "
    "stubs. python-unit does not build the wheel. Cut names, annotate/brain, "
    "and the pure-Python mainline names are still checked. "
    "`maturin develop` or the CI build job (installed wheel) checks the "
    "native re-export."
)

_RUNTIME_CHECKER = textwrap.dedent(
    r"""
    import importlib.util
    import json
    import sys
    import types
    from pathlib import Path

    root = Path(sys.argv[1])
    sys.path.insert(0, str(root))

    import kagra

    errors = []
    mainline = json.loads(sys.argv[2])
    cut = json.loads(sys.argv[3])
    native_names = set(json.loads(sys.argv[4]))
    require_native = sys.argv[5] == "1"

    def fail(msg):
        errors.append(msg)

    def native_state():
        # "absent" only when neither import path exists. A module file that
        # fails to load is "broken" and must fail the test, not skip.
        # find_spec raises only when a parent import fails; a missing
        # submodule returns None.
        load_errors = []
        found = False
        for mod_name in ("kagra.kagra_shared", "kagra_shared"):
            try:
                spec = importlib.util.find_spec(mod_name)
            except ValueError as exc:
                load_errors.append(f"{mod_name}: {exc}")
                continue
            if spec is None:
                continue
            found = True
            try:
                __import__(mod_name)
                return "present", ""
            except ImportError as exc:
                return "broken", f"{mod_name}: {exc}"
        if load_errors and not found:
            return "broken", "; ".join(load_errors)
        return "absent", ""

    state, detail = native_state()
    if state == "broken":
        fail(
            "kagra_shared is present but failed to import: "
            + detail
            + " (WorldDoc / WorldPlay must not be skipped)"
        )
    elif state == "absent" and require_native:
        sys.stdout.write("NATIVE_ABSENT\n")
        sys.exit(2)

    for name in mainline:
        obj = getattr(kagra, name, None)
        if obj is None:
            if name in native_names and state == "absent" and not require_native:
                continue
            fail(f"missing kagra.{name}")
        elif isinstance(obj, types.ModuleType):
            fail(f"kagra.{name} is a module ({getattr(obj, '__file__', '?')})")
        elif name in native_names and state == "present" and not callable(
            getattr(obj, "from_json", None)
        ):
            fail(f"kagra.{name} has no from_json")

    for name in cut:
        if hasattr(kagra, name) and name in getattr(kagra, "__all__", ()):
            fail(f"cut name still public: kagra.{name}")

    from kagra.annotate import annotate as annotate_impl
    if not callable(kagra.annotate) or isinstance(kagra.annotate, types.ModuleType):
        fail("from kagra.annotate import annotate shadowed kagra.annotate")
    if not callable(annotate_impl):
        fail("kagra.annotate.annotate missing")

    brain_mod = __import__("kagra.brain", fromlist=["Brain"])
    if not callable(kagra.brain) or isinstance(kagra.brain, types.ModuleType):
        fail("import kagra.brain shadowed kagra.brain")
    if not hasattr(brain_mod, "Brain"):
        fail("kagra.brain module missing Brain")

    if errors:
        sys.stderr.write("\n".join(errors) + "\n")
        sys.exit(1)
    if state == "absent":
        sys.stdout.write("NATIVE_ABSENT\n")
    """
).strip()


def _run_runtime_checker(*, require_native: bool) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
    return subprocess.run(
        [
            sys.executable,
            "-c",
            _RUNTIME_CHECKER,
            str(ROOT),
            json.dumps(list(MAINLINE_NAMES)),
            json.dumps(list(CUT_NAMES)),
            json.dumps(list(_NATIVE_MAINLINE)),
            "1" if require_native else "0",
        ],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )


def test_mainline_import_is_not_old_engine():
    """Public surface is WorldDoc / WorldPlay, not the archived engine.

    Cut names, annotate/brain, and pure-Python exports are always checked.
    ``WorldDoc`` / ``WorldPlay`` may be None only when the native module is
    genuinely absent. A present-but-broken extension still fails.
    """
    proc = _run_runtime_checker(require_native=False)
    assert proc.returncode == 0, proc.stdout + proc.stderr


def test_worlddoc_worldplay_follow_the_native_module():
    """When the wheel is installed, WorldDoc / WorldPlay are real types.

    python-unit does not build the wheel. Skip only in that case, with a
    reason. The CI build job imports the installed wheel and asserts both.
    """
    proc = _run_runtime_checker(require_native=True)
    if proc.returncode == 2 and "NATIVE_ABSENT" in proc.stdout:
        pytest.skip(_NATIVE_ABSENT_REASON)
    assert proc.returncode == 0, proc.stdout + proc.stderr


def test_collision_names_are_detected():
    names = _init_names_matching_submodules()
    assert "annotate" in names
    assert "brain" in names
    assert "stage" not in names
    assert "pad" not in names
    assert "look" not in names
    assert "play" not in names
    assert "camera3d" not in names
    assert "world3d" not in names


def test_crest_isle_on_enter_still_uses_archived_stage():
    """Archived Crest Isle source still calls the old ``kagra.stage``."""
    calls = _kagra_calls_in_method(OPEN_WORLD, "CrestIsle", "on_enter")
    stage_calls = [
        c
        for c in calls
        if isinstance(c.func, ast.Attribute) and c.func.attr == "stage"
    ]
    assert stage_calls, "CrestIsle.on_enter no longer calls kagra.stage"
    keywords = {k.arg for c in stage_calls for k in c.keywords if k.arg}
    assert "radius" in keywords
    text = OPEN_WORLD.read_text(encoding="utf-8")
    assert "from kagra.stage import stage" not in text
    assert "kagra.stage.stage" not in text


def test_relic_run_on_enter_still_uses_archived_stage():
    calls = _kagra_calls_in_method(RELIC_RUN, "RelicRun", "on_enter")
    stage_calls = [
        c
        for c in calls
        if isinstance(c.func, ast.Attribute) and c.func.attr == "stage"
    ]
    assert stage_calls, "RelicRun.on_enter no longer calls kagra.stage"
    keywords = {k.arg for c in stage_calls for k in c.keywords if k.arg}
    assert "radius" in keywords
    text = RELIC_RUN.read_text(encoding="utf-8")
    assert "from kagra.stage import stage" not in text
    assert "kagra.stage.stage" not in text

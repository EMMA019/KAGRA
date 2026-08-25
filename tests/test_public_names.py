"""Runtime checks: documented ``kagra.X`` must not be a same-named submodule.

Emma's Crest Isle crash after #77::

    kagra.stage(str(sky_png), radius=140.0)
    TypeError: 'module' object is not callable

``def stage`` lives in ``kagra/__init__.py``, but ``from kagra.stage import Stage``
rebinds ``kagra.stage`` to ``kagra/stage.py``. AST / source-order tests still
see ``def stage`` and miss this. These tests import the package (stub Engine,
no GPU / Rust) and fail if the name is a module.

``from kagra.stage import Stage`` must keep working.
"""
from __future__ import annotations

import ast
import json
import os
import subprocess
import sys
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INIT = ROOT / "kagra" / "__init__.py"
OPEN_WORLD = ROOT / "examples" / "vrm_open_world.py"
RELIC_RUN = ROOT / "examples" / "vrm_relic_run.py"

# Public functions Crest Isle / Relic Run call as ``kagra.X(...)`` (on_enter
# plus the helpers on_enter invokes). Classes too.
DEMO_CALLABLES = (
    "ActionController",
    "Label",
    "Prop",
    "Walk",
    "World3D",
    "apply_outdoor_look",
    "avatar",
    "can_pick",
    "draw_billboard_instances",
    "draw_vignette",
    "draw_vrm",
    "ensure_vrm",
    "fill",
    "font",
    "inject_key",
    "load",
    "load_json",
    "measure",
    "open_world_height",
    "overworld_height",
    "play_se",
    "pressed",
    "quit",
    "save_json",
    "screenshot",
    "set_bloom",
    "set_camera3d",
    "set_fog",
    "set_hdri",
    "set_light_dir",
    "set_point_light",
    "set_spot_light",
    "sky",
    "solid_tex",
    "sound",
    "stage",
    "text",
    "texture_from_fn",
    "tick_count",
    "tone",
    "water",
)

# Same-named submodules that are documented as functions (not Camera3D vs camera3d).
COLLISION_CALLABLES = ("stage", "annotate", "pad", "brain")


def _submodule_stems() -> set[str]:
    stems = {p.stem for p in (ROOT / "kagra").glob("*.py") if p.stem != "__init__"}
    stems |= {
        p.name
        for p in (ROOT / "kagra").iterdir()
        if p.is_dir() and (p / "__init__.py").is_file()
    }
    return stems


def _init_names_matching_submodules() -> list[str]:
    """``def stage`` / ``from kagra.pad import pad`` — names that collide."""
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


_RUNTIME_CHECKER = textwrap.dedent(
    r"""
    import inspect
    import importlib
    import json
    import sys
    import types
    from pathlib import Path

    root = Path(sys.argv[1])
    sys.path.insert(0, str(root))

    core = types.ModuleType("kagra.kagra_core")

    class Engine:
        pass

    core.Engine = Engine
    sys.modules["kagra.kagra_core"] = core

    import kagra

    errors = []
    names = json.loads(sys.argv[2])
    collisions = json.loads(sys.argv[3])

    def fail(msg):
        errors.append(msg)

    def must_callable(name):
        obj = getattr(kagra, name, None)
        if obj is None:
            fail(f"missing kagra.{name}")
            return None
        if isinstance(obj, types.ModuleType):
            fail(f"kagra.{name} is a module ({getattr(obj, '__file__', '?')})")
            return None
        if not callable(obj):
            fail(f"kagra.{name} is {type(obj).__name__}, not callable")
            return None
        return obj

    for name in names:
        must_callable(name)

    for name in collisions:
        must_callable(name)

    # The exact crash: kagra.stage must accept a PNG path + radius.
    stage = getattr(kagra, "stage", None)
    if callable(stage) and not isinstance(stage, types.ModuleType):
        try:
            inspect.signature(stage).bind("sky.png", radius=140.0)
        except TypeError as exc:
            fail(f"kagra.stage bind failed: {exc}")
    else:
        fail("kagra.stage is not callable (Crest Isle / Relic Run sky sphere)")

    from kagra.stage import Stage
    if not isinstance(Stage, type):
        fail(f"from kagra.stage import Stage gave {Stage!r}")
    if not callable(kagra.stage) or isinstance(kagra.stage, types.ModuleType):
        fail("from kagra.stage import Stage re-shadowed kagra.stage")

    # ``import kagra.stage as m`` follows getattr(kagra, "stage") — the
    # callable. The module itself stays in sys.modules for from-import.
    stage_mod = importlib.import_module("kagra.stage")
    if stage_mod.Stage is not Stage:
        fail("importlib.import_module('kagra.stage') did not yield Stage")
    if not callable(kagra.stage) or isinstance(kagra.stage, types.ModuleType):
        fail("loading kagra.stage re-shadowed the callable")

    from kagra.pad import axis
    if not callable(kagra.pad) or isinstance(kagra.pad, types.ModuleType):
        fail("from kagra.pad import axis shadowed kagra.pad")

    brain_mod = importlib.import_module("kagra.brain")
    if not callable(kagra.brain) or isinstance(kagra.brain, types.ModuleType):
        fail("import kagra.brain shadowed kagra.brain")
    if not hasattr(brain_mod, "Brain"):
        fail("kagra.brain module missing Brain")

    from kagra.annotate import annotate as annotate_impl
    if not callable(kagra.annotate) or isinstance(kagra.annotate, types.ModuleType):
        fail("from kagra.annotate import annotate shadowed kagra.annotate")
    if not callable(annotate_impl):
        fail("kagra.annotate.annotate missing")

    # Demo on_enter kwargs must match public signatures (next TypeError).
    calls = json.loads(sys.argv[4])
    for rec in calls:
        name = rec["name"]
        obj = getattr(kagra, name, None)
        if obj is None:
            fail(f"demo calls kagra.{name} but it is missing")
            continue
        if isinstance(obj, types.ModuleType):
            fail(f"demo calls kagra.{name}(...) but it is a module")
            continue
        target = obj
        try:
            sig = inspect.signature(target)
        except (TypeError, ValueError):
            target = getattr(obj, "__init__", obj)
            try:
                sig = inspect.signature(target)
            except (TypeError, ValueError) as exc:
                fail(f"kagra.{name} has no signature: {exc}")
                continue
        params = list(sig.parameters)
        args = [None] * rec["nargs"]
        kwargs = {k: None for k in rec["keywords"]}
        # Class __init__ still lists self.
        if params and params[0] == "self":
            args = [None, *args]
        try:
            sig.bind(*args, **kwargs)
        except TypeError as exc:
            fail(
                f"{rec['where']} kagra.{name}({rec['nargs']} pos, {rec['keywords']}) "
                f"does not bind: {exc}"
            )

    # Camera3D.follow / World3D.set_height_fn used in on_enter.
    extra = json.loads(sys.argv[5])
    for rec in extra:
        cls = getattr(kagra, rec["cls"])
        method = getattr(cls, rec["method"])
        sig = inspect.signature(method)
        args = [None] * rec["nargs"]
        kwargs = {k: None for k in rec["keywords"]}
        params = list(sig.parameters)
        if params and params[0] == "self":
            args = [None, *args]
        try:
            sig.bind(*args, **kwargs)
        except TypeError as exc:
            fail(
                f"{rec['where']} {rec['cls']}.{rec['method']} "
                f"({rec['nargs']} pos, {rec['keywords']}) does not bind: {exc}"
            )

    if errors:
        sys.stderr.write("\n".join(errors) + "\n")
        sys.exit(1)
    """
).strip()


def _call_record(call: ast.Call, where: str) -> dict:
    assert isinstance(call.func, ast.Attribute)
    return {
        "where": where,
        "name": call.func.attr,
        "nargs": len(call.args),
        "keywords": [k.arg for k in call.keywords if k.arg],
    }


def _method_call_records(path: Path, class_name: str, method: str) -> list[dict]:
    where = f"{path.name}:{class_name}.{method}"
    return [_call_record(c, where) for c in _kagra_calls_in_method(path, class_name, method)]


def _follow_and_height_records() -> list[dict]:
    """self.cam.follow / world.set_height_fn — next crash after stage()."""
    records = []
    for path, cls, method in (
        (OPEN_WORLD, "CrestIsle", "on_enter"),
        (RELIC_RUN, "RelicRun", "on_enter"),
    ):
        tree = ast.parse(path.read_text(encoding="utf-8"))
        target = None
        for node in tree.body:
            if isinstance(node, ast.ClassDef) and node.name == cls:
                for item in node.body:
                    if isinstance(item, ast.FunctionDef) and item.name == method:
                        target = item
        assert target is not None
        where = f"{path.name}:{cls}.{method}"

        class V(ast.NodeVisitor):
            def visit_Call(self, node: ast.Call) -> None:
                fn = node.func
                if isinstance(fn, ast.Attribute):
                    rec = {
                        "where": where,
                        "method": fn.attr,
                        "nargs": len(node.args),
                        "keywords": [k.arg for k in node.keywords if k.arg],
                    }
                    if fn.attr == "follow":
                        rec["cls"] = "Camera3D"
                        records.append(rec)
                    elif fn.attr == "set_height_fn":
                        rec["cls"] = "World3D"
                        records.append(rec)
                self.generic_visit(node)

        V().visit(target)
    return records


def _run_runtime_checker() -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
    calls = _method_call_records(OPEN_WORLD, "CrestIsle", "on_enter")
    calls += _method_call_records(RELIC_RUN, "RelicRun", "on_enter")
    extra = _follow_and_height_records()
    return subprocess.run(
        [
            sys.executable,
            "-c",
            _RUNTIME_CHECKER,
            str(ROOT),
            json.dumps(list(DEMO_CALLABLES)),
            json.dumps(list(_init_names_matching_submodules() or COLLISION_CALLABLES)),
            json.dumps(calls),
            json.dumps(extra),
        ],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
        env=env,
        check=False,
    )


def test_stage_is_not_a_module_at_runtime():
    """Fails if ``kagra.stage`` is ``kagra/stage.py``. Not a source-order test."""
    proc = _run_runtime_checker()
    assert proc.returncode == 0, proc.stdout + proc.stderr


def test_collision_names_are_detected():
    """Audit: every public def/import that shares a submodule name is checked."""
    names = _init_names_matching_submodules()
    assert "stage" in names
    assert "annotate" in names
    assert "pad" in names
    assert "brain" in names
    # These are modules on purpose (classes live as Camera3D / World3D / Prop).
    assert "look" not in names
    assert "play" not in names
    assert "camera3d" not in names
    assert "world3d" not in names
    assert "demo" not in names


def test_crest_isle_on_enter_calls_stage_with_radius():
    """The exact line Emma hit must stay a public ``kagra.stage`` call."""
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


def test_relic_run_on_enter_calls_stage_with_radius():
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


def test_on_enter_after_stage_uses_public_names():
    """Catch the next AttributeError: names after kagra.stage(...) must exist."""
    common = {
        "set_hdri",
        "set_fog",
        "set_bloom",
        "set_spot_light",
        "set_point_light",
        "set_camera3d",
        "Walk",
        "Label",
        "load_json",
        "Prop",
    }
    extra = {
        "CrestIsle": {"set_light_dir"},
        "RelicRun": set(),
    }
    for path, cls in ((OPEN_WORLD, "CrestIsle"), (RELIC_RUN, "RelicRun")):
        names = {
            c.func.attr
            for c in _kagra_calls_in_method(path, cls, "on_enter")
            if isinstance(c.func, ast.Attribute)
        }
        needed = common | extra[cls]
        missing = sorted(needed - names)
        assert not missing, f"{cls}.on_enter missing kagra.{missing}"
        src = path.read_text(encoding="utf-8")
        assert "self._reset_round()" in src
        assert "kagra.Prop.bake_all()" in src
        assert "self.cam.follow(" in src

# Session — unshadow `kagra.stage` (and the same-named-submodule class)

Master at start: `9868bde` (#77 Crest Isle `_chunk_props`).

## Cause (verified, not discarded)

`kagra/__init__.py` defines `def stage(...)`. Later:

```
from kagra.stage import Stage, backdrop_sphere, classify_stage_file, resolve_stage_path
```

Python's import machinery binds `kagra.stage` to `kagra/stage.py`. The function is gone. Crest Isle line 209 is `kagra.stage(str(sky_png), radius=140.0)` → `TypeError: 'module' object is not callable`. Relic Run line 188 is the same call with `radius=48.0`.

AST tests still see `def stage` in `__init__.py`. That is why source-order was not enough.

## Audit

| Name | Submodule | Documented as | After `import kagra` (before fix) |
|---|---|---|---|
| `stage` | `kagra/stage.py` | `kagra.stage(...)` | **module** (the crash) |
| `annotate` | `kagra/annotate.py` | `kagra.annotate(...)` | function, then **module on first inner import** (second call would TypeError) |
| `pad` | `kagra/pad.py` | `kagra.pad(...)` | function (`from kagra.pad import pad` rebinds the name) |
| `brain` | `kagra/brain.py` | `kagra.brain(...)` | function (same rebind) |
| `look` / `play` / `camera3d` / `world3d` / `demo` | yes | classes / `apply_*` / `Prop` / `Walk` | modules, and that is correct — demos call `kagra.World3D` / `kagra.Prop`, not `kagra.world3d()` |

## Walk after Crest Isle line 209

Stub-imported the package (dummy `Engine`, no GPU) and `inspect.signature(...).bind` on every `kagra.X(...)` in `CrestIsle.on_enter` / `RelicRun.on_enter`, plus `Camera3D.follow` and `World3D.set_height_fn`.

No next TypeError from kwargs:

- `set_hdri(path, strength=)`
- `set_fog(start=, end=, color=, enabled=)`
- `set_bloom(threshold=, intensity=)`
- `set_light_dir` (Crest Isle only)
- `set_spot_light` / `set_point_light` (`slot=` 0..3)
- `Prop(..., metallic=, roughness=)`
- `Prop.bake_all`
- `Camera3D.follow(..., lerp=, yaw=, distance=, height=, look_y=)`
- `set_camera3d`
- `Walk(..., speed=, jump=, yaw=, distance=, height=, look_y=)`
- `Label(text, x, y, size, color)`
- `load_json` / `_reset_round`

GPU paths (`Stage.load` → `kagra.load`, lights → `_engine`) still need `kagra.init` / the renderer. This VM has no `kagra_core`; those are not callable without the engine, but they exist and bind.

## Fix

Package type `_KagraPackage` refuses to let a submodule overwrite an existing non-module on `kagra`. Snapshot + restore around the late imports as a second belt. `from kagra.stage import Stage` still works (`sys.modules['kagra.stage']`). Demos still call `kagra.stage(...)`.

## Stumbles

- `import kagra.stage as m` uses `getattr(kagra, "stage")`, so `m` is the **function** after the fix. `from kagra.stage import Stage` and `importlib.import_module("kagra.stage")` still yield the module. Tests use those, not `import kagra.stage as m`.

GitHub CI on `3ddda60`: 17 checks passed.

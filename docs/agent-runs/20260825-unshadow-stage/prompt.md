Emma still cannot launch Crest Isle on D:\program\kagra after the _chunk_props fix (#77). New crash:

```
File examples/vrm_open_world.py, line 209, in on_enter
    self.sky_stage = kagra.stage(str(sky_png), radius=140.0)
TypeError: 'module' object is not callable
```

Do NOT one-line-patch the demo and bounce. She asked to check thoroughly so the next run is not another traceback.

## Likely cause (verify; discard if wrong)
`kagra/__init__.py` defines `def stage(...)` (README public API). There is also `kagra/stage.py`. At runtime `kagra.stage` is the MODULE, so calling it fails. Relic Run (`examples/vrm_relic_run.py`) uses the same `kagra.stage(str(sky_png), radius=...)` and will crash the same way if the Poly Haven PNG exists.

Fix the engine so `kagra.stage` is the callable. Do not make demos import `kagra.stage.stage`. Keep `from kagra.stage import Stage` working if that's used.

## Thorough pass (required)
1. Audit every `kagra.<name>` that collides with a submodule (`stage`, `look`, `play`, `pad`, `camera3d`, `world3d`, `demo`, …). If a documented function is shadowed by a module, unshadow it and add a GPU-free test that `callable(kagra.X)` for each public function the demos call.
2. Walk `CrestIsle.on_enter` after line 209 (set_hdri, set_fog, set_bloom, lights, Prop.bake_all, Camera3D.follow, Walk, Label, _reset_round) and Relic Run's equivalent. Catch the NEXT AttributeError/TypeError now, not after she reports it.
3. Add tests that fail if `kagra.stage` is a module. Source-order tests are not enough.
4. pytest -m "not golden". Open a PR on latest master of https://github.com/EMMA019/KAGRA.

Out of this PR: Rapier, cloth, blend trees, spatial audio, multi-avatar, visual editor, CSM/SSAO/WebXR. Slope AABB is a follow-up after this launches.

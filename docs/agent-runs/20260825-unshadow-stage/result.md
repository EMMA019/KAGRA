# Result — unshadow `kagra.stage`

## Files

- `kagra/__init__.py` — `_KagraPackage` + restore so documented callables win over same-named submodules
- `tests/test_public_names.py` — runtime `callable(kagra.stage)` (stub Engine, no GPU); demo `on_enter` signature bind
- `CHANGELOG.md` — unreleased note

Demos unchanged (`examples/vrm_open_world.py` / `vrm_relic_run.py` still call `kagra.stage(...)`).

## Verify (this session)

```bash
python3 tools/gen_api_index.py --check   # OK (422 entries)
python3 -m pytest tests -m "not golden"  # 366 passed, 10 deselected
python3 -m pytest tests -m "not golden" -p tests.no_extension_plugin  # 366 passed, 10 deselected
```

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`) was **not run** (no `kagra_core` on this VM). **GitHub CI: 17 checks passed** on `3ddda60` (https://github.com/EMMA019/KAGRA/actions).

PR: https://github.com/EMMA019/KAGRA/pull/78

## Notes

Not Rapier / cloth / blend trees / spatial audio / multi-avatar / visual editor / CSM / SSAO / WebXR. Slope AABB is a follow-up after Crest Isle launches.

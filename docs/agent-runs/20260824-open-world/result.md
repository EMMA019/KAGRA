# Result — Crest Isle

## Files

- `examples/vrm_open_world.py` — playable scene
- `examples/open_world_rules.py` — GPU-free rules
- `examples/verify_scenarios/open_world_smoke.json`
- `examples/assets/open_world/` — Kenney CC0 + LICENSE.md
- Engine: `kagra/land.py` (`open_world_height`), `kagra/world3d.py` (LOD),
  `kagra/play.py` (gltf flatten cache), `kagra/__init__.py` (`can_pick`)
- Relic Run: extra Mini Forest density + `_bind_locomotion` indent fix

## Play

```bash
python examples/vrm_open_world.py
```

WASD + mouse, SPACE jump. Collect 6/8 crests (peak flag counts). Coins
are score. Title overlay sits on the opening vista.

## Verify (this session)

```bash
python tools/gen_api_index.py --check   # OK (418 entries)
pytest tests -m "not golden"            # 338 passed, 9 deselected
```

GPU smoke (`python -m kagra.verify examples/verify_scenarios/open_world_smoke.json`)
was **not run on the agent VM** (no `kagra_core` / `maturin develop` there).
Headless smoke JSON is in the tree. **GitHub CI: 17 checks passed** on
`1b47642` (https://github.com/EMMA019/KAGRA/actions).

PR: https://github.com/EMMA019/KAGRA/pull/73

## Notes

Not Nintendo IP. Alicia Solid © Dwango. Kenney + Poly Haven CC0, not in
the pip wheel. No Rapier / OSM / voxels / kagra-shared merge.

# Result — Relic Run walk + CC0 island

## (a) Forward-arms

Root cause: Relic Run replaced built-in VRM walk with Mixamo/synthetic BVH
T-pose deltas (`resolve_asset(..., "walk")` always hits
`tests/fixtures/synthetic_walk.bvh`). ActionController empty keyframes
blended toward the overlay pose (clap arms).

Fix: built-in idle/walk; optional `walk.vrma` fails loudly; overlay rest
is the pose saved at `play()`.

## (b) Assets (CC0, not in the wheel)

`examples/assets/relic_run/` — see `LICENSE.md`.

- Kenney Mini Forest 1.0 + Nature Kit 2.1 glTF + colormap
- Poly Haven `aerial_grass_rock` 1K diffuse (+ unused normal)
- Poly Haven `kloofendal_48d_partly_cloudy_puresky` 1K → 1024×512 PNG

## (c) Spawn

Grass terrain texture, Kenney trees/rocks/plants in the +Z chase-camera
frustum, stone pedestals + large gold relics with double glow billboards,
HDRI sky sphere.

Gameplay unchanged: 5 relics, 30s, `Walk.face`, third person.

## Verify

- `pytest tests -m "not golden"`: **pass** (this VM)
- `python tools/gen_api_index.py --check`: OK (416 entries)
- GitHub CI on `96443c4` (PR #72): **all 17 checks passed**, including
  `python-unit` 3.10/3.11/3.12, `rust-test`, `kagra-shared`, maturin
  builds, iOS/Android shells, and `golden`
- `python -m kagra.verify examples/verify_scenarios/relic_run_smoke.json`: **not run** on this VM (`kagra_core` missing); golden job on CI is the GPU stand-in
- README sample line `python examples/vrm_relic_run.py` stays

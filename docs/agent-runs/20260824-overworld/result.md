# Result — Overworld island

- `World3D.set_height_fn` / `kagra.island_height` / `water()` / `Walk(..., jump=)`
- Demo: `examples/vrm_overworld.py`
- `pytest tests -m "not golden"`: 通過
- Verify: `examples/verify_scenarios/overworld_smoke.json`（この VM では GPU 未ビルド）
- まだ無い: ストリーミング、マリオの 3 段ジャンプ、水面シェーダ、広い屋外の CSM

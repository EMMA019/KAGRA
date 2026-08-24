# Result — City JSON / mesh hit / stack / 2-cascade

- 街: `load_city`。OSM ではない。
- 当たり: 静的三角形。スキンメッシュではない。
- 積み木: 動的 AABB + スリープ。Rapier ではない。
- 影: `set_shadow_cascades(2)`。既定 1。この VM では GPU 未ビルド。
- Demo: `examples/vrm_overworld.py`
- Tests: `pytest tests -m "not golden"` — **264 passed**, 3 deselected
- Verify: `overworld_smoke.json`（この VM では GPU 未ビルド）

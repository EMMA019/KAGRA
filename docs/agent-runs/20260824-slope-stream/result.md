# Result — Slope follow / tiled stream

- 坂: 接平面に沿う。急斜面は滑る。ジャンプは殺さない。
- 階段: `stair_y` の段を `step_height` で登る。
- 影: タイル AABB < 24。CSM ではない。この VM では GPU 未ビルド。
- ストリーム: 高さ場タイルの load/unload。箱街区は一度置いたら残る。街ファイルではない。
- まだ無い: Rapier、三角形メッシュ当たり、積み木物理、CSM、OSM / 都市ローダ
- Demo: `examples/vrm_overworld.py`
- Tests: `pytest tests -m "not golden"` — **254 passed**, 3 deselected
- Verify: `examples/verify_scenarios/overworld_smoke.json`（この VM では GPU 未ビルド）

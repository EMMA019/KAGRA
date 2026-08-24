# Session — 2026-08-23 World shadows (P5)

絵トラックの次。ロードマップ P5。

## 判断

- 新しい Python API は足さない。`set_shadow_enabled` のまま。
- ortho の合わせを VRM AABB だけから、即時 Mesh3D / `draw_mesh_id` / インスタンスへ広げる。
- 同じシャドウパスでワールドも depth を書く。床・箱・Prop がキャスター。
- 空（巨大 AABB）は半辺を壊すので除外（extent > 24）。
- 半辺クランプを 14 → 28。`World3D(half=7)` の 14×14 床が収まる。
- カスケード・点光源・HDRI・PBR は P6 以降。`kagra-shared` は触らない。

## Verify

- `pytest tests -m "not golden"`: 通過。
- `python3 tools/gen_api_index.py --check`: 通過。
- `cargo test -p kagra-core --no-default-features --locked`: この VM の Cargo 1.83 は
  lock の `indexmap 2.14.0`（edition2024）をパースできない。CI は `@stable`。
  `shadow_fit` テストはファイルに書いた。GPU シナリオは未実行。

# Session — 2026-08-23 Prop glTF parts

ロードマップ部屋トラックの次項。`stage()` の会場ロードとは別の部品。

## 判断

- 新しい Entity は作らない。`Prop("crate.glb")` だけ足す。
- Rust の `load_gltf` / `draw_gltf` は会場用（原点固定、スキン無し primitive を
  飛ばす）。部品は Python で静的メッシュに畳み、既存の
  `upload_mesh_3d` / `draw_mesh_instances` に載せる。
- 当たりとホバーはメッシュ AABB × スケール。メッシュコライダは足さない。
- 同梱 `kagra/data/unit_cube.glb`。エイリアス `cube` / `cube.glb`。
- Prop Garden のスモーク画素を変えないため、`cube.glb` は非スモークだけ。

## Verify

`kagra_core` がこの環境に無いので GPU シナリオは未実行。
`pytest tests -m "not golden"` と `python tools/gen_api_index.py --check`。

# Session — 2026-08-24 slope / tiles / stream

## 決断

- Rapier は入れない。積み木物理と任意メッシュ当たりは「まだ無い」。
- 坂: `height_normal` で接平面。歩ける勾配は沿う。`max_grade` 超は滑る（Y 吸着だけではない）。
- 階段: 高さ場の段 + 既存の `step_height`。三角形メッシュではない。
- 影: 地形を辺 10 のタイルに切る。AABB extent < 24 なので空扱いで飛ばされない。CSM ではない。
- ストリーム: `tile_keys` + `stream_radius` でタイルの load/unload。街ファイル / OSM ではない。
- 街区: `city_boxes` が草原タイルに箱を置く。一度置いた箱は外さない。

## 実装

- `kagra/physics3d.py` — 接平面投影、急斜面の滑り蓄積、接地は法線成分だけ消す
- `kagra/land.py` — `tile_keys` / `stair_y` / `ramp_y` / `overworld_height` / `city_boxes`
- `kagra/gamekit.py` — `heightfield_tile`（ワールド UV）
- `kagra/world3d.py` — タイル bake、`stream_tiles`、`set_chunk_fill`
- `examples/vrm_overworld.py` — 上記を全部使う

## 躓き

- Walk が毎フレーム `vx/vz` を上書きするので、滑りは body に蓄積しないと 1 フレームで消える。
- 1 枚の `half=24` メッシュは影スキップ（extent 48）。タイル化が影の本体。
- タイル UV を 0..1 にすると島テクスチャがタイルごとに繰り返す。`uv_half` でワールド UV。
- スポーン直後に箱が無いと `bake()` が unit box を載せず、後から置いた街区が描かれない。`chunk_fill` があれば box メッシュを載せる。

# Result

## What landed

kagra-shared（wgpu 30）の VRM を残り 3 スライスで 0.19 相当に引き上げた。

- **A: SpringBone コリジョン** — 球/カプセルコライダー、チェーンごとの
  colliderGroups、Verlet 内で押し出し。VRM 0 / VRM 1 両対応。
- **B: MToon matcap / normal** — Emma は 16/17 パーツに matcap + normal を持つ。
  髪に光沢（SphereAdd）と法線ディテールが付く。
- **C: ボーン制約 + firstPerson** — VRMC_node_constraint（rotation / roll 適用、
  aim はパースのみ）、VRM firstPerson 注釈のパース + 保持（適用は FPS カメラで）。

## Commands

```text
cargo test -p kagra-shared --lib
# 370 passed; 0 failed
#   new: collider_parses_v0_and_v1 / collider_pushes_joint_out
#        constraint parse/apply x3 / first_person parse x2
#        Emma matcap + normal アサート

cargo test -p kagra-shared --features render --test offscreen_render
# 12 passed

cargo clippy -p kagra-shared --all-targets --features render --locked --offline -- -D warnings
# ok

python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json --seconds 4
# ok（回帰なし）
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
# ok
```

## Try

```text
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
# 髪がコリジョンで身体と干渉せず揺れる + 髪に光沢（matcap）と細かい影（normal）
```

## Files

- `kagra-shared/src/spring.rs` — コリジョン（collider パース + 解決）
- `kagra-shared/src/scene3d.rs` — MeshData.matcap/normal、MtoonShade フラグ
- `kagra-shared/src/gltf_load.rs` — matcap/normal パース、constraints、
  first_person、apply_constraints
- `kagra-shared/src/constraint.rs`（新規）— VRMC_node_constraint
- `kagra-shared/src/first_person.rs`（新規）— firstPerson 注釈
- `kagra-shared/src/render/mod.rs` — 6 バインディング拡張
- `kagra-shared/src/render/shader3d.wgsl` — Toon の matcap / normal 適用

## Stuck（= ドキュメントの穴）

- **matcap はビュー行列が無いため反射ベクトル方式**（0.19 はビュー空間法線）。
  見た目の差異は小さいが、厳密一致を望むなら Globals に view 行列を足す。
- VRMC_node_constraint の aim は「パースのみ」。適用は look ソースが要る。
- firstPerson はパース + 保持まで。適用（FirstPersonOnly メッシュの隠し）は
  FPS カメラ実装時。

# Result

## What landed

kagra-shared（wgpu 30）に **VRM 表情プリセット** を追加。

- `walker.expression`（dump の String、default "blink"）で smile / angry /
  sad / joy / blink / aa / ih / ou / ee / oh / custom を選択
- "blink"（または未指定）= 自動まばたきエンベロープ（既存）
- 名前付き表情は「モデルのいずれかのパーツが持つ場合」重み 1.0 で適用
- `Expressions::get / has` を追加。`morphed_rest` / `sample_skinned_look` /
  `sample_skinned_hair` が名前付き expression を受け取る

## Commands

```text
cargo test -p kagra-shared --lib
# 363 passed; 0 failed
#   new: get_and_has_named_preset / named_expression_aa_moves_morph_targets
#        expression_preset_flips_morph_weight（Emma 実機）

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
# dump の player.expression を "aa" や "smile" に変えて play_world を起動
# （Emma は 17 パーツ構成で、モーフは Face 等にある）
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json
```

## Files

- `kagra-shared/src/morph.rs` — Expressions::get / has
- `kagra-shared/src/gltf_load.rs` — morphed_rest / sample_skinned_* の
  expression 引数
- `kagra-shared/src/world_doc.rs` — WorldWalker.expression、GltfSlot、
  gltf_skinned_mesh_for
- `kagra-shared/src/world_play.rs` — step_morph（全パーツ判定）
- `docs/schemas/world.json` — walker.expression

## Stuck（= ドキュメントの穴）

- **モーフは Face 等のパーツにあり、Body（parts[0]）は morphs 0 個でも
  expressions を持つ**。表情の存在判定は必ず全パーツ（load_skinned_parts）
  で行う。parts[0] だけ見ると「モーフなし」で早期 return する。
- CPU skinning は各パーツごとに morphed_rest を呼ぶので、morph 重みは
  全パーツ共通で良い（各パーツの own morphs に適用される）。

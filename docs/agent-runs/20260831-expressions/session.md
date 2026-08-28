# Session

## 調査

- 0.19 `vrm_expression.rs`: OverrideMode（Block/Blend/None）、
  ExpressionChannel（Blink/Mouth/LookAt/Other）、`effective_expression_weights`
  （override + isBinary を適用した実効ウェイト）。
- 0.30 `morph.rs`: `Expressions`（by_name）は blink/aa のみ `pick()`。
  `morphed_rest` は pick() で選ばれた 1 表情に単一の morph 重みを適用。
  WorldPlay の step_morph は自動まばたきのみ。

## 実装の往復

1. `morph.rs`: `Expressions::get(name)` / `has(name)` を追加（大文字小文字
   無視、blink は常にあり）。
2. `gltf_load.rs`: `morphed_rest` に `expression: &str` 引数を追加。
   「blink（自動）なら pick()、名前付きなら by_name から bind を取得」。
   `sample_skinned_look` / `sample_skinned_hair` も expression 引数を持つ。
3. `world_doc.rs`: `WorldWalker.expression`（default "blink"）を追加。
   `GltfSlot::Skinned` に expression、`gltf_skinned_mesh_for` に
   expression 引数（clippy too_many_arguments は allow、既存と同様）。
4. `world_play.rs` step_morph: expression が "blink" なら自動まばたき、
   名前付きなら「モデルの**いずれかのパーツ**が持つ場合」重み 1.0。
5. **躓き**: Emma のモーフ（blink/aa）は Face パーツにあり、Body（parts[0]）
   は morphs 0 個でも expressions を持つ。step_morph が parts[0] だけで
   判定して「モーフなし」で早期 return → 表情が効かない。
   `load_skinned_parts`（全パーツ）を pub(crate) にして全パーツで判定。
6. テスト: `get_and_has_named_preset`（morph.rs）、
   `named_expression_aa_moves_morph_targets`（gltf_load）、
   `expression_preset_flips_morph_weight`（world_play、Emma 実機）。

## 最終状態

- `walker.expression` で smile / angry / aa 等を選択 → 対応するモーフ
  ターゲットが適用される。blink は自動まばたき。
- ゲーム側は dump の `walker.expression` を書き換えるだけ（interact /
  event 連動はジャンルコードが take_events で行う）。

## 次（未実施）

- SpringBone 強化（コリジョン、複数連鎖）
- matcap / normal テクスチャ（MToon）
- override / isBinary の完全移植（必要になったら）

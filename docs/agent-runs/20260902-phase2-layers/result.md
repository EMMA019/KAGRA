# Result — Phase 2 前半完了（アニメーションブレンド + 上半身/下半身レイヤー分離）

汎用エンジン化ロードマップ Phase 2（アニメーションブレンド）のコア部分が完了した。

## ① ロコモーションブレンド（anim_blend 0..1）

| 項目 | 対応 | コミット |
|---|---|---|
| rest ↔ clip の連続ブレンド | `WalkerPose.anim_blend`（0 = rest/idle、1 = walk クリップ）を `blend_locals`（translation/rotation lerp・slerp、scale lerp）で適用 | 8236891 |
| WorldWalker に持たせる | dump キー `anim_blend`（serde default 0）。ゲームは速度や状態から連続的に変化させる | 8236891 |
| renderer 配線 | `update_world_gltf` / `gltf_skinned_mesh_for` が `sample_skinned_look_blend` 経由でブレンド | 8236891 |
| world_play 平滑化 | `step_walker` が速度に応じ anim_blend を 8/s で寄せる（状態切替ポップ防止） | 8236891 |
| バニーガーデン | `anim_blend: 1.0` 固定で「その場歩き」を維持（idle 揺れ + 布が動く） | 8236891 |

## ② 上半身/下半身レイヤー分離（overlay ジェスチャー）

| 項目 | 対応 | コミット |
|---|---|---|
| WalkerPose 統合 | `sample_skinned` / `_hair` / `_look` / `_look_blend` / `_cloth` / `_cloth_pose` / `_inner` を全て `WalkerPose` ベースに集約。`sample_skinned_pose`（布なし）追加 | e3cfbfb |
| overlay 適用 | `apply_overlay` が `overlay_bones`（humanoid 名 or node 名 → 目標ローカル回転 [x,y,z,w]）を `overlay_weight` で slerp。look/hair の後、布シミュの前に適用 | e3cfbfb |
| データ配線 | `WorldWalker.overlay_bones` / `overlay_weight`（dump キー）→ `GltfSlot::Skinned` → `compile_meshes` / `update_world_gltf` まで pose で流す | e3cfbfb |
| バニーガーデン TTS | 発話中のみ両腕を外側へ揺らすジェスチャー（`_gesture_overlay`）。無言なら空 dict → 何も動かない | e3cfbfb |

## verify

- lib 396 + offscreen 12 パス（`overlay_moves_upper_body_but_not_legs` 追加:
  左腕 overlay で腕頂点は動くが脚頂点は 1e-4 未満、weight 0 は完全 no-op）
- clippy `--features python -- -D warnings` クリーン、wasm32（wasm,render）OK、
  `gen_api_index --check` OK
- pytest 569 パス（bunny: gesture/dump テスト 2 件追加）
- 実機: `render_world_doc` で overlay 有無で 1339 画素変化（腕が動く）、
  `collectathon_emma_smoke` verify `ok: true`

## 次の山

Phase 3 — 経路探索（ナビメッシュ / 汎用経路）。トルネコの「歩いて敵に近づく」、
SLG の「ユニット移動範囲」に直結。Phase 2 の残り（複数クリップのブレンド、
エモートレイヤー深度）は Phase 3 の後に戻る。

# Session — Switch Room

一行プロンプト: `prompt.md`。Heart Catch の次で、円盤キャッチではない
**部屋を歩く**ゲームを Front の公開 API だけで組んだ。

## 決めたこと

- 収集ミニゲームにしない。床 + 静的箱 + スイッチ踏みでクリア。
- メッシュは毎フレーム組み立てない。`World3D.bake` → `upload_mesh_3d`、描画は `draw_mesh_id`。
- 衝突は既存の `Physics3D`（カプセル vs AABB）。新しい物理エンジンは入れない。
- カメラは `Camera3D.follow`（orbit / showcase を切って後ろ上から追う）。
- ルールは `examples/switch_room_rules.py` に切り出し、GPU なしでテストする。

## 往復

1. `docs/API_INDEX.md` / `Physics3D` / `draw_mesh_3d` を確認。保持メッシュは無かった。
2. Rust に `upload_mesh_3d` / `draw_mesh_id` / `unload_mesh_3d` を足した。
   既存の毎フレーム `draw_mesh_3d` は残す。
3. Python: `box_mesh`、`World3D`、`Camera3D.follow`。
4. `examples/vrm_switch_room.py` を公開 API だけで書いた（`_` import なし）。
5. ルールテスト + `examples/verify_scenarios/switch_room_smoke.json`。

## 躓き

1. **保持メッシュが無かった。** `draw_mesh_3d` は毎フレーム CPU 頂点を
   プールバッファへ書く。箱部屋だとそれが本体なので、upload-once を先に足した。
2. **2D `Camera.follow` と 3D が別物。** エージェントが 2D を掴むと絵が死ぬ。
   索引の Front に `Camera3D.follow` を置き、Shelf に 2D を下げた。
3. **`kagra_core` 未ビルド。** この環境では `import kagra` が落ちる。
   ルールと `World3D` の衝突は pytest で閉じた。GPU verify はシナリオだけ置いた。
4. **`World3D.bake` が `import kagra` に依存。** テストは bake を呼ばない
  （呼んでもエンジン未初期化なら空リスト）。

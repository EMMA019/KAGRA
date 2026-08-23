# Session — 2026-08-23 Meteor Dodge

エージェント: Claude（chatの汎用サンドボックス。Cursor/Grokではない）。
このリポジトリの開発には初参加— heart-catch / switch-room のセッションを
読んで文脈は把握しているが、コードは一切書いていない状態からの実施。
規約: ルート `AGENTS.md`（Cursor専用ではなく一般エージェント向けの記述と
明記されていたので、そのまま読んで従った）。

## 0. 指示

`prompt.md` の一行のみ。追加仕様は聞いていない。heart-catch（キャッチ）・
switch-room（スイッチ踏み）とは別の動詞（避ける／生存）にした。

## 1. API 検索（推測する前）

`docs/API_INDEX.md` を検索し、`examples/switch_room_rules.py` /
`examples/vrm_switch_room.py` / `examples/heart_catch_rules.py` を先に
読んで既存の型（ルールを GPU 非依存モジュールに切り出す、本体は公開 API
のみ）を確認した。

| 欲しいもの | 見つけた名前 | メモ |
|---|---|---|
| 床＋壁のある部屋 | `World3D` | switch-room と同じ構成を再利用 |
| プレイヤー移動 | `World3D.add_player` / `move_player` | カプセル、AABB壁と衝突 |
| 降ってくる物体 | なし（`Physics3D` に動的 AABB はあるが、既存2本はどちらも
  物理エンジンの衝突通知ではなく**純 Python のルール判定**を使っていた）| 既存の型に
  合わせ、隕石も `dodge_room_rules.py` の純 Python dataclass で管理し、
  当たり判定は座標比較で行うことにした。`Physics3D.on_collide` という
  コールバックがあるのも見つけたが、今回は使わなかった（下記 4-1）|
| 描画 | `draw_billboard(tex, x, y, z, size, camera)` | 索引の型どおり |
| 表情 | `avatar.feel("surprised", 1.0)` | `kagra/vrm_emotion.py` の
  `_EMOTION_CANDIDATES` に `surprised` があるのを確認してから使用 |
| セーブ | `save_json` / `load_json` | 前2件と同じ |

## 2. アセット

`kagra.ensure_vrm()` を使用。VRM本体はこの環境に無いのでダウンロード
フォールバックに任せる（前2セッションと同じ状況）。

## 3. 実装の判断

- Heart Catch・Switch Room と同じ形（ルール=純Python dataclass、
  本体=公開APIのみ）を踏襲。
- 隕石は `Physics3D` の動的剛体にしなかった。理由：既存2本がどちらも
  「物理エンジンではなく専用ルール関数で当たり判定」という統一されたスタイル
  だったので、そこから外れると一貫性が崩れると判断した。`Physics3D.add_body`
  を動的（`is_static=False`）にして重力で落とす案も検討したが、着地後の
  静止処理・複数個同時生成時の負荷・トリガー通知の受け取り方が未知数だった
  ため、確実に動く方（座標比較）を選んだ。将来 `RigidBody3D.on_collide` を
  使う版に置き換える余地はある。
- 難易度カーブ（`fall_speed` 線形増加、`spawn_gap` 半減期減衰）はテストで
  「時間が経つほど厳しくなる」ことだけを保証し、具体的な秒数のバランスは
  未調整（GPUで実際に遊べないため、数値の当てずっぽうは避けられない）。

## 4. 躓き（省略しない）

1. **テストの必須APIリストをそのままコピーして失敗した。** `switch_room`の
   テストから `test_game_file_uses_only_public_imports` の必須名リストを
   コピペしたが、`upload_mesh_3d` / `draw_mesh_id` は `World3D` 内部で
   使われるだけで `vrm_dodge_room.py` 本体には出てこないため落ちた。
   `draw_billboard`（隕石の描画に実際使っている名前）に差し替えて解決。
   → 教訓：他ゲームのテストを型として真似るのは良いが、必須名リストは
   「そのゲームが実際に呼ぶ公開API」に合わせて書き直す必要がある。
2. **Rust拡張はおろかツールチェーン自体が無い。** `cargo` / `rustc` が
   この環境に無く、`kagra_core` のビルドはそもそも不可能。Python側の
   ロジック（`pytest`）と `tools/gen_api_index.py --check` までしか
   閉じられない。
3. **`Physics3D.on_collide` を見つけたが使わなかった。** 存在は確認した
   （3節参照）。既存2本との一貫性を優先して不採用にしたが、これは
   「使えるAPIを見つけたのに使わない」という判断そのものも躓きとして
   記録しておく価値があると思う。

## 5. verify

書いたシナリオ: `examples/verify_scenarios/dodge_room_smoke.json`
（`KAGRA_SMOKE=1` で48フレーム、24フレーム目にスクショ、8フレーム目に
`move_player`で動かす — switch-roomと同じ形）。

**この環境では未実行**（`kagra_core`未ビルド、ビルドツールチェーン自体が
存在しない）。GPU環境での閉じ方:

```bash
pip install maturin && maturin develop
pytest tests -m "not golden"
python -m kagra.verify examples/verify_scenarios/dodge_room_smoke.json
```

難易度バランス（`BASE_FALL_SPEED` / `SPAWN_GAP_HALFLIFE` 等、
`examples/dodge_room_rules.py` 冒頭の定数）は実際に遊んでから
調整が要ると思う。

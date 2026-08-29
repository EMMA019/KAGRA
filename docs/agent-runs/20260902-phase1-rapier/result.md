# Result — Phase 1 完了（Rapier 剛体物理）

ユーザー決定「Rapier を本線に導入」を受けて、汎用エンジン化ロードマップ
Phase 1（物理）が完了した。これは「Rapier は 5MB wheel の外」「Rapier は
入れない」という従来方針（80% リストのエンジン都合）を覆す決定だった。

## 設計

| レイヤー | 内容 |
|---|---|
| Cargo | `rapier3d 0.35` を optional dep + `physics` feature（`enhanced-determinism` 付き）。`python` feature と組み合わせて使う |
| Rust | `kagra-shared/src/physics.rs` — `PhysicsWorld`（Rapier ラップ）。props / walkers を剛体に、`height_at` サンプリングの高さ場コライダーで地面 |
| WorldDoc | `WorldProp.is_static`（既定 true）/ `friction` / `restitution` 追加（dump キー `is_static`） |
| PyO3 | `kagra_shared.PhysicsWorld`（from_json / step / to_json / position / set_velocity / set_position） |
| Python | `kagra.rigid.PhysicsWorld` — dict 世界 ↔ 剛体の薄いラッパー |

挙動: `is_static=false` の prop は落下・衝突・積み重なる動的剛体（old の
`add_box(is_static=False)` と同じ契約、ただし Rapier で本物の物理）。walker
はカプセル剛体（床に立ち、箱に乗る）。地形は高さ場（丘に沿って箱が転がる）。

## 躓き（記録）

1. **gravity=0**: 構造体リテラル（Rust Default）だと `doc.gravity` が 0 になり
   剛体が落ちない → serde 既定 9.8 にフォールバック。
2. **HeightField が落下スルー**: Rapier 0.35 の HeightField は scale が**全体
   サイズ**で原点中心（`x_at(j) = (-0.5 + j/(n-1)) * scale.x`）。セル間隔 +
   translation では位置が合わず床に乗らない → 全体サイズ + translation 無しに修正。
3. **insert_collider のシグネチャ**: 0.35 は `insert_collider(collider, Some(body))`
   （親無しは None）。`body` 直渡しは型エラー。
4. **density(0.0) の walker が落ちない**: 質量 0 だと重力が効かない → density 1.0。

## verify

- cargo test（physics,render）: 402 lib + 12 offscreen。physics 6 件
  （落下・静的不動・積み重ね・決定論・velocity・walker 着地）
- clippy `--features python,physics -- -D warnings` クリーン、
  wasm32（wasm,render,physics）OK
- pytest: 629 パス（test_rigid.py 6 件: 実機 5 + kagra_shared 無しでスキップ）
- verify シナリオ `physics_stack_smoke`: 箱 2 個が落ちて積もる
  （`a.y=0.550 b.y=1.549`）を offscreen 描画、`ok: true`
- `gen_api_index --check` OK

## 次の山

Phase 1 の残り（任意）: 歩行者のキネマティック駆動（WorldPlay と物理の
共存）、球/カプセル prop の形状対応、コライダーイベント（on_ground の
正確化）。ユーザー長期リストの SLG は既存の `move_range` 等で続行可能。

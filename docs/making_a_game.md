# 新しいゲームの作り方（エージェント向け手順書）

KAGRA はスマホのブラウザで遊ぶ小さな 3D ゲームを量産するためのエンジン。
新しいゲームは **キット**（`KitHost` を実装した Rust の 1 ファイル）＋ **JSON**（ワールドとマニフェスト）。
ページ（`play.html`）は全ゲーム共通で、ゲームごとの分岐を書き足さない。

## 1. 作る（1 コマンド）

```bash
python scripts/new_kit.py lantern_run "Lantern Run"
cargo test -p kagra-shared --lib lantern_run::
```

これで遊べるゲームが 1 本できる（テンプレートの Starter: スティックで動き、星を取り、
跳ね回る敵を避けて 60 秒生き残る）。作られるもの:

| ファイル | 役割 |
|---|---|
| `kagra-shared/src/<slug>.rs` | キット本体（`starter.rs` のコピー）。ここを書き換える |
| `kagra-shared/src/lib.rs` / `kit_host.rs` | 登録（1 行ずつ。触らない） |
| `kagra-shared/tests/fixtures/<slug>_world.json` | ページが読むワールド（仕様の既定値そのもの） |
| `games/<slug>_minimal.json` | マニフェスト（タイトル、同梱素材、`page` 機能） |
| `examples/<slug>_minimal.py` | `--check`（ワールドと素材ファイルの確認） |

PR を出すと、プレビュー（`pr/<n>/play.html?game=<slug>`）と CI のブラウザ起動確認に
自動で載る。

## 2. 変える（`GAME:` の印の所だけ）

| 場所 | 中身 |
|---|---|
| `<Slug>Spec` | 調整値。全部に既定値、未知のキーはエラー、`validate` で範囲チェック |
| `step_sim` | ルール（1 固定ステップ）。終わりは `core.stats.alive = false`（負け）か `finish(true)`（勝ち） |
| `build_doc` | 3D の見た目（床、モデル、カメラ、ライト）。モデルが無いときは球に落ちる |
| `hud` | プレイ中の HUD。タイトルと結果のカードはページが描く |
| `state_value` | ページが読む状態（`phase` `score` `best` `death` `result` は必須） |

仕様の既定値を変えたら fixture を焼き直す:

```bash
KAGRA_BAKE=1 cargo test -p kagra-shared --lib <slug>::
```

## 3. 使える部品（自前で書かない）

| 部品 | 場所 |
|---|---|
| 固定ステップの駆動、ヒットストップ、スコア | `game_core::{tick_frame, Kit, Core}` |
| フローティングのスティック（着地、追従、アナログ、見た目） | `game_core::stick` |
| 走る・跳ぶ・ダッシュする体（コヨーテタイム、先行入力、すり抜け足場、動く足場） | `game_core::platform` |
| マス目の移動範囲・経路・射程（タクティクス、パズル） | `game_core::grid` |
| 物語カード・クリアカード（画面を暗くして中央にカード、見出し、本文、集計の行、点滅する「タップで」、ステージ選択の ◀ ▶） | `hud_kit::{Card, CardStyle, pick_side}`（文章の折り返しだけなら `hud_kit::wrap_text`） |
| HUD ボタンの当たり判定（描くのと同じ矩形の表で） | `hud_kit::hit` |
| ステージの解放と記録（端末に保存、最初の物語カードでステージ選択） | `game_core::progress::{Progress, Better}` + `KitHost::save_json` + マニフェストの `page.save` |
| 会話（顔は図形で描く、表情、口パク、文字送り、選択肢、フラグ、イベント。JSON） | `game_core::dialogue::{Script, Talk}` |
| 持ち物・お金・装備・ショップ（JSON のアイテム定義、スタック、装備の能力値、売買、保存） | `game_core::items::{ItemBook, Bag, Gear, Shop}` + `hud_kit::Menu` |
| 物理（横から見た箱とボール：落ちる、転がる、倒れる、積む、崩れる、爆発、眠る） | `game_core::physics2d::{World, Body, Shape}` |
| キャラの動き（待機・歩き・走り・ジャンプ、攻撃の当たるタイミング、やられ、倒れたまま。リグ付きは glTF クリップ、無しは手続きのポーズ） | `game_core::anim::{Animator, MoveSet}`（KayKit は `MoveSet::kaykit()`） |
| イージング、トゥイーン、タイマー、行って戻る動き | `game_core::tween::{Ease, Tween, Timer, ping_pong, pulse}` |
| ダメージ数字や「MISS」が場所から浮かんで消える | `game_core::floats::Floats` |
| ジャンプ以外のボタン（J/K/Shift、パッド B/X） | `KitHost::buttons` |
| カメラ（縦でも横でも範囲が収まる）、タップした物・地面 | `game_core::view::{View::frame, project, ground_at, pick}` |
| 群れの体の動き（ふわふわ、よろよろ、ぴょん、すべり、どしん。JSON で `Motion::parse`） | `game_core::motion` |
| 敵プール、空間グリッド、出現予算、シード付き乱数 | `game_core::swarm` |
| 敵の JSON 定義（歩く・飛ぶ・跳ぶ・予告） | `game_core::enemies`（`docs/enemy_kit.md`） |
| ライフ、ミッション、結果、ゲームの手触り | `game_core::{lives, missions, result, juice, feel}` |
| 角丸パネル、ゲージ、ボタン | `hud_kit::{round_panel, gauge, pill_button}` |
| 出すモデルを最初に宣言（途中で出る・消えるモデルでカクつかない） | `game_core::declare_models` |
| 角丸・円・リング | `Quad::round` / `Quad::circle` / `Quad::ring` |
| 数字・タイトル用の書体 | `TextQuad::with_face(TextFace::Display)`（Cinzel） |
| コイン、ショップ、デイリー | マニフェストの `meta`（`docs/meta_core.md`） |
| 広告枠 | マニフェストの `platform` |
| 画質段階 | `KitHost::supports_quality_tiers` と `quality_hints` |

## 4. ページの機能はマニフェストで頼む

`games/<slug>_minimal.json` の `page`（`kagra/page_spec.py` が検証）:

```json
"page": {"best": true, "lang": true, "pause": true, "fixed_step": true, "blurb": "blurb.stick"}
```

`audio`、`save`（`KitHost::save_json` の進み具合を端末に保存）、`one_button`、`music_sync`、`revive`、`direct`、`tagline`、`theme`、
`default_quality`、`quality_words`、`url_params` もある。`play.html` に
ゲーム名やジャンルで分岐を書かない（`tests/test_page_spec.py` が止める）。

## 5. 素材

- CC0 かオリジナルだけ。Quaternius は使わない。有料素材は先にオーナーに聞く。
- 足したら台帳（`assets/LEDGER.json`）に記録し、`python3 tools/asset_ledger.py credits --write`
  と `python3 tools/asset_ledger.py check`（0 errors）。
- モデルは同梱素材から（`assets/models/kenney/...`）。マニフェストの `bundle_assets` に足す。
- フォントは OFL。日本語の新しい漢字を使ったら `python tools/subset_fonts.py`
  （テストが教えてくれる）。

## 6. 出荷チェック表

PR を「出せる」と言う前に全部確認する。

- [ ] `cargo fmt -p kagra-shared -- --check` / `cargo clippy ... -D warnings`（render あり・なし）/ `cargo test -p kagra-shared`
- [ ] `pytest tests -m "not golden"` の関係する所、`python examples/<slug>_minimal.py --check`
- [ ] ブラウザ起動確認: `node scripts/browser_smoke.mjs dist/preview <slug>`（CI でも走る）
- [ ] タイトル → プレイ → 結果 → もう一回 が回る。ベストが残る。一時停止できる
- [ ] 最初の 10 秒で遊び方が分かる（タイトルの説明文、`page.blurb`）
- [ ] モデルの組が途中で変わらない（`declare_models` と、プレイ中に `gpu_mesh_key` が変わらないテスト）
- [ ] スマホで 60 fps 近い（`?dev=1` の計測。Pixel 10 などの PowerVR は 1.5 倍で描く）
- [ ] 日本語と英語の両方で文字が欠けない
- [ ] 圧縮後の読み込みが 3 MB 前後に収まる
- [ ] 素材の台帳とライセンスチェックが緑
- [ ] スマホで触った人の感想をもらった（プレビューのリンクを渡す）

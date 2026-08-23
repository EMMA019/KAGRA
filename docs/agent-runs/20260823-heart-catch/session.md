# Session — 2026-08-23 Heart Catch

エージェント: Cursor Grok 4.6（このリポジトリの開発ループに従う）
規約: ルート `AGENTS.md` / `.cursor/skills/kagra-agent/SKILL.md`

## 0. 指示

`prompt.md` の一行だけ。追加の仕様は聞いていない。

## 1. API 検索（推測する前）

`docs/API_INDEX.md` で次を確認した。

| 欲しいもの | 索引にあった名前 | メモ |
|---|---|---|
| シーン / ループ | `init` `run` `Scene` `cls` `text` `fill` `key` `pressed` | 安定コア |
| VRM | `avatar` `draw_vrm` `ensure_vrm` | checkout に `.vrm` が無い |
| 位置 | `VrmAvatar.set_position` / `set_yaw` | D-1 で公開済み。索引の関数表には無くクラスメソッド |
| ワールド→HUD | `Camera3D.world_to_screen` | 索引の `world_to_screen(wx, wy)` は **2D**。3D はカメラ側 |
| 手続きアート | `texture_from_fn` `tone` `draw_billboard` `disk_mesh` | D-1 |
| セーブ | `save_json` / `load_json` | **`load_data` はアセットレジストリ**。使わない |
| アクション | `ActionController` | **索引に無い**。Orb Rush は import しているが、この実証では使わない |

## 2. アセット

`kagra.contracts` の Emma エイリアスを探した。`assets/` に VRM は 0 件。
`tests/fixtures/` にあるのは synthetic BVH だけ。

→ `kagra.ensure_vrm()` を使う（無ければ Alicia サンプルを取る）。
Orb Rush が `assets/Emma.vrm` 固定で落ちるのも同じ穴。スモーク時は
`ensure_vrm()` にフォールバックするよう直した。

## 3. 実装の判断

Orb Rush のコピーはしない（星/爆弾/45 秒）。プロンプト通り 3 レーン +
奥から来るハート。ルール（レーンクランプ、キャッチ判定、スコア）は
`examples/heart_catch_rules.py` に切り出し、GPU なしでテストできるようにした。
本体 `examples/vrm_heart_catch.py` は公開 API だけ。

## 4. 躓き（省略しない）

1. **`kagra_core` がこの環境に無い。** `import kagra` は
   `maturin develop` が必要。`verify` をここで閉じられなかった。
   ルールテストと「私用 import 禁止」テストだけ先に通した。
2. **参照 verify が壊れていた。** `orb_rush_smoke.json` は存在しない
   `scratch/smoke_orb_rush.py` を指していた。実ファイル
   `examples/vrm_orb_rush.py` + `KAGRA_SMOKE=1` に付け直した。
3. **`ActionController` は公開索引に無い。** 使わず `play` / `feel` だけ。
4. **`world_to_screen` の二重定義。** 2D 関数と同名。3D は
   `Camera3D.world_to_screen`。ハートは `draw_billboard` で足りたので投影は未使用。
5. **`heart_catch_rules` の import パス。** `sys.path` に `examples/` を足さないと
   リポジトリルートからの実行で落ちる。

## 5. verify

書いたシナリオ: `examples/verify_scenarios/heart_catch_smoke.json`
（`KAGRA_SMOKE=1` で 48 フレーム、24 フレーム目にスクショ）。

この VM では `kagra_core` 未ビルドのため **未実行**。次の閉じ方:

```bash
pip install maturin && maturin develop
python -m kagra.verify examples/verify_scenarios/heart_catch_smoke.json
```

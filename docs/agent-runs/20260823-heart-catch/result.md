# Result — Heart Catch（往復 1）

## 成果物

| パス | 役割 |
|---|---|
| `examples/vrm_heart_catch.py` | ゲーム本体（公開 API のみ） |
| `examples/heart_catch_rules.py` | レーン / キャッチ判定（GPU 不要） |
| `examples/verify_scenarios/heart_catch_smoke.json` | ヘッドレス検証 |
| `tests/test_heart_catch.py` | ルール + 私用 import 禁止 |
| `examples/vrm_orb_rush.py` | `ensure_vrm` フォールバック + `KAGRA_SMOKE` |
| `examples/verify_scenarios/orb_rush_smoke.json` | 欠落スクリプトを実ファイルへ |

## テスト

`pytest tests -m "not golden"` — ルールと ban テストを含む全件。

## verify

**この環境では未実行**（`kagra_core` 拡張が無い）。シナリオとスモーク経路は置いた。

## 往復数

1。プロンプト → API 検索 → 実装 → ルールテスト。GPU verify は次の往復。

## セッション後に issue 化すべき穴

1. `orb_rush_smoke.json` が幽霊スクリプトを指していた（この PR で修正）
2. エージェント向けに「3D の `world_to_screen` は Camera3D 側」と索引で区別した方がよい
3. `ActionController` を公開するなら `__init__.py` 再エクスポート + 索引
4. checkout に VRM が無いときの公式経路は `ensure_vrm()` — Orb Rush README が Emma 固定だった

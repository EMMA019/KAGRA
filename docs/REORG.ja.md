# 旧エンジン分離 — 実施済み（2026-09-01）

旧エンジン（kagra-core / RendererV2 / 0.19 pip デモ時代）を `old/` に隔離し、
新本線（kagra-shared wgpu 30 + Python ゲームマスター）と混ざらないように
した。**`import kagra` は引き続き動く**（コンパイル済み拡張 + kagra/ の
0.19 API は残置。pip デモは old/kagra-core からビルド）。

## 移動済み

| 現在 | 移動先 | 内容 |
|---|---|---|
| `kagra-core/` | `old/kagra-core/` | wgpu 0.19 RendererV2 の Rust クレート（単体 workspace 化、pip ビルドは `old/kagra-core/Cargo.toml` を参照） |
| `examples/vrm_*.py` 等 28 本 + rules + mp4 | `old/examples/`（media/ 含む） | 旧 RendererV2 デモ・録画 |
| `examples/assets/`（open_world / relic_run） | `old/examples/assets/` | 旧デモのアセット |
| `examples/archive/` | `old/examples/archive/` | 旧 2D / タイルマップ / エディタ |
| `examples/verify_scenarios/` の旧デモ 11 件 | `old/examples/verify_scenarios/` | 旧 RendererV2 の smoke シナリオ |
| `docs/archive/` | `old/docs-archive/` | 旧ガイド・旧 63% ロードマップ |
| `app/`・`template/` | `old/` | 旧アプリ・旧テンプレート |
| 旧デモ検証テスト 6 本 | `old/tests/` | test_dodge_room / heart_catch / open_world / prop_garden / relic_run / switch_room |

## 削除

- `map/`・`models/`・`cache/`・`.vision-pm`・`examples/kagra_romance`（未追跡のゴミ）
- ルート Cargo workspace から kagra-core を外す（members = kagra-shared のみ）

## 橋渡し（テストはスイートに残し、パスだけ old/ へ）

- `tests/test_*` の `examples/vrm_*` / `kagra-core/src/*` 参照 → `old/...`（18 + 4 箇所）
- `tests/conftest.py` が `old/examples` を sys.path に追加（camera3d / look が
  `*_rules` 定数を参照）
- `AGENTS.md`・`.github/workflows/ci.yml`・`docs/PUBLISHING.md`・README 更新

## 残す（新本線）

- `kagra-shared/`（新エンジン）/ `kagra/`（新モジュール + 0.19 API 同居。
  完全分離は別タスク）/ `examples/`（新ゲーム + verify_scenarios）/
  `tests/` / `mobile/`（shared のモバイルシェル）

# 旧エンジン分離計画（REORG）— ポート完了後に実施

目的: 旧エンジン（kagra-core / RendererV2 / 0.19 pip デモ時代）を `old/` に
隔離し、新本線（kagra-shared wgpu 30 + Python ゲームマスター）と混ざらない
ようにする。**Phase 0 の移植（特に① spring/袖）が kagra-core のソースを
参照するので、ポート完了後に実行する。**

## 移動先: `old/`

| 現在 | 移動先 | 内容 |
|---|---|---|
| `kagra-core/` | `old/kagra-core/` | wgpu 0.19 RendererV2 の Rust クレート（旧エンジン本体）。pip デモのビルド設定ごと |
| `examples/vrm_*.py`（全 vrm_ 接頭辞） | `old/examples/` | RendererV2 デモ（open_world / orb_rush / heart_catch / relic_run / sing_dance / vrma / facetrack / kairi_chat / multi_avatar / stream / prop_garden / pretty_room / overworld / dodge_room / switch_room / action_demo 等） |
| `examples/*_rules.py`（open_world_rules / relic_run_rules / heart_catch_rules / dodge_room_rules / switch_room_rules / prop_garden_rules） | `old/examples/` | 旧デモのルール定数 |
| `examples/desktop_mascot.py` / `agent_verify_demo.py` / `demo_live_smoke.py` | `old/examples/` | kagra-core 時代のデモ |
| `examples/*.mp4`（レコーディング + dance.mp4 + game 154555.mp4） | `old/examples/media/` | 旧録画・成果物 |
| `docs/archive/` | `old/docs/` | 旧ガイド・旧 63% ロードマップ |
| `mobile/`（要確認: 運転デモは shared 時代の可能性） | 判定してから | Android/iOS の運転デモ |
| `map/` / `models/` / `cache/` / `.vision-pm` / `app/` / `template/` | 判定してから | 未使用なら削除 or old/ |

## 削除

- `kagra/kagra_core.pyd` 等のビルド成果物（分離後は old/ 側でビルドするため）
- 上で移動/判定された未使用ファイル

## 残す（新本線）

- `kagra-shared/` — 新エンジン（wgpu 30）
- `kagra/` — Python パッケージ（**新モジュール gameloop / ui2d / audio /
  bunny_garden / torneko / contracts / verify はここに残る**。0.19 の旧 API
  モジュールも現時点では同居 — `import kagra` が kagra_core を必須にして
  いるため。完全分離は別タスク）
- `examples/`（新: bunny_garden_minimal / torneko_minimal /
  python_game_minimal / ui_panel_demo / world_doc_window）
- `tests/` / `docs/`（新ロードマップ等）/ `scripts/` / `tools/` / `assets/`

## 実施時の注意

1. README の pip デモ節を `old/` への案内に書き換え（README 行テストを更新）
2. ルート `pyproject.toml` の kagra-core ビルド参照を分離
3. `pytest tests -m "not golden"` 全パスを確認（旧デモの README 行アサートを更新）
4. ログ: `docs/agent-runs/YYYYMMDD-reorg-old/`

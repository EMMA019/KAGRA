# Session

## API 検索

- `docs/API_INDEX.md`（Python 公開 API 索引、AST 生成）を確認 → Rust 側
  （kagra-shared）は含まれない。`tools/gen_api_index.py` は `kagra/__init__.py`
  のみ対象。Rust のドキュメントはソースコメント + `docs/schemas/world.json`。
- 棚の破片を確認:
  - event: `kagra/event_bus.py`（`on` / `emit` / `once` / `off`、priority 付き
    リスナー、deferred flush）→ ランタイムリスナー型。dump に載せると
    「emit → take_events で読む」のデータ駆動になる
  - interact: shared の `rpg.rs::near(doc, name, reach)`（距離判定 + attack）、
    Python の `hovered_prop` / `clicked_prop` → prop のメタデータ + 距離で圧縮
  - timer: `cook.rs`（`game.wait += dt` を `COOK_S` まで）、`fish.rs`
    （`waiting` + `wait`、cast → WAIT_S → bite）→ モジュール内カウントダウン
  - state→animation: `WorldWalker.clip`（歩行クリップ秒）はあるが状態名がない
    → `anim: String` を追加

## 設計

4 API すべてを WorldDoc（dump JSON）に載せ、WorldPlay が汎用に処理する:

| API | 表現 | WorldPlay |
|---|---|---|
| timer | `doc.timers[]`（id/name/seconds/remaining/on_done/active） | `start_timer` / `tick_timers`（0 で on_done を emit） |
| event | `doc.events[]`（name/count/data） | `emit_event`（同名集約）/ `take_events`（消費） |
| interact | `prop.interact`（kind/prompt/on_use/reach） | `nearest_interact` / `step_interact`（J で on_use を emit） |
| anim | `walker.anim`（"idle"/"walk"/ジャンル名） | step_walker が wish から walk/idle を導出。ジャンル設定は静止中保持 |

境界: エンジンはデータを運ぶだけ。対話の台詞・報酬・「bite で何をするか」は
ゲーム側（ジャンルコード）が `take_events` で読んで決める。

## 実装の往復

1. `world_doc.rs`: `WorldTimer` / `WorldEvent` / `WorldInteract` struct、
   `WorldDoc.timers` / `WorldDoc.events`、`WorldProp.interact`、
   `WorldWalker.anim`（default "idle"）を追加
2. `world_play.rs`: `emit_event` / `take_events` / `start_timer` / `tick_timers` /
   `nearest_interact` / `step_interact`、step_walker の anim 導出、tick への
   組み込み、HUD の interact プロンプトバー
3. **躓き 1**: `WorldProp` にフィールド追加で全フィールド明示の初期化 7 箇所
   （action2d.rs ×3、rpg.rs ×3、td.rs ×1）がコンパイルエラー → 各所に
   `interact: None` を追加
4. **躓き 2**: verify シナリオの query に `type: timer/event` を書いたが、
   Python の `world.query` は prop/walker/light/camera/terrain_tile のみ →
   シナリオから外す（timer/event の動作は Rust テストで担保）
5. **躓き 3**: `interact_fish_world.json` に `player` トップレベルはあるが
   `walkers[]` 配列が空 → Python の query が player を見つけられず
   `want count 1 got 0` → walkers 配列にも同じ walker を追加（既存
   emma_walker_world.json と同じ形）
6. テスト 6 本追加（interact 2、event 1、timer 1、anim 1、統合 1）→
   **358 passed / 0 failed**
7. verify シナリオ `interact_fish_smoke.json` → `ok: true`（world + offscreen）

## 次（未実施）

- 既存ジャンル（fish.rs / cook.rs）の内部タイマーを `doc.timers` へ移行するか
  はゲーム側の選択。エンジン API は独立して使える
- `WorldPlay` の `anim` を VRM クリップ（cast/reel 等）へ実際に繋ぐのは、
  アニメーション資産を持つゲーム側

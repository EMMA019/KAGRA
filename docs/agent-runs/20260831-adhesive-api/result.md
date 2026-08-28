# Result

## What landed

接着 API 4本（interact / event / timer / state→animation）を共有ランタイムの
**WorldDoc（dump JSON）+ WorldPlay（純ロジック）** に載せた。ジャンル専用コード
（台詞・報酬・bite の反応）はエンジンに入れていない。

- `WorldDoc.timers[]` — `WorldPlay::start_timer(name, seconds, on_done)` で起動、
  `tick_timers` がカウントダウンし 0 で `on_done` イベントを emit（cook/fish の
  モジュール内待ちの汎用化）
- `WorldDoc.events[]` — `emit_event`（同名集約、payload 付き）/ `take_events`
  （消費）。Python 棚 `event_bus.py` のランタイムリスナーを「dump がバス」に圧縮
- `WorldProp.interact` — `kind/prompt/on_use/reach`。`nearest_interact` /
  `step_interact` が J/attack で on_use を emit（rpg talk / hovered_prop の圧縮）
- `WorldWalker.anim` — エンジンが wish から walk/idle を導出、ジャンル設定
  （"cast" 等）は静止中保持。状態→アニメの境界（ゲームが決める、エンジンは運ぶ）

## Commands

```text
cargo test -p kagra-shared --lib
# 358 passed; 0 failed
#   new: interact_emits_on_use_within_reach / interact_out_of_reach_does_not_emit
#        events_aggregate_and_take_consumes / timer_counts_down_and_emits_on_done
#        anim_follows_wish_and_keeps_genre_state / cast_interact_timer_bite_loop

python -m kagra.verify examples/verify_scenarios/interact_fish_smoke.json
# ok: true（world: walker+shore、offscreen: shared wgpu 30 PNG smoke）
```

## Try

```text
# 1) J at the shore → cast event → 3s timer → bite event → anim hit
cargo test -p kagra-shared --lib cast_interact_timer_bite_loop -- --nocapture

# 2) 新しい dump で遊ぶ（箱の shore に近づいて J）
python -m kagra.play_world kagra-shared/tests/fixtures/interact_fish_world.json
```

## Files

- `kagra-shared/src/world_doc.rs` — WorldTimer / WorldEvent / WorldInteract /
  interact / anim / timers / events
- `kagra-shared/src/world_play.rs` — emit_event / take_events / start_timer /
  tick_timers / nearest_interact / step_interact + anim 導出 + HUD プロンプト
- `kagra-shared/tests/fixtures/interact_fish_world.json` — 接着 API デモ dump
- `examples/verify_scenarios/interact_fish_smoke.json` — verify
- `docs/schemas/world.json` — timer / event / interact / anim スキーマ

## Stuck（= ドキュメントの穴）

- `WorldProp` の全フィールド明示初期化が 7 箇所（action2d/rpg/td）— フィールド
  追加のたびに壊れる。`..Default::default()` への寄せは徐々に
- Python `world.query` は timer/event を query できない — 必要になったら
  `_timer_rows` / `_event_rows` を足す（今回は Rust テストで担保）
- verify シナリオの query は「dump の walkers[] 配列」を見る — 新 dump は
  player と walkers の両方に同じ walker を書く

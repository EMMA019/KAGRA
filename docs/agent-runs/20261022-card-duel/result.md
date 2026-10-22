# Result

- `kagra.CardSession` / `kagra_shared.CardSession` — 20枚の1対1。山札、seed シャッフル、ドロー、手札、マナ、プレイ、相手AI、勝敗、再戦。`dump` / `from_json` は隠れた山札と相手手札ごと往復する。
- `kagra/card_ui.py` — 扇、ドラッグ（マウスとタッチ）、めくり、tween、文字と模様のカード面。他のカードゲームから使える。
- `examples/card_minimal.py` — 短い対戦。結果のあと `confirm` で同じ seed の再戦。
- `examples/verify_scenarios/card_smoke.json`
- `kagra-shared/tests/fixtures/card_world.json` — フェルト、`crate.glb` の山札、チェスト、ナイト。`WorldPlay` には足していない。

## Verify

- `cargo test -p kagra-shared --lib --locked` — 490 passed（origin/master を merge したあと）
- `cargo clippy -p kagra-shared --lib --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --lib --locked --features render -- -D warnings` — clean
- `pytest tests -m "not golden"` — 684 passed, 14 skipped
- `python -m kagra.verify examples/verify_scenarios/card_smoke.json` — ok（lavapipe）
- 既存の verify シナリオもプロセスごとに ok。`demo_live` と `physics_stack` は既知の失敗として未実行。
- `python examples/card_minimal.py --headless` — `dragged=True winner=1`、再戦で体力 20。PNG は lavapipe（llvmpipe）の 960x540。

Screenshot: `/opt/cursor/artifacts/card_duel.png`（手元のコピー `scratch/card_duel.png`）。

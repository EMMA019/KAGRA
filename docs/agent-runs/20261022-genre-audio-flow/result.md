# Result

- 既定パッケージ Box Hop（`games/platformer_minimal.json` / `examples/platformer_minimal.py`）がタイトルで止まり、Space / Enter / タップで始まり、P か右上 PAUSE で止まり、RETRY / TITLE に戻る。
- ハイスコアと音量は `{game_id}.json`。Windows は `%APPDATA%\kagra`、Android は `filesDir/kagra`。
- `kagra.audio` の BGM フェード、効果音、グループ、ミュート。ジャンプ / 発射 / カード。クリップは `assets/audio/`（CC0 合成）。デバイス無しは無音。
- lavapipe: `/opt/cursor/artifacts/box_hop_title.png`, `/opt/cursor/artifacts/box_hop_pause.png`。文字は BOX HOP / はじめる / START と PAUSE / ポーズ。

## コマンド

- `cargo test -p kagra-shared --lib --locked` — 508 passed
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings` — clean
- `cargo clippy -p kagra-shared --features python --locked -- -D warnings` — clean
- `pytest tests -m "not golden"` — 失敗は既知の `test_mainline_import_is_not_old_engine` のみ
- `python -m kagra.verify` — `demo_live_smoke` はスクリプトが無くスキップ。残り 43 本は lavapipe + 拡張ありで成功

Package Windows / Android のワークフローはこのブランチの push で CI が回す。ローカルでは zip / APK は作っていない。

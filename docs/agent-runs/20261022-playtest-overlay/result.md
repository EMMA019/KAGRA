# 結果

- オーバーレイ: `kagra-shared/src/playtest.rs`、Python `kagra/playtest.py`、ブラウザ `kagra-shared/www/playtest.js`、Android `PlaytestOverlay.kt`
- 再現: `scripts/repro_issue.py`
- テンプレート: `.github/ISSUE_TEMPLATE/playtest.yml`（label `playtest`）
- 手順: `AGENTS.md` ルール 9

## 確認

- `cargo test -p kagra-shared --locked` — 521 passed
- `cargo test -p kagra-shared --features render --locked` — 527 + offscreen 12 passed
- `cargo clippy -p kagra-shared --all-targets -- -D warnings`
- `cargo clippy -p kagra-shared --all-targets --features render -- -D warnings`
- `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render --release --locked`
- `pytest tests -m "not golden"`
- `python tools/gen_api_index.py --check`

lavapipe（`VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json`）:

```bash
cargo run -p kagra-shared --features render --example offscreen -- 1280 720 playtest_overlay.png playtest
```

パネルに「直してここ」「送る」「コピー」「閉じる」が出る。同じフレームのパネル無しが `playtest_overlay_game.png`。

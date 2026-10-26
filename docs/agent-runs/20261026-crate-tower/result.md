# Result

- キット: `kagra-shared/src/crate_tower.rs`、マニフェスト `games/crate_tower_minimal.json`（best / audio / lang / pause / fixed_step / one_button）。
- 確認: cargo fmt / clippy / cargo test、wasm ビルド、ヘッドレス Chromium で縦横ともタイトル → プレイ → ミス 3 回でゲームオーバー → 結果（PNG はリポジトリに入れない）。
- まだ: スマホで触った感想。ゆれの速さとぴったりの幅（`speed`、`perfect_tol`）は触ってから調整する。

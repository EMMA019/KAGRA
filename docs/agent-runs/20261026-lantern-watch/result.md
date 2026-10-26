# Result

- キット: `kagra-shared/src/lantern_watch.rs`（テスト 9 件: 仕様の厳しさ、出現と前進、1 タップ / 3 タップ、空振りでコンボ切れ、ランタンが消えて負け、夜明けで勝ちとボーナス、縦横で盤面が画面内、一時停止、焼いたワールド = 既定値）。
- マニフェスト: `games/lantern_watch_minimal.json`（best / audio / lang / pause / fixed_step）。
- 確認: cargo fmt / clippy（render あり・なし）/ cargo test、pytest、wasm ビルド、ブラウザでタイトル → プレイ → タップで撃破（ヘッドレス Chromium のスクショで確認。PNG はリポジトリに入れない）。
- 往復: 絵 3 回、テスト 2 回（コンボ倍率の初撃を x1 に、クリッピーの範囲比較）。
- まだ: スマホで触った感想（プレビューのリンクを Emma に渡す）。

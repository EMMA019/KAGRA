# Result (round 1)

- キット: `kagra-shared/src/station_escape.rs`（+ `stages.rs`, `tests.rs`）、マニフェスト `games/station_escape_minimal.json`。
- テスト 11 件: 仕様、足場の上に spawn / checkpoint / exit、物語カード、タッチ操作、セル・チェックポイント・レーザー・落下、崩れる足場とリフト、ボットが 2 面クリアして結果へ、縦横で画面内、GPU のモデルの組が変わらない、一時停止、焼いたワールド。
- ヘッドレス Chromium: 物語カード → 走る（キーボード）→ セルを取る、を縦横で確認。
- まだ: スマホでの手触り（ジャンプの高さ、ダッシュの距離、ボタンの位置）。

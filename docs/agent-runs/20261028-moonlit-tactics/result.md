# Result (round 1)

- キット: `kagra-shared/src/moonlit_tactics.rs`（+ `rules.rs`, `maps.rs`, `doc.rs`, `hud.rs`, `tests.rs`）、マニフェスト `games/moonlit_tactics_minimal.json`。
- テスト 15 件（rules 5 + キット 10）: 仕様、画面タップで選択 → 移動 → 戻す → 攻撃、ターン終了 → 敵が動く → 2 ターン目、慎重な指揮官ボットが 2 マップ勝って結果へ、墓守を倒すと勝ち、全滅で負け、縦横で盤が収まる、GPU のモデルの組が変わらない、焼いたワールド。`game_core::grid` 2 件。
- ヘッドレス Chromium: 縦横で物語カード → 味方のターン。
- まだ: スマホでの手触り（タップの当たり、難しさ、テンポ）。

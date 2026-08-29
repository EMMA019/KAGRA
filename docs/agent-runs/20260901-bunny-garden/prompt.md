# Prompt — 1 本目ジャンル: バニーガーデン系ミニマルゲーム

> 3 ジャンル計画（トルネコ / SLG / バニーガーデン）の 1 本目は
> 「バニーガーデン系」（提案どおり。システム最軽量 + VRM 資産活用、
> 1 本目は私が判断）。共通コア（文字・2D UI・音・マウス）はスライス
> ①〜④で揃った。

指示（実質）:

- **ゲームロジックは全部 Python**（`Scene.update`）。Rust は世界の tick と
  描画だけ。今回は世界がほぼ静止するので WorldPlay すら使わず、dump dict +
  `draw_world(world, w, h, hud=...)` で描く（世界はデータの証明）。
- 要素: Character（VRM: assets/Emma.vrm）/ 会話（好感度ティアで台詞が
  変わる）/ 好感度（0..100、話す・ほめる・ドリンクで増える）/ 日程
  （1 日 = 営業 → メニュー → 閉店 → 翌日）/ お金・ドリンク在庫 /
  セーブ・ロード / 特別イベント（好感度 50）。
- 決定論 RNG（LCG、日付でシード）— トルネコの再現性方針と整合。
- UI: kagra.ui2d（メッセージ / 選択肢 / 好感度バー / 在庫リスト）。
  入力: ↑↓ + Z/J/Enter、X で戻る、マウスクリックで選択肢を選べる。
- SE: kagra.audio（ok / coin / bite / hurt / cast）。
- ヘッドレス verify: `--headless out.png --days N` で N 日回して
  PNG + 最終状態 JSON を出す（同じ `_do_choice` 経路を通す）。
- テストは純ロジック（kagra_core / kagra_shared 非依存、tmp_path で保存）。
  コミット + ログ（docs/agent-runs/）を閉じる。

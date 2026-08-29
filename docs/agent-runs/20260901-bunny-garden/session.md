# Session — バニーガーデン 1 本目（2026-09-01）

## 設計

- `kagra/bunny_garden.py` — ゲーム本体（`BunnyGarden(Scene)`）。kagra_core
  非依存（テスト可能）。
- `examples/bunny_garden_minimal.py` — 薄いランナー（窓 / ヘッドレス）。
  `_headless_policy`: 1 日 = 話す → ほめる → モヒート → 閉店。
- 世界 dump: 部屋（床・カウンター・ランプ 2 灯・暖色ポイントライト）+
  Emma（assets/Emma.vrm、オフスクリーンで VRM が描けることを事前確認済み）+
  正面カメラ。WorldPlay は使わない（世界が静止しているので）。
- 状態機械: msg（メッセージ表示、Z で進む）→ menu（4 択）→ drink
  （在庫サブメニュー）→ end（閉店、Z で翌日）。UI とヘッドレスは同じ
  `_do_choice` / `_do_drink` を通る。
- RNG: LCG（`rng = (rng*1103515245+12345) & 0x7FFFFFFF`）、日付でシード。
  ほめる +2..5 が日ごとに決定的（再現可能）。
- セーブ: JSON（day / money / stock / affection / events）。閉店時に保存。

## 躓き

1. **`list` と dict の混在は前スライスで踏んだので回避済み**（ui2d 側を修正済み）。
2. **ヘッドレスで日付が進まない**: `_headless_policy` は閉店で state=end に
   なるが `_next_day()` を呼んでいなかった。3 日回しても day=1 のまま。
   → ループで毎回 `_next_day()`（対話プレイと同じ「閉店確定 → 翌日」）。
3. **ヘッドレスの msg 待ち**: ポリシーがメッセージ確認を挟むと menu に
   戻らない。`_drain()`（キューを流して menu へ）をポリシー冒頭で呼ぶ。
4. **在庫表示の `or "なし"` が効かない**: `f"在庫: " + join(...) or "なし"`
   は接頭辞が truthy で常に表示。join を先に組んでから `or "なし"`。
5. コンソールの JSON 表示が文字化け（PowerShell のコードページ）— ファイル
   は UTF-8 で正しく書かれる（読み戻しテストで確認）。

## 検証

- pytest tests/test_bunny_garden.py: 11 件（初期状態 / 会話 / ほめる /
  決定論 RNG / ドリンク消費 / 在庫ゼロ表示 / 閉店売上 + セーブ /
  セーブ・ロード往復 / 特別イベント / 好感度上限 / 3 日ヘッドレス）。
- `--headless --days 3`: day=4（次に開く日）、money 500、好感度 42、
  モヒート 0。PNG 10717 bytes。セーブは UTF-8 で正しい。
- `pytest tests -m "not golden"` 全パス（599 件）。

# Result — バニーガーデン 1 本目

## 成果物

- `kagra/bunny_garden.py` — `BunnyGarden(Scene)`: 会話（ティア別台詞）/
  好感度 0..100 / ドリンク在庫とお金 / 日程（1 日 = 営業 → メニュー →
  閉店 → 翌日）/ 決定論 RNG / セーブ・ロード / 特別イベント（好感度 50）。
  世界は dump dict（VRM Emma + 部屋 + 暖色ライト）。WorldPlay 不要。
- `examples/bunny_garden_minimal.py` — 窓 / `--headless out.png --days N`
  （PNG + 最終状態 JSON + セーブ）。
- `tests/test_bunny_garden.py` — 11 件（純ロジック、kagra_core 非依存）。
- README / README.ja — 1 本目ジャンルの実行例を追加。
- ログ: `docs/agent-runs/20260901-bunny-garden/`

## verify

- pytest 全パス（599 件）。
- `python examples/bunny_garden_minimal.py --headless scratch/bunny.png --days 3`
  → `{"day": 4, "money": 500, "stock": {"モヒート": 0, "オレンジジュース": 2},
  "affection": {"ミミ": 42}}`、PNG 10717 bytes。
- セーブは UTF-8 で読み戻せる（往復テストで確認）。

## 意味

3 ジャンル計画の「共通項 = 時間が進む → イベント → 状態が変わる」の実演:
日付（時間）→ 会話/ドリンク/閉店（イベント）→ 好感度/お金/在庫（状態）。
VRM キャラ + 文字 + UI + 音 + マウス + セーブが全部 Python だけで動く。

## 次の山

⑥ トルネコ（seed 決定論 + グリッド + ターン + 在庫）→ その後 SLG。

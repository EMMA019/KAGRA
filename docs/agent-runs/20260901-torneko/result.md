# Result — トルネコライク

## 成果物

- `kagra/torneko.py` — `Torneko(Scene)`: seed 決定論ダンジョン（MapGen）/
  ターン制（プレイヤー → 敵）/ 敵 AI（近接・追跡・徘徊）/ 在庫（薬草・
  種）/ 宝箱 / 階移動（3 階で脱出）/ 死亡とやり直し / JSON セーブ・ロード
  （RNG 状態込みで未来のロールまで再現）。
- `examples/torneko_minimal.py` — 窓 / `--headless out.png --seed N --turns M`
- `tests/test_torneko.py` — 18 件（純ロジック）。
- README / README.ja — 実行例を追加。
- ログ: `docs/agent-runs/20260901-torneko/`

## verify

- pytest 全パス（617 件）。
- `--headless --seed 12345 --turns 800` を 2 回 → 状態 JSON も PNG（MD5
  `745955…`）も完全一致。**同 seed → 同ダンジョン・同敵・同ロール・同描画**。
- 描画のクロスプロセス決定論も別途確認（同 dump → バイト一致）。

## 使い方

```bash
python examples/torneko_minimal.py --seed 12345          # 窓
python examples/torneko_minimal.py --headless out.png --seed 12345 --turns 800
```

## 意味

「seed = 12345 → floor = 7 → turn = 42 まで再現できる」という 3 ジャンル
計画の最重要要件（決定論 / スナップショット / replay）を実証。
バグ報告の再現性: 「このダンジョンのこのターンで敵が…」が seed + 階 +
保存 JSON で完全に再現できる。

## 次の山

SLG 系（ウイポ/F1 風: DB + 長期シミュレーション + スケジュール + 経済）。

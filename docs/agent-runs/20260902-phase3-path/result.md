# Result — Phase 3 完了（経路探索）

汎用エンジン化ロードマップ Phase 3（経路探索）が完了した。

## 設計

「ゲームロジックはパイソンのみ」の原則どおり、経路探索は **Python の
汎用モジュール**（`kagra/path.py`）として実装した。GPU 不要・決定論的
（乱数なし。同入力 → 同経路）。

| API | 用途 |
|---|---|
| `find_path(walkable, start, goal, *, diagonal=True)` | A*。障害物を避けて最短経路（両端含む）。到達不可は None。goal 自体が歩けないマスでもよい（敵マスへ詰める用途） |
| `move_range(walkable, start, budget, *, diagonal=True, cost_fn=...)` | SLG の移動範囲。移動力 budget 内で到達できるマス集合。地形コスト付き Dijkstra |
| `line_of_sight(walkable, a, b)` | Bresenham で直線が通れるか（射線・攻撃範囲・経路簡略化） |
| `simplify(path, walkable)` | LOS で冗長なウェイポイントを削る（string pulling 簡易版） |

## torneko 置換（実証）

手書き BFS だった `_path_to` を `kagra.path.find_path` に置換した。
契約は BFS 版と同一（start 除く・target 含む・敵マス不可・到達不可 []）で、
テスト 18 件 + ヘッドレス verify はそのまま緑。

## verify

- pytest: 582 パス（test_path.py 13 件追加: 直線 / 壁回避 / 到達不可 /
  対角近道 / goal ブロック / 移動範囲 / 地形コスト / LOS / simplify / 決定論）
- `gen_api_index --check` OK（Rust 変更なし）
- 決定論: find_path / move_range は同入力で同出力（テストで検証）

## 次の山

Phase 4 — 音 DSP（リバーブ / ダッキング / クロスフェード）。バニーガーデンの
BGM・SE に空間的な響きを足す。SLG（3 本目ジャンル）はこの移動範囲 API を
そのまま使える。

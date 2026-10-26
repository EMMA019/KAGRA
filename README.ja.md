# KAGRA

AI エージェントで**小さな 3D ブラウザゲームを量産する**ためのエンジン。
ゲームは短い Python と JSON で書き、共有の Rust ランタイム（wasm）がスマホの
ブラウザで 60 Hz で動かします。エージェントが作って、画面を見ずに検証し、
人がスマホでタップして試せるプレビューリンクを出します。

[English README](README.md)

> 現状（2026-10）: master でスマホで遊べるゲームが 4 本（HOP-BEAT、Neon Cat、
> Meteor Cat、Moonlit Swarm ステップ1）、共有ゲーム部品、リテンション層、
> スマホ計測ツール、ライセンス台帳があります。ポータルへの申請はまだです。
> 昔の「ヘッドレス collectathon」中心の README は [`old/`](old/README.md) と
> [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md) に履歴として残しています。

## 遊ぶ

PR ごとに GitHub Pages のプレビューが `https://emma019.github.io/KAGRA/pr/<n>/play.html` に出ます
（[docs/preview.md](docs/preview.md)）。master のルートの `play.html` は安定版の Lumen だけなので、
下のゲームは PR のプレビューに `?game=` を付けて開きます。例:
[`pr/209/play.html?game=survivor_arena`](https://emma019.github.io/KAGRA/pr/209/play.html?game=survivor_arena)。
スマホで開いてください。

| ゲーム | 内容 | クエリ |
|---|---|---|
| HOP-BEAT | ビートに合わせて跳ぶランナー（126 BPM、タップ/ホールド、PERFECT / GOOD） | `?game=hop_beat` |
| Neon Cat | 夜の街の 3 レーンランナー | `?game=neon_cat` |
| Meteor Cat | Neon Cat の着せ替え。Python 9 行と JSON | `?game=meteor_cat` |
| Moonlit Swarm | サバイバー系アリーナ。自動攻撃、経験値、レベルアップカード、デイリー | `?game=survivor_arena` |

`&dev=1` でスマホ計測オーバーレイ、`&quality=safe|mid|high|max` で画質の段階を
固定できます（ドラフト PR）。

## エンジンにあるもの

- **ゲーム仕様** — `games/<name>.json` に世界・ルール・調整値を置きます。Python が書き、
  Rust が動かします（wasm と Android に Python が無いため）。[docs/runner_kit.md](docs/runner_kit.md)。
- **共有部品（`game_core`）** — ライフ、ミッション、結果画面、ジューシーな演出、当たり判定、
  ジャンル中立の敵スキーマ（[docs/enemy_kit.md](docs/enemy_kit.md)。歩く・飛ぶ・跳ぶ・予告）、
  空間グリッド付きの敵プール（swarm）、メタ層。新しいゲームは `KitHost` の実装 1 つと
  `kit_host::KITS` の 1 行で足せます。
- **リテンション層** — コイン、アンロックのショップ、シード付きデイリーと連続日数、
  ラン修飾、任意のリワード広告枠。サーバーもネットワークも使いません
  （[docs/meta_core.md](docs/meta_core.md)）。
- **画質段階と計測** — `?dev=1` で fps、フレーム時間 p50/p95/p99、ジャンクを表示。
  60 秒レコーダーと「限界を探す」プローブで段階を選びます
  （[docs/phone_measure.md](docs/phone_measure.md)）。段階は JSON で定義。
- **素材とライセンス** — 公開リポジトリは CC0 かオリジナルの素材だけ。すべて台帳
  （[docs/asset_ledger.md](docs/asset_ledger.md)）に記録し、CI が確認します。有料素材は
  リポジトリに入れず、ビルド時にだけ取り込む方針です。
- **公開** — 無料で非独占のポータルを先に
  （[docs/publishing/web_portals.md](docs/publishing/web_portals.md)、
  [docs/PUBLISHING.md](docs/PUBLISHING.md)）。ゲームごとのメモは `docs/publishing/`。

## ゲームを作る

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv && source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install maturin && maturin develop              # Python ブリッジをビルド
python examples/meteor_cat_minimal.py               # 窓で起動。--headless out.png でオフライン描画
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000   # ブラウザ版
```

Neon Cat 級のゲームが 9 行（[docs/runner_kit.md](docs/runner_kit.md)）:

```python
from kagra.runner import play
raise SystemExit(play("games/meteor_cat_minimal.json"))
```

## AI エージェント向け

- **[AGENTS.md](AGENTS.md)** — どのエージェント（Claude Code、Cursor、Codex など）にも共通のルール。
- **API 索引** — [`docs/API_INDEX.md`](docs/API_INDEX.md)。AST から生成するので、推測せず検索する。
- **ループを閉じる** — `python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json`、オフスクリーン描画、プレイテスト bot、公平性チェッカー（シード付きコースを連続で再生）。
- **MCP サーバー** — `tools/mcp_kagra/server.py`（`kagra_api_search`、`kagra_verify`、`kagra_render`）。
- **小さな PR** — ドラフト PR、CI グリーン、スマホ用プレビューリンク。マージはオーナーの OK の後。`kagra-shared` の CI は約 30 分。
- **ビルド記録と設計** — [`docs/agent-runs/`](docs/agent-runs/README.md)、[`docs/design/survivor_arena.md`](docs/design/survivor_arena.md)、[CHANGELOG.md](CHANGELOG.md)。

## そのほかのジャンルと旧エンジン

`examples/` には以前のジャンル最小例（カード、リズム、レース、FPS、RPG、TD、
プラットフォーマー、パズル、格闘、サバイバルなど）と Python ゲームマスター
（ローグライク、バー経営、チェス）が残っています。同じ共有 wgpu 30 ランタイムで
動きますが、今の中心ではありません。最初のデモ（`pip install kagra`、VRM が歌って踊る）は
旧エンジンで動き、[`old/`](old/README.md) に移しました。新しいゲームはそこから始めないでください。
詳しい地図は [`docs/AGENT.md`](docs/AGENT.md)。

## ライセンス

MIT — [LICENSE](LICENSE)。第三者の素材は [CREDITS.md](CREDITS.md) と
[docs/asset_ledger.md](docs/asset_ledger.md) に載せています。フォントは OFL で、
ライセンス文を同梱します。旧 pip デモがダウンロードするサンプル VRM は Alicia Solid
（ニコニ立体ちゃん）© Dwango、[利用規約](https://3d.nicovideo.jp/alicia/rule.html)のもとで使っています。

KAGRA は神岡重力波検出器にちなんだ名前です。堅く、正確に、楽しく。

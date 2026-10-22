# KAGRA

**Python で AI に体を与えるなら KAGRA。**

AI エージェント向けのゲームエンジンです。API を検索し、シーンを書き、
**人間が画面を見ずに検証する。** 体が要るときは VRM。世界は dump JSON。
プレイは 1 本のループ。歌って踊るのは昔の 2 コマンド煙 — 製品ではない。

[English README](README.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m kagra.play_world                    # Crest Isle collectathon — wgpu 30 の窓、WASD
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json  # ヘッドレスでループを閉じる
```

これが本線です。**shared wgpu 30 ランタイム。** エージェント（人間でも）
はシグネチャを検索し、Python でゲームロジックを書き、`kagra.verify` で
閉じます。昔の `pip install kagra` デモ（RendererV2 で VRM が歌って踊る）
はアーカイブ — [pip デモ](#pip-デモ旧レンダラー) を見てください。

## エージェントにゲームを作らせる

このループがエンジンです。規則は [`AGENTS.md`](AGENTS.md)。Cursor は
`.cursor/skills/` から同じ規則を拾います。

1. **API 索引** — [`docs/API_INDEX.md`](docs/API_INDEX.md) は AST から生成。
   名前を発明しない
2. **アセット** — `kagra.contracts.resolve_asset`（MCP は `kagra_resolve_asset`）。
   別名: `Emma` / `walk` / `knight` / `inn` / `chest` …
3. **ヘッドレス検証** — `python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json`
4. **MCP** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` /
   `kagra_resolve_asset` / `kagra_verify` / `kagra_render`
5. **目** — `kagra.annotate`（クリックを数値に）と `kagra.debug_trace`
   （足と地形）。人間用エディタではない
6. **ビルドログ** — [`docs/agent-runs/`](docs/agent-runs/README.md)

これを貼る:

```
KAGRA の shared wgpu 30 本線（WorldDoc / WorldPlay / kagra.gameloop）で
短い 3D ゲームを作って。公開 API だけ。先に docs/API_INDEX.md を検索。
RendererV2 と old/examples は使わない。python -m kagra.verify で閉じて。
ログは docs/agent-runs/ に残して。
```

レシピ: [docs/recipes/agent-game.md](docs/recipes/agent-game.md)。
長い索引: [`docs/AGENT.md`](docs/AGENT.md)。

新しいゲームは Python。Rust は世界の tick と描画だけ。`WorldPlay` に
ジャンルモジュールを足さない。`WorldDoc.genre` は明示する。Prop 名は
景色です。

## Python でゲームを書く

ゲームロジックは全部 Python。`kagra_shared` は世界の tick と描画だけ。

```python
import json
import kagra
from kagra.gameloop import Scene, run, draw_world, pressed, was_pressed

class MyGame(Scene):
    def __init__(self):
        super().__init__()
        self.play = kagra.WorldPlay.from_json(open("world.json").read())
        self.play.confirm()                    # タイトル → プレイ（プレイ中は無視）
        self.world = json.loads(self.play.dump())

    def update(self, dt):                      # ← ゲームロジックは全部ここ
        lx = (1.0 if pressed("d") else 0.0) - (1.0 if pressed("a") else 0.0)
        lz = (1.0 if pressed("w") else 0.0) - (1.0 if pressed("s") else 0.0)
        self.play.set_input(lx, lz, False, was_pressed("j"), False)
        self.play.tick(dt)                     # エンジンが世界を進める
        self.world = json.loads(self.play.dump())
        if self.play.take_events("cast"):      # 接着イベント → こっちのロジック
            self.play.start_timer("cast", 3.0, "bite")

    def draw(self):
        self._canvas_png = draw_world(self.world, self.width, self.height)

run(MyGame())
```

[`examples/python_game_minimal.py`](examples/python_game_minimal.py) か、
Python ゲームマスター（bunny garden / torneko / bar sim / flight / 旗取り /
chess / physics）をコピーする。

```bash
cd kagra-shared && maturin develop --release && cd ..   # 最初の一度: kagra_shared をビルド
python examples/python_game_minimal.py --headless scratch/hello.png
python examples/bar_sim_minimal.py --headless scratch/bar.png --days 3 --seed 1
```

`kagra.WorldDoc` / `kagra.WorldPlay` / `kagra.render_world_doc` は
`kagra_shared` からの再エクスポート。`kagra.gameloop` は標準ライブラリ +
tkinter（torch はコアに入れない）。

## 本線

- **世界はデータ** — `World.dump()` / `WorldDoc`（`docs/schemas/world.json`）。
  `world.query` / `dump` / `load` でスクショなしに世界を読む。同じ JSON が
  デスクトップ窓・wasm・Android・iOS・オフスクリーンを駆動する
- **プレイは 1 本のループ** — `WorldPlay` がタイトル → プレイ → 結果。
  ジャンルコードはエンジンではなくゲーム側
- **接着 API** — `prop.interact` / `doc.timers` / `doc.events` /
  `walker.anim` / `walker.expression`。dump 自体がバス
- **絵** — dump のフィールド（`sky` / `ground` / `WorldProp.texture` /
  `uv_scale`）。検証は `expect_offscreen.pixels`（領域平均 / not_solid）。
  ゴールデン PNG ではない。HDR / bloom / FXAA / IBL / PCF / 水面 / LOD は
  共有ランタイム側
- **レンダラは二つ** — `kagra-core`（pip デモ）は `old/` にアーカイブ。
  `kagra-shared` と統合しない

## エンジンが今どこまでか

[docs/ROADMAP.ja.md](docs/ROADMAP.ja.md): **100% は画面を見ずにインディーを
出荷できること。** 今約 40%。M0–M2 は閉じた。80% とは言わない。絵を
three.js 級とも言わない。

## 索引

Rust の `GenreSession`（ルールは Rust、Python は殻）:

| ジャンル | 起動 |
|---|---|
| collectathon | `python -m kagra.play_world` |
| action | `python examples/action_minimal.py` |
| card | `python examples/card_minimal.py` |
| fight / versus | `python examples/versus_local.py` |
| fishcook | `python examples/fishcook_minimal.py` |
| fps | `python examples/fps_minimal.py` |
| novel | `python examples/novel_minimal.py` |
| platformer（Box Hop） | `python examples/platformer_minimal.py` |
| puzzle | `python examples/puzzle_minimal.py` |
| puzzle_pad | `python examples/puzzle_pad_minimal.py` |
| race | `python examples/race_minimal.py` |
| rhythm | `python examples/rhythm_minimal.py` |
| rpg | `python examples/rpg_minimal.py` |
| shop | `python -m kagra.play_world kagra-shared/tests/fixtures/shop_buy_world.json` |
| sports | `python examples/sports_minimal.py` |
| stealth | `python examples/stealth_minimal.py` |
| survival | `python examples/survival_minimal.py` |
| td | `python examples/td_minimal.py` |

Python ゲームマスター（ロジックは Python、自由な `WorldDoc`）:
`examples/bunny_garden_minimal.py`、`torneko_minimal.py`、
`bar_sim_minimal.py`、`flight_minimal.py`、`backyard_brawl_minimal.py`、
`chess_minimal.py`、`physics_minimal.py`、`python_game_minimal.py`。

フィクスチャだけのもの
（`python -m kagra.play_world kagra-shared/tests/fixtures/<id>_world.json`）:
`action_side`、`sprite_card`、`sim_meter`、`fish_cast`、`rhythm_beat`、
`cook_stove`、`survival_meter`、`box_hop`。

| 仕組み | 場所 |
|---|---|
| 物理 | `examples/physics_minimal.py`、`kagra/rigid.py` — ホイールを `physics` feature でビルドしたときの Rapier `PhysicsWorld` |
| FPS | `examples/fps_minimal.py`（`FpsSession`: 視点、ヒットスキャン、弾） |
| カード | `examples/card_minimal.py`（`CardSession`） |
| 対戦 / netplay | [`docs/netplay.md`](docs/netplay.md) — `examples/versus_local.py`、`examples/versus_online.py`、`server/netplay_relay.py` |
| 配布 | [`docs/distribution.md`](docs/distribution.md) — 同じマニフェストから Windows zip と Android APK |
| 音 | `kagra/audio.py`（BGM フェード、効果音、グループ、ミュート、定位 `play_se`）、`kagra-shared/src/mixer.rs` |
| フロー | `kagra/flow.py`（`save_dir`）、`kagra-shared/src/flow.rs`（タイトル → プレイ → ポーズ → 結果、音量、ハイスコア） |

```bash
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
python -m kagra.verify examples/verify_scenarios/bar_sim_smoke.json
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest.png
```

Wasm / Android（Crest Isle カプセル。VRM ではない）:

```bash
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000
./scripts/build_android_native.sh && cd mobile/android && gradle :app:assembleDebug
```

## pip デモ（旧レンダラー）

PyPI の **0.1.4** は元祖「Python 数行で VRM が歌って踊る」デモ
（`kagra-core`、wgpu 0.19 / RendererV2）。ソース・examples・docs は
[`old/`](old/README.md) にあります。新しいゲームはそこから始めない。

```bash
python -m kagra                                  # 歌って踊る（アーカイブの 0.19）
python old/examples/vrm_orb_rush.py              # 旧参照ゲーム
```

## ソースから

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv && source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install maturin
maturin develop
python -m kagra.verify examples/verify_scenarios/blank_smoke.json
```

MCP（Cursor）: `.cursor/mcp.json` → `kagra_api_search` / `kagra_verify` /
`kagra_render`。

Windows はプロセスあたり EventLoop 一つ。GPU シーンはサブプロセス —
それが `kagra.verify` の仕事です。

## ライセンス

MIT — [LICENSE](LICENSE)。

pip デモが取得するサンプル VRM は Alicia Solid（ニコニ立体ちゃん）© Dwango。
[利用規約](https://3d.nicovideo.jp/alicia/rule.html) に従い、スクリーンショットを
出すときはクレジットしてください。

KAGRA の名は神岡重力波検出器から。堅く、正確で、遊ぶために作る。

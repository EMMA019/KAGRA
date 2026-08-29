# KAGRA

**AI エージェントが、画面を見ずにゲームを作る**エンジン。

[English README](README.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m kagra.play_world                    # Crest Isle collectathon — wgpu 30 の窓、WASD
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json  # ヘッドレスでループを閉じる
```

これが本線です。**shared wgpu 30 ランタイム**で、世界はデータ（`World.dump()` JSON）、
プレイは 1 本のループ（タイトル → プレイ → 結果）、AI エージェントは API を検索し、
シーンを書き、**画面を見ずにヘッドレスで検証**します。昔の `pip install kagra`
デモ（VRM が歌って踊る）は [pip デモ](#pip-デモ旧レンダラー-kagra-core) として
今も残っています。

## 本線

- **世界はデータ** — `World.dump()` / `WorldDoc` は安定した JSON スキーマ
  （`docs/schemas/world.json`）。`world.query` / `dump` / `load` でスクショなしに
  世界を読む。同じ JSON がデスクトップ窓・wasm・Android・iOS・オフスクリーン描画を
  駆動します。
- **プレイは 1 本のループ** — `WorldPlay` がタイトル → プレイ → 結果（WASD、
  拾う、終わる）を進めます。ジャンルコード（釣り・料理・RPG）はエンジンではなく
  ゲーム側に置きます。
- **接着 API** — `prop.interact`（調べる/話す/使う → on_use イベント）、
  `doc.timers`（待つ。0 で on_done イベント）、`doc.events`（出来事。emit →
  take で複数システムが読む）、`walker.anim` / `walker.expression`（状態 → アニメ /
  表情）。dump 自体がバスです。
- **絵** — HDR フレーム + 閾値ブルーム、FXAA、IBL、PCF 影、水面（Fresnel + IBL
  反射）、LOD / GPU インスタンス、ACES トーンマップ。完全 MToon（影 2 段階・リム・
  アウトライン・matcap/normal）、VRM 0/1 表情プリセット、SpringBone コリジョン、
  VRMC_node_constraint、firstPerson 注釈。
- **Mobile / Wasm** — 同じ共有ランタイムが wasm / Android / iOS にビルドされます
  （Crest Isle カプセル collectathon、運転デモ）。`kagra-core`（pip デモ）は別
  レンダラ。統合しません。

## AI エージェントにゲームを作らせる

KAGRA の開発ループは人間だけでなく AI コーディングエージェント用に設計されています:

- **[AGENTS.md](AGENTS.md)** — どのエージェントでも使える行動規範
- **API 索引** — [`docs/API_INDEX.md`](docs/API_INDEX.md) は AST から生成。エージェントは推測ではなく検索する
- **エージェントの目** — `kagra.annotate`（クリックを数値に）と `kagra.debug_trace`（足と地形の JSONL）
- **ヘッドレス検証** — `python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json`（世界アサーション + shared wgpu 30 オフスクリーン煙）
- **MCP サーバー** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` / `kagra_resolve_asset` / `kagra_verify` / `kagra_render`
- **ビルドログ** — [`docs/agent-runs/`](docs/agent-runs/README.md): エージェント製ゲーム（Heart Catch、Switch Room、Dodge Room）+ エンジンスライス（接着 API、HDR+ブルーム、FXAA、完全 MToon、表情、VRM 残り）

## エンジンが今どこまでか

[docs/ROADMAP.ja.md](docs/ROADMAP.ja.md): **100% は画面を見ずにインディーを出荷できること。**
今約 40% — M0–M2 閉じた、collectathon が最初の M3 ジャンル、接着 API 4本と絵の
土台が載った。旧「63%」はアーカイブ。80% とはまだ言わない。

## pip デモ（旧エンジン — `old/` にアーカイブ）

元祖「Python 数行で VRM が歌って踊る」デモ（0.1.4、PyPI）は**旧エンジン**
（`kagra-core`、wgpu 0.19 / RendererV2）で動きます。**これは過去のもの**:
ソース・examples・docs は [`old/`](old/README.md) に移し、新本線
（shared wgpu 30）と混ざらないようにしました。`import kagra` は引き続き
動きます（コンパイル済み拡張は `kagra/` に残置）。旧デモは `old/` から:

```bash
# 旧エンジン（アーカイブ）。新しいゲームはここから始めない。
python -m kagra                                  # 歌って踊る（0.19 pip デモ）
python old/examples/vrm_orb_rush.py              # 参照ゲーム（RendererV2）
python old/examples/vrm_open_world.py            # 残置 VRM Crest Isle（RendererV2）
cd old/kagra-core && maturin develop --release   # 旧拡張の再ビルド（pip デモ用）
```

## サンプル

リポジトリを clone してください。まず本線:

```bash
python -m kagra.play_world                # 公式 Crest play: タイトル→プレイ→結果（カプセル、WASD）
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json  # VRoid Emma 歩き（wgpu 30）
python -m kagra.play_world kagra-shared/tests/fixtures/crest_emma_world.json  # VRM Crest Isle collectathon（タイトル→プレイ→結果、Emma）
python -m kagra.play_world kagra-shared/tests/fixtures/interact_fish_world.json  # 接着 API デモ（水辺で J → cast → 3秒 → bite）
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest.png  # オフスクリーン描画（bloom 付き）
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json        # ヘッドレス検証（世界 + オフスクリーン）
python -m kagra.verify examples/verify_scenarios/interact_fish_smoke.json       # 接着 API の検証
# Crest Isle モバイル（kagra-shared。VRM ではない。Kenney 風カプセル）
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000
# → http://localhost:8000/crest.html
./scripts/build_android_native.sh && cd mobile/android && gradle :app:assembleDebug
```

Python ゲームマスターのゲーム（ロジックは Python のみ）:

```bash
python examples/bunny_garden_minimal.py             # VRM 会話: 好感度・日程・セーブ（ESC / × でセーブ）
python examples/torneko_minimal.py --seed 12345     # ローグライク: seed ダンジョン・ターン・在庫
```

旧 pip デモの脚本: [`old/examples/`](old/examples/) — RendererV2 専用。
レガシー 2D / タイルマップ / エディタ: [`old/examples/archive/`](old/examples/archive/)。

## Python でゲームを作る（ゲームロジックはパイソンのみ）

0.19 の `kagra.run(start_scene)` の形を、shared wgpu 30 の上で復活させたもの:
**ゲームロジックは全部 Python** — Rust（`kagra_shared`）は世界の tick と描画だけ。
新しいジャンルを作るときはこの形をコピーする。

```bash
cd kagra-shared && maturin develop --release && cd ..   # 最初の一度: kagra_shared をビルド
python examples/python_game_minimal.py                  # 窓: WASD + 水辺で J
python examples/python_game_minimal.py --headless scratch/hello.png  # CI / verify: PNG 出力
python examples/bunny_garden_minimal.py                 # 1 本目ジャンル: Emma と会話・好感度・日程・セーブ
python examples/bunny_garden_minimal.py --headless scratch/bunny.png --days 3  # ヘッドレス verify
python examples/torneko_minimal.py --seed 12345         # ローグライク: seed 決定論ダンジョン・ターン・在庫・セーブ
python examples/torneko_minimal.py --headless scratch/torneko.png --turns 800  # 決定論 verify（同 seed → 同 PNG）
```

パターン（[`examples/python_game_minimal.py`](examples/python_game_minimal.py) より）:

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
        self._canvas_png = draw_world(self.world, self.width, self.height)  # shared 描画

run(MyGame())
```

Python 橋渡し（`kagra.WorldDoc` / `kagra.WorldPlay` / `kagra.render_world_doc`）は
`kagra_shared` からの再エクスポート。`kagra/gameloop.py` に `Scene` / `run` /
`draw_world` / `pressed` / `was_pressed`（tkinter、標準ライブラリのみ）。
ジャンルロジック（敵 AI・ターン・識別）は Python に書く — dump JSON が世界で、
イベントがバス。

## エージェント / ソースから

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv && source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install maturin
maturin develop
python -m kagra.verify examples/verify_scenarios/blank_smoke.json
```

MCP（Cursor）: `.cursor/mcp.json` → `kagra_api_search` / `kagra_verify` / `kagra_render`。

## ライセンス

MIT — [LICENSE](LICENSE)。

pip デモが取得するサンプル VRM は Alicia Solid（ニコニ立体ちゃん）© Dwango です。
[利用規約](https://3d.nicovideo.jp/alicia/rule.html) に従い、スクリーンショットを
出すときはクレジットしてください。

KAGRA の名は神岡重力波検出器から。堅く、正確で、遊ぶために作る。

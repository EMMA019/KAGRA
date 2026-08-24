# KAGRA

Python で AI に体を与えるゲームエンジン。

Cursor / Claude が API を検索し、シーンを書き、**画面を見ずに検証する**。体は VRM。歩く、持つ、話す。歌う・踊るは 2 コマンドの煙テストであり、製品そのものではない。

[English README](README.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
pip install kagra
python -m kagra
```

`pip install kagra` は **0.1.4**。`python -m kagra` で GPU と VRM が生きていることを確認します（初回だけ Alicia Solid）。ESC で終了。自分のモデルは `python -m kagra --vrm me.vrm --song my.wav` — [レシピ](docs/recipes/own-vrm.md)。

Windows の cmd で `'-m' は認識されていません` と出るときは、プロンプトの `>` のあとにさらに `>` を付けています。`py -3 -m kagra` か `kagra.cmd` を使ってください。

## AI エージェントにゲームを作らせる

ここが本丸です。エージェント（人間でも）はシグネチャを検索し、シーンを書き、ヘッドレスで閉じます。

1. **[AGENTS.md](AGENTS.md)** — Claude Code / Cursor / Windsurf … の行動規範。Cursor は `.cursor/skills/` から同じ規則を拾う
2. **API 索引** — [`docs/API_INDEX.md`](docs/API_INDEX.md) は AST から生成。名前は推測せず検索する
3. **ヘッドレス検証** — `python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json`
4. **MCP** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` / `kagra_resolve_asset` / `kagra_verify` / `kagra_render`

この一行を貼る:

```
Using KAGRA, make a short 3D game where a VRM walks a room of boxes
and steps on a floor switch. Camera follows. Public APIs only.
Verify with python -m kagra.verify.
```

ログ付きの成果は [`docs/agent-runs/`](docs/agent-runs/README.md) の Heart Catch、Switch Room、Dodge Room。Dodge Room は**別のエージェント**が `AGENTS.md` と一行から書いた。`examples/vrm_orb_rush.py` は参照ゲーム（公開 API のみ。生成ログは無い）。箱部屋の 4 本目を D-6 と呼ばない。D-6 は 30 秒遊べて、スコアかゴールがあるもの。

レシピ: [docs/recipes/agent-game.md](docs/recipes/agent-game.md)。

## 短い 3D ゲームを書く

`Prop` / `Walk` / `room` / `World3D`。WASD（または左スティック）で歩く。脚本はリポジトリを clone。`import kagra` は `pip` で足ります。

```python
import kagra
from kagra.camera3d import Camera3D

class Game(kagra.Scene):
    def on_enter(self):
        self.world = kagra.World3D(half=6.0)
        self.world.add_player(0, 3)
        kagra.room(world=self.world)
        kagra.Prop("box", x=2, y=0.5, z=0, color="orange", world=self.world)
        kagra.Prop.bake_all()
        self.cam = Camera3D()
        kagra.set_camera3d(self.cam)
        self.walk = kagra.Walk(self.world, self.cam)
        self.av = kagra.avatar(str(kagra.ensure_vrm()))

    def update(self, dt):
        self.walk.update(dt)
        p = self.world.player
        self.av.set_position(p.x, p.y, p.z)
        self.av.set_yaw(self.walk.yaw)
        self.av.update(dt)

    def draw(self):
        kagra.cls(12, 10, 18)
        self.world.draw()
        kagra.Prop.draw_all()
        kagra.draw_vrm(self.av.vrm_id)

kagra.init()
kagra.run(start_scene=Game())
```

## 頭脳を刺す

モデルは wheel に入れない。HTTP 面は 0.1.4 にある。

```python
mind = kagra.brain("kairi")          # https://kairi.onrender.com — KAIRI_API_TOKEN が要る
# mind = kagra.brain("ollama")
reply = mind.ask("こんにちは。一文で自己紹介して。")
```

デモ: `python examples/vrm_kairi_chat.py`。レシピ: [docs/recipes/ai-brain.md](docs/recipes/ai-brain.md)。

## 体はまだ歌う

2 コマンドのデモは、歌って踊る VRM（リップシンク、SpringBone、Mixamo `.fbx`、[`.vrma`](https://vrm.dev/vrma/)）。インストールが生きている証明であり、エンジンの用途ではない。

```python
av = kagra.avatar(str(kagra.ensure_vrm()))
av.dance(); av.sing()
```

自分のクリップは `av.dance("ymca.fbx")` か `av.dance("wave.vrma")`。会場は `kagra.stage("venue.glb")`。レシピ: [自分の VRM](docs/recipes/own-vrm.md) · [モーション](docs/recipes/motion.md)。

| | KAGRA | Ursina | Unity + UniVRM | three-vrm |
|---|---|---|---|---|
| インストール | `pip install kagra`（Windows / Linux、Rust 不要） | `pip` + Panda3D | Unity エディタ + パッケージ | `npm` + WebGL/WebGPU |
| 体 | VRM が wheel に入る | 汎用モデル | UniVRM | JavaScript + アセット |
| 短い 3D | `Prop` / `Walk` / `room` | `Entity` | シーン + C# | JavaScript |
| エージェントループ | API 索引 + `kagra.verify` + MCP | なし | なし | なし |
| ライセンス | MIT | MIT | Unity + UniVRM の各ライセンス | MIT |

事実だけ。貶さない。短い 3D の書き味は Ursina、VRM 実装は UniVRM と three-vrm。Unity のエディタとは戦わない。

## インストール

**Python 3.10 以降。** ホイールに Rust レンダラが入っているので、Rust のインストールは不要です。

```bash
python -m venv .venv
# Windows: .venv\Scripts\activate
# macOS / Linux:
source .venv/bin/activate

pip install kagra
python -m kagra
```

このホイールが製品です。VRM、3D の遊び場（`Prop` / `Walk` / `World3D`）、局所ライト、室内・屋外の影、法線マップ、AABB の箱、EventLoop 上の USB/XInput、`kagra.brain`、エージェントループ。顔トラ・仮想カメラ・マイクだけ extra。LLM モデルは wheel に入れません。

リポジトリのフォルダ（中に `kagra\` がある場所）で `python -m kagra` すると、pip の版ではなくその場のソースが優先されます。逃げ道は `cd %TEMP%` / `maturin develop`。`No module named kagra.__main__` と出たら、別のディレクトリから実行してください。

```powershell
cd $env:TEMP
python -m kagra
```

| | |
|---|---|
| Windows / Linux | `pip install kagra` |
| macOS | ソースビルド（`maturin develop`）。CI で wheel 検証中、公開は通ってから |
| Web カメラ顔トラ | `pip install "kagra[facetrack]"`（MediaPipe + OpenCV） |
| 仮想カメラ（OBS） | `pip install "kagra[stream]"` のあと `python -m kagra --loop --stream` |
| マイク口パク | `pip install "kagra[mic]"` |
| コントリビュータ / エージェント | `pip install maturin && maturin develop` |

## 入っているもの

- **エージェントループ** — API 索引、`kagra.verify`、MCP、golden、ログ付きの実行
- **3D の遊び場** — `Prop` / `Walk` / `sky` / `room` / `World3D`。局所ライト 4 本（`slot=0..3`）、室内ウンブラ、屋外 2 段シャドウ、接空間法線。AABB の箱は落ちる・積む・`Walk` が乗る。USB/XInput は EventLoop が `gilrs` で読む（テストは `inject_pad`）
- **VRM の体** — GPU スキニング、SpringBone、MToon、視線、リップシンク、IK、表情
- **頭脳** — `kagra.brain("kairi"|"ollama"|"openai")`。ホスト kairi は `KAIRI_API_TOKEN`。モデルは wheel に入れない
- **Mobile / Wasm** — `kagra-shared` と `mobile/` は**別製品の運転デモ**（道路・トラック・OSM）。Python の VRM / ゲームスタックではない。レンダラは統合しない

タイルマップ・ECS・2D エディタは棚（[`examples/archive/`](examples/archive/)）。見出しではない。

エンジンが今どこまでか（30 秒見本はまだ）は [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md)。three.js 級とはまだ言わない。第一想起は「Python で AI に体を与えるなら KAGRA」。

## まだ無いもの

嘘をつかないリスト。忘れたのではなく、今は入れない／まだバーではない。

- **macOS ホイール** — 検証できる Mac ができるまでソースビルド
- **他人に見せる 30 秒見本** — Pretty Room / Overworld / Prop Garden の API は 0.1.4 に入った。録画がバーになるまではまだ
- **実機パッドの 30 秒** — USB/XInput のポーリングは wheel にある。CI は `inject_pad`。手元のパッドは主張しない
- **YouTube / Twitch の公式取り込み** — `{user,text}` の JSONL を自分で書く（`ChatInbox`）
- **NDI / RTMP** — 窓キャプチャは今も使える。仮想カメラは extra
- **無人配信のセーフティ / オートパイロット** — 入れてない
- **VOICEVOX / Irodori-TTS** — 同梱しない。VOICEVOX は [docs/recipes/voicevox.md](docs/recipes/voicevox.md)
- **ポインタロック** — 一人称で要求する。OS が拒否することがある
- 曲 WAV と `.vrma` はホイールに入れない。サンプル VRM は初回ダウンロード

レシピ: [エージェントゲーム](docs/recipes/agent-game.md) · [頭脳 / kairi](docs/recipes/ai-brain.md) · [自分の VRM](docs/recipes/own-vrm.md) · [ダンス / VRMA](docs/recipes/motion.md) · [VOICEVOX](docs/recipes/voicevox.md) · [OBS / 配信](docs/recipes/stream.md) · [マスコット](docs/recipes/mascot.md)。
レビュー: [docs/REVIEW.ja.md](docs/REVIEW.ja.md)。ロードマップ: [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md)。

リリース手順は [docs/PUBLISHING.md](docs/PUBLISHING.md)。

## 安定コア

README と `python -m kagra` が使う名前です。メジャーバージョンを上げるまで壊しません。

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

他の API は [`docs/API_INDEX.md`](docs/API_INDEX.md) を見てください。まだ動く可能性があります。エージェントは Front の名前（`Prop`、`Walk`、`World3D`、`brain`）を優先すること。

## サンプル

脚本はリポジトリを clone してください。`import kagra` は `pip install kagra` で足ります。

```bash
python -m kagra.verify examples/verify_scenarios/blank_smoke.json
python examples/vrm_orb_rush.py          # 参照ゲーム
python examples/vrm_heart_catch.py       # 3 レーンキャッチ（エージェントログあり）
python examples/vrm_switch_room.py       # 箱部屋、カメラ追従（エージェントログあり）
python examples/vrm_dodge_room.py        # 降ってくる箱を避ける（エージェントログあり）
python examples/vrm_prop_garden.py       # Prop / Walk / sky
python examples/vrm_pretty_room.py       # 閉じた部屋 / スポット / IBL
python examples/vrm_overworld.py         # 島
python examples/vrm_kairi_chat.py        # kairi.onrender.com の頭脳（KAIRI_API_TOKEN）
python -m kagra                          # 歌って踊る煙テスト
python -m kagra --loop --stream          # HUD + 仮想カメラ（kagra[stream]）
```

レガシー 2D / タイルマップ / エディタは [`examples/archive/`](examples/archive/)。

## ソースから

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

デモが取得するサンプル VRM は Alicia Solid（ニコニ立体ちゃん）© Dwango です。[利用規約](https://3d.nicovideo.jp/alicia/rule.html) に従い、スクリーンショットを出すときはクレジットしてください。

KAGRA の名は神岡重力波検出器から。堅く、正確で、遊ぶために作る。

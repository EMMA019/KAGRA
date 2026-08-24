# KAGRA

Python 数行で、VRM が歌って踊る。

[English README](README.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
pip install kagra
python -m kagra
python -m kagra --vrm me.vrm --song my.wav
```

これだけです。初回だけサンプル VRM（Alicia Solid）をダウンロードし、その場で合成した歌とリップシンク、同梱のダンスを再生します。ESC で終了。自分のモデルは 3 行目 — [レシピ](docs/recipes/own-vrm.md)。

Windows の cmd で `'-m' は認識されていません` と出るときは、プロンプトの `>` のあとにさらに `>` を付けています。`py -3 -m kagra` か `kagra.cmd` を使ってください。

| | KAGRA | Unity + UniVRM | VSeeFace | three-vrm |
|---|---|---|---|---|
| インストール | `pip install kagra`（約 5MB、Rust 不要） | Unity + UniVRM パッケージ | アプリを落とす | `npm` + WebGL/WebGPU |
| 歌って踊るまで | コマンド 2 行、または Python 約 15 行 | シーン + C# + Animator | GUI（コードなし） | JavaScript + アセット |
| ライセンス | MIT | Unity + UniVRM の各ライセンス | プロプライエタリ | MIT |
| AI 連携 | Python。TTS は wheel の外。`kagra.brain` は 0.1.4（モデルは同梱しない） | エディタプラグイン | 限定的 | JavaScript |

事実だけ。貶さない。UniVRM と three-vrm は VRM 実装のものさし、VSeeFace は実際に開かれるトラッカー。

```python
import kagra
from kagra.camera3d import Camera3D

kagra.init()
cam = Camera3D(); cam.use_showcase()
av = None

def ready():
    global av
    kagra.apply_live_look()
    av = kagra.avatar(str(kagra.ensure_vrm()))
    av.dance(); av.sing()

def update(dt):
    av.update(dt)
    cam.update(kagra.get_engine(), dt)

def draw():
    kagra.cls(8, 6, 18)
    kagra.draw_vrm(av.vrm_id)
    kagra.draw_vignette()

kagra.run(update, draw, on_ready=ready)
```

自分のモデルは `kagra.avatar("/path/to/me.vrm")` または `assets/Emma.vrm`。自分の曲は `av.sing("song.wav")`。Mixamo の `.fbx` は `av.dance("ymca.fbx")` か `python -m kagra --dance ymca.fbx`。[VRM Animation](https://vrm.dev/vrma/)（`.vrma`）も同じ 1 行。どの VRM にも載ります。[text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) で作ったファイルも、指・表情・LookAt ごと再生できます。会場も同じで、`kagra.stage("venue.glb")`（または `--stage` / PNG の `--backdrop`）に Sketchfab のホールを落とすだけです。

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

`pip install kagra` は **0.1.4** で、それが製品です。レンダラ、VRM、歌う、踊る、`.vrma`、リップシンク、視線、IK、表情、SpringBone に加え、3D の遊び場（`Prop` / `Walk` / `World3D`）、局所ライト、室内・屋外の影、法線マップ、AABB の箱、EventLoop 上の USB/XInput、`kagra.brain` が入ります。Rust は不要です。顔トラ・仮想カメラ・マイクだけ extra。LLM モデルは wheel に入れません。

リポジトリのフォルダ（中に `kagra\` がある場所）で `python -m kagra` すると、pip の版ではなくその場のソースが優先されます。`No module named kagra.__main__` と出たら、別のディレクトリから実行してください。

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
| コントリビュータ | `pip install maturin && maturin develop` |

シーン脚本（`examples/vrm_*.py`）は git リポジトリにあります。`pip` が渡すのは `import kagra` です。

## 入っているもの

- **VRM** — GPU スキニング、SpringBone、MToon、視線、リップシンク、IK、表情
- **3D の遊び場** — `Prop` / `Walk` / `sky` / `room` / `World3D`。局所ライト 4 本（`slot=0..3`）、室内ウンブラ、屋外 2 段シャドウ、接空間法線。AABB の箱は落ちる・積む・`Walk` が乗る。USB/XInput は EventLoop が `gilrs` で読む（テストは `inject_pad`）
- **頭脳** — `kagra.brain("kairi"|"ollama"|"openai")`。ホスト kairi は `KAIRI_API_TOKEN`。モデルは wheel に入れない
- **エージェントループ** — API 索引、`kagra.verify`、MCP、golden
- **Mobile / Wasm** — `kagra-shared` と `mobile/` は **Python `kagra-core` とは別レンダラ**。運転デモ（Corridor Haul）に加え、Crest Isle の収集スライス（Kenney 風カプセル。**VRM ではない**）を Android debug APK / wasm で遊べる。レンダラは統合しない

タイルマップ・ECS・2D エディタは棚（[`examples/archive/`](examples/archive/)）。3D の見出しではない。

エンジンが今どこまでで、何がまだ開いているか（30 秒見本）は [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md)。three.js 級とはまだ言わない。

## AI エージェントにゲームを作らせる

KAGRA の開発ループは人間だけでなく AI コーディングエージェント用に設計されています。エージェントは API を検索し、シーンを書き、**画面を見ずにヘッドレスで検証**できます。

- **[AGENTS.md](AGENTS.md)** — どのエージェントでも使える行動規範（Claude Code / Cursor / Windsurf …）。Cursor は `.cursor/skills/` から同じ規則を自動で拾う
- **API 索引** — [`docs/API_INDEX.md`](docs/API_INDEX.md) は AST から生成。エージェントは推測ではなく検索する
- **ヘッドレス検証** — `python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json` で目視なしにループを閉じる
- **MCP サーバー** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` / `kagra_resolve_asset` / `kagra_verify` / `kagra_render`

`examples/vrm_orb_rush.py` が参照ゲーム（公開 API のみ。生成ログは無い）。ログ付きのエージェント製は [`docs/agent-runs/`](docs/agent-runs/README.md) の Heart Catch、Switch Room、Dodge Room（`examples/vrm_dodge_room.py` — 降ってくる箱を避ける）。Dodge Room は別のエージェントが `AGENTS.md` と一行プロンプトから書いた。

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
- 曲 WAV と `.vrma` はホイールに入れない（約 5MB の売りを守る）。サンプル VRM は初回ダウンロード

レシピ: [自分の VRM](docs/recipes/own-vrm.md) · [ダンス / VRMA](docs/recipes/motion.md) · [VOICEVOX](docs/recipes/voicevox.md) · [OBS / 配信](docs/recipes/stream.md) · [マスコット](docs/recipes/mascot.md) · [頭脳 / kairi](docs/recipes/ai-brain.md) · [エージェントゲーム](docs/recipes/agent-game.md)。
レビュー: [docs/REVIEW.ja.md](docs/REVIEW.ja.md)。ロードマップ: [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md)（最終目標は「Python で AI に体を与えるなら KAGRA」という第一想起）。

リリース手順は [docs/PUBLISHING.md](docs/PUBLISHING.md)。

## 安定コア

README と `python -m kagra` が使う名前です。メジャーバージョンを上げるまで壊しません。

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

他の API は [`docs/API_INDEX.md`](docs/API_INDEX.md) を見てください。まだ動く可能性があります。

## サンプル

脚本はリポジトリを clone してください。`import kagra` は `pip install kagra` で足ります。

```bash
python -m kagra                          # 歌って踊る
python -m kagra --loop --stream          # HUD + 仮想カメラ（kagra[stream]）
python examples/vrm_orb_rush.py          # 参照ゲーム
python examples/vrm_heart_catch.py       # 3 レーンキャッチ（エージェントログあり）
python examples/vrm_switch_room.py       # 箱部屋、カメラ追従
python examples/vrm_dodge_room.py        # 降ってくる箱を避ける（エージェントログあり）
python examples/vrm_relic_run.py          # 島の遺跡集め 30 秒（エージェントログあり）
python examples/vrm_open_world.py         # Crest Isle — 広い草原・海・山の収集（デスクトップ VRM）
# Crest Isle モバイル（kagra-shared。VRM ではない。Kenney 風カプセル）
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000
# → http://localhost:8000/crest.html
./scripts/build_android_native.sh && cd mobile/android && gradle :app:assembleDebug
python examples/vrm_prop_garden.py       # Prop / Walk / sky
python examples/vrm_pretty_room.py       # 閉じた部屋 / スポット / IBL
python examples/vrm_overworld.py         # 島 — 街 JSON、メッシュ坂、箱
python examples/vrm_kairi_chat.py        # kairi.onrender.com で会話（KAIRI_API_TOKEN）
python examples/vrm_vrma.py              # .vrma（または生成した波）
python examples/vrm_stream.py            # OBS / JSONL チャット
```

レガシー 2D / タイルマップ / エディタは [`examples/archive/`](examples/archive/)。

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

デモが取得するサンプル VRM は Alicia Solid（ニコニ立体ちゃん）© Dwango です。[利用規約](https://3d.nicovideo.jp/alicia/rule.html) に従い、スクリーンショットを出すときはクレジットしてください。

KAGRA の名は神岡重力波検出器から。堅く、正確で、遊ぶために作る。

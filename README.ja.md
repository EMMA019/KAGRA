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
| AI 連携 | Python（TTS / LLM は wheel の外） | エディタプラグイン | 限定的 | JavaScript |

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

自分のモデルは `kagra.avatar("/path/to/me.vrm")` または `assets/Emma.vrm`。自分の曲は `av.sing("song.wav")`。[VRM Animation](https://vrm.dev/vrma/)（`.vrma`）は `av.dance("wave.vrma")` にそのまま渡せます。どの VRM にも載ります。[text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) で作ったファイルも、指・表情・LookAt ごと再生できます。会場も同じで、`kagra.stage("venue.glb")`（または `--stage` / PNG の `--backdrop`）に Sketchfab のホールを落とすだけです。

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

リポジトリのフォルダ（中に `kagra\` がある場所）で `python -m kagra` すると、pip の版ではなくその場のソースが優先されます。`No module named kagra.__main__` と出たら、別のディレクトリから実行してください。

```powershell
cd $env:TEMP
python -m kagra
```

`pip install kagra` が製品そのものです。レンダラ、VRM、歌う、踊る、`.vrma`、リップシンク、視線、IK、表情、SpringBone は全部入ります。Rust は不要です。顔トラ・仮想カメラ・マイクだけ extra。

| | |
|---|---|
| Windows / Linux | `pip install kagra` |
| macOS | ソースビルド（`maturin develop`）。CI で wheel 検証中、公開は通ってから |
| Web カメラ顔トラ | `pip install "kagra[facetrack]"`（MediaPipe + OpenCV） |
| 仮想カメラ（OBS） | `pip install "kagra[stream]"` のあと `python -m kagra --loop --stream` |
| マイク口パク | `pip install "kagra[mic]"` |
| コントリビュータ | `pip install maturin && maturin develop` |

## AI エージェントにゲームを作らせる

KAGRA の開発ループは人間だけでなく AI コーディングエージェント用に設計されています。エージェントは API を検索し、シーンを書き、**画面を見ずにヘッドレスで検証**できます。

- **[AGENTS.md](AGENTS.md)** — どのエージェントでも使える行動規範（Claude Code / Cursor / Windsurf …）。Cursor は `.cursor/skills/` から同じ規則を自動で拾う
- **API 索引** — [`docs/API_INDEX.md`](docs/API_INDEX.md) は AST から生成。エージェントは推測ではなく検索する
- **ヘッドレス検証** — `python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json` で目視なしにループを閉じる
- **MCP サーバー** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` / `kagra_resolve_asset` / `kagra_verify` / `kagra_render`

`examples/vrm_orb_rush.py` が参照ゲーム（公開 API のみ）。一行プロンプトから作った実証は `examples/vrm_heart_catch.py`（ログ: [`docs/agent-runs/20260823-heart-catch/`](docs/agent-runs/20260823-heart-catch/)）。

## まだ無いもの

嘘をつかないリスト。忘れたのではなく、今は入れない。

- **macOS ホイール** — 検証できる Mac ができるまでソースビルド
- **ゲームパッド入力** — 今はキーボード / マウス / タッチのみ
- **YouTube / Twitch の公式取り込み** — `{user,text}` の JSONL を自分で書く（`ChatInbox`）
- **NDI / RTMP** — 窓キャプチャは今も使える。仮想カメラは extra
- **無人配信のセーフティ / オートパイロット** — 0.1.3 には無い
- **VOICEVOX / Irodori-TTS** — 同梱しない。VOICEVOX は [docs/recipes/voicevox.md](docs/recipes/voicevox.md)
- 曲 WAV と `.vrma` はホイールに入れない（約 5MB の売りを守る）。サンプル VRM は初回ダウンロード

レシピ: [自分の VRM](docs/recipes/own-vrm.md) · [ダンス / VRMA](docs/recipes/motion.md) · [VOICEVOX](docs/recipes/voicevox.md) · [OBS / 配信](docs/recipes/stream.md) · [マスコット](docs/recipes/mascot.md) · [エージェントゲーム](docs/recipes/agent-game.md)。

リリース手順は [docs/PUBLISHING.md](docs/PUBLISHING.md)。

## 安定コア

README と `python -m kagra` が使う名前です。メジャーバージョンを上げるまで壊しません。

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

他の API は [`docs/API_INDEX.md`](docs/API_INDEX.md) を見てください。まだ動く可能性があります。

## ライセンス

MIT — [LICENSE](LICENSE)。

デモが取得するサンプル VRM は Alicia Solid（ニコニ立体ちゃん）© Dwango です。[利用規約](https://3d.nicovideo.jp/alicia/rule.html) に従い、スクリーンショットを出すときはクレジットしてください。

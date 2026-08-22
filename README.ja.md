# KAGRA

Python 数行で、VRM が歌って踊る。

[English README](README.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
pip install kagra
python -m kagra
```

これだけです。初回だけサンプル VRM（Alicia Solid）をダウンロードし、その場で合成した歌とリップシンク、同梱のダンスを再生します。ESC で終了。

```python
import kagra
from kagra.camera3d import Camera3D

kagra.init()
cam = Camera3D(); cam.use_orbit(radius=2.6, target=(0, 0.9, 0))
av = None

def ready():
    global av
    av = kagra.avatar(str(kagra.ensure_vrm()))
    av.dance(); av.sing()

def update(dt):
    av.update(dt)
    cam.orbit_by(dt * 0.25, 0)
    cam.update(kagra.get_engine())

def draw():
    kagra.cls(16, 12, 32)
    kagra.draw_vrm(av.vrm_id)

kagra.run(update, draw, on_ready=ready)
```

自分のモデルは `kagra.avatar("/path/to/me.vrm")` または `assets/Emma.vrm`。自分の曲は `av.sing("song.wav")`。[VRM Animation](https://vrm.dev/vrma/)（`.vrma`）は `av.dance("wave.vrma")` にそのまま渡せます。どの VRM にも載ります。[text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) で作ったファイルも、指・表情・LookAt ごと再生できます。

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

`pip install kagra` が製品そのものです。レンダラ、VRM、歌う、踊る、`.vrma`、リップシンク、視線、IK、表情、SpringBone は全部入ります。Rust は不要です。追加パッケージも不要です。

| | |
|---|---|
| Windows / Linux | `pip install kagra` |
| macOS | ホイール検証できるまでソースビルド（`maturin develop`） |
| Web カメラ顔トラ | `pip install "kagra[facetrack]"`（MediaPipe + OpenCV が入る） |
| コントリビュータ | `pip install maturin && maturin develop` |

リリース手順は [docs/PUBLISHING.md](docs/PUBLISHING.md)。

## 安定コア

README と `python -m kagra` が使う名前です。メジャーバージョンを上げるまで壊しません。

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

他の API は [`docs/API_INDEX.md`](docs/API_INDEX.md) を見てください。まだ動く可能性があります。

## ライセンス

MIT — [LICENSE](LICENSE)。

デモが取得するサンプル VRM は Alicia Solid（ニコニ立体ちゃん）© Dwango です。[利用規約](https://3d.nicovideo.jp/alicia/rule.html) に従い、スクリーンショットを出すときはクレジットしてください。

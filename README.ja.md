# KAGRA

Python 数行で、VRM が歌って踊る。

[English README](README.md)

<img width="1919" height="1029" alt="KAGRA editor" src="https://github.com/user-attachments/assets/e8c94080-0465-498e-aca9-d80e71165308" />
<img width="1276" height="744" alt="KAGRA scene" src="https://github.com/user-attachments/assets/4d9f3564-b926-492a-abb8-5000581cc1ed" />

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
av = kagra.avatar(str(kagra.ensure_vrm()))
av.dance(); av.sing()

def update(dt):
    av.update(dt)
    cam.orbit_by(dt * 0.25, 0)
    cam.update(kagra.get_engine())

def draw():
    kagra.cls(16, 12, 32)
    kagra.draw_vrm(av.vrm_id)

kagra.run(update, draw)
```

自分のモデルは `kagra.avatar("/path/to/me.vrm")` または `assets/Emma.vrm`。自分の曲は `av.sing("song.wav")`。[VRM Animation](https://vrm.dev/vrma/)（`.vrma`）は `av.dance("wave.vrma")` にそのまま渡せます。どの VRM にも載ります。

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

ホイールは Windows と Linux のみ（Mac は検証できるまでソースビルド）。PyPI にまだ上がっていない場合は clone して `maturin develop`（Rust が必要）。手順は [docs/PUBLISHING.md](docs/PUBLISHING.md)。

## 安定コア

README と `python -m kagra` が使う名前です。メジャーバージョンを上げるまで壊しません。

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

他の API は [`docs/API_INDEX.md`](docs/API_INDEX.md) を見てください。まだ動く可能性があります。

## ライセンス

MIT — [LICENSE](LICENSE)。

デモが取得するサンプル VRM は Alicia Solid（ニコニ立体ちゃん）© Dwango です。[利用規約](https://3d.nicovideo.jp/alicia/rule.html) に従い、スクリーンショットを出すときはクレジットしてください。

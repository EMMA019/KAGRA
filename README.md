# KAGRA

Python 数行で、VRM が歌って踊る。

[日本語 README](README.ja.md)

https://github.com/user-attachments/assets/39eaa077-498b-43d0-8a23-96adf043ee77

```bash
pip install kagra
python -m kagra
```

That's it. The first run downloads a sample VRM (Alicia Solid, once) and plays a synthesized song with lipsync and a bundled dance. ESC to quit.

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

Use your own model with `kagra.avatar("/path/to/me.vrm")` or `assets/Emma.vrm`. Use your own song with `av.sing("song.wav")`. Drop a [VRM Animation](https://vrm.dev/en/vrma/) (`.vrma`) on `av.dance("wave.vrma")` — same clip, any VRM. Clips from [text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) work as-is (fingers + expressions + LookAt).

## Install

**Python 3.10+.** Wheels include the Rust renderer — you do **not** need a Rust toolchain.

```bash
python -m venv .venv
# Windows: .venv\Scripts\activate
# macOS / Linux:
source .venv/bin/activate

pip install kagra
python -m kagra
```

If you run `python -m kagra` from a checkout that contains a `kagra/` folder, Python imports that folder instead of the installed wheel. `No module named kagra.__main__` means you hit an older local package — run from another directory:

```powershell
cd $env:TEMP
python -m kagra
```

`pip install kagra` is the full product: renderer, VRM, sing, dance, `.vrma`, lipsync, look-at, IK, expressions, SpringBone. No Rust toolchain. No second package.

| | |
|---|---|
| Windows / Linux | `pip install kagra` |
| macOS | from source (`maturin develop`) until wheels are verified |
| Webcam face tracking | `pip install "kagra[facetrack]"` (pulls MediaPipe + OpenCV) |
| Contributors | `pip install maturin && maturin develop` |

See [docs/PUBLISHING.md](docs/PUBLISHING.md) to cut a release.

## Stable core

These names are what the README and `python -m kagra` rely on. We will not break them without a major version:

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

Everything else in [`docs/API_INDEX.md`](docs/API_INDEX.md) is available but may still move.

## What you get

- **VRM** — GPU skinning, SpringBone, MToon, look-at, lipsync, IK, expressions
- **2D / 3D** — tilemaps, ECS, simple physics, orbit camera, fog, shadows
- **Agent loop** — API index, `kagra.verify`, MCP tools, golden renders
- **Mobile / Wasm** — experimental `kagra-shared` runtime (see `mobile/README.md`). Python games stay on desktop for now.

## Samples

```bash
python -m kagra                          # sing & dance
python examples/vrm_vrma.py              # .vrma (or a generated wave)
python examples/2Daction.py              # no assets needed
python examples/3Dmaze.py                # drop a .vrm in assets/ to see it
python examples/vrm_orb_rush.py
```

## Agent / from source

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv && source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install maturin
maturin develop
python -m kagra.verify examples/verify_scenarios/blank_smoke.json
```

MCP (Cursor): `.cursor/mcp.json` → `kagra_api_search` / `kagra_verify` / `kagra_render`.

## License

MIT — [LICENSE](LICENSE).

Sample VRM downloaded by the demo is Alicia Solid (ニコニ立体ちゃん) © Dwango, used under [their terms](https://3d.nicovideo.jp/alicia/rule.html). Credit the character if you post screenshots.

KAGRA is named after the Kamioka Gravitational Wave Detector. Solid, precise, and built for fun.

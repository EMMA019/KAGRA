# KAGRA

Python 数行で、VRM が歌って踊る。

[日本語 README](README.ja.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
pip install kagra
python -m kagra
python -m kagra --vrm me.vrm --song my.wav
```

That's it. The first run downloads a sample VRM (Alicia Solid, once) and plays a synthesized song with lipsync and a bundled dance. ESC to quit. Your own model is the third line — [recipe](docs/recipes/own-vrm.md).

On Windows cmd, `'-m' is not recognized` means an extra `>` was typed. Use `py -3 -m kagra` or `kagra.cmd`.

| | KAGRA | Unity + UniVRM | VSeeFace | three-vrm |
|---|---|---|---|---|
| Install | `pip install kagra` (~5MB wheel, no Rust) | Unity editor + UniVRM package | download the app | `npm` + a WebGL/WebGPU page |
| Code to sing & dance | 2 commands, or ~15 lines of Python | scene + C# + Animator | GUI, no code | JavaScript + assets |
| License | MIT | Unity + UniVRM licenses | proprietary app | MIT |
| AI hook | Python (TTS / LLM stay outside the wheel) | editor plugins | limited | JavaScript |

Facts only. UniVRM and three-vrm are the VRM implementations we measure against; VSeeFace is the desktop tracker people actually open.

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

Use your own model with `kagra.avatar("/path/to/me.vrm")` or `assets/Emma.vrm`. Use your own song with `av.sing("song.wav")`. Drop Mixamo `.fbx` on `av.dance("ymca.fbx")` or `python -m kagra --dance ymca.fbx`. A [VRM Animation](https://vrm.dev/en/vrma/) (`.vrma`) is the same one-liner — same clip, any VRM. Clips from [text-to-vrma](https://github.com/Kirakun0328/text-to-vrma) work as-is (fingers + expressions + LookAt). Drop a Sketchfab hall the same way: `kagra.stage("venue.glb")` (or `--stage` / a PNG `--backdrop`).

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

If you run `python -m kagra` from a checkout that contains a `kagra/` folder, Python imports that folder instead of the installed wheel. The command now prints the escape hatch (`cd %TEMP%` / `maturin develop`). `No module named kagra.__main__` is the older local package — run from another directory:

```powershell
cd $env:TEMP
python -m kagra
```

`pip install kagra` is the full product: renderer, VRM, sing, dance, `.vrma`, lipsync, look-at, IK, expressions, SpringBone. No Rust toolchain. Face tracking, virtual camera, and mic are extras.

| | |
|---|---|
| Windows / Linux | `pip install kagra` |
| macOS | from source (`maturin develop`) until CI macos wheels are published |
| Webcam face tracking | `pip install "kagra[facetrack]"` (MediaPipe + OpenCV) |
| Virtual camera (OBS) | `pip install "kagra[stream]"` then `python -m kagra --loop --stream` |
| Mic lipsync | `pip install "kagra[mic]"` |
| Contributors | `pip install maturin && maturin develop` |

## Let your AI agent build the game

KAGRA's development loop is designed for AI coding agents, not just humans. An agent can search the API, write a scene, and **verify it headlessly** — no human looking at the screen:

- **[AGENTS.md](AGENTS.md)** — rules for any agent (Claude Code, Cursor, Windsurf, ...). Cursor picks up the same rules via `.cursor/skills/`
- **API index** — [`docs/API_INDEX.md`](docs/API_INDEX.md) is generated from the AST, so agents search instead of guessing signatures
- **Headless verify** — `python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json` closes the loop in CI or a subprocess
- **MCP server** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` / `kagra_resolve_asset` / `kagra_verify` / `kagra_render`

`examples/vrm_orb_rush.py` is the reference game (public APIs only; no generation log). Logged agent-built games live in [`docs/agent-runs/`](docs/agent-runs/README.md): Heart Catch, Switch Room, and Dodge Room (`examples/vrm_dodge_room.py` — survive falling boxes). Dodge Room was written by a second agent from `AGENTS.md` + a one-line prompt.

## Not yet

Honesty list. These are missing on purpose, not forgotten:

- **macOS wheels** — build from source until a Mac can verify them
- **OS gamepad devices** — `axis` / `pad` / `inject_pad` work; USB/XInput poll is not in the wheel yet
- **YouTube / Twitch chat APIs** — write `{user,text}` JSONL yourself (`ChatInbox`)
- **NDI / RTMP** — OBS window capture still works; virtual cam is the extra
- **Autopilot / unattended safety** — not in 0.1.3
- **Official LLM brain hook** — `AiCharacter` exists; `KairiBrain` / Ollama one-liner is not shipped
- **Full PMREM / several lights / CSM / normal maps** — one directional shadow, one point *or* spot (no light shadow), HDRI + a small irradiance cube. `set_hdri` / `set_point_light` / `set_spot_light` are in
- **Pointer lock / `Prop.animate` / click-to-use** — `Walk(first_person=True)` and `hovered_prop` are in; the mouse still hits the window edge
- **VOICEVOX / Irodori-TTS** — not bundled. VOICEVOX recipe: [docs/recipes/voicevox.md](docs/recipes/voicevox.md)
- Song WAV and `.vrma` stay out of the wheel (~5MB install). First run downloads the sample VRM
- Play surface (`Prop` / `Walk` / `room` / tiled island) is on this branch. PyPI 0.1.3 does not ship it yet

Recipes: [own VRM](docs/recipes/own-vrm.md) · [dance / VRMA](docs/recipes/motion.md) · [VOICEVOX](docs/recipes/voicevox.md) · [OBS / stream](docs/recipes/stream.md) · [mascot](docs/recipes/mascot.md) · [agent game](docs/recipes/agent-game.md). Review: [docs/REVIEW.ja.md](docs/REVIEW.ja.md). Roadmap: [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md).

See [docs/PUBLISHING.md](docs/PUBLISHING.md) to cut a release.

## Stable core

These names are what the README and `python -m kagra` rely on. We will not break them without a major version:

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

Everything else in [`docs/API_INDEX.md`](docs/API_INDEX.md) is available but may still move.

## What you get

- **VRM** — GPU skinning, SpringBone, MToon, look-at, lipsync, IK, expressions
- **3D play** — `Prop` / `Walk` / `sky` / `room` / tiled heightfield island, orbit / follow / first-person, fog, one-light shadows, HDRI
- **Agent loop** — API index, `kagra.verify`, MCP tools, golden renders
- **Mobile / Wasm** — `kagra-shared` + `mobile/` is a **separate driving demo** (roads, truck, OSM). It is not the Python VRM / game stack. Do not merge the two renderers.

## Samples

```bash
python -m kagra                          # sing & dance
python -m kagra --loop --stream          # HUD + virtual cam (needs kagra[stream])
python examples/vrm_orb_rush.py          # reference game
python examples/vrm_heart_catch.py       # 3-lane catch (agent-run log)
python examples/vrm_switch_room.py       # boxed room, camera follow
python examples/vrm_dodge_room.py        # falling boxes, survive (agent-run log)
python examples/vrm_prop_garden.py       # Prop / Walk / sky (play surface)
python examples/vrm_pretty_room.py       # enclosed room / spot / IBL
python examples/vrm_overworld.py         # tiled island — slopes, stairs, box blocks
python examples/vrm_vrma.py              # .vrma (or a generated wave)
python examples/vrm_stream.py            # OBS / JSONL chat
```

Legacy 2D / tilemap / editor demos: [`examples/archive/`](examples/archive/).

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

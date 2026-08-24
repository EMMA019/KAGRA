# KAGRA

A Python game engine for giving an AI a body.

Cursor / Claude search the API, write a scene, and **verify it without looking at the screen**. The body is a VRM. It walks, carries, talks. Singing and dancing is the two-command smoke test — not the product.

[日本語 README](README.ja.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
pip install kagra
python -m kagra
```

`pip install kagra` is **0.1.4**. `python -m kagra` proves the GPU and the VRM (sample Alicia Solid, once). ESC to quit. Your own model: `python -m kagra --vrm me.vrm --song my.wav` — [recipe](docs/recipes/own-vrm.md).

On Windows cmd, `'-m' is not recognized` means an extra `>` was typed. Use `py -3 -m kagra` or `kagra.cmd`.

## Let an agent build the game

This loop is the point. An agent (or a human) searches signatures, writes a scene, and closes it headlessly:

1. **[AGENTS.md](AGENTS.md)** — rules for Claude Code, Cursor, Windsurf, … Cursor loads the same rules from `.cursor/skills/`
2. **API index** — [`docs/API_INDEX.md`](docs/API_INDEX.md) is generated from the AST. Search; do not invent names
3. **Headless verify** — `python -m kagra.verify examples/verify_scenarios/orb_rush_smoke.json`
4. **MCP** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` / `kagra_resolve_asset` / `kagra_verify` / `kagra_render`

Paste this:

```
Using KAGRA, make a short 3D game where a VRM walks a room of boxes
and steps on a floor switch. Camera follows. Public APIs only.
Verify with python -m kagra.verify.
```

Logged results live in [`docs/agent-runs/`](docs/agent-runs/README.md): Heart Catch, Switch Room, Dodge Room. Dodge Room was written by a **second** agent from `AGENTS.md` + one line. `examples/vrm_orb_rush.py` is the reference game (public APIs only; no generation log). Do not call a fourth box room D-6 — that waits on a 30-second playable demo with a score or a goal.

Recipe: [docs/recipes/agent-game.md](docs/recipes/agent-game.md).

## Write a short 3D game

`Prop` / `Walk` / `room` / `World3D`. WASD (or the left stick) to walk. Clone the repo for the full scripts; `pip` is enough for `import kagra`.

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

## Give it a brain

Models stay out of the wheel. HTTP is in 0.1.4.

```python
mind = kagra.brain("kairi")          # https://kairi.onrender.com — needs KAIRI_API_TOKEN
# mind = kagra.brain("ollama")
reply = mind.ask("こんにちは。一文で自己紹介して。")
```

Demo: `python examples/vrm_kairi_chat.py`. Recipe: [docs/recipes/ai-brain.md](docs/recipes/ai-brain.md).

## The body still sings

The two-command demo is a VRM that sings and dances (lipsync, SpringBone, Mixamo `.fbx`, [`.vrma`](https://vrm.dev/en/vrma/)). That is how you know the install worked — not what the engine is for.

```python
av = kagra.avatar(str(kagra.ensure_vrm()))
av.dance(); av.sing()
```

Own clip: `av.dance("ymca.fbx")` or `av.dance("wave.vrma")`. Venue: `kagra.stage("venue.glb")`. Recipes: [own VRM](docs/recipes/own-vrm.md) · [motion](docs/recipes/motion.md).

| | KAGRA | Ursina | Unity + UniVRM | three-vrm |
|---|---|---|---|---|
| Install | `pip install kagra` (Windows / Linux wheel, no Rust) | `pip` + Panda3D | Unity editor + package | `npm` + a WebGL/WebGPU page |
| Body | VRM in the wheel | generic models | UniVRM | JavaScript + assets |
| Short 3D | `Prop` / `Walk` / `room` | `Entity` | scene + C# | JavaScript |
| Agent loop | API index + `kagra.verify` + MCP | none | none | none |
| License | MIT | MIT | Unity + UniVRM licenses | MIT |

Facts only. Ursina is the writing we measure 3D play against; UniVRM and three-vrm are the VRM implementations; we do not fight Unity’s editor.

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

That wheel is the product: VRM, 3D play (`Prop` / `Walk` / `World3D`), local lights, indoor/outdoor shadows, normal maps, AABB crates, USB/XInput on the EventLoop, `kagra.brain`, and the agent loop. Face tracking, virtual camera, and mic are extras. LLM models are not in the wheel.

If you run `python -m kagra` from a checkout that contains a `kagra/` folder, Python imports that folder instead of the installed wheel. Escape hatch: `cd %TEMP%` / `maturin develop`. `No module named kagra.__main__` is the older local package — run from another directory:

```powershell
cd $env:TEMP
python -m kagra
```

| | |
|---|---|
| Windows / Linux | `pip install kagra` |
| macOS | from source (`maturin develop`) until CI macos wheels are published |
| Webcam face tracking | `pip install "kagra[facetrack]"` (MediaPipe + OpenCV) |
| Virtual camera (OBS) | `pip install "kagra[stream]"` then `python -m kagra --loop --stream` |
| Mic lipsync | `pip install "kagra[mic]"` |
| Contributors / agents | `pip install maturin && maturin develop` |

## What you get

- **Agent loop** — API index, `kagra.verify`, MCP, golden renders, logged runs
- **3D play** — `Prop` / `Walk` / `sky` / `room` / `World3D`. Four local lights (`slot=0..3`), indoor umbra, 2-cascade outdoor shadows, tangent-space normals. AABB crates fall, stack, and `Walk` stands on them. USB/XInput on the EventLoop (`gilrs`); tests use `inject_pad`
- **VRM body** — GPU skinning, SpringBone, MToon, look-at, lipsync, IK, expressions
- **Brain** — `kagra.brain("kairi"|"ollama"|"openai")`. Hosted kairi needs `KAIRI_API_TOKEN`. Models are not in the wheel
- **Mobile / Wasm** — `kagra-shared` + `mobile/` is a **separate driving demo** (roads, truck, OSM). It is not the Python VRM / game stack. Do not merge the two renderers

Tilemaps, ECS, and the 2D editor are on the shelf ([`examples/archive/`](examples/archive/)). They are not the headline.

Where the engine sits (30-second demos still open): [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md). Do not call this three.js-class yet. First-recall stays “if you give an AI a body in Python, it’s KAGRA”.

## Not yet

Honesty list. Missing on purpose, or not the bar yet:

- **macOS wheels** — build from source until a Mac can verify them
- **30-second stranger demos** — Pretty Room / Overworld / Prop Garden APIs are in 0.1.4; the recordings are not yet the bar
- **Real-hardware gamepad 30s** — USB/XInput poll is in the wheel; CI uses `inject_pad`. We do not claim a pad in your hand
- **YouTube / Twitch chat APIs** — write `{user,text}` JSONL yourself (`ChatInbox`)
- **NDI / RTMP** — OBS window capture still works; virtual cam is the extra
- **Autopilot / unattended safety** — not shipped
- **VOICEVOX / Irodori-TTS** — not bundled. VOICEVOX recipe: [docs/recipes/voicevox.md](docs/recipes/voicevox.md)
- **Pointer lock** — requested for first-person; the OS may refuse
- Song WAV and `.vrma` stay out of the wheel. First run downloads the sample VRM

Recipes: [agent game](docs/recipes/agent-game.md) · [brain / kairi](docs/recipes/ai-brain.md) · [own VRM](docs/recipes/own-vrm.md) · [dance / VRMA](docs/recipes/motion.md) · [VOICEVOX](docs/recipes/voicevox.md) · [OBS / stream](docs/recipes/stream.md) · [mascot](docs/recipes/mascot.md). Review: [docs/REVIEW.ja.md](docs/REVIEW.ja.md). Roadmap: [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md).

See [docs/PUBLISHING.md](docs/PUBLISHING.md) to cut a release.

## Stable core

These names are what the README and `python -m kagra` rely on. We will not break them without a major version:

`init` · `run` · `quit` · `Scene` · `avatar` · `ensure_vrm` · `draw_vrm` · `cls` · `font` · `text` · `fill` · `key` · `pressed` · `Camera3D`

Everything else in [`docs/API_INDEX.md`](docs/API_INDEX.md) is available but may still move. Agents should prefer Front names there (`Prop`, `Walk`, `World3D`, `brain`).

## Samples

Clone the repo for these scripts. `pip install kagra` is enough for `import kagra`.

```bash
python -m kagra.verify examples/verify_scenarios/blank_smoke.json
python examples/vrm_orb_rush.py          # reference game
python examples/vrm_heart_catch.py       # 3-lane catch (agent-run log)
python examples/vrm_switch_room.py       # boxed room, camera follow (agent-run log)
python examples/vrm_dodge_room.py        # falling boxes, survive (agent-run log)
python examples/vrm_prop_garden.py       # Prop / Walk / sky
python examples/vrm_pretty_room.py       # enclosed room / spot / IBL
python examples/vrm_overworld.py         # tiled island
python examples/vrm_kairi_chat.py        # brain via kairi.onrender.com (KAIRI_API_TOKEN)
python -m kagra                          # sing & dance smoke
python -m kagra --loop --stream          # HUD + virtual cam (needs kagra[stream])
```

Legacy 2D / tilemap / editor demos: [`examples/archive/`](examples/archive/).

## From source

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

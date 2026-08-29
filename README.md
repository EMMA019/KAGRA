# KAGRA

An engine where an **AI agent builds a game — headless, no human watching the screen**.

[日本語 README](README.ja.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m kagra.play_world                    # Crest Isle collectathon — wgpu 30 window, WASD
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json  # headless close-the-loop
```

That's the mainline: a **shared wgpu 30 runtime** where the world is data
(`World.dump()` JSON), play is one loop (title → play → result), and an AI
agent searches the API, writes a scene, and verifies it headlessly. The
old `pip install kagra` demo (VRM singing and dancing) still exists — see
[The pip demo](#the-pip-demo-old-renderer-kagra-core).

## The mainline

- **World is data** — `World.dump()` / `WorldDoc` is a stable JSON schema
  (`docs/schemas/world.json`). `world.query` / `dump` / `load` read the world
  without a screenshot. The same JSON drives the desktop window, wasm, Android,
  iOS, and offscreen rendering.
- **Play is one loop** — `WorldPlay` ticks title → play → result (WASD,
  pickups, finish) on `python -m kagra.play_world`. Genre code (fishing,
  cooking, RPG) lives in the game, not the engine.
- **Adhesive API** — `prop.interact` (examine / talk / use → on_use event),
  `doc.timers` (wait; emits on_done at 0), `doc.events` (happenings;
  emit → take for many systems), `walker.anim` / `walker.expression`
  (state → animation / expression). The dump is the bus — no callback soup.
- **Picture** — HDR frame + threshold bloom, FXAA, IBL, PCF shadows, water
  (Fresnel + IBL reflection), LOD / GPU instancing, ACES tonemap. Full MToon
  (2-step shade, rim, outline, matcap/normal), VRM 0/1 expression presets,
  SpringBone with colliders, VRMC_node_constraint, firstPerson annotations.
- **Mobile / Wasm** — the same shared runtime builds to wasm / Android / iOS
  (Crest Isle capsule collectathon; driving demo). `kagra-core` (the pip demo)
  is a separate renderer — do not merge the two.

## Let your AI agent build the game

KAGRA's development loop is designed for AI coding agents, not just humans:

- **[AGENTS.md](AGENTS.md)** — rules for any agent (Claude Code, Cursor, Windsurf, ...)
- **API index** — [`docs/API_INDEX.md`](docs/API_INDEX.md), generated from the AST, so agents search instead of guessing signatures
- **Agent eyes** — `kagra.annotate` (click → numbers) and `kagra.debug_trace` (foot vs terrain JSONL)
- **Headless verify** — `python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json` (world assertions + shared wgpu 30 offscreen smoke)
- **MCP server** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` / `kagra_resolve_asset` / `kagra_verify` / `kagra_render`
- **Build logs** — [`docs/agent-runs/`](docs/agent-runs/README.md): agent-built games (Heart Catch, Switch Room, Dodge Room) and engine slices (adhesive API, HDR + bloom, FXAA, full MToon, expressions, VRM rest)

## Where the engine sits

[docs/ROADMAP.ja.md](docs/ROADMAP.ja.md): **100% = an agent ships a normal indie game with no human looking at the screen.** Now ~40% — M0–M2 closed, collectathon is the first M3 genre, the adhesive API and the picture base are in. Old "63%" is archived. Do not call this 80% yet.

## The pip demo (old engine — archived under `old/`)

The original "Python 数行で VRM が歌って踊る" demo (0.1.4, PyPI) runs on the
**old engine** (`kagra-core`, wgpu 0.19 / RendererV2). It is **past history**:
source, examples and docs moved to [`old/`](old/README.md) so it never mixes
with the new shared wgpu 30 mainline. `import kagra` still works (the compiled
extension stays in `kagra/`), and the old demos run from `old/`:

```bash
# old engine (archived). New games must NOT start here.
python -m kagra                                  # sing & dance (0.19 pip demo)
python old/examples/vrm_orb_rush.py              # reference game (RendererV2)
python old/examples/vrm_open_world.py            # leftover VRM Crest Isle (RendererV2)
cd old/kagra-core && maturin develop --release   # rebuild the old extension (pip demo)
```

## Samples

Clone the repo for these. The mainline first:

```bash
python -m kagra.play_world                # official Crest play: title → play → result (capsule, WASD)
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json  # VRoid Emma walk (wgpu 30)
python -m kagra.play_world kagra-shared/tests/fixtures/crest_emma_world.json  # VRM Crest Isle collectathon (title→play→result, Emma)
python -m kagra.play_world kagra-shared/tests/fixtures/interact_fish_world.json  # adhesive-API demo (J at the shore → cast → 3s → bite)
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest.png  # offscreen render (bloom on)
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json        # headless verify (world + offscreen)
python -m kagra.verify examples/verify_scenarios/interact_fish_smoke.json       # adhesive-API verify
# Crest Isle mobile (kagra-shared; not VRM — Kenney-style capsule)
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000
# → http://localhost:8000/crest.html
./scripts/build_android_native.sh && cd mobile/android && gradle :app:assembleDebug
```

Python game-master games (game logic is Python-only):

```bash
python examples/bunny_garden_minimal.py             # VRM talk: affection, days, save (ESC / × saves)
python examples/torneko_minimal.py --seed 12345     # roguelike: seeded dungeon, turns, inventory
```

Old pip demo scripts: [`old/examples/`](old/examples/) — RendererV2 only.
Legacy 2D / tilemap / editor demos: [`old/examples/archive/`](old/examples/archive/).

## Build a game in Python (game logic is Python-only)

0.19's `kagra.run(start_scene)` shape, revived on the shared wgpu 30 runtime:
**the game logic is all Python** — Rust (`kagra_shared`) only ticks the world
and renders. That is the shape an agent should copy for a new genre.

```bash
cd kagra-shared && maturin develop --release && cd ..   # once: build kagra_shared
python examples/python_game_minimal.py                  # window: WASD + J at the shore
python examples/python_game_minimal.py --headless scratch/hello.png  # CI / verify: PNG out
python examples/bunny_garden_minimal.py                 # first genre game: talk to Emma (VRM), affection, days, save
python examples/bunny_garden_minimal.py --headless scratch/bunny.png --days 3  # headless verify
python examples/torneko_minimal.py --seed 12345         # roguelike: seeded dungeon, turns, inventory, save
python examples/torneko_minimal.py --headless scratch/torneko.png --turns 800  # deterministic verify (same seed → same PNG)
```

The pattern (from [`examples/python_game_minimal.py`](examples/python_game_minimal.py)):

```python
import json
import kagra
from kagra.gameloop import Scene, run, draw_world, pressed, was_pressed

class MyGame(Scene):
    def __init__(self):
        super().__init__()
        self.play = kagra.WorldPlay.from_json(open("world.json").read())
        self.play.confirm()                    # title → play (no-op when playing)
        self.world = json.loads(self.play.dump())

    def update(self, dt):                      # ← all game logic lives here
        lx = (1.0 if pressed("d") else 0.0) - (1.0 if pressed("a") else 0.0)
        lz = (1.0 if pressed("w") else 0.0) - (1.0 if pressed("s") else 0.0)
        self.play.set_input(lx, lz, False, was_pressed("j"), False)
        self.play.tick(dt)                     # engine steps the world
        self.world = json.loads(self.play.dump())
        if self.play.take_events("cast"):      # adhesive events → your logic
            self.play.start_timer("cast", 3.0, "bite")

    def draw(self):
        self._canvas_png = draw_world(self.world, self.width, self.height)  # shared render

run(MyGame())
```

The Python bridge (`kagra.WorldDoc` / `kagra.WorldPlay` / `kagra.render_world_doc`)
re-exports from `kagra_shared`; see `kagra/gameloop.py` for `Scene` / `run` /
`draw_world` / `pressed` / `was_pressed` (tkinter, stdlib only). Genre logic
(enemy AI, turns, item identification) stays in Python — the dump JSON is the
world, the events are the bus.

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

Sample VRM downloaded by the pip demo is Alicia Solid (ニコニ立体ちゃん) © Dwango,
used under [their terms](https://3d.nicovideo.jp/alicia/rule.html). Credit the
character if you post screenshots.

KAGRA is named after the Kamioka Gravitational Wave Detector. Solid, precise,
and built for fun.

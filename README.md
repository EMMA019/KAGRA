# KAGRA

**If you give an AI a body in Python, it's KAGRA.**

An AI-agent game engine: search the API, write a scene, verify it
**without a human looking at the screen**. The body is a VRM when you
want one. The world is dump JSON. Play is one loop. Singing and dancing
is the old two-command smoke — not the product.

[日本語 README](README.ja.md)

https://github.com/user-attachments/assets/1a1af44d-d6cc-4ea4-a05d-6f8ad6c193c2

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m kagra.play_world                    # Crest Isle collectathon — wgpu 30 window, WASD
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json  # headless close-the-loop
```

That is the mainline: a **shared wgpu 30 runtime**. An agent (or a human)
searches signatures, writes Python game logic, and closes the loop with
`kagra.verify`. The old `pip install kagra` demo (VRM singing and dancing
on RendererV2) is archived — see [The pip demo](#the-pip-demo-old-renderer).

## Let an agent build the game

This loop is the engine. Rules live in [`AGENTS.md`](AGENTS.md). Cursor
loads the same rules from `.cursor/skills/`.

1. **API index** — [`docs/API_INDEX.md`](docs/API_INDEX.md) is generated
   from the AST. Search; do not invent names
2. **Assets** — `kagra.contracts.resolve_asset` (or MCP `kagra_resolve_asset`).
   Aliases: `Emma`, `walk`, `knight`, `inn`, `chest`, …
3. **Headless verify** — `python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json`
4. **MCP** — `tools/mcp_kagra/server.py`: `kagra_api_search` / `kagra_env` /
   `kagra_resolve_asset` / `kagra_verify` / `kagra_render`
5. **Eyes** — `kagra.annotate` (click → numbers) and `kagra.debug_trace`
   (foot vs terrain). Not a human editor
6. **Build logs** — [`docs/agent-runs/`](docs/agent-runs/README.md)

Paste this:

```
Using KAGRA on the shared wgpu 30 mainline (WorldDoc / WorldPlay /
kagra.gameloop), make a short 3D game. Public APIs only. Search
docs/API_INDEX.md first. Do not use RendererV2 or old/examples.
Verify with python -m kagra.verify. Log the run under docs/agent-runs/.
```

Recipe: [docs/recipes/agent-game.md](docs/recipes/agent-game.md).
Longer map: [`docs/AGENT.md`](docs/AGENT.md).

New games are Python. Rust ticks the world and draws. Do not add a genre
module to `WorldPlay`. Set `WorldDoc.genre` explicitly. Prop names are
scenery.

## Write a game in Python

Game logic is all Python. `kagra_shared` only ticks the world and renders.

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
        self._canvas_png = draw_world(self.world, self.width, self.height)

run(MyGame())
```

Copy [`examples/python_game_minimal.py`](examples/python_game_minimal.py)
or a Python game-master sample (bunny garden, torneko, bar sim, flight,
backyard CTF, chess, physics).

```bash
cd kagra-shared && maturin develop --release && cd ..   # once: build kagra_shared
python examples/python_game_minimal.py --headless scratch/hello.png
python examples/bar_sim_minimal.py --headless scratch/bar.png --days 3 --seed 1
```

`kagra.WorldDoc` / `kagra.WorldPlay` / `kagra.render_world_doc` re-export
from `kagra_shared`. `kagra.gameloop` is stdlib + tkinter (no torch).

## The mainline

- **World is data** — `World.dump()` / `WorldDoc` (`docs/schemas/world.json`).
  `world.query` / `dump` / `load` read the world without a screenshot. The
  same JSON drives the desktop window, wasm, Android, iOS, and offscreen
- **Play is one loop** — `WorldPlay` ticks title → play → result. Genre
  code lives in the game, not the engine
- **Adhesive API** — `prop.interact`, `doc.timers`, `doc.events`,
  `walker.anim` / `walker.expression`. The dump is the bus
- **Picture** — fields on the dump (`sky`, `ground`, `WorldProp.texture`,
  `uv_scale`). Verify with `expect_offscreen.pixels` (region mean /
  not_solid), not a golden PNG. HDR / bloom / FXAA / IBL / PCF / water /
  LOD live on the shared runtime
- **Two renderers** — `kagra-core` (the pip demo) is archived under
  `old/`. Do not merge it with `kagra-shared`

## Where the engine sits

[docs/ROADMAP.ja.md](docs/ROADMAP.ja.md): **100% = an agent ships a
normal indie game with no human looking at the screen.** Now ~40%.
M0–M2 closed. Do not call this 80%, and do not call the picture
three.js-class.

## Map

Rust `GenreSession` games (rules in Rust, Python is the shell):

| Genre | Run |
|---|---|
| collectathon | `python -m kagra.play_world` |
| action | `python examples/action_minimal.py` |
| card | `python examples/card_minimal.py` |
| fight / versus | `python examples/versus_local.py` |
| fishcook | `python examples/fishcook_minimal.py` |
| fps | `python examples/fps_minimal.py` |
| novel | `python examples/novel_minimal.py` |
| platformer (Box Hop) | `python examples/platformer_minimal.py` |
| puzzle | `python examples/puzzle_minimal.py` |
| puzzle_pad | `python examples/puzzle_pad_minimal.py` |
| race | `python examples/race_minimal.py` |
| rhythm | `python examples/rhythm_minimal.py` |
| rpg | `python examples/rpg_minimal.py` |
| shop | `python -m kagra.play_world kagra-shared/tests/fixtures/shop_buy_world.json` |
| sports | `python examples/sports_minimal.py` |
| stealth | `python examples/stealth_minimal.py` |
| survival | `python examples/survival_minimal.py` |
| td | `python examples/td_minimal.py` |

Python game master (logic stays in Python, free `WorldDoc`):
`examples/bunny_garden_minimal.py`, `torneko_minimal.py`,
`bar_sim_minimal.py`, `flight_minimal.py`, `backyard_brawl_minimal.py`,
`chess_minimal.py`, `physics_minimal.py`, `python_game_minimal.py`.

WorldPlay ids that are a fixture only
(`python -m kagra.play_world kagra-shared/tests/fixtures/<id>_world.json`):
`action_side`, `sprite_card`, `sim_meter`, `fish_cast`, `rhythm_beat`,
`cook_stove`, `survival_meter`, `box_hop`.

| System | Where |
|---|---|
| Physics | `examples/physics_minimal.py`, `kagra/rigid.py` — Rapier `PhysicsWorld` when the wheel is built with the `physics` feature |
| FPS | `examples/fps_minimal.py` (`FpsSession`: look, hitscan, ammo) |
| Card | `examples/card_minimal.py` (`CardSession`) |
| Versus / netplay | [`docs/netplay.md`](docs/netplay.md) — `examples/versus_local.py`, `examples/versus_online.py`, `server/netplay_relay.py` |
| Distribution | [`docs/distribution.md`](docs/distribution.md) — one manifest, Windows zip and Android APK |
| Audio | `kagra/audio.py` (BGM fade, effects, groups, mute, positional `play_se`), `kagra-shared/src/mixer.rs` |
| Flow | `kagra/flow.py` (`save_dir`), `kagra-shared/src/flow.rs` (title → play → pause → result, volume, high score) |

```bash
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
python -m kagra.verify examples/verify_scenarios/bar_sim_smoke.json
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest.png
```

Wasm / Android (Crest Isle capsule, not VRM):

```bash
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000
./scripts/build_android_native.sh && cd mobile/android && gradle :app:assembleDebug
```

## The pip demo (old renderer)

PyPI **0.1.4** is the original “Python 数行で VRM が歌って踊る” demo
(`kagra-core`, wgpu 0.19 / RendererV2). Source, examples, and docs live
under [`old/`](old/README.md). New games must not start there.

```bash
python -m kagra                                  # sing & dance (archived 0.19 path)
python old/examples/vrm_orb_rush.py              # old reference game
```

## From source

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv && source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install maturin
maturin develop
python -m kagra.verify examples/verify_scenarios/blank_smoke.json
```

MCP (Cursor): `.cursor/mcp.json` → `kagra_api_search` / `kagra_verify` /
`kagra_render`.

Windows: one EventLoop per process. GPU scenes run in a subprocess —
that is what `kagra.verify` does.

## License

MIT — [LICENSE](LICENSE).

Sample VRM downloaded by the pip demo is Alicia Solid (ニコニ立体ちゃん)
© Dwango, used under [their terms](https://3d.nicovideo.jp/alicia/rule.html).
Credit the character if you post screenshots.

KAGRA is named after the Kamioka Gravitational Wave Detector. Solid,
precise, and built for fun.

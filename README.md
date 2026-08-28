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

## The pip demo (old renderer, kagra-core)

```bash
pip install kagra
python -m kagra
python -m kagra --vrm me.vrm --song my.wav
```

The original "Python 数行で VRM が歌って踊る" demo — still on PyPI (0.1.4).
`kagra-core` (wgpu 0.19 / RendererV2) stays for leftover VRM demos; new games
start on the shared wgpu 30 mainline. Own model: `kagra.avatar("/path/to/me.vrm")`;
own song: `av.sing("song.wav")`; Mixamo `.fbx` / `.vrma` dance: `av.dance("ymca.fbx")`.

## Samples

Clone the repo for these. The mainline first:

```bash
python -m kagra.play_world                # official Crest play: title → play → result (capsule, WASD)
python -m kagra.play_world kagra-shared/tests/fixtures/emma_walker_world.json  # VRoid Emma walk (wgpu 30)
python -m kagra.play_world kagra-shared/tests/fixtures/interact_fish_world.json  # adhesive-API demo (J at the shore → cast → 3s → bite)
python -m kagra.render_world kagra-shared/tests/fixtures/crest_isle_world.json scratch/crest.png  # offscreen render (bloom on)
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json        # headless verify (world + offscreen)
python -m kagra.verify examples/verify_scenarios/interact_fish_smoke.json       # adhesive-API verify
# Crest Isle mobile (kagra-shared; not VRM — Kenney-style capsule)
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000
# → http://localhost:8000/crest.html
./scripts/build_android_native.sh && cd mobile/android && gradle :app:assembleDebug
```

The pip demo (`kagra-core` / RendererV2) scripts: `examples/vrm_*.py` —
`python -m kagra` (sing & dance), `examples/vrm_orb_rush.py` (reference game),
`vrm_heart_catch.py` / `vrm_switch_room.py` / `vrm_dodge_room.py` /
`vrm_relic_run.py` (agent-run logs). Legacy 2D / tilemap / editor demos:
[`examples/archive/`](examples/archive/).

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

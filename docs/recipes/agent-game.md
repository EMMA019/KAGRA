# Let an agent build a KAGRA game / エージェントにゲームを作らせる

English first. 日本語は後半。

KAGRA is a Python game engine for giving an AI a body — and for an AI
coding agent to build the game around it, headless.

The loop is: search the API → write a scene → verify without looking at
the screen. Rules live in [`AGENTS.md`](../../AGENTS.md). Logged runs live
in [`docs/agent-runs/`](../agent-runs/README.md).

New games start on the shared wgpu 30 mainline:

- world = `WorldDoc` dump JSON (`docs/schemas/world.json`)
- play = `WorldPlay` (title → play → result)
- game logic = Python (`kagra.gameloop`)
- picture fields live on the dump; verify with `expect_offscreen.pixels`

Do **not** start on RendererV2 / `old/examples`. Do **not** add a genre
module to `WorldPlay`. Set `WorldDoc.genre` explicitly. Prop names are
scenery.

Engine bar: [`docs/ROADMAP.ja.md`](../ROADMAP.ja.md). **100%** = an agent
ships a normal indie game with no human screen. **80%** is that minus
net / destruction / cloth / vehicles / GI bake / DOTS / HDRP / a human
editor / VRM-on-Wasm. **Now ~40%.** Do not copy old “63%”. Brain is
`kagra.brain("kairi")` when you need one. Rapier stays out of the wheel.

## One-line prompt (mainline)

```
Using KAGRA on the shared wgpu 30 mainline (WorldDoc / WorldPlay /
kagra.gameloop), make a short 3D game. Public APIs only. Search
docs/API_INDEX.md first. Do not use RendererV2 or old/examples.
Verify with python -m kagra.verify. Log the run under docs/agent-runs/.
```

Closed genre copies (game logic is Python-only):

| Prompt shape | Script | Log |
|---|---|---|
| Talk to a VRM, days, save | `examples/bunny_garden_minimal.py` | `docs/agent-runs/20260901-bunny-garden/` |
| Seeded dungeon, turns, inventory | `examples/torneko_minimal.py` | `docs/agent-runs/20260901-torneko/` |
| Bar: stock, guests, ledger | `examples/bar_sim_minimal.py` | `docs/agent-runs/20260912-bar-sim/` |
| Fly the island, rings | `examples/flight_minimal.py` | `docs/agent-runs/20261022-flight/` |
| 3v3 backyard CTF | `examples/backyard_brawl_minimal.py` | `docs/agent-runs/20261022-biplane-match/` |
| Studio 3D chess | `examples/chess_minimal.py` | `docs/agent-runs/20261022-chess-pbr-quality/` |

Day-one box rooms (Heart Catch / Switch Room / Dodge Room) proved the
loop. They live under `old/examples/` and are not a template for a
fourth box room.

## APIs agents actually need

Search [`docs/API_INDEX.md`](../API_INDEX.md) before using a name.

| Job | Call |
|---|---|
| World | `WorldDoc` / `WorldPlay.from_json` / `play.dump` / `play.tick` |
| Input | `kagra.gameloop.pressed` / `was_pressed` then `play.set_input` |
| Events | `play.take_events` / `play.start_timer` / `prop.interact` |
| Render | `draw_world` / `kagra.render_world` / `python -m kagra.play_world` |
| Verify | `python -m kagra.verify examples/verify_scenarios/<name>_smoke.json` |
| Assets | `kagra.contracts.resolve_asset` (`Emma`, `walk`, `knight`, `inn`, …) |
| Eyes | `kagra.annotate` / `kagra.debug_trace` / `world.query` (not an editor) |

Archived RendererV2 names (`texture_from_fn` / `Walk` / `Prop` /
`kagra.avatar` as the game loop) are **not** on `import kagra`.

## Close the loop

```bash
python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json
python -m kagra.verify examples/verify_scenarios/bar_sim_smoke.json
python -m kagra.verify examples/verify_scenarios/interact_fish_smoke.json
python examples/bar_sim_minimal.py --headless scratch/bar.png --days 3 --seed 1
python examples/bunny_garden_minimal.py --headless scratch/bunny.png --days 3
python examples/torneko_minimal.py --headless scratch/torneko.png --turns 800
```

Save the prompt, the stumbles, and the verify output under
`docs/agent-runs/YYYYMMDD-<slug>/`.

---

# 日本語

一行で渡す:

```
KAGRA の shared wgpu 30 本線（WorldDoc / WorldPlay / kagra.gameloop）で
短い 3D ゲームを作って。公開 API だけ。先に docs/API_INDEX.md を検索。
RendererV2 と old/examples は使わない。python -m kagra.verify で閉じて。
ログは docs/agent-runs/ に残して。
```

コピーするなら上のジャンル表。箱部屋の 4 本目を作らない。
100% は画面なしでインディーを出荷。今約 40%。80% とは言わない。
第一想起は「Python で AI に体を与えるなら KAGRA」。

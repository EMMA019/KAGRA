# KAGRA

An engine for **mass-producing small, original 3D browser games** with AI
agents. A game is a short Python file plus JSON; a shared Rust runtime
(compiled to wasm) plays it on a phone browser at 60 Hz. The agent writes the
game, checks it headless, and ships a preview link a human can tap on a phone.

[日本語 README](README.ja.md)

> Status (2026-10): four phone-playable games on master (HOP-BEAT, Neon Cat,
> Meteor Cat, Moonlit Swarm step 1), a shared game kit, a retention layer, a
> phone measurement tool and a license ledger. Portals are not submitted yet.
> The older "headless collectathon" README is kept as history under
> [`old/`](old/README.md) and in [docs/ROADMAP.ja.md](docs/ROADMAP.ja.md).

## Play

Every pull request publishes a GitHub Pages preview at
`https://emma019.github.io/KAGRA/pr/<n>/play.html` ([docs/preview.md](docs/preview.md)).
The root `play.html` on master is the stable Lumen build only, so to try the games
below open a PR preview and add the `?game=` value, for example
[`pr/209/play.html?game=survivor_arena`](https://emma019.github.io/KAGRA/pr/209/play.html?game=survivor_arena).
Open on a phone:

| Game | What it is | Query |
|---|---|---|
| HOP-BEAT | Beat-locked hopping runner (126 BPM, tap or hold hops, PERFECT / GOOD) | `?game=hop_beat` |
| Neon Cat | Three-lane night-street runner | `?game=neon_cat` |
| Meteor Cat | A re-skin of Neon Cat made from nine lines of Python and JSON | `?game=meteor_cat` |
| Moonlit Swarm | Survivor-like arena: auto-attack, XP gems, level-up cards, a daily run | `?game=survivor_arena` |

Add `&dev=1` for the on-phone measurement overlay and `&quality=safe|mid|high|max`
to force a graphics tier (draft PRs).

## What is in the engine

- **Game spec** — a manifest in `games/<name>.json` holds the world, the rules
  and tuning. Python authors it; Rust plays it, because wasm and Android have no
  Python. See [docs/runner_kit.md](docs/runner_kit.md).
- **Shared parts (`game_core`)** — lives, missions, result screen, juice and
  feel, collisions, a genre-neutral enemy schema ([docs/enemy_kit.md](docs/enemy_kit.md),
  walkers, flyers, jumpers, telegraphs), a pooled swarm with a spatial grid,
  and the meta layer. A new game adds one `KitHost` implementation and one
  line in `kit_host::KITS`.
- **Retention layer** — coin wallet, unlock shop, daily run with a seed and a
  streak, run modifiers, an optional rewarded-ad slot; no server, no network
  ([docs/meta_core.md](docs/meta_core.md)).
- **Quality tiers and measurement** — `?dev=1` shows fps, p50/p95/p99 frame
  time and jank; a 60 s recorder and a "find my limit" probe pick a tier
  ([docs/phone_measure.md](docs/phone_measure.md)). Tier definitions are JSON.
- **Assets and licenses** — CC0 or original art only in the public repo. Every
  asset is in a ledger ([docs/asset_ledger.md](docs/asset_ledger.md)) and CI
  checks it. Paid assets would stay out of the repo and enter at build time.
- **Publishing** — free, non-exclusive portals first
  ([docs/publishing/web_portals.md](docs/publishing/web_portals.md),
  [docs/PUBLISHING.md](docs/PUBLISHING.md)); per-game notes under
  `docs/publishing/`.

## Make a game

```bash
git clone https://github.com/EMMA019/KAGRA.git
cd KAGRA
python -m venv .venv && source .venv/bin/activate   # Windows: .venv\Scripts\activate
pip install maturin && maturin develop              # builds the Python bridge
python examples/meteor_cat_minimal.py               # a window; --headless out.png renders offline
./scripts/build_wasm.sh && python -m http.server -d kagra-shared/www 8000   # browser build
```

A Neon Cat-class game in nine lines (see [docs/runner_kit.md](docs/runner_kit.md)):

```python
from kagra.runner import play
raise SystemExit(play("games/meteor_cat_minimal.json"))
```

## For AI agents

- **[AGENTS.md](AGENTS.md)** — rules for any agent (Claude Code, Cursor, Codex, ...).
- **API index** — [`docs/API_INDEX.md`](docs/API_INDEX.md), generated from the AST, so agents search instead of guessing.
- **Close the loop** — `python -m kagra.verify examples/verify_scenarios/collectathon_smoke.json`, offscreen render, a playtest bot, and fairness checkers (seeded courses played back to back).
- **MCP server** — `tools/mcp_kagra/server.py` (`kagra_api_search`, `kagra_verify`, `kagra_render`).
- **Small PRs** — draft PR, CI green, a phone preview link; merge only after the owner's OK. `kagra-shared` CI takes about 30 minutes.
- **Build logs and design** — [`docs/agent-runs/`](docs/agent-runs/README.md), [`docs/design/survivor_arena.md`](docs/design/survivor_arena.md), [CHANGELOG.md](CHANGELOG.md).

## Other genres and the older engine

`examples/` still holds the earlier genre minimals (card, rhythm, race, fps, rpg,
td, platformer, puzzle, fighting, survival, and more) and Python game masters
(roguelike, bar sim, chess). They run on the same shared wgpu 30 runtime but are
not the current focus. The first demo (`pip install kagra`, a VRM that sings and
dances) runs on the old engine and lives under [`old/`](old/README.md); new games
must not start there. [`docs/AGENT.md`](docs/AGENT.md) has the longer map.

## License

MIT — [LICENSE](LICENSE). Third-party assets are listed in [CREDITS.md](CREDITS.md)
and [docs/asset_ledger.md](docs/asset_ledger.md); fonts are OFL and bundled with
their license text. The sample VRM downloaded by the old pip demo is Alicia Solid
(ニコニ立体ちゃん) © Dwango, under [their terms](https://3d.nicovideo.jp/alicia/rule.html).

KAGRA is named after the Kamioka Gravitational Wave Detector. Solid, precise,
and built for fun.

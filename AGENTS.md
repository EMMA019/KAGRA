# AGENTS.md

Instructions for AI coding agents working in this repository (Claude Code, Cursor, Codex,
aider, ...). Cursor users get the same rules via `.cursor/skills/kagra-agent/SKILL.md`.

## What KAGRA is for

**Mass-producing small, high-quality 3D browser games for phones, with AI, while growing the
engine from the games.** One shared Rust runtime (`kagra-shared`, wasm in the browser, also
Android / Windows) runs every game; a game is a **kit** (one Rust file implementing
`KitHost`) plus **JSON** (its world and its manifest). Every PR gets a phone preview link.
The owner plays it on a phone and says what feels wrong; agents build, verify and ship.

The shipped games today: HOP-BEAT (rhythm hop), Neon Cat (lane runner), Moonlit Swarm
(survivor arena), Lantern Watch (tap defense), Crate Tower (one-button stacker), Station Escape (side-on platformer), Moonlit Tactics (tactics, in progress). Starter (`kagra-shared/src/starter.rs`) is the template.

## Rules

1. **A new game starts from the template.** `python scripts/new_kit.py <slug> "<Title>"`,
   then change the parts marked `GAME:`. The walk-through, the parts to reuse and the ship
   checklist are in [`docs/making_a_game.md`](docs/making_a_game.md). Do not add a genre to
   `WorldPlay` or game branches to `play.html`.
2. **Reuse the shared parts before writing new ones** (`game_core::{stick, motion, view, platform, grid, tween, progress, floats, swarm,
   enemies, lives, missions, juice}`, `hud_kit` (cards, buttons), rounded `Quad`s, `TextFace::Display`). When a
   game grows a part another game could use, move it into `game_core` in the same PR series.
3. **The page reads the manifest, never the genre.** Page features (best score, audio,
   language, pause, fixed 60 Hz step, one-button keys, music sync, revive ad, direct play,
   URL switches) are the manifest's `page` block (`kagra/page_spec.py`).
   `tests/test_page_spec.py` keeps genre and slug branches out of `play.html`.
4. **Close the loop before saying "done".** Run the checks in the ship checklist: fmt,
   clippy (with and without `render`), `cargo test -p kagra-shared`, the related `pytest`,
   and the browser smoke (`node scripts/browser_smoke.mjs dist/preview <slug>`; CI runs it on
   every PR). Look at a screenshot of anything visual. A headless pass is not a phone pass:
   ask the owner to try the preview link.
5. **Assets: CC0 or original only, all in the ledger.** No Quaternius. Ask the owner before
   anything paid. Record every asset in `assets/LEDGER.json`, run
   `python3 tools/asset_ledger.py credits --write` and `... check` (0 errors). Fonts are OFL
   with the license text next to them; a new kanji needs `python tools/subset_fonts.py`.
6. **Do not invent APIs.** Read the code (`kagra-shared/src`), `docs/API_INDEX.md` for the
   Python side (`python tools/gen_api_index.py --check` stays clean).
7. **Keep `tests/` extension-free.** Load pure-logic modules via
   `tests/conftest.py::load_kagra_submodule`. Run `pytest tests -m "not golden"`.
8. **Format Rust before push.** `cargo fmt -p kagra-shared -- --check` (CI fails on drift).
   MSRV 1.88.
9. **Small PRs, the owner merges.** Draft or ready PRs with a preview link and a short
   Japanese report to the owner. Merge only with the owner's OK and green CI (squash). Never
   force-push; merge `master` into a branch to resolve. kagra-shared CI takes ~30 min.
10. **Repro a playtest issue before editing it.** 「直してここ」 files a `playtest` issue
    (`KAGRA-PLAYTEST-1` block). Run `python scripts/repro_issue.py <issue number or block>`
    first and keep the screenshot; attach before / after after the fix.
11. **Phones differ.** The preview page caps PowerVR GPUs (Pixel 10) at render scale 1.5 and
    steps the scale down on a blank frame; errors show on screen instead of a black page.
    Measure on a phone with `?dev=1` (fps, frame times, Find my limit).

## Coordinate system

Right-handed, Y up. The framebuffer is not mirrored: NDC +x is the right edge.

World +X is screen-right **only when the camera looks toward -Z** (the kits' cameras sit at
+Z looking toward the origin). There, screen up is -Z and screen down is +Z: a thumb stick's
screen vector `(x, y)` moves the hero by `(x, y)` in world `(x, z)`; keys and the gamepad
(`walk(lx, lz)`, `+lz` = up) become `(lx, -lz)`. A chase camera looking along +Z flips left
and right. Every world dump names the sides (`coordinates.note`, each camera's
`screen_right` / `screen_left`); read those instead of guessing from the sign of X.

2D HUD pixels start at the top-left: +x right, +y down. A `WorldProp`'s +pitch lifts its
nose (Euler YXZ), so a lean toward the facing direction is a negative pitch. glTF bind poses
face -Z; do not add π to every yaw to "fix" one model.

## Commands

```bash
python scripts/new_kit.py <slug> "<Title>"                 # a new game from the template
cargo fmt -p kagra-shared -- --check
cargo clippy -p kagra-shared --all-targets --locked -- -D warnings
cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings
cargo test -p kagra-shared --locked                        # + --features render (needs a GPU or lavapipe)
KAGRA_BAKE=1 cargo test -p kagra-shared --lib <slug>::      # re-bake a kit's world fixture
pytest tests -m "not golden"
bash scripts/build_wasm.sh                                 # kagra-shared/www/pkg
python scripts/build_browser_preview.py --out dist/preview --require-pkg
node scripts/browser_smoke.mjs dist/preview [slug ...]     # every game starts in Chromium
python3 tools/asset_ledger.py check
python tools/subset_fonts.py                               # after adding Japanese text
python scripts/repro_issue.py 123                          # 「直してここ」
```

Preview links: `https://emma019.github.io/KAGRA/pr/<n>/play.html?game=<slug>`
(`&dev=1` measurement overlay, `&quality=safe|mid|high|max`).

## Where things are

| What | Where |
|---|---|
| Kits (games) | `kagra-shared/src/{starter,lantern_watch,crate_tower,station_escape,moonlit_tactics,survivor_arena,hop_beat,neon_cat}.rs`, registry `kit_host.rs` |
| Shared game parts | `kagra-shared/src/game_core/`, `hud_kit.rs` |
| Renderer (wgpu 30; WebGPU, WebGL2, native) | `kagra-shared/src/render/` (2D: rounded quads, glyph atlas) |
| Fonts | `kagra-shared/src/font.rs` (Public Sans, Zen Maru Gothic, Cinzel; PixelMplus off wasm) |
| Browser page | `scripts/preview_site/` (`play.html`, `quality.js`, `meta.js`, `portal.js`, `perf.js`) |
| Manifests | `games/<slug>_minimal.json` (`page`, `platform`, `meta`, `bundle_assets`) |
| Design notes per game | `docs/design/`, `docs/runner_kit.md`, `docs/enemy_kit.md`, `docs/meta_core.md` |
| Publishing | `docs/PUBLISHING.md`, `docs/publishing/` |
| Agent build logs | `docs/agent-runs/` |
| Roadmap / review | `docs/ROADMAP.ja.md`, `docs/REVIEW.ja.md` |

## Legacy

The desktop Python engine (VRM avatars, `World` / `Prop` / `Walk`, RendererV2, the genre
`WorldPlay` sessions, the MCP server `tools/mcp_kagra/server.py`) still builds and its tests
still run, but it is not where new games go. Its old agent guide is
[`old/docs-archive/AGENTS-2026-10-legacy.md`](old/docs-archive/AGENTS-2026-10-legacy.md);
`docs/AGENT.md` maps it.

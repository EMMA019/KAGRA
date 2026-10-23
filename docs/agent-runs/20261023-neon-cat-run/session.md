# Neon Cat Run

Sidecar sim, same shape as Lumen (`neon_run`). Not a WorldPlay genre.

## Collect

`tools/fetch_assets.py` plus `tools/fetch_assets/neon_cat.json`. Licenses are
CC0-1.0 only. Kenney Cube Pets cat (run / walk node clips), Platformer Kit
crate and barrel, Racing Kit hoop and barrier, and the Quaternius dungeon
arch already in the repo. Poly Pizza returned HTTP 403. The Quaternius
animated-animal pack that has a jump clip is a Drive folder with no stable
file URL, so it was not vendored. Jump is a position change while `run`
keeps playing.

Kenney GLBs referenced `Textures/colormap.png`. The loader only reads images
from a buffer view, so the fetch script embeds the PNG and keeps the loose
file. Racing-kit props sat on a translated root; `seat_origin` zeros that
translation. `tools/check_gltf_asset.py` passed on each GLB. `docs/assets.md`
is the collect → check → use note for the next game.

## Animation

Cube Pets is rigid node clips, not a skin. `gltf_load` stores the extra clips
only when `skin_joints` is empty and samples that node's world matrix.
`walker.anim = "run"` selects it. VRM paths still have empty `extra_clips`.
`compile_meshes` and `update_world_gltf` both pass `anim`.

## Sim

`SceneKind::NeonCat`. Chase camera looks along +Z, so `+lx` / D is world −X.
Space, W, and stick-up jump. Grace 1.6s. Crash is shake, 0.11s hit-stop,
sparks, then the flow result. Dump sets bloom, FXAA, and fog. Phone 8-bit
canvases still skip HDR bloom until that engine fix lands; emissive is what
colors the LDR picture. Lavapipe (fp16) blooms.

The tunnel grid and rails stay emissive boxes. No CC0 pack is an infinite
synthwave shader. Cat, arch, hoop, crate, barrel, and barrier are glTF.
A missing file omits the prop (the cat falls back to the engine capsule).

## Title tap on the preview

The catalog page (`scripts/preview_site/play.html`) kept the HTML card on
「はじめる」 because it read `statsJson().game_phase`. That field stays the
demo phase for a sidecar, so the card never followed Neon Cat. The card also
listened for `click` only, while `touch-action: none` and a document
`touchmove` preventDefault drop the click on Chrome Android. A tap that did
reach the canvas started a run; the cat crashed, and the CRASH / RETRY /
TITLE column showed through the translucent card.

The page now reads `gameJson().phase` (`play` / `result`), puts the card at
`z-index: 4` above the canvas, and starts the run from `pointerup` via
`startGame` (no center hit-test that can land on TITLE). `showTitle` runs
when that card is the title, and it puts the cat back at the start.
`setExternalMenu` skips the in-canvas column. `stats().game_phase` maps the
sidecar phase so other hosts see `playing` / `complete`.

Preview's Pages wait treated a cancelled `kagra-pages` run as fatal.
GitHub cancels the previous pending deploy in that group. The wait now
skips `completed cancelled` and follows the run that finishes. The live
`/pr/165/` tree already had this page before that job went red.

## Checks

- `cargo test -p kagra-shared --lib neon_cat::` — steer, jump, ring, glTF, session swap with Lumen
- `rigid_run_clip_moves_the_mesh_node`
- `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings`
- `cargo clippy -p kagra-shared --all-targets --features render --locked -- -D warnings`
- `pytest tests/test_neon_cat_assets.py tests/test_browser_preview.py`
- `python tools/gen_api_index.py --check`
- Lavapipe offscreen: title / play / crash at 1280×720 and play at 390×844

`python -m kagra.verify` needs the PyO3 extension (`maturin develop`). It was
not built in this run. The smoke scenario is `examples/verify_scenarios/neon_cat_smoke.json`.

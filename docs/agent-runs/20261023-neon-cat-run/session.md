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

## Unexplained deaths

Two kills were not obstacles.

The rail check used `|x| > 2.05` after a 1.6s grace. Full steer during
grace carries the cat to about x = 8. The first frame after grace then
dies in empty space. The arch pillars are visual only (`|x|` about
1.08–2.36). That wall plane sits inside a pillar, so a brush of the
arch looked like a death with nothing to hit. The floor grid, the
hoops, and the arches are not colliders. Spawn obstacles start at
z = 26, after grace.

`crate.glb` and `barrel.glb` use glTF index componentType 5121
(UNSIGNED_BYTE). The loader rejected that, `glb_ready` failed, and
those props were omitted. The sim still killed on them, so the crate
and the barrel were invisible hit volumes. The old volume was also the
mesh plus 0.22 in x and 0.25 in z. The barrier mesh was not centered
in z, so the padded box closed the gap beside a side barrier.

The lane now clamps to ±2.15 (the rail is at ±2.35) and does not kill.
The kill box is the baked mesh AABB, inset 6cm on x/z and cut at 82%
of the height. Feet are a point. A jump still clears it (apex about
1.15, crate top about 0.58). `read_indices` accepts UNSIGNED_BYTE, so
the crate, the barrel, and the cat's legs load. The barrier is placed
so its mesh center sits on the anchor.

`state_json.death` is `CRATE` / `BARREL` / `BARRIER`. The result line
is `HIT CRATE`. The HTML card shows the same when the phase is
complete. `?debug=1` calls `setDebug` and draws a cage on the mesh
skin (the inset kill box would sit inside the crate) plus the lane
limits.

## Preview was still the previous wasm

The Pages tree for `/pr/165/` after `9e27b10` was last modified
2026-10-23 19:29Z and does not contain `setDebug` or `HIT `. The wasm
still has the string `unsupported index componentType` and the result
word `CRASH`. GitHub Actions did not start a workflow for that commit,
so Emma's phone was still on the build where a failed glTF load leaves
the collider in place.

`gltf_mesh_for` also treated any file named `crate.glb` as the embedded
unit cube, including the Kenney path. A bare `crate.glb` stays that
alias. A path loads the file. If `mesh_from_glb` fails, the obstacle is
not spawned and the miss is logged once. Portrait `412/915` with
`quality` `mobile` and `low` keeps each obstacle in the compiled scene.

## Chase camera

Emma asked for a more 3D camera on the same phone path. The default rig
sits 4.85 m behind the cat and 2.15 m up, and looks 10.5 m down the
tunnel. Eye X, eye Y, look X, Dutch roll, and FOV are critically damped
springs. Look X tracks 0.9 of the cat so a portrait frame still holds
the next crate when the cat is on the rail. The eye spring is quick
enough that the cat stays in that frame while the stick is held. FOV moves from 50° at the start speed to 58° at the cap. Roll
is at most about 0.07 rad and does not change the stick: `+lx` stays
world −X. A landing with `vy < -1.5` calls the same shake as
`kagra.juice.Juice.shake` (0.08 m, 0.18 s, 28 Hz, square decay). The
offset is added to the eye and the look target.

`cam: "classic"` (page `?cam=classic`) keeps the old eye (7.4 m back,
1.72 m up, look 12 m ahead, FOV 50, no roll, no landing shake).
`WorldCamera.roll` is the Dutch angle. Zero stays world up and is
omitted from dumps. Portrait 412×915 keeps the next crate inside the
frame, including when the cat is on the rail.

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

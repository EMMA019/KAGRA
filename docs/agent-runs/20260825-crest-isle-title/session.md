# Session — Crest Isle opaque title screen

Master at start: local snapshot was `32686a3` (#81). Fetched `origin/master` = `f11b74e` (#83 chase-cam / meadow tint already merged). Branched `cursor/crest-isle-title-screen-d128` from that.

## Cause (verified in source)

`CrestIsle.draw` always ran:

1. `cls` fog-grey
2. `sky_stage` / `sky`
3. `world.draw` / `water` / `Prop.draw_all` / glow billboards / `draw_vrm` / vignette
4. then, if `mode == "title"`, `_banner` → `kagra.fill(..., 118)`

Public `fill` defaults `alpha=255`. 118 is a see-through navy wash. The chase camera sits on the spawn looking at a half-streamed / UV-split island, so Emma's pre-SPACE shot is the live 3D showing through, not a missing Label. Play HUD `self.title` is only drawn in the play arm.

`self.mode = "play" if SMOKE else "title"` was already correct. SPACE/ENTER in `update` already leaves title.

## Decisions

- Title arm returns **before** sky / world / water / props / VRM. Solid `cls(6, 12, 20)` + `_banner(..., overlay_alpha=255)`. No puresky on title: Emma's shot already had a split/blocky sky, so a sky-only backdrop was not worth the risk.
- Result keeps drawing the island and the old alpha-118 overlay (allowed).
- Play HUD, meadow `GRASS_TINT`, chase-cam clamp, IBL, rehold: untouched.
- GPU-free source test `test_title_draw_skips_live_world` in `tests/test_open_world.py` (same style as the other Crest Isle source asserts). SMOKE verify still covers play only.

## Stumbles

- Snapshot git was behind #82/#83. Fetched `origin/master` before branching.
- Did not treat "title text missing" as a Label bug: `self.title` is play HUD; the banner `kagra.text` was there, just lost on the glitched composite.

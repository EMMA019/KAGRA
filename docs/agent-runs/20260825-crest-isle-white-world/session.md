# Session — Crest Isle white world + delayed stop after long hold

Master at start: local snapshot was `88d074c` (#79). Fetched `origin/master` = `c70875a` (#80 sticky-walk already merged). First PR (#81) landed IBL/fog/sun/Python intern. Follow-up clip analysis re-prioritized untextured 1×1 white over IBL wash.

## Hypotheses (kept / discarded)

| Hypothesis | Verdict |
|---|---|
| PR #78 `kagra.stage` unshadow / TypeError | **Discarded.** Window title is VRM Crest Isle; she is in-game. Stage is callable. |
| PR #79 slope foot AABB | **Discarded.** White world and delayed stop are not grounding. |
| SMOKE `inject_key("W")` missing `down=False` | **Discarded** as instructed. `inject_key` uses `on_key_down` and bypasses `apply_key` / `rehold_*`. |
| Terrain `_upload_tile` skipped (`_terrain_tex<=0`) | **Discarded.** Missing mesh ≠ white plane. A plane is drawing. |
| `cam.toon` from PR #76 (`lit>1`) | **Discarded.** `apply_outdoor_look` does not call `set_toon_params`; default softness=1.0 keeps Lambert. |
| Kenney `Textures/colormap.png` missing from git | **Discarded.** File is vendored; `flatten_gltf(tree.glb)` already returned the PNG in tests. |
| Grass JPEG missing on disk | **Discarded.** File is 666655 bytes JFIF. Missing-file fallback is procedural green `(76,140,62)`, not white. |
| Additive Lambert IBL at puresky 0.95 | **Amplifier, not the clip.** Nature Kit vertex colors and bronze coins stayed distinct; the meadow/trees read as untextured 1×1 white, not bloom. IBL/`env*albedo`/sun/`fog` still shipped in #81. |
| Crest Isle `set_light_dir(-0.32, -1.0, 0.22)` | **Amplifier.** API is vector *toward the sun*. Fixed in #81. |
| Sky sphere past `fog_end` | **Amplifier for the grey sky.** Radius 140, fog end 102. Backdrop fog skip shipped in #81. |
| Mesh3D bind-group FIFO 64 | **Primary white ground/trees.** `MESH3D_TEX_BG_MAX=64`; `ensure_mesh3d_tex_bg` `pop_front` when full. Cache miss → Fallback White 1×1 `[255,255,255,255]`. Crest Isle `VISTA_PROPS >= 120`. Grass uploaded first in `bake_terrain`, then 120+ Props. Same-pass `ensure` recreates grass then evicts it before draw. Nature Kit vertex colors * a live white `solid_tex` still show hue. |
| Shared `kagra_prop_gltf.png` + no path intern | **Amplifier / hitch.** Every Kenney Prop rewrote one tempfile and `load_texture_ex` minted a new GPU id, so FIFO 64 filled with unique colormap ids. Python hash intern shipped in #81; this pass adds engine path intern (not the rig `(id, part)` map). |
| `#80` `rehold_block` only this frame + next | **Kept.** Long auto-repeat then WM_KEYUP can deliver leftover `repeat=false` KEYDOWN more than 1–2 frames later, worse if a hitch stalled `begin_frame`. Taps stay on the short window. Wish-idle snap in `Walk` is already correct once `held` clears. |

## Fixes (this pass, after #81)

- Mesh3D BG cache: LRU touch on hit, max 256, drop only dead keys (live grass/colormap must not become Fallback White).
- `load_texture_ex` path intern (`path_texture_cache`). Rig `texture_cache` unchanged.
- `World3D.update`: at most 1 new stream tile per frame after the first ring (`bake_terrain` still fills the ring).
- Input: leftover non-repeat KEYDOWN after a long hold (or any auto-repeat while held) refreshes a 15-frame quiet window. 30 leftover frames stay idle; 15 silent frames then a real press holds. `saw_repeat` arms quiet even when a hitch starved `begin_frame` counts. Taps keep #80’s 1–2 frame `rehold_block`. `inject_key` / IME scan pairing unchanged.

## Stumbles

- Snapshot git was behind #80. Fetched `origin/master` before branching.
- First pass treated IBL as the white-world cause. Clip + FIFO 64 measurement discarded that as primary; IBL/fog/sun stay as extras already on master.
- Did not treat “trees are white” as colormap-missing once Nature Kit still showed vertex color and `flatten_gltf` already found the PNG.
- PR #81 merged while this pass was still in flight; follow-up work is a new PR to master.

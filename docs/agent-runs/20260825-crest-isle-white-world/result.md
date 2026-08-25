# Result — Crest Isle white meadow + delayed stop

## Cause

White meadow / white Kenney pines in Emma’s clip were **untextured Fallback White**, not missing files and not (primarily) IBL blowout.

1. Mesh3D `(diffuse, normal)` bind-group cache was FIFO 64. Crest Isle uploads grass first, then `VISTA_PROPS >= 120`. `ensure_mesh3d_tex_bg` evicted grass in the same pass that drew it; `mesh3d_tex_bg` miss uses 1×1 `[255,255,255,255]`.
2. Amplifier: every Kenney `Prop.bake` clobbered one tempfile and minted a new GPU id (`load_texture_ex` had no path intern), so unique keys blew past 64. Nature Kit GLBs with no image use `solid_tex(color)` and keep vertex hue.
3. Amplifier (already in #81): Lambert IBL, inverted Crest Isle sun, puresky past `fog_end`.

Delayed stop: `#80` `rehold_block` lasted one extra frame. After a long auto-repeat, Win32 leftover `repeat=false` KEYDOWN can arrive later, especially if `stream_tiles` / Prop bake stalled winit. Wish-idle `vx/vz` snap was already correct once `held` clears.

## Verify

Pending local commands on this pass (bind-group LRU + quiet window).

## Files

- `kagra-core/src/renderer/gpu_helpers.rs` / `mod.rs` — LRU Mesh3D BG cache, max 256, never evict live
- `kagra-core/src/window.rs` — path intern for `load_texture_ex`
- `kagra-core/src/input.rs` — 15-frame quiet window, leftover refresh, `saw_repeat`
- `kagra/world3d.py` — 1 new tile / frame after the first ring
- `tests/test_open_world.py` / `test_world3d.py` — GPU-free cache + stream budget checks

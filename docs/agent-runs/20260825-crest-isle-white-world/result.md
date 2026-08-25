# Result — Crest Isle white meadow + delayed stop

## Cause

White world was lighting + fog, not missing assets:

1. Lambert IBL added raw irradiance (`env * 0.95`) instead of `env * albedo * 0.35` (VRM).
2. Crest Isle `set_light_dir` used −Y (away from the sun).
3. Puresky sky sphere sat past `fog_end`, so the backdrop became fog/`cls` grey.

Delayed stop: `#80` `rehold_block` lasted one extra frame. After a long auto-repeat, Win32 leftover `repeat=false` KEYDOWN can arrive later.

Loading hitch: every Kenney `Prop.bake` rewrote one shared tempfile and uploaded a new texture, so the unit-mesh cache missed.

## Verify (this VM)

- `python3 tools/gen_api_index.py --check`: **OK** (422 entries)
- `python3 -m pytest tests -m "not golden"`: **384 passed**, 10 deselected
- `rustup run stable cargo test -p kagra-core --no-default-features --locked input`: **14 passed** (includes `long_hold_keyup_blocks_multiframe_nonrepeat_burst` and `short_tap_still_reholds_after_one_frame_block`; IME / `#80` tests still pass)
- `python -m kagra.verify` / GPU golden: **not run** here (`kagra_core` wheel not built; Lambert HDRI-off goldens should be unchanged). CI is the GPU stand-in.

## Files

- `kagra-core/src/renderer/shaders.rs` — Lambert IBL
- `kagra-core/src/input.rs` — long-hold `rehold_left`
- `kagra/look.py` — outdoor IBL constants, fog snapshot, CPU Lambert
- `kagra/stage.py` / `kagra/play.py` — backdrop fog skip, Prop tex cache
- `kagra/gltf_mesh.py` — Windows sidecar URI
- `examples/vrm_open_world.py` / `vrm_relic_run.py` — sun dir / IBL strength

# Session — agent eyes (annotate + debug_trace + follow clip + Prop toon)

Master at start: `8a2af0a` (Crest Isle #73). Relic Run / sticky-walk already on master.

## API search

- `avatar.pick` / `pick_vrm_bone` / `Camera3D.ray_from_screen` already exist.
- `hovered_prop` / `clicked_prop` already exist. `Prop` had no stable id → added sequential `Prop.id`.
- `Physics3D.raycast` hits AABB / capsule / OBB / trimesh. Extended with `ignore=`, `skip_triggers`, `static_only` so chase-cam rays skip the player capsule and falling crates.
- `World3D.ground_y` / `set_height_fn` are the terrain numbers `debug_trace` needs. No Rapier.
- `SHADER_3D` Lambert used `clamp(dot(n,L), 0.2, 1.0)`. VRM used `cam.toon` smoothstep when softness < 0.999. Default softness=1.0 keeps existing goldens.

## Decisions

- New public functions, not a Tk inspector: `kagra.annotate`, `kagra.debug_trace`, `DebugTrace`.
- JSONL under `scratch/` (gitignored). Tests pass a tmp path.
- `Camera3D.follow(..., world=)` clips. `Walk` passes `world=` automatically. Switch Room / Dodge Room explicit follows do too.
- Prop/terrain toon only on the Lambert path. PBR (metal) unchanged.
- Pairwise golden `prop_toon` / `prop_toon_off` (same pattern as indoor_spot). GPU / `golden` mark; CI may skip here.

## Stumbles

- `annotate(..., screenshot=)` would shadow `kagra.screenshot()`; capture uses `_engine.request_screenshot`.
- Look-at sits inside the player capsule; raycast `t==0` is already skipped, but `ignore=player` is still required when the eye is outside.
- Switch walls are 1.8 m; camera height 1.9. The *segment* still hits (look_y=1.0 → eye 1.9 crosses the wall at ~1.4 m). Boxed-room test uses `walls()` from switch_room_rules.

# Session — flight pose + bundled glTF showcase on a free WorldDoc

## Decisions

- Do not add a flight or CTF genre to `WorldPlay`. Games live in `kagra/flight.py` (`examples/flight_minimal.py`) and `kagra/backyard_brawl.py` (`examples/backyard_brawl_minimal.py`) on a free dump (`genre` omitted).
- `position` stays world-space unless `local: true`. Crest coins already set `parent` as a link; composing that as TRS would move them.
- Pose is intrinsic yaw, then pitch (nose up), then roll. Positive yaw turns +Z toward +X.
- A disabled prop is still a hinge, so the airframe root or player hand mount is not drawn as a stray box.
- Rings are `model: "ring"` (XY torus, hole along +Z), mesh id 7. glTF slots start at 8. Ids stay dense.
- Window frames: `Scene._frame_rgba` from `render_rgba`, presented as PPM. Headless files stay PNG.
- Multi-primitive static `.glb`/`.gltf` (Quaternius medieval village buildings, trees) now apply node transforms and synthesize a 1D color palette from `pbrMetallicRoughness.baseColorFactor` so roofs, plaster walls, timber, and foliage render in color instead of only the first primitive.
- Skinned `.glb` characters with unskinned weapon/shield attachment meshes at the origin (KayKit Adventurers: `Knight.glb`, `Mage.glb`, `Rogue.glb`, `Barbarian.glb`) skip unskinned primitives when `JOINTS_0` is present on body parts, and `pick_clip` matches `Walking_*` / `Running_*`.
- Added asset aliases (`knight`, `rogue`, `mage`, `barbarian`, `raccoon`, `wolf`, `inn`, `house`, `tower`, `mill`, `gazebo`, `well`, `stylized_tree`, `stylized_pine`, `rock`, `chest`) in `kagra-shared/src/assets.rs` and `kagra/contracts.py`.

## Stumbles

- Exhaustive `WorldProp { ... }` literals in action2d / rpg / td needed `pitch` / `roll` / `local`.
- Quaternius medieval village `.glb` files had `scale: [100, 100, 100]` and a -90 deg X rotation on child mesh nodes plus multi-primitive `baseColorFactor` materials; before merging primitives and node transforms, only the first primitive rendered at 0.01x scale.
- KayKit `Knight.glb` bundled 9 unskinned weapon/shield meshes at the origin and named its walk clip `Walking_A`; filtering unskinned primitives when skinned primitives exist and matching `Walking_*` + setting `anim_blend: 1.0` fixed both the origin clutter and the T-pose.
- `kagra.ui2d.message` wraps text inside its own `panel` background; inside an existing HUD panel, `list_lines` avoids double boxes.

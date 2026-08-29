# Relic Run example assets — licenses

These files are **not** part of the KAGRA pip wheel. They ship with the
git tree so `python examples/vrm_relic_run.py` looks like a 30s game, not
a debug island.

All files below are **CC0 1.0 Universal** (public domain dedication).
No paid packs. No VRoid / AvatarSample. The VRM on screen is Alicia
Solid (`kagra.ensure_vrm`), credited separately (Dwango) in
`examples/vrm_relic_run.py`.

## Kenney Mini Forest 1.0

- License: CC0 1.0 (see `kenney/Kenney_Mini_Forest_License.txt`)
- Pack page: https://kenney.nl/assets/mini-forest
- Zip used: https://kenney.nl/media/pages/assets/mini-forest/44a89aed7f-1784024079/kenney_mini-forest_1.0.zip
- Author: Kenney (www.kenney.nl)

| File | Zip path |
|---|---|
| `kenney/tree.glb` | Models/GLB format/tree.glb |
| `kenney/tree-high.glb` | Models/GLB format/tree-high.glb |
| `kenney/rocks-high.glb` | Models/GLB format/rocks-high.glb |
| `kenney/rocks-low.glb` | Models/GLB format/rocks-low.glb |
| `kenney/stones.glb` | Models/GLB format/stones.glb |
| `kenney/plant.glb` | Models/GLB format/plant.glb |
| `kenney/fence.glb` | Models/GLB format/fence.glb |
| `kenney/flag.glb` | Models/GLB format/flag.glb |
| `kenney/patch-grass.glb` | Models/GLB format/patch-grass.glb |
| `kenney/patch-dirt.glb` | Models/GLB format/patch-dirt.glb |
| `kenney/tent.glb` | Models/GLB format/tent.glb |
| `kenney/Textures/colormap.png` | Models/GLB format/Textures/colormap.png (referenced by the Mini Forest glTFs) |

## Kenney Nature Kit 2.1

- License: CC0 1.0 (see `kenney/Kenney_Nature_Kit_License.txt`)
- Pack page: https://kenney.nl/assets/nature-kit
- Zip used: https://kenney.nl/media/pages/assets/nature-kit/37ac38a37b-1677698939/kenney_nature-kit.zip
- Author: Kenney (www.kenney.nl)

| File | Zip path |
|---|---|
| `kenney/rock_largeA.glb` | Models/GLB format/rock_largeA.glb |
| `kenney/rock_tallA.glb` | Models/GLB format/rock_tallA.glb |
| `kenney/stone_smallTopA.glb` | Models/GLB format/stone_smallTopA.glb |
| `kenney/mushroom_red.glb` | Models/GLB format/mushroom_red.glb |

Nature Kit GLBs use vertex `baseColorFactor` (no external PNG). KAGRA’s
`flatten_gltf` currently applies the **first** material’s factor to the
whole mesh, so mixed-material kits look like a single tint — still
readable rocks/pedestals. Relic Run therefore plants Mini Forest
`plant.glb` (colormap) instead of `mushroom_red.glb` (white first
material). `mushroom_red.glb` stays in the tree for credit / reuse.

## Poly Haven — aerial_grass_rock

- License: CC0 1.0
- Page: https://polyhaven.com/a/aerial_grass_rock
- Author: Rob Tuytel
- Files: `polyhaven/aerial_grass_rock_diff_1k.jpg`,
  `polyhaven/aerial_grass_rock_nor_gl_1k.jpg` (1K JPG downloads from
  the asset page). Normal map is vendored for completeness; Relic Run
  currently samples the diffuse only (`kagra.load`).

## Poly Haven — kloofendal_48d_partly_cloudy_puresky

- License: CC0 1.0
- Page: https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky
- Author: Greg Zaal
- Source download: 1K HDR (`kloofendal_48d_partly_cloudy_puresky_1k.hdr`)
- Vendored: `polyhaven/kloofendal_48d_partly_cloudy_puresky_1k.png`
  (1024×512 equirectangular, Reinhard + gamma from the HDR so
  `kagra.stage` / `set_hdri` can load it as an 8-bit texture).

## Locomotion

Relic Run does **not** vendor Mixamo FBX / synthetic BVH. Walk and idle
use `VrmAvatar` built-in clips (`kagra/vrm_motion.py`) so arms hang and
swing instead of Mixamo T-pose deltas. Optional override:
`examples/assets/relic_run/walk.vrma` (none shipped).

# Crest Isle example assets — licenses

These files are **not** part of the KAGRA pip wheel. They ship with the
git tree so `python examples/vrm_open_world.py` looks like a wide outdoor
collectathon, not a debug heightmap.

All files below are **CC0 1.0 Universal** (public domain dedication).
No paid packs. No Nintendo IP. The VRM on screen is Alicia Solid
(`kagra.ensure_vrm`), credited separately (Dwango) in
`examples/vrm_open_world.py`.

Poly Haven grass + puresky HDRI are **shared with Relic Run** at
`examples/assets/relic_run/polyhaven/` (not copied twice).

## Kenney Mini Forest 1.0

- License: CC0 1.0 (see `kenney/forest/License.txt`)
- Pack page: https://kenney.nl/assets/mini-forest
- Zip used: https://kenney.nl/media/pages/assets/mini-forest/44a89aed7f-1784024079/kenney_mini-forest_1.0.zip
- Author: Kenney (www.kenney.nl)

| File | Zip path |
|---|---|
| `kenney/forest/tree.glb` | Models/GLB format/tree.glb |
| `kenney/forest/tree-high.glb` | Models/GLB format/tree-high.glb |
| `kenney/forest/plant.glb` | Models/GLB format/plant.glb |
| `kenney/forest/fence.glb` | Models/GLB format/fence.glb |
| `kenney/forest/flag.glb` | Models/GLB format/flag.glb |
| `kenney/forest/patch-grass.glb` | Models/GLB format/patch-grass.glb |
| `kenney/forest/patch-dirt.glb` | Models/GLB format/patch-dirt.glb |
| `kenney/forest/bridge.glb` | Models/GLB format/bridge.glb |
| `kenney/forest/tent.glb` | Models/GLB format/tent.glb |
| `kenney/forest/rocks-high.glb` | Models/GLB format/rocks-high.glb |
| `kenney/forest/rocks-low.glb` | Models/GLB format/rocks-low.glb |
| `kenney/forest/rocks-ramp.glb` | Models/GLB format/rocks-ramp.glb |
| `kenney/forest/stones.glb` | Models/GLB format/stones.glb |
| `kenney/forest/platform.glb` | Models/GLB format/platform.glb |
| `kenney/forest/ladder.glb` | Models/GLB format/ladder.glb |
| `kenney/forest/Textures/colormap.png` | Models/GLB format/Textures/colormap.png |

## Kenney Nature Kit 2.1

- License: CC0 1.0 (see `kenney/nature/License.txt`)
- Pack page: https://kenney.nl/assets/nature-kit
- Zip used: https://kenney.nl/media/pages/assets/nature-kit/37ac38a37b-1677698939/kenney_nature-kit.zip
- Author: Kenney (www.kenney.nl)

Cliffs, flowers, grass tufts, pines, palms, bushes, fences, logs.
Folder: `kenney/nature/*.glb` (vertex `baseColorFactor`; no external PNG).

## Kenney Fantasy Town Kit 2.0

- License: CC0 1.0 (see `kenney/town/License.txt`)
- Pack page: https://kenney.nl/assets/fantasy-town-kit
- Zip used: https://kenney.nl/media/pages/assets/fantasy-town-kit/efe948d309-1754222374/kenney_fantasy-town-kit_2.0.zip
- Author: Kenney (www.kenney.nl)

Ruins, banners, stairs, pillars, extra trees: `kenney/town/*.glb`
plus `kenney/town/Textures/colormap.png`.

## Kenney Castle Kit

- License: CC0 1.0 (see `kenney/castle/License.txt`)
- Pack page: https://kenney.nl/assets/castle-kit
- Zip used: https://kenney.nl/media/pages/assets/castle-kit/a395102d20-1711543616/kenney_castle-kit.zip
- Author: Kenney (www.kenney.nl)

Peak flags and large trees: `kenney/castle/*.glb`
plus `kenney/castle/Textures/colormap.png`.

## Kenney Mini Dungeon

- License: CC0 1.0 (see `kenney/dungeon/License.txt`)
- Pack page: https://kenney.nl/assets/mini-dungeon
- Zip used: https://kenney.nl/media/pages/assets/mini-dungeon/6cd72dc849-1785314274/kenney_mini-dungeon.zip
- Author: Kenney (www.kenney.nl)

Coins / chest / column / banner: `kenney/dungeon/*.glb`
plus `kenney/dungeon/Textures/colormap.png`.

## Poly Haven — aerial_grass_rock

- License: CC0 1.0
- Page: https://polyhaven.com/a/aerial_grass_rock
- Author: Rob Tuytel
- Files: `examples/assets/relic_run/polyhaven/aerial_grass_rock_diff_1k.jpg`
  (and normal map, unused here).

## Poly Haven — kloofendal_48d_partly_cloudy_puresky

- License: CC0 1.0
- Page: https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky
- Author: Greg Zaal
- File: `examples/assets/relic_run/polyhaven/kloofendal_48d_partly_cloudy_puresky_1k.png`
  (1024×512 equirectangular for `kagra.stage` / `set_hdri`).

## Locomotion

Crest Isle does **not** vendor Mixamo FBX / synthetic BVH. Walk and idle
use `VrmAvatar` built-in clips. Optional override:
`examples/assets/open_world/walk.vrma` (none shipped).

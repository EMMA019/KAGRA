# Collecting glTF for a game

New games use authored `.glb` meshes (cat, obstacles, gates, props) through
the shared glTF PBR loader. A missing file is not a reason to invent the
hero out of boxes. Collect it, check it, then reference the path.

## License

Every committed asset is recorded in `assets/LEDGER.json` (see `docs/asset_ledger.md`: ledger, CI license check,
import checks, and the private-assets design). Only **CC0** and **CC-BY**. Record the source URL, the license, and the
credit line next to the files. CC-BY must keep that credit. Do not download
paid packs, ShareAlike, non-commercial, or "free for personal use" files.

Places that publish those terms and a direct file:

- Kenney — `https://kenney.nl/assets` (CC0 zip on the asset page)
- Quaternius — pack pages and the CC0 kits already under `assets/models/quaternius`
- Poly Pizza — filter to Public Domain (CC0) or CC-BY, and keep the model page URL
- OpenGameArt — CC0 or CC-BY only, from the author's file

## Flow

1. Add a pack entry to a manifest such as `tools/fetch_assets/neon_cat.json`.
   `license` is `CC0-1.0`, `CC-BY-4.0`, or `CC-BY-3.0`. `download_url` is the
   official `https` zip or `.glb`. `files` lists zip paths and the repo path
   under `dest`.
2. Run:

```bash
python tools/fetch_assets.py tools/fetch_assets/neon_cat.json
```

The script downloads, extracts only the named files, writes `CREDITS.md`,
and runs `tools/check_gltf_asset.py` on every `.glb`. `--check` repeats the
checker without downloading.

3. Point the prop or walker `gltf` field at the repo-relative `.glb`.
   Bundle those paths in `games/<game>.json` `bundle_assets` so wasm and
   Android mount them. A missing mount leaves the engine's own fallback
   (capsule for a walker). Do not add a second, game-local mesh.

## Packaging notes the script records

Kenney GLBs often point at `Textures/colormap.png`. The loader reads images
from a buffer view, so `embed_images: true` copies that PNG into the GLB.
The loose PNG stays beside it. Vertices and material factors are unchanged.

Older Kenney exports put the mesh on a translated root node.
`seat_origin: true` sets that translation to zero so a prop at the origin
is the mesh. Vertices are not moved by hand.

Prefer an animated character when the pack has one. The walker plays the
clip named `run` / `jump` / `dance` when `walker.anim` says so, and otherwise
the Walk clip. A rig with no skin (Kenney Cube Pets: separate limb nodes)
is sampled as rigid parts on that same walker path.

Neon Cat uses Kenney Cube Pets `animal-cat.glb` (`run` / `walk` node clips).
That pack has no jump clip, and the Quaternius animal pack that does is a
Drive folder without a stable file URL, so the jump stays a position change
while `run` keeps playing. Do not substitute a paid or non-commercial cat.

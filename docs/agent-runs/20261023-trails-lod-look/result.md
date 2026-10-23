# Result

## Shipped

- Dump `cull` and `prop_lod`. Omit either and every prop stays a full mesh.
- glTF `metallicRoughnessTexture` lands on the ORM texture. Toon skips it. Dump maps still win.
- `kagra.Trail` and `apply_look(world, "pastel"|"night")`.
- Box Hop night shot with the hop ribbon. `docs/quality_gap.md` moves those rows to have / partial. SSAO, DOF, area lights, SMAA, custom shaders, cubemap PMREM, HDR env files, contact-hardening shadows, character LOD, VRM-on-Wasm, and Android stay parked.

## Verify

`cargo test -p kagra-shared --features render --locked` (538 lib + 15 offscreen). Clippy `-D warnings` with and without `render`. `pytest tests -m "not golden"`. `python tools/gen_api_index.py --check`.

Screenshots (lavapipe, same landed hop; night, trail, cull, and prop LOD only on the after frame):

```
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
  python3 examples/platformer_minimal.py --compare-wave3 docs/screenshots
```

`mean_abs=149.09`. The after frame is the night sky, the cooler platforms, and the hop ribbon. The platforms stay meshes.

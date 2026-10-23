# Result

## Shipped

- Dump `emissive`, `emissive_color`, `emissive_map`, `roughness_map`, `metalness_map`, `shadow_softness`.
- Box Hop well glow, worn crates, shadow radius 3.2.
- `docs/quality_gap.md` marks this wave and leaves image IBL as the next one.

## Verify

`cargo test -p kagra-shared --features render --locked emissive` (ORM unit test and the lavapipe emissive frame).

Screenshots (lavapipe, same juicy pose, wave 2 only on the after frame):

```
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
  python3 examples/platformer_minimal.py --compare docs/screenshots
```

`mean_abs=1.66`. The after frame is the glowing well, the split crate map, and the wider sun shadow.

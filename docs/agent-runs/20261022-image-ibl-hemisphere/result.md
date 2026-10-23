# Result

## Shipped

- Dump `hemi_sky`, `hemi_ground`, `env_map` (`preset:studio` or a PNG equirect).
- Box Hop: cooler sky bounce, warmer ground, studio env, on top of the emissive / ORM / soft-shadow picture.
- `docs/quality_gap.md` moves image IBL out of the next wave. Trails, prop LOD, dump frustum bounds, and a pastel/night preset remain. Cubemap PMREM and HDR env files stay parked.

## Verify

`cargo test -p kagra-shared --features render --locked studio_env` (bake unit test and the lavapipe metal sphere).

Screenshots (lavapipe, same juicy + wave-2 pose, IBL only on the after frame):

```
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
  python3 examples/platformer_minimal.py --compare-ibl docs/screenshots
```

`mean_abs=0.95`. The after frame is the cooler platform bounce and the warmer ground. The sky clear is the same.

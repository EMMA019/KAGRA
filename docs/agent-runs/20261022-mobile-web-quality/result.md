# Result

## Shipped

- `docs/quality_gap.md` — Unity URP table and three.js parity table, plus the next waves.
- Dump post fields and a 16³ strip LUT on the shared composite.
- `kagra.Juice`, `kagra.ease`, `kagra.apply_post`, `kagra.Emitter`.
- `WorldWalker.scale` for squash/stretch.
- Box Hop uses them. `python examples/platformer_minimal.py --compare docs/screenshots` writes the pair.

## Verify

Pure Python: `pytest tests/test_juice.py tests/test_particles.py`.

Rust: `cargo test -p kagra-shared --locked` and, with lavapipe, `cargo test -p kagra-shared --features render --locked` (`warm_lut_lifts_red_and_drops_blue`).

Screenshots (lavapipe, same landed pose, quality off for the sim):

```
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json \
  python3 examples/platformer_minimal.py --compare docs/screenshots
```

`mean_abs=5.36`. Files: `docs/screenshots/box_hop_before.png`, `docs/screenshots/box_hop_after.png`. The after frame has the punch LUT, bloom on the knight's head and the well, a stronger vignette, landing squash, and yellow sparks.

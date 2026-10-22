# Result

- `FpsSession` on `kagra_shared` / `kagra.FpsSession`
- Arena: `kagra-shared/tests/fixtures/fps_world.json`
- Play: `examples/fps_minimal.py`
- Smoke: `examples/verify_scenarios/fps_smoke.json`
- Headless (lavapipe): `scratch/fps_combat.png` (wave 1, HUD, KayKit raiders), `scratch/fps.png` (clear), `scratch/fps_shared.png` (third-person arena)

Three waves, hitscan, ammo, reload, enemy chase, and HP are Rust. Python draws input and the ASCII HUD.

Headless clear: won, HP 5/5, 7 kills, wave 3/3, phase complete. Retry restores HP, ammo, and wave 1.

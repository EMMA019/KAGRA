# Result

- Plane: `assets/models/biplane.glb` via alias `biplane`. See `assets/models/BIPLANE.txt`.
- Flight scene: `kagra/flight.py`, runner `examples/flight_minimal.py`, smoke `examples/verify_scenarios/flight_smoke.json`.
- Chess pale squares + studio floor: `kagra/chess_game.py`, floor gradient in `kagra-shared/src/render/shader3d.wgsl`, vignette in `kagra-shared/src/render/bloom.wgsl`.
- Backyard title / 90s / result: `kagra/backyard_brawl.py`. `default_state()` stays `phase=play` so `scripted_match` and tests keep running the match. The live scene starts on `title`.
- Screenshots (not committed): `/opt/cursor/artifacts/flight_clear.png`, `flight_bank.png`, `chess_studio.png`, `backyard_title.png`, `backyard_victory.png`.

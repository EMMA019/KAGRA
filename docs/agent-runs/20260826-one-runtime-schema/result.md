# Result

- `Scene3D::from_world_json` / `to_world_json` ingest `World.dump()` JSON (`docs/schemas/world.json` v1).
- GPU-free: Crest Isle-shaped and Orb Rush-shaped dumps roundtrip stable ids, positions, parent, heightfield `fn` / tile keys. Python dump/load of the same fixtures.
- Verify: `cargo test -p kagra-shared --locked` **113 passed**. `pytest tests -m "not golden"` **500 passed**, 10 deselected. `cargo clippy -p kagra-shared --all-targets --locked -- -D warnings` clean. `cargo fmt` clean.
- Roadmap: M2 started (schema). Renderer switch is next. Fake-headless `(-12800,-12800)` left in place (needs the renderer switch).
- Log: this directory.


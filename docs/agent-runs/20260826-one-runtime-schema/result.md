# Result

- `Scene3D::from_world_json` / `to_world_json` ingest `World.dump()` JSON (`docs/schemas/world.json` v1).
- GPU-free: Crest Isle-shaped and Orb Rush-shaped dumps roundtrip stable ids, positions, parent, heightfield `fn` / tile keys. Python dump/load of the same fixtures.
- Roadmap: M2 started (schema). Renderer switch is next. Fake-headless left in place.
- Log: this directory.

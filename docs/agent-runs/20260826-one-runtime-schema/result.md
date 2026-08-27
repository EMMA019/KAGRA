# Result

- Persistent `WorldDoc` ingests `World.dump()` JSON (`docs/schemas/world.json` v1). `Scene3D` stays a one-frame draw list.
- `WorldDoc::compile_scene` emits camera + batches (capsules / box / sphere / plane). GPU-free.
- Crest Isle-shaped and Orb Rush-shaped dumps roundtrip stable ids, positions, parent, heightfield `fn` / tile keys. Python dump/load of the same fixtures.
- Merged origin/master (PR #98). Did not touch tile UV/streaming. Did not start renderer switch.
- Verify: `cargo test -p kagra-shared --locked` **115 passed**. `pytest tests -m "not golden"` **507 passed**, 10 deselected. Clippy `-D warnings` and rustfmt clean. `python3 tools/gen_api_index.py --check` OK (409).
- Log: this directory.



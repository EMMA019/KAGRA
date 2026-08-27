# examples/

Front of the shelf — VRM body, agent games, verify. Start here.

```bash
python -m kagra                          # sing & dance
python examples/vrm_orb_rush.py          # reference game (public APIs)
python examples/vrm_heart_catch.py       # agent-run log in docs/agent-runs/
python examples/vrm_switch_room.py       # boxed room + camera follow
python examples/vrm_dodge_room.py        # falling boxes, survive (agent-run log)
python examples/vrm_relic_run.py          # island relic collect 30s
python examples/vrm_open_world.py         # Crest Isle collectathon (RendererV2)
python examples/world_doc_window.py       # World.dump JSON in a shared wgpu 30 window (capsules; not VRM)
python examples/vrm_vrma.py
python examples/vrm_stream.py
python examples/desktop_mascot.py
```

`kagra-shared` and `mobile/` are a **separate driving demo** (roads, truck, OSM / Wasm).
They are not this Python VRM / game stack. Do not merge the two renderers.

## Archive

Legacy 2D / tilemap / editor / romance / boids live in [`archive/`](archive/).
They still run; they are not the recommended surface.

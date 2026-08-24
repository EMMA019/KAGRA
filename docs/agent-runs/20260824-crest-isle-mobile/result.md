# Result — Crest Isle mobile

## What shipped

- `kagra-shared/src/collectathon.rs` — same height / 8 crests / coins / score as desktop
- `SceneKind::Collectathon` (`set_scene(2)`), `set_walk` / `set_jump`
- Wasm: `kagra-shared/www/crest.html`
- Android debug APK boots Crest Isle (left stick, lower-right jump)
- iOS shell: `setScene(.collectathon)` + the same stick/jump mapping

## What is / is not VRM

| | Desktop (`examples/vrm_open_world.py`) | Mobile / wasm (`kagra-shared`) |
|---|---|---|
| Player | Alicia Solid VRM | Teal Kenney-style **capsule** |
| Terrain / props | Kenney GLB + Poly Haven | Procedural heightfield + cone/box stand-ins |
| Renderer | Python `kagra-core` (wgpu 0.19) | `kagra-shared` (wgpu 30) |
| In the pip wheel | No Kenney | No Kenney |

Do not merge the two renderers.

## Run

```bash
./scripts/build_wasm.sh
python -m http.server -d kagra-shared/www 8000
# http://localhost:8000/crest.html

./scripts/build_android_native.sh
cd mobile/android && gradle :app:assembleDebug
```

Offscreen (GPU host):

```bash
cargo run -p kagra-shared --features render --example offscreen -- 960 540 scratch/crest_isle.png isle
```

## Verify (fill after cargo / pytest)

```bash
cargo test -p kagra-shared
pytest tests -m "not golden"
```

# kagra-shared

Cross-platform shared runtime for **Wasm / Android / iOS**.

```bash
# host tests
cargo test -p kagra-shared

# wasm（Crest Isle: kagra-shared/www/crest.html）
./scripts/build_wasm.sh

# android .so（アプリは Crest Isle を起動）
./scripts/build_android_native.sh
```

C header: `include/kagra_shared.h`

`set_scene(2)` が Crest Isle（Kenney 風カプセル。VRM ではない）。運転デモは `0`。

`WorldDoc::from_json` がデスクトップ `World.dump()` JSON（`docs/schemas/world.json` version 1）を読む。`WorldDoc::compile_scene` が 1 フレームの `Scene3D`（draw list）を出す。高さ場は named fn（`open_world_height` ほか）か dump samples。glTF 部品は `gltf_load`。`WorldPlay` が WASD / 視点で dump を毎フレーム進める。`Scene3D` に dump を詰め込まない。`render_world_doc`（`--features render`）が `compile_meshes` を upload してオフスクリーン RGBA を返す。PNG は `cargo run -p kagra-shared --features render --example offscreen -- W H out.png world dump.json`。Python は `python -m kagra.render_world dump.json out.png`（ヘルパ無しはスキップ）。本物のデスクトップ窓は `cargo run -p kagra-shared --features render --example window -- dump.json` / `python -m kagra.play_world dump.json`（同じ wgpu 30 `Renderer`。公式 Crest プレイ。kagra-core `RendererV2` ではない。旧 VRM デモは RendererV2 のまま）。GPU mesh id はゲームオブジェクトではない。

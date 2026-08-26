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

`WorldDoc::from_json` がデスクトップ `World.dump()` JSON（`docs/schemas/world.json` version 1）を読む。`WorldDoc::compile_scene` が 1 フレームの `Scene3D`（draw list）を出す。`Scene3D` に dump を詰め込まない。GPU mesh id はゲームオブジェクトではない。レンダラ切替はまだ。

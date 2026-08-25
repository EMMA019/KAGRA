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

# Agent / Platform contracts

| Piece | Path |
|-------|------|
| API index | `docs/API_INDEX.md` |
| Input schema | `docs/schemas/input_events.json` |
| Verify scenarios | `examples/verify_scenarios/` |
| MCP server | `tools/mcp_kagra/server.py` |
| Contracts | `kagra/contracts.py` |
| Verify runner | `kagra/verify.py` |
| Touch | `kagra/touch.py` |
| Mobile scaffold | `mobile/` |
| Shared scene (GPU 非依存) | `kagra-shared/src/scene.rs` |
| Shared renderer | `kagra-shared/src/render/` |

Rust errors surface as `[CODE] message` (see `KaguraError::code`).

## CI に合わせたローカル実行

| 確認したいこと | コマンド |
|---|---|
| pure-python テスト（拡張なし相当） | `pytest tests -m "not golden" -p tests.no_extension_plugin` |
| API 索引のドリフト | `python tools/gen_api_index.py --check` |
| 共有コア | `cargo fmt -p kagra-shared -- --check` / `cargo clippy -p kagra-shared --all-targets -- -D warnings` / `cargo test -p kagra-shared` |
| 共有コアの描画 | `cargo test -p kagra-shared --features render`（GPU が無ければ自動スキップ） |
| 描画を目で確認 | `cargo run -p kagra-shared --features render --example offscreen` → `scratch/shared_offscreen.png` |
| Wasm ビルド | `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render` |

`tests/` のテストは `import kagra`（Rust 拡張）に依存させないこと。純ロジックの
モジュールは `tests/conftest.py` の `load_kagra_submodule()` で読む。索引は AST
のみから生成するので、拡張の有無で内容が変わらない。

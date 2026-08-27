# Agent / Platform contracts

エージェント向けの行動規範はリポジトリ直下の [`AGENTS.md`](../AGENTS.md)。
エージェントに何かを作らせるときのログ規約は
[`docs/agent-runs/README.md`](agent-runs/README.md)。

| Piece | Path |
|-------|------|
| API index | `docs/API_INDEX.md` |
| Engine review | `docs/REVIEW.ja.md` |
| Roadmap | `docs/ROADMAP.ja.md` (100% = screenless indie ship; now ~15%. Old 63% is archived) |
| World dump schema | `docs/schemas/world.json` |
| Input schema | `docs/schemas/input_events.json` |
| Verify scenarios | `examples/verify_scenarios/` |
| MCP server | `tools/mcp_kagra/server.py` |
| Contracts | `kagra/contracts.py` |
| Verify runner | `kagra/verify.py` (`expect_world` + optional `expect_offscreen`) |
| Shared offscreen CLI | `kagra/render_world.py` (`python -m kagra.render_world dump.json out.png`) |
| Shared desktop window | `kagra/play_world.py` (`python -m kagra.play_world dump.json`) — wgpu 30 winit; WASD + look; official Crest play |
| Touch | `kagra/touch.py` |
| Mobile scaffold | `mobile/` |
| Shared scene (GPU 非依存) | `kagra-shared/src/scene.rs` |
| Shared 3D draw list | `kagra-shared/src/scene3d.rs` (`Scene3D` = 1 frame: camera / batches / fog) |
| Shared world dump | `kagra-shared/src/world_doc.rs` (`WorldDoc::from_json`, `compile_scene` heightfield+glTF, `WorldPlay` tick, schema `docs/schemas/world.json`) |
| Shared renderer | `kagra-shared/src/render/` (`render_world_doc` = wgpu 30 offscreen; `new_for_window` = same Renderer on a real window; not RendererV2) |

Rust errors surface as `[CODE] message` (see `KaguraError::code`).

## CI に合わせたローカル実行

| 確認したいこと | コマンド |
|---|---|
| 初回体験（サンプル VRM を取得して歌う） | `python -m kagra` / `python -m kagra --offline` |
| PyPI ホイール | タグ `v*` → `.github/workflows/publish.yml`（[PUBLISHING.md](PUBLISHING.md)） |
| pure-python テスト（拡張なし相当） | `pytest tests -m "not golden" -p tests.no_extension_plugin` |
| API 索引のドリフト | `python tools/gen_api_index.py --check` |
| 共有コア | `cargo fmt -p kagra-shared -- --check` / `cargo clippy -p kagra-shared --all-targets -- -D warnings` / `cargo test -p kagra-shared` |
| 共有コアの描画 | `cargo test -p kagra-shared --features render`（GPU が無ければ自動スキップ） |
| 描画を目で確認 | `cargo run -p kagra-shared --features render --example offscreen` → `scratch/shared_offscreen.png`（`world dump.json` で WorldDoc） |
| World.dump → PNG | `python -m kagra.render_world dump.json out.png`（ヘルパ無しはスキップ。`(-12800,-12800)` ではない） |
| World.dump → 窓 | `python -m kagra.play_world dump.json` / `cargo run -p kagra-shared --features render --example window`（WASD。画面無しはスキップ。公式 Crest プレイ。RendererV2 ではない） |
| Wasm ビルド | `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render` |

`tests/` のテストは `import kagra`（Rust 拡張）に依存させないこと。純ロジックの
モジュールは `tests/conftest.py` の `load_kagra_submodule()` で読む。索引は AST
のみから生成するので、拡張の有無で内容が変わらない。

## Cargo.lock は追跡する

依存のパッチ更新だけでビルドが壊れることが実際にあった（wgpu-hal 30 が要求する
gpu-allocator 0.28 が windows 0.62.2 でコンパイルできなくなった）。ライブラリだけの
リポジトリなら lock を無視してよいが、ここはアプリとモバイルシェルを含むので lock を
コミットする。CI の cargo 呼び出しは `--locked` 付きなので、`Cargo.toml` を触ったら
`Cargo.lock` も一緒にコミットすること。忘れると CI が落ちて気づける。

依存を意図的に更新するときは `cargo update -p <crate>` で範囲を絞り、更新後に
共有コアの描画テストまで通してから lock をコミットする。

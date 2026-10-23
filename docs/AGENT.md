# Agent / Platform contracts

エージェント向けの行動規範はリポジトリ直下の [`AGENTS.md`](../AGENTS.md)。
エージェントに何かを作らせるときのログ規約は
[`docs/agent-runs/README.md`](agent-runs/README.md)。

| Piece | Path |
|-------|------|
| API index | `docs/API_INDEX.md` |
| Engine review | `docs/REVIEW.ja.md` |
| Roadmap | `docs/ROADMAP.ja.md` (100% = screenless indie ship; now ~40%, first M3 genre = collectathon. Old 63% is archived) |
| World dump schema | `docs/schemas/world.json` |
| Input schema | `docs/schemas/input_events.json` |
| Verify scenarios | `examples/verify_scenarios/` |
| MCP server | `tools/mcp_kagra/server.py` |
| Contracts | `kagra/contracts.py` |
| Verify runner | `kagra/verify.py` (`expect_world` + optional `expect_offscreen`) |
| Shared offscreen CLI | `kagra/render_world.py` (`python -m kagra.render_world dump.json out.png`) |
| Shared desktop window | `kagra/play_world.py` (`python -m kagra.play_world dump.json`) — wgpu 30 winit; collectathon title→play→result; official Crest play |
| Touch | `kagra/touch.py` |
| Local versus + input sync | `docs/netplay.md` (`FightSession.set_player_input`, `kagra.netplay`, `server/netplay_relay.py`, `examples/versus_local.py`, `examples/versus_online.py`) |
| Physics | `kagra/rigid.py`, `examples/physics_minimal.py`, `kagra-shared/src/physics.rs` (Rapier `PhysicsWorld`, wheel feature `physics`) |
| FPS | `examples/fps_minimal.py`, `kagra-shared/src/fps.rs` (`FpsSession`) |
| Card | `examples/card_minimal.py`, `kagra-shared/src/card.rs` (`CardSession`) |
| Distribution | `docs/distribution.md` (one manifest → Windows zip and Android APK) |
| Phone preview | `docs/preview.md` (GitHub Pages wasm, per-PR path, KAGRA Dev APK) |
| Audio | `kagra/audio.py` (BGM fade, sfx, groups, mute, positional `play_se`), `kagra-shared/src/mixer.rs` |
| Flow | `kagra/flow.py` (`save_dir`), `kagra-shared/src/flow.rs` (title → play → pause → result, volume, high score) |
| Mobile scaffold | `mobile/` |
| Shared scene (GPU 非依存) | `kagra-shared/src/scene.rs` |
| Shared 3D draw list | `kagra-shared/src/scene3d.rs` (`Scene3D` = 1 frame: camera / batches / fog) |
| Shared world dump | `kagra-shared/src/world_doc.rs` (`WorldDoc::from_json`, `compile_scene` heightfield+metal coins+lights 1:1 (empty dump key+fill, contact blob), `WorldPlay` title/play/result, schema `docs/schemas/world.json`) |
| Shared renderer | `kagra-shared/src/render/` (`render_world_doc` = wgpu 30 offscreen; `new_for_window` = same Renderer on a real window; not RendererV2) |

Rust errors surface as `[CODE] message` (see `KaguraError::code`).

## Coordinate system

Right-handed, Y up. View is `look_at_rh`; projection NDC is +x right, +y up.
Screenshot readback copies rows in order and does not flip X.

World +X is the right side of the frame only when the camera looks toward
-Z. The default camera (eye `(0, 2, 6)`, target the origin) does. A chase
camera looking along +Z — the usual "forward is +Z" setup — has
`screen_right = -X`. `D`, `ArrowRight`, and `+lx` move toward that
`screen_right` vector, not toward world +X.

`WorldDoc::to_json` writes `coordinates` (`handedness`, `up`, `note`) and,
on every camera, `screen_right` / `screen_left` (world-space unit vectors).
`from_json` ignores those derived fields. Python `kagra.annotate_dump`
writes the same keys, and `render_rgba` stamps them onto the world dict
before it serializes. Helpers: `kagra.screen_right` / `screen_left` /
`screen_basis` / `wish_world`, and Rust `scene3d::screen_right_xyz` /
`wish_world_xz`.

2D HUD pixels start at the top-left (+x right, +y down). glTF/VRM bind
poses face -Z (model right hand is +X). Walker yaw 0 moves toward +Z.

## Genres

Rust `GenreSession` (rules in Rust, Python is the shell). Smoke JSON is under `examples/verify_scenarios/`.

| Genre | Example | Session |
|---|---|---|
| collectathon | `python -m kagra.play_world` | `WorldPlay` |
| action | `examples/action_minimal.py` | `ActionSession` |
| card | `examples/card_minimal.py` | `CardSession` |
| fight / versus | `examples/versus_local.py` | `FightSession` |
| fishcook | `examples/fishcook_minimal.py` | `FishCookSession` |
| fps | `examples/fps_minimal.py` | `FpsSession` |
| novel | `examples/novel_minimal.py` | `NovelSession` |
| platformer (Box Hop) | `examples/platformer_minimal.py` | `PlatformerSession` |
| puzzle | `examples/puzzle_minimal.py` | `PuzzleSession` |
| puzzle_pad | `examples/puzzle_pad_minimal.py` | `PuzzlePadSession` |
| race | `examples/race_minimal.py` | `RaceSession` |
| rhythm | `examples/rhythm_minimal.py` | `RhythmSession` |
| rpg | `examples/rpg_minimal.py` | `RpgSession` |
| shop | `kagra-shared/tests/fixtures/shop_buy_world.json` | `ShopSession` (`shop_buy_smoke`) |
| sports | `examples/sports_minimal.py` | `SportsSession` |
| stealth | `examples/stealth_minimal.py` | `StealthSession` |
| survival | `examples/survival_minimal.py` | `SurvivalSession` |
| td | `examples/td_minimal.py` | `TdSession` |

Python game master (free `WorldDoc`, logic in Python): `bunny_garden_minimal.py`, `torneko_minimal.py`, `bar_sim_minimal.py`, `flight_minimal.py`, `backyard_brawl_minimal.py`, `chess_minimal.py`, `physics_minimal.py`, `python_game_minimal.py`.

WorldPlay ids with a fixture and no `*_minimal.py` (`python -m kagra.play_world kagra-shared/tests/fixtures/<id>_world.json`):

| id | fixture |
|---|---|
| action_side | `action_side_world.json` |
| sprite_card | `sprite_card_world.json` |
| sim_meter | `sim_meter_world.json` |
| fish_cast | `fish_cast_world.json` |
| rhythm_beat | `rhythm_beat_world.json` |
| cook_stove | `cook_stove_world.json` |
| survival_meter | `survival_meter_world.json` |
| box_hop | `box_hop_world.json` (same loop as platformer) |

`demo_live_smoke` is the archived RendererV2 live demo. It lives at `old/examples/verify_scenarios/demo_live_smoke.json` with `old/examples/demo_live_smoke.py`. The mainline scenario folder does not point at a missing script.

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
| World.dump → 窓 | `python -m kagra.play_world dump.json` / `cargo run -p kagra-shared --features render --example window`（collectathon: タイトル→プレイ→結果。画面無しはスキップ。公式 Crest プレイ。RendererV2 ではない） |
| Wasm ビルド | `cargo build -p kagra-shared --target wasm32-unknown-unknown --features wasm,render` |
| Windows 配布 zip | `python scripts/package_windows.py --game examples/platformer_minimal.py`（exe は Windows。手順は [distribution.md](distribution.md)） |
| Android 配布 APK | `python scripts/package_android.py --game examples/platformer_minimal.py`（デバッグ署名。リリース署名は distribution.md） |

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

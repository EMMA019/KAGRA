# KAGRA Public API Index

このファイルは `tools/gen_api_index.py` により自動生成されます。手編集しないでください。

エントリ数: **54**

棚の**手前**は shared wgpu 30（WorldDoc / WorldPlay / gameloop）。
棚の**奥**は再エクスポートの余り。旧 RendererV2 API は `old/`。

## Front (recommended)

| Name | Signature |
|---|---|
| `add_table` | `export add_table  (from kagra.i18n)` |
| `annotate` | `export annotate  (from kagra.annotate)` |
| `AssetKind` | `class AssetKind  (from kagra.contracts)` |
| `bar` | `export bar  (from kagra.ui2d)` |
| `Brain` | `class Brain  (from kagra.brain)` |
| `brain` | `export brain  (from kagra.brain)` |
| `BrainError` | `class BrainError  (from kagra.brain)` |
| `choice_menu` | `export choice_menu  (from kagra.ui2d)` |
| `debug_trace` | `export debug_trace  (from kagra.trace)` |
| `debug_trace_summary` | `export debug_trace_summary  (from kagra.trace)` |
| `draw_world` | `export draw_world  (from kagra.gameloop)` |
| `eval_world_expect` | `export eval_world_expect  (from kagra.world_expect)` |
| `find_path` | `export find_path  (from kagra.path)` |
| `get_lang` | `export get_lang  (from kagra.i18n)` |
| `image` | `export image  (from kagra.ui2d)` |
| `island_height` | `export island_height  (from kagra.land)` |
| `KagraContractError` | `class KagraContractError  (from kagra.contracts)` |
| `KairiBrain` | `class KairiBrain  (from kagra.brain)` |
| `list_lines` | `export list_lines  (from kagra.ui2d)` |
| `load_data` | `export load_data  (from kagra.save)` |
| `load_scenario` | `export load_scenario  (from kagra.verify)` |
| `merge` | `export merge  (from kagra.ui2d)` |
| `message` | `export message  (from kagra.ui2d)` |
| `mouse_clicked` | `export mouse_clicked  (from kagra.gameloop)` |
| `mouse_down` | `export mouse_down  (from kagra.gameloop)` |
| `mouse_pos` | `export mouse_pos  (from kagra.gameloop)` |
| `move_range` | `export move_range  (from kagra.path)` |
| `open_world_height` | `export open_world_height  (from kagra.land)` |
| `OpenAIBrain` | `class OpenAIBrain  (from kagra.brain)` |
| `overworld_height` | `export overworld_height  (from kagra.land)` |
| `paged_menu` | `export paged_menu  (from kagra.ui2d)` |
| `panel` | `export panel  (from kagra.ui2d)` |
| `play_se` | `export play_se  (from kagra.audio)` |
| `play_wav` | `export play_wav  (from kagra.audio)` |
| `pressed` | `export pressed  (from kagra.gameloop)` |
| `render_world_doc` | `export render_world_doc  (from kagra.kagra_shared)` |
| `resolve_asset` | `export resolve_asset  (from kagra.contracts)` |
| `rgba_to_png` | `export rgba_to_png  (from kagra.gameloop)` |
| `run` | `export run  (from kagra.gameloop)` |
| `run_scenario` | `export run_scenario  (from kagra.verify)` |
| `run_scenario_path` | `export run_scenario_path  (from kagra.verify)` |
| `save_data` | `export save_data  (from kagra.save)` |
| `Scene` | `class Scene  (from kagra.gameloop)` |
| `scroll_window` | `export scroll_window  (from kagra.ui2d)` |
| `se` | `export se  (from kagra.audio)` |
| `set_lang` | `export set_lang  (from kagra.i18n)` |
| `set_listener` | `export set_listener  (from kagra.audio)` |
| `SlotStore` | `class SlotStore  (from kagra.save)` |
| `sound` | `export sound  (from kagra.audio)` |
| `t` | `export t  (from kagra.i18n)` |
| `tone` | `export tone  (from kagra.audio)` |
| `was_pressed` | `export was_pressed  (from kagra.gameloop)` |
| `WorldDoc` | `class WorldDoc  (from kagra.kagra_shared)` |
| `WorldPlay` | `class WorldPlay  (from kagra.kagra_shared)` |

## Shelf (legacy 2D / tilemap / editor / ECS)

| Name | Signature |
|---|---|

## Agent notes

- 存在しない API を呼ばないこと。ここに無い名前は未公開か内部用です。
- 新しいゲームは Front だけ。世界は `WorldDoc` JSON、プレイは `WorldPlay` / `kagra.gameloop`。`Walk` / `Prop` / `World3D` / `kagra_core` は `old/`（`import kagra` に出ない）。
- ジャンルは `WorldDoc.genre`（`crest_isle` / `town_gate` / `fish_cast` / …）。無指定は自由世界。prop 名（`door` / `dock` / `stove`）は景色でありジャンルを起動しない。
- テクスチャは `WorldProp.texture`（PNG パス）。HUD 画像は `kagra.ui2d.image` → `draw_world(..., hud=)`。
- dump JSON の共有オフスクリーン: `python -m kagra.render_world dump.json out.png`（または `kagra.verify` の `expect_offscreen`）。wgpu 30。アダプタ無しはスキップ。
- dump JSON の共有デスクトップ窓: `python -m kagra.play_world dump.json`。Crest collectathon は `genre: crest_isle`。新しいゲームは RendererV2 で始めない。
- 参照ゲーム: `examples/bunny_garden_minimal.py` / `examples/torneko_minimal.py` / `examples/bar_sim_minimal.py`。
- セーブは `save_data` / `load_data` / `SlotStore`。検証は `eval_world_expect` / `kagra.verify`。
- 音は `tone` / `se` / `play_wav` / `play_se`。3D は `set_listener` + `play_se(..., x=, y=, z=)`。
- エージェントの目: `kagra.annotate(sx, sy)` はプレビュークリックを JSONL に残す。`kagra.debug_trace` は接地浮き。
- 頭脳: `kagra.brain("kairi"|"ollama"|"openai")`。既定は `https://kairi.onrender.com`（`KAIRI_API_TOKEN`）。wheel にモデルは入れない。
- Rust バインディングの整合は `tests/test_api_bindings.py` も参照。
- 再生成: `python tools/gen_api_index.py`

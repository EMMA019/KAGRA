# KAGRA Public API Index

このファイルは `tools/gen_api_index.py` により自動生成されます。手編集しないでください。

エントリ数: **93**

棚の**手前**は shared wgpu 30（WorldDoc / WorldPlay / gameloop）。
棚の**奥**は再エクスポートの余り。旧 RendererV2 API は `old/`。

## Front (recommended)

| Name | Signature |
|---|---|
| `ActionSession` | `class ActionSession  (from kagra.kagra_shared)` |
| `add_table` | `export add_table  (from kagra.i18n)` |
| `annotate` | `export annotate  (from kagra.annotate)` |
| `apply_look` | `export apply_look  (from kagra.juice)` |
| `apply_post` | `export apply_post  (from kagra.juice)` |
| `AssetKind` | `class AssetKind  (from kagra.contracts)` |
| `bar` | `export bar  (from kagra.ui2d)` |
| `Brain` | `class Brain  (from kagra.brain)` |
| `brain` | `export brain  (from kagra.brain)` |
| `BrainError` | `class BrainError  (from kagra.brain)` |
| `CardSession` | `class CardSession  (from kagra.kagra_shared)` |
| `choice_menu` | `export choice_menu  (from kagra.ui2d)` |
| `debug_trace` | `export debug_trace  (from kagra.trace)` |
| `debug_trace_summary` | `export debug_trace_summary  (from kagra.trace)` |
| `draw_world` | `export draw_world  (from kagra.gameloop)` |
| `ease` | `export ease  (from kagra.juice)` |
| `Emitter` | `class Emitter  (from kagra.particles)` |
| `eval_world_expect` | `export eval_world_expect  (from kagra.world_expect)` |
| `FightSession` | `class FightSession  (from kagra.kagra_shared)` |
| `find_path` | `export find_path  (from kagra.path)` |
| `FishCookSession` | `class FishCookSession  (from kagra.kagra_shared)` |
| `FpsSession` | `class FpsSession  (from kagra.kagra_shared)` |
| `get_lang` | `export get_lang  (from kagra.i18n)` |
| `image` | `export image  (from kagra.ui2d)` |
| `island_height` | `export island_height  (from kagra.land)` |
| `Juice` | `class Juice  (from kagra.juice)` |
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
| `NeonCat` | `class NeonCat  (from kagra.kagra_shared)` |
| `NeonRun` | `class NeonRun  (from kagra.kagra_shared)` |
| `NovelSession` | `class NovelSession  (from kagra.kagra_shared)` |
| `open_world_height` | `export open_world_height  (from kagra.land)` |
| `OpenAIBrain` | `class OpenAIBrain  (from kagra.brain)` |
| `overworld_height` | `export overworld_height  (from kagra.land)` |
| `paged_menu` | `export paged_menu  (from kagra.ui2d)` |
| `panel` | `export panel  (from kagra.ui2d)` |
| `PlatformerSession` | `class PlatformerSession  (from kagra.kagra_shared)` |
| `play_se` | `export play_se  (from kagra.audio)` |
| `play_wav` | `export play_wav  (from kagra.audio)` |
| `pressed` | `export pressed  (from kagra.gameloop)` |
| `PuzzlePadSession` | `class PuzzlePadSession  (from kagra.kagra_shared)` |
| `PuzzleSession` | `class PuzzleSession  (from kagra.kagra_shared)` |
| `RaceSession` | `class RaceSession  (from kagra.kagra_shared)` |
| `render_rgba` | `export render_rgba  (from kagra.gameloop)` |
| `render_world_doc` | `export render_world_doc  (from kagra.kagra_shared)` |
| `resolve_asset` | `export resolve_asset  (from kagra.contracts)` |
| `rgba_to_png` | `export rgba_to_png  (from kagra.gameloop)` |
| `rgba_to_ppm` | `export rgba_to_ppm  (from kagra.gameloop)` |
| `RhythmSession` | `class RhythmSession  (from kagra.kagra_shared)` |
| `RpgSession` | `class RpgSession  (from kagra.kagra_shared)` |
| `run` | `export run  (from kagra.gameloop)` |
| `run_scenario` | `export run_scenario  (from kagra.verify)` |
| `run_scenario_path` | `export run_scenario_path  (from kagra.verify)` |
| `save_data` | `export save_data  (from kagra.save)` |
| `Scene` | `class Scene  (from kagra.gameloop)` |
| `scroll_window` | `export scroll_window  (from kagra.ui2d)` |
| `se` | `export se  (from kagra.audio)` |
| `set_lang` | `export set_lang  (from kagra.i18n)` |
| `set_listener` | `export set_listener  (from kagra.audio)` |
| `ShopSession` | `class ShopSession  (from kagra.kagra_shared)` |
| `SlotStore` | `class SlotStore  (from kagra.save)` |
| `sound` | `export sound  (from kagra.audio)` |
| `SportsSession` | `class SportsSession  (from kagra.kagra_shared)` |
| `StealthSession` | `class StealthSession  (from kagra.kagra_shared)` |
| `SurvivalSession` | `class SurvivalSession  (from kagra.kagra_shared)` |
| `t` | `export t  (from kagra.i18n)` |
| `TdSession` | `class TdSession  (from kagra.kagra_shared)` |
| `tone` | `export tone  (from kagra.audio)` |
| `Trail` | `class Trail  (from kagra.particles)` |
| `was_pressed` | `export was_pressed  (from kagra.gameloop)` |
| `WorldDoc` | `class WorldDoc  (from kagra.kagra_shared)` |
| `WorldPlay` | `class WorldPlay  (from kagra.kagra_shared)` |

## Shelf (legacy 2D / tilemap / editor / ECS)

| Name | Signature |
|---|---|
| `annotate_dump` | `export annotate_dump  (from kagra.axes)` |
| `mixer_state` | `export mixer_state  (from kagra.audio)` |
| `play_bgm` | `export play_bgm  (from kagra.audio)` |
| `play_sfx` | `export play_sfx  (from kagra.audio)` |
| `save_dir` | `export save_dir  (from kagra.flow)` |
| `screen_basis` | `export screen_basis  (from kagra.axes)` |
| `screen_left` | `export screen_left  (from kagra.axes)` |
| `screen_right` | `export screen_right  (from kagra.axes)` |
| `set_group_volume` | `export set_group_volume  (from kagra.audio)` |
| `set_mute` | `export set_mute  (from kagra.audio)` |
| `stop_bgm` | `export stop_bgm  (from kagra.audio)` |
| `wish_world` | `export wish_world  (from kagra.axes)` |

## Agent notes

- 存在しない API を呼ばないこと。ここに無い名前は未公開か内部用です。
- 新しいゲームは Front だけ。世界は `WorldDoc` JSON、プレイは `WorldPlay` / `kagra.gameloop`。`Walk` / `Prop` / `World3D` / `kagra_core` は `old/`（`import kagra` に出ない）。
- ジャンルは `WorldDoc.genre`（`crest_isle` / `town_gate` / `fish_cast` / `fishcook` / …）。無指定は自由世界。prop 名（`door` / `dock` / `stove`）は景色でありジャンルを起動しない。
- テクスチャは `WorldProp.texture`（PNG パス）+ `uv_scale`（CPU タイル）。室内は dump `sky` / `ground: false`。HUD 画像は `kagra.ui2d.image` → `draw_world(..., hud=)`。
- プロペラや機体は `pitch` / `roll` と `local: true`（親の空間、4 段まで）。`local` 無しの `parent` はリンクだけで position はワールドのまま。リングは `model: "ring"`（穴は +Z）。窓の毎フレームは `render_rgba` → `Scene.frame()`（中身は `_frame_rgba` の RGBA）。見本は `examples/flight_minimal.py`。
- 絵の検証は `expect_offscreen.pixels`（領域の平均色 / not_solid）。golden PNG は使わない。新ジャンルを `WorldPlay` に足さない。
- 電話の絵: dump `bloom` / `bloom_threshold` / `vignette` / `lut`（`off` `warm` `punch` `cool`）/ `lut_strength` / `fxaa` / `ssao`（半径px、省略はオフ）/ `taa` / `dof_focus` / `dof_aperture` / `motion_blur`。省略すると旧来の絵（bloom 0、vignette 1、LUT なし、FXAA あり、SSAO/TAA/DOF/モーションなし）。`KAGRA_QUALITY=0` / `off` / `low` は重いパスを切る。`high` / `2` は未指定の SSAO（半径 8）と TAA を入れる。DOF とモーションは high でも自動では入らない。dump `quality`（`low` / `mobile` / `high`、別名 `0` / `off` / `2`）を書くとその段が env の high より勝つ。省略は今までどおり。`mobile` は未指定を SSAO 半径 4、TAA オフ（FXAA）、カスケード 2、PCSS オフ、bloom 0.22 で埋める。`high` は未指定を SSAO 8、TAA、カスケード 3、PCSS、bloom 0.35 で埋める。明示したフィールドが勝つ。env の high だけでは bloom は付かない。`quality=low` は重いパスを切る。8bit スワップチェーンは 3D があるとき常にグレードと FXAA（またはコピー）を通し、輝度とエミッシブマスクのミップブルームを足す。`Juice` は shake / hitstop / squash / tween。`Emitter` は CPU の火花を sphere prop にする（`emissive` で光らせられる）。`Trail` はエミッシブな quad のリボン。`apply_post(world, "mobile")` が電話プリセット、`"neon"` はブルーム強め。`apply_look(world, "night"|"pastel")` が空・ライト・露出・半球・LUT をまとめる。`cull: true` と `prop_lod` は遠い prop を落とす／看板にする（省略時は全部フルメッシュ）。prop `emissive` / `emissive_color` / `emissive_map`、`hemi_sky` / `hemi_ground`、`env_map`（`preset:studio` か PNG）は共有レンダラの絵。`shadow_cascades` は 1 枚の 2048 マップ（省略時）。2 か 3 は同じマップをタイルに分ける。`shadow_pcss` は省略時オフ。`KAGRA_QUALITY=high` または `2` は省略時に 3 段と PCSS。`0` / `off` / `low` は 1 段のまま。`WorldDoc::to_json_with_profile` が draws / triangles / texture bytes と電話予算の warning を足す（`to_json` 自体は変えない）。デスクトップの window 例は dump JSON とマテリアルファイルを `notify` で見ている。
- dump JSON の共有オフスクリーン: `python -m kagra.render_world dump.json out.png`（または `kagra.verify` の `expect_offscreen`）。wgpu 30。アダプタ無しはスキップ。
- dump JSON の共有デスクトップ窓: `python -m kagra.play_world dump.json`。Crest collectathon は `genre: crest_isle`。新しいゲームは RendererV2 で始めない。
- 参照ゲーム: `examples/bunny_garden_minimal.py` / `examples/torneko_minimal.py` / `examples/bar_sim_minimal.py` / `examples/flight_minimal.py` / `examples/backyard_brawl_minimal.py` / `examples/chess_minimal.py`。
- セーブは `save_data` / `load_data` / `SlotStore`。検証は `eval_world_expect` / `kagra.verify`。
- 音は `tone` / `se` / `play_wav` / `play_se`。3D は `set_listener` + `play_se(..., x=, y=, z=)`。
- エージェントの目: `kagra.annotate(sx, sy)` はプレビュークリックを JSONL に残す。`kagra.debug_trace` は接地浮き。
- 頭脳: `kagra.brain("kairi"|"ollama"|"openai")`。既定は `https://kairi.onrender.com`（`KAIRI_API_TOKEN`）。wheel にモデルは入れない。
- Rust バインディングの整合は `tests/test_api_bindings.py` も参照。
- 再生成: `python tools/gen_api_index.py`

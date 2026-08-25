# KAGRA Public API Index

このファイルは `tools/gen_api_index.py` により自動生成されます。手編集しないでください。

エントリ数: **422**

棚の**手前**は VRM / 3D ワールド / エージェントゲーム。
棚の**奥**はレガシー 2D・タイルマップ・ECS・エディタ。推奨しない。

## Front (recommended)

| Name | Signature |
|---|---|
| `annotate` | `annotate(sx: float \| None = None, sy: float \| None = None, *, cam=None, avatar=None, world=None, screenshot: str \| None = None, note: str \| None = None, path: str \| None = None, capture: bool = False, persist: bool = True)` |
| `apply_live_look` | `apply_live_look(*, mascot: bool = False)` |
| `apply_outdoor_look` | `apply_outdoor_look()` |
| `apply_room_look` | `apply_room_look()` |
| `avatar` | `avatar(vrm_path: str) -> 'VrmAvatar'` |
| `billboard_mesh` | `billboard_mesh(x: float, y: float, z: float, size: float, camera=None, *, yaw: float \| None = None)` |
| `box_mesh` | `box_mesh(cx: float, cy: float, cz: float, w: float, h: float, d: float)` |
| `camera_world_to_screen` | `camera_world_to_screen(wx: float, wy: float, wz: float)` |
| `can_pick` | `can_pick(px: float, pz: float, x: float, z: float, *, reach: float = 1.2) -> bool` |
| `city_boxes` | `city_boxes(ix: int, iz: int, *, tile: float = 10.0, fn=None, water_y: float = 0.0)` |
| `city_chunk` | `city_chunk(city, ix: int, iz: int, *, tile: float \| None = None)` |
| `clicked_prop` | `clicked_prop(cam=None, *, button: int = 1, max_dist: float = 80.0)` |
| `cls` | `cls(r=0, g=0, b=0)` |
| `cylinder_mesh` | `cylinder_mesh(cx: float = 0.0, cy: float = 0.0, cz: float = 0.0, radius: float = 0.5, height: float = 1.0, segs: int = 16)` |
| `debug_trace` | `debug_trace(*, foot_y: float, x: float = 0.0, z: float = 0.0, ground_y: float \| None = None, height_fn=None, world=None, vx: float \| None = None, vz: float \| None = None, on_ground: bool \| None = None, camera_distance: float \| None = None, threshold: float = 0.05, frame: int \| None = None, path: str \| None = None, persist: bool = True, reset: bool = False)` |
| `debug_trace_summary` | `debug_trace_summary() -> str` |
| `destroy` | `destroy(prop) -> None` |
| `disk_mesh` | `disk_mesh(cx: float, cy: float, cz: float, radius: float, segs: int = 48)` |
| `down` | `down(name: str) -> bool` |
| `draw_billboard` | `draw_billboard(tex: int, x: float, y: float, z: float, size: float, camera=None, *, yaw: float \| None = None)` |
| `draw_billboard_instances` | `draw_billboard_instances(tex: int, items, camera=None, *, yaw: float \| None = None)` |
| `draw_mesh_3d` | `draw_mesh_3d(texture_id: int, verts: list, indices: list)` |
| `draw_mesh_id` | `draw_mesh_id(mesh_id: int)` |
| `draw_mesh_instances` | `draw_mesh_instances(mesh_id: int, instances: list)` |
| `draw_vignette` | `draw_vignette(sw: int \| None = None, sh: int \| None = None, strength: float = 0.42)` |
| `draw_vrm` | `draw_vrm(vrm_id: int)` |
| `fill` | `fill(x: float, y: float, w: float, h: float, color=(255, 255, 255), alpha: int = 255)` |
| `font` | `font(path: str = None) -> int` |
| `get_camera3d` | `get_camera3d() -> Camera3D \| None` |
| `get_engine` | `get_engine() -> _Engine` |
| `get_screen_size` | `get_screen_size() -> tuple` |
| `height_normal` | `height_normal(fn, x: float, z: float, eps: float = 0.12)` |
| `heightfield_mesh` | `heightfield_mesh(fn, half: float = 16.0, cells: int = 32, *, origin_x: float = 0.0, origin_z: float = 0.0, uv_half: float \| None = None)` |
| `heightfield_tile` | `heightfield_tile(fn, origin_x: float, origin_z: float, tile: float = 10.0, cells: int = 8, *, uv_half: float \| None = None)` |
| `hovered_prop` | `hovered_prop(cam=None, sx: float \| None = None, sy: float \| None = None, *, max_dist: float = 80.0)` |
| `init` | `init(width=1280, height=720, title='KAGRA Game', fps=60, transparent=False, decorations=True, always_on_top=False, visible=True)` |
| `inject_key` | `inject_key(name: str, down: bool = True)` |
| `island_height` | `island_height(x: float, z: float) -> float` |
| `key` | `key(name: str) -> bool` |
| `load_city` | `load_city(path)` |
| `load_json` | `load_json(name: str, default=None, *, directory: str \| None = None)` |
| `load_vrma` | `load_vrma(path: str, *, sample_fps: float = 30.0) -> 'VrmaMotion'` |
| `mouse_delta` | `mouse_delta() -> tuple` |
| `open_world_height` | `open_world_height(x: float, z: float) -> float` |
| `overworld_height` | `overworld_height(x: float, z: float) -> float` |
| `pressed` | `pressed(name: str) -> bool` |
| `quad_y_mesh` | `quad_y_mesh(cx: float = 0.0, cy: float = 0.0, cz: float = 0.0, size: float = 0.5)` |
| `quit` | `quit()` |
| `ramp_mesh` | `ramp_mesh(x0: float, x1: float, z0: float, z1: float, y0: float, y1: float)` |
| `released` | `released(name: str) -> bool` |
| `render_stats` | `render_stats() -> dict` |
| `room` | `room(half: float = 6.0, height: float = 3.2, *, thick: float = 0.18, world=None, look: bool = True, textured: bool = True)` |
| `run` | `run(update=None, draw=None, start_scene: Scene = None, max_frames=None, fixed_dt=None, on_ready=None)` |
| `save_json` | `save_json(name: str, data: dict, *, directory: str \| None = None)` |
| `screenshot` | `screenshot(path: str)` |
| `se` | `se(path: str, vol: float = 1.0) -> None` |
| `set_ambient` | `set_ambient(r: float = 0.22, g: float = 0.2, b: float = 0.28, strength: float = 0.28)` |
| `set_bloom` | `set_bloom(threshold: float = 0.85, intensity: float = 0.35, enabled: bool = True)` |
| `set_camera3d` | `set_camera3d(cam: Camera3D \| None)` |
| `set_cursor_locked` | `set_cursor_locked(locked: bool = True)` |
| `set_exposure` | `set_exposure(value: float = 1.0)` |
| `set_fog` | `set_fog(start: float = 5.0, end: float = 20.0, color: tuple = (110, 180, 230), *, enabled: bool = True)` |
| `set_hdri` | `set_hdri(path: str \| None = 'studio', strength: float = 1.0)` |
| `set_light_dir` | `set_light_dir(x: float, y: float, z: float)` |
| `set_mesh_cull` | `set_mesh_cull(enabled: bool = True)` |
| `set_mesh_normal` | `set_mesh_normal(mesh_id: int, texture_id: int = 0)` |
| `set_mesh_pbr` | `set_mesh_pbr(mesh_id: int, metallic: float = 0.0, roughness: float = 1.0, base_color: tuple = (1.0, 1.0, 1.0))` |
| `set_point_light` | `set_point_light(x: float, y: float, z: float, *, r: float = 1.0, g: float = 0.95, b: float = 0.85, intensity: float = 1.0, radius: float = 8.0, slot: int = 0)` |
| `set_rim` | `set_rim(intensity: float = 0.45)` |
| `set_shadow_cascades` | `set_shadow_cascades(count: int = 1)` |
| `set_shadow_enabled` | `set_shadow_enabled(enabled: bool = True)` |
| `set_spot_light` | `set_spot_light(x: float, y: float, z: float, dx: float, dy: float, dz: float, *, angle: float = 0.8, penumbra: float = 0.25, intensity: float = 1.0, radius: float = 10.0, r: float = 1.0, g: float = 0.95, b: float = 0.85, slot: int = 0)` |
| `set_tonemap` | `set_tonemap(enabled: bool = True)` |
| `set_toon_params` | `set_toon_params(threshold: float = 0.5, softness: float = 1.0, shade: float = 0.55, lit: float = 1.0)` |
| `sky` | `sky(*, radius: float = 18.0, look: bool = True)` |
| `solid_tex` | `solid_tex(color)` |
| `sound` | `sound(name: str = 'coin', freqs=None, duration: float = 0.1, volume: float = 0.32) -> str` |
| `sphere_mesh` | `sphere_mesh(cx: float = 0.0, cy: float = 0.0, cz: float = 0.0, radius: float = 0.5, segs: int = 16)` |
| `stage` | `stage(path: str = 'stage', *, radius: float = 12.0) -> 'Stage'` |
| `stair_y` | `stair_y(x: float, z: float, *, x0: float, x1: float, z0: float, z1: float, y0: float, y1: float, steps: int = 6, axis: str = 'z')` |
| `text` | `text(s, x: float, y: float, size: int = 24, color=(255, 255, 255), font: int = None, alpha: int = 255)` |
| `texture_from_fn` | `texture_from_fn(width: int, height: int, pixel_fn, *, name: str \| None = None, srgb: bool = True) -> int` |
| `tick_count` | `tick_count() -> int` |
| `tile_keys` | `tile_keys(x: float, z: float, *, tile: float = 10.0, radius: float = 28.0, half: float \| None = None)` |
| `tone` | `tone(name: str, freqs, duration: float = 0.12, volume: float = 0.35, decay: bool = True) -> str` |
| `unload_mesh_3d` | `unload_mesh_3d(mesh_id: int)` |
| `upload_mesh_3d` | `upload_mesh_3d(texture_id: int, verts: list, indices: list, *, metallic: float = 0.0, roughness: float = 1.0, base_color: tuple = (1.0, 1.0, 1.0), normal_texture_id: int = 0) -> int` |
| `water` | `water(y: float = 0.0, *, half: float = 24.0, world=None)` |
| `AABB` | `class AABB  (from kagra.physics3d)` |
| `ActionController` | `class ActionController  (from kagra.vrm_action)` |
| `AiCharacter` | `class AiCharacter  (from kagra.ai_character)` |
| `animate` | `export animate  (from kagra.motion)` |
| `apply_pad` | `export apply_pad  (from kagra.touch)` |
| `axis` | `export axis  (from kagra.pad)` |
| `Brain` | `class Brain  (from kagra.brain)` |
| `brain` | `export brain  (from kagra.brain)` |
| `BrainError` | `class BrainError  (from kagra.brain)` |
| `Button` | `class Button  (from kagra.hud)` |
| `Camera3D` | `class Camera3D  (from kagra.camera3d)` |
| `ChatInbox` | `class ChatInbox  (from kagra.stream)` |
| `DebugTrace` | `class DebugTrace  (from kagra.trace)` |
| `describe_environment` | `export describe_environment  (from kagra.contracts)` |
| `EmotionController` | `class EmotionController  (from kagra.vrm_emotion)` |
| `ensure_vrm` | `export ensure_vrm  (from kagra.samples)` |
| `inject_pad` | `export inject_pad  (from kagra.pad)` |
| `KairiBrain` | `class KairiBrain  (from kagra.brain)` |
| `Label` | `class Label  (from kagra.hud)` |
| `LipSyncController` | `class LipSyncController  (from kagra.vrm_lipsync)` |
| `load_scenario` | `export load_scenario  (from kagra.verify)` |
| `LookAtController` | `class LookAtController  (from kagra.vrm_lookat)` |
| `MicLipsync` | `class MicLipsync  (from kagra.mic)` |
| `OpenAIBrain` | `class OpenAIBrain  (from kagra.brain)` |
| `pad` | `export pad  (from kagra.pad)` |
| `pad_pressed` | `export pad_pressed  (from kagra.pad)` |
| `Physics3D` | `class Physics3D  (from kagra.physics3d)` |
| `PointerEvent` | `class PointerEvent  (from kagra.touch)` |
| `poll_pad` | `export poll_pad  (from kagra.pad)` |
| `Prop` | `class Prop  (from kagra.play)` |
| `resolve_asset` | `export resolve_asset  (from kagra.contracts)` |
| `RigidBody3D` | `class RigidBody3D  (from kagra.physics3d)` |
| `run_scenario` | `export run_scenario  (from kagra.verify)` |
| `run_scenario_path` | `export run_scenario_path  (from kagra.verify)` |
| `Scene` | `class Scene` |
| `sequence` | `export sequence  (from kagra.motion)` |
| `Sequence` | `class Sequence  (from kagra.motion)` |
| `Stage` | `class Stage  (from kagra.stage)` |
| `StreamHud` | `class StreamHud  (from kagra.stream)` |
| `Tween` | `class Tween  (from kagra.motion)` |
| `VirtualCam` | `class VirtualCam  (from kagra.stream)` |
| `VirtualPad` | `class VirtualPad  (from kagra.touch)` |
| `Walk` | `class Walk  (from kagra.play)` |
| `World3D` | `class World3D  (from kagra.world3d)` |

## Shelf (legacy 2D / tilemap / editor / ECS)

| Name | Signature |
|---|---|
| `after` | `after(frames: int, from_tick: int = 0) -> bool` |
| `angle_to` | `angle_to(x1: float, y1: float, x2: float, y2: float) -> float` |
| `backspace_pressed` | `backspace_pressed()` |
| `bar` | `bar(x: float, y: float, w: float, h: float, value: float, max_value: float = 100, *, bg=(25, 25, 35), fill=(50, 220, 80))` |
| `bgm` | `bgm(path: str, loop: bool = True, vol: float = 0.8) -> None` |
| `button` | `button(x: float, y: float, w: float, h: float, label: str = '', *, bg=(70, 70, 90), hover=(100, 100, 150), color=(255, 255, 255), size: int = 20, font: int = None) -> bool` |
| `camera_ray_from_screen` | `camera_ray_from_screen(sx: float, sy: float)` |
| `camera_update` | `camera_update(dt: float)` |
| `circle` | `circle(x: float, y: float, radius: float, r: int = 255, g: int = 255, b: int = 255, a: int = 255, segments: int = 24)` |
| `circle_fill` | `circle_fill(x: float, y: float, radius: float, color=(255, 255, 255), alpha: int = 255)` |
| `circle_outline` | `circle_outline(x: float, y: float, radius: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `clamp` | `clamp(value: float, lo: float, hi: float) -> float` |
| `collide_rect` | `collide_rect(ax, ay, aw, ah, bx, by, bw, bh)` |
| `collide_rect_overlap` | `collide_rect_overlap(ax, ay, aw, ah, bx, by, bw, bh)` |
| `create_boid_system` | `create_boid_system(count: int, width: float = 1280.0, height: float = 720.0) -> int` |
| `create_boid_system_gpu` | `create_boid_system_gpu(count: int, width: float = 1280.0, height: float = 720.0) -> int` |
| `distance` | `distance(x1: float, y1: float, x2: float, y2: float) -> float` |
| `distance_sq` | `distance_sq(x1: float, y1: float, x2: float, y2: float) -> float` |
| `drag_window` | `drag_window()` |
| `draw_boids` | `draw_boids(boid_id: int, batch_id: int, sprite_w: float = 6.0, sprite_h: float = 3.0)` |
| `draw_boids_gpu` | `draw_boids_gpu(boid_id: int)` |
| `draw_gltf` | `draw_gltf(model_id: int)` |
| `draw_mesh` | `draw_mesh(texture_id: int, verts: list, shader_id: int = 0, shader_params: list = None)` |
| `draw_polygon` | `draw_polygon(verts: list, r=255, g=255, b=255, a=255, color=None)` |
| `draw_rig` | `draw_rig(rig_id: int, x, y)` |
| `draw_text` | `draw_text(font_id, text_str, x, y, size=24, r=255, g=255, b=255, a=255, color=None)` |
| `draw_texture` | `draw_texture(tid, x, y, w=None, h=None, sx=0.0, sy=0.0, sw=None, sh=None, alpha=1.0, rotation_deg=0.0, pivot_x=0.5, pivot_y=0.5, flip_x=True, flip_y=False, shader_id=0, shader_params=None)` |
| `draw_texture_glow` | `draw_texture_glow(tid, x, y, w=None, h=None, r=1.0, g=0.8, b=1.0, intensity=1.0, alpha=1.0, rotation_deg=0.0, flip_x=False)` |
| `draw_texture_spotlight` | `draw_texture_spotlight(tid, x, y, w=None, h=None, spot_x=0.5, spot_y=0.5, radius=0.4, intensity=1.0, alpha=1.0, rotation_deg=0.0, flip_x=False)` |
| `draw_texture_tint` | `draw_texture_tint(tid, x, y, w=None, h=None, r=1.0, g=1.0, b=1.0, intensity=1.0, alpha=1.0, rotation_deg=0.0, flip_x=False)` |
| `draw_texture_world` | `draw_texture_world(tid, wx, wy, w=None, h=None, sx=0.0, sy=0.0, sw=None, sh=None, alpha=1.0, rotation_deg=0.0, pivot_x=0.5, pivot_y=0.5, flip_x=False, flip_y=False, shader_id=0, shader_params=None)` |
| `draw_ui_button` | `draw_ui_button(x, y, w, h, text, bg_r=70, bg_g=70, bg_b=90, hv_r=100, hv_g=100, hv_b=150, txt_r=255, txt_g=255, txt_b=255, font_size=20, bg_color=None, hover_color=None, text_color=None, font_id=1) -> bool` |
| `draw_ui_progress_bar` | `draw_ui_progress_bar(x, y, w, h, max_val, current_val, bg_r=30, bg_g=30, bg_b=30, fl_r=50, fl_g=255, fl_b=50, bg_color=None, fill_color=None)` |
| `emit` | `emit(event, data=None, deferred=False)` |
| `enter_pressed` | `enter_pressed()` |
| `escape_pressed` | `escape_pressed()` |
| `every` | `every(frames: int) -> bool` |
| `flush_events` | `flush_events()` |
| `focus_window` | `focus_window()` |
| `frame_count` | `frame_count() -> int` |
| `frame_index` | `frame_index(count: int, hold_for: int = 4, repeat: bool = True, offset: int = 0) -> int` |
| `get_camera` | `get_camera() -> Camera \| None` |
| `get_fps` | `get_fps() -> float` |
| `get_preedit_text` | `get_preedit_text() -> str` |
| `get_typed_chars` | `get_typed_chars() -> str` |
| `get_vrm_look_at` | `get_vrm_look_at(vrm_id: int) -> dict \| None` |
| `go` | `go(next_scene: Scene) -> None` |
| `grab_frame` | `grab_frame()` |
| `has_vrm_bone` | `has_vrm_bone(vrm_id: int, name: str) -> bool` |
| `image` | `image(tex: int, x: float, y: float, w: float = None, h: float = None, *, alpha: float = 1.0, rotation: float = 0.0, flip_x: bool = False, flip_y: bool = False, sx: float = 0.0, sy: float = 0.0, sw: float = None, sh: float = None)` |
| `image_world` | `image_world(tex: int, wx: float, wy: float, w: float, h: float, *, alpha: float = 1.0, flip_x: bool = False, flip_y: bool = False)` |
| `inject_mouse` | `inject_mouse(x=None, y=None, button=None, down=None)` |
| `inside_circle` | `inside_circle(px: float, py: float, cx: float, cy: float, radius: float) -> bool` |
| `inside_rect` | `inside_rect(px: float, py: float, rx: float, ry: float, rw: float, rh: float) -> bool` |
| `intersect_circle_rect` | `intersect_circle_rect(cx: float, cy: float, cr: float, rx: float, ry: float, rw: float, rh: float) -> bool` |
| `intersect_rect` | `intersect_rect(ax: float, ay: float, aw: float, ah: float, bx: float, by: float, bw: float, bh: float) -> bool` |
| `key_down` | `key_down(code: int) -> bool` |
| `key_pressed` | `key_pressed(code: int) -> bool` |
| `key_released` | `key_released(code: int) -> bool` |
| `lerp` | `lerp(a: float, b: float, t: float) -> float` |
| `line` | `line(x1: float, y1: float, x2: float, y2: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `line_h` | `line_h(x: float, y: float, length: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `line_v` | `line_v(x: float, y: float, length: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `list_blend_shapes` | `list_blend_shapes(vrm_id: int) -> list[str]` |
| `list_human_bones` | `list_human_bones(vrm_id: int) -> list[str]` |
| `load` | `load(path: str, *, srgb: bool = True) -> int` |
| `load_bvh` | `load_bvh(path: str, extra_map: dict = None) -> 'BvhMotion'` |
| `load_data` | `load_data(key: str, force_reload=False) -> 'DataObject'` |
| `load_fbx` | `load_fbx(path: str, clip_name: str = None) -> 'FbxMotion'` |
| `load_font` | `load_font(path: str) -> int` |
| `load_gltf` | `load_gltf(path: str) -> int` |
| `load_rig` | `load_rig(path: str) -> int` |
| `load_shader` | `load_shader(path: str) -> int` |
| `load_shader_src` | `load_shader_src(wgsl_src: str) -> int` |
| `load_texture` | `load_texture(path: str, *, srgb: bool = True) -> int` |
| `load_vrm` | `load_vrm(path: str) -> int` |
| `measure` | `measure(s, size: int = 24, font: int = None) -> tuple` |
| `measure_text` | `measure_text(font_id, text_str, size=24) -> tuple` |
| `mouse` | `mouse() -> tuple` |
| `mouse_btn` | `mouse_btn(button_id: int = 1) -> bool` |
| `mouse_click` | `mouse_click(button_id: int = 1) -> bool` |
| `mouse_down` | `mouse_down(btn: int) -> bool` |
| `mouse_pos` | `mouse_pos() -> tuple` |
| `mouse_pressed` | `mouse_pressed(btn: int) -> bool` |
| `mouse_released` | `mouse_released(btn: int) -> bool` |
| `mouse_wheel` | `mouse_wheel() -> tuple` |
| `off` | `off(event, callback)` |
| `off_all` | `off_all(event)` |
| `on` | `on(event, callback, priority=0, once=False)` |
| `once` | `once(event, callback, priority=0)` |
| `pick_vrm_bone` | `pick_vrm_bone(vrm_id: int, ox: float, oy: float, oz: float, dx: float, dy: float, dz: float, max_dist: float = 100.0)` |
| `play_bgm` | `play_bgm(path: str, loop_=True, volume=0.8)` |
| `play_se` | `play_se(path: str, volume=1.0)` |
| `point_in_rect` | `point_in_rect(px, py, rx, ry, rw, rh)` |
| `polygon` | `polygon(pts: list, color=(255, 255, 255), alpha: int = 255)` |
| `polygon_outline` | `polygon_outline(pts: list, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `pop` | `pop() -> None` |
| `preload_data` | `preload_data(subdir='', recursive=True) -> list` |
| `push` | `push(next_scene: Scene) -> None` |
| `rect` | `rect(x, y, w, h, color=255, g=None, b=None, a=255)` |
| `rect_world` | `rect_world(wx, wy, w, h, r, g, b, a=255)` |
| `reset_blend_shapes` | `reset_blend_shapes(vrm_id: int)` |
| `reset_vrm_pose` | `reset_vrm_pose(vrm_id: int)` |
| `reset_vrm_spring` | `reset_vrm_spring(vrm_id: int)` |
| `resolve_vrm_bone` | `resolve_vrm_bone(vrm_id: int, name: str) -> int \| None` |
| `rounded_rect` | `rounded_rect(x: float, y: float, w: float, h: float, radius: float = 8, color=(255, 255, 255), alpha: int = 255)` |
| `rounded_rect_outline` | `rounded_rect_outline(x: float, y: float, w: float, h: float, radius: float = 8, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `screen_h` | `screen_h() -> int` |
| `screen_to_world` | `screen_to_world(sx: float, sy: float) -> tuple` |
| `screen_to_world` | `screen_to_world(sx: float, sy: float) -> tuple[float, float]` |
| `screen_w` | `screen_w() -> int` |
| `set_always_on_top` | `set_always_on_top(enabled: bool)` |
| `set_blend_shape` | `set_blend_shape(vrm_id: int, name: str, weight: float)` |
| `set_boid_active_count` | `set_boid_active_count(boid_id: int, count: int)` |
| `set_camera` | `set_camera(cam: Camera \| None)` |
| `set_click_through` | `set_click_through(enabled: bool)` |
| `set_decorations` | `set_decorations(enabled: bool)` |
| `set_font` | `set_font(font_id: int)` |
| `set_grab_frames` | `set_grab_frames(enabled: bool = True)` |
| `set_ime_cursor_pos` | `set_ime_cursor_pos(x: float, y: float)` |
| `set_vrm_bone_euler` | `set_vrm_bone_euler(vrm_id: int, bone: str, rx=0.0, ry=0.0, rz=0.0)` |
| `set_vrm_bone_scale` | `set_vrm_bone_scale(vrm_id: int, bone: str, sx: float = 1.0, sy: float = 1.0, sz: float = 1.0)` |
| `set_vrm_bone_trans` | `set_vrm_bone_trans(vrm_id: int, bone: str, tx: float = 0.0, ty: float = 0.0, tz: float = 0.0)` |
| `set_vrm_first_person` | `set_vrm_first_person(vrm_id: int, enabled: bool = True)` |
| `set_vrm_offset` | `set_vrm_offset(vrm_id: int, x: float = 0.0, y: float = 0.0, z: float = 0.0)` |
| `set_vrm_pose` | `set_vrm_pose(vrm_id: int, bones: list)` |
| `set_vrm_spring_enabled` | `set_vrm_spring_enabled(vrm_id: int, enabled: bool = True)` |
| `set_vrm_spring_wind` | `set_vrm_spring_wind(vrm_id: int, x: float = 0.0, y: float = 0.0, z: float = 0.0)` |
| `set_window_position` | `set_window_position(x: int, y: int)` |
| `set_window_title` | `set_window_title(title: str)` |
| `sign` | `sign(value: float) -> int` |
| `spring_bone` | `spring_bone(vrm_path: str, vrm_id: int) -> 'SpringBone'` |
| `step_vrm_spring` | `step_vrm_spring(vrm_id: int, dt: float)` |
| `stop_bgm` | `stop_bgm(fade: float = 0.0)` |
| `texture_from_pixels` | `texture_from_pixels(width: int, height: int, pixels: bytes, *, name: str \| None = None, srgb: bool = True) -> int` |
| `texture_size` | `texture_size(tid: int) -> tuple` |
| `unload_gltf` | `unload_gltf(model_id: int)` |
| `update_boids` | `update_boids(boid_id: int, dt: float)` |
| `update_boids_gpu` | `update_boids_gpu(boid_id: int, dt: float)` |
| `update_camera_3d` | `update_camera_3d(view: list, proj: list)` |
| `vrm_spring_info` | `vrm_spring_info(vrm_id: int) -> tuple` |
| `world_to_screen` | `world_to_screen(wx: float, wy: float) -> tuple` |
| `world_to_screen` | `world_to_screen(wx: float, wy: float) -> tuple[float, float]` |
| `AnimationClip` | `class AnimationClip  (from kagra.skeleton)` |
| `AnimationTrack` | `class AnimationTrack  (from kagra.skeleton)` |
| `AnimatorComponent` | `class AnimatorComponent  (from kagra.entity)` |
| `ArmIK` | `class ArmIK  (from kagra.vrm_ik)` |
| `asset_debug_info` | `export asset_debug_info  (from kagra.debug_tools)` |
| `AssetDatabase` | `class AssetDatabase  (from kagra.asset_db)` |
| `AssetKind` | `class AssetKind  (from kagra.contracts)` |
| `AssetManifest` | `class AssetManifest  (from kagra.asset_manifest)` |
| `assets` | `assets` |
| `Attachment` | `class Attachment  (from kagra.skeleton)` |
| `audio` | `audio` |
| `backdrop_sphere` | `export backdrop_sphere  (from kagra.stage)` |
| `BgmCue` | `class BgmCue  (from kagra.bgm_sync)` |
| `BgmSync` | `class BgmSync  (from kagra.bgm_sync)` |
| `Bone` | `class Bone  (from kagra.skeleton)` |
| `BoxCollider` | `class BoxCollider  (from kagra.physics)` |
| `Camera` | `class Camera  (from kagra.camera)` |
| `CameraFollower` | `class CameraFollower  (from kagra.components)` |
| `CameraTrack` | `class CameraTrack  (from kagra.timeline)` |
| `CharState` | `class CharState  (from kagra.ai_character)` |
| `ChatMessage` | `class ChatMessage  (from kagra.stream)` |
| `ChoiceMenu` | `class ChoiceMenu  (from kagra.ui)` |
| `classify_stage_file` | `export classify_stage_file  (from kagra.stage)` |
| `Collider` | `class Collider  (from kagra.entity)` |
| `Component` | `class Component  (from kagra.entity)` |
| `DataObject` | `class DataObject  (from kagra.scriptable)` |
| `DataRegistry` | `class DataRegistry  (from kagra.scriptable)` |
| `DevConsole` | `class DevConsole  (from kagra.console)` |
| `DialogScript` | `class DialogScript  (from kagra.ui)` |
| `Easing` | `class Easing  (from kagra.ui)` |
| `Entity` | `class Entity  (from kagra.entity)` |
| `EntityAnimTrack` | `class EntityAnimTrack  (from kagra.timeline)` |
| `EntityScene` | `class EntityScene  (from kagra.entity)` |
| `EventBus` | `class EventBus  (from kagra.event_bus)` |
| `EventFlags` | `class EventFlags  (from kagra.ui)` |
| `EventTrack` | `class EventTrack  (from kagra.timeline)` |
| `FourDirAnimator` | `class FourDirAnimator  (from kagra.components)` |
| `get_blend_shape_names` | `get_blend_shape_names` |
| `get_console` | `export get_console  (from kagra.console)` |
| `get_data_registry` | `export get_data_registry  (from kagra.scriptable)` |
| `get_global_bus` | `export get_global_bus  (from kagra.event_bus)` |
| `HBox` | `class HBox  (from kagra.ui)` |
| `HotReloader` | `class HotReloader  (from kagra.hot_reload)` |
| `http_get` | `export http_get  (from kagra.http_client)` |
| `http_post` | `export http_post  (from kagra.http_client)` |
| `http_tick` | `export http_tick  (from kagra.http_client)` |
| `HttpClient` | `class HttpClient  (from kagra.http_client)` |
| `HttpResponse` | `class HttpResponse  (from kagra.http_client)` |
| `inject_pointer` | `export inject_pointer  (from kagra.touch)` |
| `InstanceBatch` | `class InstanceBatch  (from kagra.instances)` |
| `KagraContractError` | `class KagraContractError  (from kagra.contracts)` |
| `KEY_DOWN` | `KEY_DOWN` |
| `KEY_ESCAPE` | `KEY_ESCAPE` |
| `KEY_LEFT` | `KEY_LEFT` |
| `KEY_RETURN` | `KEY_RETURN` |
| `KEY_RIGHT` | `KEY_RIGHT` |
| `KEY_SPACE` | `KEY_SPACE` |
| `KEY_UP` | `KEY_UP` |
| `KEY_X` | `KEY_X` |
| `KEY_Z` | `KEY_Z` |
| `Keyframe` | `class Keyframe  (from kagra.skeleton)` |
| `keys` | `keys` |
| `LipSyncTimeline` | `class LipSyncTimeline  (from kagra.vrm_lipsync)` |
| `list_saved` | `export list_saved  (from kagra.anim_io)` |
| `LiveScore` | `class LiveScore  (from kagra.bgm_sync)` |
| `load_clips_into` | `export load_clips_into  (from kagra.anim_io)` |
| `load_entity` | `export load_entity  (from kagra.scene_loader)` |
| `load_scene` | `export load_scene  (from kagra.scene_loader)` |
| `load_state_machine` | `export load_state_machine  (from kagra.anim_io)` |
| `load_timeline` | `export load_timeline  (from kagra.anim_io)` |
| `make_hot_scene` | `export make_hot_scene  (from kagra.hot_reload)` |
| `MeshAttachment` | `class MeshAttachment  (from kagra.skeleton)` |
| `MeshVertex` | `class MeshVertex  (from kagra.skeleton)` |
| `MessageWindow` | `class MessageWindow  (from kagra.ui)` |
| `MOUSE_LEFT` | `MOUSE_LEFT` |
| `MOUSE_MIDDLE` | `MOUSE_MIDDLE` |
| `MOUSE_RIGHT` | `MOUSE_RIGHT` |
| `openai_chat` | `export openai_chat  (from kagra.http_client)` |
| `pad_released` | `export pad_released  (from kagra.pad)` |
| `Panel` | `class Panel  (from kagra.ui)` |
| `PhysicsSystem` | `class PhysicsSystem  (from kagra.physics)` |
| `PointerPhase` | `class PointerPhase  (from kagra.touch)` |
| `PoseKeyframe` | `class PoseKeyframe  (from kagra.vrm_anim)` |
| `Prefab` | `class Prefab  (from kagra.prefab)` |
| `ProgressBar` | `class ProgressBar  (from kagra.ui)` |
| `RectRenderer` | `class RectRenderer  (from kagra.entity)` |
| `register_spawn_rule` | `export register_spawn_rule  (from kagra.scriptable)` |
| `reset_global_bus` | `export reset_global_bus  (from kagra.event_bus)` |
| `resolve_stage_path` | `export resolve_stage_path  (from kagra.stage)` |
| `RhythmJudge` | `class RhythmJudge  (from kagra.bgm_sync)` |
| `Rigidbody` | `class Rigidbody  (from kagra.physics)` |
| `RigRenderer` | `class RigRenderer  (from kagra.entity)` |
| `save_all` | `export save_all  (from kagra.anim_io)` |
| `save_clips` | `export save_clips  (from kagra.anim_io)` |
| `save_scene` | `export save_scene  (from kagra.scene_io)` |
| `save_state_machine` | `export save_state_machine  (from kagra.anim_io)` |
| `save_timeline` | `export save_timeline  (from kagra.anim_io)` |
| `SaveLoad` | `class SaveLoad  (from kagra.ui)` |
| `scan_assets` | `export scan_assets  (from kagra.asset_scan)` |
| `scene` | `scene` |
| `SceneGraph` | `class SceneGraph  (from kagra.scenegraph)` |
| `SceneRuntime` | `class SceneRuntime  (from kagra.scene_runtime)` |
| `Script` | `class Script  (from kagra.entity)` |
| `ScrollView` | `class ScrollView  (from kagra.ui)` |
| `serialize_component` | `export serialize_component  (from kagra.scene_io)` |
| `serialize_entity` | `export serialize_entity  (from kagra.scene_io)` |
| `serialize_transform` | `export serialize_transform  (from kagra.scene_io)` |
| `set_data_dir` | `export set_data_dir  (from kagra.scriptable)` |
| `SHADER_DEFAULT` | `SHADER_DEFAULT` |
| `SHADER_FLASH` | `SHADER_FLASH` |
| `SHADER_GLOW` | `SHADER_GLOW` |
| `SHADER_GRAYSCALE` | `SHADER_GRAYSCALE` |
| `SHADER_SPOTLIGHT` | `SHADER_SPOTLIGHT` |
| `SHADER_TINT` | `SHADER_TINT` |
| `Skeleton` | `class Skeleton  (from kagra.skeleton)` |
| `SkeletonAnimator` | `class SkeletonAnimator  (from kagra.skeleton)` |
| `spawn_from` | `export spawn_from  (from kagra.scriptable)` |
| `spawn_rule` | `export spawn_rule  (from kagra.scriptable)` |
| `SpringBone` | `class SpringBone  (from kagra.vrm_spring)` |
| `Sprite` | `class Sprite  (from kagra.entity)` |
| `SpriteRenderer` | `class SpriteRenderer  (from kagra.entity)` |
| `TextRenderer` | `class TextRenderer  (from kagra.entity)` |
| `TILE_DAMAGE` | `class TILE_DAMAGE  (from kagra.tilemap)` |
| `TILE_DOOR` | `class TILE_DOOR  (from kagra.tilemap)` |
| `TILE_LADDER` | `class TILE_LADDER  (from kagra.tilemap)` |
| `TILE_SOLID` | `class TILE_SOLID  (from kagra.tilemap)` |
| `TILE_WATER` | `class TILE_WATER  (from kagra.tilemap)` |
| `TileMap` | `class TileMap  (from kagra.tilemap)` |
| `TileSet` | `class TileSet  (from kagra.tilemap)` |
| `Timeline` | `class Timeline  (from kagra.timeline)` |
| `timeline_from_audio_query` | `export timeline_from_audio_query  (from kagra.vrm_lipsync)` |
| `TimelinePlayer` | `class TimelinePlayer  (from kagra.timeline)` |
| `TopDownMovement` | `class TopDownMovement  (from kagra.components)` |
| `TopDownPhysicsSystem` | `class TopDownPhysicsSystem  (from kagra.physics)` |
| `Track` | `class Track  (from kagra.timeline)` |
| `Transform` | `class Transform  (from kagra.entity)` |
| `Transform2D` | `class Transform2D  (from kagra.skeleton)` |
| `TransitionScene` | `class TransitionScene  (from kagra.ui)` |
| `TweenManager` | `class TweenManager  (from kagra.ui)` |
| `TwoBoneIK` | `class TwoBoneIK  (from kagra.vrm_ik)` |
| `UIGroup` | `class UIGroup  (from kagra.ui)` |
| `VBox` | `class VBox  (from kagra.ui)` |
| `voicevox_speak` | `export voicevox_speak  (from kagra.http_client)` |
| `VoicevoxError` | `class VoicevoxError  (from kagra.voicevox)` |
| `VrmAnimator` | `class VrmAnimator  (from kagra.vrm_anim)` |
| `VrmModel` | `class VrmModel  (from kagra.vrm_loader)` |
| `World` | `class World  (from kagra.entity)` |

## Agent notes

- 存在しない API を呼ばないこと。ここに無い名前は未公開か内部用です。
- 3D ゲームは Front から探す。Shelf の tilemap / ECS / 2D `Camera` は推奨しない。
- `world_to_screen(wx, wy)` は **2D**。3D は `Camera3D.world_to_screen(wx, wy, wz)`。
- セーブは `save_json` / `load_json`。`load_data` はアセットレジストリ。
- VRM が checkout に無いときは `ensure_vrm()`。パスを直書きしない。
- ワンショットポーズは `ActionController`（`ActionController.names()`）。
- 静的メッシュは `upload_mesh_3d` で一度載せ、`draw_mesh_id` で描く。
- ワールド箱は視錐台カリングされる。箱の描画は `draw_mesh_instances`。直前フレームは `render_stats()`。
- VRM プリミティブはパッド付きボーン AABB でカリング。`doubleSided` のときだけ両面。MToon は裏面法線を反転（頭の中からのリム白飛び / 髪越しの顔を防ぐ）。
- 床と箱: `World3D`（または `Physics3D` + `box_mesh`）。カメラは `Camera3D.follow`。
- 短い 3D: `Prop` + `Walk` + `sky()` / `room()` / `water()`。地形は `World3D.set_height_fn` + `island_height` / `overworld_height` / `open_world_height`。タイル化は `tile=` / `stream_radius=`。遠いタイルは `lod_radius=` / `lod_cells=`。拾いは `can_pick`。`Walk(..., jump=)`。
- 一人称: `Walk(..., first_person=True)`。目線は `eye_height`。ポインタロックは一人称のとき（OS が拒めばフォールバック）。`F` で切替えるデモは Prop Garden。
- ホバー / クリック: `hovered_prop(cam)`。`clicked_prop(cam)` は押下。レイ直打ちは `kagra.play.hovered_prop(ox,oy,oz,dx,dy,dz)`。`plane` は除外。
- エージェントの目: `kagra.annotate(sx, sy)` はプレビュークリックを JSONL に残す（screen / world / bone / Prop id）。`kagra.debug_trace(foot_y=…, height_fn=…)` は接地浮き。エディタではない。「ここもう少し」は数値にする。
- カメラ壁クリップ: `Camera3D.follow(..., world=)` がプレイヤー→カメラの線分を静的箱に当て、当たったら距離を縮める。`min_distance` / `max_distance` で VRM 頭の中と Tiny speck を防ぐ。`Walk` は自動。
- 動く Prop: `p.x` / `set_position` / `vx` + `Prop.update_all(dt)`。消すのは `destroy(p)` か `p.enabled = False`。持つのは `Walk.carry(prop)`。
- `animate(obj, "y", end)` / `sequence` / `Tween`。`Prop.update_all` が回す。
- HUD: `Label` / `Button`（画面空間。2D `kagra.ui` の同名は棚）。音は `sound("coin")`。
- 球 / 円柱の当たりとホバーは AABB ではない。`World3D.add_sphere` / `add_cylinder`。
- Prop テクスチャ: `texture=kagra.texture_from_fn(...)` または `load`。0 なら `color`。
- Prop 親子は 4 段（玄孫まで）。子の `x,y,z,yaw` はローカル。
- glTF 部品: `Prop("crate.glb")`。`stage()` / `load_gltf` は会場。同梱エイリアス `cube.glb`。当たりは AABB。`mesh_hit=True` で三角形。
- ゲームパッド: `axis("left")` / `pad("a")` / `inject_pad`。`Walk` は左スティック移動・右スティック視点。実機 USB/XInput は EventLoop で gilrs（`inject_pad` が優先。CI は inject）。
- 影は床・箱・Prop も落とす。`set_shadow_cascades(2)` で近／遠の 2 段（既定 1。Prop Garden は変えない）。屋外はテクセルスナップ。OSM ではない街 JSON は `load_city`。三角形当たりは `add_trimesh` / `Prop(..., mesh_hit=True)`。積み木は `add_box(..., is_static=False)`（落ちて積もり、Walk が乗る。Rapier クレートは wheel に入れない）。
- 点光源 4: `set_point_light(..., slot=0..3)`。0 がキー（影は無し）。1..3 は埋め。スポットは `set_spot_light(..., slot=)`。室内の透視影はスロット 0 のスポットだけ。平行光は埋め。
- HDRI: `set_hdri("studio")` または正距円筒のパス。拡散は小さな irradiance キューブ。スペキュラは mip LOD。露出は `set_exposure`（既定 1）。ACES は `set_tonemap`（既定オフ）。
- 坂は接平面、急斜面は滑る。接地は小さい足 AABB + 接平面（太いカプセル AABB の max-Y は浮く。`debug_trace` で測る。Rapier は入れない）。デモは Pretty Room / Overworld。
- 汎用メッシュの金属/粗さ: `upload_mesh_3d(..., metallic=, roughness=)` / `Prop(..., metallic=)` / `set_mesh_pbr`。接空間法線は `normal_texture_id` / `Prop(..., normal=)` / `set_mesh_normal` / glTF `normalTexture`（cotangent frame。ストライドは 32）。MToon は触らない。
- 色付きメッシュ: `solid_tex` + `sphere_mesh` / `cylinder_mesh` / `box_mesh`。
- `kagra-shared` / `mobile/` は別の運転デモ。この Python スタックと混ぜない。
- 頭脳: `kagra.brain("kairi"|"ollama"|"openai")` / `KairiBrain`。既定は `https://kairi.onrender.com`（チャットは `KAIRI_API_TOKEN`）。モデルは wheel に入れない。`AiCharacter.set_llm_func(mind.ask)`。
- Rust バインディングの整合は `tests/test_api_bindings.py` も参照。
- 再生成: `python tools/gen_api_index.py`

# KAGRA Public API Index

このファイルは `tools/gen_api_index.py` により自動生成されます。手編集しないでください。

エントリ数: **316**

## Functions

| Name | Signature |
|---|---|
| `after` | `after(frames: int, from_tick: int = 0) -> bool` |
| `angle_to` | `angle_to(x1: float, y1: float, x2: float, y2: float) -> float` |
| `avatar` | `avatar(vrm_path: str) -> 'VrmAvatar'` |
| `backspace_pressed` | `backspace_pressed()` |
| `bar` | `bar(x: float, y: float, w: float, h: float, value: float, max_value: float = 100, *, bg=(25, 25, 35), fill=(50, 220, 80))` |
| `bgm` | `bgm(path: str, loop: bool = True, vol: float = 0.8) -> None` |
| `button` | `button(x: float, y: float, w: float, h: float, label: str = '', *, bg=(70, 70, 90), hover=(100, 100, 150), color=(255, 255, 255), size: int = 20, font: int = None) -> bool` |
| `camera_update` | `camera_update(dt: float)` |
| `circle` | `circle(x: float, y: float, radius: float, r: int = 255, g: int = 255, b: int = 255, a: int = 255, segments: int = 24)` |
| `circle_fill` | `circle_fill(x: float, y: float, radius: float, color=(255, 255, 255), alpha: int = 255)` |
| `circle_outline` | `circle_outline(x: float, y: float, radius: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `clamp` | `clamp(value: float, lo: float, hi: float) -> float` |
| `cls` | `cls(r=0, g=0, b=0)` |
| `collide_rect` | `collide_rect(ax, ay, aw, ah, bx, by, bw, bh)` |
| `collide_rect_overlap` | `collide_rect_overlap(ax, ay, aw, ah, bx, by, bw, bh)` |
| `create_boid_system` | `create_boid_system(count: int, width: float = 1280.0, height: float = 720.0) -> int` |
| `create_boid_system_gpu` | `create_boid_system_gpu(count: int, width: float = 1280.0, height: float = 720.0) -> int` |
| `distance` | `distance(x1: float, y1: float, x2: float, y2: float) -> float` |
| `distance_sq` | `distance_sq(x1: float, y1: float, x2: float, y2: float) -> float` |
| `down` | `down(name: str) -> bool` |
| `drag_window` | `drag_window()` |
| `draw_boids` | `draw_boids(boid_id: int, batch_id: int, sprite_w: float = 6.0, sprite_h: float = 3.0)` |
| `draw_boids_gpu` | `draw_boids_gpu(boid_id: int)` |
| `draw_mesh` | `draw_mesh(texture_id: int, verts: list, shader_id: int = 0, shader_params: list = None)` |
| `draw_mesh_3d` | `draw_mesh_3d(texture_id: int, verts: list, indices: list)` |
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
| `draw_vrm` | `draw_vrm(vrm_id: int)` |
| `emit` | `emit(event, data=None, deferred=False)` |
| `enter_pressed` | `enter_pressed()` |
| `escape_pressed` | `escape_pressed()` |
| `every` | `every(frames: int) -> bool` |
| `fill` | `fill(x: float, y: float, w: float, h: float, color=(255, 255, 255), alpha: int = 255)` |
| `flush_events` | `flush_events()` |
| `focus_window` | `focus_window()` |
| `font` | `font(path: str = None) -> int` |
| `frame_count` | `frame_count() -> int` |
| `frame_index` | `frame_index(count: int, hold_for: int = 4, repeat: bool = True, offset: int = 0) -> int` |
| `get_camera` | `get_camera() -> Camera \| None` |
| `get_camera3d` | `get_camera3d() -> Camera3D \| None` |
| `get_engine` | `get_engine() -> _Engine` |
| `get_fps` | `get_fps() -> float` |
| `get_preedit_text` | `get_preedit_text() -> str` |
| `get_screen_size` | `get_screen_size() -> tuple` |
| `get_typed_chars` | `get_typed_chars() -> str` |
| `get_vrm_look_at` | `get_vrm_look_at(vrm_id: int) -> dict \| None` |
| `go` | `go(next_scene: Scene) -> None` |
| `has_vrm_bone` | `has_vrm_bone(vrm_id: int, name: str) -> bool` |
| `image` | `image(tex: int, x: float, y: float, w: float = None, h: float = None, *, alpha: float = 1.0, rotation: float = 0.0, flip_x: bool = False, flip_y: bool = False, sx: float = 0.0, sy: float = 0.0, sw: float = None, sh: float = None)` |
| `image_world` | `image_world(tex: int, wx: float, wy: float, w: float, h: float, *, alpha: float = 1.0, flip_x: bool = False, flip_y: bool = False)` |
| `init` | `init(width=1280, height=720, title='KAGRA Game', fps=60, transparent=False, decorations=True, always_on_top=False, visible=True)` |
| `inject_key` | `inject_key(name: str, down: bool = True)` |
| `inject_mouse` | `inject_mouse(x=None, y=None, button=None, down=None)` |
| `inside_circle` | `inside_circle(px: float, py: float, cx: float, cy: float, radius: float) -> bool` |
| `inside_rect` | `inside_rect(px: float, py: float, rx: float, ry: float, rw: float, rh: float) -> bool` |
| `intersect_circle_rect` | `intersect_circle_rect(cx: float, cy: float, cr: float, rx: float, ry: float, rw: float, rh: float) -> bool` |
| `intersect_rect` | `intersect_rect(ax: float, ay: float, aw: float, ah: float, bx: float, by: float, bw: float, bh: float) -> bool` |
| `key` | `key(name: str) -> bool` |
| `key_down` | `key_down(code: int) -> bool` |
| `key_pressed` | `key_pressed(code: int) -> bool` |
| `key_released` | `key_released(code: int) -> bool` |
| `lerp` | `lerp(a: float, b: float, t: float) -> float` |
| `line` | `line(x1: float, y1: float, x2: float, y2: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `line_h` | `line_h(x: float, y: float, length: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `line_v` | `line_v(x: float, y: float, length: float, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `list_blend_shapes` | `list_blend_shapes(vrm_id: int) -> list[str]` |
| `list_human_bones` | `list_human_bones(vrm_id: int) -> list[str]` |
| `load` | `load(path: str) -> int` |
| `load_bvh` | `load_bvh(path: str, extra_map: dict = None) -> 'BvhMotion'` |
| `load_data` | `load_data(key: str, force_reload=False) -> 'DataObject'` |
| `load_fbx` | `load_fbx(path: str, clip_name: str = None) -> 'FbxMotion'` |
| `load_font` | `load_font(path: str) -> int` |
| `load_rig` | `load_rig(path: str) -> int` |
| `load_shader` | `load_shader(path: str) -> int` |
| `load_shader_src` | `load_shader_src(wgsl_src: str) -> int` |
| `load_texture` | `load_texture(path: str) -> int` |
| `load_vrm` | `load_vrm(path: str) -> int` |
| `load_vrma` | `load_vrma(path: str, *, sample_fps: float = 30.0) -> 'VrmaMotion'` |
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
| `play_bgm` | `play_bgm(path: str, loop_=True, volume=0.8)` |
| `play_se` | `play_se(path: str, volume=1.0)` |
| `point_in_rect` | `point_in_rect(px, py, rx, ry, rw, rh)` |
| `polygon` | `polygon(pts: list, color=(255, 255, 255), alpha: int = 255)` |
| `polygon_outline` | `polygon_outline(pts: list, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `pop` | `pop() -> None` |
| `preload_data` | `preload_data(subdir='', recursive=True) -> list` |
| `pressed` | `pressed(name: str) -> bool` |
| `push` | `push(next_scene: Scene) -> None` |
| `quit` | `quit()` |
| `rect` | `rect(x, y, w, h, color=255, g=None, b=None, a=255)` |
| `rect_world` | `rect_world(wx, wy, w, h, r, g, b, a=255)` |
| `released` | `released(name: str) -> bool` |
| `reset_blend_shapes` | `reset_blend_shapes(vrm_id: int)` |
| `reset_vrm_pose` | `reset_vrm_pose(vrm_id: int)` |
| `resolve_vrm_bone` | `resolve_vrm_bone(vrm_id: int, name: str) -> int \| None` |
| `rounded_rect` | `rounded_rect(x: float, y: float, w: float, h: float, radius: float = 8, color=(255, 255, 255), alpha: int = 255)` |
| `rounded_rect_outline` | `rounded_rect_outline(x: float, y: float, w: float, h: float, radius: float = 8, color=(255, 255, 255), width: float = 1, alpha: int = 255)` |
| `run` | `run(update=None, draw=None, start_scene: Scene = None, max_frames=None, fixed_dt=None)` |
| `screen_h` | `screen_h() -> int` |
| `screen_to_world` | `screen_to_world(sx: float, sy: float) -> tuple` |
| `screen_to_world` | `screen_to_world(sx: float, sy: float) -> tuple[float, float]` |
| `screen_w` | `screen_w() -> int` |
| `screenshot` | `screenshot(path: str)` |
| `se` | `se(path: str, vol: float = 1.0) -> None` |
| `set_always_on_top` | `set_always_on_top(enabled: bool)` |
| `set_blend_shape` | `set_blend_shape(vrm_id: int, name: str, weight: float)` |
| `set_boid_active_count` | `set_boid_active_count(boid_id: int, count: int)` |
| `set_camera` | `set_camera(cam: Camera \| None)` |
| `set_camera3d` | `set_camera3d(cam: Camera3D \| None)` |
| `set_click_through` | `set_click_through(enabled: bool)` |
| `set_decorations` | `set_decorations(enabled: bool)` |
| `set_fog` | `set_fog(start: float = 5.0, end: float = 20.0, color: tuple = (110, 180, 230), *, enabled: bool = True)` |
| `set_font` | `set_font(font_id: int)` |
| `set_ime_cursor_pos` | `set_ime_cursor_pos(x: float, y: float)` |
| `set_light_dir` | `set_light_dir(x: float, y: float, z: float)` |
| `set_shadow_enabled` | `set_shadow_enabled(enabled: bool = True)` |
| `set_toon_params` | `set_toon_params(threshold: float = 0.5, softness: float = 1.0, shade: float = 0.55, lit: float = 1.0)` |
| `set_vrm_bone_euler` | `set_vrm_bone_euler(vrm_id: int, bone: str, rx=0.0, ry=0.0, rz=0.0)` |
| `set_vrm_bone_scale` | `set_vrm_bone_scale(vrm_id: int, bone: str, sx: float = 1.0, sy: float = 1.0, sz: float = 1.0)` |
| `set_vrm_bone_trans` | `set_vrm_bone_trans(vrm_id: int, bone: str, tx: float = 0.0, ty: float = 0.0, tz: float = 0.0)` |
| `set_vrm_offset` | `set_vrm_offset(vrm_id: int, x: float = 0.0, y: float = 0.0, z: float = 0.0)` |
| `set_window_position` | `set_window_position(x: int, y: int)` |
| `set_window_title` | `set_window_title(title: str)` |
| `sign` | `sign(value: float) -> int` |
| `spring_bone` | `spring_bone(vrm_path: str, vrm_id: int) -> 'SpringBone'` |
| `stop_bgm` | `stop_bgm(fade: float = 0.0)` |
| `text` | `text(s, x: float, y: float, size: int = 24, color=(255, 255, 255), font: int = None, alpha: int = 255)` |
| `texture_size` | `texture_size(tid: int) -> tuple` |
| `tick_count` | `tick_count() -> int` |
| `update_boids` | `update_boids(boid_id: int, dt: float)` |
| `update_boids_gpu` | `update_boids_gpu(boid_id: int, dt: float)` |
| `update_camera_3d` | `update_camera_3d(view: list, proj: list)` |
| `world_to_screen` | `world_to_screen(wx: float, wy: float) -> tuple` |
| `world_to_screen` | `world_to_screen(wx: float, wy: float) -> tuple[float, float]` |

## Classes / Exports / Objects

| Name | Note |
|---|---|
| `AABB` | `class AABB  (from kagra.physics3d)` (class) |
| `AiCharacter` | `class AiCharacter  (from kagra.ai_character)` (class) |
| `AnimationClip` | `class AnimationClip  (from kagra.skeleton)` (class) |
| `AnimationTrack` | `class AnimationTrack  (from kagra.skeleton)` (class) |
| `AnimatorComponent` | `class AnimatorComponent  (from kagra.entity)` (class) |
| `apply_pad` | `export apply_pad  (from kagra.touch)` (export) |
| `ArmIK` | `class ArmIK  (from kagra.vrm_ik)` (class) |
| `asset_debug_info` | `export asset_debug_info  (from kagra.debug_tools)` (export) |
| `AssetDatabase` | `class AssetDatabase  (from kagra.asset_db)` (class) |
| `AssetKind` | `class AssetKind  (from kagra.contracts)` (class) |
| `AssetManifest` | `class AssetManifest  (from kagra.asset_manifest)` (class) |
| `assets` | `assets` (object) |
| `Attachment` | `class Attachment  (from kagra.skeleton)` (class) |
| `audio` | `audio` (object) |
| `BgmCue` | `class BgmCue  (from kagra.bgm_sync)` (class) |
| `BgmSync` | `class BgmSync  (from kagra.bgm_sync)` (class) |
| `Bone` | `class Bone  (from kagra.skeleton)` (class) |
| `BoxCollider` | `class BoxCollider  (from kagra.physics)` (class) |
| `Button` | `class Button  (from kagra.ui)` (class) |
| `Camera` | `class Camera  (from kagra.camera)` (class) |
| `Camera3D` | `class Camera3D  (from kagra.camera3d)` (class) |
| `CameraFollower` | `class CameraFollower  (from kagra.components)` (class) |
| `CameraTrack` | `class CameraTrack  (from kagra.timeline)` (class) |
| `CharState` | `class CharState  (from kagra.ai_character)` (class) |
| `ChoiceMenu` | `class ChoiceMenu  (from kagra.ui)` (class) |
| `Collider` | `class Collider  (from kagra.entity)` (class) |
| `Component` | `class Component  (from kagra.entity)` (class) |
| `DataObject` | `class DataObject  (from kagra.scriptable)` (class) |
| `DataRegistry` | `class DataRegistry  (from kagra.scriptable)` (class) |
| `describe_environment` | `export describe_environment  (from kagra.contracts)` (export) |
| `DevConsole` | `class DevConsole  (from kagra.console)` (class) |
| `DialogScript` | `class DialogScript  (from kagra.ui)` (class) |
| `Easing` | `class Easing  (from kagra.ui)` (class) |
| `EmotionController` | `class EmotionController  (from kagra.vrm_emotion)` (class) |
| `ensure_vrm` | `export ensure_vrm  (from kagra.samples)` (export) |
| `Entity` | `class Entity  (from kagra.entity)` (class) |
| `EntityAnimTrack` | `class EntityAnimTrack  (from kagra.timeline)` (class) |
| `EntityScene` | `class EntityScene  (from kagra.entity)` (class) |
| `EventBus` | `class EventBus  (from kagra.event_bus)` (class) |
| `EventFlags` | `class EventFlags  (from kagra.ui)` (class) |
| `EventTrack` | `class EventTrack  (from kagra.timeline)` (class) |
| `FourDirAnimator` | `class FourDirAnimator  (from kagra.components)` (class) |
| `get_blend_shape_names` | `get_blend_shape_names` (object) |
| `get_console` | `export get_console  (from kagra.console)` (export) |
| `get_data_registry` | `export get_data_registry  (from kagra.scriptable)` (export) |
| `get_global_bus` | `export get_global_bus  (from kagra.event_bus)` (export) |
| `HBox` | `class HBox  (from kagra.ui)` (class) |
| `HotReloader` | `class HotReloader  (from kagra.hot_reload)` (class) |
| `http_get` | `export http_get  (from kagra.http_client)` (export) |
| `http_post` | `export http_post  (from kagra.http_client)` (export) |
| `http_tick` | `export http_tick  (from kagra.http_client)` (export) |
| `HttpClient` | `class HttpClient  (from kagra.http_client)` (class) |
| `HttpResponse` | `class HttpResponse  (from kagra.http_client)` (class) |
| `inject_pointer` | `export inject_pointer  (from kagra.touch)` (export) |
| `InstanceBatch` | `class InstanceBatch  (from kagra.instances)` (class) |
| `KagraContractError` | `class KagraContractError  (from kagra.contracts)` (class) |
| `KEY_DOWN` | `KEY_DOWN` (object) |
| `KEY_ESCAPE` | `KEY_ESCAPE` (object) |
| `KEY_LEFT` | `KEY_LEFT` (object) |
| `KEY_RETURN` | `KEY_RETURN` (object) |
| `KEY_RIGHT` | `KEY_RIGHT` (object) |
| `KEY_SPACE` | `KEY_SPACE` (object) |
| `KEY_UP` | `KEY_UP` (object) |
| `KEY_X` | `KEY_X` (object) |
| `KEY_Z` | `KEY_Z` (object) |
| `Keyframe` | `class Keyframe  (from kagra.skeleton)` (class) |
| `keys` | `keys` (object) |
| `Label` | `class Label  (from kagra.ui)` (class) |
| `LipSyncController` | `class LipSyncController  (from kagra.vrm_lipsync)` (class) |
| `LipSyncTimeline` | `class LipSyncTimeline  (from kagra.vrm_lipsync)` (class) |
| `list_saved` | `export list_saved  (from kagra.anim_io)` (export) |
| `LiveScore` | `class LiveScore  (from kagra.bgm_sync)` (class) |
| `load_clips_into` | `export load_clips_into  (from kagra.anim_io)` (export) |
| `load_entity` | `export load_entity  (from kagra.scene_loader)` (export) |
| `load_scenario` | `export load_scenario  (from kagra.verify)` (export) |
| `load_scene` | `export load_scene  (from kagra.scene_loader)` (export) |
| `load_state_machine` | `export load_state_machine  (from kagra.anim_io)` (export) |
| `load_timeline` | `export load_timeline  (from kagra.anim_io)` (export) |
| `LookAtController` | `class LookAtController  (from kagra.vrm_lookat)` (class) |
| `make_hot_scene` | `export make_hot_scene  (from kagra.hot_reload)` (export) |
| `MeshAttachment` | `class MeshAttachment  (from kagra.skeleton)` (class) |
| `MeshVertex` | `class MeshVertex  (from kagra.skeleton)` (class) |
| `MessageWindow` | `class MessageWindow  (from kagra.ui)` (class) |
| `MOUSE_LEFT` | `MOUSE_LEFT` (object) |
| `MOUSE_MIDDLE` | `MOUSE_MIDDLE` (object) |
| `MOUSE_RIGHT` | `MOUSE_RIGHT` (object) |
| `openai_chat` | `export openai_chat  (from kagra.http_client)` (export) |
| `Panel` | `class Panel  (from kagra.ui)` (class) |
| `Physics3D` | `class Physics3D  (from kagra.physics3d)` (class) |
| `PhysicsSystem` | `class PhysicsSystem  (from kagra.physics)` (class) |
| `PointerEvent` | `class PointerEvent  (from kagra.touch)` (class) |
| `PointerPhase` | `class PointerPhase  (from kagra.touch)` (class) |
| `PoseKeyframe` | `class PoseKeyframe  (from kagra.vrm_anim)` (class) |
| `Prefab` | `class Prefab  (from kagra.prefab)` (class) |
| `ProgressBar` | `class ProgressBar  (from kagra.ui)` (class) |
| `RectRenderer` | `class RectRenderer  (from kagra.entity)` (class) |
| `register_spawn_rule` | `export register_spawn_rule  (from kagra.scriptable)` (export) |
| `reset_global_bus` | `export reset_global_bus  (from kagra.event_bus)` (export) |
| `resolve_asset` | `export resolve_asset  (from kagra.contracts)` (export) |
| `RhythmJudge` | `class RhythmJudge  (from kagra.bgm_sync)` (class) |
| `Rigidbody` | `class Rigidbody  (from kagra.physics)` (class) |
| `RigidBody3D` | `class RigidBody3D  (from kagra.physics3d)` (class) |
| `RigRenderer` | `class RigRenderer  (from kagra.entity)` (class) |
| `run_scenario` | `export run_scenario  (from kagra.verify)` (export) |
| `run_scenario_path` | `export run_scenario_path  (from kagra.verify)` (export) |
| `save_all` | `export save_all  (from kagra.anim_io)` (export) |
| `save_clips` | `export save_clips  (from kagra.anim_io)` (export) |
| `save_scene` | `export save_scene  (from kagra.scene_io)` (export) |
| `save_state_machine` | `export save_state_machine  (from kagra.anim_io)` (export) |
| `save_timeline` | `export save_timeline  (from kagra.anim_io)` (export) |
| `SaveLoad` | `class SaveLoad  (from kagra.ui)` (class) |
| `scan_assets` | `export scan_assets  (from kagra.asset_scan)` (export) |
| `Scene` | `class Scene` (class) |
| `scene` | `scene` (object) |
| `SceneGraph` | `class SceneGraph  (from kagra.scenegraph)` (class) |
| `SceneRuntime` | `class SceneRuntime  (from kagra.scene_runtime)` (class) |
| `Script` | `class Script  (from kagra.entity)` (class) |
| `ScrollView` | `class ScrollView  (from kagra.ui)` (class) |
| `serialize_component` | `export serialize_component  (from kagra.scene_io)` (export) |
| `serialize_entity` | `export serialize_entity  (from kagra.scene_io)` (export) |
| `serialize_transform` | `export serialize_transform  (from kagra.scene_io)` (export) |
| `set_data_dir` | `export set_data_dir  (from kagra.scriptable)` (export) |
| `SHADER_DEFAULT` | `SHADER_DEFAULT` (object) |
| `SHADER_FLASH` | `SHADER_FLASH` (object) |
| `SHADER_GLOW` | `SHADER_GLOW` (object) |
| `SHADER_GRAYSCALE` | `SHADER_GRAYSCALE` (object) |
| `SHADER_SPOTLIGHT` | `SHADER_SPOTLIGHT` (object) |
| `SHADER_TINT` | `SHADER_TINT` (object) |
| `Skeleton` | `class Skeleton  (from kagra.skeleton)` (class) |
| `SkeletonAnimator` | `class SkeletonAnimator  (from kagra.skeleton)` (class) |
| `spawn_from` | `export spawn_from  (from kagra.scriptable)` (export) |
| `spawn_rule` | `export spawn_rule  (from kagra.scriptable)` (export) |
| `SpringBone` | `class SpringBone  (from kagra.vrm_spring)` (class) |
| `Sprite` | `class Sprite  (from kagra.entity)` (class) |
| `SpriteRenderer` | `class SpriteRenderer  (from kagra.entity)` (class) |
| `TextRenderer` | `class TextRenderer  (from kagra.entity)` (class) |
| `TILE_DAMAGE` | `class TILE_DAMAGE  (from kagra.tilemap)` (class) |
| `TILE_DOOR` | `class TILE_DOOR  (from kagra.tilemap)` (class) |
| `TILE_LADDER` | `class TILE_LADDER  (from kagra.tilemap)` (class) |
| `TILE_SOLID` | `class TILE_SOLID  (from kagra.tilemap)` (class) |
| `TILE_WATER` | `class TILE_WATER  (from kagra.tilemap)` (class) |
| `TileMap` | `class TileMap  (from kagra.tilemap)` (class) |
| `TileSet` | `class TileSet  (from kagra.tilemap)` (class) |
| `Timeline` | `class Timeline  (from kagra.timeline)` (class) |
| `TimelinePlayer` | `class TimelinePlayer  (from kagra.timeline)` (class) |
| `TopDownMovement` | `class TopDownMovement  (from kagra.components)` (class) |
| `TopDownPhysicsSystem` | `class TopDownPhysicsSystem  (from kagra.physics)` (class) |
| `Track` | `class Track  (from kagra.timeline)` (class) |
| `Transform` | `class Transform  (from kagra.entity)` (class) |
| `Transform2D` | `class Transform2D  (from kagra.skeleton)` (class) |
| `TransitionScene` | `class TransitionScene  (from kagra.ui)` (class) |
| `Tween` | `class Tween  (from kagra.ui)` (class) |
| `TweenManager` | `class TweenManager  (from kagra.ui)` (class) |
| `TwoBoneIK` | `class TwoBoneIK  (from kagra.vrm_ik)` (class) |
| `UIGroup` | `class UIGroup  (from kagra.ui)` (class) |
| `VBox` | `class VBox  (from kagra.ui)` (class) |
| `VirtualPad` | `class VirtualPad  (from kagra.touch)` (class) |
| `voicevox_speak` | `export voicevox_speak  (from kagra.http_client)` (export) |
| `VrmAnimator` | `class VrmAnimator  (from kagra.vrm_anim)` (class) |
| `VrmModel` | `class VrmModel  (from kagra.vrm_loader)` (class) |
| `World` | `class World  (from kagra.entity)` (class) |

## Agent notes

- 存在しない API を呼ばないこと。ここに無い名前は未公開か内部用です。
- Rust バインディングの整合は `tests/test_api_bindings.py` も参照。
- 再生成: `python tools/gen_api_index.py`

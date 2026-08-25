// src/window.rs
// winit イベントループ + Renderer ブリッジ
// 修正: Surface は RendererV2 が所有するため、window.rs では一切触らない

use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::{
    event::{DeviceEvent, ElementState, Event, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, NativeKey},
    window::{CursorGrabMode, WindowBuilder, WindowLevel},
};

/// エージェント検証用: 次フレーム開始前に適用される入力イベント
#[derive(Clone, Debug)]
pub enum InjectEvent {
    KeyDown(u32),
    KeyUp(u32),
    MouseMove(f32, f32),
    MouseDown(u32),
    MouseUp(u32),
}

use crate::color::Color;
use crate::error::lock_recover;
use crate::input::InputState;
use crate::renderer::{
    DrawCommand, PolygonCommand, RectCommand, RendererV2, SkinnedMeshCommand, SpriteCommand, TextCommand,
};
use crate::rig::{self, Rig};
use crate::error::KaguraError;

enum WindowDrawCommand {
    Clear(Color),
    Draw(DrawCommand),
}

#[derive(Clone)]
pub enum WindowCommand {
    Drag,
    Focus,
    SetPosition(i32, i32),
    SetClickThrough(bool),
    SetAlwaysOnTop(bool),
    SetDecorations(bool),
    SetTitle(String),
    SetCursorLocked(bool),
    SetImeAllowed(bool),
}

pub struct KagraWindow {
    pub width: u32,
    pub height: u32,
    pub target_fps: u32,
    pub current_fps_atomic: Arc<AtomicU32>,
    title: String,

    pub input: Arc<Mutex<InputState>>,
    draw_queue: Arc<Mutex<Vec<WindowDrawCommand>>>,
    pub renderer: Arc<Mutex<Option<RendererV2>>>,

    pub rigs: Arc<Mutex<HashMap<u32, Rig>>>,
    pub next_rig_id: Arc<Mutex<u32>>,
    pub texture_cache: Arc<Mutex<HashMap<(u32, String), u32>>>,
    /// Path intern for `load_texture_ex`. Not the rig `(id, part)` map.
    pub path_texture_cache: Arc<Mutex<HashMap<(String, bool), u32>>>,
    pub texture_refcount: Arc<Mutex<HashMap<u32, u32>>>,

    pub transparent: bool,
    pub decorations: bool,
    pub always_on_top: bool,
    pub visible: bool,
    pub window_commands: Arc<Mutex<Vec<WindowCommand>>>,

    /// エージェント検証: 次の update 前に適用
    pub inject_queue: Arc<Mutex<Vec<InjectEvent>>>,
    /// 描画完了後に PNG 保存（1フレーム分）
    pub pending_screenshot: Arc<Mutex<Option<String>>>,
    /// 毎フレーム GPU readback（仮想カメラ）。720p 推奨。
    pub grab_frames: Arc<AtomicBool>,
    pub last_frame: Arc<Mutex<Option<(u32, u32, Vec<u8>)>>>,
    pub exit_requested: Arc<AtomicBool>,
    pub frame_count: Arc<AtomicU64>,
    pub pad: Arc<Mutex<crate::pad::PadHw>>,
}

impl KagraWindow {
    pub fn new(
        width: u32,
        height: u32,
        title: &str,
        fps: u32,
        transparent: bool,
        decorations: bool,
        always_on_top: bool,
        visible: bool,
    ) -> Result<Self, String> {
        Ok(KagraWindow {
            width,
            height,
            target_fps: fps,
            current_fps_atomic: Arc::new(AtomicU32::new(0)),
            title: title.to_string(),
            input: Arc::new(Mutex::new(InputState::new())),
            draw_queue: Arc::new(Mutex::new(Vec::new())),
            renderer: Arc::new(Mutex::new(None)),
            rigs: Arc::new(Mutex::new(HashMap::new())),
            next_rig_id: Arc::new(Mutex::new(1)),
            texture_cache: Arc::new(Mutex::new(HashMap::new())),
            path_texture_cache: Arc::new(Mutex::new(HashMap::new())),
            texture_refcount: Arc::new(Mutex::new(HashMap::new())),
            transparent,
            decorations,
            always_on_top,
            visible,
            window_commands: Arc::new(Mutex::new(Vec::new())),
            inject_queue: Arc::new(Mutex::new(Vec::new())),
            pending_screenshot: Arc::new(Mutex::new(None)),
            grab_frames: Arc::new(AtomicBool::new(false)),
            last_frame: Arc::new(Mutex::new(None)),
            exit_requested: Arc::new(AtomicBool::new(false)),
            frame_count: Arc::new(AtomicU64::new(0)),
            pad: Arc::new(Mutex::new(crate::pad::PadHw::default())),
        })
    }

    pub fn request_exit(&self) {
        self.exit_requested.store(true, Ordering::SeqCst);
    }

    pub fn request_screenshot(&self, path: &str) {
        *lock_recover(&self.pending_screenshot) = Some(path.to_string());
    }

    pub fn set_grab_frames(&self, enabled: bool) {
        self.grab_frames.store(enabled, Ordering::SeqCst);
        if !enabled {
            *lock_recover(&self.last_frame) = None;
        }
    }

    pub fn grab_frame(&self) -> Option<(u32, u32, Vec<u8>)> {
        lock_recover(&self.last_frame).take()
    }

    pub fn queue_inject(&self, event: InjectEvent) {
        lock_recover(&self.inject_queue).push(event);
    }

    pub fn pad_axis(&self, stick: u32) -> (f32, f32) {
        lock_recover(&self.pad).axis(stick)
    }

    pub fn pad_down(&self, name: &str) -> bool {
        lock_recover(&self.pad).down(name)
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count.load(Ordering::Relaxed)
    }

    // ========== 入力系メソッド（変更なし）==========
    pub fn is_key_down(&self, code: u32) -> bool { lock_recover(&self.input).is_key_down(code) }
    pub fn is_key_pressed(&self, code: u32) -> bool { lock_recover(&self.input).is_key_pressed(code) }
    pub fn is_key_released(&self, code: u32) -> bool { lock_recover(&self.input).is_key_released(code) }

    pub fn mouse_pos(&self) -> (f32, f32) { lock_recover(&self.input).mouse_pos() }
    pub fn mouse_delta(&self) -> (f32, f32) { lock_recover(&self.input).mouse_delta() }
    pub fn is_mouse_down(&self, btn: u32) -> bool { lock_recover(&self.input).is_mouse_down(btn) }
    pub fn is_mouse_pressed(&self, btn: u32) -> bool { lock_recover(&self.input).is_mouse_pressed(btn) }
    pub fn is_mouse_released(&self, btn: u32) -> bool { lock_recover(&self.input).is_mouse_released(btn) }
    pub fn mouse_wheel(&self) -> (f32, f32) { lock_recover(&self.input).mouse_wheel() }

    // ========== 描画コマンドキューイング ==========
    pub fn cls(&self, r: u8, g: u8, b: u8) {
        let a = if self.transparent { 0 } else { 255 };
        lock_recover(&self.draw_queue).push(WindowDrawCommand::Clear(Color { r, g, b, a }));
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        lock_recover(&self.draw_queue).push(WindowDrawCommand::Draw(DrawCommand::Rect(RectCommand { x, y, w, h, color })));
    }

    pub fn polygon(&self, verts: Vec<[f32; 2]>, color: Color) {
        lock_recover(&self.draw_queue).push(WindowDrawCommand::Draw(DrawCommand::Polygon(PolygonCommand { verts, color })));
    }

    pub fn draw_mesh(&self, texture_id: u32, verts: Vec<[f32;5]>, shader_id: u32, shader_params: [f32;4]) {
        use crate::renderer::{DrawCommand, MeshCommand};
        lock_recover(&self.draw_queue).push(WindowDrawCommand::Draw(DrawCommand::Mesh(MeshCommand { texture_id, verts, shader_id, shader_params })));
    }

    pub fn update_camera_3d(&self, view: [f32; 16], proj: [f32; 16]) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.update_camera_3d(&view, &proj);
        }
    }

    pub fn set_light_dir(&self, x: f32, y: f32, z: f32) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.set_light_dir(x, y, z);
        }
    }

    pub fn set_rim(&self, intensity: f32) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.set_rim(intensity);
        }
    }

    pub fn set_shadow_enabled(&self, enabled: bool) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.set_shadow_enabled(enabled);
        }
    }

    pub fn set_shadow_cascades(&self, count: u32) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.set_shadow_cascades(count);
        }
    }

    pub fn set_toon_params(&self, threshold: f32, softness: f32, shade: f32, lit: f32) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.set_toon_params(threshold, softness, shade, lit);
        }
    }

    pub fn set_fog(&self, start: f32, end: f32, r: u8, g: u8, b: u8, enabled: bool) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_fog(start, end, r, g, b, enabled);
        }
    }

    pub fn set_bloom(&self, threshold: f32, intensity: f32) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.set_bloom(threshold, intensity);
        }
    }

    pub fn set_ambient(&self, r: f32, g: f32, b: f32, strength: f32) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_ambient(r, g, b, strength);
        }
    }

    pub fn set_point_light(
        &self,
        x: f32, y: f32, z: f32,
        r: f32, g: f32, b: f32,
        intensity: f32, radius: f32,
        slot: u32,
    ) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_point_light(x, y, z, r, g, b, intensity, radius, slot);
        }
    }

    pub fn set_spot_light(
        &self,
        x: f32, y: f32, z: f32,
        dx: f32, dy: f32, dz: f32,
        angle: f32, penumbra: f32,
        intensity: f32, radius: f32,
        r: f32, g: f32, b: f32,
        slot: u32,
    ) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_spot_light(x, y, z, dx, dy, dz, angle, penumbra, intensity, radius, r, g, b, slot);
        }
    }

    pub fn set_cursor_locked(&self, locked: bool) {
        lock_recover(&self.window_commands).push(WindowCommand::SetCursorLocked(locked));
    }

    pub fn set_exposure(&self, value: f32) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_exposure(value);
        }
    }

    pub fn set_tonemap(&self, enabled: bool) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_tonemap(enabled);
        }
    }

    pub fn set_hdri(&self, path: &str, strength: f32) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_hdri(path, strength);
        }
    }

    pub fn set_mesh_pbr(&self, mesh_id: u32, metallic: f32, roughness: f32, br: f32, bg: f32, bb: f32) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_mesh_pbr(mesh_id, metallic, roughness, br, bg, bb);
        }
    }

    pub fn set_mesh_normal(&self, mesh_id: u32, texture_id: u32) {
        if let Some(rend) = lock_recover(&self.renderer).as_mut() {
            rend.set_mesh_normal(mesh_id, texture_id);
        }
    }

    pub fn set_mesh_cull(&self, enabled: bool) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.set_mesh_cull(enabled);
        }
    }

    pub fn render_stats(&self) -> (u32, u32, u32) {
        if let Some(r) = lock_recover(&self.renderer).as_ref() {
            let s = r.render_stats();
            (s.draw_calls, s.triangles, s.culled)
        } else {
            (0, 0, 0)
        }
    }

    pub fn queue_skinned_mesh_3d(&self, cmd: crate::renderer::SkinnedMeshCommand) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.queue_skinned_mesh_3d(cmd);
        }
    }

    pub fn queue_mesh_3d(&self, texture_id: u32, verts: Vec<[f32; 8]>, indices: Vec<u32>) {
        use crate::renderer::Mesh3DCommand;
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.queue_mesh_3d(Mesh3DCommand {
                texture_id,
                verts,
                indices,
                metallic: 0.0,
                roughness: 1.0,
                base_color: [1.0, 1.0, 1.0],
                skip_fog: false,
            });
        }
    }

    pub fn upload_mesh_3d(
        &self,
        texture_id: u32,
        verts: Vec<[f32; 8]>,
        indices: Vec<u32>,
        metallic: f32,
        roughness: f32,
        base_color: [f32; 3],
        normal_texture_id: u32,
    ) -> u32 {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.upload_mesh_3d(
                texture_id, verts, indices, metallic, roughness, base_color, normal_texture_id,
            )
        } else {
            0
        }
    }

    pub fn queue_retained_mesh_3d(&self, mesh_id: u32) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.queue_retained_mesh_3d(mesh_id);
        }
    }

    pub fn queue_mesh_instances(&self, mesh_id: u32, instances: Vec<crate::renderer::Instance3D>) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.queue_mesh_instances(mesh_id, instances);
        }
    }

    pub fn unload_mesh_3d(&self, mesh_id: u32) {
        if let Some(r) = lock_recover(&self.renderer).as_mut() {
            r.unload_mesh_3d(mesh_id);
        }
    }

    pub fn draw_texture_ex(
        &self, id: u32, x: f32, y: f32,
        w: Option<f32>, h: Option<f32>,
        sx: f32, sy: f32, sw: Option<f32>, sh: Option<f32>,
        alpha: f32, rotation_deg: f32,
        pivot_x: f32, pivot_y: f32,
        flip_x: bool, flip_y: bool,
        shader_id: u32, shader_params: [f32;4],
    ) {
        lock_recover(&self.draw_queue).push(WindowDrawCommand::Draw(
            DrawCommand::Sprite(SpriteCommand {
                texture_id: id, shader_id, shader_params,
                dx: x, dy: y,
                dw: w.unwrap_or(0.0), dh: h.unwrap_or(0.0),
                sx, sy,
                sw: sw.unwrap_or(0.0), sh: sh.unwrap_or(0.0),
                alpha: alpha.clamp(0.0, 1.0),
                rotation_deg, pivot_x, pivot_y, flip_x, flip_y,
            }),
        ));
    }

    #[allow(dead_code)]
    pub fn draw_texture(&self, id: u32, x: f32, y: f32, w: Option<f32>, h: Option<f32>, sx: f32, sy: f32, sw: Option<f32>, sh: Option<f32>, alpha: f32, rotation_deg: f32, pivot_x: f32, pivot_y: f32, flip_x: bool, flip_y: bool) {
        self.draw_texture_ex(id, x, y, w, h, sx, sy, sw, sh, alpha, rotation_deg, pivot_x, pivot_y, flip_x, flip_y, 0, [1.0;4]);
    }

    pub fn draw_text(&self, font_id: u32, text: &str, x: f32, y: f32, size_px: u32, r: u8, g: u8, b: u8, a: u8) {
        lock_recover(&self.draw_queue).push(WindowDrawCommand::Draw(DrawCommand::Text(TextCommand { font_id, text: text.to_string(), x, y, size_px, color: Color { r, g, b, a } })));
    }

    pub fn queue_skinned_mesh(&self, cmd: SkinnedMeshCommand) {
        lock_recover(&self.draw_queue).push(WindowDrawCommand::Draw(DrawCommand::SkinnedMesh(cmd)));
    }

    // ========== テクスチャ / フォント ==========
    pub fn load_texture(&self, path: &str) -> Result<u32, String> {
        self.load_texture_ex(path, true)
    }

    pub fn load_texture_ex(&self, path: &str, srgb: bool) -> Result<u32, String> {
        let intern_key = (path.to_string(), srgb);
        {
            let cache = lock_recover(&self.path_texture_cache);
            if let Some(&id) = cache.get(&intern_key) {
                if self.texture_size(id).is_some() {
                    let mut rc = lock_recover(&self.texture_refcount);
                    *rc.entry(id).or_insert(0) += 1;
                    return Ok(id);
                }
            }
        }
        let id = match lock_recover(&self.renderer).as_mut() {
            Some(r) => r.load_texture_ex(path, srgb)?,
            None => return Err("load_texture: run() 開始前です。update()/draw() 内で呼んでください。".into()),
        };
        lock_recover(&self.path_texture_cache).insert(intern_key, id);
        let mut rc = lock_recover(&self.texture_refcount);
        *rc.entry(id).or_insert(0) += 1;
        Ok(id)
    }

    pub fn texture_size(&self, id: u32) -> Option<(u32, u32)> {
        lock_recover(&self.renderer).as_ref()?.texture_size(id)
    }

    pub fn unload_texture(&self, id: u32) -> Result<(), String> {
        let mut rc = lock_recover(&self.texture_refcount);
        if let Some(count) = rc.get_mut(&id) {
            *count -= 1;
            if *count == 0 {
                rc.remove(&id);
                let mut cache = lock_recover(&self.texture_cache);
                cache.retain(|_, v| *v != id);
                lock_recover(&self.path_texture_cache).retain(|_, v| *v != id);
                if let Some(r) = lock_recover(&self.renderer).as_mut() {
                    r.unload_texture(id)?;
                }
            }
            Ok(())
        } else {
            Err(format!("Texture {} not found or already unloaded", id))
        }
    }

    pub fn load_font(&self, path: &str) -> Result<u32, String> {
        match lock_recover(&self.renderer).as_mut() {
            Some(r) => r.load_font(path),
            None => Err("load_font: run() 開始前です。update()/draw() 内で呼んでください。".into()),
        }
    }

    pub fn measure_text(&self, font_id: u32, text: &str, size_px: u32) -> (f32, f32) {
        match lock_recover(&self.renderer).as_mut() {
            Some(r) => r.measure_text(font_id, text, size_px),
            None => (0.0, 0.0),
        }
    }

    // ========== リグ ==========
    pub fn load_rig(&self, path: &str) -> Result<u32, String> {
        let rig = {
            let renderer_guard = lock_recover(&self.renderer);
            let device = match renderer_guard.as_ref() {
                Some(r) => &r.device,
                None => return Err("load_rig: Renderer 未初期化。update()/draw() 内で呼んでください。".into()),
            };
            let mut rig = rig::load_rig(path).map_err(|e| format!("リグ読み込み失敗: {}", e))?;
            rig::build_gpu_cache(&mut rig, device).map_err(|e| format!("GPUキャッシュ構築失敗: {}", e))?;
            rig
        };
        let mut id_guard = lock_recover(&self.next_rig_id);
        let id = *id_guard;
        *id_guard += 1;
        drop(id_guard);
        lock_recover(&self.rigs).insert(id, rig);
        log::info!("load_rig: id={} path='{}'", id, path);
        Ok(id)
    }

    pub fn with_rig<F, T>(&self, id: u32, f: F) -> Option<T>
    where F: FnOnce(&Rig) -> T,
    {
        let guard = lock_recover(&self.rigs);
        guard.get(&id).map(f)
    }

    pub fn get_cached_texture(&self, rig_id: u32, part_name: &str, loader: impl FnOnce() -> Result<u32, String>) -> Result<u32, String> {
        let key = (rig_id, part_name.to_string());
        if let Some(&id) = lock_recover(&self.texture_cache).get(&key) {
            return Ok(id);
        }
        let id = loader()?;
        lock_recover(&self.texture_cache).insert(key, id);
        let mut rc = lock_recover(&self.texture_refcount);
        *rc.entry(id).or_insert(0) += 1;
        Ok(id)
    }

    pub fn unload_rig_textures(&self, rig_id: u32) {
        let keys: Vec<(u32, String)> = {
            let cache = lock_recover(&self.texture_cache);
            cache.iter().filter_map(|((id, name), &_tex_id)| if *id == rig_id { Some((*id, name.clone())) } else { None }).collect()
        };
        for (_, part_name) in keys {
            let key = (rig_id, part_name);
            let _tex_id = { let mut cache = lock_recover(&self.texture_cache); cache.remove(&key) };
            if let Some(_tex_id) = _tex_id {
                let _ = self.unload_texture(_tex_id);
            }
        }
    }

    // ========== メインループ ==========
    pub fn run(
        &self,
        py: Python<'_>,
        update_fn: PyObject,
        draw_fn: PyObject,
        max_frames: Option<u64>,
        fixed_dt: Option<f64>,
    ) -> PyResult<()> {
        self.exit_requested.store(false, Ordering::SeqCst);
        self.frame_count.store(0, Ordering::Relaxed);

        let event_loop = EventLoop::new().map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let window = WindowBuilder::new()
            .with_title(&self.title)
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height))
            .with_transparent(self.transparent)
            .with_decorations(if self.visible { self.decorations } else { false })
            // Windows では完全非表示だと Redraw / DXGI present が止まることがあるため、
            // visible=false でも一旦表示し、直後に画面外へ退避する。
            .with_visible(true)
            .with_window_level(if self.always_on_top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal })
            .build(&event_loop)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let window_arc = Arc::new(window);
        if !self.visible {
            window_arc.set_outer_position(winit::dpi::PhysicalPosition::new(-12800, -12800));
        }

        // ★ RendererV2 が Surface を内部で作成する
        let renderer = pollster::block_on(RendererV2::new(window_arc.clone(), self.width, self.height, self.transparent))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;

        // Games (Crest Isle WASD) must not go through IME: JP Windows maps
        // key-up to VK_PROCESSKEY / Unidentified and the avatar keeps walking.
        // Text fields opt in via set_ime_cursor_pos → SetImeAllowed(true).
        window_arc.set_ime_allowed(false);
        window_arc.set_ime_cursor_area(
            winit::dpi::PhysicalPosition::new(100, 600),
            winit::dpi::PhysicalSize::new(400, 30),
        );

        *lock_recover(&self.renderer) = Some(renderer);

        let input_ref = Arc::clone(&self.input);
        let queue_ref = Arc::clone(&self.draw_queue);
        let renderer_ref = Arc::clone(&self.renderer);
        let fps_atomic = Arc::clone(&self.current_fps_atomic);
        let window_cmds_ref = Arc::clone(&self.window_commands);
        let inject_ref = Arc::clone(&self.inject_queue);
        let screenshot_ref = Arc::clone(&self.pending_screenshot);
        let grab_ref = Arc::clone(&self.grab_frames);
        let last_frame_ref = Arc::clone(&self.last_frame);
        let exit_ref = Arc::clone(&self.exit_requested);
        let frame_count_ref = Arc::clone(&self.frame_count);
        let pad_ref = Arc::clone(&self.pad);
        let mut gilrs = gilrs::Gilrs::new().ok();

        let frame_duration = Duration::from_secs_f64(1.0 / self.target_fps as f64);
        // max_frames 指定時はフレーム待ちをせず全速で回す（エージェント検証向け）
        let uncapped = max_frames.is_some() || fixed_dt.is_some();
        let mut last_time = Instant::now();
        let mut next_frame_time = Instant::now();
        let mut fps_timer = Instant::now();
        let mut fps_count = 0u32;
        let mut frames_done: u64 = 0;

        let run_result = Arc::new(Mutex::new(Ok::<(), PyErr>(())));
        let run_result_inner = Arc::clone(&run_result);

        // 隠れウィンドウでも最初のフレームが必ず回るように即 redraw 要求
        window_arc.request_redraw();

        event_loop
            .run(move |event, elwt| {
                if lock_recover(&run_result_inner).is_err() || exit_ref.load(Ordering::SeqCst) {
                    elwt.exit();
                    return;
                }

                // 検証モードは待ち無し。通常は次フレーム時刻まで Wait
                if uncapped {
                    elwt.set_control_flow(ControlFlow::Poll);
                } else {
                    elwt.set_control_flow(ControlFlow::WaitUntil(next_frame_time));
                }

                let window = &*window_arc;

                match event {
                    Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => elwt.exit(),
                    Event::WindowEvent { event: WindowEvent::Ime(ime_event), .. } => {
                        let mut inp = lock_recover(&input_ref);
                        match ime_event {
                            Ime::Preedit(text, cursor) => inp.on_preedit(&text, cursor),
                            Ime::Commit(text) => inp.on_commit(&text),
                            Ime::Enabled => inp.release_all(),
                            Ime::Disabled => { inp.preedit_text.clear(); inp.preedit_cursor = None; },
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::KeyboardInput { event: key_ev, .. }, .. } => {
                        let KeyEvent { physical_key, logical_key: ref lkey, state, repeat, .. } = key_ev;
                        let down = matches!(state, ElementState::Pressed);
                        let mut inp = lock_recover(&input_ref);
                        if let Some(code) = inp.ingest_key(physical_key, lkey, down, repeat) {
                            if down {
                                match code {
                                    KeyCode::Backspace => inp.set_backspace_pressed(),
                                    KeyCode::Enter => inp.set_enter_pressed(),
                                    KeyCode::Escape => inp.set_escape_pressed(),
                                    _ => {}
                                }
                                if let Key::Character(ref s) = lkey {
                                    if inp.preedit_text.is_empty() {
                                        for ch in s.chars() { if ch >= ' ' { inp.on_char(ch); } }
                                    }
                                }
                            }
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                        lock_recover(&input_ref).on_mouse_move(position.x as f32, position.y as f32);
                    },
                    Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                        lock_recover(&input_ref).on_mouse_delta(delta.0 as f32, delta.1 as f32);
                    },
                    Event::DeviceEvent { event: DeviceEvent::Key(raw), .. } => {
                        // Pointer-lock can drop WindowEvent key-up. Apply raw
                        // *releases* only — DeviceEvent repeats have no
                        // `repeat` flag and would re-hold after #71.
                        if matches!(raw.state, ElementState::Released) {
                            let mut inp = lock_recover(&input_ref);
                            if inp.focused {
                                let logical = Key::Unidentified(NativeKey::Unidentified);
                                let _ = inp.ingest_key(raw.physical_key, &logical, false, false);
                            }
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::Focused(gained), .. } => {
                        let mut inp = lock_recover(&input_ref);
                        inp.focused = gained;
                        if !gained {
                            inp.release_all();
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::MouseInput { state, button, .. }, .. } => {
                        let btn = match button {
                            MouseButton::Left => 1,
                            MouseButton::Right => 2,
                            MouseButton::Middle => 3,
                            _ => return,
                        };
                        let mut inp = lock_recover(&input_ref);
                        match state {
                            ElementState::Pressed => {
                                inp.on_mouse_down(btn);
                                let _ = window.focus_window();
                            }
                            ElementState::Released => inp.on_mouse_up(btn),
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } => {
                        let (dx, dy) = match delta {
                            MouseScrollDelta::LineDelta(x, y) => (x, y),
                            MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                        };
                        lock_recover(&input_ref).on_mouse_wheel(dx, dy);
                    },
                    Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                        let w = size.width.max(1);
                        let h = size.height.max(1);
                        if let Some(r) = lock_recover(&renderer_ref).as_mut() {
                            r.resize(w, h);
                        }
                    },
                    Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                        if exit_ref.load(Ordering::SeqCst) {
                            elwt.exit();
                            return;
                        }

                        // ウィンドウコマンド実行
                        {
                            let mut cmds = lock_recover(&window_cmds_ref);
                            for cmd in cmds.drain(..) {
                                match cmd {
                                    WindowCommand::Focus => { let _ = window.focus_window(); }
                                    WindowCommand::Drag => { let _ = window.drag_window(); }
                                    WindowCommand::SetPosition(x, y) => { window.set_outer_position(winit::dpi::PhysicalPosition::new(x, y)); }
                                    WindowCommand::SetClickThrough(click_through) => {
                                        #[cfg(any(target_os = "windows", target_os = "macos"))] { let _ = window.set_cursor_hittest(!click_through); }
                                        #[cfg(not(any(target_os = "windows", target_os = "macos")))] { if click_through { log::warn!("SetClickThrough not supported"); } }
                                    }
                                    WindowCommand::SetAlwaysOnTop(always_on_top) => {
                                        window.set_window_level(if always_on_top { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal });
                                    }
                                    WindowCommand::SetDecorations(decorations) => { window.set_decorations(decorations); }
                                    WindowCommand::SetTitle(title) => { window.set_title(&title); }
                                    WindowCommand::SetCursorLocked(locked) => {
                                        window.set_cursor_visible(!locked);
                                        let mode = if locked {
                                            CursorGrabMode::Locked
                                        } else {
                                            CursorGrabMode::None
                                        };
                                        if window.set_cursor_grab(mode).is_err() && locked {
                                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                                        }
                                    }
                                    WindowCommand::SetImeAllowed(allowed) => {
                                        if allowed {
                                            lock_recover(&input_ref).release_all();
                                        }
                                        window.set_ime_allowed(allowed);
                                    }
                                }
                            }
                        }

                        // USB / XInput: same thread as EventLoop (Windows: one loop).
                        if let Some(g) = gilrs.as_mut() {
                            crate::pad::pump(g, &mut lock_recover(&pad_ref));
                        }

                        // 注入入力を update の直前に適用（pressed エッジがこのフレームで見える）
                        {
                            let events: Vec<InjectEvent> = lock_recover(&inject_ref).drain(..).collect();
                            if !events.is_empty() {
                                let mut inp = lock_recover(&input_ref);
                                for ev in events {
                                    apply_inject(&mut inp, ev);
                                }
                            }
                        }

                        let now = Instant::now();
                        let dt = if let Some(fixed) = fixed_dt {
                            fixed
                        } else {
                            (now - last_time).as_secs_f64()
                        };
                        last_time = now;

                        // Python 更新
                        if let Err(e) = update_fn.call1(py, (dt,)) {
                            *lock_recover(&run_result_inner) = Err(e);
                            elwt.exit();
                            return;
                        }
                        if exit_ref.load(Ordering::SeqCst) {
                            elwt.exit();
                            return;
                        }
                        if let Err(e) = draw_fn.call0(py) {
                            *lock_recover(&run_result_inner) = Err(e);
                            elwt.exit();
                            return;
                        }

                        let screenshot_path = lock_recover(&screenshot_ref).take();

                        // コマンドをレンダラーに転送
                        {
                            let mut rend = lock_recover(&renderer_ref);
                            let renderer = match rend.as_mut() {
                                Some(r) => r,
                                None => return,
                            };
                            for cmd in lock_recover(&queue_ref).drain(..) {
                                match cmd {
                                    WindowDrawCommand::Clear(color) => renderer.clear_color = color,
                                    WindowDrawCommand::Draw(draw_cmd) => renderer.queue_command(draw_cmd),
                                }
                            }
                        }

                        let mut rend = lock_recover(&renderer_ref);
                        let renderer = match rend.as_mut() {
                            Some(r) => r,
                            None => return,
                        };
                        let grab = grab_ref.load(Ordering::Relaxed);
                        match renderer.render(screenshot_path.as_deref(), grab) {
                            Ok(pixels) => {
                                if let Some(frame) = pixels {
                                    *lock_recover(&last_frame_ref) = Some(frame);
                                }
                            }
                            Err(KaguraError::Gpu(msg)) => {
                                log::error!("GPU render error: {}", msg);
                                let (w, h) = (renderer.width(), renderer.height());
                                renderer.resize(w, h);
                            }
                            Err(e) => {
                                log::error!("Render error: {}", e);
                                let (w, h) = (renderer.width(), renderer.height());
                                renderer.resize(w, h);
                            }
                        }

                        frames_done += 1;
                        frame_count_ref.store(frames_done, Ordering::Relaxed);
                        fps_count += 1;
                        if fps_timer.elapsed() >= Duration::from_secs(1) {
                            fps_atomic.store(fps_count, Ordering::Relaxed);
                            fps_count = 0;
                            fps_timer = Instant::now();
                        }
                        next_frame_time = now + frame_duration;
                        lock_recover(&input_ref).begin_frame();

                        if let Some(max) = max_frames {
                            if frames_done >= max {
                                elwt.exit();
                                return;
                            }
                        }
                        if exit_ref.load(Ordering::SeqCst) {
                            elwt.exit();
                            return;
                        }
                        // 隠れウィンドウ + Poll でもフレームが途切れないように明示要求
                        if uncapped {
                            window.request_redraw();
                        }
                    },
                    Event::AboutToWait => {
                        if exit_ref.load(Ordering::SeqCst) {
                            elwt.exit();
                        } else if uncapped {
                            // 隠れウィンドウでも必ずフレームが進むよう AboutToWait から駆動
                            window.request_redraw();
                        } else if Instant::now() >= next_frame_time {
                            window.request_redraw();
                        }
                    },
                    _ => {}
                }
            })
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Arc::try_unwrap(run_result)
            .map_err(|_| pyo3::exceptions::PyRuntimeError::new_err("run_result unwrap 失敗"))?
            .into_inner()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
    }
}

fn apply_inject(inp: &mut InputState, ev: InjectEvent) {
    match ev {
        InjectEvent::KeyDown(code) => {
            inp.on_key_down(code);
            if code == KeyCode::Backspace as u32 {
                inp.set_backspace_pressed();
            } else if code == KeyCode::Enter as u32 {
                inp.set_enter_pressed();
            } else if code == KeyCode::Escape as u32 {
                inp.set_escape_pressed();
            }
        }
        InjectEvent::KeyUp(code) => inp.on_key_up(code),
        InjectEvent::MouseMove(x, y) => inp.on_mouse_move(x, y),
        InjectEvent::MouseDown(btn) => inp.on_mouse_down(btn),
        InjectEvent::MouseUp(btn) => inp.on_mouse_up(btn),
    }
}
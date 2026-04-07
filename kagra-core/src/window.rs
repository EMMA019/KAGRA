// kagra-core/src/window.rs
// winit イベントループ + Renderer ブリッジ
// Phase 2: Window側も DrawCommand を一本化

use pyo3::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winit::{
    event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::EventLoop,
    keyboard::PhysicalKey,
    window::WindowBuilder,
};

use crate::color::Color;
use crate::input::InputState;
use crate::renderer::{
    DrawCommand, RectCommand, Renderer, SkinnedMeshCommand, SpriteCommand, TextCommand,
};
use crate::rig::{self, Rig};

enum WindowDrawCommand {
    Clear(Color),
    Draw(DrawCommand),
}

pub struct KagraWindow {
    pub width: u32,
    pub height: u32,
    pub target_fps: u32,
    pub current_fps_atomic: Arc<AtomicU32>,
    title: String,

    pub input: Arc<Mutex<InputState>>,
    draw_queue: Arc<Mutex<Vec<WindowDrawCommand>>>,
    pub renderer: Arc<Mutex<Option<Renderer>>>,

    pub rigs: Arc<Mutex<HashMap<u32, Rig>>>,
    pub next_rig_id: Arc<Mutex<u32>>,
    pub texture_cache: Arc<Mutex<HashMap<(u32, String), u32>>>,
}

impl KagraWindow {
    pub fn new(width: u32, height: u32, title: &str, fps: u32) -> Result<Self, String> {
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
        })
    }

    pub fn is_key_down(&self, code: u32) -> bool { self.input.lock().unwrap().is_key_down(code) }
    pub fn is_key_pressed(&self, code: u32) -> bool { self.input.lock().unwrap().is_key_pressed(code) }
    pub fn is_key_released(&self, code: u32) -> bool { self.input.lock().unwrap().is_key_released(code) }

    pub fn mouse_pos(&self) -> (f32, f32) { self.input.lock().unwrap().mouse_pos() }
    pub fn is_mouse_down(&self, btn: u32) -> bool { self.input.lock().unwrap().is_mouse_down(btn) }
    pub fn is_mouse_pressed(&self, btn: u32) -> bool { self.input.lock().unwrap().is_mouse_pressed(btn) }
    pub fn is_mouse_released(&self, btn: u32) -> bool { self.input.lock().unwrap().is_mouse_released(btn) }
    pub fn mouse_wheel(&self) -> (f32, f32) { self.input.lock().unwrap().mouse_wheel() }

    pub fn cls(&self, r: u8, g: u8, b: u8) {
        self.draw_queue
            .lock()
            .unwrap()
            .push(WindowDrawCommand::Clear(Color { r, g, b, a: 255 }));
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.draw_queue.lock().unwrap().push(WindowDrawCommand::Draw(
            DrawCommand::Rect(RectCommand { x, y, w, h, color }),
        ));
    }

    pub fn draw_mesh(
        &self, texture_id: u32, verts: Vec<[f32;5]>,
        shader_id: u32, shader_params: [f32;4],
    ) {
        use crate::renderer::{DrawCommand, MeshCommand};
        self.draw_queue.lock().unwrap().push(
            WindowDrawCommand::Draw(DrawCommand::Mesh(MeshCommand {
                texture_id, verts, shader_id, shader_params,
            }))
        );
    }

    pub fn update_camera_3d(&self, view: [f32; 16], proj: [f32; 16]) {
        if let Some(r) = self.renderer.lock().unwrap().as_mut() {
            r.update_camera_3d(&view, &proj);
        }
    }

    pub fn queue_skinned_mesh_3d(&self, cmd: crate::renderer::SkinnedMeshCommand) {
        if let Some(r) = self.renderer.lock().unwrap().as_mut() {
            r.queue_skinned_mesh_3d(cmd);
        }
    }

    pub fn queue_mesh_3d(&self, texture_id: u32,
                         verts: Vec<[f32; 8]>, indices: Vec<u32>) {
        use crate::renderer::Mesh3DCommand;
        if let Some(r) = self.renderer.lock().unwrap().as_mut() {
            r.queue_mesh_3d(Mesh3DCommand { texture_id, verts, indices });
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
        self.draw_queue.lock().unwrap().push(WindowDrawCommand::Draw(
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
    pub fn draw_texture(
        &self, id: u32, x: f32, y: f32,
        w: Option<f32>, h: Option<f32>,
        sx: f32, sy: f32, sw: Option<f32>, sh: Option<f32>,
        alpha: f32, rotation_deg: f32,
        pivot_x: f32, pivot_y: f32,
        flip_x: bool, flip_y: bool,
    ) {
        self.draw_texture_ex(id, x, y, w, h, sx, sy, sw, sh,
            alpha, rotation_deg, pivot_x, pivot_y, flip_x, flip_y,
            0, [1.0;4]);
    }

    pub fn draw_text(
        &self,
        font_id: u32,
        text: &str,
        x: f32,
        y: f32,
        size_px: u32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) {
        self.draw_queue.lock().unwrap().push(WindowDrawCommand::Draw(
            DrawCommand::Text(TextCommand {
                font_id,
                text: text.to_string(),
                x,
                y,
                size_px,
                color: Color { r, g, b, a },
            }),
        ));
    }

    pub fn queue_skinned_mesh(&self, cmd: SkinnedMeshCommand) {
        self.draw_queue
            .lock()
            .unwrap()
            .push(WindowDrawCommand::Draw(DrawCommand::SkinnedMesh(cmd)));
    }

    pub fn load_texture(&self, path: &str) -> Result<u32, String> {
        match self.renderer.lock().unwrap().as_mut() {
            Some(r) => r.load_texture(path),
            None => Err("load_texture: run() 開始前です。update()/draw() 内で呼んでください。".into()),
        }
    }

    pub fn texture_size(&self, id: u32) -> Option<(u32, u32)> {
        self.renderer.lock().unwrap().as_ref()?.texture_size(id)
    }

    pub fn load_font(&self, path: &str) -> Result<u32, String> {
        match self.renderer.lock().unwrap().as_mut() {
            Some(r) => r.load_font(path),
            None => Err("load_font: run() 開始前です。update()/draw() 内で呼んでください。".into()),
        }
    }

    pub fn measure_text(&self, font_id: u32, text: &str, size_px: u32) -> (f32, f32) {
        match self.renderer.lock().unwrap().as_mut() {
            Some(r) => r.measure_text(font_id, text, size_px),
            None => (0.0, 0.0),
        }
    }

    pub fn load_rig(&self, path: &str) -> Result<u32, String> {
        let mut rig = rig::load_rig(path).map_err(|e| format!("リグ読み込み失敗: {}", e))?;

        {
            let renderer_guard = self.renderer.lock().unwrap();
            match renderer_guard.as_ref() {
                Some(r) => rig::build_gpu_cache(&mut rig, &r.device),
                None => log::warn!(
                    "load_rig: Renderer 未初期化。GPU キャッシュなし。update()/draw() 内で呼んでください。"
                ),
            }
        }

        let mut id_guard = self.next_rig_id.lock().unwrap();
        let id = *id_guard;
        *id_guard += 1;
        drop(id_guard);

        self.rigs.lock().unwrap().insert(id, rig);
        log::info!("load_rig: id={} path='{}'", id, path);
        Ok(id)
    }

    pub fn with_rig<F, T>(&self, id: u32, f: F) -> Option<T>
    where
        F: FnOnce(&Rig) -> T,
    {
        let guard = self.rigs.lock().unwrap();
        guard.get(&id).map(f)
    }

    pub fn get_cached_texture(
        &self,
        rig_id: u32,
        part_name: &str,
        loader: impl FnOnce() -> Result<u32, String>,
    ) -> Result<u32, String> {
        let key = (rig_id, part_name.to_string());
        if let Some(&id) = self.texture_cache.lock().unwrap().get(&key) {
            return Ok(id);
        }
        let id = loader()?;
        self.texture_cache.lock().unwrap().insert(key, id);
        Ok(id)
    }

    pub fn run(&self, py: Python<'_>, update_fn: PyObject, draw_fn: PyObject) -> PyResult<()> {
        let event_loop = EventLoop::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let window = WindowBuilder::new()
            .with_title(&self.title)
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height))
            .build(&event_loop)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        {
            let r = pollster::block_on(Renderer::new(&window, self.width, self.height))
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;
            *self.renderer.lock().unwrap() = Some(r);
        }

        let input_ref = Arc::clone(&self.input);
        let queue_ref = Arc::clone(&self.draw_queue);
        let renderer_ref = Arc::clone(&self.renderer);
        let fps_atomic = Arc::clone(&self.current_fps_atomic);

        let frame_duration = Duration::from_secs_f64(1.0 / self.target_fps as f64);
        let mut last_time = Instant::now();
        let mut next_frame_time = Instant::now();
        let mut fps_timer = Instant::now();
        let mut fps_count = 0u32;

        let run_result = Arc::new(Mutex::new(Ok::<(), PyErr>(() )));
        let run_result_inner = Arc::clone(&run_result);

        event_loop
            .run(move |event, elwt| {
                if run_result_inner.lock().unwrap().is_err() {
                    elwt.exit();
                    return;
                }

                match event {
                    Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                        elwt.exit();
                    }

                    Event::WindowEvent {
                        event: WindowEvent::KeyboardInput {
                            event: KeyEvent {
                                physical_key: PhysicalKey::Code(code),
                                state,
                                ..
                            },
                            ..
                        },
                        ..
                    } => {
                        let c = code as u32;
                        let mut inp = input_ref.lock().unwrap();
                        match state {
                            ElementState::Pressed => inp.on_key_down(c),
                            ElementState::Released => inp.on_key_up(c),
                        }
                    }

                    Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                        input_ref
                            .lock()
                            .unwrap()
                            .on_mouse_move(position.x as f32, position.y as f32);
                    }

                    Event::WindowEvent { event: WindowEvent::MouseInput { state, button, .. }, .. } => {
                        let btn = match button {
                            MouseButton::Left => 1,
                            MouseButton::Right => 2,
                            MouseButton::Middle => 3,
                            MouseButton::Back => 4,
                            MouseButton::Forward => 5,
                            MouseButton::Other(n) => n as u32 + 100,
                        };
                        let mut inp = input_ref.lock().unwrap();
                        match state {
                            ElementState::Pressed => inp.on_mouse_down(btn),
                            ElementState::Released => inp.on_mouse_up(btn),
                        }
                    }

                    Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } => {
                        let (dx, dy) = match delta {
                            MouseScrollDelta::LineDelta(x, y) => (x, y),
                            MouseScrollDelta::PixelDelta(p) => (p.x as f32, p.y as f32),
                        };
                        input_ref.lock().unwrap().on_mouse_wheel(dx, dy);
                    }

                    Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                        if let Some(r) = renderer_ref.lock().unwrap().as_mut() {
                            r.resize(size.width, size.height);
                        }
                    }

                    Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                        let now = Instant::now();
                        let dt = (now - last_time).as_secs_f64();
                        last_time = now;

                        if let Err(e) = update_fn.call1(py, (dt,)) {
                            *run_result_inner.lock().unwrap() = Err(e);
                            return;
                        }

                        queue_ref.lock().unwrap().clear();

                        if let Err(e) = draw_fn.call0(py) {
                            *run_result_inner.lock().unwrap() = Err(e);
                            return;
                        }

                        let mut rend = renderer_ref.lock().unwrap();
                        let renderer = match rend.as_mut() {
                            Some(r) => r,
                            None => return,
                        };

                        for cmd in queue_ref.lock().unwrap().drain(..) {
                            match cmd {
                                WindowDrawCommand::Clear(color) => {
                                    renderer.clear_color = color;
                                }
                                WindowDrawCommand::Draw(DrawCommand::Rect(rc)) => {
                                    renderer.queue_command(DrawCommand::Rect(rc));
                                }
                                WindowDrawCommand::Draw(DrawCommand::Sprite(mut sc)) => {
                                    if let Some((tw, th)) = renderer.texture_size(sc.texture_id) {
                                        if sc.sw == 0.0 { sc.sw = tw as f32; }
                                        if sc.sh == 0.0 { sc.sh = th as f32; }
                                        if sc.dw == 0.0 { sc.dw = sc.sw; }
                                        if sc.dh == 0.0 { sc.dh = sc.sh; }
                                    }
                                    renderer.queue_command(DrawCommand::Sprite(sc));
                                }
                                WindowDrawCommand::Draw(DrawCommand::Text(tc)) => {
                                    renderer.queue_command(DrawCommand::Text(tc));
                                }
                                WindowDrawCommand::Draw(DrawCommand::SkinnedMesh(cmd)) => {
                                    renderer.queue_command(DrawCommand::SkinnedMesh(cmd));
                                }
                                WindowDrawCommand::Draw(DrawCommand::Mesh(cmd)) => {
                                    renderer.queue_command(DrawCommand::Mesh(cmd));
                                }
                            }
                        }

                        match renderer.render() {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost) => {
                                let (w, h) = (renderer.width(), renderer.height());
                                renderer.resize(w, h);
                            }
                            Err(e) => {
                                *run_result_inner.lock().unwrap() =
                                    Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string()));
                                return;
                            }
                        }

                        fps_count += 1;
                        if fps_timer.elapsed() >= Duration::from_secs(1) {
                            fps_atomic.store(fps_count, Ordering::Relaxed);
                            fps_count = 0;
                            fps_timer = Instant::now();
                        }
                        next_frame_time = now + frame_duration;

                        input_ref.lock().unwrap().begin_frame();
                    }

                    Event::AboutToWait => {
                        if Instant::now() >= next_frame_time {
                            window.request_redraw();
                        }
                    }

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

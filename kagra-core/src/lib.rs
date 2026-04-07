// kagra-core/src/lib.rs
use pyo3::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

mod window;
mod renderer;
mod input;
mod color;
mod audio;
mod text;
mod rig;
mod vrm;
mod fbx_loader;

use window::KagraWindow;
use vrm::VrmModel;
use color::Color;
use audio::AudioEngine;
use nalgebra::Point3;
use crate::renderer::SkinnedMeshCommand;

// マウスボタン定数
const MOUSE_LEFT:   u32 = 1;
const MOUSE_RIGHT:  u32 = 2;
const MOUSE_MIDDLE: u32 = 3;

#[pyclass(unsendable)]
pub struct Engine {
    window:     KagraWindow,
    audio:      Arc<Mutex<Option<AudioEngine>>>,
    keymap:     Arc<Mutex<HashMap<String, u32>>>,
    vrm_models: Arc<Mutex<HashMap<u32, VrmModel>>>,
    next_vrm_id: Arc<Mutex<u32>>,
}

#[pymethods]
impl Engine {
    #[new]
    #[pyo3(signature = (width=1280, height=720, title="KAGRA", fps=60))]
    pub fn new(width: u32, height: u32, title: &str, fps: u32) -> PyResult<Self> {
        let _ = env_logger::try_init();
        let window = KagraWindow::new(width, height, title, fps)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;
        let audio_engine = match AudioEngine::new() {
            Ok(a)  => { log::info!("AudioEngine OK"); Some(a) }
            Err(e) => { log::warn!("AudioEngine失敗（音声なし）: {}", e); None }
        };

        let keymap = Self::load_or_create_keymap();

        Ok(Engine {
            window,
            audio: Arc::new(Mutex::new(audio_engine)),
            keymap: Arc::new(Mutex::new(keymap)),
            vrm_models:  Arc::new(Mutex::new(HashMap::new())),
            next_vrm_id: Arc::new(Mutex::new(1u32)),
        })
    }

    #[staticmethod]
    fn load_or_create_keymap() -> HashMap<String, u32> {
        let path = "keymap.json";
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(map) = serde_json::from_str(&content) {
                    return map;
                }
            }
        }
        // デフォルト HID 標準値
        let mut default = HashMap::new();
        default.insert("Z".to_string(), 29);
        default.insert("X".to_string(), 27);
        default.insert("D".to_string(), 7);
        default.insert("S".to_string(), 22);
        default.insert("SPACE".to_string(), 44);
        default.insert("RETURN".to_string(), 40);
        default.insert("ESCAPE".to_string(), 41);
        default.insert("UP".to_string(), 82);
        default.insert("DOWN".to_string(), 81);
        default.insert("LEFT".to_string(), 80);
        default.insert("RIGHT".to_string(), 79);
        default.insert("U".to_string(), 24);
        default.insert("I".to_string(), 12);
        default.insert("K".to_string(), 14);
        default.insert("J".to_string(), 13);
        default.insert("Y".to_string(), 28);
        default.insert("H".to_string(), 11);
        default.insert("T".to_string(), 23);
        default.insert("G".to_string(), 10);
        let _ = fs::write(path, serde_json::to_string_pretty(&default).unwrap());
        default
    }

    pub fn get_keymap(&self) -> HashMap<String, u32> {
        self.keymap.lock().unwrap().clone()
    }

    pub fn run(&self, py: Python<'_>, update_fn: PyObject, draw_fn: PyObject) -> PyResult<()> {
        self.window.run(py, update_fn, draw_fn)
    }

    #[getter]
    pub fn width(&self) -> u32 {
        self.window.width
    }

    #[getter]
    pub fn height(&self) -> u32 {
        self.window.height
    }

    #[getter]
    pub fn fps(&self) -> f64 {
        self.window.current_fps_atomic.load(Ordering::Relaxed) as f64
    }

    pub fn cls(&self, r: u8, g: u8, b: u8) {
        self.window.cls(r, g, b);
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8, a: u8) {
        self.window.rect(x, y, w, h, Color { r, g, b, a });
    }

    pub fn load_texture(&self, path: &str) -> PyResult<u32> {
        self.window.load_texture(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
    }

    pub fn texture_size(&self, id: u32) -> Option<(u32, u32)> {
        self.window.texture_size(id)
    }

    #[pyo3(signature = (view, proj))]
    pub fn update_camera_3d(&self, view: Vec<f32>, proj: Vec<f32>) {
        let v: [f32; 16] = view.try_into().unwrap_or([0.0; 16]);
        let p: [f32; 16] = proj.try_into().unwrap_or([0.0; 16]);
        self.window.update_camera_3d(v, p);
    }

    // ── VRM GPU スキニング API ────────────────────────────────

    /// VRM ファイルを Rust で読み込む（GPU スキニング）
    /// 戻り値: vrm_id（draw_vrm 等で使う）
    pub fn load_vrm(&self, path: &str) -> PyResult<u32> {
        use crate::vrm::{load_vrm as vrm_load, extract_texture_data};
        use std::collections::HashMap;

        // ── Pass 1: テクスチャバイト列を抽出（renderer 不要）──
        let tex_data = extract_texture_data(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;

        // ── Pass 2: テクスチャを renderer で読み込む ──────────
        let mut tex_id_map: HashMap<usize, u32> = HashMap::new();
        {
            let mut rg = self.window.renderer.lock().unwrap();
            let renderer = rg.as_mut()
                .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Renderer 未初期化"))?;

            for (ti, bytes, ext) in &tex_data {
                let tmp_name = format!("kagra_vrm_{}_{}.{}", rand_id(), ti, ext);
                let tmp = std::env::temp_dir().join(&tmp_name);
                if let Err(e) = std::fs::write(&tmp, bytes) {
                    log::warn!("VRM tex[{}] 書き込み失敗: {}", ti, e);
                    continue;
                }
                match renderer.load_texture(tmp.to_str().unwrap()) {
                    Ok(id) => { tex_id_map.insert(*ti, id); }
                    Err(e) => log::warn!("VRM tex[{}] 読み込み失敗: {}", ti, e),
                }
            }
        } // renderer の lock をここで解放

        // ── Pass 3: GPU バッファ構築（device のみ使用）────────
        let model = {
            let mut rg = self.window.renderer.lock().unwrap();
            let renderer = rg.as_mut()
                .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Renderer 未初期化"))?;
            vrm_load(path, &renderer.device, &tex_id_map)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?
        };

        let id = {
            let mut next = self.next_vrm_id.lock().unwrap();
            let id = *next; *next += 1; id
        };
        self.vrm_models.lock().unwrap().insert(id, model);
        log::info!("load_vrm: id={} path={}", id, path);
        Ok(id)
    }

    /// VRM を GPU スキニングで描画する
    pub fn draw_vrm(&self, vrm_id: u32) -> PyResult<()> {
        // 1. スキニングコマンドを生成（VrmModel の lock を解放するため先にまとめる）
        let cmds = {
            let mut models = self.vrm_models.lock().unwrap();
            let model = models.get_mut(&vrm_id).ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    format!("vrm_id={} が見つかりません", vrm_id))
            })?;
            model.build_draw_commands()
        };

        // 2. renderer に渡す（3D スキニングキューへ）
        let mut rg = self.window.renderer.lock().unwrap();
        if let Some(renderer) = rg.as_mut() {
            for (matrices, cmd) in cmds {
                renderer.update_skin_uniforms(&matrices);
                renderer.queue_skinned_mesh_3d(cmd);
            }
        }
        Ok(())
    }

    /// VRM のボーンをクォータニオンで回転させる
    #[pyo3(signature = (vrm_id, bone_name, qx=0.0, qy=0.0, qz=0.0, qw=1.0))]
    pub fn set_vrm_bone_rot(
        &self, vrm_id: u32, bone_name: &str,
        qx: f32, qy: f32, qz: f32, qw: f32,
    ) {
        if let Some(model) = self.vrm_models.lock().unwrap().get_mut(&vrm_id) {
            model.set_bone_rot_quat(bone_name, qx, qy, qz, qw);
        }
    }

    /// VRM のボーンをオイラー角（ラジアン）で回転させる
    #[pyo3(signature = (vrm_id, bone_name, rx=0.0, ry=0.0, rz=0.0))]
    pub fn set_vrm_bone_euler(
        &self, vrm_id: u32, bone_name: &str,
        rx: f32, ry: f32, rz: f32,
    ) {
        let (cx, sx) = ((rx/2.0).cos(), (rx/2.0).sin());
        let (cy, sy) = ((ry/2.0).cos(), (ry/2.0).sin());
        let (cz, sz) = ((rz/2.0).cos(), (rz/2.0).sin());
        let qx = sx*cy*cz + cx*sy*sz;
        let qy = cx*sy*cz - sx*cy*sz;
        let qz = cx*cy*sz + sx*sy*cz;
        let qw = cx*cy*cz - sx*sy*sz;
        self.set_vrm_bone_rot(vrm_id, bone_name, qx, qy, qz, qw);
    }

    /// VRM の全ボーンをバインドポーズに戻す
    pub fn reset_vrm_pose(&self, vrm_id: u32) {
        if let Some(model) = self.vrm_models.lock().unwrap().get_mut(&vrm_id) {
            model.reset_pose();
        }
    }

    /// FBX ファイルからアニメーションを読み込む
    /// 戻り値: [(clip_name, frame_time, frames), ...]
    /// frames: [[(bone_name, tx,ty,tz, qx,qy,qz,qw, has_trans), ...], ...]
    pub fn load_fbx_anim(&self, path: &str) -> PyResult<Vec<(String, f64, Vec<Vec<(String, f32,f32,f32, f32,f32,f32,f32, bool)>>)>> {
        use crate::fbx_loader::load_fbx_anim;
        let clips = load_fbx_anim(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))?;

        Ok(clips.into_iter().map(|clip| {
            let py_frames = clip.frames.into_iter().map(|frame| {
                frame.into_iter().map(|b| {
                    (b.name,
                     b.translation[0], b.translation[1], b.translation[2],
                     b.rotation[0], b.rotation[1], b.rotation[2], b.rotation[3],
                     b.has_trans)
                }).collect()
            }).collect();
            (clip.name, clip.frame_time, py_frames)
        }).collect())
    }

    /// VRM のルート位置オフセットを設定する（BVH の Root 位置に使用）
    #[pyo3(signature = (vrm_id, x=0.0, y=0.0, z=0.0))]
    pub fn set_vrm_offset(&self, vrm_id: u32, x: f32, y: f32, z: f32) {
        if let Some(model) = self.vrm_models.lock().unwrap().get_mut(&vrm_id) {
            model.root_offset = [x, y, z];
            model.dirty = true;
        }
    }

    /// VRM ボーンの並進（位置）を設定する
    #[pyo3(signature = (vrm_id, bone_name, tx=0.0, ty=0.0, tz=0.0))]
    pub fn set_vrm_bone_trans(
        &self, vrm_id: u32, bone_name: &str,
        tx: f32, ty: f32, tz: f32,
    ) {
        if let Some(model) = self.vrm_models.lock().unwrap().get_mut(&vrm_id) {
            model.set_bone_trans(bone_name, tx, ty, tz);
        }
    }

    /// VRM ボーンのスケールを設定する
    #[pyo3(signature = (vrm_id, bone_name, sx=1.0, sy=1.0, sz=1.0))]
    pub fn set_vrm_bone_scale(
        &self, vrm_id: u32, bone_name: &str,
        sx: f32, sy: f32, sz: f32,
    ) {
        if let Some(model) = self.vrm_models.lock().unwrap().get_mut(&vrm_id) {
            model.set_bone_scale(bone_name, sx, sy, sz);
        }
    }

    #[pyo3(signature = (texture_id, verts, indices))]
    pub fn draw_mesh_3d(&self, texture_id: u32, verts: Vec<Vec<f32>>, indices: Vec<u32>) {
        let cv: Vec<[f32; 8]> = verts.iter().map(|v| {
            let mut a = [0f32; 8];
            for (i, val) in v.iter().enumerate().take(8) { a[i] = *val; }
            a
        }).collect();
        self.window.queue_mesh_3d(texture_id, cv, indices);
    }

    #[pyo3(signature = (texture_id, verts, shader_id=0u32, shader_params=None))]
    pub fn draw_mesh(&self, texture_id: u32, verts: Vec<Vec<f32>>,
                     shader_id: u32, shader_params: Option<Vec<f32>>) {
        let params: [f32;4] = if let Some(p) = shader_params {
            [p.get(0).copied().unwrap_or(1.0), p.get(1).copied().unwrap_or(1.0),
             p.get(2).copied().unwrap_or(1.0), p.get(3).copied().unwrap_or(1.0)]
        } else { [1.0;4] };
        let cv: Vec<[f32;5]> = verts.iter().map(|v| [
            v.get(0).copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0),
            v.get(2).copied().unwrap_or(0.0), v.get(3).copied().unwrap_or(0.0),
            v.get(4).copied().unwrap_or(1.0),
        ]).collect();
        self.window.draw_mesh(texture_id, cv, shader_id, params);
    }

    #[pyo3(signature = (
        id, x, y,
        w=None, h=None,
        sx=0.0, sy=0.0, sw=None, sh=None,
        alpha=1.0, rotation_deg=0.0,
        pivot_x=0.5, pivot_y=0.5,
        flip_x=false, flip_y=false,
        shader_id=0u32, shader_params=None,
    ))]
    pub fn draw_texture(
        &self, id: u32, x: f32, y: f32,
        w: Option<f32>, h: Option<f32>,
        sx: f32, sy: f32, sw: Option<f32>, sh: Option<f32>,
        alpha: f32, rotation_deg: f32,
        pivot_x: f32, pivot_y: f32,
        flip_x: bool, flip_y: bool,
        shader_id: u32, shader_params: Option<Vec<f32>>,
    ) {
        let params: [f32;4] = if let Some(p) = shader_params {
            [p.get(0).copied().unwrap_or(1.0), p.get(1).copied().unwrap_or(1.0),
             p.get(2).copied().unwrap_or(1.0), p.get(3).copied().unwrap_or(1.0)]
        } else { [1.0;4] };
        self.window.draw_texture_ex(id, x, y, w, h, sx, sy, sw, sh,
            alpha, rotation_deg, pivot_x, pivot_y, flip_x, flip_y,
            shader_id, params);
    }

    pub fn load_shader(&self, path: &str) -> PyResult<u32> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(
                format!("シェーダー読み込み失敗: {} ({})", path, e)))?;
        let mut rg = self.window.renderer.lock().unwrap();
        match rg.as_mut() {
            Some(r) => r.load_shader_src(&src)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e)),
            None => Err(pyo3::exceptions::PyRuntimeError::new_err("Renderer 未初期化")),
        }
    }

    pub fn load_shader_src(&self, wgsl_src: &str) -> PyResult<u32> {
        let mut rg = self.window.renderer.lock().unwrap();
        match rg.as_mut() {
            Some(r) => r.load_shader_src(wgsl_src)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e)),
            None => Err(pyo3::exceptions::PyRuntimeError::new_err("Renderer 未初期化")),
        }
    }

    pub fn load_font(&self, path: &str) -> PyResult<u32> {
        self.window.load_font(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
    }

    #[pyo3(signature = (font_id, text, x, y, size=24, r=255, g=255, b=255, a=255))]
    pub fn draw_text(&self, font_id: u32, text: &str, x: f32, y: f32,
                     size: u32, r: u8, g: u8, b: u8, a: u8) {
        self.window.draw_text(font_id, text, x, y, size, r, g, b, a);
    }

    pub fn measure_text(&self, font_id: u32, text: &str, size: u32) -> (f32, f32) {
        self.window.measure_text(font_id, text, size)
    }

    pub fn key_down(&self, code: u32) -> bool {
        self.window.is_key_down(code)
    }

    pub fn key_pressed(&self, code: u32) -> bool {
        self.window.is_key_pressed(code)
    }

    pub fn key_released(&self, code: u32) -> bool {
        self.window.is_key_released(code)
    }

    pub fn collide_rect(&self, ax: f32, ay: f32, aw: f32, ah: f32,
                        bx: f32, by: f32, bw: f32, bh: f32) -> bool {
        ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
    }

    pub fn collide_rect_overlap(&self, ax: f32, ay: f32, aw: f32, ah: f32,
                                bx: f32, by: f32, bw: f32, bh: f32) -> Option<(f32, f32)> {
        let ox = (ax + aw).min(bx + bw) - ax.max(bx);
        let oy = (ay + ah).min(by + bh) - ay.max(by);
        if ox > 0.0 && oy > 0.0 { Some((ox, oy)) } else { None }
    }

    pub fn point_in_rect(&self, px: f32, py: f32, rx: f32, ry: f32, rw: f32, rh: f32) -> bool {
        px >= rx && px <= rx + rw && py >= ry && py <= ry + rh
    }

    #[pyo3(signature = (path, loop_=true, volume=0.8))]
    pub fn play_bgm(&self, path: &str, loop_: bool, volume: f32) -> PyResult<()> {
        self.with_audio(|a| a.play_bgm(path, loop_, volume))
    }

    pub fn stop_bgm(&self, fade: f32) {
        if let Some(a) = self.audio.lock().unwrap().as_ref() { a.stop_bgm(fade); }
    }
    pub fn pause_bgm(&self) {
        if let Some(a) = self.audio.lock().unwrap().as_ref() { a.pause_bgm(); }
    }
    pub fn resume_bgm(&self) {
        if let Some(a) = self.audio.lock().unwrap().as_ref() { a.resume_bgm(); }
    }
    pub fn set_bgm_volume(&self, volume: f32) {
        if let Some(a) = self.audio.lock().unwrap().as_ref() { a.set_bgm_volume(volume); }
    }
    #[pyo3(signature = (path, volume=1.0))]
    pub fn play_se(&self, path: &str, volume: f32) -> PyResult<()> {
        self.with_audio(|a| a.play_se(path, volume))
    }
    pub fn stop_all_se(&self) {
        if let Some(a) = self.audio.lock().unwrap().as_ref() { a.stop_all_se(); }
    }

    pub fn mouse_pos(&self) -> (f32, f32) { self.window.mouse_pos() }
    pub fn mouse_down(&self, btn: u32) -> bool { self.window.is_mouse_down(btn) }
    pub fn mouse_pressed(&self, btn: u32) -> bool { self.window.is_mouse_pressed(btn) }
    pub fn mouse_released(&self, btn: u32) -> bool { self.window.is_mouse_released(btn) }
    pub fn mouse_wheel(&self) -> (f32, f32) { self.window.mouse_wheel() }
    pub fn mouse_wheel_y(&self) -> f32 { self.window.input.lock().unwrap().wheel_y() }

    // UI 拡張
    #[pyo3(signature = (
        x, y, w, h, text,
        bg_r=70, bg_g=70, bg_b=90,
        hv_r=100, hv_g=100, hv_b=150,
        txt_r=255, txt_g=255, txt_b=255,
        font_size=20, font_id=1,
    ))]
    pub fn draw_ui_button(
        &self, x: f32, y: f32, w: f32, h: f32, text: String,
        bg_r: u8, bg_g: u8, bg_b: u8,
        hv_r: u8, hv_g: u8, hv_b: u8,
        txt_r: u8, txt_g: u8, txt_b: u8,
        font_size: u32, font_id: u32,
    ) -> PyResult<bool> {
        let (mx, my, is_clicked) = {
            let input = self.window.input.lock().unwrap();
            (input.mouse_pos().0, input.mouse_pos().1, input.is_mouse_pressed(MOUSE_LEFT))
        };
        let is_hovered = mx >= x && mx <= x + w && my >= y && my <= y + h;
        let is_clicked_on_button = is_hovered && is_clicked;
        let (cr, cg, cb) = if is_hovered { (hv_r, hv_g, hv_b) } else { (bg_r, bg_g, bg_b) };
        self.window.rect(x, y, w, h, Color { r: cr, g: cg, b: cb, a: 255 });
        let text_len = text.chars().count() as f32;
        let text_x = x + w/2.0 - text_len * font_size as f32 * 0.35;
        let text_y = y + h/2.0 - font_size as f32 * 0.5;
        self.window.draw_text(font_id, &text, text_x, text_y, font_size, txt_r, txt_g, txt_b, 255);
        Ok(is_clicked_on_button)
    }

    #[pyo3(signature = (x, y, w, h, max_val, current_val, bg_r=30, bg_g=30, bg_b=30, fl_r=50, fl_g=255, fl_b=50))]
    pub fn draw_ui_progress_bar(&self, x: f32, y: f32, w: f32, h: f32,
                                max_val: f32, current_val: f32,
                                bg_r: u8, bg_g: u8, bg_b: u8,
                                fl_r: u8, fl_g: u8, fl_b: u8) -> PyResult<()> {
        self.window.rect(x, y, w, h, Color { r: bg_r, g: bg_g, b: bg_b, a: 255 });
        let ratio = (current_val / max_val).clamp(0.0, 1.0);
        if ratio > 0.0 {
            self.window.rect(x, y, w * ratio, h, Color { r: fl_r, g: fl_g, b: fl_b, a: 255 });
        }
        Ok(())
    }

    // ── リグ ────────────────────────────────────────────────
    pub fn load_rig(&self, path: &str) -> PyResult<u32> {
        self.window.load_rig(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
    }

    pub fn draw_rig(&self, rig_id: u32, x: f32, y: f32) -> PyResult<()> {
        use std::sync::Arc;

        struct RigSnapshot {
            model_name: String,
            part_entries: Vec<(String, String)>,
            matrices: Vec<nalgebra::Matrix4<f32>>,
            gpu_parts: Vec<(String, Arc<wgpu::Buffer>, Arc<wgpu::Buffer>, u32)>,
        }

        let snapshot = {
            let rigs = self.window.rigs.lock().unwrap();
            let rig = match rigs.get(&rig_id) {
                Some(r) => r,
                None => {
                    log::warn!("draw_rig: rig_id={} not found", rig_id);
                    return Ok(());
                }
            };

            let rotations = vec![0.0f32; rig.bones.len()];
            let mut matrices = rig.compute_bone_matrices(&rotations);
            if !matrices.is_empty() {
                matrices[0][(0, 3)] += x;
                matrices[0][(1, 3)] += y;
            }

            let part_entries: Vec<(String, String)> = rig
                .parts
                .iter()
                .map(|(name, part)| (name.clone(), part.texture.clone()))
                .collect();

            let gpu_parts: Vec<_> = rig
                .gpu_cache
                .iter()
                .map(|(name, cache)| {
                    (
                        name.clone(),
                        Arc::clone(&cache.vertex_buffer),
                        Arc::clone(&cache.index_buffer),
                        cache.num_indices,
                    )
                })
                .collect();

            RigSnapshot {
                model_name: rig.model_name.clone(),
                part_entries,
                matrices,
                gpu_parts,
            }
        };

        let mut renderer_guard = self.window.renderer.lock().unwrap();
        let renderer = match renderer_guard.as_mut() {
            Some(r) => r,
            None => return Ok(()),
        };

        renderer.update_skin_uniforms(&snapshot.matrices);

        let base_path = format!("static/output/rigs/{}", snapshot.model_name);

        let gpu_map: std::collections::HashMap<&str, _> = snapshot
            .gpu_parts
            .iter()
            .map(|(name, vb, ib, n)| (name.as_str(), (vb, ib, *n)))
            .collect();

        for (part_name, part_texture) in &snapshot.part_entries {
            let tex_path = format!("{}/{}", base_path, part_texture);

            let tex_id = match self.window.get_cached_texture(rig_id, part_name, || {
                renderer.load_texture(&tex_path)
            }) {
                Ok(id) => id,
                Err(e) => {
                    log::warn!("draw_rig: texture load failed '{}': {}", part_name, e);
                    continue;
                }
            };

            if let Some((vb, ib, num_indices)) = gpu_map.get(part_name.as_str()) {
                let cmd = SkinnedMeshCommand {
                    texture_id:       tex_id,
                    vertex_buffer:    Arc::clone(vb),
                    index_buffer:     Arc::clone(ib),
                    num_indices:      *num_indices,
                    morph_bind_group: None,
                    morph_weights:    [0.0f32; 8],
                };
                self.window.queue_skinned_mesh(cmd);
            } else {
                log::debug!(
                    "draw_rig: no GPU cache for part '{}' (load_rig 後に確認)",
                    part_name
                );
            }
        }

        Ok(())
    }

    // ── IK 関連の公開メソッド ────────────────────────────────
    pub fn set_ik_target(&self, rig_id: u32, bone_name: &str, x: f32, y: f32, z: f32) -> PyResult<()> {
        let mut rigs = self.window.rigs.lock().unwrap();
        if let Some(rig) = rigs.get_mut(&rig_id) {
            rig.ik_targets.insert(bone_name.to_string(), Point3::new(x, y, z));
            rig.ik_enabled = true;
            Ok(())
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("Rig {} not found", rig_id)))
        }
    }

    pub fn disable_ik(&self, rig_id: u32) -> PyResult<()> {
        let mut rigs = self.window.rigs.lock().unwrap();
        if let Some(rig) = rigs.get_mut(&rig_id) {
            rig.ik_enabled = false;
            rig.ik_targets.clear();
            Ok(())
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("Rig {} not found", rig_id)))
        }
    }

    pub fn is_ik_enabled(&self, rig_id: u32) -> PyResult<bool> {
        let rigs = self.window.rigs.lock().unwrap();
        if let Some(rig) = rigs.get(&rig_id) {
            Ok(rig.ik_enabled)
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("Rig {} not found", rig_id)))
        }
    }
}

impl Engine {
    fn with_audio<F>(&self, f: F) -> PyResult<()>
    where F: FnOnce(&AudioEngine) -> Result<(), String>
    {
        match self.audio.lock().unwrap().as_ref() {
            Some(a) => f(a).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e)),
            None => Ok(()),
        }
    }
}

#[pymodule]
fn kagra_core(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Engine>()?;
    Ok(())
}

// ── ヘルパー ─────────────────────────────────────────────────

fn rand_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0)
}

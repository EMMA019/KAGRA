// src/engine/mod.rs
// ============================================================================
// 注意: このファイル内では以下のロック順序を守ること
// 1. window.renderer  ->  window.texture_refcount
// 2. vrm_models  ->  window.renderer
// 3. camera_3d  ->  window.renderer
// 逆順ロックはデッドロックを引き起こす可能性がある
// ============================================================================

use pyo3::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::window::{InjectEvent, KagraWindow};
use crate::error::{lock_py, lock_recover};
use crate::audio::AudioEngine;
use crate::color::Color;
use crate::renderer::SkinnedMeshCommand;
use crate::vrm::VrmModel;
use crate::gltf::GltfModel;
use crate::camera::{row_major_to_column_major, unproject_ray, Camera3D};
use crate::pick::{bone_pick_radius, ray_sphere};

// モジュール
mod ui;

#[pyclass(unsendable)]
pub struct Engine {
    pub(crate) window: KagraWindow,
    pub(crate) audio: Arc<Mutex<Option<AudioEngine>>>,
    pub(crate) keymap: Arc<Mutex<HashMap<String, u32>>>,
    pub(crate) vrm_models: Arc<Mutex<HashMap<u32, VrmModel>>>,
    pub(crate) next_vrm_id: Arc<Mutex<u32>>,
    pub(crate) boid_systems: Arc<Mutex<HashMap<u32, crate::boids::BoidSystem>>>,
    pub(crate) next_boid_id: Arc<Mutex<u32>>,
    pub(crate) boid_gpu_systems: Arc<Mutex<HashMap<u32, Arc<Mutex<crate::boids_gpu::BoidSystemGpu>>>>>,
    pub(crate) next_boid_gpu_id: Arc<Mutex<u32>>,
    pub(crate) gltf_models: Arc<Mutex<HashMap<u32, GltfModel>>>,
    pub(crate) next_gltf_id: Arc<Mutex<u32>>,
    pub(crate) camera_3d: Arc<Mutex<Option<Camera3D>>>,
    /// Python 側が update_camera_3d() で行列を直接指定したか。
    /// true の間は組み込みカメラで uniform を上書きしない。
    pub(crate) camera_3d_external: Arc<AtomicBool>,
    /// 最後に GPU へ送った view / proj（列優先）。スクリーンレイ用。
    pub(crate) last_view_col: Arc<Mutex<[f32; 16]>>,
    pub(crate) last_proj_col: Arc<Mutex<[f32; 16]>>,
}

impl Engine {
    fn default_keymap() -> HashMap<String, u32> {
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
        default
    }

    /// キーマップを読む。無ければ内蔵デフォルト。CWD には書かない
    /// （`pip install` したライブラリが作業ディレクトリを汚さないため）。
    ///
    /// 探索順: `$KAGRA_KEYMAP` → `./keymap.json`（読み取り専用）
    fn load_keymap() -> HashMap<String, u32> {
        let mut candidates = Vec::new();
        if let Ok(p) = std::env::var("KAGRA_KEYMAP") {
            if !p.is_empty() {
                candidates.push(std::path::PathBuf::from(p));
            }
        }
        candidates.push(std::path::PathBuf::from("keymap.json"));
        for path in candidates {
            if !path.is_file() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(map) = serde_json::from_str(&content) {
                    log::info!("keymap loaded from {}", path.display());
                    return map;
                }
            }
        }
        Self::default_keymap()
    }

    /// テクスチャの参照カウントを減らし、0ならアンロードする（内部ヘルパ）
    fn decrement_texture_refcount(&self, texture_id: u32) {
        let mut rc = lock_recover(&self.window.texture_refcount);
        if let Some(count) = rc.get_mut(&texture_id) {
            *count -= 1;
            if *count == 0 {
                rc.remove(&texture_id);
                let mut cache = lock_recover(&self.window.texture_cache);
                cache.retain(|_, &mut v| v != texture_id);
                if let Some(renderer) = lock_recover(&self.window.renderer).as_mut() {
                    let _ = renderer.unload_texture(texture_id);
                }
            }
        }
    }
}

#[pymethods]
impl Engine {
    #[new]
    #[pyo3(signature = (width=1280, height=720, title="KAGRA", fps=60, transparent=false, decorations=true, always_on_top=false, visible=true))]
    pub fn new(
        width: u32,
        height: u32,
        title: &str,
        fps: u32,
        transparent: bool,
        decorations: bool,
        always_on_top: bool,
        visible: bool,
    ) -> PyResult<Self> {
        let _ = env_logger::try_init();
        let window = KagraWindow::new(width, height, title, fps, transparent, decorations, always_on_top, visible)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let audio_engine = match AudioEngine::new() {
            Ok(a) => { log::info!("AudioEngine OK"); Some(a) }
            Err(e) => { log::warn!("AudioEngine 初期化失敗: {}", e); None }
        };

        let keymap = Self::load_keymap();

        Ok(Engine {
            window,
            audio: Arc::new(Mutex::new(audio_engine)),
            keymap: Arc::new(Mutex::new(keymap)),
            vrm_models: Arc::new(Mutex::new(HashMap::new())),
            next_vrm_id: Arc::new(Mutex::new(1u32)),
            boid_systems: Arc::new(Mutex::new(HashMap::new())),
            next_boid_id: Arc::new(Mutex::new(1u32)),
            boid_gpu_systems: Arc::new(Mutex::new(HashMap::new())),
            next_boid_gpu_id: Arc::new(Mutex::new(1u32)),
            gltf_models: Arc::new(Mutex::new(HashMap::new())),
            next_gltf_id: Arc::new(Mutex::new(1u32)),
            camera_3d: Arc::new(Mutex::new(None)),
            camera_3d_external: Arc::new(AtomicBool::new(false)),
            last_view_col: Arc::new(Mutex::new([0.0; 16])),
            last_proj_col: Arc::new(Mutex::new([0.0; 16])),
        })
    }

    pub fn get_keymap(&self) -> HashMap<String, u32> {
        lock_recover(&self.keymap).clone()
    }

    #[pyo3(signature = (update_fn, draw_fn, max_frames=None, fixed_dt=None))]
    pub fn run(
        &self,
        py: Python<'_>,
        update_fn: PyObject,
        draw_fn: PyObject,
        max_frames: Option<u64>,
        fixed_dt: Option<f64>,
    ) -> PyResult<()> {
        self.window.run(py, update_fn, draw_fn, max_frames, fixed_dt)
    }

    /// 次のフレーム処理後にループを終了する
    pub fn request_exit(&self) {
        self.window.request_exit();
    }

    /// 次に描画されるフレームを PNG として保存する（パスは update/draw 内で指定）
    pub fn request_screenshot(&self, path: &str) {
        self.window.request_screenshot(path);
    }

    /// GPU から毎フレーム RGB を取り出す（仮想カメラ用）。720p 推奨。
    pub fn set_grab_frames(&self, enabled: bool) {
        self.window.set_grab_frames(enabled);
    }

    /// 直前フレームの (width, height, rgb)。無ければ None。1 回読むと消える。
    pub fn grab_frame(&self) -> Option<(u32, u32, Vec<u8>)> {
        self.window.grab_frame()
    }

    /// 完了したフレーム数（run 開始後）
    pub fn frame_count(&self) -> u64 {
        self.window.frame_count()
    }

    pub fn inject_key_down(&self, code: u32) {
        self.window.queue_inject(InjectEvent::KeyDown(code));
    }

    pub fn inject_key_up(&self, code: u32) {
        self.window.queue_inject(InjectEvent::KeyUp(code));
    }

    pub fn inject_mouse_move(&self, x: f32, y: f32) {
        self.window.queue_inject(InjectEvent::MouseMove(x, y));
    }

    pub fn inject_mouse_down(&self, button: u32) {
        self.window.queue_inject(InjectEvent::MouseDown(button));
    }

    pub fn inject_mouse_up(&self, button: u32) {
        self.window.queue_inject(InjectEvent::MouseUp(button));
    }

    #[getter]
    pub fn width(&self) -> u32 { self.window.width }
    #[getter]
    pub fn height(&self) -> u32 { self.window.height }
    #[getter]
    pub fn fps(&self) -> f64 {
        self.window.current_fps_atomic.load(Ordering::Relaxed) as f64
    }

    // ウィンドウ操作
    pub fn focus_window(&self) {
        use crate::window::WindowCommand;
        lock_recover(&self.window.window_commands).push(WindowCommand::Focus);
    }
    pub fn drag_window(&self) {
        use crate::window::WindowCommand;
        lock_recover(&self.window.window_commands).push(WindowCommand::Drag);
    }
    pub fn set_window_position(&self, x: i32, y: i32) {
        use crate::window::WindowCommand;
        lock_recover(&self.window.window_commands).push(WindowCommand::SetPosition(x, y));
    }
    pub fn set_click_through(&self, click_through: bool) {
        use crate::window::WindowCommand;
        lock_recover(&self.window.window_commands).push(WindowCommand::SetClickThrough(click_through));
    }
    pub fn set_always_on_top(&self, always_on_top: bool) {
        use crate::window::WindowCommand;
        lock_recover(&self.window.window_commands).push(WindowCommand::SetAlwaysOnTop(always_on_top));
    }
    pub fn set_decorations(&self, decorations: bool) {
        use crate::window::WindowCommand;
        lock_recover(&self.window.window_commands).push(WindowCommand::SetDecorations(decorations));
    }
    pub fn set_window_title(&self, title: String) {
        use crate::window::WindowCommand;
        lock_recover(&self.window.window_commands).push(WindowCommand::SetTitle(title));
    }

    pub fn screen_width(&self) -> u32 {
        lock_recover(&self.window.renderer).as_ref().map(|r| r.screen_width).unwrap_or(1280)
    }
    pub fn screen_height(&self) -> u32 {
        lock_recover(&self.window.renderer).as_ref().map(|r| r.screen_height).unwrap_or(720)
    }

    // 衝突判定
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

    /// Mixamo 等の FBX アニメを読み込む。
    /// 戻り値: [(clip_name, frame_time, frames), ...]
    /// frames[i] = [(name, tx,ty,tz, qx,qy,qz,qw, has_trans), ...]
    pub fn load_fbx_anim(
        &self,
        path: &str,
    ) -> PyResult<Vec<(String, f64, Vec<Vec<(String, f32, f32, f32, f32, f32, f32, f32, bool)>>)>> {
        let clips = crate::fbx_loader::load_fbx_anim(path)?;
        Ok(clips
            .into_iter()
            .map(|c| {
                let frames = c
                    .frames
                    .into_iter()
                    .map(|frame| {
                        frame
                            .into_iter()
                            .map(|b| {
                                (
                                    b.name,
                                    b.translation[0],
                                    b.translation[1],
                                    b.translation[2],
                                    b.rotation[0],
                                    b.rotation[1],
                                    b.rotation[2],
                                    b.rotation[3],
                                    b.has_trans,
                                )
                            })
                            .collect()
                    })
                    .collect();
                (c.name, c.frame_time, frames)
            })
            .collect())
    }

    // ========== VRM  ==========
    pub fn load_vrm(&self, path: &str) -> PyResult<u32> {
        use crate::gltf_common::extract_texture_data_from_glb;
        use crate::vrm::load_vrm;
        use std::collections::HashMap;

        let tex_data = extract_texture_data_from_glb(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let mut tex_id_map = HashMap::new();
        {
            let mut rg = lock_py(&self.window.renderer)?;
            let renderer = rg.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "load_vrm: run() 開始前です。Scene.on_enter か run(on_ready=...) 内で呼んでください。".to_string(),
                )
            })?;
            let mut rc = lock_py(&self.window.texture_refcount)?;
            for (ti, bytes, ext) in tex_data {
                if let Ok(id) = renderer.load_gltf_image(&bytes, &ext) {
                    tex_id_map.insert(ti, id);
                    *rc.entry(id).or_insert(0) += 1;
                }
            }
        }

        let model = {
            let mut rg = lock_py(&self.window.renderer)?;
            let renderer = rg.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(
                    "load_vrm: run() 開始前です。Scene.on_enter か run(on_ready=...) 内で呼んでください。".to_string(),
                )
            })?;
            load_vrm(path, &renderer.device, &tex_id_map)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
        };

        let id = {
            let mut next = lock_py(&self.next_vrm_id)?;
            let id = *next;
            *next += 1;
            id
        };
        lock_py(&self.vrm_models)?.insert(id, model);
        Ok(id)
    }

    /// VRMモデルをアンロードし、使用していたテクスチャの参照カウントを減らす
    pub fn unload_vrm(&self, vrm_id: u32) -> PyResult<()> {
        let model = {
            let mut models = lock_py(&self.vrm_models)?;
            models.remove(&vrm_id)
        };
        if let Some(model) = model {
            let mut tex_ids = std::collections::HashSet::new();
            for prim in &model.primitives {
                if prim.texture_id != 0 {
                    tex_ids.insert(prim.texture_id);
                }
            }
            for tid in tex_ids {
                self.decrement_texture_refcount(tid);
            }
            log::info!("Unloaded VRM model id={}", vrm_id);
            Ok(())
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("VRM model {} not found", vrm_id)))
        }
    }

    pub fn draw_vrm(&self, vrm_id: u32) -> PyResult<()> {
        self.ensure_camera_uniforms()?;

        let cmds = {
            let mut rg = lock_py(&self.window.renderer)?;
            let renderer = rg.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Renderer not initialized".to_string())
            })?;
            let mut models = lock_py(&self.vrm_models)?;
            let model = models.get_mut(&vrm_id).ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err(format!("vrm_id={} not found", vrm_id))
            })?;
            model.build_draw_commands(&renderer.device)
        };
        let mut rg = lock_py(&self.window.renderer)?;
        if let Some(renderer) = rg.as_mut() {
            for (matrices, cmd) in cmds {
                renderer.queue_skinned_mesh_3d_with_palette(cmd, &matrices);
            }
        }
        Ok(())
    }

    #[pyo3(signature = (vrm_id, bone_name, qx=0.0, qy=0.0, qz=0.0, qw=1.0))]
    pub fn set_vrm_bone_rot(&self, vrm_id: u32, bone_name: &str, qx: f32, qy: f32, qz: f32, qw: f32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.set_bone_rot_quat(bone_name, qx, qy, qz, qw);
        }
    }

    #[pyo3(signature = (vrm_id, bone_name, rx=0.0, ry=0.0, rz=0.0))]
    pub fn set_vrm_bone_euler(&self, vrm_id: u32, bone_name: &str, rx: f32, ry: f32, rz: f32) {
        let (cx, sx) = ((rx / 2.0).cos(), (rx / 2.0).sin());
        let (cy, sy) = ((ry / 2.0).cos(), (ry / 2.0).sin());
        let (cz, sz) = ((rz / 2.0).cos(), (rz / 2.0).sin());
        let qx = sx * cy * cz + cx * sy * sz;
        let qy = cx * sy * cz - sx * cy * sz;
        let qz = cx * cy * sz + sx * sy * cz;
        let qw = cx * cy * cz - sx * sy * sz;
        self.set_vrm_bone_rot(vrm_id, bone_name, qx, qy, qz, qw);
    }

    pub fn set_blend_shape(&self, vrm_id: u32, name: &str, weight: f32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.set_blend_shape(name, weight);
        }
    }

    pub fn reset_blend_shapes(&self, vrm_id: u32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.reset_blend_shapes();
        }
    }

    pub fn list_blend_shapes(&self, vrm_id: u32) -> Vec<String> {
        lock_recover(&self.vrm_models).get(&vrm_id).map(|m| m.list_blend_shapes()).unwrap_or_default()
    }

    pub fn set_vrm_first_person(&self, vrm_id: u32, enabled: bool) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.set_first_person(enabled);
        }
    }

    /// SpringBone のチェーン / ジョイント / コライダー数。未ロードなら (0,0,0)。
    pub fn vrm_spring_info(&self, vrm_id: u32) -> (u32, u32, u32) {
        lock_recover(&self.vrm_models)
            .get(&vrm_id)
            .map(|m| {
                let (c, j, col) = m.spring.counts();
                (c as u32, j as u32, col as u32)
            })
            .unwrap_or((0, 0, 0))
    }

    pub fn step_vrm_spring(&self, vrm_id: u32, dt: f32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.step_spring(dt);
        }
    }

    pub fn reset_vrm_spring(&self, vrm_id: u32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.reset_spring();
        }
    }

    #[pyo3(signature = (vrm_id, x=0.0, y=0.0, z=0.0))]
    pub fn set_vrm_spring_wind(&self, vrm_id: u32, x: f32, y: f32, z: f32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.set_spring_wind(x, y, z);
        }
    }

    pub fn set_vrm_spring_enabled(&self, vrm_id: u32, enabled: bool) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.set_spring_enabled(enabled);
        }
    }

    /// ライブモーキャプ用。`(name, qx, qy, qz, qw)` をまとめて書く。
    pub fn set_vrm_pose(&self, vrm_id: u32, bones: Vec<(String, f32, f32, f32, f32)>) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            for (name, qx, qy, qz, qw) in bones {
                model.set_bone_rot_quat(&name, qx, qy, qz, qw);
            }
        }
    }

    /// VRM humanoid 標準ボーン名の一覧（hips, head, leftUpperArm, …）
    pub fn list_human_bones(&self, vrm_id: u32) -> Vec<String> {
        lock_recover(&self.vrm_models)
            .get(&vrm_id)
            .map(|m| m.list_human_bones())
            .unwrap_or_default()
    }

    /// ボーン名をノード index に解決する。実ノード名 / 標準名 / J_Bip_* のいずれでも可。
    pub fn resolve_vrm_bone(&self, vrm_id: u32, name: &str) -> Option<u32> {
        lock_recover(&self.vrm_models)
            .get(&vrm_id)
            .and_then(|m| m.resolve_bone(name))
            .map(|i| i as u32)
    }

    /// ボーン名がこの VRM で解決できるか。
    pub fn has_vrm_bone(&self, vrm_id: u32, name: &str) -> bool {
        self.resolve_vrm_bone(vrm_id, name).is_some()
    }

    /// デバッグ: ボーンの現在のローカル回転 (qx,qy,qz,qw) を返す。
    pub fn debug_bone_local_rot(&self, vrm_id: u32, name: &str) -> Option<(f32, f32, f32, f32)> {
        let models = lock_recover(&self.vrm_models);
        let m = models.get(&vrm_id)?;
        let idx = m.resolve_bone(name)?;
        let r = m.bones.get(idx)?.local_rot;
        Some((r[0], r[1], r[2], r[3]))
    }

    /// デバッグ: ボーンのワールド行列の平行移動部を返す。
    pub fn debug_bone_world_pos(&self, vrm_id: u32, name: &str) -> Option<(f32, f32, f32)> {
        let mut models = lock_recover(&self.vrm_models);
        let m = models.get_mut(&vrm_id)?;
        if m.dirty {
            m.recompute_world();
            m.dirty = false;
        }
        let idx = m.resolve_bone(name)?;
        let w = m.bones.get(idx)?.world_mat;
        Some((w[(0, 3)], w[(1, 3)], w[(2, 3)]))
    }

    /// VRM LookAt メタを返す。
    ///
    /// `(type, ox, oy, oz,
    ///   hi_in, hi_out, ho_in, ho_out, vd_in, vd_out, vu_in, vu_out)`
    /// - type: `"bone"` | `"expression"`
    /// - offset: 頭ボーンからのオフセット
    /// - 各 range map: `(inputMaxValue, outputScale)`（度）
    pub fn get_vrm_look_at(
        &self,
        vrm_id: u32,
    ) -> Option<(
        String,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
        f32,
    )> {
        lock_recover(&self.vrm_models)
            .get(&vrm_id)
            .and_then(|m| m.look_at.as_ref())
            .map(|la| {
                (
                    la.look_at_type.clone(),
                    la.offset_from_head_bone[0],
                    la.offset_from_head_bone[1],
                    la.offset_from_head_bone[2],
                    la.range_map_horizontal_inner.input_max_value,
                    la.range_map_horizontal_inner.output_scale,
                    la.range_map_horizontal_outer.input_max_value,
                    la.range_map_horizontal_outer.output_scale,
                    la.range_map_vertical_down.input_max_value,
                    la.range_map_vertical_down.output_scale,
                    la.range_map_vertical_up.input_max_value,
                    la.range_map_vertical_up.output_scale,
                )
            })
    }

    pub fn set_vrm_offset(&self, vrm_id: u32, x: f32, y: f32, z: f32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.root_offset = [x, y, z];
            model.dirty = true;
        }
    }

    pub fn set_vrm_bone_trans(&self, vrm_id: u32, bone_name: &str, tx: f32, ty: f32, tz: f32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.set_bone_trans(bone_name, tx, ty, tz);
        }
    }

    pub fn set_vrm_bone_scale(&self, vrm_id: u32, bone_name: &str, sx: f32, sy: f32, sz: f32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.set_bone_scale(bone_name, sx, sy, sz);
        }
    }

    pub fn reset_vrm_pose(&self, vrm_id: u32) {
        if let Some(model) = lock_recover(&self.vrm_models).get_mut(&vrm_id) {
            model.reset_pose();
        }
    }

    // ========== リグ ==========
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
            gpu_caches: Vec<(String, crate::rig::PartGpuCache)>,
        }
        let snapshot = {
            let rigs = lock_py(&self.window.rigs)?;
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
            let part_entries = rig.parts.iter().map(|(name, part)| (name.clone(), part.texture.clone())).collect();
            let gpu_caches = rig.gpu_cache.iter().map(|(name, cache)| (name.clone(), cache.clone())).collect();
            RigSnapshot { model_name: rig.model_name.clone(), part_entries, matrices, gpu_caches }
        };
        let mut renderer_guard = lock_py(&self.window.renderer)?;
        let renderer = match renderer_guard.as_mut() {
            Some(r) => r,
            None => return Ok(()),
        };
        renderer.update_skin_uniforms(&snapshot.matrices);
        let base_path = format!("static/output/rigs/{}", snapshot.model_name);
        let gpu_map: std::collections::HashMap<&str, &crate::rig::PartGpuCache> =
            snapshot.gpu_caches.iter().map(|(name, cache)| (name.as_str(), cache)).collect();
        for (part_name, part_texture) in &snapshot.part_entries {
            let tex_path = format!("{}/{}", base_path, part_texture);
            let tex_id = match self.window.get_cached_texture(rig_id, part_name, || renderer.load_texture(&tex_path)) {
                Ok(id) => id,
                Err(e) => {
                    log::warn!("draw_rig: texture load failed '{}': {}", part_name, e);
                    continue;
                }
            };
            if let Some(cache) = gpu_map.get(part_name.as_str()) {
                let cmd = SkinnedMeshCommand {
                    texture_id: tex_id,
                    vertex_buffer: Arc::clone(&cache.vertex_buffer),
                    index_buffer: Arc::clone(&cache.index_buffer),
                    num_indices: cache.num_indices,
                    blend_weights_buffer: Arc::clone(&cache.blend_weights_buffer),
                    morph_delta_buffer: Arc::clone(&cache.morph_delta_buffer),
                    num_morph_targets: 0,
                    mtoon_buffer: None,
                    shade_texture_id: None,
                    matcap_texture_id: None,
                    normal_texture_id: None,
                    uv_mask_texture_id: None,
                    outline_width: 0.0,
                    skin_slot: None,
                    aabb: None,
                    double_sided: true,
                };
                self.window.queue_skinned_mesh(cmd);
            }
        }
        Ok(())
    }

    pub fn set_ik_target(&self, rig_id: u32, bone_name: &str, x: f32, y: f32, z: f32) -> PyResult<()> {
        let mut rigs = lock_py(&self.window.rigs)?;
        if let Some(rig) = rigs.get_mut(&rig_id) {
            rig.ik_targets.insert(bone_name.to_string(), nalgebra::Point3::new(x, y, z));
            rig.ik_enabled = true;
            Ok(())
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("Rig {} not found", rig_id)))
        }
    }

    pub fn disable_ik(&self, rig_id: u32) -> PyResult<()> {
        let mut rigs = lock_py(&self.window.rigs)?;
        if let Some(rig) = rigs.get_mut(&rig_id) {
            rig.ik_enabled = false;
            rig.ik_targets.clear();
            Ok(())
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("Rig {} not found", rig_id)))
        }
    }

    pub fn is_ik_enabled(&self, rig_id: u32) -> PyResult<bool> {
        let rigs = lock_py(&self.window.rigs)?;
        if let Some(rig) = rigs.get(&rig_id) {
            Ok(rig.ik_enabled)
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("Rig {} not found", rig_id)))
        }
    }

    // ========== ボイド（CPU版、非推奨） ==========
    #[pyo3(signature = (count, width=1280.0, height=720.0))]
    pub fn create_boid_system(&self, count: u32, width: f32, height: f32) -> u32 {
        let system = crate::boids::BoidSystem::new(count as usize, width, height);
        let mut systems = lock_recover(&self.boid_systems);
        let mut next = lock_recover(&self.next_boid_id);
        let id = *next;
        *next += 1;
        systems.insert(id, system);
        id
    }

    pub fn update_boids(&self, boid_id: u32, dt: f32) {
        if let Some(s) = lock_recover(&self.boid_systems).get_mut(&boid_id) {
            s.update(dt);
        }
    }

    #[pyo3(signature = (boid_id, batch_id, _sprite_w=6.0, _sprite_h=3.0))]
    pub fn draw_boids(&self, boid_id: u32, batch_id: u32, _sprite_w: f32, _sprite_h: f32) {
        let mut rg = lock_recover(&self.window.renderer);
        if let Some(r) = rg.as_mut() {
            let mut systems = lock_recover(&self.boid_systems);
            if let Some(system) = systems.get_mut(&boid_id) {
                if let Some(batch) = r.instance_renderer.as_ref().and_then(|ir| ir.batches.get(&batch_id)) {
                    system.write_to_buffer(&r.queue, &batch.instance_buffer);
                    let count = system.boids.len().min(batch.capacity as usize) as u32;
                    drop(systems);
                    if let Some(ir) = r.instance_renderer.as_mut() {
                        if let Some(b) = ir.batches.get_mut(&batch_id) {
                            b.count = count;
                        }
                    }
                    r.queue_instance_batch(batch_id);
                }
            }
        }
    }

    pub fn boid_count(&self, boid_id: u32) -> u32 {
        lock_recover(&self.boid_systems).get(&boid_id).map(|s| s.boids.len() as u32).unwrap_or(0)
    }

    // ========== ボイド（GPU版、推奨） ==========
    #[pyo3(signature = (count, width=1280.0, height=720.0))]
    pub fn create_boid_system_gpu(&self, count: u32, width: f32, height: f32) -> u32 {
        let mut rg = lock_recover(&self.window.renderer);
        if let Some(r) = rg.as_mut() {
            if r.instance_renderer.is_none() {
                r.instance_renderer = Some(crate::instance_renderer::InstanceRenderer::new(
                    &r.device, r.surface_format, r.screen_width, r.screen_height,
                ));
            }
            let system = Arc::new(Mutex::new(crate::boids_gpu::BoidSystemGpu::new(
                &r.device, &r.queue, count, width, height,
            )));
            let mut systems = lock_recover(&self.boid_gpu_systems);
            let mut next = lock_recover(&self.next_boid_gpu_id);
            let id = *next;
            *next += 1;
            systems.insert(id, system);
            id
        } else { 0 }
    }

    pub fn set_boid_active_count(&self, boid_id: u32, count: u32) {
        let systems = lock_recover(&self.boid_gpu_systems);
        if let Some(sys_arc) = systems.get(&boid_id) {
            let mut sys = lock_recover(&sys_arc);
            sys.active_count = count.min(sys.count);
            if let Some(bid) = sys.cached_batch_id {
                drop(sys);
                let mut rg = lock_recover(&self.window.renderer);
                if let Some(r) = rg.as_mut() {
                    if let Some(ir) = r.instance_renderer.as_mut() {
                        if let Some(batch) = ir.batches.get_mut(&bid) {
                            batch.count = count.min(batch.capacity);
                        }
                    }
                }
            }
        }
    }

    pub fn update_boids_gpu(&self, boid_id: u32, dt: f32) {
        let systems = lock_recover(&self.boid_gpu_systems);
        if let Some(sys_arc) = systems.get(&boid_id) {
            let mut sys = lock_recover(&sys_arc);
            sys.time += dt;
            sys.pending_dt = Some(dt);
        }
    }

    pub fn draw_boids_gpu(&self, boid_id: u32) {
        let mut rg = lock_recover(&self.window.renderer);
        let r = match rg.as_mut() { Some(r) => r, None => return };
        let sys_arc = {
            let systems = lock_recover(&self.boid_gpu_systems);
            match systems.get(&boid_id) {
                Some(s) => s.clone(),
                None => return,
            }
        };
        let (active_count, time, dt, width, height, batch_id_opt) = {
            let sys = lock_recover(&sys_arc);
            let dt = sys.pending_dt.unwrap_or(0.016);
            let time = sys.time;
            let count = sys.active_count;
            let w = r.screen_width as f32;
            let h = r.screen_height as f32;
            (count, time, dt, w, h, sys.cached_batch_id)
        };
        let batch_id = match batch_id_opt {
            Some(id) => id,
            None => {
                let mut sys = lock_recover(&sys_arc);
                if let Some(ir) = r.instance_renderer.as_mut() {
                    let arc_buf = sys.instance_buffer.clone();
                    let bid = ir.register_gpu_buffer(arc_buf, active_count);
                    sys.cached_batch_id = Some(bid);
                    bid
                } else { return; }
            }
        };
        let sys_clone = sys_arc.clone();
        r.pending_computes.push(Box::new(move |_device, queue, encoder| {
            let sys = lock_recover(&sys_clone);
            sys.record_compute_pass(encoder, queue, dt, width, height, time, active_count);
        }));
        r.queue_instance_batch(batch_id);
    }

    // ========== glTF ==========
    pub fn load_gltf(&self, path: &str) -> PyResult<u32> {
        use crate::gltf_common::extract_texture_data_from_glb;
        use crate::gltf::load_gltf;
        use std::collections::HashMap;

        let tex_data = extract_texture_data_from_glb(path)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        let mut tex_id_map = HashMap::new();
        {
            let mut rg = lock_py(&self.window.renderer)?;
            let renderer = rg.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Renderer not initialized".to_string())
            })?;
            let mut rc = lock_py(&self.window.texture_refcount)?;
            for (ti, bytes, ext) in tex_data {
                if let Ok(id) = renderer.load_gltf_image(&bytes, &ext) {
                    tex_id_map.insert(ti, id);
                    *rc.entry(id).or_insert(0) += 1;
                }
            }
        }

        let model = {
            let mut rg = lock_py(&self.window.renderer)?;
            let renderer = rg.as_mut().ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("Renderer not initialized".to_string())
            })?;
            load_gltf(path, &renderer.device, &tex_id_map)
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?
        };

        let id = {
            let mut next = lock_py(&self.next_gltf_id)?;
            let id = *next;
            *next += 1;
            id
        };
        lock_py(&self.gltf_models)?.insert(id, model);
        Ok(id)
    }

    /// glTFモデルをアンロードし、使用していたテクスチャの参照カウントを減らす
    pub fn unload_gltf(&self, model_id: u32) -> PyResult<()> {
        let model = {
            let mut models = lock_py(&self.gltf_models)?;
            models.remove(&model_id)
        };
        if let Some(model) = model {
            let mut tex_ids = std::collections::HashSet::new();
            for prim in &model.primitives {
                if prim.texture_id != 0 {
                    tex_ids.insert(prim.texture_id);
                }
            }
            for tid in tex_ids {
                self.decrement_texture_refcount(tid);
            }
            log::info!("Unloaded glTF model id={}", model_id);
            Ok(())
        } else {
            Err(pyo3::exceptions::PyValueError::new_err(format!("glTF model {} not found", model_id)))
        }
    }

    pub fn draw_gltf(&self, model_id: u32) -> PyResult<()> {
        self.ensure_camera_uniforms()?;

        let mut models = lock_py(&self.gltf_models)?;
        let model = models.get_mut(&model_id).ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!("glTF model {} not found", model_id))
        })?;

        let mut rg = lock_py(&self.window.renderer)?;
        let renderer = rg.as_mut().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("Renderer not initialized".to_string())
        })?;

        let commands = model.build_draw_commands(&renderer.device);
        for (matrices, cmd) in commands {
            renderer.queue_skinned_mesh_3d_with_palette(cmd, &matrices);
        }
        Ok(())
    }

    /// 3D 描画の直前にカメラ uniform を用意する。
    ///
    /// Python の kagra.Camera3D は update_camera_3d() で行列を直接書き込む。
    /// そちらが使われている場合に組み込みカメラで上書きすると、
    /// update() で設定した画角が draw() で毎フレーム捨てられてしまうため触らない。
    fn ensure_camera_uniforms(&self) -> PyResult<()> {
        if self.camera_3d_external.load(Ordering::Relaxed) {
            return Ok(());
        }
        {
            let mut cam_guard = lock_py(&self.camera_3d)?;
            if cam_guard.is_none() {
                *cam_guard = Some(Camera3D::new(self.width(), self.height()));
            }
        }
        self.update_camera_uniforms();
        Ok(())
    }

    // カメラ行列をRendererに反映するヘルパ
    fn update_camera_uniforms(&self) {
        // ここに external=true で来るのは組み込みカメラ操作 API のみ。
        // Python の Camera3D と併用されると次のフレームで上書きされて効かないので警告する。
        if self.camera_3d_external.load(Ordering::Relaxed) {
            log::warn!(
                "組み込みカメラ操作は kagra.Camera3D と併用できません（Camera3D 側が優先されます）"
            );
        }
        let (w, h) = (self.width(), self.height());
        let mut cam_guard = lock_recover(&self.camera_3d);
        let cam = match cam_guard.as_mut() {
            Some(c) => c,
            None => {
                log::warn!("update_camera_uniforms: camera_3d is None");
                return;
            }
        };
        // ウィンドウがリサイズされていればアスペクト比を追従させる
        cam.resize(w, h);
        cam.update_matrices();
        let view = cam.view_matrix();
        let proj = cam.proj_matrix();
        let view_arr: [f32; 16] = view.as_slice().try_into().unwrap();
        let proj_arr: [f32; 16] = proj.as_slice().try_into().unwrap();
        *lock_recover(&self.last_view_col) = view_arr;
        *lock_recover(&self.last_proj_col) = proj_arr;
        let mut rg = lock_recover(&self.window.renderer);
        if let Some(renderer) = rg.as_mut() {
            renderer.update_camera_3d(&view_arr, &proj_arr);
        } else {
            log::error!("update_camera_uniforms: renderer is None");
        }
    }

    pub fn zoom_camera(&self, delta: f32) -> PyResult<()> {
        {
            let mut cam_guard = lock_py(&self.camera_3d)?;
            if let Some(cam) = cam_guard.as_mut() {
                cam.zoom(delta);
            } else {
                *cam_guard = Some(Camera3D::new(self.width(), self.height()));
                if let Some(cam) = cam_guard.as_mut() {
                    cam.zoom(delta);
                }
            }
        }
        self.update_camera_uniforms();
        Ok(())
    }

    pub fn orbit_camera(&self, delta_x: f32, delta_y: f32) -> PyResult<()> {
        {
            let mut cam_guard = lock_py(&self.camera_3d)?;
            if let Some(cam) = cam_guard.as_mut() {
                cam.orbit(delta_x, delta_y);
            } else {
                *cam_guard = Some(Camera3D::new(self.width(), self.height()));
                if let Some(cam) = cam_guard.as_mut() {
                    cam.orbit(delta_x, delta_y);
                }
            }
        }
        self.update_camera_uniforms();
        Ok(())
    }

    #[pyo3(signature = (x, y, z))]
    pub fn set_camera_target(&self, x: f32, y: f32, z: f32) -> PyResult<()> {
        {
            let mut cam_guard = lock_py(&self.camera_3d)?;
            if let Some(cam) = cam_guard.as_mut() {
                cam.set_target(x, y, z);
            } else {
                let mut new_cam = Camera3D::new(self.width(), self.height());
                new_cam.set_target(x, y, z);
                *cam_guard = Some(new_cam);
            }
        }
        self.update_camera_uniforms();
        Ok(())
    }

    #[pyo3(signature = (x, y, z))]
    pub fn set_camera_position(&self, x: f32, y: f32, z: f32) -> PyResult<()> {
        {
            let mut cam_guard = lock_py(&self.camera_3d)?;
            if let Some(cam) = cam_guard.as_mut() {
                cam.set_position(x, y, z);
            } else {
                let mut new_cam = Camera3D::new(self.width(), self.height());
                new_cam.set_position(x, y, z);
                *cam_guard = Some(new_cam);
            }
        }
        self.update_camera_uniforms();
        Ok(())
    }

    // ========== オーディオ ==========
    #[pyo3(signature = (path, loop_=true, volume=0.8))]
    pub fn play_bgm(&self, path: &str, loop_: bool, volume: f32) -> PyResult<()> {
        self.with_audio(|a| a.play_bgm(path, loop_, volume))
    }
    pub fn stop_bgm(&self, fade: f32) {
        if let Some(a) = lock_recover(&self.audio).as_ref() { a.stop_bgm(fade); }
    }
    pub fn pause_bgm(&self) {
        if let Some(a) = lock_recover(&self.audio).as_ref() { a.pause_bgm(); }
    }
    pub fn resume_bgm(&self) {
        if let Some(a) = lock_recover(&self.audio).as_ref() { a.resume_bgm(); }
    }
    pub fn set_bgm_volume(&self, volume: f32) {
        if let Some(a) = lock_recover(&self.audio).as_ref() { a.set_bgm_volume(volume); }
    }
    #[pyo3(signature = (path, volume=1.0))]
    pub fn play_se(&self, path: &str, volume: f32) -> PyResult<()> {
        self.with_audio(|a| a.play_se(path, volume))
    }
    pub fn stop_all_se(&self) {
        if let Some(a) = lock_recover(&self.audio).as_ref() { a.stop_all_se(); }
    }

    // ========== 入力 ==========
    pub fn get_key_code(&self, name: &str) -> Option<u32> {
        use winit::keyboard::KeyCode;
        let key_code = match name {
            "Digit1" | "1" => KeyCode::Digit1,
            "Digit2" | "2" => KeyCode::Digit2,
            "Digit3" | "3" => KeyCode::Digit3,
            "Digit4" | "4" => KeyCode::Digit4,
            "Digit5" | "5" => KeyCode::Digit5,
            "Digit6" | "6" => KeyCode::Digit6,
            "Digit7" | "7" => KeyCode::Digit7,
            "Digit8" | "8" => KeyCode::Digit8,
            "Digit9" | "9" => KeyCode::Digit9,
            "Digit0" | "0" => KeyCode::Digit0,
            "KeyA" | "A" | "a" => KeyCode::KeyA,
            "KeyB" | "B" | "b" => KeyCode::KeyB,
            "KeyC" | "C" | "c" => KeyCode::KeyC,
            "KeyD" | "D" | "d" => KeyCode::KeyD,
            "KeyE" | "E" | "e" => KeyCode::KeyE,
            "KeyF" | "F" | "f" => KeyCode::KeyF,
            "KeyG" | "G" | "g" => KeyCode::KeyG,
            "KeyH" | "H" | "h" => KeyCode::KeyH,
            "KeyI" | "I" | "i" => KeyCode::KeyI,
            "KeyJ" | "J" | "j" => KeyCode::KeyJ,
            "KeyK" | "K" | "k" => KeyCode::KeyK,
            "KeyL" | "L" | "l" => KeyCode::KeyL,
            "KeyM" | "M" | "m" => KeyCode::KeyM,
            "KeyN" | "N" | "n" => KeyCode::KeyN,
            "KeyO" | "O" | "o" => KeyCode::KeyO,
            "KeyP" | "P" | "p" => KeyCode::KeyP,
            "KeyQ" | "Q" | "q" => KeyCode::KeyQ,
            "KeyR" | "R" | "r" => KeyCode::KeyR,
            "KeyS" | "S" | "s" => KeyCode::KeyS,
            "KeyT" | "T" | "t" => KeyCode::KeyT,
            "KeyU" | "U" | "u" => KeyCode::KeyU,
            "KeyV" | "V" | "v" => KeyCode::KeyV,
            "KeyW" | "W" | "w" => KeyCode::KeyW,
            "KeyX" | "X" | "x" => KeyCode::KeyX,
            "KeyY" | "Y" | "y" => KeyCode::KeyY,
            "KeyZ" | "Z" | "z" => KeyCode::KeyZ,
            "ArrowUp" | "Up" | "UP" => KeyCode::ArrowUp,
            "ArrowDown" | "Down" | "DOWN" => KeyCode::ArrowDown,
            "ArrowLeft" | "Left" | "LEFT" => KeyCode::ArrowLeft,
            "ArrowRight" | "Right" | "RIGHT" => KeyCode::ArrowRight,
            "Space" | "SPACE" => KeyCode::Space,
            "Enter" | "Return" | "RETURN" => KeyCode::Enter,
            "Escape" | "ESC" | "ESCAPE" => KeyCode::Escape,
            "Backspace" | "BACKSPACE" => KeyCode::Backspace,
            "Tab" | "TAB" => KeyCode::Tab,
            "F1" => KeyCode::F1,
            "F2" => KeyCode::F2,
            "F3" => KeyCode::F3,
            "F4" => KeyCode::F4,
            "F5" => KeyCode::F5,
            "F6" => KeyCode::F6,
            "F7" => KeyCode::F7,
            "F8" => KeyCode::F8,
            "F9" => KeyCode::F9,
            "F10" => KeyCode::F10,
            "F11" => KeyCode::F11,
            "F12" => KeyCode::F12,
            _ => return None,
        };
        Some(key_code as u32)
    }
    pub fn get_typed_chars(&self) -> String {
        lock_recover(&self.window.input).char_buffer.iter().collect()
    }
    pub fn get_preedit_text(&self) -> String {
        lock_recover(&self.window.input).preedit_text.clone()
    }
    pub fn get_preedit_cursor(&self) -> Option<(usize, usize)> {
        lock_recover(&self.window.input).preedit_cursor
    }
    pub fn set_ime_cursor_pos(&self, x: f64, y: f64) {
        let mut inp = lock_recover(&self.window.input);
        inp.ime_x = x as f32;
        inp.ime_y = y as f32;
    }
    pub fn backspace_pressed(&self) -> bool { lock_recover(&self.window.input).backspace_pressed }
    pub fn enter_pressed(&self) -> bool { lock_recover(&self.window.input).enter_pressed }
    pub fn escape_pressed(&self) -> bool { lock_recover(&self.window.input).escape_pressed }
    pub fn key_down(&self, code: u32) -> bool { self.window.is_key_down(code) }
    pub fn key_pressed(&self, code: u32) -> bool { self.window.is_key_pressed(code) }
    pub fn key_released(&self, code: u32) -> bool { self.window.is_key_released(code) }
    pub fn mouse_pos(&self) -> (f32, f32) { self.window.mouse_pos() }
    pub fn mouse_down(&self, btn: u32) -> bool { self.window.is_mouse_down(btn) }
    pub fn mouse_pressed(&self, btn: u32) -> bool { self.window.is_mouse_pressed(btn) }
    pub fn mouse_released(&self, btn: u32) -> bool { self.window.is_mouse_released(btn) }
    pub fn mouse_wheel(&self) -> (f32, f32) { self.window.mouse_wheel() }
    pub fn mouse_wheel_y(&self) -> f32 { lock_recover(&self.window.input).wheel_y() }

    // ========== UI ==========
    #[pyo3(signature = (x, y, w, h, text, bg_r=70, bg_g=70, bg_b=90, hv_r=100, hv_g=100, hv_b=150, txt_r=255, txt_g=255, txt_b=255, font_size=20, font_id=1))]
    pub fn draw_ui_button(
        &self, x: f32, y: f32, w: f32, h: f32, text: String,
        bg_r: u8, bg_g: u8, bg_b: u8,
        hv_r: u8, hv_g: u8, hv_b: u8,
        txt_r: u8, txt_g: u8, txt_b: u8,
        font_size: u32, font_id: u32,
    ) -> PyResult<bool> {
        let (mx, my, is_clicked) = {
            let input = lock_py(&self.window.input)?;
            (input.mouse_pos().0, input.mouse_pos().1, input.is_mouse_pressed(1))
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

    // ========== 描画プリミティブ ==========
    pub fn cls(&self, r: u8, g: u8, b: u8) { self.window.cls(r, g, b); }
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8, a: u8) {
        self.window.rect(x, y, w, h, Color { r, g, b, a });
    }
    #[pyo3(signature = (verts, r=255, g=255, b=255, a=255))]
    pub fn draw_polygon(&self, verts: Vec<Vec<f32>>, r: u8, g: u8, b: u8, a: u8) {
        let cv: Vec<[f32;2]> = verts.iter().map(|v| [v[0], v[1]]).collect();
        self.window.polygon(cv, Color { r, g, b, a });
    }
    pub fn load_texture(&self, path: &str) -> PyResult<u32> {
        self.window.load_texture(path).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
    }
    pub fn texture_size(&self, id: u32) -> Option<(u32, u32)> { self.window.texture_size(id) }

    pub fn unload_texture(&self, id: u32) -> PyResult<()> {
        self.window.unload_texture(id)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
    }
    pub fn load_font(&self, path: &str) -> PyResult<u32> {
        self.window.load_font(path).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e))
    }
    #[pyo3(signature = (font_id, text, x, y, size=24, r=255, g=255, b=255, a=255))]
    pub fn draw_text(&self, font_id: u32, text: &str, x: f32, y: f32, size: u32, r: u8, g: u8, b: u8, a: u8) {
        self.window.draw_text(font_id, text, x, y, size, r, g, b, a);
    }
    pub fn measure_text(&self, font_id: u32, text: &str, size: u32) -> (f32, f32) {
        self.window.measure_text(font_id, text, size)
    }
    pub fn load_shader(&self, path: &str) -> PyResult<u32> {
        let src = std::fs::read_to_string(path).map_err(|e| {
            pyo3::exceptions::PyRuntimeError::new_err(format!("シェーダー読み込み失敗: {} ({})", path, e))
        })?;
        let mut rg = lock_py(&self.window.renderer)?;
        match rg.as_mut() {
            Some(r) => r.load_shader_src(&src).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e)),
            None => Err(pyo3::exceptions::PyRuntimeError::new_err("Renderer not initialized")),
        }
    }
    pub fn load_shader_src(&self, wgsl_src: &str) -> PyResult<u32> {
        let mut rg = lock_py(&self.window.renderer)?;
        match rg.as_mut() {
            Some(r) => r.load_shader_src(wgsl_src).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e)),
            None => Err(pyo3::exceptions::PyRuntimeError::new_err("Renderer not initialized")),
        }
    }
    #[pyo3(signature = (id, x, y, w=None, h=None, sx=0.0, sy=0.0, sw=None, sh=None, alpha=1.0, rotation_deg=0.0, pivot_x=0.5, pivot_y=0.5, flip_x=false, flip_y=false, shader_id=0u32, shader_params=None))]
    pub fn draw_texture(
        &self, id: u32, x: f32, y: f32, w: Option<f32>, h: Option<f32>,
        sx: f32, sy: f32, sw: Option<f32>, sh: Option<f32>, alpha: f32, rotation_deg: f32,
        pivot_x: f32, pivot_y: f32, flip_x: bool, flip_y: bool,
        shader_id: u32, shader_params: Option<Vec<f32>>,
    ) {
        let params: [f32;4] = if let Some(p) = shader_params {
            [p.get(0).copied().unwrap_or(1.0),
             p.get(1).copied().unwrap_or(1.0),
             p.get(2).copied().unwrap_or(1.0),
             p.get(3).copied().unwrap_or(1.0)]
        } else { [1.0;4] };
        self.window.draw_texture_ex(id, x, y, w, h, sx, sy, sw, sh, alpha, rotation_deg, pivot_x, pivot_y, flip_x, flip_y, shader_id, params);
    }
    #[pyo3(signature = (view, proj))]
    /// view / proj を直接指定する。行列は行優先（行を上から並べた順）で渡す。
    ///
    /// 以降このエンジンの組み込みカメラは無効になる（ensure_camera_uniforms 参照）。
    pub fn update_camera_3d(&self, view: Vec<f32>, proj: Vec<f32>) {
        let v: [f32;16] = view.try_into().unwrap_or([0.0;16]);
        let p: [f32;16] = proj.try_into().unwrap_or([0.0;16]);
        self.camera_3d_external.store(true, Ordering::Relaxed);
        let v_col = row_major_to_column_major(&v);
        let p_col = row_major_to_column_major(&p);
        *lock_recover(&self.last_view_col) = v_col;
        *lock_recover(&self.last_proj_col) = p_col;
        self.window.update_camera_3d(v_col, p_col);
    }

    /// スクリーン座標（左上、ピクセル）→ ワールドレイ (ox,oy,oz, dx,dy,dz)。
    #[pyo3(signature = (sx, sy))]
    pub fn camera_ray_from_screen(&self, sx: f32, sy: f32) -> Option<(f32, f32, f32, f32, f32, f32)> {
        let w = self.width().max(1) as f32;
        let h = self.height().max(1) as f32;
        let view = *lock_recover(&self.last_view_col);
        let proj = *lock_recover(&self.last_proj_col);
        if view.iter().all(|v| *v == 0.0) && proj.iter().all(|v| *v == 0.0) {
            let mut cam_g = lock_recover(&self.camera_3d);
            if cam_g.is_none() {
                *cam_g = Some(Camera3D::new(self.width(), self.height()));
            }
            if let Some(cam) = cam_g.as_mut() {
                cam.resize(self.width(), self.height());
                cam.update_matrices();
                if let Some((o, d)) = cam.ray_from_screen(sx, sy, w, h) {
                    return Some((o.x, o.y, o.z, d.x, d.y, d.z));
                }
            }
            return None;
        }
        unproject_ray(&view, &proj, sx, sy, w, h).map(|(o, d)| (o.x, o.y, o.z, d.x, d.y, d.z))
    }

    /// humanoid ボーン球とレイの最近ヒット。ヒットしなければ None。
    #[pyo3(signature = (vrm_id, ox, oy, oz, dx, dy, dz, max_dist=100.0))]
    pub fn pick_vrm_bone(
        &self,
        vrm_id: u32,
        ox: f32,
        oy: f32,
        oz: f32,
        dx: f32,
        dy: f32,
        dz: f32,
        max_dist: f32,
    ) -> Option<String> {
        use nalgebra::Vector3;
        let mut dir = Vector3::new(dx, dy, dz);
        let len = dir.magnitude();
        if len < 1e-8 {
            return None;
        }
        dir /= len;
        let origin = Vector3::new(ox, oy, oz);
        let mut models = lock_recover(&self.vrm_models);
        let m = models.get_mut(&vrm_id)?;
        if m.dirty {
            m.recompute_world();
            m.dirty = false;
        }
        let mut best_t = max_dist;
        let mut best_name: Option<String> = None;
        let names: Vec<String> = m.human_bones.keys().cloned().collect();
        for name in names {
            let Some(&idx) = m.human_bones.get(&name) else {
                continue;
            };
            let Some(bone) = m.bones.get(idx) else {
                continue;
            };
            let center = Vector3::new(bone.world_mat[(0, 3)], bone.world_mat[(1, 3)], bone.world_mat[(2, 3)]);
            let r = bone_pick_radius(&name);
            if let Some(t) = ray_sphere(origin, dir, center, r) {
                if t > 0.0 && t < best_t {
                    best_t = t;
                    best_name = Some(name);
                }
            }
        }
        best_name
    }

    /// 3D 平行光の方向（光源へ向かうベクトル）。正規化はエンジン側で行う。
    #[pyo3(signature = (x, y, z))]
    pub fn set_light_dir(&self, x: f32, y: f32, z: f32) {
        self.window.set_light_dir(x, y, z);
    }

    /// グローバルリム（フレネル + 逆光 + 床バウンス）。0 でオフ。
    #[pyo3(signature = (intensity))]
    pub fn set_rim(&self, intensity: f32) {
        self.window.set_rim(intensity);
    }

    /// 平行光シャドウの有効/無効。
    #[pyo3(signature = (enabled))]
    pub fn set_shadow_enabled(&self, enabled: bool) {
        self.window.set_shadow_enabled(enabled);
    }

    /// 平行光シャドウの段数。1（既定・室内互換）か 2（屋外カスケード）。
    #[pyo3(signature = (count=1))]
    pub fn set_shadow_cascades(&self, count: u32) {
        self.window.set_shadow_cascades(count);
    }

    /// VRM トゥーン階調。softness≥0.999 で連続照明（デフォルト互換）。
    #[pyo3(signature = (threshold=0.5, softness=1.0, shade=0.55, lit=1.0))]
    pub fn set_toon_params(&self, threshold: f32, softness: f32, shade: f32, lit: f32) {
        self.window.set_toon_params(threshold, softness, shade, lit);
    }

    /// 3D 距離フォグ。enabled=false で無効。
    #[pyo3(signature = (start, end, r, g, b, enabled=true))]
    pub fn set_fog(&self, start: f32, end: f32, r: u8, g: u8, b: u8, enabled: bool) {
        self.window.set_fog(start, end, r, g, b, enabled);
    }

    /// 半球アンビエント（簡易 IBL）。strength=0 でオフ。
    #[pyo3(signature = (r, g, b, strength=0.0))]
    pub fn set_ambient(&self, r: f32, g: f32, b: f32, strength: f32) {
        self.window.set_ambient(r, g, b, strength);
    }

    /// 点光源 1（影は無し）。intensity=0 でオフ。スポットを消して点に戻す。
    #[pyo3(signature = (x, y, z, r=1.0, g=0.95, b=0.85, intensity=1.0, radius=8.0))]
    pub fn set_point_light(
        &self,
        x: f32, y: f32, z: f32,
        r: f32, g: f32, b: f32,
        intensity: f32, radius: f32,
    ) {
        self.window.set_point_light(x, y, z, r, g, b, intensity, radius);
    }

    /// スポット 1（影は無し）。点光源スロットを共有。
    #[pyo3(signature = (x, y, z, dx, dy, dz, angle=0.8, penumbra=0.25, intensity=1.0, radius=10.0, r=1.0, g=0.95, b=0.85))]
    pub fn set_spot_light(
        &self,
        x: f32, y: f32, z: f32,
        dx: f32, dy: f32, dz: f32,
        angle: f32, penumbra: f32,
        intensity: f32, radius: f32,
        r: f32, g: f32, b: f32,
    ) {
        self.window.set_spot_light(
            x, y, z, dx, dy, dz, angle, penumbra, intensity, radius, r, g, b,
        );
    }

    /// カラーの掛け算。1.0 は何もしない。
    #[pyo3(signature = (value=1.0))]
    pub fn set_exposure(&self, value: f32) {
        self.window.set_exposure(value);
    }

    /// HDRI キューブ。``studio`` は内蔵。空 / strength=0 でオフ。
    #[pyo3(signature = (path, strength=1.0))]
    pub fn set_hdri(&self, path: String, strength: f32) {
        self.window.set_hdri(&path, strength);
    }

    /// 保持メッシュの金属/粗さ。MToon は触らない。
    #[pyo3(signature = (mesh_id, metallic=0.0, roughness=1.0, base_r=1.0, base_g=1.0, base_b=1.0))]
    pub fn set_mesh_pbr(
        &self,
        mesh_id: u32,
        metallic: f32,
        roughness: f32,
        base_r: f32,
        base_g: f32,
        base_b: f32,
    ) {
        self.window.set_mesh_pbr(mesh_id, metallic, roughness, base_r, base_g, base_b);
    }

    /// 閾値ブルーム。輝度が threshold を超えた画素だけをぼかして加算する。
    /// intensity<=0 でオフ（画面全体ぼかしはしない）。
    #[pyo3(signature = (threshold=0.85, intensity=0.0))]
    pub fn set_bloom(&self, threshold: f32, intensity: f32) {
        self.window.set_bloom(threshold, intensity);
    }

    /// ワールド 3D メッシュの視錐台カリング。VRM は対象外。
    #[pyo3(signature = (enabled))]
    pub fn set_mesh_cull(&self, enabled: bool) {
        self.window.set_mesh_cull(enabled);
    }

    /// 直前フレームの 3D 統計。(draw_calls, triangles, culled)
    pub fn render_stats(&self) -> (u32, u32, u32) {
        self.window.render_stats()
    }

    #[pyo3(signature = (texture_id, verts, indices))]
    pub fn draw_mesh_3d(&self, texture_id: u32, verts: Vec<Vec<f32>>, indices: Vec<u32>) {
        let cv: Vec<[f32;8]> = verts.iter().map(|v| {
            let mut a = [0f32;8];
            for (i, val) in v.iter().enumerate().take(8) { a[i] = *val; }
            a
        }).collect();
        self.window.queue_mesh_3d(texture_id, cv, indices);
    }

    /// 3D メッシュを GPU に一度載せる。毎フレームは ``draw_mesh_id``。
    #[pyo3(signature = (texture_id, verts, indices, metallic=0.0, roughness=1.0, base_r=1.0, base_g=1.0, base_b=1.0))]
    pub fn upload_mesh_3d(
        &self,
        texture_id: u32,
        verts: Vec<Vec<f32>>,
        indices: Vec<u32>,
        metallic: f32,
        roughness: f32,
        base_r: f32,
        base_g: f32,
        base_b: f32,
    ) -> u32 {
        let cv: Vec<[f32;8]> = verts.iter().map(|v| {
            let mut a = [0f32;8];
            for (i, val) in v.iter().enumerate().take(8) { a[i] = *val; }
            a
        }).collect();
        self.window.upload_mesh_3d(
            texture_id, cv, indices, metallic, roughness, [base_r, base_g, base_b],
        )
    }

    #[pyo3(signature = (mesh_id))]
    pub fn draw_mesh_id(&self, mesh_id: u32) {
        self.window.queue_retained_mesh_3d(mesh_id);
    }

    /// 保持メッシュをインスタンス描画。各行は x,y,z[,sx,sy,sz[,yaw]]。
    #[pyo3(signature = (mesh_id, instances))]
    pub fn draw_mesh_instances(&self, mesh_id: u32, instances: Vec<Vec<f32>>) {
        use crate::renderer::Instance3D;
        let packed: Vec<Instance3D> = instances
            .iter()
            .filter_map(|row| {
                if row.len() < 3 {
                    return None;
                }
                let x = row[0];
                let y = row[1];
                let z = row[2];
                let (sx, sy, sz, yaw) = match row.len() {
                    3 => (1.0, 1.0, 1.0, 0.0),
                    4 => (row[3], row[3], row[3], 0.0),
                    6 => (row[3], row[4], row[5], 0.0),
                    _ => (
                        row.get(3).copied().unwrap_or(1.0),
                        row.get(4).copied().unwrap_or(1.0),
                        row.get(5).copied().unwrap_or(1.0),
                        row.get(6).copied().unwrap_or(0.0),
                    ),
                };
                Some(Instance3D {
                    pos: [x, y, z],
                    yaw,
                    scale: [sx, sy, sz],
                    _pad: 0.0,
                })
            })
            .collect();
        self.window.queue_mesh_instances(mesh_id, packed);
    }

    #[pyo3(signature = (mesh_id))]
    pub fn unload_mesh_3d(&self, mesh_id: u32) {
        self.window.unload_mesh_3d(mesh_id);
    }
    #[pyo3(signature = (texture_id, verts, shader_id=0u32, shader_params=None))]
    pub fn draw_mesh(&self, texture_id: u32, verts: Vec<Vec<f32>>, shader_id: u32, shader_params: Option<Vec<f32>>) {
        let params: [f32;4] = if let Some(p) = shader_params {
            [p.get(0).copied().unwrap_or(1.0),
             p.get(1).copied().unwrap_or(1.0),
             p.get(2).copied().unwrap_or(1.0),
             p.get(3).copied().unwrap_or(1.0)]
        } else { [1.0;4] };
        let cv: Vec<[f32;5]> = verts.iter().map(|v| [v[0], v[1], v[2], v[3], v[4]]).collect();
        self.window.draw_mesh(texture_id, cv, shader_id, params);
    }
    // queue_skinned_mesh は Python に公開しない
    pub fn create_instance_batch(&self, texture_id: u32, capacity: u32, sprite_w: f32, sprite_h: f32) -> u32 {
        let mut rg = lock_recover(&self.window.renderer);
        if let Some(r) = rg.as_mut() {
            r.create_instance_batch(texture_id, capacity, sprite_w, sprite_h)
        } else { 0 }
    }
    pub fn update_instance_batch(&self, batch_id: u32, data: Vec<[f32;6]>) {
        let mut rg = lock_recover(&self.window.renderer);
        if let Some(r) = rg.as_mut() { r.update_instance_batch(batch_id, &data); }
    }
    pub fn draw_instance_batch(&self, batch_id: u32) {
        let mut rg = lock_recover(&self.window.renderer);
        if let Some(r) = rg.as_mut() { r.queue_instance_batch(batch_id); }
    }
}

impl Engine {
    pub(crate) fn with_audio<F>(&self, f: F) -> PyResult<()>
    where F: FnOnce(&AudioEngine) -> Result<(), String>
    {
        match lock_recover(&self.audio).as_ref() {
            Some(a) => f(a).map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e)),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod keymap_tests {
    use super::Engine;

    #[test]
    fn default_keymap_has_escape() {
        let m = Engine::default_keymap();
        assert_eq!(m.get("ESCAPE"), Some(&41));
        assert!(m.contains_key("Z"));
        assert!(m.contains_key("SPACE"));
    }
}
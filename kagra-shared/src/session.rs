//! 共有セッション状態（ネイティブシェルが毎フレーム駆動）。

use crate::audio::AudioLevels;
use crate::driving::DrivingScene;
use crate::game::{DemoGame, GamePhase, GAME_ID, GAME_TITLE};
use crate::input::{KeyEvent, PointerEvent, PointerPhase, VirtualPad};
use crate::save::{SaveGame, Settings};
use crate::scene::{DemoScene, DrawList};
use crate::ui::{PauseMenu, UiAction, UiMode};
use crate::vehicle::DriveInput;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub struct FrameStats {
    pub frame: u64,
    pub width: u32,
    pub height: u32,
    pub paused: bool,
    pub pointer_count: u32,
    /// km/h（絶対値）。運転モードのときだけ意味を持つ。
    pub speed_kmh: f32,
    /// 後退中か。
    pub reversing: bool,
    /// 走行距離（m）。
    pub distance_m: f32,
    pub audio: AudioLevels,
    /// pickup / dropoff / complete
    pub mission: String,
    pub mission_progress: f32,
    pub ui: UiMode,
    pub traffic_count: u32,
    pub game_id: &'static str,
    pub game_title: &'static str,
    pub game_phase: GamePhase,
    pub game_time_s: f32,
    pub game_score: u32,
    pub game_best_s: Option<f32>,
    pub has_cargo: bool,
    pub objective: String,
}

impl Default for FrameStats {
    fn default() -> Self {
        Self {
            frame: 0,
            width: 0,
            height: 0,
            paused: false,
            pointer_count: 0,
            speed_kmh: 0.0,
            reversing: false,
            distance_m: 0.0,
            audio: AudioLevels::default(),
            mission: "pickup".into(),
            mission_progress: 0.0,
            ui: UiMode::Hud,
            traffic_count: 0,
            game_id: GAME_ID,
            game_title: GAME_TITLE,
            game_phase: GamePhase::Title,
            game_time_s: 0.0,
            game_score: 0,
            game_best_s: None,
            has_cargo: false,
            objective: "title".into(),
        }
    }
}

/// どのシーンを動かすか。既定は運転デモ。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SceneKind {
    /// 3D の運転デモ。
    #[default]
    Driving,
    /// 2D のタッチデモ。描画経路の最小確認用に残してある。
    Demo2D,
}

#[derive(Debug)]
pub struct SharedSession {
    pub width: u32,
    pub height: u32,
    pub paused: bool,
    pub frame: u64,
    pub asset_root: String,
    pub pad: VirtualPad,
    pub kind: SceneKind,
    pub scene: DemoScene,
    pub driving: DrivingScene,
    pub game: DemoGame,
    pub settings: Settings,
    pointers: Vec<PointerEvent>,
    pending_keys: Vec<KeyEvent>,
    /// このフレームで新しく触れた座標。`poll_pointers` の意味を変えずに
    /// 「押した瞬間」をシーンへ渡すための別キュー。
    pending_taps: Vec<(f32, f32)>,
    #[cfg(feature = "render")]
    renderer: Option<crate::render::Renderer>,
    /// 運転シーンのメッシュを載せたときのハンドル。
    #[cfg(feature = "render")]
    mesh_ids: Option<crate::driving::MeshIds>,
}

impl Default for SharedSession {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            paused: false,
            frame: 0,
            asset_root: String::new(),
            pad: VirtualPad::new(0.25),
            kind: SceneKind::default(),
            scene: DemoScene::default(),
            driving: DrivingScene::default(),
            // 単体テスト／オフスクリーンはすぐ運転できるよう Playing 開始。
            // Wasm デモは `show_title()` でタイトルに戻す。
            game: DemoGame {
                phase: GamePhase::Playing,
                ..DemoGame::default()
            },
            settings: Settings::default(),
            pointers: Vec::new(),
            pending_keys: Vec::new(),
            pending_taps: Vec::new(),
            #[cfg(feature = "render")]
            renderer: None,
            #[cfg(feature = "render")]
            mesh_ids: None,
        }
    }
}

impl SharedSession {
    pub fn create_surface(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        #[cfg(feature = "render")]
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height);
        }
    }

    pub fn set_asset_root(&mut self, root: impl Into<String>) {
        self.asset_root = root.into();
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn push_pointer(&mut self, ev: PointerEvent) {
        if matches!(ev.phase, PointerPhase::Begin) {
            match self.game.phase {
                GamePhase::Title | GamePhase::Complete if self.kind == SceneKind::Driving => {
                    self.start_game();
                }
                _ if self.paused && self.kind == SceneKind::Driving => {
                    self.handle_pause_tap(ev.x, ev.y);
                }
                _ => self.pending_taps.push((ev.x, ev.y)),
            }
        }
        // 同一 id は最新で置換、begin は追加
        if let Some(slot) = self.pointers.iter_mut().find(|p| p.id == ev.id) {
            *slot = ev;
        } else {
            self.pointers.push(ev);
        }
        // end/cancel は次フレームで掃除してもよいが、ここでは保持して poll で返す
    }

    fn handle_pause_tap(&mut self, x: f32, y: f32) {
        let menu = PauseMenu::layout(self.width, self.height);
        match menu.hit(x, y) {
            Some(UiAction::Resume) => self.resume(),
            Some(UiAction::Restart) => {
                self.start_game();
            }
            Some(UiAction::ToggleMute) => {
                self.settings.muted = !self.settings.muted;
            }
            None => {}
        }
    }

    /// タイトル画面へ。デモの起動時用。
    pub fn show_title(&mut self) {
        let best = self.game.best_time_s;
        self.driving.restart();
        self.game = DemoGame {
            phase: GamePhase::Title,
            best_time_s: best,
            ..DemoGame::default()
        };
        self.paused = false;
        self.driving.set_input(DriveInput::default());
    }

    /// 配送ランを最初から開始する。
    pub fn start_game(&mut self) {
        let best = self.game.best_time_s;
        self.driving.restart();
        self.game = DemoGame {
            best_time_s: best,
            ..DemoGame::default()
        };
        self.game.start();
        self.paused = false;
        self.kind = SceneKind::Driving;
    }

    pub fn set_pad(&mut self, x: f32, y: f32) {
        self.pad.set_stick(x, y);
        self.pending_keys.extend(self.pad.drain_key_events());
        // 運転シーンでは左右がハンドル、上下がアクセルとブレーキ。仮想パッド
        // しか持たないシェルでも運転できるようにしておく。
        self.set_drive(x, (-y).max(0.0), y.max(0.0));
    }

    /// 連続値のドライバ入力。仮想ハンドルや傾きセンサを持つシェルはこちらを使う。
    pub fn set_drive(&mut self, steer: f32, throttle: f32, brake: f32) {
        if !self.game.is_driving() {
            self.driving.set_input(DriveInput::default());
            return;
        }
        let sens = self.settings.steer_sensitivity;
        self.driving.set_input(DriveInput {
            steer: (steer * sens).clamp(-1.0, 1.0),
            throttle,
            brake,
        });
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings.clamped();
    }

    pub fn save_json(&self) -> Result<String, String> {
        SaveGame::capture(self).to_json()
    }

    pub fn load_json(&mut self, json: &str) -> Result<(), String> {
        SaveGame::from_json(json)?.apply(self);
        Ok(())
    }

    pub fn audio_levels(&self) -> AudioLevels {
        AudioLevels::from_truck(
            &self.driving.truck,
            self.driving.input,
            self.settings.muted,
            self.settings.master_volume,
        )
    }

    pub fn set_scene_kind(&mut self, kind: SceneKind) {
        self.kind = kind;
    }

    pub fn poll_pointers(&mut self) -> Vec<PointerEvent> {
        let out = self.pointers.clone();
        self.pointers.retain(|p| {
            matches!(
                p.phase,
                crate::input::PointerPhase::Begin | crate::input::PointerPhase::Move
            )
        });
        out
    }

    pub fn poll_keys(&mut self) -> Vec<KeyEvent> {
        std::mem::take(&mut self.pending_keys)
    }

    pub fn request_frame(&mut self) -> FrameStats {
        if !self.paused {
            self.frame = self.frame.saturating_add(1);
            let taps = std::mem::take(&mut self.pending_taps);
            match self.kind {
                SceneKind::Driving => {
                    if self.game.is_driving() {
                        self.driving.update();
                        self.game.tick(&self.driving.mission);
                    }
                }
                SceneKind::Demo2D => {
                    let pad = self.pad.stick();
                    self.scene.update(self.width, self.height, pad, &taps);
                }
            }
        }
        self.stats()
    }

    fn stats(&self) -> FrameStats {
        FrameStats {
            frame: self.frame,
            width: self.width,
            height: self.height,
            paused: self.paused,
            pointer_count: self.pointers.len() as u32,
            speed_kmh: self.driving.truck.speed_kmh(),
            reversing: self.driving.truck.is_reversing(),
            distance_m: self.driving.odometer,
            audio: self.audio_levels(),
            mission: self.driving.mission.label().into(),
            mission_progress: self
                .driving
                .mission
                .progress_along_route(self.driving.path_s),
            ui: if self.paused {
                UiMode::Pause
            } else {
                UiMode::Hud
            },
            traffic_count: self.driving.traffic.count() as u32,
            game_id: GAME_ID,
            game_title: GAME_TITLE,
            game_phase: self.game.phase,
            game_time_s: self.game.time_s,
            game_score: self.game.score,
            game_best_s: self.game.best_time_s,
            has_cargo: self.game.has_cargo,
            objective: self.game.objective_key(&self.driving.mission).into(),
        }
    }

    /// 現在の状態から 2D の描画内容を作る。GPU に触らないので単体テスト可能。
    /// 運転シーンでは HUD になる。
    pub fn draw_list(&self) -> DrawList {
        match self.kind {
            SceneKind::Driving => self.driving.build_hud(self.width, self.height, self.paused),
            SceneKind::Demo2D => self
                .scene
                .draw(self.width, self.height, self.frame, self.paused),
        }
    }

    pub fn stats_json(&self) -> String {
        serde_json::to_string(&self.stats()).unwrap_or_else(|_| "{}".into())
    }
}

#[cfg(feature = "render")]
impl SharedSession {
    /// レンダラを束ねる。セッション側の画面サイズに合わせ、運転シーンの
    /// メッシュをこの時点で GPU に載せる。
    pub fn attach_renderer(&mut self, mut renderer: crate::render::Renderer) {
        renderer.resize(self.width, self.height);
        let set = crate::driving::MeshSet::build(self.driving.truck.spec.size);
        let [ground, road, dash, pole, truck, cab, sky, shadow, building] = set.as_slice();
        self.mesh_ids = Some(crate::driving::MeshIds {
            ground: renderer.upload_mesh(ground),
            road: renderer.upload_mesh(road),
            dash: renderer.upload_mesh(dash),
            pole: renderer.upload_mesh(pole),
            truck: renderer.upload_mesh(truck),
            cab: renderer.upload_mesh(cab),
            sky: renderer.upload_mesh(sky),
            shadow: renderer.upload_mesh(shadow),
            building: renderer.upload_mesh(building),
        });
        self.renderer = Some(renderer);
    }

    pub fn detach_renderer(&mut self) {
        self.renderer = None;
        self.mesh_ids = None;
    }

    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }

    /// 現在のシーンを 1 枚描く。レンダラ未接続ならエラー。
    pub fn render(&mut self) -> Result<(), String> {
        let hud = self.draw_list();
        let Some(r) = self.renderer.as_mut() else {
            return Err("no renderer attached".into());
        };
        let world = match (self.kind, self.mesh_ids.as_ref()) {
            (SceneKind::Driving, Some(ids)) => Some(self.driving.build_scene(ids, r.aspect())),
            _ => None,
        };
        r.render_frame(world.as_ref(), &hud)
    }

    /// オフスクリーンレンダラの内容を RGBA8 で読み出す。
    pub fn render_readback(&self) -> Result<Vec<u8>, String> {
        match self.renderer.as_ref() {
            Some(r) => r.read_rgba(),
            None => Err("no renderer attached".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::PointerPhase;

    fn demo2d() -> SharedSession {
        let mut s = SharedSession::default();
        s.set_scene_kind(SceneKind::Demo2D);
        s
    }

    #[test]
    fn frame_advances_and_scene_follows_pad() {
        let mut s = demo2d();
        s.create_surface(800, 600);
        s.set_pad(1.0, 0.0);
        let start = s.scene.player_x;
        for _ in 0..30 {
            s.request_frame();
        }
        assert_eq!(s.frame, 30);
        assert!(s.scene.player_x > start);
    }

    #[test]
    fn paused_session_freezes_scene() {
        let mut s = demo2d();
        s.set_pad(1.0, 0.0);
        s.pause();
        let before = s.scene.player_x;
        for _ in 0..30 {
            s.request_frame();
        }
        assert_eq!(s.frame, 0);
        assert_eq!(s.scene.player_x, before);
    }

    #[test]
    fn tap_is_consumed_once() {
        let mut s = demo2d();
        s.push_pointer(PointerEvent {
            id: 0,
            x: 10.0,
            y: 20.0,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        });
        // 押しっぱなしでも波紋は 1 回だけ増える
        let first = s.request_frame();
        assert_eq!(first.frame, 1);
        let before = s.draw_list().quads.len();
        s.request_frame();
        let after = s.draw_list().quads.len();
        assert!(after <= before, "held pointer kept spawning ripples");
    }

    #[test]
    fn draw_list_is_not_empty() {
        assert!(!demo2d().draw_list().quads.is_empty());
        // 運転シーンでも HUD は出る。
        assert!(!SharedSession::default().draw_list().quads.is_empty());
    }

    #[test]
    fn driving_is_the_default_scene() {
        let mut s = SharedSession::default();
        s.set_drive(0.0, 1.0, 0.0);
        for _ in 0..120 {
            s.request_frame();
        }
        let stats = s.request_frame();
        assert!(stats.speed_kmh > 10.0, "throttle should build speed");
        assert!(stats.distance_m > 5.0);
    }

    #[test]
    fn pad_up_is_throttle_and_down_is_brake() {
        let mut s = SharedSession::default();
        // 画面座標に合わせ、上（y が負）がアクセル。
        s.set_pad(0.0, -1.0);
        for _ in 0..120 {
            s.request_frame();
        }
        let moving = s.request_frame().speed_kmh;
        assert!(moving > 5.0);

        s.set_pad(0.0, 1.0);
        for _ in 0..180 {
            s.request_frame();
        }
        // 減速して停車帯に入り、そのままだと後退に入る。
        assert!(
            s.driving.truck.speed < 5.0,
            "pad down should scrub speed, got {}",
            s.driving.truck.speed
        );
        for _ in 0..180 {
            s.request_frame();
        }
        assert!(
            s.driving.truck.is_reversing() || s.driving.truck.speed.abs() < 0.5,
            "pad down at rest should reverse or stay stopped, speed={}",
            s.driving.truck.speed
        );
    }

    #[test]
    fn paused_driving_does_not_move() {
        let mut s = SharedSession::default();
        s.set_drive(0.0, 1.0, 0.0);
        s.pause();
        for _ in 0..60 {
            s.request_frame();
        }
        assert_eq!(s.request_frame().distance_m, 0.0);
    }

    #[test]
    fn stats_json_reports_driving_fields() {
        let s = SharedSession::default();
        let j = s.stats_json();
        assert!(j.contains("speed_kmh"), "{j}");
        assert!(j.contains("distance_m"), "{j}");
        assert!(j.contains("audio"), "{j}");
        assert!(j.contains("mission"), "{j}");
        assert!(j.contains("traffic_count"), "{j}");
    }

    #[test]
    fn pause_menu_resume_unpauses() {
        let mut s = SharedSession::default();
        s.create_surface(1280, 720);
        s.pause();
        let quads = PauseMenu::layout(1280, 720).quads();
        let resume = quads[0];
        s.push_pointer(PointerEvent {
            id: 1,
            x: resume.x + resume.w * 0.5,
            y: resume.y + resume.h * 0.5,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        });
        assert!(!s.paused);
    }

    #[test]
    fn pause_menu_restart_resets_odometer() {
        let mut s = SharedSession::default();
        s.create_surface(1280, 720);
        s.set_drive(0.0, 1.0, 0.0);
        for _ in 0..120 {
            s.request_frame();
        }
        assert!(s.driving.odometer > 1.0);
        s.pause();
        let quads = PauseMenu::layout(1280, 720).quads();
        let restart = quads[1];
        s.push_pointer(PointerEvent {
            id: 2,
            x: restart.x + restart.w * 0.5,
            y: restart.y + restart.h * 0.5,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        });
        assert_eq!(s.driving.odometer, 0.0);
        assert!(!s.paused);
    }

    #[test]
    fn save_load_roundtrip_via_session() {
        let mut s = SharedSession::default();
        s.set_drive(0.0, 1.0, 0.0);
        for _ in 0..180 {
            s.request_frame();
        }
        s.set_settings(Settings {
            master_volume: 0.5,
            steer_sensitivity: 1.2,
            muted: true,
        });
        let json = s.save_json().unwrap();
        let mut other = SharedSession::default();
        other.load_json(&json).unwrap();
        assert!((other.driving.odometer - s.driving.odometer).abs() < 1e-3);
        assert!((other.settings.master_volume - 0.5).abs() < 1e-4);
        assert!(other.settings.muted);
        assert_eq!(other.audio_levels(), AudioLevels::default());
    }

    #[test]
    fn steer_sensitivity_scales_input() {
        let mut soft = SharedSession::default();
        soft.set_settings(Settings {
            steer_sensitivity: 0.5,
            ..Default::default()
        });
        soft.set_drive(1.0, 0.0, 0.0);
        assert!((soft.driving.input.steer - 0.5).abs() < 1e-4);
    }

    #[test]
    fn demo_game_title_blocks_driving_until_start() {
        let mut s = SharedSession::default();
        s.show_title();
        s.set_drive(0.0, 1.0, 0.0);
        for _ in 0..60 {
            s.request_frame();
        }
        assert_eq!(s.driving.odometer, 0.0);
        assert_eq!(s.game.phase, crate::game::GamePhase::Title);

        s.start_game();
        s.set_drive(0.0, 1.0, 0.0);
        for _ in 0..60 {
            s.request_frame();
        }
        assert!(s.driving.odometer > 1.0);
        assert_eq!(s.game.phase, crate::game::GamePhase::Playing);
        assert!(s.game.time_s > 0.5);
    }

    #[test]
    fn demo_game_completes_when_mission_done() {
        let mut s = SharedSession::default();
        s.start_game();
        s.driving.path_s = s.driving.mission.pickup_s;
        s.driving.mission.update(s.driving.path_s);
        s.game.tick(&s.driving.mission);
        assert!(s.game.has_cargo);
        s.driving.path_s = s.driving.mission.dropoff_s;
        s.driving.mission.update(s.driving.path_s);
        s.game.tick(&s.driving.mission);
        assert_eq!(s.game.phase, crate::game::GamePhase::Complete);
        assert!(s.game.score >= 500);
    }
}

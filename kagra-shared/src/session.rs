//! 共有セッション状態（ネイティブシェルが毎フレーム駆動）。

use crate::audio::AudioLevels;
use crate::collectathon::{
    CollectathonScene, WalkInput, GAME_ID as ISLE_ID, GAME_TITLE as ISLE_TITLE,
};
use crate::driving::DrivingScene;
use crate::game::{DemoGame, GamePhase, GAME_ID, GAME_TITLE};
use crate::input::{KeyEvent, PointerEvent, PointerPhase, VirtualPad};
use crate::kit_host::KitHost;
use crate::playtest::Playtest;
use crate::save::{SaveGame, Settings};
use crate::scene::{DemoScene, DrawList};
use crate::ui::{PauseMenu, UiAction, UiMode};
use crate::vehicle::DriveInput;
use crate::world_play::WorldPlay;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub stars: u32,
    pub coins: u32,
    pub star_need: u32,
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
            stars: 0,
            coins: 0,
            star_need: 0,
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
    /// Crest Isle 収集（Kenney 風カプセル。VRM ではない）。
    Collectathon,
    /// パッケージしたジャンル例。`WorldPlay` が dump JSON を進める。
    World,
    /// A game kit behind [`SharedSession::kit`] (Neon Cat, Lumen, ...). Not a
    /// WorldPlay genre: the kit owns the tick.
    Kit,
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
    pub isle: CollectathonScene,
    pub game: DemoGame,
    pub settings: Settings,
    /// `load_game_json` で入ったジャンル dump。`kind == World` のときだけ進む。
    pub world: Option<WorldPlay>,
    /// The loaded game kit (`"genre": "neon_cat"`, `"neon_run"`, ...), kept off
    /// WorldPlay on purpose. `kit_host::KITS` lists what can load.
    pub kit: Option<Box<dyn KitHost>>,
    pointers: Vec<PointerEvent>,
    pending_keys: Vec<KeyEvent>,
    /// このフレームで新しく触れた座標。`poll_pointers` の意味を変えずに
    /// 「押した瞬間」をシーンへ渡すための別キュー。
    pending_taps: Vec<(f32, f32)>,
    /// Debug / preview handoff. Off until a host enables it.
    playtest: Playtest,
    #[cfg(feature = "render")]
    renderer: Option<crate::render::Renderer>,
    /// 運転シーンのメッシュを載せたときのハンドル。
    #[cfg(feature = "render")]
    mesh_ids: Option<crate::driving::MeshIds>,
    #[cfg(feature = "render")]
    isle_mesh_ids: Option<crate::collectathon::MeshIds>,
    /// World draws need `compile_meshes`, not the driving/isle list from attach.
    #[cfg(feature = "render")]
    world_meshes: bool,
    /// HTML title / result card is the menu. Skip the in-canvas flow column.
    external_menu: bool,
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
            isle: CollectathonScene::default(),
            // 単体テスト／オフスクリーンはすぐ運転できるよう Playing 開始。
            // Wasm デモは `show_title()` でタイトルに戻す。
            game: DemoGame {
                phase: GamePhase::Playing,
                ..DemoGame::default()
            },
            settings: Settings::default(),
            world: None,
            kit: None,
            pointers: Vec::new(),
            pending_keys: Vec::new(),
            pending_taps: Vec::new(),
            playtest: Playtest::new(false),
            #[cfg(feature = "render")]
            renderer: None,
            #[cfg(feature = "render")]
            mesh_ids: None,
            #[cfg(feature = "render")]
            isle_mesh_ids: None,
            #[cfg(feature = "render")]
            world_meshes: false,
            external_menu: false,
        }
    }
}

/// Neon Run / Neon Cat use `title` / `play` / `pause` / `result`.
/// `FrameStats` only has the demo phases, which the preview page reads.
fn flow_stats_phase(name: Option<&str>) -> GamePhase {
    match name.unwrap_or("title") {
        "play" | "pause" => GamePhase::Playing,
        "result" => GamePhase::Complete,
        _ => GamePhase::Title,
    }
}

impl SharedSession {
    /// The loaded kit as a Neon Cat, if it is one.
    pub fn cat(&self) -> Option<&crate::neon_cat::NeonCat> {
        self.kit.as_ref()?.as_any().downcast_ref()
    }

    pub fn cat_mut(&mut self) -> Option<&mut crate::neon_cat::NeonCat> {
        self.kit.as_mut()?.as_any_mut().downcast_mut()
    }

    /// The loaded kit as a Lumen run, if it is one.
    pub fn neon(&self) -> Option<&crate::neon_run::NeonRun> {
        self.kit.as_ref()?.as_any().downcast_ref()
    }

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
        let path = PathBuf::from(&self.asset_root);
        crate::assets::set_packaged_root(if path.as_os_str().is_empty() {
            None
        } else {
            Some(path)
        });
    }

    /// ジャンル dump を読み、シーンを `World` にする。タイトルのまま待つ。
    pub fn load_game_json(&mut self, json: &str) -> Result<(), String> {
        crate::world_doc::clear_runtime_asset_cache();
        if let Some(kit) = crate::kit_host::load(json) {
            let kit = kit?;
            let dump = kit.dump_json();
            self.kit = Some(kit);
            self.world = None;
            self.kind = SceneKind::Kit;
            self.paused = false;
            #[cfg(feature = "render")]
            {
                self.world_meshes = false;
            }
            self.playtest.snapshot_loaded(&dump);
            return Ok(());
        }
        self.kit = None;
        let mut play = WorldPlay::from_json(json)?;
        play.load_progress();
        let dump = play.doc.to_json().unwrap_or_else(|_| json.to_string());
        self.world = Some(play);
        self.kind = SceneKind::World;
        self.paused = false;
        #[cfg(feature = "render")]
        {
            self.world_meshes = false;
        }
        self.playtest.snapshot_loaded(&dump);
        Ok(())
    }

    /// `load_game_json` のファイル版。Android は APK から展開したパスを渡す。
    pub fn load_game_file(&mut self, path: &str) -> Result<(), String> {
        let json = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        self.load_game_json(&json)
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn push_pointer(&mut self, ev: PointerEvent) {
        self.playtest.set_viewport(self.width, self.height);
        if self.playtest.on_pointer(ev) {
            return;
        }
        self.playtest.record_pointer(ev);
        if self.kind == SceneKind::Kit {
            // Frame clock: flick and swipe speeds assume about 60 Hz events.
            let t = self.frame as f32 / 60.0;
            let (w, h) = (self.width, self.height);
            if let Some(kit) = self.kit.as_mut() {
                kit.touch(ev, w, h, t);
                if matches!(ev.phase, PointerPhase::Begin) {
                    let handled = kit.pointer(ev.x, ev.y, w, h);
                    if !handled && !kit.playing() && kit.phase_name() != "pause" {
                        kit.confirm();
                    }
                }
            }
        }
        if self.kind == SceneKind::World && matches!(ev.phase, PointerPhase::Begin) {
            if let Some(play) = self.world.as_mut() {
                let handled = play.apply_flow_pointer(ev.x, ev.y, self.width, self.height);
                if !handled && !play.game.is_playing() {
                    play.confirm();
                    self.playtest.record_confirm();
                }
            }
        }
        if matches!(ev.phase, PointerPhase::Begin) {
            let isle_phase = self.isle.game.phase;
            match (self.kind, self.game.phase, isle_phase) {
                (SceneKind::Kit, _, _) => {}
                (SceneKind::Driving, GamePhase::Title | GamePhase::Complete, _)
                | (SceneKind::Collectathon, _, GamePhase::Title | GamePhase::Complete) => {
                    self.start_game();
                }
                _ if self.paused
                    && matches!(self.kind, SceneKind::Driving | SceneKind::Collectathon) =>
                {
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
    /// Hosts drop their own touch steering and action buttons when true.
    pub fn uses_swipe(&self) -> bool {
        self.kind == SceneKind::Kit && self.kit.as_ref().is_some_and(|k| k.uses_swipe())
    }

    pub fn show_title(&mut self) {
        if self.kind == SceneKind::Kit {
            if let Some(kit) = self.kit.as_mut() {
                kit.show_title();
            }
            self.paused = false;
            return;
        }
        if self.kind == SceneKind::Collectathon {
            self.isle.show_title();
            self.paused = false;
            return;
        }
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

    /// 配送ラン、または Crest Isle を最初から開始する。
    pub fn start_game(&mut self) {
        if self.kind == SceneKind::Kit {
            if let Some(kit) = self.kit.as_mut() {
                kit.confirm();
            }
            self.paused = false;
            return;
        }
        if self.kind == SceneKind::Collectathon {
            self.isle.start();
            self.paused = false;
            return;
        }
        if self.kind == SceneKind::World {
            if let Some(play) = self.world.as_mut() {
                play.confirm();
            }
            self.paused = false;
            return;
        }
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

    /// Crest Isle を選んでタイトルへ。Android / Wasm の既定起動用。
    pub fn boot_collectathon(&mut self) {
        self.kind = SceneKind::Collectathon;
        self.isle.show_title();
        self.paused = false;
    }

    pub fn set_pad(&mut self, x: f32, y: f32) {
        self.pad.set_stick(x, y);
        self.pending_keys.extend(self.pad.drain_key_events());
        if self.kind == SceneKind::Kit {
            // Virtual pad +y is down the screen. Up climbs / jumps.
            self.set_play_input(x, -y, false, false, false);
            return;
        }
        if self.kind == SceneKind::Collectathon {
            // 画面座標 +y 下。上（負の y）が島の奥（+Z を向いたカメラの前）。
            let jump = self.isle.input.jump;
            self.isle.set_input(WalkInput {
                lx: x,
                lz: -y,
                jump,
                attack: false,
                dodge: false,
            });
            return;
        }
        // 運転シーンでは左右がハンドル、上下がアクセルとブレーキ。仮想パッド
        // しか持たないシェルでも運転できるようにしておく。
        self.set_drive(x, (-y).max(0.0), y.max(0.0));
    }

    /// パッケージした `WorldPlay` への歩き。攻撃と回避は 0/1。
    pub fn set_play_input(&mut self, lx: f32, lz: f32, jump: bool, attack: bool, dodge: bool) {
        if self.playtest.blocks_sim() {
            return;
        }
        self.playtest.note_walk(lx, lz, jump, attack, dodge);
        if self.kind == SceneKind::Kit {
            if let Some(kit) = self.kit.as_mut() {
                kit.walk(lx, lz, jump);
            }
            return;
        }
        if self.kind == SceneKind::Collectathon {
            self.set_walk(lx, lz, jump);
            return;
        }
        if self.kind != SceneKind::World {
            return;
        }
        let Some(play) = self.world.as_mut() else {
            return;
        };
        if !play.game.is_playing() || play.flow_paused() {
            play.input = WalkInput::default();
            return;
        }
        play.input = WalkInput {
            lx,
            lz,
            jump,
            attack,
            dodge,
        }
        .clamped();
    }

    /// Crest Isle の歩き。`lx`/`lz` は -1..1、`jump` は今フレーム跳ぶか。
    /// `World` のときは攻撃なしの `set_play_input` になる。
    pub fn set_walk(&mut self, lx: f32, lz: f32, jump: bool) {
        if self.playtest.blocks_sim() {
            return;
        }
        if self.kind == SceneKind::World {
            self.set_play_input(lx, lz, jump, false, false);
            return;
        }
        self.playtest.note_walk(lx, lz, jump, false, false);
        if self.kind != SceneKind::Collectathon {
            return;
        }
        if !self.isle.game.is_playing() {
            self.isle.set_input(WalkInput::default());
            return;
        }
        self.isle.set_input(
            WalkInput {
                lx,
                lz,
                jump,
                attack: false,
                dodge: false,
            }
            .clamped(),
        );
    }

    pub fn set_jump(&mut self, jump: bool) {
        if self.kind == SceneKind::Collectathon {
            self.isle.input.jump = jump;
        }
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
        self.playtest.set_viewport(self.width, self.height);
        self.playtest.advance(crate::scene::FIXED_DT);
        if self.playtest.blocks_sim() {
            return self.stats();
        }
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
                SceneKind::Collectathon => {
                    if self.isle.game.is_playing() {
                        self.isle
                            .apply_pointers(self.width, self.height, &self.pointers);
                        self.isle.update();
                    }
                    let _ = taps;
                    self.playtest.commit_walk(crate::scene::FIXED_DT);
                }
                SceneKind::Demo2D => {
                    let pad = self.pad.stick();
                    self.scene.update(self.width, self.height, pad, &taps);
                }
                SceneKind::World => {
                    let _ = taps;
                    if let Some(play) = self.world.as_mut() {
                        if play.game.is_playing() {
                            play.tick(crate::scene::FIXED_DT);
                        }
                    }
                    self.playtest.commit_walk(crate::scene::FIXED_DT);
                }
                SceneKind::Kit => {
                    let _ = taps;
                    if let Some(kit) = self.kit.as_mut() {
                        kit.tick(crate::scene::FIXED_DT);
                    }
                }
            }
        }
        if self.playtest.wants_snapshot() {
            self.capture_playtest_dump();
        }
        self.stats()
    }

    fn capture_playtest_dump(&mut self) {
        if !self.playtest.wants_snapshot() {
            return;
        }
        let dump = if self.kind == SceneKind::Kit {
            self.kit.as_ref().map(|kit| kit.dump_json())
        } else {
            self.world.as_ref().and_then(|play| play.doc.to_json().ok())
        };
        let dump = dump.unwrap_or_else(|| self.stats_json());
        self.playtest.snapshot_dump(&dump);
    }

    fn stats(&self) -> FrameStats {
        let isle = self.kind == SceneKind::Collectathon;
        let packaged = self.kind == SceneKind::World;
        let play = self.world.as_ref();
        FrameStats {
            frame: self.frame,
            width: self.width,
            height: self.height,
            paused: self.paused,
            pointer_count: self.pointers.len() as u32,
            speed_kmh: if isle {
                0.0
            } else {
                self.driving.truck.speed_kmh()
            },
            reversing: self.driving.truck.is_reversing(),
            distance_m: if isle {
                (self.isle.walker.z - crate::collectathon::START_XZ.1).abs()
            } else {
                self.driving.odometer
            },
            audio: self.audio_levels(),
            mission: if packaged {
                play.and_then(|p| p.doc.genre.clone())
                    .unwrap_or_else(|| "world".into())
            } else if isle {
                format!(
                    "{}/{}",
                    self.isle.game.stars,
                    crate::collectathon::STAR_NEED
                )
            } else {
                self.driving.mission.label().into()
            },
            mission_progress: if isle {
                self.isle.game.stars as f32 / crate::collectathon::STAR_NEED as f32
            } else {
                self.driving
                    .mission
                    .progress_along_route(self.driving.path_s)
            },
            ui: if self.paused || (packaged && play.is_some_and(|p| p.flow_paused())) {
                UiMode::Pause
            } else {
                UiMode::Hud
            },
            traffic_count: self.driving.traffic.count() as u32,
            game_id: if packaged {
                "world"
            } else if isle {
                ISLE_ID
            } else {
                GAME_ID
            },
            game_title: if packaged {
                "KAGRA"
            } else if isle {
                ISLE_TITLE
            } else {
                GAME_TITLE
            },
            game_phase: if self.kind == SceneKind::Kit {
                flow_stats_phase(self.kit.as_ref().map(|kit| kit.phase_name()))
            } else if packaged {
                play.map(|p| p.game.phase).unwrap_or(self.game.phase)
            } else if isle {
                self.isle.game.phase
            } else {
                self.game.phase
            },
            game_time_s: if packaged {
                play.map(|p| p.game.time_s).unwrap_or(0.0)
            } else if isle {
                self.isle.game.time_s
            } else {
                self.game.time_s
            },
            game_score: if packaged {
                play.map(|p| p.game.score).unwrap_or(0)
            } else if isle {
                self.isle.game.score
            } else {
                self.game.score
            },
            game_best_s: if isle {
                self.isle.game.best_score.map(|s| s as f32)
            } else {
                self.game.best_time_s
            },
            has_cargo: self.game.has_cargo,
            objective: if packaged {
                play.and_then(|p| p.doc.genre.clone()).unwrap_or_default()
            } else if isle {
                self.isle.game.objective_key().into()
            } else {
                self.game.objective_key(&self.driving.mission).into()
            },
            stars: if isle { self.isle.game.stars } else { 0 },
            coins: if isle { self.isle.game.coins } else { 0 },
            star_need: if isle {
                crate::collectathon::STAR_NEED
            } else {
                0
            },
        }
    }

    /// 現在の状態から 2D の描画内容を作る。GPU に触らないので単体テスト可能。
    /// 運転シーンでは HUD になる。
    pub fn draw_list(&self) -> DrawList {
        let mut list = match self.kind {
            SceneKind::Driving => self.driving.build_hud(self.width, self.height, self.paused),
            SceneKind::Collectathon => self.isle.build_hud(self.width, self.height, self.paused),
            SceneKind::Demo2D => self
                .scene
                .draw(self.width, self.height, self.frame, self.paused),
            // Title / pause / result quads and text ride on the world picture.
            SceneKind::World => self
                .world
                .as_ref()
                .map(|play| play.build_hud(self.width, self.height))
                .unwrap_or_default(),
            SceneKind::Kit => self
                .kit
                .as_ref()
                .map(|kit| kit.hud(self.width, self.height, !self.external_menu))
                .unwrap_or_default(),
        };
        self.playtest.append_hud(&mut list, self.width, self.height);
        list
    }

    pub fn set_playtest_enabled(&mut self, on: bool) {
        self.playtest.set_enabled(on);
        if on {
            self.capture_playtest_dump();
        }
    }

    pub fn set_playtest_external_chrome(&mut self, on: bool) {
        self.playtest.set_external_chrome(on);
    }

    /// The host draws title and result. Neon Cat then skips the in-canvas
    /// CRASH / RETRY / TITLE column so it cannot show through the page.
    pub fn set_external_menu(&mut self, on: bool) {
        self.external_menu = on;
    }

    /// Neon Cat `?debug=1`. Draws kill boxes. Other scenes ignore it.
    pub fn set_debug(&mut self, on: bool) {
        if let Some(kit) = self.kit.as_mut() {
            kit.set_debug(on);
        }
    }

    pub fn set_playtest_device(&mut self, label: &str, lang: &str) {
        self.playtest.set_device(label, lang);
    }

    pub fn playtest_pointer(&mut self, ev: PointerEvent) -> bool {
        self.playtest.set_viewport(self.width, self.height);
        self.playtest.on_pointer(ev)
    }

    pub fn push_key(&mut self, name: &str, down: bool) -> bool {
        if self.playtest.on_key(name, down) {
            return true;
        }
        if self.kind == SceneKind::Kit {
            if let Some(kit) = self.kit.as_mut() {
                return kit.on_key(name, down);
            }
        }
        false
    }

    pub fn playtest_set_note(&mut self, note: &str) {
        self.playtest.set_note(note);
    }

    pub fn playtest_push_text(&mut self, text: &str) {
        self.playtest.push_text(text);
    }

    pub fn playtest_action(&mut self, name: &str) {
        self.playtest.action(name);
    }

    pub fn playtest_open(&self) -> bool {
        self.playtest.is_open()
    }

    pub fn playtest_blocks_input(&self) -> bool {
        self.playtest.blocks_sim()
    }

    /// Clears the one-shot screenshot flag. Hosts read this between
    /// `request_frame` and `render` so the saved frame is the game.
    pub fn playtest_state_json(&mut self) -> String {
        self.playtest.set_viewport(self.width, self.height);
        self.playtest.state_json()
    }

    pub fn playtest_packet_json(&self) -> String {
        self.playtest.packet_json()
    }

    pub fn playtest_set_shot_path(&mut self, path: &str) {
        self.playtest.set_shot_path(path);
    }

    pub fn open_playtest(&mut self) -> bool {
        self.playtest.set_viewport(self.width, self.height);
        self.playtest.try_open()
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
        let isle = crate::collectathon::MeshSet::build();
        let [terrain, water, isky, ishadow, boxy, cone, cylinder] = isle.as_slice();
        self.isle_mesh_ids = Some(crate::collectathon::MeshIds {
            terrain: renderer.upload_mesh(terrain),
            water: renderer.upload_mesh(water),
            sky: renderer.upload_mesh(isky),
            shadow: renderer.upload_mesh(ishadow),
            boxy: renderer.upload_mesh(boxy),
            cone: renderer.upload_mesh(cone),
            cylinder: renderer.upload_mesh(cylinder),
        });
        // A second attach (WebGL2 fallback) must upload into the new device.
        self.world_meshes = false;
        self.renderer = Some(renderer);
    }

    pub fn detach_renderer(&mut self) {
        self.renderer = None;
        self.mesh_ids = None;
        self.isle_mesh_ids = None;
    }

    pub fn has_renderer(&self) -> bool {
        self.renderer.is_some()
    }

    /// Tier, backend, and which post passes ran. `pending` before the first 3D frame.
    pub fn picture_status(&self) -> String {
        self.renderer
            .as_ref()
            .map(|r| r.picture_status())
            .unwrap_or_else(|| "pending".into())
    }

    /// 現在のシーンを 1 枚描く。レンダラ未接続ならエラー。
    pub fn render(&mut self) -> Result<(), String> {
        if self.kind == SceneKind::Kit {
            let hud = self.draw_list();
            let doc = self.kit.as_ref().map(|kit| kit.document().clone());
            let missing = "no game kit loaded";
            let mut renderer = self.renderer.take();
            let result = match (renderer.as_mut(), doc.as_ref()) {
                (Some(r), Some(doc)) => {
                    if !self.world_meshes {
                        r.clear_meshes();
                    }
                    let drawn = r.draw_world_doc_with_hud(doc, &hud);
                    if drawn.is_ok() {
                        self.world_meshes = true;
                    }
                    drawn
                }
                (None, _) => Err("no renderer attached".into()),
                (Some(_), None) => Err(missing.into()),
            };
            self.renderer = renderer;
            return result;
        }
        if self.kind == SceneKind::World {
            let hud = self.draw_list();
            let mut renderer = self.renderer.take();
            let result = match (renderer.as_mut(), self.world.as_ref()) {
                (Some(r), Some(play)) => {
                    // attach_renderer loads driving + isle meshes. WorldDoc ids
                    // start at the primitive/glTF table, so reuse would
                    // `update_mesh` past the end of that list.
                    if !self.world_meshes {
                        r.clear_meshes();
                    }
                    let drawn = r.draw_world_doc_with_hud(&play.doc, &hud);
                    if drawn.is_ok() {
                        self.world_meshes = true;
                    }
                    drawn
                }
                (None, _) => Err("no renderer attached".into()),
                (Some(_), None) => Err("no packaged world".into()),
            };
            self.renderer = renderer;
            return result;
        }
        let hud = self.draw_list();
        let Some(r) = self.renderer.as_mut() else {
            return Err("no renderer attached".into());
        };
        let world = match (
            self.kind,
            self.mesh_ids.as_ref(),
            self.isle_mesh_ids.as_ref(),
        ) {
            (SceneKind::Driving, Some(ids), _) => Some(self.driving.build_scene(ids, r.aspect())),
            (SceneKind::Collectathon, _, Some(ids)) => Some(self.isle.build_scene(ids, r.aspect())),
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

    fn touch(s: &mut SharedSession, phase: PointerPhase, x: f32, y: f32) {
        s.push_pointer(PointerEvent {
            id: 7,
            x,
            y,
            phase,
            pressure: 0.5,
        });
    }

    #[test]
    fn neon_cat_swipes_move_and_jump_mid_touch() {
        let mut s = SharedSession::default();
        s.create_surface(400, 800);
        s.load_game_json(include_str!("../tests/fixtures/neon_cat_world.json"))
            .unwrap();
        s.request_frame();
        assert!(s.uses_swipe());
        // Tap on the title starts the run.
        touch(&mut s, PointerPhase::Begin, 200.0, 400.0);
        touch(&mut s, PointerPhase::End, 200.0, 400.0);
        s.request_frame();
        assert_eq!(s.cat().unwrap().phase_name(), "play");
        // Swipe left and keep the finger down; then swipe up in the same touch.
        touch(&mut s, PointerPhase::Begin, 200.0, 400.0);
        touch(&mut s, PointerPhase::Move, 150.0, 400.0);
        s.set_play_input(0.0, 0.0, false, false, false);
        for _ in 0..30 {
            s.request_frame();
        }
        let x = s.cat().unwrap().x();
        assert!(x > 0.5, "screen-left swipe moves world +x: {x}");
        touch(&mut s, PointerPhase::Move, 150.0, 340.0);
        s.request_frame();
        assert!(s.cat().unwrap().y() > 0.0);
        touch(&mut s, PointerPhase::End, 150.0, 340.0);
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

    #[test]
    fn collectathon_pad_walks_and_stats_name_the_isle() {
        let mut s = SharedSession::default();
        s.boot_collectathon();
        assert_eq!(s.kind, SceneKind::Collectathon);
        s.start_game();
        s.set_pad(0.0, -1.0);
        let z0 = s.isle.walker.z;
        for _ in 0..60 {
            s.request_frame();
        }
        assert!(s.isle.walker.z > z0 + 2.0);
        let j = s.stats_json();
        assert!(j.contains("crest_isle"), "{j}");
        assert!(j.contains("stars"), "{j}");
        assert_eq!(s.request_frame().star_need, crate::collectathon::STAR_NEED);
    }

    #[test]
    fn collectathon_title_blocks_until_start() {
        let mut s = SharedSession::default();
        s.boot_collectathon();
        s.set_walk(0.0, 1.0, false);
        let z0 = s.isle.walker.z;
        for _ in 0..30 {
            s.request_frame();
        }
        assert_eq!(s.isle.walker.z, z0);
        assert_eq!(s.isle.game.phase, crate::game::GamePhase::Title);
    }

    #[test]
    fn packaged_world_stays_on_title_until_confirm() {
        let json = include_str!("../tests/fixtures/platformer_world.json");
        let mut s = SharedSession::default();
        assert!(s.load_game_json("{").is_err());
        assert_eq!(s.kind, SceneKind::Driving);
        s.load_game_json(json).unwrap();
        assert_eq!(s.kind, SceneKind::World);
        assert_eq!(s.stats().game_phase, crate::game::GamePhase::Title);
        assert_eq!(s.stats().objective, "box_hop");
        let start = s
            .world
            .as_ref()
            .unwrap()
            .doc
            .player
            .as_ref()
            .unwrap()
            .position;
        s.set_play_input(1.0, 0.0, true, false, false);
        for _ in 0..20 {
            s.request_frame();
        }
        let held = s
            .world
            .as_ref()
            .unwrap()
            .doc
            .player
            .as_ref()
            .unwrap()
            .position;
        assert_eq!(held, start);
        s.push_pointer(PointerEvent {
            id: 1,
            x: 8.0,
            y: 8.0,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        });
        assert_eq!(s.stats().game_phase, crate::game::GamePhase::Playing);
        for _ in 0..30 {
            s.set_play_input(1.0, 0.0, false, false, false);
            s.request_frame();
        }
        let moved = s
            .world
            .as_ref()
            .unwrap()
            .doc
            .player
            .as_ref()
            .unwrap()
            .position;
        assert!(s.stats().game_time_s > 0.4);
        assert_ne!(moved, start);
    }

    #[test]
    fn packaged_title_and_pause_are_on_the_hud() {
        let json = include_str!("../tests/fixtures/platformer_world.json");
        let mut s = SharedSession::default();
        s.load_game_json(json).unwrap();
        let title = s.draw_list();
        assert!(title.texts.iter().any(|t| t.text == "BOX HOP"));
        assert!(title.texts.iter().any(|t| t.text == "はじめる"));
        s.push_pointer(PointerEvent {
            id: 2,
            x: 8.0,
            y: 8.0,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        });
        assert_eq!(s.stats().game_phase, crate::game::GamePhase::Playing);
        let start = s
            .world
            .as_ref()
            .unwrap()
            .doc
            .player
            .as_ref()
            .unwrap()
            .position;
        assert!(s.world.as_mut().unwrap().request_pause());
        assert_eq!(s.stats().ui, crate::ui::UiMode::Pause);
        let pause = s.draw_list();
        assert!(pause.texts.iter().any(|t| t.text == "ポーズ"));
        for _ in 0..20 {
            s.set_play_input(1.0, 0.0, true, false, false);
            s.request_frame();
        }
        let held = s
            .world
            .as_ref()
            .unwrap()
            .doc
            .player
            .as_ref()
            .unwrap()
            .position;
        assert_eq!(held, start);
    }

    #[test]
    fn playtest_f12_and_three_fingers_freeze_the_frame() {
        let mut s = demo2d();
        s.create_surface(800, 600);
        s.set_playtest_enabled(true);
        s.set_pad(1.0, 0.0);
        s.request_frame();
        let frame = s.frame;
        assert!(s.push_key("F12", true));
        assert!(s.playtest_open());
        s.request_frame();
        assert_eq!(s.frame, frame);
        assert!(s.push_key("Escape", true) || s.playtest_open());
        s.playtest_action("close");
        assert!(!s.playtest_open());

        s.request_frame();
        let frame = s.frame;
        for id in 0..3u32 {
            s.push_pointer(PointerEvent {
                id,
                x: 70.0 + id as f32 * 30.0,
                y: 80.0,
                phase: PointerPhase::Begin,
                pressure: 1.0,
            });
        }
        assert!(s.playtest_blocks_input());
        for _ in 0..40 {
            s.request_frame();
        }
        assert!(s.playtest_open());
        assert_eq!(s.frame, frame);
        let state = s.playtest_state_json();
        assert!(state.contains("\"shot\":true") || state.contains("\"open\":true"));
    }

    #[test]
    fn playtest_load_checkpoints_the_world_not_the_empty_stats() {
        let mut s = SharedSession::default();
        s.set_playtest_enabled(true);
        s.load_game_json(include_str!("../tests/fixtures/platformer_world.json"))
            .unwrap();
        assert!(s.open_playtest());
        let report = s.playtest.report_value();
        assert_eq!(report["dump"]["version"], 1);
        assert_eq!(report["dump"]["genre"], "box_hop");
        assert_eq!(report["dump_t"], 0.0);
    }
}

//! Crest Isle — モバイル／Wasm 向け収集スライス。
//!
//! Python `kagra-core` の VRM ゲームとは**別レンダラ**。プレイヤーは Kenney 風の
//! カプセル（VRM ではない）。高さ関数と紋章／コイン配置は
//! `kagra.land.open_world_height` / `examples/open_world_rules.py` と同じ幻想。
//! Kenney GLB は pip ホイールに入れない。ここは手続きメッシュで同じ草原・海・山を出す。

use crate::game::GamePhase;
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, FIXED_DT};
use crate::scene3d::{primitives, Aabb, Camera, Material, MeshData, MeshId, Scene3D, SceneBuilder};
use crate::ui::PauseMenu;
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

pub const GAME_ID: &str = "crest_isle";
pub const GAME_TITLE: &str = "Crest Isle";

pub const HALF: f32 = 80.0;
pub const WATER_Y: f32 = 0.0;
pub const START_XZ: (f32, f32) = (0.0, -8.0);
pub const PEAK_XZ: (f32, f32) = (8.0, 52.0);

pub const CAM_DISTANCE: f32 = 12.2;
pub const CAM_HEIGHT: f32 = 4.4;
pub const CAM_LOOK_Y: f32 = 1.25;
pub const PLAYER_SPEED: f32 = 5.6;
pub const JUMP_V: f32 = 7.2;
pub const GRAVITY: f32 = 22.0;
pub const FOV_DEG: f32 = 54.0;
pub const PICK_REACH: f32 = 1.25;
pub const STAR_NEED: u32 = 6;
pub const BODY_H: f32 = 0.95;

/// デスクトップと同じ 8 紋章。最後が峰の旗。
pub const STAR_XZ: [(f32, f32); 8] = [
    (3.2, -0.6),
    (-7.2, 7.5),
    (10.4, 5.8),
    (1.4, 17.5),
    (-8.4, 19.2),
    (13.6, 15.8),
    (8.0, 34.0),
    PEAK_XZ,
];

#[derive(Clone, Copy, Debug, Default)]
pub struct WalkInput {
    /// カメラ基準。+x 右、+z 前（画面の上）。
    pub lx: f32,
    pub lz: f32,
    pub jump: bool,
    /// One-shot melee. Shared action genre; collectathon ignores it.
    pub attack: bool,
    /// One-shot i-frame dash. Shared action genre; collectathon ignores it.
    pub dodge: bool,
}

impl WalkInput {
    pub fn clamped(self) -> Self {
        Self {
            lx: self.lx.clamp(-1.0, 1.0),
            lz: self.lz.clamp(-1.0, 1.0),
            jump: self.jump,
            attack: self.attack,
            dodge: self.dodge,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pickup {
    pub x: f32,
    pub z: f32,
    pub live: bool,
    pub phase: f32,
    pub kind: PickupKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    Star,
    Coin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    Sea,
    Grass,
    Mountain,
}

#[derive(Clone, Debug)]
pub struct Walker {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub vy: f32,
    pub on_ground: bool,
}

impl Walker {
    pub fn spawn() -> Self {
        let (x, z) = START_XZ;
        let ground = open_world_height(x, z);
        Self {
            x,
            y: ground + BODY_H,
            z,
            yaw: 0.0,
            vy: 0.0,
            on_ground: true,
        }
    }

    pub fn pos(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct IsleGame {
    pub phase: GamePhase,
    pub time_s: f32,
    pub score: u32,
    pub stars: u32,
    pub coins: u32,
    pub best_score: Option<u32>,
}

impl Default for IsleGame {
    fn default() -> Self {
        Self {
            phase: GamePhase::Title,
            time_s: 0.0,
            score: 0,
            stars: 0,
            coins: 0,
            best_score: None,
        }
    }
}

impl IsleGame {
    pub fn start(&mut self) {
        let best = self.best_score;
        *self = Self {
            phase: GamePhase::Playing,
            best_score: best,
            ..Self::default()
        };
        self.phase = GamePhase::Playing;
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.phase, GamePhase::Playing)
    }

    pub fn finish(&mut self, stars: u32, coins: u32, time_s: f32) {
        self.phase = GamePhase::Complete;
        self.stars = stars;
        self.coins = coins;
        self.time_s = time_s;
        self.score = round_score(stars, coins, time_s);
        match self.best_score {
            Some(b) if self.score > b => self.best_score = Some(self.score),
            None => self.best_score = Some(self.score),
            _ => {}
        }
    }

    pub fn objective_key(&self) -> &'static str {
        match self.phase {
            GamePhase::Title => "title",
            GamePhase::Complete => "complete",
            GamePhase::Playing => {
                if self.stars >= STAR_NEED {
                    "complete"
                } else {
                    "crests"
                }
            }
        }
    }
}

/// 共有コアが使うメッシュ一式。GPU には載せない。
pub struct MeshSet {
    pub terrain: MeshData,
    pub water: MeshData,
    pub sky: MeshData,
    pub shadow: MeshData,
    pub boxy: MeshData,
    pub cone: MeshData,
    pub cylinder: MeshData,
}

#[derive(Clone, Copy, Debug)]
pub struct MeshIds {
    pub terrain: MeshId,
    pub water: MeshId,
    pub sky: MeshId,
    pub shadow: MeshId,
    pub boxy: MeshId,
    pub cone: MeshId,
    pub cylinder: MeshId,
}

impl MeshSet {
    pub fn build() -> Self {
        Self {
            terrain: heightfield_mesh(72.0, 36),
            water: primitives::plane_mesh(400.0, 400.0),
            sky: primitives::sky_dome(220.0, 20),
            shadow: primitives::plane_mesh(1.4, 1.4),
            boxy: primitives::box_mesh(Vec3::ONE),
            cone: primitives::cone_mesh(0.5, 1.0, 8),
            cylinder: primitives::cylinder_mesh(0.5, 1.0, 8),
        }
    }

    pub fn as_slice(&self) -> [&MeshData; 7] {
        [
            &self.terrain,
            &self.water,
            &self.sky,
            &self.shadow,
            &self.boxy,
            &self.cone,
            &self.cylinder,
        ]
    }
}

#[derive(Clone, Debug)]
pub struct CollectathonScene {
    pub walker: Walker,
    pub input: WalkInput,
    pub stars: Vec<Pickup>,
    pub coins: Vec<Pickup>,
    pub game: IsleGame,
    elapsed: f32,
    cam_yaw: f32,
}

impl Default for CollectathonScene {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectathonScene {
    pub fn new() -> Self {
        Self {
            walker: Walker::spawn(),
            input: WalkInput::default(),
            stars: spawn_stars(),
            coins: spawn_coins(),
            game: IsleGame::default(),
            elapsed: 0.0,
            cam_yaw: 0.0,
        }
    }

    pub fn restart(&mut self) {
        let best = self.game.best_score;
        *self = Self::new();
        self.game.best_score = best;
    }

    pub fn start(&mut self) {
        let best = self.game.best_score;
        *self = Self::new();
        self.game.best_score = best;
        self.game.start();
    }

    pub fn show_title(&mut self) {
        let best = self.game.best_score;
        *self = Self::new();
        self.game.best_score = best;
        self.game.phase = GamePhase::Title;
    }

    pub fn set_input(&mut self, input: WalkInput) {
        self.input = input.clamped();
    }

    /// 画面左下の仮想スティック＋右下ジャンプ。シェルが `set_walk` を
    /// 渡さなくても、ポインタだけで歩ける。
    pub fn apply_pointers(&mut self, width: u32, height: u32, pointers: &[PointerEvent]) {
        let layout = TouchLayout::new(width, height);
        let mut stick = None;
        let mut jump = false;
        let mut released_stick = false;
        for p in pointers {
            let in_stick = layout.stick_well.contains(p.x, p.y) || p.x < layout.mid_x;
            if matches!(p.phase, PointerPhase::End | PointerPhase::Cancel) {
                if in_stick {
                    released_stick = true;
                }
                continue;
            }
            if layout.jump.contains(p.x, p.y) {
                jump = true;
            } else if in_stick {
                let cx = layout.stick_well.x + layout.stick_well.w * 0.5;
                let cy = layout.stick_well.y + layout.stick_well.h * 0.5;
                let r = layout.stick_well.w * 0.5;
                let dx = ((p.x - cx) / r).clamp(-1.0, 1.0);
                let dy = ((p.y - cy) / r).clamp(-1.0, 1.0);
                stick = Some((dx, -dy));
            }
        }
        if let Some((lx, lz)) = stick {
            self.input.lx = lx;
            self.input.lz = lz;
        } else if released_stick {
            self.input.lx = 0.0;
            self.input.lz = 0.0;
        }
        self.input.jump = jump || self.input.jump;
    }

    pub fn update(&mut self) {
        if !self.game.is_playing() {
            self.elapsed += FIXED_DT;
            return;
        }
        self.elapsed += FIXED_DT;
        self.game.time_s += FIXED_DT;
        step_walker(&mut self.walker, self.input, self.cam_yaw, FIXED_DT);
        self.cam_yaw += angle_delta(self.cam_yaw, self.walker.yaw) * (1.0 - 0.86);
        collect_pickups(&mut self.stars, &mut self.coins, &self.walker);
        self.game.stars = self.stars.iter().filter(|p| !p.live).count() as u32;
        self.game.coins = self.coins.iter().filter(|p| !p.live).count() as u32;
        if won(self.game.stars) {
            self.game
                .finish(self.game.stars, self.game.coins, self.game.time_s);
        }
        self.input.jump = false;
    }

    pub fn camera3d(&self) -> Camera {
        let look = Vec3::new(self.walker.x, self.walker.y + CAM_LOOK_Y, self.walker.z);
        let (s, c) = self.cam_yaw.sin_cos();
        // yaw=0 は +Z。カメラは後ろ（-Z）から見る。
        let eye = look
            + Vec3::new(
                -s * CAM_DISTANCE,
                CAM_HEIGHT - CAM_LOOK_Y,
                -c * CAM_DISTANCE,
            );
        Camera {
            eye,
            target: look,
            up: Vec3::Y,
            fov_y: FOV_DEG.to_radians(),
            near: 0.2,
            far: 220.0,
        }
    }

    pub fn build_scene(&self, ids: &MeshIds, aspect: f32) -> Scene3D {
        let camera = self.camera3d();
        let mut b = SceneBuilder::new(&camera, aspect);
        b.register(ids.boxy, Aabb::from_center_size(Vec3::ZERO, Vec3::ONE));
        b.register(
            ids.cone,
            Aabb {
                min: Vec3::new(-0.5, 0.0, -0.5),
                max: Vec3::new(0.5, 1.0, 0.5),
            },
        );
        b.register(
            ids.cylinder,
            Aabb {
                min: Vec3::new(-0.5, 0.0, -0.5),
                max: Vec3::new(0.5, 1.0, 0.5),
            },
        );
        b.register(ids.shadow, plane_bounds(1.4, 1.4));
        b.register(ids.terrain, plane_bounds(144.0, 144.0));
        b.register(ids.water, plane_bounds(400.0, 400.0));

        let sky = [150, 188, 228, 255];
        b.push_material(
            ids.sky,
            Mat4::from_translation(camera.eye),
            sky,
            Material::Sky,
        );
        b.push_material(
            ids.water,
            Mat4::from_translation(Vec3::new(0.0, WATER_Y - 0.04, 20.0)),
            [38, 92, 118, 255],
            Material::Solid,
        );
        b.push_material(
            ids.terrain,
            Mat4::IDENTITY,
            [78, 138, 64, 255],
            Material::Grass,
        );

        emit_vista(&mut b, ids);
        emit_pickups(&mut b, ids, &self.stars, &self.coins, self.elapsed);
        emit_player(&mut b, ids, &self.walker);

        Scene3D {
            camera,
            clear: sky,
            light_dir: Vec3::new(-0.42, 0.86, 0.28).normalize(),
            ambient: 0.46,
            local_lights: [crate::scene3d::LocalLight::OFF; 4],
            fog_color: sky,
            fog_start: 48.0,
            fog_end: 150.0,
            ibl: 0.35,
            exposure: 1.0,
            tonemap: true,
            batches: b.finish(),
        }
    }

    pub fn build_hud(&self, width: u32, height: u32, paused: bool) -> DrawList {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let mut quads = Vec::with_capacity(48);
        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let pad = 16.0 * scale;

        // 紋章スロット
        let pip = 22.0 * scale;
        let gap = 6.0 * scale;
        for i in 0..STAR_XZ.len() {
            let x = pad + i as f32 * (pip + gap);
            let got = (i as u32) < self.game.stars;
            quads.push(Quad::new(
                x,
                pad,
                pip,
                pip,
                if got {
                    [240, 196, 72, 255]
                } else {
                    [20, 24, 18, 150]
                },
            ));
        }

        // コイン（小さな金の帯）
        let coin_w = 8.0 * scale;
        for i in 0..self.game.coins.min(24) {
            quads.push(Quad::new(
                pad + i as f32 * (coin_w + 3.0 * scale),
                pad + pip + 8.0 * scale,
                coin_w,
                coin_w,
                [255, 210, 70, 255],
            ));
        }

        let layout = TouchLayout::new(width, height);
        // スティック井戸
        quads.push(layout.stick_well);
        let knob_r = layout.stick_well.w * 0.18;
        let kx = layout.stick_well.x + layout.stick_well.w * (0.5 + 0.32 * self.input.lx);
        let ky = layout.stick_well.y + layout.stick_well.h * (0.5 - 0.32 * self.input.lz);
        quads.push(Quad::new(
            kx - knob_r,
            ky - knob_r,
            knob_r * 2.0,
            knob_r * 2.0,
            [230, 220, 190, 210],
        ));
        // ジャンプ
        let jump_on = self.input.jump || !self.walker.on_ground;
        let mut jump = layout.jump;
        if jump_on {
            jump.color = [240, 200, 90, 230];
        }
        quads.push(jump);
        // ジャンプ印（上向きの短い棒）
        quads.push(Quad::new(
            jump.x + jump.w * 0.42,
            jump.y + jump.h * 0.28,
            jump.w * 0.16,
            jump.h * 0.44,
            [30, 24, 14, 220],
        ));

        if paused {
            quads.extend(PauseMenu::layout(width, height).quads());
        }

        // タイトル／結果はシェルの文字 HUD に任せる。ここは薄い帯だけ。
        if matches!(self.game.phase, GamePhase::Title | GamePhase::Complete) {
            quads.push(Quad::new(0.0, h * 0.28, w, h * 0.22, [12, 16, 12, 140]));
        }

        DrawList {
            clear: [150, 188, 228, 255],
            quads,
        }
    }
}

struct TouchLayout {
    stick_well: Quad,
    jump: Quad,
    mid_x: f32,
}

impl TouchLayout {
    fn new(width: u32, height: u32) -> Self {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
        let well = 168.0 * scale;
        let pad = 18.0 * scale;
        let stick = Quad::new(pad, h - pad - well, well, well, [18, 22, 16, 90]);
        let btn = 92.0 * scale;
        let jump = Quad::new(w - pad - btn, h - pad - btn, btn, btn, [18, 22, 16, 120]);
        Self {
            stick_well: stick,
            jump,
            mid_x: w * 0.45,
        }
    }
}

fn plane_bounds(w: f32, d: f32) -> Aabb {
    Aabb::from_center_size(Vec3::ZERO, Vec3::new(w, 0.02, d))
}

fn angle_delta(from: f32, to: f32) -> f32 {
    let mut d = to - from;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[allow(clippy::too_many_arguments)]
fn stair_y(
    x: f32,
    z: f32,
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    y0: f32,
    y1: f32,
    steps: i32,
) -> Option<f32> {
    if !(x0 <= x && x <= x1 && z0 <= z && z <= z1) {
        return None;
    }
    let n = steps.max(1) as f32;
    let span = z1 - z0;
    let t = if span <= 1e-9 {
        0.0
    } else {
        ((z - z0) / span).clamp(0.0, 0.999999)
    };
    let step = (t * n).floor();
    Some(y0 + (y1 - y0) * (step + 1.0) / n)
}

pub fn open_world_base(x: f32, z: f32) -> f32 {
    let mut meadow = 1.22 + 0.30 * (x * 0.15).sin() * (z * 0.13).cos();
    meadow += 0.18 * (x * 0.33 + 1.1).sin() * (z * 0.27).sin();
    let west = -9.5 * smoothstep((-x - 15.0) / 13.0);
    let sw = -4.8 * (-((x + 24.0).powi(2) + (z + 6.0).powi(2)) / 160.0).exp();
    let south = -7.2 * smoothstep((-z - 36.0) / 14.0);
    let h1 = 0.85 * (-((x - 18.0).powi(2) + (z - 18.0).powi(2)) / 95.0).exp();
    let h2 = 0.70 * (-((x + 5.0).powi(2) + (z - 22.0).powi(2)) / 80.0).exp();
    let peak = 11.2 * (-((x - 8.0).powi(2) + (z - 52.0).powi(2)) / 190.0).exp();
    let ridge = 9.4 * (-((x - 24.0).powi(2) + (z - 48.0).powi(2)) / 220.0).exp();
    let far = 13.5 * (-((x - 1.0).powi(2) + (z - 66.0).powi(2)) / 200.0).exp();
    let east = 0.90 * (-((x - 30.0).powi(2) + (z - 12.0).powi(2)) / 110.0).exp();
    let y = meadow + west + sw + south + h1 + h2 + peak + ridge + far + east;
    let w = (-(x * x + (z + 7.0).powi(2)) / 58.0).exp();
    y * (1.0 - 0.58 * w) + 1.18 * w
}

/// `kagra.land.open_world_height` と同じ半島。
pub fn open_world_height(x: f32, z: f32) -> f32 {
    let mut y = open_world_base(x, z);
    if let Some(stair) = stair_y(x, z, 5.0, 11.0, 26.0, 53.5, 1.85, 12.8, 16) {
        y = y.max(stair);
    }
    y
}

pub fn biome_at(x: f32, z: f32) -> Biome {
    let h = open_world_height(x, z);
    if h < WATER_Y - 0.04 {
        Biome::Sea
    } else if h > 2.2 {
        Biome::Mountain
    } else {
        Biome::Grass
    }
}

pub fn coin_path() -> Vec<(f32, f32)> {
    let mut pts = Vec::new();
    for (i, z) in (-5..30).step_by(3).enumerate() {
        pts.push((1.1 + (i as i32 % 3 - 1) as f32 * 1.35, z as f32));
    }
    for (i, z) in (1..20).step_by(3).enumerate() {
        pts.push((-8.6 + (i as i32 % 2) as f32 * 0.7, z as f32));
    }
    for (i, z) in (8..22).step_by(4).enumerate() {
        pts.push((7.4 + (i as i32 % 2) as f32 * 0.9, z as f32));
    }
    pts.push((8.0, 28.0));
    pts.push((8.0, 40.0));
    pts.push((-4.0, 12.0));
    pts.push((5.0, -3.5));
    pts.into_iter()
        .filter(|(x, z)| {
            let ds = (*x - START_XZ.0).hypot(*z - START_XZ.1);
            if ds < 1.6 {
                return false;
            }
            !STAR_XZ
                .iter()
                .any(|(sx, sz)| (*x - sx).hypot(*z - sz) < 1.4)
        })
        .collect()
}

pub fn spawn_stars() -> Vec<Pickup> {
    STAR_XZ
        .iter()
        .enumerate()
        .map(|(i, (x, z))| Pickup {
            x: *x,
            z: *z,
            live: true,
            phase: i as f32 * 0.6,
            kind: PickupKind::Star,
        })
        .collect()
}

pub fn spawn_coins() -> Vec<Pickup> {
    coin_path()
        .into_iter()
        .enumerate()
        .map(|(i, (x, z))| Pickup {
            x,
            z,
            live: true,
            phase: i as f32 * 0.35,
            kind: PickupKind::Coin,
        })
        .collect()
}

pub fn nearest_live(px: f32, pz: f32, items: &[Pickup]) -> Option<&Pickup> {
    items.iter().filter(|it| it.live).min_by(|a, b| {
        let da = (px - a.x).hypot(pz - a.z);
        let db = (px - b.x).hypot(pz - b.z);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    })
}

pub fn round_score(stars: u32, coins: u32, time_s: f32) -> u32 {
    let stars = stars.min(STAR_XZ.len() as u32);
    let coins = coins.min(coin_path().len() as u32);
    let base = stars * 250 + coins * 10;
    if stars == 0 {
        return coins * 10;
    }
    let mut bonus = 0;
    if stars >= STAR_NEED {
        let leftover = (180.0 - time_s).max(0.0);
        bonus = (leftover * 2.5) as u32;
        if stars >= STAR_XZ.len() as u32 {
            bonus += 400;
        }
    }
    base + bonus
}

pub fn grade_for(score: u32) -> &'static str {
    if score >= 2200 {
        "S"
    } else if score >= 1600 {
        "A"
    } else if score >= 1100 {
        "B"
    } else if score >= 600 {
        "C"
    } else {
        "D"
    }
}

pub fn won(stars: u32) -> bool {
    stars >= STAR_NEED
}

fn step_walker(w: &mut Walker, input: WalkInput, cam_yaw: f32, dt: f32) {
    let (s, c) = cam_yaw.sin_cos();
    // カメラ前 = +yaw の +Z。スティック lz が前。
    let fwd = Vec3::new(s, 0.0, c);
    let right = Vec3::new(c, 0.0, -s);
    let wish = right * input.lx + fwd * input.lz;
    let wish_len = wish.length();
    if wish_len > 0.08 {
        let dir = wish / wish_len;
        let speed = PLAYER_SPEED * wish_len.min(1.0);
        w.x += dir.x * speed * dt;
        w.z += dir.z * speed * dt;
        w.yaw = dir.x.atan2(dir.z);
    }
    w.x = w.x.clamp(-HALF + 2.0, HALF - 2.0);
    w.z = w.z.clamp(-HALF + 2.0, HALF - 2.0);

    if input.jump && w.on_ground {
        w.vy = JUMP_V;
        w.on_ground = false;
    }
    w.vy -= GRAVITY * dt;
    w.y += w.vy * dt;
    let ground = open_world_height(w.x, w.z) + BODY_H;
    if w.y <= ground {
        w.y = ground;
        w.vy = 0.0;
        w.on_ground = true;
    } else {
        w.on_ground = false;
    }
}

fn collect_pickups(stars: &mut [Pickup], coins: &mut [Pickup], w: &Walker) {
    for it in stars.iter_mut().chain(coins.iter_mut()) {
        if !it.live {
            continue;
        }
        if (w.x - it.x).hypot(w.z - it.z) <= PICK_REACH {
            it.live = false;
        }
    }
}

fn heightfield_mesh(half: f32, cells: u32) -> MeshData {
    let cells = cells.max(8);
    let step = (half * 2.0) / cells as f32;
    let mut mesh = MeshData::default();
    for iz in 0..=cells {
        for ix in 0..=cells {
            let x = -half + ix as f32 * step;
            let z = -half + iz as f32 * step;
            let y = open_world_height(x, z);
            let dx =
                (open_world_height(x + step, z) - open_world_height(x - step, z)) / (2.0 * step);
            let dz =
                (open_world_height(x, z + step) - open_world_height(x, z - step)) / (2.0 * step);
            let n = Vec3::new(-dx, 1.0, -dz).normalize_or(Vec3::Y);
            mesh.vertices
                .push(crate::scene3d::Vertex3::new(Vec3::new(x, y, z), n));
        }
    }
    let stride = cells + 1;
    for iz in 0..cells {
        for ix in 0..cells {
            let i = iz * stride + ix;
            mesh.indices.extend_from_slice(&[
                i,
                i + stride,
                i + 1,
                i + 1,
                i + stride,
                i + stride + 1,
            ]);
        }
    }
    mesh
}

fn sit(x: f32, z: f32, extra: f32) -> Vec3 {
    Vec3::new(x, open_world_height(x, z) + extra, z)
}

fn emit_player(b: &mut SceneBuilder, ids: &MeshIds, w: &Walker) {
    let yaw = Quat::from_rotation_y(w.yaw);
    let feet = Vec3::new(w.x, w.y - BODY_H, w.z);
    // 胴（シアンのカプセル胴）。VRM ではない Kenney 風スタンドイン。
    let body = Mat4::from_scale_rotation_translation(
        Vec3::new(0.55, 0.85, 0.55),
        yaw,
        feet + Vec3::Y * 0.08,
    );
    b.push(ids.cylinder, body, [62, 168, 176, 255]);
    let head = Mat4::from_scale_rotation_translation(
        Vec3::new(0.42, 0.38, 0.42),
        yaw,
        feet + Vec3::Y * 1.02,
    );
    b.push(ids.boxy, head, [236, 214, 176, 255]);
    // 小さな頭巾
    let hood = Mat4::from_scale_rotation_translation(
        Vec3::new(0.48, 0.16, 0.48),
        yaw,
        feet + Vec3::Y * 1.22,
    );
    b.push(ids.boxy, hood, [48, 122, 96, 255]);
    b.push_material(
        ids.shadow,
        Mat4::from_translation(Vec3::new(w.x, open_world_height(w.x, w.z) + 0.03, w.z)),
        [20, 24, 16, 90],
        Material::Solid,
    );
}

fn emit_pickups(b: &mut SceneBuilder, ids: &MeshIds, stars: &[Pickup], coins: &[Pickup], t: f32) {
    for (i, p) in stars.iter().enumerate() {
        if !p.live {
            continue;
        }
        let bob = (t * 2.2 + p.phase).sin() * 0.12;
        let ground = open_world_height(p.x, p.z);
        let pole_h = if i + 1 == STAR_XZ.len() { 2.6 } else { 1.7 };
        let pole = Mat4::from_scale_rotation_translation(
            Vec3::new(0.10, pole_h, 0.10),
            Quat::IDENTITY,
            Vec3::new(p.x, ground, p.z),
        );
        b.push(ids.cylinder, pole, [210, 190, 140, 255]);
        let flag = Mat4::from_scale_rotation_translation(
            Vec3::new(0.85, 0.45, 0.08),
            Quat::from_rotation_y(t * 0.8 + p.phase),
            Vec3::new(p.x + 0.35, ground + pole_h * 0.78 + bob, p.z),
        );
        let color = if i + 1 == STAR_XZ.len() {
            [255, 214, 70, 255]
        } else if i % 2 == 0 {
            [220, 70, 70, 255]
        } else {
            [70, 170, 90, 255]
        };
        b.push(ids.boxy, flag, color);
    }
    for p in coins {
        if !p.live {
            continue;
        }
        let bob = (t * 3.4 + p.phase).sin() * 0.10;
        let spin = Quat::from_rotation_y(t * 2.6 + p.phase);
        let ground = open_world_height(p.x, p.z);
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(0.42, 0.08, 0.42),
            spin,
            Vec3::new(p.x, ground + 0.55 + bob, p.z),
        );
        b.push(ids.cylinder, m, [255, 208, 64, 255]);
    }
}

fn tree(b: &mut SceneBuilder, ids: &MeshIds, x: f32, z: f32, scale: f32, pine: bool, yaw: f32) {
    let q = Quat::from_rotation_y(yaw);
    let g = open_world_height(x, z);
    if g < WATER_Y + 0.05 {
        return;
    }
    let trunk_h = scale * if pine { 1.15 } else { 0.85 };
    let trunk = Mat4::from_scale_rotation_translation(
        Vec3::new(0.22 * scale, trunk_h, 0.22 * scale),
        q,
        Vec3::new(x, g, z),
    );
    b.push(ids.cylinder, trunk, [92, 62, 38, 255]);
    let foliage_h = scale * if pine { 2.6 } else { 1.9 };
    let foliage_r = scale * if pine { 0.95 } else { 1.35 };
    let fol = Mat4::from_scale_rotation_translation(
        Vec3::new(foliage_r, foliage_h, foliage_r),
        q,
        Vec3::new(x, g + trunk_h * 0.55, z),
    );
    b.push(
        ids.cone,
        fol,
        if pine {
            [46, 102, 58, 255]
        } else {
            [62, 132, 52, 255]
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn rock(b: &mut SceneBuilder, ids: &MeshIds, x: f32, z: f32, sx: f32, sy: f32, sz: f32, yaw: f32) {
    let g = open_world_height(x, z);
    let m = Mat4::from_scale_rotation_translation(
        Vec3::new(sx, sy, sz),
        Quat::from_rotation_y(yaw),
        sit(x, z, sy * 0.45),
    );
    let _ = g;
    b.push(ids.boxy, m, [128, 118, 108, 255]);
}

fn emit_vista(b: &mut SceneBuilder, ids: &MeshIds) {
    // 開口ショット（+Z、海は -X）に Kenney 風の木・岩・崖。
    let trees = [
        (-3.4, 1.2, 2.15, true, 0.40),
        (4.1, 2.0, 1.85, false, 1.10),
        (2.6, 6.4, 2.25, true, -0.35),
        (-5.2, 4.8, 1.95, false, 0.80),
        (6.8, 3.1, 2.05, true, 0.20),
        (-2.0, 8.6, 1.75, false, -0.90),
        (8.8, 9.4, 1.55, false, 0.50),
        (-6.6, 11.2, 1.45, false, 1.30),
        (3.8, 12.5, 2.10, true, -0.20),
        (11.2, 2.4, 1.80, false, 2.00),
        (-1.2, 14.8, 2.00, true, 0.70),
        (5.4, 16.2, 1.70, false, -0.55),
        (9.6, 13.0, 1.90, true, 0.15),
        (12.4, 18.5, 3.20, true, 0.40),
        (15.0, 21.0, 3.50, true, -0.30),
        (18.2, 24.6, 3.80, true, 0.80),
        (6.2, 22.4, 3.40, true, 1.10),
        (-4.8, 9.8, 2.60, false, 0.25),
        (-11.2, 3.4, 2.40, false, 0.60),
        (-10.4, 8.2, 2.20, false, -0.40),
        (-9.6, 13.6, 2.50, false, 1.20),
        (1.6, 10.2, 2.00, false, 0.10),
        (-7.8, 2.2, 1.70, false, 0.90),
        (7.2, 7.8, 1.60, true, -1.40),
        (-3.8, 17.4, 1.95, true, 0.45),
        (8.0, 40.0, 2.40, true, 0.20),
        (10.8, 46.0, 2.80, true, -0.40),
        (4.2, 48.5, 2.50, true, 0.70),
    ];
    for (x, z, s, pine, yaw) in trees {
        tree(b, ids, x, z, s, pine, yaw);
    }

    let rocks = [
        (1.4, 0.8, 1.4, 0.9, 1.2, 0.20),
        (-1.6, 3.2, 1.6, 0.7, 1.3, 0.90),
        (5.0, 1.1, 1.3, 0.6, 1.1, -0.40),
        (-9.2, 5.4, 1.5, 1.0, 1.4, 0.55),
        (12.0, 7.2, 1.8, 0.8, 1.3, -0.80),
        (2.2, 4.6, 0.9, 1.2, 0.8, 0.30),
        (9.0, 19.4, 1.6, 1.1, 1.5, 0.15),
        (4.6, 9.0, 1.3, 0.7, 1.4, 0.70),
        (-6.0, 15.4, 1.1, 1.3, 0.9, -0.25),
    ];
    for (x, z, sx, sy, sz, yaw) in rocks {
        rock(b, ids, x, z, sx, sy, sz, yaw);
    }

    // 西の海崖
    let cliffs = [
        (-14.2, 2.0, 4.2, 6.4, 3.4, 0.15),
        (-13.6, 6.5, 3.6, 5.6, 3.0, 0.40),
        (-14.8, 11.0, 4.6, 7.0, 3.6, -0.20),
        (-13.2, 15.4, 3.8, 5.8, 3.2, 0.55),
        (-14.0, 19.8, 4.0, 6.2, 3.4, 0.10),
        (-15.4, 24.0, 5.0, 7.6, 3.8, -0.35),
        (20.5, 26.0, 4.0, 6.0, 3.2, 1.10),
    ];
    for (x, z, sx, sy, sz, yaw) in cliffs {
        let g = open_world_height(x, z);
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(sx, sy, sz),
            Quat::from_rotation_y(yaw),
            Vec3::new(x, g + sy * 0.25, z),
        );
        b.push(ids.boxy, m, [118, 108, 98, 255]);
    }

    // 花の点在（開口コーン）
    let flowers = [
        [210u8, 70, 70, 255],
        [230, 200, 60, 255],
        [150, 90, 200, 255],
        [80, 160, 70, 255],
    ];
    let mut n = 0u32;
    let mut z = -6.0;
    while z < 22.0 {
        let mut x = -11.0;
        while x < 14.0 {
            if (x - START_XZ.0).hypot(z - START_XZ.1) >= 1.8 && x > -10.0 {
                let jx = ((n.wrapping_mul(37) + 11) % 100) as f32 / 100.0 * 0.7 - 0.35;
                let jz = ((n.wrapping_mul(17) + 4) % 100) as f32 / 100.0 * 0.55 - 0.27;
                let col = flowers[(n as usize) % flowers.len()];
                let h = 0.22 + (n % 5) as f32 * 0.04;
                let m = Mat4::from_scale_rotation_translation(
                    Vec3::new(0.16, h, 0.16),
                    Quat::from_rotation_y(n as f32 * 0.47),
                    sit(x + jx, z + jz, h * 0.5),
                );
                b.push(ids.boxy, m, col);
                n += 1;
            }
            x += 2.6;
        }
        z += 2.4;
    }

    // 峰の段の目印
    rock(b, ids, 8.0, 30.0, 1.2, 0.5, 1.6, 0.1);
    rock(b, ids, 7.2, 36.0, 1.4, 0.6, 1.3, -0.2);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_is_grass_west_is_sea_peak_is_mountain() {
        assert_eq!(biome_at(START_XZ.0, START_XZ.1), Biome::Grass);
        assert_eq!(biome_at(-22.0, 10.0), Biome::Sea);
        assert_eq!(biome_at(PEAK_XZ.0, PEAK_XZ.1), Biome::Mountain);
        assert!(
            open_world_height(PEAK_XZ.0, PEAK_XZ.1)
                > open_world_height(START_XZ.0, START_XZ.1) + 6.0
        );
    }

    #[test]
    fn layout_matches_desktop_rules() {
        assert_eq!(STAR_XZ.len(), 8);
        assert_eq!(STAR_NEED, 6);
        assert!(STAR_XZ.contains(&PEAK_XZ));
        assert!(!STAR_XZ.contains(&START_XZ));
        assert!(coin_path().len() >= 20);
    }

    #[test]
    fn walk_forward_moves_plus_z() {
        let mut w = Walker::spawn();
        let start_z = w.z;
        let input = WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        };
        for _ in 0..60 {
            step_walker(&mut w, input, 0.0, FIXED_DT);
        }
        assert!(w.z > start_z + 3.0, "z {} from {}", w.z, start_z);
        assert!(w.on_ground);
    }

    #[test]
    fn jump_leaves_ground_and_lands() {
        let mut w = Walker::spawn();
        let mut input = WalkInput {
            lx: 0.0,
            lz: 0.0,
            jump: true,
            attack: false,
            dodge: false,
        };
        step_walker(&mut w, input, 0.0, FIXED_DT);
        input.jump = false;
        assert!(!w.on_ground);
        assert!(w.vy > 0.0);
        for _ in 0..90 {
            step_walker(&mut w, input, 0.0, FIXED_DT);
        }
        assert!(w.on_ground);
    }

    #[test]
    fn collecting_six_stars_wins() {
        let mut s = CollectathonScene::new();
        s.start();
        assert!(!won(s.game.stars));
        for star in &mut s.stars {
            star.live = false;
            // only first 6
        }
        s.stars[6].live = true;
        s.stars[7].live = true;
        s.game.stars = 6;
        assert!(won(s.game.stars));
        s.game.finish(6, 4, 40.0);
        assert_eq!(s.game.phase, GamePhase::Complete);
        assert!(s.game.score >= 1500);
        assert_eq!(grade_for(s.game.score), "A");
    }

    #[test]
    fn pick_reach_collects_nearest_star() {
        let mut s = CollectathonScene::new();
        s.start();
        let first = s.stars[0];
        s.walker.x = first.x;
        s.walker.z = first.z;
        s.walker.y = open_world_height(first.x, first.z) + BODY_H;
        s.update();
        assert!(!s.stars[0].live);
        assert_eq!(s.game.stars, 1);
    }

    #[test]
    fn title_does_not_move() {
        let mut s = CollectathonScene::new();
        s.set_input(WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        });
        let z = s.walker.z;
        for _ in 0..30 {
            s.update();
        }
        assert_eq!(s.walker.z, z);
        assert_eq!(s.game.phase, GamePhase::Title);
    }

    #[test]
    fn hud_has_stick_and_jump() {
        let s = CollectathonScene::new();
        let hud = s.build_hud(1280, 720, false);
        assert!(hud.quads.len() >= 10);
    }

    #[test]
    fn pointers_drive_stick_and_jump() {
        let mut s = CollectathonScene::new();
        s.start();
        s.apply_pointers(
            800,
            600,
            &[PointerEvent {
                id: 1,
                x: 90.0,
                y: 460.0,
                phase: PointerPhase::Move,
                pressure: 1.0,
            }],
        );
        assert!(s.input.lz.abs() > 0.1 || s.input.lx.abs() > 0.1);
        s.apply_pointers(
            800,
            600,
            &[PointerEvent {
                id: 2,
                x: 740.0,
                y: 540.0,
                phase: PointerPhase::Begin,
                pressure: 1.0,
            }],
        );
        assert!(s.input.jump);
    }

    #[test]
    fn score_formula_matches_desktop() {
        assert_eq!(round_score(0, 3, 10.0), 30);
        let s = round_score(6, 10, 0.0);
        assert_eq!(s, 6 * 250 + 100 + 450);
        assert_eq!(grade_for(0), "D");
        assert_eq!(grade_for(2200), "S");
    }
}

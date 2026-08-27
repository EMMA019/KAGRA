//! Hit on beat as an M3 slice on play_world.
//!
//! Sibling of survival. A metal marker travels a stage toward a judge
//! box; J / click (`WalkInput.attack`) inside the window scores. Miss is
//! a beat that passes with no press. `NEED` hits is dump-visible `clear`.
//! Title -> play -> result reuses `WorldPlay` / `GamePhase`. Capsules/boxes,
//! not VRM. Indoor lights stay 4 slots. Marker stays readable (contact
//! blob plus metal GGX inherited). Stage camera looks at the judge so the
//! moving marker is the picture. Does not rewrite other genre loops. No
//! Rapier, SSAO, GI, audio chart, net, or ECS. Picture slice stays.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldProp, WorldWalker};

pub const GAME_ID: &str = "rhythm_beat";
pub const BODY_H: f32 = 0.95;
pub const NEED: u32 = 4;
pub const MARKER_SPEED: f32 = 5.0;
pub const HIT_HALF: f32 = 0.55;
pub const NAME_PLAYER: &str = "player";
pub const NAME_HIT: &str = "hit";
pub const NAME_MISS: &str = "miss";
pub const NAME_CLEAR: &str = "clear";

const FLAG_CLEAR_COLOR: [u32; 3] = [70, 180, 110];
const MARKER_WAIT: [u32; 3] = [240, 196, 72];
const MARKER_WINDOW: [u32; 3] = [255, 230, 96];
const MARKER_HIT: [u32; 3] = [70, 180, 110];
const MARKER_MISS: [u32; 3] = [196, 64, 54];
const HIT_PIP: [u8; 4] = [240, 196, 72, 255];
const CLEAR_PIP: [u8; 4] = [70, 180, 110, 255];
const MISS_PIP: [u8; 4] = [196, 64, 54, 255];

/// Live rhythm around a dump. Hit/miss/clear stay here; `name` + flag
/// enable and `coins` in the dump are the query source of truth. Marker
/// motion is kinematic, not Rapier, not an audio chart.
#[derive(Clone, Debug)]
pub struct RhythmGame {
    pub hits: u32,
    pub misses: u32,
    pub in_window: bool,
    pub clear: bool,
    pub done: bool,
    pub spawn_z: f32,
    pub progress: f32,
    last: LastBeat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LastBeat {
    None,
    Hit,
    Miss,
}

impl Default for RhythmGame {
    fn default() -> Self {
        Self {
            hits: 0,
            misses: 0,
            in_window: false,
            clear: false,
            done: false,
            spawn_z: -5.0,
            progress: 0.0,
            last: LastBeat::None,
        }
    }
}

impl RhythmGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let spawn_z = doc
            .props
            .iter()
            .find(|p| p.name == "marker")
            .map(|p| p.position[2])
            .unwrap_or(-5.0);
        Self {
            spawn_z,
            ..Self::default()
        }
    }
}

pub fn is_rhythm(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "stage")
}

fn player_ref(doc: &WorldDoc) -> Option<&WorldWalker> {
    doc.player.as_ref().or(doc.walkers.first())
}

fn write_player(doc: &mut WorldDoc, walker: WorldWalker) {
    if let Some(existing) = doc.player.as_mut() {
        *existing = walker.clone();
    } else {
        doc.player = Some(walker.clone());
    }
    let mut found = false;
    for w in &mut doc.walkers {
        if w.id == walker.id {
            *w = walker.clone();
            found = true;
        }
    }
    if !found {
        if let Some(first) = doc.walkers.first_mut() {
            *first = walker;
        } else {
            doc.walkers.push(walker);
        }
    }
}

/// Sit capsules/boxes on the floor, keep the flag off, coins 0, stage cam.
pub fn seed(doc: &mut WorldDoc) {
    if !is_rhythm(doc) {
        return;
    }
    sit_named(doc, NAME_PLAYER);
    sit_props(doc);
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
        if p.name == "marker" {
            p.enabled = true;
            p.color = Some(MARKER_WAIT);
        }
    }
    doc.coins = 0;
    place_stage_camera(doc);
}

/// Stage cam looks at the judge/marker so the moving beat reads.
pub fn place_stage_camera(doc: &mut WorldDoc) {
    let (jx, jy, jz) = named_prop(doc, "judge")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((0.0, 0.4, 0.8));
    let (mx, my, mz) = named_prop(doc, "marker")
        .map(|p| (p.position[0], p.position[1], p.position[2]))
        .unwrap_or((jx, jy, jz - 4.0));
    let target = [
        jx * 0.55 + mx * 0.45,
        jy.max(my).max(0.55),
        jz * 0.55 + mz * 0.45,
    ];
    let eye = [jx, (jy + 4.6).max(5.0), jz + 8.4];
    let fov = doc.cameras.first().map(|cam| cam.fov).unwrap_or(52.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "stage".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "stage".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// Marker marches toward the judge. J/click in the window hits.
/// Caller may have walked; stage cam wins so the beat stays readable.
pub fn tick(doc: &mut WorldDoc, game: &mut RhythmGame, input: WalkInput, dt: f32) {
    if game.done {
        write_beat(doc, game);
        place_stage_camera(doc);
        return;
    }
    march_marker(doc, dt);
    let in_win = marker_in_window(doc);
    let passed = marker_passed(doc);
    game.in_window = in_win;
    game.progress = marker_progress(doc, game.spawn_z);
    if input.attack && in_win {
        game.hits = game.hits.saturating_add(1);
        game.last = LastBeat::Hit;
        paint_marker(doc, MARKER_HIT);
        reset_marker(doc, game.spawn_z);
        game.in_window = false;
        game.progress = 0.0;
        if game.hits >= NEED {
            game.clear = true;
            game.done = true;
        }
    } else if passed {
        game.misses = game.misses.saturating_add(1);
        game.last = LastBeat::Miss;
        paint_marker(doc, MARKER_MISS);
        reset_marker(doc, game.spawn_z);
        game.in_window = false;
        game.progress = 0.0;
    } else {
        paint_marker(doc, if in_win { MARKER_WINDOW } else { MARKER_WAIT });
    }
    write_beat(doc, game);
    place_stage_camera(doc);
}

fn beat_name(game: &RhythmGame) -> &'static str {
    if game.clear {
        NAME_CLEAR
    } else if game.last == LastBeat::Hit {
        NAME_HIT
    } else if game.last == LastBeat::Miss {
        NAME_MISS
    } else {
        NAME_PLAYER
    }
}

fn write_beat(doc: &mut WorldDoc, game: &RhythmGame) {
    sit_named(doc, beat_name(game));
    doc.coins = game.hits;
    if game.clear {
        set_flag_prop(doc);
    }
}

fn set_flag_prop(doc: &mut WorldDoc) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(FLAG_CLEAR_COLOR);
        }
    }
}

fn sit_named(doc: &mut WorldDoc, name: &str) {
    let Some(p) = player_ref(doc) else {
        return;
    };
    let id = p.id.clone();
    let x = p.position[0];
    let z = p.position[2];
    let yaw = p.yaw;
    let y = doc.height_at(x, z) + BODY_H;
    write_player(
        doc,
        WorldWalker {
            id,
            kind: "walker".into(),
            name: name.into(),
            position: [x, y, z],
            yaw,
            face: yaw,
            on_ground: true,
            ..Default::default()
        },
    );
}

fn sit_props(doc: &mut WorldDoc) {
    let mut updates: Vec<(usize, f32)> = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        let extra = match p.name.as_str() {
            "stage" | "marker" | "judge" | "flag" | "floor" => p.scale[1].abs() * 0.5,
            _ => continue,
        };
        let y = doc.height_at(p.position[0], p.position[2]) + extra;
        updates.push((i, y));
    }
    for (i, y) in updates {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
}

fn named_prop<'a>(doc: &'a WorldDoc, name: &str) -> Option<&'a WorldProp> {
    doc.props.iter().find(|p| p.name == name && p.enabled)
}

fn named_prop_mut<'a>(doc: &'a mut WorldDoc, name: &str) -> Option<&'a mut WorldProp> {
    doc.props.iter_mut().find(|p| p.name == name && p.enabled)
}

fn judge_z(doc: &WorldDoc) -> f32 {
    named_prop(doc, "judge")
        .map(|p| p.position[2])
        .unwrap_or(0.8)
}

fn marker_in_window(doc: &WorldDoc) -> bool {
    let Some(marker) = named_prop(doc, "marker") else {
        return false;
    };
    (marker.position[2] - judge_z(doc)).abs() <= HIT_HALF
}

fn marker_passed(doc: &WorldDoc) -> bool {
    let Some(marker) = named_prop(doc, "marker") else {
        return false;
    };
    marker.position[2] > judge_z(doc) + HIT_HALF
}

fn march_marker(doc: &mut WorldDoc, dt: f32) {
    let y_extra = named_prop(doc, "marker")
        .map(|p| p.scale[1].abs() * 0.5)
        .unwrap_or(0.22);
    let x = named_prop(doc, "marker")
        .map(|p| p.position[0])
        .unwrap_or(0.0);
    let z = named_prop(doc, "marker")
        .map(|p| p.position[2])
        .unwrap_or(-5.0);
    let nz = z + MARKER_SPEED * dt;
    let y = doc.height_at(x, nz) + y_extra;
    if let Some(p) = named_prop_mut(doc, "marker") {
        p.position = [x, y, nz];
    }
}

fn reset_marker(doc: &mut WorldDoc, spawn_z: f32) {
    let y_extra = named_prop(doc, "marker")
        .map(|p| p.scale[1].abs() * 0.5)
        .unwrap_or(0.22);
    let x = named_prop(doc, "marker")
        .map(|p| p.position[0])
        .unwrap_or(0.0);
    let y = doc.height_at(x, spawn_z) + y_extra;
    if let Some(p) = named_prop_mut(doc, "marker") {
        p.position = [x, y, spawn_z];
    }
}

fn paint_marker(doc: &mut WorldDoc, color: [u32; 3]) {
    if let Some(p) = named_prop_mut(doc, "marker") {
        p.color = Some(color);
    }
}

fn marker_progress(doc: &WorldDoc, spawn_z: f32) -> f32 {
    let Some(marker) = named_prop(doc, "marker") else {
        return 0.0;
    };
    let jz = judge_z(doc);
    let span = (jz - spawn_z).abs().max(0.01);
    ((marker.position[2] - spawn_z) / span).clamp(0.0, 1.2)
}

pub fn build_hud(game: &RhythmGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [12, 14, 18, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [22, 24, 32, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [240, 196, 72, 255],
            ));
        }
        GamePhase::Playing => {
            let lane_x = w * 0.18;
            let lane_y = h * 0.78;
            let lane_w = w * 0.64;
            let lane_h = 18.0 * scale;
            quads.push(Quad::new(lane_x, lane_y, lane_w, lane_h, [28, 24, 20, 180]));
            let judge_x = lane_x + lane_w * 0.82;
            quads.push(Quad::new(
                judge_x - 8.0 * scale,
                lane_y - 6.0 * scale,
                16.0 * scale,
                lane_h + 12.0 * scale,
                if game.in_window {
                    HIT_PIP
                } else {
                    [80, 84, 78, 200]
                },
            ));
            let frac = (game.hits as f32 / NEED as f32).clamp(0.0, 1.0);
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale,
                18.0 * scale,
                [28, 24, 20, 160],
            ));
            quads.push(Quad::new(
                16.0 * scale,
                16.0 * scale,
                160.0 * scale * frac.max(0.04),
                18.0 * scale,
                if game.last == LastBeat::Hit {
                    HIT_PIP
                } else if game.last == LastBeat::Miss {
                    MISS_PIP
                } else {
                    [80, 84, 78, 200]
                },
            ));
            let pip_w = 14.0 * scale;
            let pip_x = lane_x + (lane_w - pip_w) * game.progress.clamp(0.0, 1.0);
            quads.push(Quad::new(
                pip_x,
                lane_y - 4.0 * scale,
                pip_w,
                lane_h + 8.0 * scale,
                if game.in_window {
                    HIT_PIP
                } else {
                    [255, 230, 96, 255]
                },
            ));
        }
        GamePhase::Complete => {
            quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 28, 18, 210]));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                CLEAR_PIP,
            ));
        }
    }
    DrawList {
        clear: [22, 24, 32, 255],
        quads,
    }
}

/// HUD overlay pip for the moving marker. `progress` is 0 at spawn, 1 at judge.
pub fn build_hud_with_doc(
    game: &RhythmGame,
    doc: &WorldDoc,
    phase: GamePhase,
    width: u32,
    height: u32,
) -> DrawList {
    let mut hud = build_hud(game, phase, width, height);
    if phase != GamePhase::Playing {
        return hud;
    }
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let lane_x = w * 0.18;
    let lane_y = h * 0.78;
    let lane_w = w * 0.64;
    let lane_h = 18.0 * scale;
    let progress = marker_progress(doc, game.spawn_z);
    let pip_w = 14.0 * scale;
    let pip_x = lane_x + (lane_w - pip_w) * progress.clamp(0.0, 1.0);
    let color = if game.in_window {
        HIT_PIP
    } else {
        [255, 230, 96, 255]
    };
    hud.quads.push(Quad::new(
        pip_x,
        lane_y - 4.0 * scale,
        pip_w,
        lane_h + 8.0 * scale,
        color,
    ));
    hud
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const STAGE: &str = include_str!("../tests/fixtures/rhythm_beat_world.json");
    const CAMP: &str = include_str!("../tests/fixtures/survival_meter_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const PAGES: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const HIDE: &str = include_str!("../tests/fixtures/stealth_hide_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const ROOM: &str = include_str!("../tests/fixtures/puzzle_pad_world.json");
    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");
    const PITCH: &str = include_str!("../tests/fixtures/sports_goal_world.json");
    const YARD: &str = include_str!("../tests/fixtures/sim_meter_world.json");
    const SIDE: &str = include_str!("../tests/fixtures/action_side_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(STAGE).unwrap();
        play.confirm();
        play
    }

    fn in_window(play: &WorldPlay) -> bool {
        marker_in_window(&play.doc)
    }

    #[test]
    fn dump_is_rhythm_stage_not_other_genres() {
        let mut doc = WorldDoc::from_json(STAGE).unwrap();
        seed(&mut doc);
        assert!(is_rhythm(&doc));
        assert_eq!(GAME_ID, "rhythm_beat");
        assert!(!crate::survival::is_survival(&doc));
        assert!(!crate::sim::is_sim(&doc));
        assert!(!crate::puzzle::is_puzzle(&doc));
        assert!(!crate::stealth::is_stealth(&doc));
        assert!(!crate::novel::is_novel(&doc));
        assert!(!crate::rpg::is_rpg(&doc));
        assert!(!crate::fight::is_fight(&doc));
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::action2d::is_action2d(&doc));
        assert!(!crate::fps::is_fps(&doc));
        assert!(!crate::td::is_td(&doc));
        assert!(!crate::race::is_race(&doc));
        assert!(!crate::platformer::is_platformer(&doc));
        assert!(!crate::sports::is_sports(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_rhythm(&crest));
        let camp = WorldDoc::from_json(CAMP).unwrap();
        assert!(crate::survival::is_survival(&camp));
        assert!(!is_rhythm(&camp), "survival camp must not count as rhythm");
        let yard = WorldDoc::from_json(YARD).unwrap();
        assert!(crate::sim::is_sim(&yard));
        assert!(!is_rhythm(&yard));
        let side = WorldDoc::from_json(SIDE).unwrap();
        assert!(crate::action2d::is_action2d(&side));
        assert!(!is_rhythm(&side));
        let room = WorldDoc::from_json(ROOM).unwrap();
        assert!(crate::puzzle::is_puzzle(&room));
        assert!(!is_rhythm(&room), "puzzle pad must not count as rhythm");
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "stage" && p.model == "box"));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "marker" && p.model == "box" && p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "judge" && p.model == "box"));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "floor" && p.model == "box"));
        let marker = doc.props.iter().find(|p| p.name == "marker").unwrap();
        assert!(
            marker.metallic >= 0.5,
            "marker should read metal GGX, metallic={}",
            marker.metallic
        );
        let judge = doc.props.iter().find(|p| p.name == "judge").unwrap();
        assert!(
            judge.metallic >= 0.5,
            "judge should read metal GGX, metallic={}",
            judge.metallic
        );
        let stage = doc.props.iter().find(|p| p.name == "stage").unwrap();
        assert!(
            stage.metallic >= 0.5,
            "stage rim should read metal GGX, metallic={}",
            stage.metallic
        );
        assert_eq!(doc.player.as_ref().unwrap().name, NAME_PLAYER);
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("stage"));
        assert!(json.contains("marker"));
        assert!(json.contains("judge"));
        assert!(json.contains("flag"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 4,
            "floor + stage + marker + judge must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(STAGE).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_rhythm());
        assert!(!play.is_survival());
        assert!(!play.is_sim());
        assert!(!play.is_action2d());
        assert!(!play.is_sports());
        assert!(!play.is_puzzle());
        assert!(!play.is_collectathon());
        let start_z = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "marker")
            .unwrap()
            .position[2];
        let start_coins = play.doc.coins;
        play.input.attack = true;
        play.input.lz = 1.0;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert!(!play.rhythm.done, "title must not march");
        let z = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "marker")
            .unwrap()
            .position[2];
        assert_eq!(z, start_z);
        assert_eq!(play.doc.coins, start_coins);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "stage");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn idle_passes_then_miss() {
        let mut play = play_started();
        assert_eq!(play.doc.coins, 0);
        for _ in 0..240 {
            play.tick(1.0 / 60.0);
            if play.rhythm.misses > 0 {
                break;
            }
        }
        assert!(play.rhythm.misses >= 1, "letting a beat pass is a miss");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_MISS);
        assert_eq!(play.doc.coins, 0);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("miss"), "miss must be dump-visible");
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "miss does not raise the clear flag");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "miss overlay/lane");
    }

    #[test]
    fn attack_in_window_hits() {
        let mut play = play_started();
        for _ in 0..240 {
            play.tick(1.0 / 60.0);
            if in_window(&play) {
                break;
            }
        }
        assert!(in_window(&play), "marker should enter the judge window");
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.rhythm.hits >= 1);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_HIT);
        assert_eq!(play.doc.coins, play.rhythm.hits);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("hit"), "hit must be dump-visible");
        let marker = play.doc.props.iter().find(|p| p.name == "marker").unwrap();
        assert!(
            (marker.position[2] - play.rhythm.spawn_z).abs() < 0.2,
            "hit resets the marker, z={}",
            marker.position[2]
        );
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn attack_outside_window_is_ignored() {
        let mut play = play_started();
        let z0 = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "marker")
            .unwrap()
            .position[2];
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rhythm.hits, 0, "early press is not a hit");
        assert_eq!(play.rhythm.misses, 0, "early press is not a miss");
        let z1 = play
            .doc
            .props
            .iter()
            .find(|p| p.name == "marker")
            .unwrap()
            .position[2];
        assert!(z1 > z0, "marker still marches after ignored press");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
    }

    #[test]
    fn four_window_hits_clear() {
        let mut play = play_started();
        for _ in 0..2000 {
            if in_window(&play) {
                play.input.attack = true;
            }
            play.tick(1.0 / 60.0);
            if play.rhythm.done {
                break;
            }
        }
        assert!(play.rhythm.clear);
        assert!(play.rhythm.done);
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_CLEAR);
        assert_eq!(play.doc.coins, NEED);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "clear flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_CLEAR_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("clear"), "clear must be dump-visible");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "clear overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.rhythm.done);
        assert!(!play.rhythm.clear);
        assert_eq!(play.doc.coins, 0);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_PLAYER);
        let marker = play.doc.props.iter().find(|p| p.name == "marker").unwrap();
        assert!(marker.enabled);
        assert_eq!(play.doc.cameras[0].name, "stage");
    }

    #[test]
    fn held_attack_clears_from_spawn() {
        let mut play = play_started();
        let cam0 = play.doc.cameras[0].position;
        play.input.attack = true;
        for _ in 0..800 {
            play.tick(1.0 / 60.0);
            play.input.attack = true;
            if play.rhythm.done {
                break;
            }
        }
        assert!(
            play.rhythm.clear && play.rhythm.done,
            "held J/click should hit each window and clear, misses={} hits={} coins={} name={:?}",
            play.rhythm.misses,
            play.rhythm.hits,
            play.doc.coins,
            play.doc.player.as_ref().unwrap().name
        );
        assert_eq!(play.game.phase, GamePhase::Complete);
        assert_eq!(play.doc.player.as_ref().unwrap().name, NAME_CLEAR);
        assert_eq!(play.doc.cameras[0].name, "stage");
        let cam1 = play.doc.cameras[0].position;
        let _ = cam0;
        let _ = cam1;
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
        let overlay = crate::rhythm::build_hud_with_doc(
            &play.rhythm,
            &play.doc,
            GamePhase::Playing,
            960,
            540,
        );
        assert!(
            overlay.quads.len() >= 3,
            "lane + judge + moving pip must be visible"
        );
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_rhythm());
        let novel = WorldPlay::from_json(PAGES).unwrap();
        assert!(novel.is_novel());
        assert!(!novel.is_rhythm());
        let stealth = WorldPlay::from_json(HIDE).unwrap();
        assert!(stealth.is_stealth());
        assert!(!stealth.is_rhythm());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_rhythm());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_rhythm());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_rhythm());
        let puzzle = WorldPlay::from_json(ROOM).unwrap();
        assert!(puzzle.is_puzzle());
        assert!(!puzzle.is_rhythm());
        let hop = WorldPlay::from_json(HOP).unwrap();
        assert!(hop.is_platformer());
        assert!(!hop.is_rhythm());
        let sports = WorldPlay::from_json(PITCH).unwrap();
        assert!(sports.is_sports());
        assert!(!sports.is_rhythm());
        let sim = WorldPlay::from_json(YARD).unwrap();
        assert!(sim.is_sim());
        assert!(!sim.is_rhythm());
        let side = WorldPlay::from_json(SIDE).unwrap();
        assert!(side.is_action2d());
        assert!(!side.is_rhythm());
        let camp = WorldPlay::from_json(CAMP).unwrap();
        assert!(camp.is_survival());
        assert!(!camp.is_rhythm());
    }
}

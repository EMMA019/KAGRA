//! Novel pages + a 2-way choice + dump flag on play_world.
//!
//! Sibling of RPG talk. Overlay text pages (Space / click advance), then one
//! 2-way choice writes a dump-visible flag. Title -> play -> result reuses
//! `WorldPlay` / `GamePhase`. Capsule in a room (not VRM). Indoor lights stay
//! 4 slots. Text is overlay HUD state, not screenshot-only. Does not rewrite
//! `rpg.rs` or other genre loops. No portraits, inventory, branching save
//! editor, or net.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldWalker};

pub const GAME_ID: &str = "novel_pages";
pub const STORY_PAGES: u32 = 2;
pub const CHOICE_PAGE: u32 = STORY_PAGES;
pub const FLAG_STAY: &str = "stay";
pub const FLAG_LEAVE: &str = "leave";
pub const BODY_H: f32 = 0.95;
const REPEAT_S: f32 = 0.28;
const SEAT: [f32; 3] = [1.05, 0.95, 1.35];
const SEAT_YAW: f32 = -std::f32::consts::FRAC_PI_2;

const FLAG_STAY_COLOR: [u32; 3] = [240, 196, 72];
const FLAG_LEAVE_COLOR: [u32; 3] = [90, 140, 220];

/// Live novel around a dump. Page / choice stay here; `name` page/choice/stay/leave
/// + flag enable in the dump are the query source of truth.
#[derive(Clone, Debug)]
pub struct NovelGame {
    pub page: u32,
    pub choice: u32,
    pub done: bool,
    pub flag: Option<String>,
    held: bool,
    repeat_t: f32,
}

impl Default for NovelGame {
    fn default() -> Self {
        Self {
            page: 0,
            choice: 0,
            done: false,
            flag: None,
            held: false,
            repeat_t: 0.0,
        }
    }
}

impl NovelGame {
    pub fn from_doc(_doc: &WorldDoc) -> Self {
        Self::default()
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.flag.as_deref() == Some(name)
    }
}

pub fn is_novel(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "page")
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

/// Sit the capsule, keep the flag off until a choice, room camera.
pub fn seed(doc: &mut WorldDoc) {
    if !is_novel(doc) {
        return;
    }
    sit_player(doc, "player");
    let mut speaker_y = Vec::new();
    for (i, p) in doc.props.iter().enumerate() {
        if p.name == "speaker" {
            let extra = BODY_H * p.scale[1].abs().max(0.6);
            let y = doc.height_at(p.position[0], p.position[2]) + extra;
            speaker_y.push((i, y));
        }
    }
    for (i, y) in speaker_y {
        if let Some(p) = doc.props.get_mut(i) {
            p.position[1] = y;
        }
    }
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = false;
        }
    }
    doc.coins = 0;
    place_room_camera(doc);
}

/// Fixed indoor camera so the room stays readable (not chase).
pub fn place_room_camera(doc: &mut WorldDoc) {
    let eye = [0.0, 2.35, 6.2];
    let target = [0.0, 1.05, 0.35];
    let fov = doc.cameras.first().map(|c| c.fov).unwrap_or(48.0);
    if let Some(cam) = doc.cameras.first_mut() {
        cam.position = eye;
        cam.target = target;
        cam.name = "room".into();
    } else {
        doc.cameras.push(crate::world_doc::WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "room".into(),
            position: eye,
            target,
            fov,
        });
    }
}

/// Page / choice tick. Caller may have walked; seated pose + room camera win.
pub fn tick(doc: &mut WorldDoc, game: &mut NovelGame, input: WalkInput, dt: f32) {
    let input = input.clamped();
    sit_player(doc, beat_name(game));
    if game.done {
        write_beat(doc, game);
        place_room_camera(doc);
        return;
    }
    if input.lx < -0.2 {
        game.choice = 0;
    } else if input.lx > 0.2 {
        game.choice = 1;
    }
    let press = input.jump || input.attack;
    if press {
        if !game.held {
            advance(doc, game);
            game.held = true;
            game.repeat_t = REPEAT_S;
        } else {
            game.repeat_t = (game.repeat_t - dt).max(0.0);
            if game.repeat_t <= 0.0 {
                advance(doc, game);
                game.repeat_t = REPEAT_S;
            }
        }
    } else {
        game.held = false;
        game.repeat_t = 0.0;
    }
    write_beat(doc, game);
    place_room_camera(doc);
}

fn beat_name(game: &NovelGame) -> &'static str {
    if game.done {
        if game.choice == 0 {
            FLAG_STAY
        } else {
            FLAG_LEAVE
        }
    } else if game.page >= CHOICE_PAGE {
        "choice"
    } else {
        "page"
    }
}

fn advance(doc: &mut WorldDoc, game: &mut NovelGame) {
    if game.done {
        return;
    }
    if game.page < CHOICE_PAGE {
        game.page += 1;
        return;
    }
    let stay = game.choice == 0;
    let name = if stay { FLAG_STAY } else { FLAG_LEAVE };
    game.flag = Some(name.into());
    game.done = true;
    set_flag_prop(doc, stay);
}

fn set_flag_prop(doc: &mut WorldDoc, stay: bool) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = true;
            p.color = Some(if stay {
                FLAG_STAY_COLOR
            } else {
                FLAG_LEAVE_COLOR
            });
        }
    }
}

fn write_beat(doc: &mut WorldDoc, game: &NovelGame) {
    let name = beat_name(game);
    sit_player(doc, name);
    doc.coins = if game.done {
        10 + game.choice
    } else {
        game.page
    };
}

fn sit_player(doc: &mut WorldDoc, name: &str) {
    let id = player_ref(doc)
        .map(|w| w.id.clone())
        .unwrap_or_else(|| "walker:player".into());
    let y = doc.height_at(SEAT[0], SEAT[2]) + BODY_H;
    write_player(
        doc,
        WorldWalker {
            id,
            kind: "walker".into(),
            name: name.into(),
            position: [SEAT[0], y, SEAT[2]],
            yaw: SEAT_YAW,
            face: SEAT_YAW,
            on_ground: true,
            ..Default::default()
        },
    );
}

pub fn build_hud(game: &NovelGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [18, 12, 16, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [36, 22, 28, 230],
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
            let pip = 16.0 * scale;
            let gap = 6.0 * scale;
            for i in 0..=CHOICE_PAGE {
                let x = 16.0 * scale + i as f32 * (pip + gap);
                let on = game.page >= i;
                quads.push(Quad::new(
                    x,
                    16.0 * scale,
                    pip,
                    pip,
                    if on {
                        [240, 210, 170, 255]
                    } else {
                        [28, 20, 22, 160]
                    },
                ));
            }
            if game.page < CHOICE_PAGE {
                quads.push(Quad::new(
                    w * 0.10,
                    h * 0.62,
                    w * 0.80,
                    h * 0.28,
                    [18, 14, 20, 230],
                ));
                let lines = (game.page + 1).min(2);
                for i in 0..lines {
                    quads.push(Quad::new(
                        w * 0.14,
                        h * 0.68 + i as f32 * 22.0 * scale,
                        w * (0.52 - i as f32 * 0.10),
                        14.0 * scale,
                        [230, 214, 220, 255],
                    ));
                }
            } else {
                quads.push(Quad::new(
                    w * 0.10,
                    h * 0.58,
                    w * 0.80,
                    h * 0.32,
                    [18, 14, 20, 230],
                ));
                let left_on = game.choice == 0;
                quads.push(Quad::new(
                    w * 0.16,
                    h * 0.68,
                    w * 0.30,
                    48.0 * scale,
                    if left_on {
                        [240, 196, 72, 255]
                    } else {
                        [70, 50, 48, 200]
                    },
                ));
                quads.push(Quad::new(
                    w * 0.54,
                    h * 0.68,
                    w * 0.30,
                    48.0 * scale,
                    if !left_on {
                        [90, 140, 220, 255]
                    } else {
                        [48, 50, 70, 200]
                    },
                ));
            }
            if game.has_flag(FLAG_STAY) || game.has_flag(FLAG_LEAVE) {
                quads.push(Quad::new(
                    w - 38.0 * scale,
                    16.0 * scale,
                    22.0 * scale,
                    22.0 * scale,
                    if game.has_flag(FLAG_STAY) {
                        [240, 196, 72, 255]
                    } else {
                        [90, 140, 220, 255]
                    },
                ));
            }
        }
        GamePhase::Complete => {
            let stay = game.choice == 0;
            quads.push(Quad::new(
                0.0,
                h * 0.22,
                w,
                h * 0.36,
                if stay {
                    [28, 18, 12, 210]
                } else {
                    [12, 16, 28, 210]
                },
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                if stay {
                    [240, 196, 72, 240]
                } else {
                    [90, 140, 220, 240]
                },
            ));
        }
    }
    DrawList {
        clear: [42, 32, 36, 255],
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const ROOM: &str = include_str!("../tests/fixtures/novel_pages_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");

    fn play_started() -> WorldPlay {
        let mut play = WorldPlay::from_json(ROOM).unwrap();
        play.confirm();
        play
    }

    fn tap_advance(play: &mut WorldPlay) {
        play.input.jump = true;
        play.input.attack = false;
        play.tick(1.0 / 60.0);
        play.input.jump = false;
        play.tick(1.0 / 60.0);
    }

    #[test]
    fn dump_is_novel_not_rpg_or_fight() {
        let doc = WorldDoc::from_json(ROOM).unwrap();
        assert!(is_novel(&doc));
        assert_eq!(GAME_ID, "novel_pages");
        assert!(!crate::rpg::is_rpg(&doc));
        assert!(!crate::fight::is_fight(&doc));
        assert!(!crate::action::is_action(&doc));
        assert!(!crate::fps::is_fps(&doc));
        let crest = WorldDoc::from_json(CREST).unwrap();
        assert!(!is_novel(&crest));
        let town = WorldDoc::from_json(TOWN).unwrap();
        assert!(!is_novel(&town));
        let ring = WorldDoc::from_json(RING).unwrap();
        assert!(!is_novel(&ring));
        assert!(doc.props.iter().any(|p| p.name == "page"));
        assert!(doc.props.iter().any(|p| p.name == "flag" && !p.enabled));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "speaker" && p.model == "capsule"));
        assert_eq!(doc.player.as_ref().unwrap().name, "player");
        assert!(doc.player.as_ref().unwrap().on_ground);
        assert_eq!(doc.lights.len(), 4);
        assert!(doc.lights.iter().all(|l| l.slot <= 3));
        let mut slots: Vec<u32> = doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let json = doc.to_json().unwrap();
        assert!(json.contains("page"));
        assert!(json.contains("capsule"));
        let scene = doc.compile_scene(16.0 / 9.0);
        assert!(
            scene.instance_count() >= 6,
            "room + capsule must read, n={}",
            scene.instance_count()
        );
        assert_eq!(scene.local_lights.len(), 4);
    }

    #[test]
    fn title_blocks_until_confirm() {
        let mut play = WorldPlay::from_json(ROOM).unwrap();
        assert_eq!(play.game.phase, GamePhase::Title);
        assert!(play.is_novel());
        assert!(!play.is_rpg());
        assert!(!play.is_fight());
        assert!(!play.is_collectathon());
        let start = play.doc.player.as_ref().unwrap().position;
        play.input.jump = true;
        play.input.attack = true;
        for _ in 0..20 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.novel.page, 0, "title must not advance pages");
        assert_eq!(play.doc.player.as_ref().unwrap().position, start);
        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert_eq!(play.doc.cameras[0].name, "room");
    }

    #[test]
    fn space_and_click_advance_pages_as_overlay() {
        let mut play = play_started();
        let hud0 = play.build_hud(960, 540);
        assert!(hud0.quads.len() >= 3, "page overlay");
        play.input.jump = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.novel.page, 1, "Space advances a page");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "page");
        play.input.jump = false;
        play.tick(1.0 / 60.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.novel.page, CHOICE_PAGE, "click advances to choice");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "choice");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("choice"), "choice beat must be dump-visible");
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 4, "choice overlay");
        assert_eq!(play.doc.cameras[0].name, "room");
    }

    #[test]
    fn choice_writes_dump_flag_and_result() {
        let mut play = play_started();
        tap_advance(&mut play);
        tap_advance(&mut play);
        assert_eq!(play.novel.page, CHOICE_PAGE);
        play.input.lx = -1.0;
        tap_advance(&mut play);
        assert!(play.novel.done);
        assert!(play.novel.has_flag(FLAG_STAY));
        assert_eq!(play.game.phase, GamePhase::Complete);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "flag must be dump-visible");
        assert_eq!(flag.color, Some(FLAG_STAY_COLOR));
        assert_eq!(play.doc.player.as_ref().unwrap().name, FLAG_STAY);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("stay"), "stay must be dump-visible");
        assert!(dump.contains("flag"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2, "result overlay");

        play.confirm();
        assert_eq!(play.game.phase, GamePhase::Playing);
        assert!(!play.novel.done);
        assert_eq!(play.novel.page, 0);
        assert!(play.novel.flag.is_none());
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled, "retry restores flag off");
        assert_eq!(play.doc.player.as_ref().unwrap().name, "player");
    }

    #[test]
    fn leave_choice_writes_other_flag() {
        let mut play = play_started();
        tap_advance(&mut play);
        tap_advance(&mut play);
        play.input.lx = 1.0;
        tap_advance(&mut play);
        assert!(play.novel.has_flag(FLAG_LEAVE));
        assert!(!play.novel.has_flag(FLAG_STAY));
        assert_eq!(play.doc.player.as_ref().unwrap().name, FLAG_LEAVE);
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled);
        assert_eq!(flag.color, Some(FLAG_LEAVE_COLOR));
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("leave"));
        assert_eq!(play.game.phase, GamePhase::Complete);
    }

    #[test]
    fn rpg_fight_action_still_own_their_dumps() {
        let town = WorldPlay::from_json(TOWN).unwrap();
        assert!(town.is_rpg());
        assert!(!town.is_novel());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_novel());
        let action = WorldPlay::from_json(ARENA).unwrap();
        assert!(action.is_action());
        assert!(!action.is_novel());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_novel());
    }
}

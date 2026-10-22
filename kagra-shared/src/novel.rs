//! Novel pages + a 2-way choice + dump flag on play_world.
//!
//! Sibling of RPG talk. Overlay text pages (Space / click / tap advance), then
//! one 2-way choice writes a dump-visible flag. Title -> play -> result reuses
//! `WorldPlay` / `GamePhase`. The lines (name, body, choices) live here so a
//! host without Python (Android) can draw the same story. Glyphs go through
//! `font.rs` (Public Sans, PixelMplus fallback). Indoor lights stay 4 slots.
//! Does not rewrite `rpg.rs`. No portraits atlas, inventory, script editor, or net.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
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

/// Speaker name. Katakana so the name plate stays inside PixelMplus.
const SPEAKER: &str = "ユキ";
const TITLE: &str = "雨の駅";
const TITLE_HINT: &str = "タップではじめる";
const PAGES: [&str; STORY_PAGES as usize] =
    ["雨の駅で、傘を閉じた。", "終電はもう出た。残るか、歩くか。"];
const CHOICE_PROMPT: &str = "どうする?";
const CHOICE_LABELS: [&str; 2] = ["残る", "歩く"];
const END_STAY: &str = "朝まで、駅の明かりを数えた。";
const END_LEAVE: &str = "傘を開いて、夜の道へ出た。";
const ADVANCE_HINT: &str = "タップで進む";

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
    /// One-shot advance from a tap (`pointer`). Consumed by `tick`.
    pointer: bool,
    /// Choice index chosen by a tap on a button. Applied after stick `lx`.
    pointer_choice: Option<u32>,
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
            pointer: false,
            pointer_choice: None,
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

pub fn speaker() -> &'static str {
    SPEAKER
}

pub fn body(game: &NovelGame) -> &'static str {
    if game.done {
        if game.choice == 0 {
            END_STAY
        } else {
            END_LEAVE
        }
    } else if game.page >= CHOICE_PAGE {
        CHOICE_PROMPT
    } else {
        PAGES[game.page as usize]
    }
}

pub fn choice_label(index: u32) -> &'static str {
    CHOICE_LABELS.get(index as usize).copied().unwrap_or("")
}

/// Tap / click. Text pages advance anywhere. The choice page only the buttons.
/// Title and result are the host's `confirm` (see `NovelSession::pointer`).
pub fn pointer(game: &mut NovelGame, x: f32, y: f32, width: u32, height: u32) {
    if game.done {
        return;
    }
    let (w, h, scale) = ui_scale(width, height);
    if game.page >= CHOICE_PAGE {
        for (i, rect) in choice_buttons(w, h, scale).iter().enumerate() {
            if rect.contains(x, y) {
                game.pointer_choice = Some(i as u32);
                game.pointer = true;
                return;
            }
        }
        return;
    }
    game.pointer = true;
}

pub fn is_novel(doc: &WorldDoc) -> bool {
    doc.genre_is(GAME_ID)
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
    if let Some(picked) = game.pointer_choice.take() {
        game.choice = picked;
    }
    // A tap is its own edge. Key hold still repeats. Calling `pointer` then
    // `tick` twice must advance two pages (touch does not send a release).
    let tap = game.pointer;
    game.pointer = false;
    let key = input.jump || input.attack;
    if tap {
        advance(doc, game);
        game.held = true;
        game.repeat_t = REPEAT_S;
    } else if key {
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

/// The capsule reader sits in the room. A separate `character` walker is the
/// portrait, so the capsule steps off camera and does not cover them.
fn seat_for(doc: &WorldDoc) -> [f32; 3] {
    if doc.walkers.iter().any(|w| w.name == "character") {
        [8.0, 0.95, 8.0]
    } else {
        SEAT
    }
}

fn sit_player(doc: &mut WorldDoc, name: &str) {
    let id = player_ref(doc)
        .map(|w| w.id.clone())
        .unwrap_or_else(|| "walker:player".into());
    let seat = seat_for(doc);
    let y = doc.height_at(seat[0], seat[2]) + BODY_H;
    write_player(
        doc,
        WorldWalker {
            id,
            kind: "walker".into(),
            name: name.into(),
            position: [seat[0], y, seat[2]],
            yaw: SEAT_YAW,
            face: SEAT_YAW,
            on_ground: true,
            ..Default::default()
        },
    );
}

struct ScreenRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl ScreenRect {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

fn ui_scale(width: u32, height: u32) -> (f32, f32, f32) {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    (w, h, scale)
}

fn choice_buttons(w: f32, h: f32, scale: f32) -> [ScreenRect; 2] {
    let bh = 48.0 * scale;
    [
        ScreenRect {
            x: w * 0.16,
            y: h * 0.68,
            w: w * 0.30,
            h: bh,
        },
        ScreenRect {
            x: w * 0.54,
            y: h * 0.68,
            w: w * 0.30,
            h: bh,
        },
    ]
}

fn text_line(text: &str, x: f32, y: f32, size: f32, color: [u8; 4], align: TextAlign) -> TextQuad {
    TextQuad::new(text, x, y, size, color).aligned(align)
}

pub fn build_hud(game: &NovelGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let (w, h, scale) = ui_scale(width, height);
    let mut quads = Vec::new();
    let mut texts = Vec::new();
    let ink = [236, 228, 220, 255];
    let name_ink = [36, 22, 18, 255];
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
            texts.push(text_line(
                TITLE,
                w * 0.5,
                h * 0.32,
                28.0 * scale,
                ink,
                TextAlign::Center,
            ));
            texts.push(text_line(
                TITLE_HINT,
                w * 0.5,
                h * 0.60,
                16.0 * scale,
                name_ink,
                TextAlign::Center,
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
            let name_y = h * 0.62 - 34.0 * scale;
            quads.push(Quad::new(
                w * 0.10,
                name_y - 4.0 * scale,
                92.0 * scale,
                28.0 * scale,
                [240, 196, 72, 240],
            ));
            texts.push(text_line(
                SPEAKER,
                w * 0.10 + 12.0 * scale,
                name_y,
                16.0 * scale,
                name_ink,
                TextAlign::Left,
            ));
            if game.page < CHOICE_PAGE {
                quads.push(Quad::new(
                    w * 0.10,
                    h * 0.62,
                    w * 0.80,
                    h * 0.28,
                    [18, 14, 20, 230],
                ));
                texts.push(text_line(
                    body(game),
                    w * 0.14,
                    h * 0.70,
                    18.0 * scale,
                    ink,
                    TextAlign::Left,
                ));
                texts.push(text_line(
                    ADVANCE_HINT,
                    w * 0.86,
                    h * 0.84,
                    12.0 * scale,
                    [190, 170, 160, 255],
                    TextAlign::Right,
                ));
            } else {
                quads.push(Quad::new(
                    w * 0.10,
                    h * 0.58,
                    w * 0.80,
                    h * 0.32,
                    [18, 14, 20, 230],
                ));
                texts.push(text_line(
                    body(game),
                    w * 0.14,
                    h * 0.60,
                    16.0 * scale,
                    ink,
                    TextAlign::Left,
                ));
                let buttons = choice_buttons(w, h, scale);
                for (i, rect) in buttons.iter().enumerate() {
                    let on = game.choice == i as u32;
                    quads.push(Quad::new(
                        rect.x,
                        rect.y,
                        rect.w,
                        rect.h,
                        if on {
                            if i == 0 {
                                [240, 196, 72, 255]
                            } else {
                                [90, 140, 220, 255]
                            }
                        } else if i == 0 {
                            [70, 50, 48, 200]
                        } else {
                            [48, 50, 70, 200]
                        },
                    ));
                    texts.push(text_line(
                        CHOICE_LABELS[i],
                        rect.x + rect.w * 0.5,
                        rect.y + 10.0 * scale,
                        18.0 * scale,
                        if on { name_ink } else { ink },
                        TextAlign::Center,
                    ));
                }
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
            texts.push(text_line(
                SPEAKER,
                w * 0.5,
                h * 0.28,
                16.0 * scale,
                [240, 210, 160, 255],
                TextAlign::Center,
            ));
            texts.push(text_line(
                body(game),
                w * 0.5,
                h * 0.36,
                20.0 * scale,
                ink,
                TextAlign::Center,
            ));
        }
    }
    DrawList {
        clear: [42, 32, 36, 255],
        quads,
        texts,
        ..Default::default()
    }
}

/// HUD JSON for `draw_world(..., hud=)`. Same quads and Japanese lines as `build_hud`.
pub fn hud_json(game: &NovelGame, phase: GamePhase, width: u32, height: u32) -> String {
    let hud = build_hud(game, phase, width, height);
    let quads: Vec<_> = hud
        .quads
        .iter()
        .map(|q| {
            serde_json::json!({
                "x": q.x,
                "y": q.y,
                "w": q.w,
                "h": q.h,
                "color": q.color,
            })
        })
        .collect();
    let texts: Vec<_> = hud
        .texts
        .iter()
        .map(|t| {
            let align = match t.align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Right => "right",
            };
            serde_json::json!({
                "text": t.text,
                "x": t.x,
                "y": t.y,
                "size": t.size,
                "color": t.color,
                "align": align,
            })
        })
        .collect();
    serde_json::to_string(&serde_json::json!({ "quads": quads, "texts": texts }))
        .unwrap_or_else(|_| "{}".to_string())
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

    #[test]
    fn japanese_lines_rasterize_without_missing_glyph_boxes() {
        use crate::font::TextRaster;
        use crate::scene::TextQuad;

        let mut play = play_started();
        let mut lines = Vec::new();
        lines.extend(hud_lines(&play.build_hud(960, 540)));
        tap_advance(&mut play);
        lines.extend(hud_lines(&play.build_hud(960, 540)));
        tap_advance(&mut play);
        lines.extend(hud_lines(&play.build_hud(960, 540)));
        let joined = lines.join("");
        assert!(joined.contains(super::SPEAKER), "name plate");
        assert!(joined.contains("雨"), "page one");
        assert!(
            joined.contains("残る") && joined.contains("歩く"),
            "choices"
        );
        let mut raster = TextRaster::new();
        for ch in joined.chars() {
            if ch.is_whitespace() {
                continue;
            }
            let qs = raster.text_quads(&TextQuad::new(
                &ch.to_string(),
                0.0,
                0.0,
                16.0,
                [255, 255, 255, 255],
            ));
            assert!(
                !qs.is_empty(),
                "glyph {ch} (U+{:04X}) did not rasterize",
                ch as u32
            );
            assert!(
                !hollow_box(&qs),
                "glyph {ch} (U+{:04X}) looks like a missing-glyph box",
                ch as u32
            );
        }
    }

    #[test]
    fn tap_advances_and_picks_a_choice() {
        let mut play = play_started();
        pointer(&mut play.novel, 40.0, 40.0, 960, 540);
        play.tick(1.0 / 60.0);
        assert_eq!(play.novel.page, 1, "tap advances a page");
        pointer(&mut play.novel, 40.0, 40.0, 960, 540);
        play.tick(1.0 / 60.0);
        assert_eq!(play.novel.page, CHOICE_PAGE);
        // A tap outside the buttons must not confirm.
        pointer(&mut play.novel, 8.0, 8.0, 960, 540);
        play.tick(1.0 / 60.0);
        assert!(!play.novel.done);
        let (w, h, scale) = super::ui_scale(960, 540);
        let buttons = super::choice_buttons(w, h, scale);
        let stay = &buttons[0];
        pointer(
            &mut play.novel,
            stay.x + stay.w * 0.5,
            stay.y + stay.h * 0.5,
            960,
            540,
        );
        play.tick(1.0 / 60.0);
        assert!(play.novel.has_flag(FLAG_STAY));
        assert_eq!(play.game.phase, GamePhase::Complete);
        let hud = hud_json(&play.novel, play.game.phase, 960, 540);
        assert!(hud.contains(super::END_STAY));
    }

    fn hud_lines(hud: &crate::scene::DrawList) -> Vec<String> {
        hud.texts.iter().map(|t| t.text.clone()).collect()
    }

    fn hollow_box(qs: &[crate::scene::Quad]) -> bool {
        if qs.len() < 8 {
            return false;
        }
        let min_x = qs.iter().map(|q| q.x).fold(f32::MAX, f32::min);
        let max_x = qs.iter().map(|q| q.x).fold(f32::MIN, f32::max);
        let min_y = qs.iter().map(|q| q.y).fold(f32::MAX, f32::min);
        let max_y = qs.iter().map(|q| q.y).fold(f32::MIN, f32::max);
        if max_x - min_x < 4.0 || max_y - min_y < 4.0 {
            return false;
        }
        qs.iter().all(|q| {
            let on_x = (q.x - min_x).abs() < 0.5 || (q.x - max_x).abs() < 0.5;
            let on_y = (q.y - min_y).abs() < 0.5 || (q.y - max_y).abs() < 0.5;
            on_x || on_y
        })
    }
}

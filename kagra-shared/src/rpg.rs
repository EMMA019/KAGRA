//! RPG talk + scene switch + flags on play_world.
//!
//! Sibling of collectathon / action / platformer. Town dump talks to an NPC
//! (overlay + a dump-visible flag), then the door switches to the dungeon dump.
//! No inventory, party, or turn combat. Does not rewrite action or jump.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldDoc, WorldWalker};

pub const GAME_ID: &str = "town_gate";
pub const TALK_REACH: f32 = 1.45;
pub const DOOR_REACH: f32 = 1.55;
pub const FLAG_KEY: &str = "key";
const TOWN_JSON: &str = include_str!("../tests/fixtures/rpg_town_world.json");
const DUNGEON_JSON: &str = include_str!("../tests/fixtures/rpg_dungeon_world.json");

#[derive(Clone, Debug)]
pub struct RpgGame {
    pub talking: bool,
    pub scene: String,
    pub flags: Vec<String>,
    town: WorldDoc,
    dungeon: WorldDoc,
}

impl Default for RpgGame {
    fn default() -> Self {
        Self {
            talking: false,
            scene: "town".into(),
            flags: Vec::new(),
            town: WorldDoc::from_json(TOWN_JSON).unwrap_or_default(),
            dungeon: WorldDoc::from_json(DUNGEON_JSON).unwrap_or_default(),
        }
    }
}

impl RpgGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        Self {
            scene: if is_dungeon(doc) {
                "dungeon".into()
            } else {
                "town".into()
            },
            ..Self::default()
        }
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

pub fn is_rpg(doc: &WorldDoc) -> bool {
    doc.props
        .iter()
        .any(|p| p.name == "npc" || p.name == "door")
}

fn is_dungeon(doc: &WorldDoc) -> bool {
    doc.props.iter().any(|p| p.name == "crystal") && !doc.props.iter().any(|p| p.name == "npc")
}

fn player_ref(doc: &WorldDoc) -> Option<&WorldWalker> {
    doc.player.as_ref().or(doc.walkers.first())
}

fn near(doc: &WorldDoc, name: &str, reach: f32) -> bool {
    let Some(w) = player_ref(doc) else {
        return false;
    };
    doc.props.iter().any(|p| {
        p.enabled && p.name == name && {
            let dx = w.position[0] - p.position[0];
            let dz = w.position[2] - p.position[2];
            (dx * dx + dz * dz).sqrt() <= reach
        }
    })
}

fn set_flag_prop(doc: &mut WorldDoc, on: bool) {
    for p in &mut doc.props {
        if p.name == "flag" {
            p.enabled = on;
        }
    }
}

pub fn seed(_doc: &mut WorldDoc) {}

/// Talk / door. Interact is `WalkInput.attack` (J / click). Jump is unused.
pub fn tick(doc: &mut WorldDoc, game: &mut RpgGame, input: WalkInput, _dt: f32) {
    if input.attack {
        if game.talking {
            game.talking = false;
        } else if near(doc, "npc", TALK_REACH) {
            game.talking = true;
            if !game.has_flag(FLAG_KEY) {
                game.flags.push(FLAG_KEY.into());
            }
            set_flag_prop(doc, true);
        } else if near(doc, "door", DOOR_REACH) {
            try_switch(doc, game);
        }
    }
}

fn try_switch(doc: &mut WorldDoc, game: &mut RpgGame) {
    if game.scene == "town" {
        if !game.has_flag(FLAG_KEY) {
            return;
        }
        *doc = game.dungeon.clone();
        game.scene = "dungeon".into();
        game.talking = false;
    } else {
        let flags = game.flags.clone();
        *doc = game.town.clone();
        set_flag_prop(doc, flags.iter().any(|f| f == FLAG_KEY));
        game.scene = "town".into();
        game.talking = false;
    }
}

pub fn build_hud(game: &RpgGame, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    let mut quads = Vec::new();
    match phase {
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [12, 10, 18, 160]));
            quads.push(Quad::new(
                w * 0.18,
                h * 0.28,
                w * 0.64,
                h * 0.18,
                [28, 22, 40, 230],
            ));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [160, 90, 220, 255],
            ));
        }
        GamePhase::Playing => {
            if game.has_flag(FLAG_KEY) {
                quads.push(Quad::new(
                    16.0 * scale,
                    16.0 * scale,
                    22.0 * scale,
                    22.0 * scale,
                    [240, 196, 72, 255],
                ));
            }
            if game.talking {
                quads.push(Quad::new(
                    w * 0.12,
                    h * 0.62,
                    w * 0.76,
                    h * 0.28,
                    [18, 16, 28, 230],
                ));
                quads.push(Quad::new(
                    w * 0.16,
                    h * 0.68,
                    w * 0.68,
                    18.0 * scale,
                    [220, 210, 230, 255],
                ));
            }
            if game.scene == "dungeon" {
                quads.push(Quad::new(0.0, 0.0, w, h, [40, 20, 70, 40]));
            }
        }
        GamePhase::Complete => {
            quads.push(Quad::new(0.0, h * 0.22, w, h * 0.36, [12, 10, 18, 210]));
            quads.push(Quad::new(
                w * 0.32,
                h * 0.62,
                w * 0.36,
                48.0 * scale,
                [160, 90, 220, 240],
            ));
        }
    }
    DrawList {
        clear: if game.scene == "dungeon" {
            [36, 32, 48, 255]
        } else {
            [120, 160, 110, 255]
        },
        quads,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;

    const TOWN: &str = include_str!("../tests/fixtures/rpg_town_world.json");
    const CREST: &str = include_str!("../tests/fixtures/crest_isle_world.json");
    const ARENA: &str = include_str!("../tests/fixtures/action_arena_world.json");
    const HOP: &str = include_str!("../tests/fixtures/box_hop_world.json");

    fn started() -> WorldPlay {
        let mut play = WorldPlay::from_json(TOWN).unwrap();
        play.start();
        play
    }

    fn put(play: &mut WorldPlay, x: f32, z: f32) {
        let y = play.doc.height_at(x, z) + 0.95;
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [x, y, z];
            p.on_ground = true;
        }
        let w = play.doc.player.clone().unwrap();
        if let Some(existing) = play.doc.player.as_mut() {
            *existing = w.clone();
        }
        for walker in &mut play.doc.walkers {
            if walker.id == w.id {
                *walker = w.clone();
            }
        }
    }

    #[test]
    fn town_dump_is_rpg_not_other_genres() {
        let play = WorldPlay::from_json(TOWN).unwrap();
        assert!(play.is_rpg());
        assert!(!play.is_action());
        assert!(!play.is_platformer());
        assert!(!play.is_collectathon());
        assert!(play.doc.props.iter().any(|p| p.name == "npc"));
        assert!(play.doc.props.iter().any(|p| p.name == "door"));
        assert_eq!(play.game.phase, GamePhase::Title);
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(!crest.is_rpg());
        assert!(!WorldPlay::from_json(ARENA).unwrap().is_rpg());
        assert!(!WorldPlay::from_json(HOP).unwrap().is_rpg());
    }

    #[test]
    fn talk_sets_overlay_and_dump_flag() {
        let mut play = started();
        put(&mut play, 0.0, 2.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.rpg.talking, "talk overlay");
        assert!(play.rpg.has_flag(FLAG_KEY));
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "flag must be dump-visible");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("flag"));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn door_without_flag_stays_in_town() {
        let mut play = started();
        put(&mut play, 4.5, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.scene, "town");
        assert!(play.doc.props.iter().any(|p| p.name == "npc"));
        assert!(!play.doc.props.iter().any(|p| p.name == "crystal"));
    }

    #[test]
    fn talk_then_door_switches_to_dungeon() {
        let mut play = started();
        put(&mut play, 0.0, 2.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = false;
        play.tick(1.0 / 60.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0); // close talk
        put(&mut play, 4.5, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.scene, "dungeon");
        assert!(play.doc.props.iter().any(|p| p.name == "crystal"));
        assert!(!play.doc.props.iter().any(|p| p.name == "npc"));
        assert!(play.rpg.has_flag(FLAG_KEY), "flag survives the switch");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("crystal"));
        // return door
        put(&mut play, 0.0, -5.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.scene, "town");
        assert!(play.doc.props.iter().any(|p| p.name == "npc"));
    }
}

//! RPG talk + menu + party + inventory + turn combat on play_world.
//!
//! Sibling of collectathon / action / platformer / shop. Town dump talks to an
//! NPC (overlay + a dump-visible flag + talk-grant item), menu is an overlay
//! (not a human editor), party is two named walkers, inventory slots are dump
//! props, using/holding is queryable, scene switch keeps that state, and the
//! dungeon has a short two-sided turn overlay (not the 3D action dodge-room).
//! Save roundtrip is the official WorldDoc dump (query/dump/flag). Title ->
//! play -> result. Capsules/boxes, not VRM. Indoor lights stay 4 slots. Does
//! not rewrite action, fight, shop, or jump. No Rapier, SSAO, GI, net, or ECS.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::scene::{DrawList, Quad};
use crate::world_doc::{WorldCamera, WorldDoc, WorldLight, WorldProp, WorldWalker};

pub const GAME_ID: &str = "town_gate";
pub const TALK_REACH: f32 = 1.45;
pub const DOOR_REACH: f32 = 1.55;
pub const ENEMY_REACH: f32 = 1.65;
pub const FLAG_KEY: &str = "key";
pub const ITEM_KEY: &str = "key";
pub const NAME_HERO: &str = "hero";
pub const NAME_ALLY: &str = "ally";
pub const NAME_SLOT: &str = "slot";
pub const NAME_HELD: &str = "held";
pub const NAME_ENEMY: &str = "enemy";
pub const NAME_HP: &str = "hp";
pub const ID_OVERLAY: &str = "prop:overlay";
pub const ID_ITEM: &str = "prop:item-key";
pub const ID_HELD: &str = "prop:held";
pub const ID_SLOT_0: &str = "prop:slot-0";
pub const ID_SLOT_1: &str = "prop:slot-1";
pub const ID_ALLY: &str = "walker:ally";
pub const ID_ENEMY: &str = "prop:enemy";
pub const ID_HP: &str = "prop:hp-foe";
pub const HERO_HP: u32 = 5;
pub const FOE_HP: u32 = 3;
pub const BODY_H: f32 = 0.95;
const TOWN_JSON: &str = include_str!("../tests/fixtures/rpg_town_world.json");
const DUNGEON_JSON: &str = include_str!("../tests/fixtures/rpg_dungeon_world.json");

const OVERLAY_FIELD: &str = "field";
const OVERLAY_TALK: &str = "talk";
const OVERLAY_MENU: &str = "menu";
const OVERLAY_COMBAT: &str = "combat";
const OVERLAY_WIN: &str = "win";
const OVERLAY_LOSE: &str = "lose";

#[derive(Clone, Debug)]
pub struct RpgGame {
    pub talking: bool,
    pub menu: bool,
    pub combat: bool,
    pub scene: String,
    pub flags: Vec<String>,
    pub inventory: Vec<String>,
    pub held: Option<String>,
    pub hp: u32,
    pub foe_hp: u32,
    pub won: bool,
    pub lost: bool,
    town: WorldDoc,
    dungeon: WorldDoc,
}

impl Default for RpgGame {
    fn default() -> Self {
        Self {
            talking: false,
            menu: false,
            combat: false,
            scene: "town".into(),
            flags: Vec::new(),
            inventory: Vec::new(),
            held: None,
            hp: HERO_HP,
            foe_hp: FOE_HP,
            won: false,
            lost: false,
            town: WorldDoc::from_json(TOWN_JSON).unwrap_or_default(),
            dungeon: WorldDoc::from_json(DUNGEON_JSON).unwrap_or_default(),
        }
    }
}

impl RpgGame {
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut g = Self {
            scene: if is_dungeon(doc) {
                "dungeon".into()
            } else {
                "town".into()
            },
            ..Self::default()
        };
        if !is_rpg(doc) {
            return g;
        }
        if doc.props.iter().any(|p| p.name == "flag" && p.enabled) && !g.has_flag(FLAG_KEY) {
            g.flags.push(FLAG_KEY.into());
        }
        if doc.props.iter().any(|p| p.id == ID_ITEM && p.enabled) {
            g.inventory.push(ITEM_KEY.into());
        }
        if doc.props.iter().any(|p| p.name == NAME_HELD && p.enabled) {
            g.held = Some(ITEM_KEY.into());
        }
        match overlay_name(doc) {
            OVERLAY_TALK => g.talking = true,
            OVERLAY_MENU => g.menu = true,
            OVERLAY_COMBAT => g.combat = true,
            OVERLAY_WIN => {
                g.combat = true;
                g.won = true;
            }
            OVERLAY_LOSE => {
                g.combat = true;
                g.lost = true;
            }
            _ => {}
        }
        g.hp = if g.lost {
            0
        } else if doc.coins == 0 {
            HERO_HP
        } else {
            doc.coins.min(HERO_HP)
        };
        if let Some(hp_prop) = doc.props.iter().find(|p| p.id == ID_HP) {
            let n = hp_from_scale(hp_prop.scale[1]);
            if n > 0 {
                g.foe_hp = n.min(FOE_HP);
            }
        }
        if let Some(enemy) = doc.props.iter().find(|p| p.name == NAME_ENEMY) {
            if !enemy.enabled {
                g.foe_hp = 0;
                if g.combat {
                    g.won = true;
                }
            }
        }
        g
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }

    pub fn holding(&self, item: &str) -> bool {
        self.held.as_deref() == Some(item)
    }

    pub fn blocks_walk(&self) -> bool {
        self.talking || self.menu || self.combat
    }

    pub fn overlay(&self) -> &'static str {
        if self.lost {
            OVERLAY_LOSE
        } else if self.won {
            OVERLAY_WIN
        } else if self.combat {
            OVERLAY_COMBAT
        } else if self.talking {
            OVERLAY_TALK
        } else if self.menu {
            OVERLAY_MENU
        } else {
            OVERLAY_FIELD
        }
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

fn overlay_name(doc: &WorldDoc) -> &str {
    doc.props
        .iter()
        .find(|p| p.id == ID_OVERLAY)
        .map(|p| p.name.as_str())
        .unwrap_or(OVERLAY_FIELD)
}

fn hp_from_scale(scale_y: f32) -> u32 {
    let n = (scale_y / 0.28).round();
    if n < 0.0 {
        0
    } else {
        n as u32
    }
}

fn hp_scale(hp: u32) -> f32 {
    0.28 * hp.max(1) as f32
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
        doc.walkers.insert(0, walker);
    }
}

fn named_prop_mut<'a>(doc: &'a mut WorldDoc, id: &str) -> Option<&'a mut WorldProp> {
    doc.props.iter_mut().find(|p| p.id == id)
}

fn ensure_prop(doc: &mut WorldDoc, prop: WorldProp) {
    if doc.props.iter().any(|p| p.id == prop.id) {
        return;
    }
    doc.props.push(prop);
}

fn tiny_prop(
    id: &str,
    name: &str,
    position: [f32; 3],
    enabled: bool,
    color: [u32; 3],
) -> WorldProp {
    WorldProp {
        id: id.into(),
        kind: "prop".into(),
        name: name.into(),
        position,
        yaw: 0.0,
        model: "box".into(),
        gltf: None,
        scale: [0.22, 0.22, 0.22],
        enabled,
        parent: None,
        color: Some(color),
        metallic: 0.0,
        roughness: 1.0,
    }
}

/// Sit party / slots / overlay. Do not wipe a mid-dump inventory or flag.
pub fn seed(doc: &mut WorldDoc) {
    if !is_rpg(doc) {
        return;
    }
    ensure_party(doc);
    ensure_slots(doc);
    ensure_overlay(doc);
    ensure_flag(doc);
    if is_dungeon(doc) {
        ensure_enemy(doc);
        ensure_indoor_lights(doc);
    }
}

fn ensure_flag(doc: &mut WorldDoc) {
    if doc.props.iter().any(|p| p.name == "flag") {
        return;
    }
    ensure_prop(
        doc,
        tiny_prop(
            "prop:flag-key",
            "flag",
            [0.0, 0.4, 2.2],
            false,
            [240, 196, 72],
        ),
    );
}

/// After collectathon coin recount (which zeros non-coin dumps), put HP back.
pub fn restore_coins(doc: &mut WorldDoc, coins_before: u32) {
    if !is_rpg(doc) {
        return;
    }
    if overlay_name(doc) == OVERLAY_LOSE {
        doc.coins = 0;
        return;
    }
    doc.coins = if coins_before == 0 {
        HERO_HP
    } else {
        coins_before
    };
}

fn ensure_party(doc: &mut WorldDoc) {
    if let Some(p) = doc.player.as_mut() {
        if p.name != NAME_ALLY {
            p.name = NAME_HERO.into();
        }
    }
    for w in &mut doc.walkers {
        if w.id != ID_ALLY {
            w.name = NAME_HERO.into();
        }
    }
    if let Some(p) = doc.player.clone() {
        write_player(doc, p);
    }
    if doc
        .walkers
        .iter()
        .any(|w| w.id == ID_ALLY || w.name == NAME_ALLY)
    {
        for w in &mut doc.walkers {
            if w.id == ID_ALLY {
                w.name = NAME_ALLY.into();
            }
        }
        return;
    }
    let (x, z, yaw) = player_ref(doc)
        .map(|p| (p.position[0], p.position[2], p.yaw))
        .unwrap_or((0.0, -2.0, 0.0));
    let y = doc.height_at(x - 1.1, z) + BODY_H;
    doc.walkers.push(WorldWalker {
        id: ID_ALLY.into(),
        kind: "walker".into(),
        name: NAME_ALLY.into(),
        position: [x - 1.1, y, z],
        yaw,
        face: yaw,
        on_ground: true,
    });
}

fn ensure_slots(doc: &mut WorldDoc) {
    let (px, pz) = player_ref(doc)
        .map(|p| (p.position[0], p.position[2]))
        .unwrap_or((0.0, -2.0));
    ensure_prop(
        doc,
        tiny_prop(
            ID_SLOT_0,
            NAME_SLOT,
            [px - 0.55, 0.35, pz - 0.8],
            true,
            [70, 62, 54],
        ),
    );
    ensure_prop(
        doc,
        tiny_prop(
            ID_SLOT_1,
            NAME_SLOT,
            [px - 0.15, 0.35, pz - 0.8],
            true,
            [70, 62, 54],
        ),
    );
    ensure_prop(
        doc,
        tiny_prop(
            ID_ITEM,
            ITEM_KEY,
            [px - 0.55, 0.55, pz - 0.8],
            false,
            [240, 196, 72],
        ),
    );
    ensure_prop(
        doc,
        tiny_prop(
            ID_HELD,
            NAME_HELD,
            [px + 0.35, 1.4, pz],
            false,
            [240, 196, 72],
        ),
    );
    if let Some(item) = named_prop_mut(doc, ID_ITEM) {
        item.name = ITEM_KEY.into();
        item.metallic = 0.85;
        item.roughness = 0.18;
    }
}

fn ensure_overlay(doc: &mut WorldDoc) {
    ensure_prop(
        doc,
        tiny_prop(
            ID_OVERLAY,
            OVERLAY_FIELD,
            [0.0, 0.2, 0.0],
            true,
            [40, 36, 52],
        ),
    );
}

fn ensure_enemy(doc: &mut WorldDoc) {
    if !doc.props.iter().any(|p| p.name == NAME_ENEMY) {
        doc.props.push(WorldProp {
            id: ID_ENEMY.into(),
            kind: "prop".into(),
            name: NAME_ENEMY.into(),
            position: [1.15, BODY_H, 1.15],
            yaw: -2.2,
            model: "capsule".into(),
            gltf: None,
            scale: [0.7, 1.1, 0.7],
            enabled: true,
            parent: None,
            color: Some([200, 64, 72]),
            metallic: 0.0,
            roughness: 1.0,
        });
    }
    if doc.props.iter().any(|p| p.id == ID_HP) {
        return;
    }
    let (ex, ez) = doc
        .props
        .iter()
        .find(|p| p.name == NAME_ENEMY)
        .map(|p| (p.position[0], p.position[2]))
        .unwrap_or((1.15, 1.15));
    ensure_prop(
        doc,
        WorldProp {
            id: ID_HP.into(),
            kind: "prop".into(),
            name: NAME_HP.into(),
            position: [ex, 2.15, ez],
            yaw: 0.0,
            model: "box".into(),
            gltf: None,
            scale: [0.35, hp_scale(FOE_HP), 0.35],
            enabled: false,
            parent: None,
            color: Some([220, 70, 70]),
            metallic: 0.0,
            roughness: 1.0,
        },
    );
}

fn ensure_indoor_lights(doc: &mut WorldDoc) {
    let fills: [(&str, [f32; 3], f32); 4] = [
        ("key", [2.2, 3.4, 1.6], 1.0),
        ("fill", [-2.4, 2.8, 2.2], 0.7),
        ("rim", [0.2, 2.6, -3.4], 0.55),
        ("bounce", [-1.6, 0.9, -1.2], 0.4),
    ];
    let mut used = [false; 4];
    for light in &mut doc.lights {
        let slot = (light.slot as usize).min(3);
        light.slot = slot as u32;
        used[slot] = true;
        if light.kind.is_empty() {
            light.kind = "point".into();
        }
    }
    for (slot, (name, pos, intensity)) in fills.iter().enumerate() {
        if used[slot] {
            continue;
        }
        doc.lights.push(WorldLight {
            id: format!("light:{slot}"),
            kind_type: "light".into(),
            name: (*name).into(),
            position: *pos,
            kind: "point".into(),
            slot: slot as u32,
            intensity: *intensity,
            radius: 12.0,
            color: Some([1.0, 0.9, 0.78]),
            direction: None,
        });
    }
    if doc.cameras.is_empty() {
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "main".into(),
            position: [0.0, 5.1, -12.2],
            target: [0.0, 1.25, 0.0],
            fov: 54.0,
        });
    }
}

/// Talk / menu / door / turn. Interact is `WalkInput.attack` (J / click).
/// Jump toggles the menu overlay. Dodge closes overlays. Jump is not a hop.
pub fn tick(doc: &mut WorldDoc, game: &mut RpgGame, input: WalkInput, _dt: f32) {
    if game.lost || game.won {
        write_beat(doc, game);
        return;
    }
    if game.combat {
        if input.attack {
            turn_action(game);
        }
        write_beat(doc, game);
        return;
    }
    if game.talking {
        if input.attack || input.dodge {
            game.talking = false;
        }
        write_beat(doc, game);
        return;
    }
    if game.menu {
        if input.dodge || input.jump {
            game.menu = false;
        } else if input.attack {
            use_held_item(game);
        }
        write_beat(doc, game);
        return;
    }
    if input.jump {
        game.menu = true;
        write_beat(doc, game);
        return;
    }
    if input.attack {
        if near(doc, "npc", TALK_REACH) {
            game.talking = true;
            grant_key(doc, game);
        } else if near(doc, "door", DOOR_REACH) {
            try_switch(doc, game);
        } else if near(doc, NAME_ENEMY, ENEMY_REACH) {
            game.combat = true;
            game.menu = false;
            if game.hp == 0 {
                game.hp = HERO_HP;
            }
            if game.foe_hp == 0 {
                game.foe_hp = FOE_HP;
            }
        }
    }
    write_beat(doc, game);
}

fn grant_key(doc: &mut WorldDoc, game: &mut RpgGame) {
    if !game.has_flag(FLAG_KEY) {
        game.flags.push(FLAG_KEY.into());
    }
    if !game.inventory.iter().any(|i| i == ITEM_KEY) {
        game.inventory.push(ITEM_KEY.into());
    }
    set_flag_prop(doc, true);
}

fn use_held_item(game: &mut RpgGame) {
    if game.inventory.iter().any(|i| i == ITEM_KEY) {
        game.held = Some(ITEM_KEY.into());
    }
}

fn turn_action(game: &mut RpgGame) {
    if game.foe_hp > 0 {
        game.foe_hp -= 1;
    }
    if game.foe_hp == 0 {
        game.won = true;
        return;
    }
    if game.hp > 0 {
        game.hp -= 1;
    }
    if game.hp == 0 {
        game.lost = true;
    }
}

fn try_switch(doc: &mut WorldDoc, game: &mut RpgGame) {
    if game.scene == "town" {
        if !game.has_flag(FLAG_KEY) {
            return;
        }
        *doc = game.dungeon.clone();
        game.scene = "dungeon".into();
    } else {
        *doc = game.town.clone();
        game.scene = "town".into();
    }
    game.talking = false;
    game.menu = false;
    game.combat = false;
    apply_carried(doc, game);
}

fn apply_carried(doc: &mut WorldDoc, game: &RpgGame) {
    seed(doc);
    set_flag_prop(doc, game.has_flag(FLAG_KEY));
    write_beat(doc, game);
}

fn write_beat(doc: &mut WorldDoc, game: &RpgGame) {
    ensure_party(doc);
    ensure_slots(doc);
    ensure_overlay(doc);
    ensure_flag(doc);
    if game.scene == "dungeon" {
        ensure_enemy(doc);
        ensure_indoor_lights(doc);
    }
    sit_party(doc);
    if let Some(item) = named_prop_mut(doc, ID_ITEM) {
        item.enabled = game.inventory.iter().any(|i| i == ITEM_KEY);
        item.name = ITEM_KEY.into();
        item.metallic = 0.85;
        item.roughness = 0.18;
    }
    if let Some(held) = named_prop_mut(doc, ID_HELD) {
        held.enabled = game.held.is_some();
        held.name = NAME_HELD.into();
    }
    set_flag_prop(doc, game.has_flag(FLAG_KEY));
    if let Some(ov) = named_prop_mut(doc, ID_OVERLAY) {
        ov.name = game.overlay().into();
        ov.enabled = true;
    }
    doc.coins = game.hp;
    let enemy_xz = doc
        .props
        .iter()
        .find(|p| p.name == NAME_ENEMY)
        .map(|p| (p.position[0], p.position[2]));
    if let Some(enemy) = doc.props.iter_mut().find(|p| p.name == NAME_ENEMY) {
        enemy.enabled = game.foe_hp > 0;
    }
    if let Some((ex, ez)) = enemy_xz {
        if let Some(hp) = doc.props.iter_mut().find(|p| p.id == ID_HP) {
            hp.name = NAME_HP.into();
            hp.enabled = game.combat;
            hp.position = [ex, 2.15, ez];
            hp.scale = [0.35, hp_scale(game.foe_hp.max(1)), 0.35];
            hp.color = Some([40 + game.foe_hp * 60, 48, 52]);
        }
    }
}

fn sit_party(doc: &mut WorldDoc) {
    let Some(p) = player_ref(doc) else {
        return;
    };
    let mut hero = p.clone();
    hero.name = NAME_HERO.into();
    let x = hero.position[0];
    let z = hero.position[2];
    let yaw = hero.yaw;
    hero.position[1] = doc.height_at(x, z) + BODY_H;
    hero.on_ground = true;
    write_player(doc, hero);
    let ay = doc.height_at(x - 1.1, z) + BODY_H;
    if let Some(ally) = doc.walkers.iter_mut().find(|w| w.id == ID_ALLY) {
        ally.name = NAME_ALLY.into();
        ally.position = [x - 1.1, ay, z];
        ally.yaw = yaw;
        ally.face = yaw;
        ally.on_ground = true;
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
            if game.inventory.iter().any(|i| i == ITEM_KEY) {
                quads.push(Quad::new(
                    44.0 * scale,
                    16.0 * scale,
                    18.0 * scale,
                    18.0 * scale,
                    [240, 196, 72, 220],
                ));
            }
            if game.held.is_some() {
                quads.push(Quad::new(
                    68.0 * scale,
                    16.0 * scale,
                    14.0 * scale,
                    14.0 * scale,
                    [255, 230, 140, 255],
                ));
            }
            quads.push(Quad::new(
                16.0 * scale,
                44.0 * scale,
                16.0 * scale,
                16.0 * scale,
                [90, 160, 220, 255],
            ));
            quads.push(Quad::new(
                36.0 * scale,
                44.0 * scale,
                16.0 * scale,
                16.0 * scale,
                [120, 200, 140, 255],
            ));
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
            if game.menu {
                quads.push(Quad::new(
                    w * 0.22,
                    h * 0.18,
                    w * 0.56,
                    h * 0.58,
                    [16, 14, 24, 236],
                ));
                quads.push(Quad::new(
                    w * 0.28,
                    h * 0.26,
                    w * 0.44,
                    22.0 * scale,
                    [160, 90, 220, 255],
                ));
                quads.push(Quad::new(
                    w * 0.28,
                    h * 0.40,
                    w * 0.20,
                    18.0 * scale,
                    [90, 160, 220, 255],
                ));
                quads.push(Quad::new(
                    w * 0.52,
                    h * 0.40,
                    w * 0.20,
                    18.0 * scale,
                    [120, 200, 140, 255],
                ));
            }
            if game.combat {
                quads.push(Quad::new(
                    w * 0.10,
                    h * 0.12,
                    w * 0.80,
                    h * 0.22,
                    [18, 10, 16, 230],
                ));
                let hero_frac = (game.hp as f32 / HERO_HP as f32).clamp(0.0, 1.0);
                let foe_frac = (game.foe_hp as f32 / FOE_HP as f32).clamp(0.0, 1.0);
                quads.push(Quad::new(
                    w * 0.14,
                    h * 0.16,
                    w * 0.30 * hero_frac,
                    16.0 * scale,
                    [80, 200, 110, 255],
                ));
                quads.push(Quad::new(
                    w * 0.56,
                    h * 0.16,
                    w * 0.30 * foe_frac,
                    16.0 * scale,
                    [220, 70, 70, 255],
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
                if game.won {
                    [160, 90, 220, 240]
                } else {
                    [180, 50, 60, 240]
                },
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
    const STALL: &str = include_str!("../tests/fixtures/shop_buy_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");

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
        for walker in &mut play.doc.walkers {
            if walker.id == w.id {
                *walker = w.clone();
            }
        }
    }

    fn names(doc: &WorldDoc) -> Vec<String> {
        let mut n: Vec<String> = doc.walkers.iter().map(|w| w.name.clone()).collect();
        if let Some(p) = doc.player.as_ref() {
            n.push(p.name.clone());
        }
        n.sort();
        n.dedup();
        n
    }

    #[test]
    fn town_dump_is_rpg_not_other_genres() {
        let play = WorldPlay::from_json(TOWN).unwrap();
        assert!(play.is_rpg());
        assert!(!play.is_action());
        assert!(!play.is_platformer());
        assert!(!play.is_collectathon());
        assert!(!play.is_shop());
        assert!(!play.is_fight());
        assert!(play.doc.props.iter().any(|p| p.name == "npc"));
        assert!(play.doc.props.iter().any(|p| p.name == "door"));
        assert_eq!(play.game.phase, GamePhase::Title);
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(!crest.is_rpg());
        assert!(!WorldPlay::from_json(ARENA).unwrap().is_rpg());
        assert!(!WorldPlay::from_json(HOP).unwrap().is_rpg());
        assert!(!WorldPlay::from_json(STALL).unwrap().is_rpg());
        assert!(!WorldPlay::from_json(RING).unwrap().is_rpg());
    }

    #[test]
    fn party_two_named_members_in_dump() {
        let play = started();
        let n = names(&play.doc);
        assert!(n.iter().any(|s| s == NAME_HERO), "hero in dump: {n:?}");
        assert!(n.iter().any(|s| s == NAME_ALLY), "ally in dump: {n:?}");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains(NAME_HERO));
        assert!(dump.contains(NAME_ALLY));
        assert!(play.doc.walkers.len() >= 2);
    }

    #[test]
    fn talk_sets_overlay_and_dump_flag() {
        let mut play = started();
        put(&mut play, 0.0, 2.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.rpg.talking, "talk overlay");
        assert_eq!(play.rpg.overlay(), OVERLAY_TALK);
        assert!(play.rpg.has_flag(FLAG_KEY));
        let flag = play.doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(flag.enabled, "flag must be dump-visible");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("flag"));
        assert!(dump.contains(OVERLAY_TALK));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 2);
    }

    #[test]
    fn talk_grants_inventory_item_dump_visible() {
        let mut play = started();
        assert!(play.doc.props.iter().any(|p| p.name == NAME_SLOT));
        put(&mut play, 0.0, 2.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.rpg.inventory.iter().any(|i| i == ITEM_KEY));
        let item = play
            .doc
            .props
            .iter()
            .find(|p| p.id == ID_ITEM)
            .expect("item slot");
        assert!(item.enabled, "granted item must be dump-visible");
        assert_eq!(item.name, ITEM_KEY);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains(ITEM_KEY));
        assert!(dump.contains(NAME_SLOT));
    }

    #[test]
    fn menu_overlay_uses_and_holds_item() {
        let mut play = started();
        put(&mut play, 0.0, 2.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0); // close talk
        play.input.jump = true;
        play.tick(1.0 / 60.0);
        assert!(play.rpg.menu, "menu overlay");
        assert_eq!(play.rpg.overlay(), OVERLAY_MENU);
        assert!(play.doc.props.iter().any(|p| p.name == OVERLAY_MENU));
        play.input.jump = false;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.rpg.holding(ITEM_KEY), "using/holding is queryable");
        let held = play
            .doc
            .props
            .iter()
            .find(|p| p.name == NAME_HELD)
            .expect("held prop");
        assert!(held.enabled, "holding must be dump-visible");
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains(NAME_HELD));
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 4, "menu overlay");
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
        assert!(
            play.rpg.inventory.iter().any(|i| i == ITEM_KEY),
            "inventory survives the switch"
        );
        let n = names(&play.doc);
        assert!(n.iter().any(|s| s == NAME_HERO));
        assert!(n.iter().any(|s| s == NAME_ALLY));
        assert_eq!(play.doc.lights.len(), 4);
        let mut slots: Vec<u32> = play.doc.lights.iter().map(|l| l.slot).collect();
        slots.sort();
        assert_eq!(slots, vec![0, 1, 2, 3]);
        let dump = play.doc.to_json().unwrap();
        assert!(dump.contains("crystal"));
        assert!(
            !dump.contains("\"name\": \"npc\"") || play.doc.props.iter().all(|p| p.name != "npc")
        );
        // return door
        put(&mut play, 0.0, -5.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.scene, "town");
        assert!(play.doc.props.iter().any(|p| p.name == "npc"));
    }

    #[test]
    fn dump_save_roundtrip_party_inventory_flags_scene() {
        let mut play = started();
        put(&mut play, 0.0, 2.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.jump = true;
        play.tick(1.0 / 60.0);
        play.input.jump = false;
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = false;
        play.input.jump = true;
        play.tick(1.0 / 60.0); // close menu
        play.input.jump = false;
        put(&mut play, 4.5, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.scene, "dungeon");
        let json = play.doc.to_json().unwrap();
        let loaded_doc = WorldDoc::from_json(&json).unwrap();
        let loaded = RpgGame::from_doc(&loaded_doc);
        assert_eq!(loaded.scene, "dungeon");
        assert!(loaded.has_flag(FLAG_KEY));
        assert!(loaded.inventory.iter().any(|i| i == ITEM_KEY));
        assert!(loaded.holding(ITEM_KEY));
        let n = names(&loaded_doc);
        assert!(n.iter().any(|s| s == NAME_HERO));
        assert!(n.iter().any(|s| s == NAME_ALLY));
        let play2 = WorldPlay::from_json(&json).unwrap();
        assert!(play2.is_rpg());
        assert_eq!(play2.rpg.scene, "dungeon");
        assert!(play2.rpg.has_flag(FLAG_KEY));
        assert!(play2.rpg.inventory.iter().any(|i| i == ITEM_KEY));
        assert!(play2.doc.props.iter().any(|p| p.name == "crystal"));
        assert_eq!(play2.doc.coins, play.rpg.hp);
    }

    #[test]
    fn turn_combat_hp_in_dump_then_win() {
        let mut play = started();
        put(&mut play, 0.0, 2.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        put(&mut play, 4.5, 0.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.scene, "dungeon");
        put(&mut play, 1.15, 1.15);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert!(play.rpg.combat, "turn overlay");
        assert_eq!(play.rpg.overlay(), OVERLAY_COMBAT);
        assert_eq!(play.rpg.hp, HERO_HP);
        assert_eq!(play.rpg.foe_hp, FOE_HP);
        assert_eq!(play.doc.coins, HERO_HP, "hero HP in dump");
        let hp_prop = play
            .doc
            .props
            .iter()
            .find(|p| p.id == ID_HP)
            .expect("foe hp dump");
        assert!(hp_prop.enabled);
        assert_eq!(hp_prop.name, NAME_HP);
        let hud = play.build_hud(960, 540);
        assert!(hud.quads.len() >= 3, "combat overlay");
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.foe_hp, FOE_HP - 1);
        assert_eq!(play.rpg.hp, HERO_HP - 1);
        assert_eq!(play.doc.coins, HERO_HP - 1);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        play.input.attack = true;
        play.tick(1.0 / 60.0);
        assert_eq!(play.rpg.foe_hp, 0);
        assert!(play.rpg.won);
        assert!(!play.rpg.lost);
        assert_eq!(play.rpg.overlay(), OVERLAY_WIN);
        assert_eq!(play.game.phase, GamePhase::Complete);
        let enemy = play
            .doc
            .props
            .iter()
            .find(|p| p.name == NAME_ENEMY)
            .unwrap();
        assert!(!enemy.enabled, "foe down is dump-visible");
    }

    #[test]
    fn other_genres_still_own_their_dumps() {
        let stall = WorldPlay::from_json(STALL).unwrap();
        assert!(stall.is_shop());
        assert!(!stall.is_rpg());
        let fight = WorldPlay::from_json(RING).unwrap();
        assert!(fight.is_fight());
        assert!(!fight.is_rpg());
        let crest = WorldPlay::from_json(CREST).unwrap();
        assert!(crest.is_collectathon());
        assert!(!crest.is_rpg());
    }
}

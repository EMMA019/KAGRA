//! SURVIVOR ARENA: a one-thumb arena survivor (auto-attack, a swarm, XP gems,
//! pick-one-of-three upgrades). Design: `docs/design/survivor_arena.md`.
//!
//! Build step 1: the hero follows the finger, one auto-aimed weapon, one swarm
//! enemy in growing numbers, gems, the level-up cards, and a result with the run
//! stats the meta layer turns into coins. Everything tunable is in the JSON
//! (`SurvivorSpec`); the sim is a fixed 60 Hz step, deterministic for a seed.
//!
//! Performance rules (see the design, section 10): fixed pools sized once from
//! the quality tier, a spatial grid for every neighbour question, no allocation
//! in a step, and a persistent prop list whose entries are updated in place.

use std::cell::Cell;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::flow::{FlowPhase, GameFlow};
use crate::game_core::enemies::{weighted_pick, EnemySpec};
use crate::game_core::motion::{self, Motion};
use crate::game_core::stick;
use crate::game_core::swarm::{Budget, Foes, Grid, Rng};
use crate::game_core::view::View;
use crate::game_core::{tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};
use crate::survivor_content::{
    default_enemies, default_foes, default_synergies, default_weapons, DropSpec, FoeAi, FoeSpec,
    PickupKind, SynergySpec,
};
use crate::world_doc::{WorldCamera, WorldDoc};

mod doc;
mod hud;
mod hud_extra;
mod input;
mod loot;
mod view;
mod weapons;

pub const GAME_ID: &str = "survivor_arena";

/// True for a world file whose `genre` is this kit's.
pub fn is_survivor_dump(json: &str) -> bool {
    serde_json::from_str::<Value>(json)
        .ok()
        .and_then(|v| {
            v.get("genre")
                .and_then(|g| g.as_str())
                .map(|s| s == GAME_ID)
        })
        .unwrap_or(false)
}

mod spec;
pub use spec::*;

// --------------------------------------------------------------- stats ----

/// A weapon's numbers at its current level.
#[derive(Clone, Copy, Debug, PartialEq)]
struct WeaponStats {
    damage: f32,
    cooldown: f32,
    speed: f32,
    range: f32,
    count: i32,
    pierce: i32,
    spread: f32,
    size: f32,
    area: f32,
}

impl WeaponStats {
    fn from_base(b: &WeaponBase) -> Self {
        Self {
            damage: b.damage,
            cooldown: b.cooldown,
            speed: b.speed,
            range: b.range,
            count: b.count as i32,
            pierce: b.pierce as i32,
            spread: b.spread_deg.to_radians(),
            size: b.size,
            area: b.area,
        }
    }

    fn add(&mut self, l: &WeaponLevel) {
        self.damage += l.damage;
        self.cooldown += l.cooldown;
        self.speed += l.speed;
        self.range += l.range;
        self.count += l.count;
        self.pierce += l.pierce;
        self.area += l.area;
    }
}

/// What the passives add up to.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Mods {
    hp_max: f32,
    speed: f32,
    magnet: f32,
    damage: f32,
    cooldown: f32,
    xp: f32,
}

/// One enemy kind, flattened for the hot loops.
#[derive(Clone, Copy, Debug)]
struct Kind {
    hp: f32,
    speed: f32,
    radius: f32,
    dmg: f32,
    xp: f32,
    unlock: f32,
    weight: u32,
    color: [u32; 3],
    linear: [f32; 3],
    ai: FoeAi,
    scale: f32,
    tint: [u32; 3],
    weave: f32,
    armor: f32,
    resist: f32,
    hold: f32,
    every: f32,
    shot_dmg: f32,
    shot_speed: f32,
    /// Index into `drops.tables`, or `NO_TABLE`.
    table: u8,
    drop_mul: f32,
    elite: bool,
    /// Which model pool draws it (`MODEL_*`).
    model: u8,
}

const NO_TABLE: u8 = 255;
/// Pool slots: ghost, zombie, skeleton, vampire, keeper. `MODEL_MIX` is the ghoul crowd that
/// shows as ghosts and (one in four) zombies.
const MODEL_MIX: u8 = 255;
const POOLS: usize = 5;
/// Enemy orbs, blades, zaps and rings alive at once.
const ORB_CAP: usize = 24;
const BLADE_CAP: usize = 6;
const ZAP_CAP: usize = 6;
const ZAP_PTS: usize = 10;
const RING_CAP: usize = 8;
const CHEST_CAP: usize = 3;
const ORB_LIFE: f32 = 4.5;
const ORB_R: f32 = 0.22;
const TOAST_LIFE: f32 = 2.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Card {
    Weapon(u8),
    Passive(u8),
    Heal,
    /// A full weapon bar: drop this weapon for the one that was found.
    Swap(u8),
    /// A full weapon bar: keep what you have (a little XP instead).
    Keep,
}

/// Per-foe state that the shared `Foes` pool does not carry; index-for-index with it.
#[derive(Clone, Copy, Debug, Default)]
struct FoeAux {
    /// Seconds until the blades may cut this foe again.
    blade_cd: f32,
    /// Shooter: seconds until the next orb.
    clock: f32,
    hp_max: f32,
}

/// An enemy orb.
#[derive(Clone, Copy, Debug)]
struct Orb {
    x: f32,
    z: f32,
    vx: f32,
    vz: f32,
    life: f32,
    dmg: f32,
}

/// A pickup lying in the arena.
#[derive(Clone, Copy, Debug)]
struct Pickup {
    x: f32,
    z: f32,
    /// Index into `drops.items`, or `CHEST`.
    item: u8,
    /// The table a chest opens on.
    table: u8,
    age: f32,
    fly: bool,
}

const CHEST: u8 = 255;

/// A lightning bolt on screen for a moment: the hero, then each foe it hit.
#[derive(Clone, Copy, Debug)]
struct Zap {
    pts: [[f32; 2]; ZAP_PTS],
    n: u8,
    age: f32,
    w: u8,
}

const ZAP_LIFE: f32 = 0.24;

/// An expanding ring on the floor (pulse, blast, bomb).
#[derive(Clone, Copy, Debug)]
struct Ring {
    x: f32,
    z: f32,
    r: f32,
    age: f32,
    life: f32,
    color: [u8; 3],
}

#[derive(Clone, Copy, Debug)]
struct Shot {
    x: f32,
    z: f32,
    vx: f32,
    vz: f32,
    life: f32,
    dmg: f32,
    pierce: i32,
    r: f32,
    hits: [u32; 4],
    n: u8,
    /// 0 bolt, 1 pellet, 2 homing wisp.
    kind: u8,
    /// The weapon that fired it (its colour and sums).
    w: u8,
    /// Wisp: blast radius.
    area: f32,
}

#[derive(Clone, Copy, Debug)]
struct Gem {
    x: f32,
    z: f32,
    v: f32,
    fly: bool,
}

/// Where each pooled prop sits in `doc.props`.
#[derive(Clone, Copy, Debug, Default)]
struct Slots {
    hero: usize,
    /// The blaster in the hero's hands (`glb_gun`: the model is there).
    gun: usize,
    glb_gun: bool,
    shadow: usize,
    /// Per model pool: first prop, size (0: no such model, its foes use pool 0), props drawn last frame.
    pool0: [usize; POOLS],
    pool_cap: [usize; POOLS],
    drawn: [usize; POOLS],
    gem0: usize,
    shot0: usize,
    orb0: usize,
    blade0: usize,
    glb_blade: bool,
    pick0: usize,
    glow0: usize,
    chest0: usize,
    glb_chest: bool,
    glb_foe: bool,
    glb_gem: bool,
    // line list offsets in `doc.lines`
    ln_ring: usize,
    ln_aim: usize,
    ln_beam: usize,
    ln_zap: usize,
    ln_fring: usize,
    ln_shield: usize,
    ln_orbit: usize,
}

const MAX_FOE_R: f32 = 1.0;
/// Contact distance a foe is held at, as a share of the two radii.
const HOLD: f32 = 0.85;
const PUSH_NEIGHBOURS: u32 = 6;

/// One effect particle (hit spark, puff, glint, muzzle flash).
#[derive(Clone, Copy, Debug)]
struct Fx {
    x: f32,
    y: f32,
    z: f32,
    vx: f32,
    vy: f32,
    vz: f32,
    age: f32,
    life: f32,
    kind: u8,
}

/// A floating damage number (world position, shown by the HUD).
#[derive(Clone, Copy, Debug)]
struct DmgNum {
    x: f32,
    z: f32,
    val: i32,
    age: f32,
}

/// A gem dropped within this far (m) of a resting one adds to it; the wider reach applies
/// once the pool is more than three-fifths full.
const GEM_MERGE_R: f32 = 0.7;
const GEM_MERGE_FULL_R: f32 = 1.7;

const DMG_CAP: [usize; 3] = [6, 12, 20];
const DMG_LIFE: f32 = 0.7;
const FX_SPARK: u8 = 0;
const FX_PUFF_GHOST: u8 = 1;
const FX_PUFF_ZOMBIE: u8 = 2;
const FX_GLINT: u8 = 3;
const FX_MUZZLE: u8 = 4;
const FX_HURT: u8 = 5;
const FX_BLAST: u8 = 6;
const FX_ZAP: u8 = 7;
/// Per kind: sRGB colour, sprite radius (m) and opacity for the young, middle and old third
/// of its life. One point cloud per kind and third, refilled in place each frame.
const FX_LOOK: [([u8; 3], [f32; 3], [u8; 3]); 8] = [
    ([185, 240, 255], [0.11, 0.08, 0.05], [255, 235, 190]),
    ([185, 165, 255], [0.16, 0.26, 0.34], [210, 130, 60]),
    ([140, 225, 150], [0.16, 0.26, 0.34], [210, 130, 60]),
    ([255, 225, 110], [0.10, 0.08, 0.05], [255, 230, 160]),
    ([255, 225, 160], [0.28, 0.18, 0.10], [255, 200, 110]),
    ([255, 85, 95], [0.16, 0.12, 0.08], [255, 230, 160]),
    ([255, 190, 150], [0.22, 0.34, 0.42], [200, 90, 150]),
    ([255, 250, 160], [0.13, 0.09, 0.06], [190, 220, 255]),
];

#[derive(Debug)]
pub struct SurvivorArena {
    pub spec: SurvivorSpec,
    pub flow: GameFlow,
    pub core: Core,
    doc: WorldDoc,
    slots: Slots,
    picture_quality: Option<String>,
    /// Graphics-tier hints (`quality_hints`): the share of arena dressing and of spark
    /// particles drawn. Picture only: the sim and its RNG never read them.
    hint_atmo: f32,
    hint_particles: f32,
    kinds: Vec<Kind>,
    foes: Foes,
    grid: Grid,
    push_x: Vec<f32>,
    push_z: Vec<f32>,
    shots: Vec<Shot>,
    gems: Vec<Gem>,
    aux: Vec<FoeAux>,
    orbs: Vec<Orb>,
    pickups: Vec<Pickup>,
    zaps: Vec<Zap>,
    rings: Vec<Ring>,
    /// Scratch for area hits: foe indices, sized once to the foe cap.
    scratch: Vec<usize>,
    /// Spins the blades.
    orbit_phase: f32,
    // buffs and loot
    shield_t: f32,
    frenzy_t: f32,
    frenzy_amt: f32,
    /// While above 0 every gem and pickup flies to the hero (a magnet).
    pull_all_t: f32,
    since_drop: u32,
    /// Seconds into the run of the next elite (0: none).
    next_elite: f32,
    /// The weapon waiting for a slot while the swap cards are up.
    swap_in: Option<u8>,
    toast: String,
    toast_t: f32,
    toast_col: [u8; 3],
    /// Which synergies are on (bit per `spec.synergies` row).
    syn_mask: u32,
    budget: Budget,
    rng: Rng,
    /// Effects roll their own dice, so switching them off never changes a run.
    fx_rng: Rng,
    fx: Vec<Fx>,
    fx_cap: usize,
    dmg: Vec<DmgNum>,
    tier: usize,
    /// Camera zoom-in kick, 0..1, decays fast.
    zoom_kick: f32,
    // hero
    hx: f32,
    hz: f32,
    vx: f32,
    vz: f32,
    face: f32,
    /// Seconds the hero keeps facing its last target after a shot.
    aim_hold: f32,
    hp: f32,
    hurt_cd: f32,
    hurt_flash: f32,
    // run
    t: f32,
    anim_t: f32,
    kills: u32,
    level: u32,
    xp: f32,
    pending_levels: u32,
    weapon_lv: Vec<u32>,
    weapon_cd: Vec<f32>,
    weapon_stats: Vec<WeaponStats>,
    passive_lv: Vec<u32>,
    mods: Mods,
    cards: Vec<Card>,
    win: bool,
    new_best: bool,
    death_text: String,
    // input
    finger_id: Option<u32>,
    finger: Option<(f32, f32)>,
    origin: (f32, f32),
    mute_finger: bool,
    /// Manual aim: 0 auto, 1 manual, 2 hybrid (a pause-card setting, kept across runs).
    aim_mode: u8,
    /// The locked foe (`Foes::uid`), its last known index, and a ground-aimed direction (yaw).
    lock: Option<u32>,
    lock_idx: usize,
    aim_dir: Option<f32>,
    /// The lone finger's first position and age (s), to tell a tap from a drag.
    tap: Option<(f32, f32, f32)>,
    key_dir: (f32, f32),
    size: Cell<(f32, f32)>,
    // presentation
    lang_ja: bool,
    sfx: bool,
    cue_t: [f32; 5],
}

fn lin(c: [u32; 3]) -> [f32; 3] {
    c.map(|v| (v as f32 / 255.0).powf(2.2))
}

impl SurvivorArena {
    pub fn new(spec: SurvivorSpec) -> Self {
        let mut flow = GameFlow::new(GAME_ID);
        flow.title = spec.title.to_uppercase();
        flow.tagline = "SURVIVE UNTIL DAWN".into();
        let kinds = spec
            .enemies
            .types
            .iter()
            .map(|t| {
                let s = t.swarm.clone().unwrap_or_default();
                let f = spec
                    .foes
                    .iter()
                    .find(|f| f.id == t.id)
                    .cloned()
                    .unwrap_or_else(|| FoeSpec {
                        id: t.id.clone(),
                        ..FoeSpec::default()
                    });
                Kind {
                    ai: f.ai,
                    scale: f.scale,
                    tint: f.tint,
                    weave: f.weave,
                    armor: f.armor,
                    resist: f.knock_resist,
                    hold: f.hold_m,
                    every: f.shoot_every,
                    shot_dmg: f.shot_dmg,
                    shot_speed: f.shot_speed,
                    table: spec
                        .drops
                        .tables
                        .iter()
                        .position(|d| d.id == f.table)
                        .map_or(NO_TABLE, |i| i as u8),
                    drop_mul: f.drop_mul,
                    elite: f.elite,
                    model: match t.look.as_str() {
                        "ghost" => 0,
                        "zombie" => 1,
                        "skeleton" => 2,
                        "vampire" => 3,
                        "keeper" => 4,
                        _ => MODEL_MIX,
                    },
                    hp: s.hp,
                    speed: s.speed,
                    radius: s.radius,
                    dmg: s.touch_dmg,
                    xp: s.xp,
                    unlock: s.unlock_secs,
                    weight: s.weight,
                    color: t.color,
                    linear: lin(t.color),
                }
            })
            .collect();
        let half = spec.arena.half;
        let (nw, np) = (spec.weapons.len(), spec.passives.len());
        let mut me = Self {
            flow,
            core: Core::default(),
            doc: WorldDoc::default(),
            slots: Slots::default(),
            picture_quality: None,
            hint_atmo: 1.0,
            hint_particles: 1.0,
            kinds,
            foes: Foes::default(),
            grid: Grid::new(half + 3.0, 2.0, 1),
            push_x: Vec::new(),
            push_z: Vec::new(),
            shots: Vec::new(),
            gems: Vec::new(),
            aux: Vec::new(),
            orbs: Vec::new(),
            pickups: Vec::new(),
            zaps: Vec::new(),
            rings: Vec::new(),
            scratch: Vec::new(),
            orbit_phase: 0.0,
            shield_t: 0.0,
            frenzy_t: 0.0,
            frenzy_amt: 0.0,
            pull_all_t: 0.0,
            since_drop: 0,
            next_elite: 0.0,
            swap_in: None,
            toast: String::new(),
            toast_t: 0.0,
            toast_col: [255, 255, 255],
            syn_mask: 0,
            budget: Budget::default(),
            rng: Rng::new(spec.director.seed),
            fx_rng: Rng::new(spec.director.seed ^ 0x9e37_79b9),
            fx: Vec::new(),
            fx_cap: 0,
            dmg: Vec::new(),
            tier: 1,
            zoom_kick: 0.0,
            hx: 0.0,
            hz: 0.0,
            vx: 0.0,
            vz: 0.0,
            face: 0.0,
            aim_hold: 0.0,
            hp: spec.hero.max_hp,
            hurt_cd: 0.0,
            hurt_flash: 0.0,
            t: 0.0,
            anim_t: 0.0,
            kills: 0,
            level: 1,
            xp: 0.0,
            pending_levels: 0,
            weapon_lv: vec![0; nw],
            weapon_cd: vec![0.0; nw],
            weapon_stats: Vec::new(),
            passive_lv: vec![0; np],
            mods: Mods {
                hp_max: spec.hero.max_hp,
                speed: 1.0,
                magnet: 1.0,
                damage: 1.0,
                cooldown: 1.0,
                xp: 1.0,
            },
            cards: Vec::new(),
            win: false,
            new_best: false,
            death_text: String::new(),
            finger_id: None,
            finger: None,
            origin: (0.0, 0.0),
            mute_finger: false,
            aim_mode: aim_code(&spec.input.aim),
            lock: None,
            lock_idx: 0,
            aim_dir: None,
            tap: None,
            key_dir: (0.0, 0.0),
            size: Cell::new((390.0, 844.0)),
            lang_ja: false,
            sfx: true,
            cue_t: [0.0; 5],
            spec,
        };
        me.apply_quality();
        me.core.stats.alive = true;
        me.recompute();
        me.hp = me.mods.hp_max;
        me
    }

    /// Size the pools for the quality tier (`low`, `mobile` / default, `high`).
    fn apply_quality(&mut self) {
        let tier = match self.picture_quality.as_deref() {
            Some("low") => 0,
            Some("high") => 2,
            _ => 1,
        };
        self.tier = tier;
        self.fx_cap = self.spec.fx.cap[tier] as usize;
        self.fx = Vec::with_capacity(self.fx_cap);
        self.dmg = Vec::with_capacity(DMG_CAP[tier]);
        let cap = self.spec.caps.foes[tier] as usize;
        self.foes = Foes::with_capacity(cap);
        self.grid = Grid::new(self.spec.arena.half + 3.0, 2.0, cap);
        self.push_x = Vec::with_capacity(cap);
        self.push_z = Vec::with_capacity(cap);
        self.shots = Vec::with_capacity(self.spec.caps.shots as usize);
        self.gems = Vec::with_capacity(self.spec.caps.gems as usize);
        self.aux = Vec::with_capacity(cap);
        self.scratch = Vec::with_capacity(cap);
        self.orbs = Vec::with_capacity(ORB_CAP);
        self.pickups = Vec::with_capacity(self.spec.drops.cap as usize);
        self.zaps = Vec::with_capacity(ZAP_CAP);
        self.rings = Vec::with_capacity(RING_CAP);
        self.doc = WorldDoc::default();
    }

    pub fn set_sfx(&mut self, on: bool) {
        self.sfx = on;
    }

    pub fn set_lang(&mut self, lang: &str) {
        self.lang_ja = lang.eq_ignore_ascii_case("ja");
    }

    fn tr<'a>(&self, en: &'a str, ja: &'a str) -> &'a str {
        if self.lang_ja {
            ja
        } else {
            en
        }
    }

    fn cue(&mut self, name: &str, slot: usize, gap: f32) {
        if self.sfx && self.cue_t[slot] <= 0.0 {
            self.cue_t[slot] = gap;
            crate::mixer::cue_sfx(name);
        }
    }

    // ---- derived numbers

    fn recompute(&mut self) {
        let mut m = Mods {
            hp_max: self.spec.hero.max_hp,
            speed: 1.0,
            magnet: 1.0,
            damage: 1.0,
            cooldown: 1.0,
            xp: 1.0,
        };
        for (i, p) in self.spec.passives.iter().enumerate() {
            let n = self.passive_lv[i] as f32;
            let add = p.per_level * n;
            match p.stat {
                PassiveStat::MaxHp => m.hp_max += add,
                PassiveStat::Speed => m.speed += add,
                PassiveStat::Magnet => m.magnet += add,
                PassiveStat::Might => m.damage += add,
                PassiveStat::Haste => m.cooldown = (m.cooldown - add).max(0.2),
                PassiveStat::Growth => m.xp += add,
            }
        }
        self.mods = m;
        self.weapon_stats.clear();
        for (i, w) in self.spec.weapons.iter().enumerate() {
            let mut s = WeaponStats::from_base(&w.base);
            let lv = self.weapon_lv[i].max(1) as usize;
            for l in w.levels.iter().take(lv - 1) {
                s.add(l);
            }
            self.weapon_stats.push(s);
        }
        // Synergies: a weapon in hand plus a passive at its level switches a bonus on.
        let old = self.syn_mask;
        self.syn_mask = 0;
        for (k, sy) in self.spec.synergies.iter().enumerate().take(32) {
            let (Some(wi), Some(pi)) = (
                self.spec.weapons.iter().position(|w| w.id == sy.weapon),
                self.spec.passives.iter().position(|p| p.id == sy.passive),
            ) else {
                continue;
            };
            if self.weapon_lv[wi] == 0 || self.passive_lv[pi] < sy.min_level {
                continue;
            }
            self.syn_mask |= 1 << k;
            let st = &mut self.weapon_stats[wi];
            st.damage *= 1.0 + sy.damage;
            st.cooldown *= 1.0 + sy.cooldown;
            st.count += sy.count;
            st.pierce += sy.pierce;
            st.area *= 1.0 + sy.area;
        }
        for (i, w) in self.spec.weapons.iter().enumerate() {
            let st = &mut self.weapon_stats[i];
            let max_n = match w.kind {
                WeaponKind::Orbit | WeaponKind::Missile => BLADE_CAP as i32,
                _ => 8,
            };
            st.count = st.count.clamp(1, max_n);
            st.pierce = st.pierce.clamp(0, 3);
            st.cooldown = st.cooldown.max(0.1);
        }
        let fresh = self.syn_mask & !old;
        if fresh != 0 && self.flow.phase == FlowPhase::Play {
            let k = fresh.trailing_zeros() as usize;
            let name = self.spec.synergies[k].name.get(self.lang_ja).to_string();
            let head = self.tr("SYNERGY", "シナジー");
            self.set_toast(&format!("{head}  {name}"), [255, 225, 120]);
        }
    }

    fn set_toast(&mut self, text: &str, col: [u8; 3]) {
        self.toast.clear();
        self.toast.push_str(text);
        self.toast_t = TOAST_LIFE;
        self.toast_col = col;
    }

    /// Weapons in hand.
    fn held_weapons(&self) -> u32 {
        self.weapon_lv.iter().filter(|l| **l > 0).count() as u32
    }

    fn xp_need(&self) -> f32 {
        let n = (self.level - 1) as f32;
        let l = &self.spec.levels;
        l.xp_base + l.xp_step * n + l.xp_quad * n * n
    }

    fn score_now(&self) -> u32 {
        let r = &self.spec.run;
        self.kills * r.score_per_kill
            + (self.level - 1) * r.score_per_level
            + self.t as u32 * r.score_per_sec
            + if self.win { r.win_bonus } else { 0 }
    }

    // ---- run control

    pub fn start_run(&mut self) {
        self.flow.enter_play();
        crate::mixer::play_bgm("bgm", 0.5);
        self.core.stats = Default::default();
        self.core.juice.shake = 0.0;
        self.core.juice.hitstop = 0.0;
        self.core.result.reset();
        self.foes.clear();
        self.shots.clear();
        self.gems.clear();
        self.aux.clear();
        self.orbs.clear();
        self.pickups.clear();
        self.zaps.clear();
        self.rings.clear();
        self.orbit_phase = 0.0;
        self.shield_t = 0.0;
        self.frenzy_t = 0.0;
        self.pull_all_t = 0.0;
        self.since_drop = 0;
        self.swap_in = None;
        self.toast.clear();
        self.toast_t = 0.0;
        self.syn_mask = 0;
        let d = &self.spec.director;
        let has_elite = self.kinds.iter().any(|k| k.elite);
        self.next_elite = if d.elite_first > 0.0 && has_elite {
            d.elite_first
        } else {
            0.0
        };
        self.budget.reset();
        self.rng = Rng::new(self.spec.director.seed);
        self.fx_rng = Rng::new(self.spec.director.seed ^ 0x9e37_79b9);
        self.fx.clear();
        self.dmg.clear();
        self.zoom_kick = 0.0;
        (self.hx, self.hz, self.vx, self.vz, self.face) = (0.0, 0.0, 0.0, 0.0, 0.0);
        self.hurt_cd = 0.0;
        self.hurt_flash = 0.0;
        self.t = 0.0;
        self.kills = 0;
        self.level = 1;
        self.xp = 0.0;
        self.pending_levels = 0;
        self.cards.clear();
        self.win = false;
        self.new_best = false;
        self.death_text.clear();
        self.finger = None;
        self.finger_id = None;
        self.mute_finger = false;
        self.lock = None;
        self.aim_dir = None;
        self.tap = None;
        self.weapon_lv.iter_mut().for_each(|l| *l = 0);
        self.weapon_cd.iter_mut().for_each(|c| *c = 0.0);
        self.passive_lv.iter_mut().for_each(|l| *l = 0);
        if let Some(w) = self
            .spec
            .weapons
            .iter()
            .position(|w| w.id == self.spec.hero.start_weapon)
        {
            self.weapon_lv[w] = 1;
        }
        for sp in self.spec.hero.start_passives.clone() {
            if let Some(i) = self.spec.passives.iter().position(|p| p.id == sp.id) {
                self.passive_lv[i] = sp.level;
            }
        }
        self.recompute();
        self.hp = self.mods.hp_max;
        // Say what the hero starts with: the chip alone was an unreadable glyph.
        if let Some(i) = (0..self.spec.weapons.len()).find(|&i| self.weapon_lv[i] > 0) {
            let w = &self.spec.weapons[i];
            let line = format!(
                "{}  {}  {}",
                w.glyph,
                w.name.get(self.lang_ja),
                self.tr("equipped", "そうび")
            );
            let col = w.color.map(|c| c.min(255) as u8);
            self.set_toast(&line, col);
        }
    }

    pub fn show_title(&mut self) {
        self.flow.show_title();
        self.core.stats.alive = true;
        self.core.stats.score = 0;
        self.cards.clear();
    }

    pub fn confirm(&mut self) {
        if matches!(self.flow.phase, FlowPhase::Title | FlowPhase::Result) {
            self.start_run();
        }
    }

    pub fn pause(&mut self) {
        if self.flow.phase == FlowPhase::Play && self.core.stats.alive {
            self.flow.pause();
            self.finger = None;
            self.finger_id = None;
        }
    }

    pub fn resume_from_page(&mut self) {
        self.flow.resume();
    }

    fn finish(&mut self, win: bool) {
        self.win = win;
        self.cards.clear();
        self.core.stats.score = self.score_now();
        self.core.stats.rings = self.level;
        self.core.stats.alive = false;
        self.core.stats.death = if win { String::new() } else { "swarmed".into() };
        self.death_text = if win {
            self.tr("You made it to dawn!", "あさまでまもりきった！")
                .into()
        } else {
            self.tr("The swarm got you", "むれにつかまった").into()
        };
        self.new_best = self.core.stats.score > self.flow.best_score;
        self.core.juice.hitstop = if win { 0.45 } else { 0.35 };
        self.core.juice.shake = if win { 0.0 } else { 0.8 };
        crate::mixer::stop_bgm(0.6);
        self.cue_t[3] = 0.0;
        self.cue(if win { "win" } else { "gameover" }, 3, 1.0);
    }

    // ---- the step

    fn step_sim(&mut self, dt: f32) {
        self.anim_t += dt;
        if let Some(t) = &mut self.tap {
            t.2 += dt;
        }
        for c in &mut self.cue_t {
            *c = (*c - dt).max(0.0);
        }
        self.hurt_flash = (self.hurt_flash - dt * 3.0).max(0.0);
        self.toast_t = (self.toast_t - dt).max(0.0);
        self.step_fx(dt);
        if !self.cards.is_empty() {
            return;
        }
        if self.pending_levels > 0 {
            self.open_cards();
            return;
        }
        self.t += dt;
        self.hurt_cd = (self.hurt_cd - dt).max(0.0);
        self.shield_t = (self.shield_t - dt).max(0.0);
        self.frenzy_t = (self.frenzy_t - dt).max(0.0);
        self.pull_all_t = (self.pull_all_t - dt).max(0.0);
        self.move_hero(dt);
        self.spawn(dt);
        self.grid.rebuild(&self.foes.x, &self.foes.z);
        self.move_foes(dt);
        self.step_orbs(dt);
        self.fire(dt);
        self.step_blades(dt);
        self.move_shots(dt);
        self.step_vfx(dt);
        self.reap();
        self.collect(dt);
        self.collect_loot(dt);
        let need = self.xp_need();
        if self.xp >= need {
            self.xp -= need;
            self.level += 1;
            self.pending_levels += 1;
            self.cue("levelup", 2, 0.2);
            self.zoom_kick = 1.0;
            let at = [self.hx, 0.3, self.hz];
            self.burst(FX_GLINT, 14, at, [0.0, 0.0], 3.4, 1.0, 3.2, 0.7);
        }
        self.core.stats.score = self.score_now();
        if self.hp <= 0.0 {
            self.hp = 0.0;
            self.finish(false);
        } else if self.t >= self.spec.run.secs {
            self.finish(true);
        }
    }

    /// The camera for a screen aspect: eye, look point, vertical fov (degrees).
    fn camera(&self, aspect: f32) -> ([f32; 3], [f32; 3], f32) {
        let c = &self.spec.camera;
        let half = self.spec.arena.half - 2.0;
        let (fx, fz) = (self.hx.clamp(-half, half), self.hz.clamp(-half, half));
        let eye = [fx, c.height, fz + c.back + c.hero_up_m];
        let target = [fx, 0.0, fz + c.hero_up_m];
        let dist = ((eye[1] - target[1]).powi(2) + (eye[2] - target[2]).powi(2)).sqrt();
        let half_w = c.view_width_m * 0.5;
        let fov = 2.0 * (half_w / (dist * aspect.max(0.2))).atan().to_degrees();
        (eye, target, fov.clamp(c.fov_min, c.fov_max))
    }

    /// A world point on the canvas (pixels), or `None` behind the camera.
    fn project(&self, p: [f32; 3], w: f32, h: f32) -> Option<(f32, f32)> {
        let (eye, target, fov) = self.camera(w / h.max(1.0));
        View { eye, target, fov }.project(p, w, h)
    }

    /// The ground point under a canvas pixel, or `None` for a sky ray.
    fn ground_at(&self, px: f32, py: f32) -> Option<(f32, f32)> {
        let (w, h) = self.size.get();
        let (eye, target, fov) = self.camera(w / h.max(1.0));
        View { eye, target, fov }.ground_at(px, py, w, h, 0.0)
    }

    fn move_hero(&mut self, dt: f32) {
        let sp = self.spec.hero.speed * self.mods.speed;
        let mut want = (0.0_f32, 0.0_f32);
        if let Some((fx, fy)) = self.finger {
            if self.stick_mode() {
                let (w, h) = self.size.get();
                let rad = self.stick_radius(w, h);
                let (ax, ay) =
                    stick::analog(self.origin, (fx, fy), rad, self.spec.input.stick_dead);
                want = (ax * sp, ay * sp);
            } else if let Some((gx, gz)) = self.ground_at(fx, fy) {
                let (tx, tz) = (gx, gz - self.spec.input.thumb_offset_m);
                let (dx, dz) = (tx - self.hx, tz - self.hz);
                let d = dx.hypot(dz);
                let i = &self.spec.input;
                if d > i.dead_zone_m {
                    let k = ((d - i.dead_zone_m) / (i.full_speed_m - i.dead_zone_m)).min(1.0);
                    want = (dx / d * sp * k, dz / d * sp * k);
                }
            }
        } else {
            let (kx, kz) = self.key_dir;
            let len = kx.hypot(kz);
            if len > 0.01 {
                want = (kx / len * sp, kz / len * sp);
            }
        }
        let a = (dt * 16.0).min(1.0);
        self.vx += (want.0 - self.vx) * a;
        self.vz += (want.1 - self.vz) * a;
        self.hx += self.vx * dt;
        self.hz += self.vz * dt;
        let lim = self.spec.arena.half - self.spec.hero.radius;
        self.hx = self.hx.clamp(-lim, lim);
        self.hz = self.hz.clamp(-lim, lim);
        self.aim_hold = (self.aim_hold - dt).max(0.0);
        if self.aim_hold <= 0.0 && self.vx.hypot(self.vz) > 0.4 {
            self.face = self.vx.atan2(self.vz);
        }
    }

    fn spawn(&mut self, dt: f32) {
        self.spawn_elite();
        let d = &self.spec.director;
        let rate = Budget::rate(d.start_rate, d.end_rate, d.ramp_secs, d.curve, self.t);
        let n = self.budget.take(rate, dt);
        if n == 0 {
            return;
        }
        let mut weights = [0_u32; 8];
        let mut ids = [0_usize; 8];
        let mut m = 0;
        for (i, k) in self.kinds.iter().enumerate() {
            if k.unlock <= self.t && !k.elite && m < 8 {
                weights[m] = k.weight;
                ids[m] = i;
                m += 1;
            }
        }
        if m == 0 {
            return;
        }
        let half = self.spec.arena.half - 0.6;
        let hp_mul = 1.0 + self.spec.director.hp_per_min * self.t / 60.0;
        for _ in 0..n {
            let kind = ids[weighted_pick(&weights[..m], self.rng.next_u32())];
            let mut pos = (0.0, 0.0);
            for _ in 0..4 {
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let r = self.spec.director.spawn_dist;
                pos = (
                    (self.hx + a.sin() * r).clamp(-half, half),
                    (self.hz + a.cos() * r).clamp(-half, half),
                );
                if (pos.0 - self.hx).hypot(pos.1 - self.hz) > r * 0.7 {
                    break;
                }
            }
            let hp = self.kinds[kind].hp * hp_mul;
            if !self.add_foe(kind, pos, hp) {
                break; // the cap is the budget: the rest of this step's births are dropped
            }
        }
    }

    /// Add a foe and its aux row. False when the pool is full.
    fn add_foe(&mut self, kind: usize, pos: (f32, f32), hp: f32) -> bool {
        self.sync_aux();
        if self.foes.spawn(kind as u8, pos.0, pos.1, hp).is_none() {
            return false;
        }
        let every = self.kinds[kind].every;
        let clock = self.rng.range(0.6, every.max(0.7));
        self.aux.push(FoeAux {
            blade_cd: 0.0,
            clock,
            hp_max: hp,
        });
        true
    }

    /// Keep `aux` index-for-index with the foes (a test or a mod may add foes directly).
    fn sync_aux(&mut self) {
        let n = self.foes.len();
        if self.aux.len() > n {
            self.aux.truncate(n);
        }
        while self.aux.len() < n {
            let hp = self.foes.hp[self.aux.len()];
            self.aux.push(FoeAux {
                blade_cd: 0.0,
                clock: 1.5,
                hp_max: hp,
            });
        }
    }

    /// Rough damage per second of the weapons the hero holds (every hit landing).
    pub(super) fn firepower(&self) -> f32 {
        let mut sum = 0.0;
        for (i, w) in self.spec.weapons.iter().enumerate() {
            if self.weapon_lv[i] == 0 {
                continue;
            }
            let st = &self.weapon_stats[i];
            let gap = match w.kind {
                // Blades re-hit a foe on a short gap, about twice a second a blade.
                WeaponKind::Orbit => 0.5,
                _ => (st.cooldown * self.cd_scale()).max(0.1),
            };
            let n = if w.kind == WeaponKind::Pulse {
                1
            } else {
                st.count.max(1)
            };
            sum += st.damage * self.mods.damage * n as f32 / gap;
        }
        sum
    }

    /// The timed elite: the first at `director.elite_first`, then every `elite_every`.
    fn spawn_elite(&mut self) {
        if self.next_elite <= 0.0 || self.t < self.next_elite {
            return;
        }
        let Some(kind) = self
            .kinds
            .iter()
            .position(|k| k.elite && k.unlock <= self.t)
        else {
            return;
        };
        let half = self.spec.arena.half - 1.2;
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let r = self.spec.director.spawn_dist * 0.8;
        let pos = (
            (self.hx + a.sin() * r).clamp(-half, half),
            (self.hz + a.cos() * r).clamp(-half, half),
        );
        let hp_mul = 1.0 + self.spec.director.hp_per_min * self.t / 60.0;
        let base = self.kinds[kind].hp * hp_mul;
        // A fight, not a speed bump: scale with what the hero can dish out (several weapons
        // and levels made the warden fall in two seconds), within a cap so it stays fair.
        let d = &self.spec.director;
        let hp = (self.firepower() * d.elite_fight_secs * 0.5).clamp(base, base * d.elite_hp_cap);
        if !self.add_foe(kind, pos, hp) {
            return; // the pool is full: try again next step
        }
        self.next_elite += self.spec.director.elite_every.max(5.0);
        let msg = self.tr("A WARDEN APPROACHES!", "ばんにんがやってくる！");
        self.set_toast(msg, [255, 215, 90]);
        self.zoom_kick = self.zoom_kick.max(0.5);
        self.core.juice.shake = self.core.juice.shake.max(0.25);
        self.cue("elite", 2, 0.5);
    }

    /// A burst of `n` particles (scaled by the quality tier), thrown along (`dx`,`dz`) with a
    /// spread of `fan` and an upward `up`. Dropped when the pool is full.
    #[allow(clippy::too_many_arguments)]
    fn burst(
        &mut self,
        kind: u8,
        n: u32,
        at: [f32; 3],
        dir: [f32; 2],
        speed: f32,
        fan: f32,
        up: f32,
        life: f32,
    ) {
        if !self.spec.fx.particles {
            return;
        }
        let n = (n as f32 * [0.5, 1.0, 1.5][self.tier] * self.hint_particles).ceil() as u32;
        for _ in 0..n {
            if self.fx.len() >= self.fx_cap {
                return;
            }
            let a = self.fx_rng.range(0.0, std::f32::consts::TAU);
            let sp = speed * self.fx_rng.range(0.4, 1.0);
            self.fx.push(Fx {
                x: at[0],
                y: at[1],
                z: at[2],
                vx: dir[0] * speed * 0.7 + a.cos() * sp * fan,
                vy: up * self.fx_rng.range(0.5, 1.0),
                vz: dir[1] * speed * 0.7 + a.sin() * sp * fan,
                age: 0.0,
                life: life * self.fx_rng.range(0.7, 1.0),
                kind,
            });
        }
    }

    fn step_fx(&mut self, dt: f32) {
        self.zoom_kick = (self.zoom_kick - dt * 3.5).max(0.0);
        for i in (0..self.fx.len()).rev() {
            let p = &mut self.fx[i];
            p.age += dt;
            if p.age >= p.life {
                self.fx.swap_remove(i);
                continue;
            }
            let (drag, grav) = match p.kind {
                FX_SPARK | FX_HURT => (1.6, 12.0),
                FX_GLINT => (0.8, -2.2),
                _ => (2.6, 0.0),
            };
            let k = (1.0 - drag * dt).max(0.0);
            p.vx *= k;
            p.vz *= k;
            p.vy = p.vy * k - grav * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.z += p.vz * dt;
            if p.y < 0.04 {
                p.y = 0.04;
                p.vy = -p.vy * 0.25;
            }
        }
        for i in (0..self.dmg.len()).rev() {
            self.dmg[i].age += dt;
            if self.dmg[i].age >= DMG_LIFE {
                self.dmg.remove(i);
            }
        }
    }

    fn hurt(&mut self, dmg: f32) {
        if self.hurt_cd > 0.0 || dmg <= 0.0 {
            return;
        }
        if self.shield_t > 0.0 {
            // The barrier takes it: a flash, a short breather, no damage.
            self.hurt_cd = 0.25;
            let at = [self.hx, 0.7, self.hz];
            self.burst(FX_GLINT, 4, at, [0.0, 0.0], 3.0, 1.0, 1.5, 0.35);
            self.cue("shield", 1, 0.2);
            return;
        }
        self.hp -= dmg;
        self.hurt_cd = self.spec.hero.hurt_cd;
        self.hurt_flash = 1.0;
        self.core.juice.shake = self.core.juice.shake.max(0.35);
        self.zoom_kick = self.zoom_kick.max(0.7);
        let at = [self.hx, 0.7, self.hz];
        self.burst(FX_HURT, 8, at, [0.0, 0.0], 4.5, 1.0, 3.0, 0.45);
        self.cue("hurt", 1, 0.15);
    }

    fn move_foes(&mut self, dt: f32) {
        self.sync_aux();
        let n = self.foes.len();
        self.push_x.clear();
        self.push_z.clear();
        for i in 0..n {
            let (x, z) = (self.foes.x[i], self.foes.z[i]);
            let ri = self.kinds[self.foes.kind[i] as usize].radius;
            let (mut px, mut pz, mut seen) = (0.0_f32, 0.0_f32, 0_u32);
            let foes = &self.foes;
            let kinds = &self.kinds;
            self.grid.query(x, z, ri + MAX_FOE_R, |j| {
                if j == i || seen >= PUSH_NEIGHBOURS {
                    return;
                }
                let (dx, dz) = (x - foes.x[j], z - foes.z[j]);
                let rs = ri + kinds[foes.kind[j] as usize].radius;
                let d2 = dx * dx + dz * dz;
                if d2 < rs * rs {
                    seen += 1;
                    let d = d2.sqrt().max(1e-3);
                    let k = (rs - d) / d;
                    px += dx * k;
                    pz += dz * k;
                }
            });
            self.push_x.push(px);
            self.push_z.push(pz);
        }
        let rh = self.spec.hero.radius;
        let lim = self.spec.arena.half - 0.3;
        let mut contact = 0.0_f32;
        for i in 0..n {
            let k = self.kinds[self.foes.kind[i] as usize];
            self.foes.age[i] += dt;
            self.foes.flash[i] = (self.foes.flash[i] - dt).max(0.0);
            let (dx, dz) = (self.hx - self.foes.x[i], self.hz - self.foes.z[i]);
            let d = dx.hypot(dz).max(1e-3);
            // A little speed spread per foe keeps a pack from moving as one block.
            let jitter =
                0.9 + 0.2 * ((self.foes.uid[i].wrapping_mul(2_654_435_761) >> 24) as f32 / 255.0);
            let sp = k.speed * jitter;
            let (ux, uz) = (dx / d, dz / d);
            let (mut vx, mut vz) = (ux * sp, uz * sp);
            match k.ai {
                FoeAi::Chase => {
                    if k.weave > 0.0 {
                        // A zigzag: sideways sway across the line to the hero.
                        let sway =
                            (self.foes.age[i] * 3.4 + self.foes.uid[i] as f32).sin() * k.weave * sp;
                        vx += -uz * sway;
                        vz += ux * sway;
                    }
                }
                FoeAi::Shoot => {
                    // Keep the distance, circle a little, and throw an orb now and then.
                    let go = if d > k.hold {
                        1.0
                    } else if d < k.hold * 0.7 {
                        -0.8
                    } else {
                        0.0
                    };
                    let strafe = if self.foes.uid[i].is_multiple_of(2) {
                        0.4
                    } else {
                        -0.4
                    };
                    vx = ux * sp * go - uz * sp * strafe;
                    vz = uz * sp * go + ux * sp * strafe;
                    let aux = &mut self.aux[i];
                    aux.clock -= dt;
                    if aux.clock <= 0.0 && d < k.hold * 2.2 && self.orbs.len() < ORB_CAP {
                        aux.clock = k.every * (0.85 + 0.3 * self.rng.unit());
                        self.orbs.push(Orb {
                            x: self.foes.x[i],
                            z: self.foes.z[i],
                            vx: ux * k.shot_speed,
                            vz: uz * k.shot_speed,
                            life: ORB_LIFE,
                            dmg: k.shot_dmg,
                        });
                    }
                }
            }
            vx += self.push_x[i] * 6.0;
            vz += self.push_z[i] * 6.0;
            let x = (self.foes.x[i] + vx * dt).clamp(-lim, lim);
            let z = (self.foes.z[i] + vz * dt).clamp(-lim, lim);
            let rs = rh + k.radius;
            let (ndx, ndz) = (self.hx - x, self.hz - z);
            let nd = ndx.hypot(ndz).max(1e-3);
            if nd < rs * HOLD {
                // Held at the hero's edge instead of stacking on top.
                let keep = rs * HOLD;
                self.foes.x[i] = self.hx - ndx / nd * keep;
                self.foes.z[i] = self.hz - ndz / nd * keep;
            } else {
                self.foes.x[i] = x;
                self.foes.z[i] = z;
            }
            if nd < rs * 1.1 {
                contact = contact.max(k.dmg);
            }
        }
        if contact > 0.0 {
            self.hurt(contact);
        }
    }

    // ---- aiming: auto, tap-to-lock, tap-the-ground

    /// The locked foe's index now, without touching the lock.
    fn lock_foe(&self) -> Option<usize> {
        let uid = self.lock?;
        let ok = |i: usize| i < self.foes.len() && self.foes.uid[i] == uid && self.foes.hp[i] > 0.0;
        if ok(self.lock_idx) {
            return Some(self.lock_idx);
        }
        (0..self.foes.len()).find(|&i| ok(i))
    }

    /// Where a weapon with this range fires: `(yaw, distance)`. Manual and hybrid prefer the
    /// locked foe (in range), then the ground direction (manual) or the nearest foe, then
    /// the ground direction. A lock that died is dropped.
    fn pick_aim(&mut self, range: f32) -> Option<(f32, f32)> {
        if self.aim_mode != 0 {
            if self.lock.is_some() {
                match self.lock_foe() {
                    Some(j) => {
                        self.lock_idx = j;
                        let (dx, dz) = (self.foes.x[j] - self.hx, self.foes.z[j] - self.hz);
                        let d = dx.hypot(dz);
                        if d <= range {
                            return Some((dx.atan2(dz), d));
                        }
                    }
                    None => self.lock = None, // the target died: back to nearest
                }
            }
            if self.aim_mode == 1 {
                if let Some(a) = self.aim_dir {
                    return Some((a, f32::INFINITY));
                }
            }
        }
        if let Some((j, _)) = self
            .grid
            .nearest(&self.foes.x, &self.foes.z, self.hx, self.hz, range)
        {
            let (dx, dz) = (self.foes.x[j] - self.hx, self.foes.z[j] - self.hz);
            return Some((dx.atan2(dz), dx.hypot(dz)));
        }
        if self.aim_mode != 0 {
            if let Some(a) = self.aim_dir {
                return Some((a, f32::INFINITY));
            }
        }
        None
    }

    /// `auto`, `manual` or `hybrid`.
    pub fn aim_name(&self) -> &'static str {
        ["auto", "manual", "hybrid"][self.aim_mode.min(2) as usize]
    }

    /// Set the aim scheme (the pause card cycles it). Clears any lock.
    pub fn set_aim(&mut self, mode: &str) {
        self.aim_mode = aim_code(mode);
        self.lock = None;
        self.aim_dir = None;
    }

    /// The locked foe's uid, if the lock is still alive.
    pub fn locked(&self) -> Option<u32> {
        self.lock_foe().and(self.lock)
    }

    /// A tap at a canvas pixel: a foe near it is locked (tap it again to release), the hero
    /// clears the aim, bare ground aims that way. Auto mode ignores taps.
    fn tap_select(&mut self, x: f32, y: f32) {
        if self.aim_mode == 0 {
            return;
        }
        let (w, h) = self.size.get();
        let pick = self.spec.input.tap_px * hud_scale(w, h);
        let mut best: Option<(usize, f32)> = None;
        for i in 0..self.foes.len() {
            let Some((sx, sy)) = self.project([self.foes.x[i], 0.45, self.foes.z[i]], w, h) else {
                continue;
            };
            let d = (sx - x).hypot(sy - y);
            if d < pick && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        if let Some((i, _)) = best {
            let uid = self.foes.uid[i];
            if self.lock == Some(uid) {
                self.lock = None;
            } else {
                self.lock = Some(uid);
                self.lock_idx = i;
            }
            self.aim_dir = None;
            self.cue("pick", 2, 0.1);
        } else if let Some((gx, gz)) = self.ground_at(x, y) {
            let (dx, dz) = (gx - self.hx, gz - self.hz);
            self.lock = None;
            self.aim_dir = if dx.hypot(dz) < 0.8 {
                None
            } else {
                Some(dx.atan2(dz))
            };
            self.cue("pick", 2, 0.1);
        }
    }

    fn reap(&mut self) {
        self.sync_aux();
        for i in (0..self.foes.len()).rev() {
            if self.foes.hp[i] > 0.0 {
                continue;
            }
            let k = self.kinds[self.foes.kind[i] as usize];
            self.kills += 1;
            self.cue("kill", 4, 0.06);
            let (x, z) = (self.foes.x[i], self.foes.z[i]);
            let puff = if self.foes.uid[i].is_multiple_of(4) {
                FX_PUFF_ZOMBIE
            } else {
                FX_PUFF_GHOST
            };
            self.burst(puff, 6, [x, 0.5, z], [0.0, 0.0], 2.4, 1.0, 1.2, 0.55);
            self.burst(FX_SPARK, 4, [x, 0.5, z], [0.0, 0.0], 5.0, 1.0, 2.5, 0.35);
            self.core.juice.shake = self.core.juice.shake.max(0.07);
            self.zoom_kick = self.zoom_kick.max(0.12);
            if k.elite {
                self.burst(FX_BLAST, 12, [x, 0.6, z], [0.0, 0.0], 4.5, 1.0, 2.5, 0.8);
                self.core.juice.shake = self.core.juice.shake.max(0.4);
                self.zoom_kick = self.zoom_kick.max(0.6);
            }
            self.drop_loot(x, z, &k);
            // Gems that land beside a resting gem melt into it (a bigger gem worth the sum), more
            // eagerly as the pool fills: a crowd's kills leave a few fat gems, not a carpet.
            let cap = self.spec.caps.gems as usize;
            let reach = if self.gems.len() * 5 > cap * 3 {
                GEM_MERGE_FULL_R
            } else {
                GEM_MERGE_R
            };
            let near = self
                .gems
                .iter()
                .position(|g| !g.fly && (g.x - x).powi(2) + (g.z - z).powi(2) < reach * reach);
            if let Some(gi) = near {
                self.gems[gi].v += k.xp;
            } else if self.gems.len() < cap {
                self.gems.push(Gem {
                    x,
                    z,
                    v: k.xp,
                    fly: false,
                });
            } else {
                self.xp += k.xp * self.mods.xp; // pool full: the XP goes straight in
            }
            self.foes.remove(i);
            self.aux.swap_remove(i);
        }
    }

    fn collect(&mut self, dt: f32) {
        let pull = self.spec.hero.pickup_radius * self.mods.magnet;
        let take = self.spec.hero.radius + 0.3;
        let mut got = false;
        for gi in (0..self.gems.len()).rev() {
            let g = &mut self.gems[gi];
            let (dx, dz) = (self.hx - g.x, self.hz - g.z);
            let d = dx.hypot(dz);
            if d < pull {
                g.fly = true;
            }
            if g.fly {
                let sp = (8.0 + 14.0 * (1.0 - (d / (pull + 4.0)).min(1.0))) * dt;
                let k = sp.min(d) / d.max(1e-3);
                g.x += dx * k;
                g.z += dz * k;
            }
            if d < take {
                let (gx, gz, gv) = (g.x, g.z, g.v);
                self.xp += gv * self.mods.xp;
                self.gems.swap_remove(gi);
                got = true;
                self.burst(FX_GLINT, 5, [gx, 0.3, gz], [0.0, 0.0], 1.6, 1.0, 2.4, 0.45);
            }
        }
        if got {
            self.cue("gem", 1, 0.05);
        }
    }

    // ---- level-up cards

    fn open_cards(&mut self) {
        self.cards.clear();
        let lv = &self.spec.levels;
        let held = self.passive_lv.iter().filter(|l| **l > 0).count() as u32;
        let free_slot = self.held_weapons() < lv.max_weapons;
        let mut pool: Vec<(Card, u32)> = Vec::with_capacity(16);
        for (i, w) in self.spec.weapons.iter().enumerate() {
            let cur = self.weapon_lv[i];
            if cur > 0 && (cur as usize) < w.levels.len() + 1 {
                pool.push((Card::Weapon(i as u8), lv.weapon_weight.max(1)));
            } else if cur == 0 && free_slot {
                // A new weapon is a bigger change than a level: a little likelier to show.
                pool.push((Card::Weapon(i as u8), lv.weapon_weight.max(1) + 1));
            }
        }
        for (i, p) in self.spec.passives.iter().enumerate() {
            let cur = self.passive_lv[i];
            if cur < p.max_level && (cur > 0 || held < lv.max_passives) {
                pool.push((Card::Passive(i as u8), lv.passive_weight.max(1)));
            }
        }
        // The same level always offers the same cards for a seed (the daily challenge).
        let mut r = Rng::new(
            self.spec
                .director
                .seed
                .wrapping_mul(0x9e37_79b1)
                .wrapping_add(self.level.wrapping_mul(0x85eb_ca6b)),
        );
        for _ in 0..lv.choices {
            if pool.is_empty() {
                break;
            }
            let ws: Vec<u32> = pool.iter().map(|p| p.1).collect();
            let at = weighted_pick(&ws, r.next_u32());
            self.cards.push(pool.remove(at).0);
        }
        while (self.cards.len() as u32) < lv.choices {
            self.cards.push(Card::Heal); // nothing left to improve: a heal is never a dead end
        }
    }

    fn pick_card(&mut self, at: usize) {
        let Some(card) = self.cards.get(at).copied() else {
            return;
        };
        let from_swap = self.swap_in.take();
        match card {
            Card::Weapon(i) => {
                let w = &mut self.weapon_lv[i as usize];
                *w += 1;
                if *w == 1 {
                    self.weapon_cd[i as usize] = 0.0;
                }
            }
            Card::Swap(old) => {
                if let Some(new) = from_swap {
                    self.weapon_lv[old as usize] = 0;
                    self.weapon_lv[new as usize] = 1;
                    self.weapon_cd[new as usize] = 0.0;
                }
            }
            Card::Keep => self.xp += 4.0 * self.mods.xp,
            Card::Passive(i) => {
                self.passive_lv[i as usize] += 1;
                if self.spec.passives[i as usize].stat == PassiveStat::MaxHp {
                    self.hp += self.spec.passives[i as usize].per_level;
                }
            }
            Card::Heal => {
                self.hp += self.mods.hp_max * self.spec.levels.heal_frac;
            }
        }
        self.recompute();
        self.hp = self.hp.min(self.mods.hp_max);
        self.cards.clear();
        if from_swap.is_none() {
            self.pending_levels = self.pending_levels.saturating_sub(1);
        }
        self.cue("pick", 2, 0.1);
    }

    /// (kind label, name, description, accent colour) of a card, in the page language.
    /// A glyph from the bundled UI font for a card.
    fn card_glyph(&self, c: Card) -> &str {
        match c {
            Card::Weapon(i) | Card::Swap(i) => match self.spec.weapons[i as usize].glyph.as_str() {
                "" => "★",
                g => g,
            },
            Card::Keep => "×",
            Card::Heal => "♡",
            Card::Passive(i) => match self.spec.passives[i as usize].stat {
                PassiveStat::MaxHp => "♥",
                PassiveStat::Speed => "→",
                PassiveStat::Magnet => "◎",
                PassiveStat::Might => "▲",
                PassiveStat::Haste => "◆",
                PassiveStat::Growth => "☀",
            },
        }
    }

    /// Level pips for a card: levels owned and the most it can reach (0 max: no pips).
    fn card_pips(&self, c: Card) -> (usize, usize) {
        match c {
            Card::Weapon(i) | Card::Swap(i) => (
                self.weapon_lv[i as usize] as usize,
                self.spec.weapons[i as usize].levels.len() + 1,
            ),
            Card::Passive(i) => (
                self.passive_lv[i as usize] as usize,
                self.spec.passives[i as usize].max_level as usize,
            ),
            Card::Heal | Card::Keep => (0, 0),
        }
    }

    fn card_text(&self, c: Card) -> (String, String, String, [u8; 3]) {
        match c {
            Card::Weapon(i) => {
                let w = &self.spec.weapons[i as usize];
                let cur = self.weapon_lv[i as usize] as usize;
                let acc = w.color.map(|c| c.min(255) as u8);
                if cur == 0 {
                    return (
                        self.tr("NEW WEAPON", "あたらしいぶき").to_string(),
                        w.name.get(self.lang_ja).to_string(),
                        w.desc.get(self.lang_ja).to_string(),
                        acc,
                    );
                }
                let d = w
                    .levels
                    .get(cur - 1)
                    .map(|l| l.desc.get(self.lang_ja).to_string())
                    .unwrap_or_default();
                (
                    format!("{} LV {}", self.tr("WEAPON", "ぶき"), cur + 1),
                    w.name.get(self.lang_ja).to_string(),
                    d,
                    acc,
                )
            }
            Card::Swap(old) => {
                let o = &self.spec.weapons[old as usize];
                let new = self
                    .swap_in
                    .map(|n| self.spec.weapons[n as usize].name.get(self.lang_ja))
                    .unwrap_or_default();
                (
                    format!(
                        "{} LV {}",
                        self.tr("SWAP", "いれかえ"),
                        self.weapon_lv[old as usize]
                    ),
                    o.name.get(self.lang_ja).to_string(),
                    format!("{} {}", self.tr("Drop it for", "すてて"), new),
                    [255, 120, 110],
                )
            }
            Card::Keep => (
                self.tr("KEEP", "そのまま").to_string(),
                self.tr("Keep your weapons", "いまのぶきでいく").to_string(),
                self.tr("Take a little XP instead", "かわりにXPをすこし")
                    .to_string(),
                [150, 160, 255],
            ),
            Card::Passive(i) => {
                let p = &self.spec.passives[i as usize];
                let cur = self.passive_lv[i as usize];
                let tag = if cur == 0 {
                    self.tr("NEW", "あたらしい").to_string()
                } else {
                    format!("LV {}", cur + 1)
                };
                (
                    tag,
                    p.name.get(self.lang_ja).to_string(),
                    p.desc.get(self.lang_ja).to_string(),
                    [110, 220, 230],
                )
            }
            Card::Heal => (
                self.tr("EXTRA", "おまけ").to_string(),
                self.tr("Snack time", "おやつタイム").to_string(),
                format!(
                    "{} {}%",
                    self.tr("Heal", "かいふく"),
                    (self.spec.levels.heal_frac * 100.0).round()
                ),
                [255, 130, 170],
            ),
        }
    }

    // ---- input

    // ---- bot / tool read access

    pub fn phase_name(&self) -> &'static str {
        self.flow.phase.name()
    }
    pub fn hero(&self) -> (f32, f32) {
        (self.hx, self.hz)
    }
    pub fn foes(&self) -> &Foes {
        &self.foes
    }
    pub fn gem_positions(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.gems.iter().map(|g| (g.x, g.z))
    }
    pub fn level(&self) -> u32 {
        self.level
    }
    pub fn kills(&self) -> u32 {
        self.kills
    }
    pub fn hp(&self) -> f32 {
        self.hp
    }
    pub fn secs(&self) -> f32 {
        self.t
    }
    pub fn card_count(&self) -> usize {
        self.cards.len()
    }
    pub fn won(&self) -> bool {
        self.win
    }
    /// A card by index, for a bot: pick it as a tap would.
    pub fn choose(&mut self, at: usize) {
        self.pick_card(at);
    }
    /// What card `at` is: `weapon:<id>`, `passive:<id>` or `heal`.
    pub fn card_id(&self, at: usize) -> Option<String> {
        self.cards.get(at).map(|c| match c {
            Card::Weapon(i) => format!("weapon:{}", self.spec.weapons[*i as usize].id),
            Card::Passive(i) => format!("passive:{}", self.spec.passives[*i as usize].id),
            Card::Heal => "heal".into(),
            Card::Swap(i) => format!("swap:{}", self.spec.weapons[*i as usize].id),
            Card::Keep => "keep".into(),
        })
    }
    pub fn passive_level(&self, id: &str) -> u32 {
        self.spec
            .passives
            .iter()
            .position(|p| p.id == id)
            .map_or(0, |i| self.passive_lv[i])
    }
    pub fn weapon_level(&self, id: &str) -> u32 {
        self.spec
            .weapons
            .iter()
            .position(|w| w.id == id)
            .map_or(0, |i| self.weapon_lv[i])
    }
    pub fn hp_max(&self) -> f32 {
        self.mods.hp_max
    }
}

// ------------------------------------------------------------ layout -----

fn aim_code(name: &str) -> u8 {
    match name {
        "auto" => 0,
        "manual" => 1,
        _ => 2,
    }
}

fn hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PauseBtn {
    Resume,
    Aim,
    Retry,
    Title,
}

fn pause_rect(w: f32, s: f32) -> Quad {
    let side = 46.0 * s;
    Quad::new(w - side - 8.0 * s, 22.0 * s, side, side, [0, 0, 0, 0])
}

fn pause_buttons(w: f32, h: f32, s: f32) -> Vec<(PauseBtn, Quad)> {
    let bw = (w * 0.7).min(320.0 * s);
    let bh = (54.0 * s).min(h * 0.12);
    let gap = bh * 0.26;
    let x = (w - bw) * 0.5;
    let y0 = h * 0.30;
    [
        PauseBtn::Resume,
        PauseBtn::Aim,
        PauseBtn::Retry,
        PauseBtn::Title,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, b)| (b, Quad::new(x, y0 + i as f32 * (bh + gap), bw, bh, [0; 4])))
    .collect()
}

fn card_rects(w: f32, h: f32, s: f32, n: usize) -> Vec<Quad> {
    let n_f = n as f32;
    if h >= w {
        let cw = (w * 0.9).min(460.0 * s);
        let ch = (h * 0.15).clamp(78.0 * s, 132.0 * s);
        let gap = 12.0 * s;
        let total = n_f * ch + (n_f - 1.0) * gap;
        let top = (h * 0.5 - total * 0.5 + 24.0 * s).max(60.0 * s);
        (0..n)
            .map(|i| Quad::new((w - cw) * 0.5, top + i as f32 * (ch + gap), cw, ch, [0; 4]))
            .collect()
    } else {
        let gap = 16.0 * s;
        let cw = ((w - gap * (n_f + 1.0)) / n_f).min(300.0 * s);
        let ch = (h * 0.5).min(230.0 * s);
        let total = n_f * cw + (n_f - 1.0) * gap;
        let left = (w - total) * 0.5;
        let top = h * 0.5 - ch * 0.5 + 18.0 * s;
        (0..n)
            .map(|i| Quad::new(left + i as f32 * (cw + gap), top, cw, ch, [0; 4]))
            .collect()
    }
}

/// Sounds: CC0 retro synth effects (OpenGameArt), one Kenney confirmation chime
/// and a CC0 music loop. Every file has a ledger entry.
fn default_audio() -> Value {
    let sfx = "assets/audio/oga-retro-synth-sfx/";
    let cue =
        |f: &str, v: f64, r: f64| json!({"file": format!("{sfx}{f}"), "volume": v, "rate": r});
    json!({
        "enabled": true,
        "master": 0.8,
        "sfx_volume": 1.0,
        "bgm_volume": 0.5,
        "bgm": {"file": "assets/audio/oga-flowerbed-fields/flowerbed_fields.mp3", "volume": 0.6, "fade_secs": 0.5},
        "sfx": {
            "shoot": cue("shot_01.wav", 0.16, 1.0),
            "hit": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.45, "rate": 1.0},
            "kill": {"file": "assets/audio/oga-wet-squish/impactsplat03.wav", "volume": 0.3, "rate": 1.15},
            "hurt": cue("retro_die_01.wav", 0.6, 1.0),
            "gem": cue("retro_coin_01.wav", 0.25, 1.3),
            "levelup": cue("power_up_02.wav", 0.7, 1.0),
            "pick": {"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": 0.7, "rate": 1.0},
            "win": cue("power_up_04.wav", 0.8, 1.0),
            "pickup": cue("retro_coin_01.wav", 0.35, 1.0),
            "chest": cue("power_up_04.wav", 0.6, 1.25),
            "boom": cue("retro_explosion_02.wav", 0.5, 1.0),
            "elite": cue("retro_explosion_02.wav", 0.6, 0.55),
            "zap": cue("shot_01.wav", 0.2, 2.0),
            "pulse": cue("shot_01.wav", 0.3, 0.55),
            "slash": {"file": "assets/audio/kenney-impact-sounds/impactGeneric_light_001.wav", "volume": 0.3, "rate": 1.6},
            "shield": cue("power_up_02.wav", 0.5, 1.5),
            "gameover": cue("retro_die_03.wav", 0.8, 1.0)
        }
    })
}

// ----------------------------------------------------------- picture -----

/// The hero's glow colour.
const HERO_GLOW: [u32; 3] = [255, 214, 120];
/// Kenney Mini Characters (CC0), one per skin id. Unknown skins use the first.
const HERO_GLBS: [(&str, &str); 5] = [
    (
        "classic",
        "assets/models/kenney/mini-characters/character-female-a.glb",
    ),
    (
        "mint",
        "assets/models/kenney/mini-characters/character-male-c.glb",
    ),
    (
        "ember",
        "assets/models/kenney/mini-characters/character-male-a.glb",
    ),
    (
        "ghost",
        "assets/models/kenney/mini-characters/character-female-d.glb",
    ),
    (
        "gold",
        "assets/models/kenney/mini-characters/character-male-e.glb",
    ),
];
/// Kenney Graveyard Kit (CC0): the two foes.
const GHOST_GLB: &str = "assets/models/kenney/graveyard-kit/character-ghost.glb";
const ZOMBIE_GLB: &str = "assets/models/kenney/graveyard-kit/character-zombie.glb";
/// The foe model pools, in `Kind::model` order: ghost, zombie, skeleton, vampire, keeper.
const FOE_GLBS: [&str; POOLS] = [
    GHOST_GLB,
    ZOMBIE_GLB,
    "assets/models/kenney/graveyard-kit/character-skeleton.glb",
    "assets/models/kenney/graveyard-kit/character-vampire.glb",
    "assets/models/kenney/graveyard-kit/character-keeper.glb",
];
/// Share of the foe cap (percent) each pool can draw at once, plus 4.
const POOL_SHARE: [usize; POOLS] = [85, 40, 25, 30, 15];
/// Albedo tints of the models before a foe row's own tint.
/// (The ghost is white in the kit and burnt out under the moon without one.)
/// Which motion each foe pool uses (ghost, zombie, skeleton, vampire, keeper).
const POOL_MOTION: [Motion; POOLS] = [
    Motion::Float,
    Motion::Shamble,
    Motion::Hop,
    Motion::Glide,
    Motion::Stomp,
];
const POOL_TINT: [[u32; 3]; POOLS] = [
    [150, 150, 225],
    [205, 225, 190],
    [200, 205, 235],
    [215, 190, 215],
    [215, 200, 175],
];
/// Kenney Mini Dungeon (CC0): the elite's treasure chest and the orbiting blade.
const CHEST_GLB: &str = "assets/models/kenney/mini-dungeon/chest.glb";
const BLADE_GLB: &str = "assets/models/kenney/mini-dungeon/weapon-sword.glb";
/// Kenney Platformer Kit jewel (CC0): the XP gem.
/// Kenney Blaster Kit (CC0): the hero's blaster. The model's muzzle is at -Z.
const BLASTER_GLB: &str = "assets/models/kenney/blaster-kit/blaster-a.glb";
/// Where the blaster sits in front of the hero (metres along the aim, and up) and how
/// big it is drawn. The muzzle is `GUN_FWD + 0.37 * GUN_SCALE` ahead of the hero.
const GUN_FWD: f32 = 0.86;
const GUN_UP: f32 = 0.9;
const GUN_SCALE: f32 = 1.3;
/// Height of the muzzle above the floor (m).
/// Segments in the lock-on ring.
const LOCK_SEG: usize = 16;
/// Segments in a floor ring (pulse, blast, shield, the blades' track).
const RING_SEG: usize = 28;
const MUZZLE_Y: f32 = 0.62;
const MUZZLE: f32 = GUN_FWD + 0.37 * GUN_SCALE;
/// How long the hero keeps facing the target after a shot.
const AIM_HOLD: f32 = 0.5;
const GEM_GLB: &str = "assets/models/kenney/platformer-kit/jewel.glb";
/// Arena dressing from the Graveyard Kit inside the walls: (file, scale, how many).
const DECOR: [(&str, f32, u32); 6] = [
    ("gravestone-round", 1.5, 4),
    ("gravestone-cross", 1.5, 3),
    ("gravestone-broken", 1.5, 3),
    ("pumpkin-carved", 1.4, 3),
    ("lantern-candle", 1.6, 3),
    ("cross-wood", 1.4, 2),
];
/// Pines ringed outside the wall, so the arena has a dark treeline round it.
/// Seen from the high camera the pines read as flat pale boxes, so the ring is off.
const TREES: [(&str, u32); 0] = [];
/// Kenney Mini Arena (CC0): the stone boundary wall (1 m long, x) and the corner column.
const WALL_GLB: &str = "assets/models/kenney/mini-arena/wall.glb";
const COLUMN_GLB: &str = "assets/models/kenney/mini-arena/column.glb";
/// The hand-drawn paver texture (original, CC0) and how many metres one tile covers.
/// The Mini Arena stone is sandy; a cool tint puts it under the moon.
const STONE_TINT: [u32; 3] = [135, 150, 205];
const FLOOR_TEX: &str = "assets/textures/survivor_arena/floor.png";
const FLOOR_TILE_M: f32 = 12.0;
const GRAVEYARD: &str = "assets/models/kenney/graveyard-kit/";
/// Every Kenney character is about 0.8 tall; this puts a foe of radius r at ~2.2 r tall x 1.4.
const FOE_MODEL_K: f32 = 5.2;
/// Albedo tints: the ghost is white in the kit and burnt out under the moon without one.
const HERO_MODEL_SCALE: f32 = 2.2;
const BLADE_SCALE: f32 = 1.5;
const CHEST_SCALE: f32 = 1.5;

fn hero_glb(skin: &str) -> &'static str {
    HERO_GLBS
        .iter()
        .find(|(id, _)| *id == skin)
        .map_or(HERO_GLBS[0].1, |(_, p)| p)
}

fn decor_path(name: &str) -> String {
    format!("{GRAVEYARD}{name}.glb")
}

fn props_has_model(p: &crate::WorldProp) -> bool {
    p.gltf.is_some()
}

impl SurvivorArena {
    // ---- state for the page

    fn card_values(&self) -> Vec<Value> {
        self.cards
            .iter()
            .map(|c| {
                let (tag, name, desc, _) = self.card_text(*c);
                json!({"tag": tag, "name": name, "desc": desc})
            })
            .collect()
    }

    fn result_value(&self) -> Value {
        json!({
            "key": format!("{}:{}", self.spec.id, self.spec.director.seed),
            "title": self.spec.title,
            "enabled": true,
            "top_n": 5,
            "columns": ["time", "kills", "level", "score"],
            "sort": "score",
            "run": {
                "score": self.core.stats.score,
                "time": (self.t * 10.0).round() / 10.0,
                "kills": self.kills,
                "level": self.level,
                "win": u32::from(self.win),
            },
            "can_revive": false,
            "new_best": self.new_best,
        })
    }

    pub fn state_value(&self) -> Value {
        json!({
            "phase": self.flow.phase.name(),
            "score": self.core.stats.score,
            "best": self.flow.best_score,
            "hp": self.hp.max(0.0).ceil(),
            "hp_max": self.mods.hp_max,
            "level": self.level,
            "xp": self.xp,
            "xp_need": self.xp_need(),
            "kills": self.kills,
            "secs": self.t,
            "secs_total": self.spec.run.secs,
            "foes": self.foes.len(),
            "foes_cap": self.foes.cap(),
            "gems": self.gems.len(),
            "shots": self.shots.len(),
            "aim": self.aim_name(),
            "locked": self.locked().is_some(),
            "cards": self.card_values(),
            "weapons": self.weapon_values(),
            "max_weapons": self.spec.levels.max_weapons,
            "pickups": self.pickups.len(),
            "chests": self.pickups.iter().filter(|p| p.item == CHEST).count(),
            "orbs": self.orbs.len(),
            "shield": self.shield_t,
            "frenzy": self.frenzy_t,
            "synergies": self.syn_mask.count_ones(),
            "elites": (0..self.foes.len()).filter(|&i| self.kinds[self.foes.kind[i] as usize].elite).count(),
            "toast": if self.toast_t > 0.0 { self.toast.as_str() } else { "" },
            "win": self.win,
            "death": self.core.stats.death,
            "death_text": self.death_text,
            "revives": 0,
            "result": if self.flow.phase == FlowPhase::Result { self.result_value() } else { Value::Null },
            "pos": [self.hx, self.hz],
        })
    }

    fn weapon_values(&self) -> Value {
        Value::Array(
            self.spec
                .weapons
                .iter()
                .enumerate()
                .filter(|(i, _)| self.weapon_lv[*i] > 0)
                .map(|(i, w)| json!({"id": w.id, "lv": self.weapon_lv[i]}))
                .collect(),
        )
    }

    pub fn state_json(&self) -> String {
        self.state_value().to_string()
    }

    pub fn world_value(&self) -> Value {
        // Via a string so f32 fields print as 0.3, not 0.30000001192092896.
        let game: Value =
            serde_json::from_str(&serde_json::to_string(&self.spec).unwrap_or_default())
                .unwrap_or_default();
        json!({"version": 1, "genre": GAME_ID, "game": game})
    }

    pub fn dump_json(&self) -> String {
        let body = self.doc.to_json().unwrap_or_else(|_| "{}".into());
        let sim = self.state_json();
        match body.strip_prefix('{') {
            Some(rest) => format!("{{\n  \"sim\": {sim},{rest}"),
            None => body,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(json).map_err(|e| format!("survivor dump: {e}"))?;
        let spec = match value.get("game") {
            Some(g) => SurvivorSpec::from_value(g)?,
            None => SurvivorSpec::default(),
        };
        let mut run = Self::new(spec);
        if let Some(best) = value.get("best").and_then(|v| v.as_u64()) {
            run.flow.best_score = best as u32;
        }
        if let Some(q) = value.get("quality").and_then(|v| v.as_str()) {
            let q = q.trim();
            if !q.is_empty() {
                run.picture_quality = Some(q.to_string());
                run.apply_quality();
            }
        }
        run.build_doc();
        Ok(run)
    }

    pub fn document(&self) -> &WorldDoc {
        &self.doc
    }

    // ---- HUD
}

fn list_lv(ja: bool, level: u32) -> String {
    if ja {
        format!("Lv {level}")
    } else {
        format!("LV {level}")
    }
}

// ------------------------------------------------------------ the host ----

impl Kit for SurvivorArena {
    fn core(&self) -> &Core {
        &self.core
    }
    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }
    fn flow(&self) -> &GameFlow {
        &self.flow
    }
    fn flow_mut(&mut self) -> &mut GameFlow {
        &mut self.flow
    }
    fn substeps(&self, _dt: f32) -> u32 {
        1
    }
    fn step(&mut self, dt: f32) {
        self.step_sim(dt);
    }
    fn attract(&mut self, dt: f32) {
        self.anim_t += dt;
    }
    fn follow(&mut self, _dt: f32) {}
    fn blocks_spark(&self, _x: f32, _y: f32, _z: f32) -> bool {
        false
    }
    fn rebuild(&mut self) {
        self.build_doc();
    }
}

impl crate::kit_host::KitHost for SurvivorArena {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn name(&self) -> &'static str {
        "survivor arena"
    }
    fn phase_name(&self) -> &'static str {
        SurvivorArena::phase_name(self)
    }
    fn playing(&self) -> bool {
        self.flow.phase == FlowPhase::Play
    }
    fn document(&self) -> &WorldDoc {
        SurvivorArena::document(self)
    }
    fn dump_json(&self) -> String {
        SurvivorArena::dump_json(self)
    }
    fn state_json(&self) -> String {
        SurvivorArena::state_json(self)
    }
    fn confirm(&mut self) {
        SurvivorArena::confirm(self)
    }
    fn show_title(&mut self) {
        SurvivorArena::show_title(self)
    }
    fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        SurvivorArena::pointer(self, x, y, width, height)
    }
    fn on_key(&mut self, _name: &str, _down: bool) -> bool {
        false
    }
    fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList {
        SurvivorArena::hud(self, width, height, menu)
    }
    fn walk(&mut self, lx: f32, lz: f32, _jump: bool) {
        SurvivorArena::walk(self, lx, lz);
    }
    fn touch(&mut self, ev: PointerEvent, w: u32, h: u32, _t: f32) {
        self.touch_ev(ev, w, h);
    }
    fn uses_swipe(&self) -> bool {
        true
    }
    fn set_lang(&mut self, lang: &str) {
        SurvivorArena::set_lang(self, lang);
    }
    fn supports_quality_tiers(&self) -> bool {
        true
    }
    fn quality_hints(&mut self, atmo: f32, particles: f32) {
        self.hint_atmo = atmo;
        self.hint_particles = particles.clamp(0.0, 1.0);
        self.apply_hints();
    }
    fn pause_run(&mut self) {
        self.pause();
    }
    fn resume_run(&mut self) {
        self.resume_from_page();
    }
}

impl SurvivorArena {
    /// One fixed step (what the session calls through [`KitHost::tick`]).
    pub fn tick(&mut self, dt: f32) {
        tick_frame(self, dt);
    }
}

#[cfg(test)]
mod tests;

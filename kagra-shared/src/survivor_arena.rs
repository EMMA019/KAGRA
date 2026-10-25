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
use crate::game_core::swarm::{Budget, Foes, Grid, Rng};
use crate::game_core::{tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::survivor_content::{
    default_enemies, default_foes, default_synergies, default_weapons, DropSpec, FoeAi, FoeSpec,
    PickupKind, SynergySpec,
};
use crate::world_doc::{WorldCamera, WorldDoc};

mod hud_extra;
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

// ---------------------------------------------------------------- spec ----

/// A name in both page languages.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Loc {
    pub en: String,
    pub ja: String,
}

impl Loc {
    pub(crate) fn new(en: &str, ja: &str) -> Self {
        Self {
            en: en.into(),
            ja: ja.into(),
        }
    }

    pub(crate) fn get(&self, ja: bool) -> &str {
        if ja && !self.ja.is_empty() {
            &self.ja
        } else {
            &self.en
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArenaSpec {
    /// Half size (m) of the square arena.
    pub half: f32,
    pub floor: [u32; 3],
    pub wall: [u32; 3],
}

impl Default for ArenaSpec {
    fn default() -> Self {
        Self {
            half: 13.0,
            floor: [34, 46, 66],
            wall: [84, 70, 128],
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StartPassive {
    pub id: String,
    pub level: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HeroSpec {
    /// Metres per second at full speed.
    pub speed: f32,
    pub max_hp: f32,
    pub radius: f32,
    /// Gems inside this distance (m) fly to the hero.
    pub pickup_radius: f32,
    /// Seconds of protection after a hit.
    pub hurt_cd: f32,
    /// A colour id from the HOP-BEAT skin table (`classic`, `mint`, `gold`, ...).
    pub skin: String,
    pub start_weapon: String,
    /// Passives the hero starts with (the unlockable heroes use this).
    pub start_passives: Vec<StartPassive>,
}

impl Default for HeroSpec {
    fn default() -> Self {
        Self {
            speed: 4.6,
            max_hp: 100.0,
            radius: 0.4,
            pickup_radius: 1.7,
            hurt_cd: 0.5,
            skin: "classic".into(),
            start_weapon: "bolt".into(),
            start_passives: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InputSpec {
    /// `follow` (the hero goes to the finger) or `stick` (a floating stick).
    pub mode: String,
    /// Follow: the hero aims this far (m) above the finger, so the thumb does not cover it.
    pub thumb_offset_m: f32,
    /// Follow: closer than this (m) the hero stands still.
    pub dead_zone_m: f32,
    /// Follow: at this distance (m) the hero runs at full speed.
    pub full_speed_m: f32,
    /// Stick: radius in pixels at 400 px of short screen side.
    pub stick_px: f32,
    /// Who picks the target: `auto` (nearest foe), `manual` (tap a foe to lock it, tap the
    /// ground to aim that way) or `hybrid` (a locked foe first, else nearest).
    pub aim: String,
    /// Manual and hybrid: a tap picks a foe within this many pixels (at 400 px short side).
    pub tap_px: f32,
}

impl Default for InputSpec {
    fn default() -> Self {
        Self {
            mode: "follow".into(),
            thumb_offset_m: 2.0,
            dead_zone_m: 0.3,
            full_speed_m: 2.2,
            stick_px: 70.0,
            aim: "hybrid".into(),
            tap_px: 44.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunSpec {
    /// Surviving this long (s) wins the run.
    pub secs: f32,
    pub score_per_kill: u32,
    pub score_per_level: u32,
    pub score_per_sec: u32,
    pub win_bonus: u32,
}

impl Default for RunSpec {
    fn default() -> Self {
        Self {
            secs: 120.0,
            score_per_kill: 10,
            score_per_level: 50,
            score_per_sec: 2,
            win_bonus: 500,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LevelSpec {
    /// XP to leave level `n`: `xp_base + xp_step * (n - 1) + xp_quad * (n - 1)^2`.
    pub xp_base: f32,
    pub xp_step: f32,
    pub xp_quad: f32,
    /// Cards offered per level-up (2..=4).
    pub choices: u32,
    /// How many different passives the hero can hold.
    pub max_passives: u32,
    /// How many different weapons the hero can hold (1..=6).
    pub max_weapons: u32,
    /// Draw weights of a weapon card and a passive card.
    pub weapon_weight: u32,
    pub passive_weight: u32,
    /// The fallback card heals this share of max HP.
    pub heal_frac: f32,
}

impl Default for LevelSpec {
    fn default() -> Self {
        Self {
            xp_base: 3.0,
            xp_step: 2.0,
            xp_quad: 0.3,
            choices: 3,
            max_passives: 4,
            max_weapons: 3,
            weapon_weight: 3,
            passive_weight: 2,
            heal_frac: 0.3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DirectorSpec {
    /// Seeds the spawns and the card draws. The daily challenge sets this.
    pub seed: u32,
    /// Foes born per second at the start and at `ramp_secs`.
    pub start_rate: f32,
    pub end_rate: f32,
    pub ramp_secs: f32,
    /// The rate follows `t^curve` (above 1 it stays gentle, then climbs).
    pub curve: f32,
    /// Enemy HP grows by this share per minute.
    pub hp_per_min: f32,
    /// Foes appear this far (m) from the hero, off screen.
    pub spawn_dist: f32,
    /// The first elite (a foe row with `elite: true`) walks in at this second (0: no elites),
    /// and another one every `elite_every` seconds.
    pub elite_first: f32,
    pub elite_every: f32,
}

impl Default for DirectorSpec {
    fn default() -> Self {
        Self {
            seed: 1,
            start_rate: 1.5,
            end_rate: 8.5,
            ramp_secs: 100.0,
            curve: 2.0,
            hp_per_min: 0.35,
            spawn_dist: 10.5,
            elite_first: 45.0,
            elite_every: 40.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CameraSpec {
    pub height: f32,
    /// Metres the eye sits behind the look point (toward +Z).
    pub back: f32,
    /// Metres of arena width wanted across the screen at the hero.
    pub view_width_m: f32,
    pub fov_min: f32,
    pub fov_max: f32,
    /// The look point sits this far (m) toward the camera from the hero: the hero
    /// stands in the upper half of the screen, clear of the thumb.
    pub hero_up_m: f32,
}

impl Default for CameraSpec {
    fn default() -> Self {
        Self {
            height: 16.0,
            back: 11.0,
            view_width_m: 9.6,
            fov_min: 36.0,
            fov_max: 66.0,
            hero_up_m: 1.4,
        }
    }
}

/// Game feel and effects, all switchable from the JSON. `cap` is the live-particle cap for
/// the low, mid and high quality tiers; the costly extras are only there on `high`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FxSpec {
    /// Hit sparks, death puffs, gem glints, muzzle flash.
    pub particles: bool,
    /// Camera shake scale (0 = none, 1 = as tuned, 2 = double).
    pub shake: f32,
    /// Floating damage numbers.
    pub damage_numbers: bool,
    /// Metres a foe is shoved by a hit.
    pub knockback: f32,
    /// Camera zoom-in pulse scale on hurt, level-up and big kills.
    pub zoom_pulse: f32,
    /// Streaks behind shots.
    pub trails: bool,
    /// On the `high` picture tier only: wider soft shadows, stronger bloom and SSAO
    /// (TAA, cascades and PCSS come from the tier itself).
    pub high_post: bool,
    pub cap: [u32; 3],
}

impl Default for FxSpec {
    fn default() -> Self {
        Self {
            particles: true,
            shake: 1.0,
            damage_numbers: true,
            knockback: 0.12,
            zoom_pulse: 1.0,
            trails: true,
            high_post: true,
            cap: [48, 160, 480],
        }
    }
}

/// Pool sizes. `foes` is the live-enemy cap for the low, mid and high quality tiers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CapSpec {
    pub foes: [u32; 3],
    pub gems: u32,
    pub shots: u32,
}

impl Default for CapSpec {
    fn default() -> Self {
        Self {
            foes: [120, 220, 320],
            gems: 140,
            shots: 48,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponKind {
    /// Auto-aimed shots at the nearest enemy.
    Projectile,
    /// A fan of short-lived pellets with a little scatter.
    Spread,
    /// `count` blades circle the hero (`area` is the radius, `speed` rad/s) and cut what they touch.
    Orbit,
    /// Homing wisps that burst for `damage` to every foe within `area`.
    Missile,
    /// Lightning: hits the nearest foe, then jumps `count - 1` times (`area` m per jump).
    Chain,
    /// A ring around the hero: `damage` to every foe within `area`, and a shove.
    Pulse,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeaponBase {
    pub damage: f32,
    pub cooldown: f32,
    pub speed: f32,
    pub range: f32,
    pub count: u32,
    pub spread_deg: f32,
    pub pierce: u32,
    pub size: f32,
    /// Orbit radius, blast radius, jump length or ring radius (m), by weapon kind.
    pub area: f32,
}

impl Default for WeaponBase {
    fn default() -> Self {
        Self {
            damage: 4.0,
            cooldown: 0.5,
            speed: 13.0,
            range: 9.0,
            count: 1,
            spread_deg: 12.0,
            pierce: 0,
            size: 0.3,
            area: 0.0,
        }
    }
}

/// What one weapon level adds to the one before.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeaponLevel {
    pub desc: Loc,
    pub damage: f32,
    pub cooldown: f32,
    pub count: i32,
    pub pierce: i32,
    pub speed: f32,
    pub range: f32,
    pub area: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponSpec {
    pub id: String,
    pub name: Loc,
    /// What the weapon is, on its card when it is new.
    #[serde(default)]
    pub desc: Loc,
    /// One glyph from the UI font (HUD chip and card badge).
    #[serde(default = "default_weapon_glyph")]
    pub glyph: String,
    #[serde(default = "default_weapon_color")]
    pub color: [u32; 3],
    pub kind: WeaponKind,
    pub base: WeaponBase,
    /// `levels[0]` is the step from level 1 to 2, and so on.
    pub levels: Vec<WeaponLevel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassiveStat {
    MaxHp,
    Speed,
    Magnet,
    Might,
    Haste,
    Growth,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassiveSpec {
    pub id: String,
    pub name: Loc,
    pub desc: Loc,
    pub stat: PassiveStat,
    /// Added per level: HP points for `max_hp`, a fraction for the others.
    pub per_level: f32,
    pub max_level: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SurvivorSpec {
    pub id: String,
    pub title: String,
    pub arena: ArenaSpec,
    pub hero: HeroSpec,
    pub input: InputSpec,
    pub run: RunSpec,
    pub levels: LevelSpec,
    pub director: DirectorSpec,
    pub camera: CameraSpec,
    pub caps: CapSpec,
    pub fx: FxSpec,
    pub enemies: EnemySpec,
    /// Survivor extras per enemy type (ai, armor, drop table).
    pub foes: Vec<FoeSpec>,
    pub drops: DropSpec,
    pub weapons: Vec<WeaponSpec>,
    pub passives: Vec<PassiveSpec>,
    /// Weapon x passive bonuses that switch on at a passive level.
    pub synergies: Vec<SynergySpec>,
    /// Sound cues for the page (`game.audio`: file names under the bundle). Passed through.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<Value>,
}

fn default_weapon_glyph() -> String {
    "★".into()
}

fn default_weapon_color() -> [u32; 3] {
    [255, 170, 80]
}

fn passive(
    id: &str,
    en: &str,
    ja: &str,
    d_en: &str,
    d_ja: &str,
    stat: PassiveStat,
    per: f32,
) -> PassiveSpec {
    PassiveSpec {
        id: id.into(),
        name: Loc::new(en, ja),
        desc: Loc::new(d_en, d_ja),
        stat,
        per_level: per,
        max_level: 5,
    }
}

pub(crate) fn lv(en: &str, ja: &str) -> WeaponLevel {
    WeaponLevel {
        desc: Loc::new(en, ja),
        ..WeaponLevel::default()
    }
}

impl Default for SurvivorSpec {
    /// The shipped game: one hero, six weapons, five foe kinds, drops, six passives.
    fn default() -> Self {
        Self {
            id: GAME_ID.into(),
            title: "Moonlit Swarm".into(),
            arena: ArenaSpec::default(),
            hero: HeroSpec::default(),
            input: InputSpec::default(),
            run: RunSpec::default(),
            levels: LevelSpec::default(),
            director: DirectorSpec::default(),
            camera: CameraSpec::default(),
            caps: CapSpec::default(),
            fx: FxSpec::default(),
            enemies: EnemySpec {
                types: default_enemies(),
                ..EnemySpec::default()
            },
            foes: default_foes(),
            drops: DropSpec::default(),
            weapons: default_weapons(),
            passives: vec![
                passive(
                    "vitality",
                    "Vitality",
                    "げんき",
                    "Max HP +20",
                    "さいだいHP +20",
                    PassiveStat::MaxHp,
                    20.0,
                ),
                passive(
                    "swift",
                    "Swift paws",
                    "はやあし",
                    "Move speed +8%",
                    "いどうそく +8%",
                    PassiveStat::Speed,
                    0.08,
                ),
                passive(
                    "magnet",
                    "Magnet",
                    "じしゃく",
                    "Gem pull range +30%",
                    "ジェムをよせる +30%",
                    PassiveStat::Magnet,
                    0.30,
                ),
                passive(
                    "might",
                    "Might",
                    "ちからもち",
                    "Damage +12%",
                    "ダメージ +12%",
                    PassiveStat::Might,
                    0.12,
                ),
                passive(
                    "haste",
                    "Haste",
                    "せっかち",
                    "Throws 7% faster",
                    "こうげき +7%はやく",
                    PassiveStat::Haste,
                    0.07,
                ),
                passive(
                    "growth",
                    "Growth",
                    "せいちょう",
                    "XP +12%",
                    "けいけんち +12%",
                    PassiveStat::Growth,
                    0.12,
                ),
            ],
            synergies: default_synergies(),
            audio: Some(default_audio()),
        }
    }
}

fn range(who: &str, name: &str, v: f32, lo: f32, hi: f32) -> Result<(), String> {
    if v.is_finite() && (lo..=hi).contains(&v) {
        Ok(())
    } else {
        Err(format!("{who}: {name} must be {lo}..={hi}"))
    }
}

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 16
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl SurvivorSpec {
    pub fn from_value(v: &Value) -> Result<Self, String> {
        let spec: Self =
            serde_json::from_value(v.clone()).map_err(|e| format!("survivor spec: {e}"))?;
        spec.validate()?;
        Ok(spec)
    }

    /// The drop tables, foe rows, weapon slots and synergies.
    fn validate_content(&self, w: &str) -> Result<(), String> {
        if !(1..=4).contains(&self.levels.max_weapons) {
            return Err(format!("{w}: levels.max_weapons must be 1..=4"));
        }
        range(
            w,
            "director.elite_first",
            self.director.elite_first,
            0.0,
            1800.0,
        )?;
        range(
            w,
            "director.elite_every",
            self.director.elite_every,
            0.0,
            1800.0,
        )?;
        for (i, wp) in self.weapons.iter().enumerate() {
            let n = format!("weapons[{i}]");
            range(w, &format!("{n}.base.area"), wp.base.area, 0.0, 12.0)?;
            if wp.color.iter().any(|c| *c > 255) {
                return Err(format!("{w}: {n}.color is 0..=255 each"));
            }
            if wp.glyph.chars().count() > 2 {
                return Err(format!("{w}: {n}.glyph is one or two characters"));
            }
        }
        if self.foes.len() > 8 {
            return Err(format!("{w}: foes holds at most 8 rows"));
        }
        let mut elites = 0;
        for (i, f) in self.foes.iter().enumerate() {
            let n = format!("foes[{i}]");
            // A row for a type this world does not have is simply unused.
            if self.foes[..i].iter().any(|o| o.id == f.id) {
                return Err(format!("{w}: {n}.id is used twice"));
            }
            range(w, &format!("{n}.scale"), f.scale, 0.3, 3.0)?;
            range(w, &format!("{n}.weave"), f.weave, 0.0, 1.0)?;
            range(w, &format!("{n}.armor"), f.armor, 0.0, 50.0)?;
            range(w, &format!("{n}.knock_resist"), f.knock_resist, 0.0, 1.0)?;
            range(w, &format!("{n}.hold_m"), f.hold_m, 1.0, 20.0)?;
            range(w, &format!("{n}.shoot_every"), f.shoot_every, 0.4, 20.0)?;
            range(w, &format!("{n}.shot_dmg"), f.shot_dmg, 0.0, 1000.0)?;
            range(w, &format!("{n}.shot_speed"), f.shot_speed, 1.0, 20.0)?;
            range(w, &format!("{n}.drop_mul"), f.drop_mul, 0.0, 20.0)?;
            if f.tint.iter().any(|c| *c > 255) {
                return Err(format!("{w}: {n}.tint is 0..=255 each"));
            }
            if !f.table.is_empty() && !self.drops.tables.iter().any(|t| t.id == f.table) {
                return Err(format!("{w}: {n}.table names no drops.tables entry"));
            }
            if f.elite {
                elites += 1;
            }
        }
        if elites > 1 {
            return Err(format!("{w}: at most one foe row may be the elite"));
        }
        let d = &self.drops;
        if d.cap > 24 || d.items.len() > 16 || d.tables.len() > 6 {
            return Err(format!(
                "{w}: drops.cap 0..=24, items at most 16, tables at most 6"
            ));
        }
        range(w, "drops.life", d.life, 2.0, 120.0)?;
        range(w, "drops.blink", d.blink, 0.0, 10.0)?;
        range(w, "drops.pull_m", d.pull_m, 0.0, 12.0)?;
        if d.pity_kills > 1000 {
            return Err(format!("{w}: drops.pity_kills is 0..=1000"));
        }
        for (i, it) in d.items.iter().enumerate() {
            let n = format!("drops.items[{i}]");
            if !id_ok(&it.id) || d.items[..i].iter().any(|o| o.id == it.id) {
                return Err(format!("{w}: {n}.id is 1..=16 of a-z 0-9 _ and unique"));
            }
            if it.rarity > 2 || it.color.iter().any(|c| *c > 255) || it.glyph.chars().count() > 2 {
                return Err(format!(
                    "{w}: {n}: rarity 0..=2, color 0..=255, glyph 1-2 chars"
                ));
            }
            range(w, &format!("{n}.amount"), it.amount, 0.0, 10_000.0)?;
            range(w, &format!("{n}.secs"), it.secs, 0.0, 120.0)?;
            range(w, &format!("{n}.area"), it.area, 0.0, 30.0)?;
            if it.weight > 1000 {
                return Err(format!("{w}: {n}.weight is 0..=1000"));
            }
        }
        for (i, t) in d.tables.iter().enumerate() {
            let n = format!("drops.tables[{i}]");
            if !id_ok(&t.id) || d.tables[..i].iter().any(|o| o.id == t.id) {
                return Err(format!("{w}: {n}.id is 1..=16 of a-z 0-9 _ and unique"));
            }
            range(w, &format!("{n}.chance"), t.chance, 0.0, 1.0)?;
            if !(1..=6).contains(&t.rolls) || t.rarity_min > 2 {
                return Err(format!("{w}: {n}: rolls 1..=6, rarity_min 0..=2"));
            }
            if !t.guarantee.is_empty() && !d.items.iter().any(|o| o.id == t.guarantee) {
                return Err(format!("{w}: {n}.guarantee names no item"));
            }
            if d.enabled
                && !d
                    .items
                    .iter()
                    .any(|o| o.rarity >= t.rarity_min && o.weight > 0)
                && !t.chest
            {
                return Err(format!("{w}: {n}: no item reaches its rarity_min"));
            }
        }
        for (i, sy) in self.synergies.iter().enumerate() {
            let n = format!("synergies[{i}]");
            // A synergy for a weapon or passive this world lacks is simply unused.
            if self.synergies.len() > 32 {
                return Err(format!("{w}: synergies holds at most 32 entries"));
            }
            range(w, &format!("{n}.damage"), sy.damage, -0.9, 3.0)?;
            range(w, &format!("{n}.cooldown"), sy.cooldown, -0.6, 2.0)?;
            range(w, &format!("{n}.area"), sy.area, -0.9, 3.0)?;
            if !(-2..=4).contains(&sy.count) || !(0..=2).contains(&sy.pierce) || sy.min_level == 0 {
                return Err(format!(
                    "{w}: {n}: count -2..=4, pierce 0..=2, min_level >= 1"
                ));
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        let w = "survivor spec";
        range(w, "arena.half", self.arena.half, 6.0, 40.0)?;
        range(w, "hero.speed", self.hero.speed, 1.0, 12.0)?;
        range(w, "hero.max_hp", self.hero.max_hp, 1.0, 10_000.0)?;
        range(w, "hero.radius", self.hero.radius, 0.1, 1.0)?;
        range(w, "hero.pickup_radius", self.hero.pickup_radius, 0.3, 10.0)?;
        range(w, "hero.hurt_cd", self.hero.hurt_cd, 0.0, 5.0)?;
        if !matches!(self.input.mode.as_str(), "follow" | "stick") {
            return Err(format!("{w}: input.mode is follow|stick"));
        }
        range(
            w,
            "input.thumb_offset_m",
            self.input.thumb_offset_m,
            0.0,
            6.0,
        )?;
        range(w, "input.dead_zone_m", self.input.dead_zone_m, 0.0, 2.0)?;
        range(w, "input.full_speed_m", self.input.full_speed_m, 0.5, 8.0)?;
        if self.input.full_speed_m <= self.input.dead_zone_m {
            return Err(format!("{w}: input.full_speed_m must exceed dead_zone_m"));
        }
        range(w, "input.stick_px", self.input.stick_px, 20.0, 200.0)?;
        if !matches!(self.input.aim.as_str(), "auto" | "manual" | "hybrid") {
            return Err(format!("{w}: input.aim is auto|manual|hybrid"));
        }
        range(w, "input.tap_px", self.input.tap_px, 10.0, 200.0)?;
        range(w, "run.secs", self.run.secs, 10.0, 1800.0)?;
        range(w, "levels.xp_base", self.levels.xp_base, 1.0, 10_000.0)?;
        range(w, "levels.xp_step", self.levels.xp_step, 0.0, 1000.0)?;
        range(w, "levels.xp_quad", self.levels.xp_quad, 0.0, 100.0)?;
        if !(2..=4).contains(&self.levels.choices) {
            return Err(format!("{w}: levels.choices must be 2..=4"));
        }
        if self.levels.max_passives > 8 {
            return Err(format!("{w}: levels.max_passives must be 0..=8"));
        }
        range(w, "levels.heal_frac", self.levels.heal_frac, 0.0, 1.0)?;
        range(
            w,
            "director.start_rate",
            self.director.start_rate,
            0.0,
            60.0,
        )?;
        range(w, "director.end_rate", self.director.end_rate, 0.0, 60.0)?;
        range(
            w,
            "director.ramp_secs",
            self.director.ramp_secs,
            1.0,
            1800.0,
        )?;
        range(w, "director.curve", self.director.curve, 0.3, 4.0)?;
        range(w, "director.hp_per_min", self.director.hp_per_min, 0.0, 5.0)?;
        range(
            w,
            "director.spawn_dist",
            self.director.spawn_dist,
            4.0,
            30.0,
        )?;
        range(w, "camera.height", self.camera.height, 4.0, 60.0)?;
        range(w, "camera.back", self.camera.back, 0.0, 60.0)?;
        range(
            w,
            "camera.view_width_m",
            self.camera.view_width_m,
            4.0,
            40.0,
        )?;
        range(w, "camera.fov_min", self.camera.fov_min, 20.0, 80.0)?;
        range(w, "camera.fov_max", self.camera.fov_max, 20.0, 90.0)?;
        range(w, "camera.hero_up_m", self.camera.hero_up_m, -4.0, 8.0)?;
        if self.camera.fov_min > self.camera.fov_max {
            return Err(format!("{w}: camera.fov_min must not exceed fov_max"));
        }
        if self.caps.foes.iter().any(|c| !(8..=600).contains(c))
            || !(0..=400).contains(&self.caps.gems)
            || !(4..=200).contains(&self.caps.shots)
        {
            return Err(format!(
                "{w}: caps.foes 8..=600, gems 0..=400, shots 4..=200"
            ));
        }
        if self.caps.foes[0] > self.caps.foes[1] || self.caps.foes[1] > self.caps.foes[2] {
            return Err(format!("{w}: caps.foes must rise from low to high"));
        }
        let f = &self.fx;
        range(w, "fx.shake", f.shake, 0.0, 2.0)?;
        range(w, "fx.knockback", f.knockback, 0.0, 0.6)?;
        range(w, "fx.zoom_pulse", f.zoom_pulse, 0.0, 2.0)?;
        if f.cap.iter().any(|c| *c > 600) || f.cap[0] > f.cap[1] || f.cap[1] > f.cap[2] {
            return Err(format!("{w}: fx.cap is 0..=600 and rises from low to high"));
        }
        self.enemies.validate(
            w,
            &["ghoul", "ghost", "zombie", "skeleton", "vampire", "keeper"],
        )?;
        if self.enemies.types.is_empty() {
            return Err(format!("{w}: enemies.types needs at least one kind"));
        }
        if self.enemies.types.iter().any(|t| t.swarm.is_none()) {
            return Err(format!("{w}: every enemy type needs a `swarm` block"));
        }
        if !self
            .enemies
            .types
            .iter()
            .any(|t| t.swarm.as_ref().is_some_and(|s| s.unlock_secs <= 0.0))
        {
            return Err(format!("{w}: one enemy type must be there from second 0"));
        }
        if self.weapons.is_empty() || self.weapons.len() > 8 {
            return Err(format!("{w}: weapons holds 1..=8 entries"));
        }
        for (i, wp) in self.weapons.iter().enumerate() {
            let n = format!("weapons[{i}]");
            if !id_ok(&wp.id) || self.weapons[..i].iter().any(|o| o.id == wp.id) {
                return Err(format!("{w}: {n}.id is 1..=16 of a-z 0-9 _ and unique"));
            }
            range(
                w,
                &format!("{n}.base.damage"),
                wp.base.damage,
                0.1,
                10_000.0,
            )?;
            range(
                w,
                &format!("{n}.base.cooldown"),
                wp.base.cooldown,
                0.1,
                30.0,
            )?;
            range(w, &format!("{n}.base.speed"), wp.base.speed, 1.0, 60.0)?;
            range(w, &format!("{n}.base.range"), wp.base.range, 1.0, 40.0)?;
            range(
                w,
                &format!("{n}.base.spread_deg"),
                wp.base.spread_deg,
                0.0,
                90.0,
            )?;
            range(w, &format!("{n}.base.size"), wp.base.size, 0.05, 2.0)?;
            if !(1..=8).contains(&wp.base.count) || wp.base.pierce > 3 {
                return Err(format!("{w}: {n}.base.count 1..=8, pierce 0..=3"));
            }
            if wp.levels.len() > 9 {
                return Err(format!("{w}: {n}.levels holds at most 9 steps"));
            }
            let mut eff = WeaponStats::from_base(&wp.base);
            for (k, l) in wp.levels.iter().enumerate() {
                eff.add(l);
                if !(1..=8).contains(&eff.count) || !(0..=3).contains(&eff.pierce) {
                    return Err(format!(
                        "{w}: {n}.levels[{k}] takes count outside 1..=8 or pierce outside 0..=3"
                    ));
                }
                if eff.cooldown < 0.1 || eff.damage <= 0.0 || eff.speed < 1.0 {
                    return Err(format!("{w}: {n}.levels[{k}] drives a stat out of range"));
                }
            }
        }
        if self.passives.len() > 12 {
            return Err(format!("{w}: passives holds at most 12 entries"));
        }
        for (i, p) in self.passives.iter().enumerate() {
            let n = format!("passives[{i}]");
            if !id_ok(&p.id)
                || self.passives[..i].iter().any(|o| o.id == p.id)
                || self.weapons.iter().any(|o| o.id == p.id)
            {
                return Err(format!("{w}: {n}.id is 1..=16 of a-z 0-9 _ and unique"));
            }
            if !(1..=9).contains(&p.max_level) {
                return Err(format!("{w}: {n}.max_level must be 1..=9"));
            }
            range(w, &format!("{n}.per_level"), p.per_level, 0.0, 1000.0)?;
            if p.stat == PassiveStat::Haste && p.per_level * p.max_level as f32 > 0.8 {
                return Err(format!(
                    "{w}: {n}: haste may cut at most 80% of the cooldown"
                ));
            }
        }
        if !self.weapons.iter().any(|x| x.id == self.hero.start_weapon) {
            return Err(format!("{w}: hero.start_weapon names no weapon"));
        }
        self.validate_content(w)?;
        if self.hero.start_passives.len() > 4 {
            return Err(format!("{w}: hero.start_passives holds at most 4"));
        }
        for sp in &self.hero.start_passives {
            let Some(p) = self.passives.iter().find(|p| p.id == sp.id) else {
                return Err(format!(
                    "{w}: hero.start_passives names no passive {:?}",
                    sp.id
                ));
            };
            if sp.level == 0 || sp.level > p.max_level {
                return Err(format!("{w}: hero.start_passives level is 1..=max_level"));
            }
        }
        Ok(())
    }
}

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
        let aspect = w / h.max(1.0);
        let (eye, target, fov) = self.camera(aspect);
        let f = norm3([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
        let r = norm3(cross(f, [0.0, 1.0, 0.0]));
        let u = cross(r, f);
        let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cz = dot(v, f);
        if cz < 0.1 {
            return None;
        }
        let t = (fov.to_radians() * 0.5).tan();
        let nx = dot(v, r) / (cz * t * aspect.max(0.2));
        let ny = dot(v, u) / (cz * t);
        Some(((nx * 0.5 + 0.5) * w, (0.5 - ny * 0.5) * h))
    }

    /// The ground point under a canvas pixel, or `None` for a sky ray.
    fn ground_at(&self, px: f32, py: f32) -> Option<(f32, f32)> {
        let (w, h) = self.size.get();
        let aspect = w / h.max(1.0);
        let (eye, target, fov) = self.camera(aspect);
        let f = norm3([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
        let right = norm3(cross(f, [0.0, 1.0, 0.0]));
        let up = cross(right, f);
        let ty = (fov.to_radians() * 0.5).tan();
        let nx = (2.0 * px / w - 1.0) * ty * aspect;
        let ny = (1.0 - 2.0 * py / h) * ty;
        let d = [
            f[0] + right[0] * nx + up[0] * ny,
            f[1] + right[1] * nx + up[1] * ny,
            f[2] + right[2] * nx + up[2] * ny,
        ];
        if d[1] > -0.02 {
            return None;
        }
        let t = -eye[1] / d[1];
        Some((eye[0] + d[0] * t, eye[2] + d[2] * t))
    }

    fn move_hero(&mut self, dt: f32) {
        let sp = self.spec.hero.speed * self.mods.speed;
        let mut want = (0.0_f32, 0.0_f32);
        if let Some((fx, fy)) = self.finger {
            if self.spec.input.mode == "stick" {
                let (w, h) = self.size.get();
                let s = (w.min(h) / 400.0).clamp(0.9, 2.6);
                let rad = self.spec.input.stick_px * s;
                let (dx, dy) = ((fx - self.origin.0) / rad, (fy - self.origin.1) / rad);
                let len = dx.hypot(dy);
                if len > 0.12 {
                    let k = len.min(1.0) / len;
                    want = (dx * k * sp, dy * k * sp);
                }
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
        let hp = self.kinds[kind].hp * hp_mul;
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
        let n = (n as f32 * [0.5, 1.0, 1.5][self.tier]).ceil() as u32;
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
            if self.gems.len() < self.spec.caps.gems as usize {
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

    fn touch_ev(&mut self, ev: PointerEvent, w: u32, h: u32) {
        self.size.set((w.max(1) as f32, h.max(1) as f32));
        let live = self.flow.phase == FlowPhase::Play && self.cards.is_empty();
        match ev.phase {
            PointerPhase::Begin => {
                if live && self.finger_id.is_none() {
                    self.finger_id = Some(ev.id);
                    self.finger = Some((ev.x, ev.y));
                    self.origin = (ev.x, ev.y);
                    self.tap = Some((ev.x, ev.y, 0.0));
                } else if live && self.finger_id != Some(ev.id) {
                    // The other thumb: a tap picks the target.
                    self.tap_select(ev.x, ev.y);
                }
            }
            PointerPhase::Move => {
                if self.finger_id == Some(ev.id) && !self.mute_finger {
                    self.finger = Some((ev.x, ev.y));
                }
            }
            PointerPhase::End | PointerPhase::Cancel if self.finger_id == Some(ev.id) => {
                // A quick, short touch of the lone finger is a tap too.
                if let (PointerPhase::End, Some((tx, ty, age))) = (ev.phase, self.tap.take()) {
                    let (w, h) = self.size.get();
                    if live && age < 0.25 && (ev.x - tx).hypot(ev.y - ty) < 12.0 * hud_scale(w, h) {
                        self.tap_select(ev.x, ev.y);
                    }
                }
                self.tap = None;
                self.finger_id = None;
                self.finger = None;
                self.mute_finger = false;
            }
            #[allow(unreachable_patterns)]
            _ => {}
        }
    }

    /// A tap on a card, a pause button or a pause-card button. True when used.
    pub fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool {
        let (w, h) = (width.max(1) as f32, height.max(1) as f32);
        let s = hud_scale(w, h);
        match self.flow.phase {
            FlowPhase::Pause => {
                for (b, r) in pause_buttons(w, h, s) {
                    if r.contains(x, y) {
                        match b {
                            PauseBtn::Resume => self.flow.resume(),
                            PauseBtn::Aim => {
                                let next = (self.aim_mode + 1) % 3;
                                self.set_aim(["auto", "manual", "hybrid"][next as usize]);
                            }
                            PauseBtn::Retry => self.start_run(),
                            PauseBtn::Title => self.show_title(),
                        }
                        self.mute_after_tap();
                        return true;
                    }
                }
                true
            }
            FlowPhase::Play if !self.cards.is_empty() => {
                for (i, r) in card_rects(w, h, s, self.cards.len()).iter().enumerate() {
                    if r.contains(x, y) {
                        self.pick_card(i);
                        self.mute_after_tap();
                        break;
                    }
                }
                true
            }
            FlowPhase::Play => {
                if pause_rect(w, s).contains(x, y) {
                    self.pause();
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// The finger that made this tap must lift before it steers again.
    fn mute_after_tap(&mut self) {
        self.finger = None;
        self.tap = None;
        if self.finger_id.is_some() {
            self.mute_finger = true;
        }
    }

    /// Keyboard or pad steering: `lz` is up the screen.
    pub fn walk(&mut self, lx: f32, lz: f32) {
        self.key_dir = (lx, -lz);
    }

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

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm3(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-6);
    [a[0] / l, a[1] / l, a[2] / l]
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
    /// Build the persistent prop list once: scenery, then the pools (hero, foes,
    /// gems, shots). After this a frame only edits fields in place.
    fn init_doc(&mut self) {
        let half = self.spec.arena.half;
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: half + 4.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([12, 18, 40, 255]),
            hemi_sky: Some([0.30, 0.38, 0.62]),
            hemi_ground: Some([0.20, 0.20, 0.30]),
            env_map: Some("preset:studio".into()),
            shadow_softness: Some(1.0),
            bloom: Some(0.42),
            bloom_threshold: Some(0.95),
            vignette: Some(0.35),
            lut: Some("punch".into()),
            lut_strength: Some(0.3),
            fxaa: Some(true),
            exposure: Some(1.0),
            ibl: Some(0.3),
            tonemap: Some(true),
            cull: Some(true),
            prop_lod: Some(80.0),
            fog_start: Some(40.0),
            fog_end: Some(90.0),
            ..WorldDoc::default()
        };
        doc.quality = self.picture_quality.clone();
        if self.tier == 2 && self.spec.fx.high_post {
            doc.shadow_softness = Some(2.6);
            doc.ssao = Some(10.0);
            doc.bloom = doc.bloom.map(|b| b * 1.3);
        }
        // One tiny buried prop per model keeps the renderer's glTF table constant.
        let hero_path = hero_glb(&self.spec.hero.skin);
        let mut wanted: Vec<String> = vec![
            hero_path.to_string(),
            BLASTER_GLB.to_string(),
            GHOST_GLB.to_string(),
            ZOMBIE_GLB.to_string(),
            GEM_GLB.to_string(),
        ];
        wanted.extend(FOE_GLBS.iter().map(|m| m.to_string()));
        wanted.push(CHEST_GLB.to_string());
        wanted.push(BLADE_GLB.to_string());
        wanted.push(WALL_GLB.to_string());
        wanted.push(COLUMN_GLB.to_string());
        wanted.extend(DECOR.iter().map(|(n, _, _)| decor_path(n)));
        wanted.extend(TREES.iter().map(|(n, _)| decor_path(n)));
        let models: Vec<String> = wanted.into_iter().filter(|m| glb_ready(m)).collect();
        for (i, m) in models.iter().enumerate() {
            doc.props.push(glb_prop(
                &format!("anchor-{i}"),
                m,
                "anchor",
                [0.0, -60.0, 60.0],
                [1e-4; 3],
                [0, 0, 0],
                0.0,
                None,
            ));
        }
        let has = |p: &str| models.iter().any(|m| m == p);
        let glb_hero = has(hero_path);
        self.slots.glb_gun = glb_hero && has(BLASTER_GLB);
        let glb_foe = has(GHOST_GLB);
        self.slots.glb_chest = has(CHEST_GLB);
        self.slots.glb_blade = has(BLADE_GLB);
        self.slots.glb_gem = has(GEM_GLB);
        let a = &self.spec.arena;
        let side = half * 2.0;
        let mut floor = prop(
            "floor",
            "box",
            "floor",
            [0.0, -0.12, 0.0],
            [side + 8.0, 0.24, side + 8.0],
            [255, 255, 255],
            0.0,
            0.9,
            0.0,
        );
        floor.texture = Some(FLOOR_TEX.into());
        let reps = (side + 8.0) / FLOOR_TILE_M;
        floor.uv_scale = Some([reps, reps]);
        doc.props.push(floor);
        // Graveyard dressing: a fixed scatter in the outer ring so the floor shows
        // the hero moving and the middle stays clear. Primitive pebbles when the
        // models are missing.
        let mut r = Rng::new(77);
        let mut n_decor = 0;
        for (name, scale, count) in DECOR {
            let path = decor_path(name);
            for _ in 0..count {
                let (mut x, mut z) = (0.0f32, 0.0f32);
                while x.abs().max(z.abs()) < half * 0.45 {
                    x = r.range(-half + 1.0, half - 1.0);
                    z = r.range(-half + 1.0, half - 1.0);
                }
                let yaw = r.range(0.0, std::f32::consts::TAU);
                let mut p = if has(&path) {
                    glb_prop(
                        &format!("decor-{n_decor}"),
                        &path,
                        "decor",
                        [x, 0.0, z],
                        [scale; 3],
                        [165, 170, 210],
                        0.0,
                        None,
                    )
                } else {
                    prop(
                        &format!("decor-{n_decor}"),
                        "sphere",
                        "decor",
                        [x, 0.12, z],
                        [0.4, 0.25, 0.4],
                        [70, 80, 100],
                        0.0,
                        0.9,
                        0.0,
                    )
                };
                p.yaw = yaw;
                doc.props.push(p);
                n_decor += 1;
            }
        }
        // The boundary: 2 m stone wall pieces just outside the playable square, a column
        // on each corner. Plain boxes when the models are missing.
        let (wall_ok, col_ok) = (has(WALL_GLB), has(COLUMN_GLB));
        let pieces = (side / 2.0).ceil() as i32 + 1;
        let off = half + 0.45;
        let mut n_wall = 0;
        for side_i in 0..4 {
            for k in 0..pieces {
                let along = -half - 0.9 + k as f32 * 2.0;
                let (x, z, yaw) = match side_i {
                    0 => (along, -off, 0.0),
                    1 => (along, off, std::f32::consts::PI),
                    2 => (-off, along, std::f32::consts::FRAC_PI_2),
                    _ => (off, along, -std::f32::consts::FRAC_PI_2),
                };
                if wall_ok {
                    let mut p = glb_prop(
                        &format!("wall-{n_wall}"),
                        WALL_GLB,
                        "wall",
                        [x, 0.0, z],
                        [2.0, 1.1, 1.5],
                        STONE_TINT,
                        0.0,
                        None,
                    );
                    p.yaw = yaw;
                    doc.props.push(p);
                } else if k == 0 {
                    let (pos, sc) = if side_i < 2 {
                        (
                            [0.0, 0.4, off * if side_i == 0 { -1.0 } else { 1.0 }],
                            [side + 1.0, 0.8, 0.5],
                        )
                    } else {
                        (
                            [off * if side_i == 2 { -1.0 } else { 1.0 }, 0.4, 0.0],
                            [0.5, 0.8, side + 1.0],
                        )
                    };
                    doc.props.push(prop(
                        &format!("wall-{n_wall}"),
                        "box",
                        "wall",
                        pos,
                        sc,
                        a.wall,
                        0.45,
                        0.6,
                        0.0,
                    ));
                }
                n_wall += 1;
            }
        }
        if col_ok {
            for (i, (sx, sz)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
                .into_iter()
                .enumerate()
            {
                doc.props.push(glb_prop(
                    &format!("column-{i}"),
                    COLUMN_GLB,
                    "wall",
                    [sx * (half + 0.6), 0.0, sz * (half + 0.6)],
                    [2.4, 2.4, 2.4],
                    STONE_TINT,
                    0.0,
                    None,
                ));
            }
        }
        // Pines ringed outside the wall.
        let mut n_tree = 0;
        for (name, count) in TREES {
            let path = decor_path(name);
            for _ in 0..count {
                if !has(&path) {
                    continue;
                }
                let a_ = r.range(0.0, std::f32::consts::TAU);
                let ring = half + r.range(2.6, 4.2);
                let (c, s_) = (a_.cos(), a_.sin());
                // Push the angle out to the square ring so trees hug the walls.
                let m = c.abs().max(s_.abs());
                let sc = r.range(1.5, 2.3);
                let mut p = glb_prop(
                    &format!("tree-{n_tree}"),
                    &path,
                    "decor",
                    [c / m * ring, 0.0, s_ / m * ring],
                    [sc; 3],
                    [110, 150, 135],
                    0.0,
                    None,
                );
                p.yaw = r.range(0.0, std::f32::consts::TAU);
                doc.props.push(p);
                n_tree += 1;
            }
        }
        self.slots.shadow = doc.props.len();
        doc.props.push(prop(
            "hero-shadow",
            "sphere",
            "shadow",
            [0.0, 0.02, 0.0],
            [0.95, 0.02, 0.95],
            [6, 8, 16],
            0.0,
            1.0,
            0.0,
        ));
        self.slots.hero = doc.props.len();
        if glb_hero {
            doc.props.push(glb_prop(
                "hero-body",
                hero_path,
                "hero",
                [0.0, 0.0, 0.0],
                [HERO_MODEL_SCALE; 3],
                [255, 255, 255],
                0.12,
                Some(lin(HERO_GLOW)),
            ));
        } else {
            doc.props.push(prop(
                "hero-body",
                "sphere",
                "hero",
                [0.0, 0.4, 0.0],
                [0.8, 0.8, 0.8],
                HERO_GLOW,
                0.35,
                0.5,
                0.0,
            ));
        }
        self.slots.gun = doc.props.len();
        if self.slots.glb_gun {
            doc.props.push(glb_prop(
                "hero-gun",
                BLASTER_GLB,
                "gun",
                [0.0, -50.0, 0.0],
                [GUN_SCALE; 3],
                [255, 255, 255],
                0.1,
                Some(lin(HERO_GLOW)),
            ));
        }
        self.slots.glb_foe = glb_foe;
        let cap = self.foes.cap();
        for m in 0..POOLS {
            // A missing model has no pool: its foes draw as ghosts (or spheres).
            self.slots.pool_cap[m] = if m == 0 {
                if glb_foe {
                    cap * POOL_SHARE[0] / 100 + 4
                } else {
                    cap
                }
            } else if glb_foe && has(FOE_GLBS[m]) {
                cap * POOL_SHARE[m] / 100 + 4
            } else {
                0
            };
        }
        let foe_prop = |id: String, m: usize| {
            let mut p = if glb_foe {
                glb_prop(
                    &id,
                    FOE_GLBS[m],
                    "foe",
                    [0.0, -50.0, 0.0],
                    [1.0; 3],
                    POOL_TINT[m],
                    0.06,
                    Some([0.2, 0.8, 0.3]),
                )
            } else {
                prop(
                    &id,
                    "sphere",
                    "foe",
                    [0.0, -50.0, 0.0],
                    [0.8; 3],
                    [120, 240, 150],
                    0.15,
                    0.5,
                    0.0,
                )
            };
            p.enabled = false;
            p
        };
        for m in 0..POOLS {
            self.slots.pool0[m] = doc.props.len();
            for i in 0..self.slots.pool_cap[m] {
                doc.props.push(foe_prop(format!("foe{m}-{i}"), m));
            }
        }
        self.slots.gem0 = doc.props.len();
        for i in 0..self.spec.caps.gems as usize {
            let mut p = if self.slots.glb_gem {
                glb_prop(
                    &format!("gem-{i}"),
                    GEM_GLB,
                    "gem",
                    [0.0, -50.0, 0.0],
                    [1.3; 3],
                    [255, 255, 255],
                    0.9,
                    Some([0.35, 0.9, 1.0]),
                )
            } else {
                prop(
                    &format!("gem-{i}"),
                    "sphere",
                    "gem",
                    [0.0, -50.0, 0.0],
                    [0.28; 3],
                    [90, 230, 255],
                    1.6,
                    0.3,
                    0.0,
                )
            };
            p.enabled = false;
            doc.props.push(p);
        }
        self.slots.shot0 = doc.props.len();
        for i in 0..self.spec.caps.shots as usize {
            let mut p = prop(
                &format!("shot-{i}"),
                "sphere",
                "shot",
                [0.0, -50.0, 0.0],
                [0.24, 0.24, 0.6],
                [255, 150, 60],
                1.2,
                0.4,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        // Enemy orbs.
        self.slots.orb0 = doc.props.len();
        for i in 0..ORB_CAP {
            let mut p = prop(
                &format!("orb-{i}"),
                "sphere",
                "orb",
                [0.0, -50.0, 0.0],
                [ORB_R * 2.0; 3],
                [200, 120, 255],
                2.2,
                0.3,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        // The orbiting blades.
        self.slots.blade0 = doc.props.len();
        for i in 0..BLADE_CAP {
            let mut p = if self.slots.glb_blade {
                glb_prop(
                    &format!("blade-{i}"),
                    BLADE_GLB,
                    "blade",
                    [0.0, -50.0, 0.0],
                    [BLADE_SCALE; 3],
                    [255, 255, 255],
                    0.9,
                    Some([0.55, 0.85, 1.0]),
                )
            } else {
                prop(
                    &format!("blade-{i}"),
                    "box",
                    "blade",
                    [0.0, -50.0, 0.0],
                    [0.12, 0.08, 0.9],
                    [150, 220, 255],
                    1.2,
                    0.3,
                    0.0,
                )
            };
            p.enabled = false;
            doc.props.push(p);
        }
        // Pickups: a glowing orb and a soft ground glow each, plus a few chest models.
        let ncap = self.spec.drops.cap as usize;
        self.slots.pick0 = doc.props.len();
        for i in 0..ncap {
            let mut p = prop(
                &format!("pick-{i}"),
                "sphere",
                "pickup",
                [0.0, -50.0, 0.0],
                [0.36; 3],
                [255, 255, 255],
                1.6,
                0.3,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        self.slots.glow0 = doc.props.len();
        for i in 0..ncap {
            let mut p = prop(
                &format!("glow-{i}"),
                "sphere",
                "glow",
                [0.0, -50.0, 0.0],
                [0.9, 0.03, 0.9],
                [255, 255, 255],
                1.0,
                1.0,
                0.0,
            );
            p.enabled = false;
            doc.props.push(p);
        }
        self.slots.chest0 = doc.props.len();
        for i in 0..CHEST_CAP {
            let mut p = if self.slots.glb_chest {
                glb_prop(
                    &format!("chest-{i}"),
                    CHEST_GLB,
                    "chest",
                    [0.0, -50.0, 0.0],
                    [CHEST_SCALE; 3],
                    [255, 235, 190],
                    0.5,
                    Some([1.0, 0.8, 0.3]),
                )
            } else {
                prop(
                    &format!("chest-{i}"),
                    "box",
                    "chest",
                    [0.0, -50.0, 0.0],
                    [0.7, 0.5, 0.5],
                    [255, 200, 80],
                    0.6,
                    0.5,
                    0.0,
                )
            };
            p.enabled = false;
            doc.props.push(p);
        }
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "arena".into(),
            position: [0.0, 20.0, 11.0],
            target: [0.0, 0.0, 0.0],
            fov: 50.0,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "key",
            [0.0, 9.0, 3.0],
            [0.0; 3],
            [0.78, 0.86, 1.0],
            1.1,
            34.0,
            false,
        ));
        // A warm lantern that follows the hero: it lifts the hero off the cold floor.
        doc.lights.push(light(
            1,
            "lantern",
            [0.0, 2.4, 0.0],
            [0.0; 3],
            [1.0, 0.78, 0.5],
            1.3,
            7.0,
            false,
        ));
        // Effect clouds (one per kind and third of life) and shot streaks: fixed lists whose
        // vectors are refilled in place each frame.
        for (k, (rgb, size, alpha)) in FX_LOOK.iter().enumerate() {
            for b in 0..3 {
                doc.points.push(crate::strokes::PointCloud {
                    id: format!("fx-{k}-{b}"),
                    positions: Vec::with_capacity(self.fx_cap),
                    color: [rgb[0], rgb[1], rgb[2], alpha[b]],
                    size: size[b],
                });
            }
        }
        for i in 0..self.spec.caps.shots {
            doc.lines.push(crate::strokes::FatLine {
                id: format!("trail-{i}"),
                points: Vec::with_capacity(2),
                color: [150, 235, 255, 150],
                width: 3.0,
            });
        }
        // The lock-on ring and the ground-aim line (empty unless used).
        let line = |id: String, cap: usize, color: [u8; 4], width: f32| crate::strokes::FatLine {
            id,
            points: Vec::with_capacity(cap),
            color,
            width,
        };
        self.slots.ln_ring = doc.lines.len();
        doc.lines.push(line(
            "lock-ring".into(),
            LOCK_SEG + 1,
            [255, 96, 96, 235],
            4.0,
        ));
        self.slots.ln_aim = doc.lines.len();
        doc.lines
            .push(line("aim-line".into(), 2, [255, 220, 120, 150], 3.0));
        // Loot beams, lightning, floor rings, the shield bubble and the blades' track.
        self.slots.ln_beam = doc.lines.len();
        for i in 0..ncap {
            doc.lines
                .push(line(format!("beam-{i}"), 2, [255, 255, 255, 120], 5.0));
        }
        self.slots.ln_zap = doc.lines.len();
        for i in 0..ZAP_CAP {
            doc.lines.push(line(
                format!("zap-{i}"),
                (ZAP_PTS - 1) * 4 + 2,
                [255, 245, 130, 255],
                5.0,
            ));
        }
        self.slots.ln_fring = doc.lines.len();
        for i in 0..RING_CAP {
            doc.lines.push(line(
                format!("ring-{i}"),
                RING_SEG + 1,
                [255, 255, 255, 200],
                5.0,
            ));
        }
        self.slots.ln_shield = doc.lines.len();
        doc.lines.push(line(
            "shield".into(),
            RING_SEG + 1,
            [120, 220, 255, 200],
            4.0,
        ));
        self.slots.ln_orbit = doc.lines.len();
        doc.lines
            .push(line("orbit".into(), RING_SEG + 1, [150, 220, 255, 50], 3.0));
        self.doc = doc;
    }

    fn build_doc(&mut self) {
        if self.doc.props.is_empty() {
            self.init_doc();
        }
        let (w, h) = self.size.get();
        let (mut eye, mut target, fov) = self.camera(w / h.max(1.0));
        let fov = fov * (1.0 - 0.035 * self.zoom_kick * self.spec.fx.zoom_pulse);
        let sh = self.core.juice.shake.min(1.0) * 0.25 * self.spec.fx.shake;
        eye[0] += (self.anim_t * 61.0).sin() * sh;
        eye[2] += (self.anim_t * 47.0).sin() * sh;
        target[0] += (self.anim_t * 61.0).sin() * sh;
        target[2] += (self.anim_t * 47.0).sin() * sh;
        let (hx, hz, face) = (self.hx, self.hz, self.face);
        let speed = self.vx.hypot(self.vz);
        let bob = if speed > 0.4 {
            0.09 * (self.anim_t * 12.0).sin().abs()
        } else {
            0.02 * (self.anim_t * 3.0).sin()
        };
        let blink = self.hurt_cd > 0.0 && (self.anim_t * 24.0) as i32 % 2 == 0;
        let (hero_em, hero_glow) = if blink {
            (1.6, [1.0, 0.25, 0.25])
        } else if self.shield_t > 0.0 {
            (0.55, [0.4, 0.85, 1.0])
        } else if self.frenzy_t > 0.0 {
            (0.6, [1.0, 0.6, 0.2])
        } else {
            (0.12, lin(HERO_GLOW))
        };
        let mut slots = self.slots;
        let glb_hero = props_has_model(&self.doc.props[slots.hero]);
        let props = &mut self.doc.props;
        {
            let p = &mut props[slots.shadow];
            p.position = [hx, 0.02, hz];
        }
        {
            let p = &mut props[slots.hero];
            p.yaw = face;
            if glb_hero {
                p.position = [hx, bob, hz];
                p.emissive = Some(hero_em);
                p.emissive_color = Some(hero_glow);
                // A little squash as the hero runs (the models are rest-pose statics).
                let sq = if speed > 0.4 {
                    0.05 * (self.anim_t * 12.0).sin()
                } else {
                    0.0
                };
                p.scale = [
                    HERO_MODEL_SCALE * (1.0 + sq),
                    HERO_MODEL_SCALE * (1.0 - sq),
                    HERO_MODEL_SCALE * (1.0 + sq),
                ];
            } else {
                p.position = [hx, 0.4 + bob, hz];
                p.color = Some(if blink { [255, 60, 60] } else { HERO_GLOW });
            }
        }
        if slots.glb_gun {
            let (sn, cs) = face.sin_cos();
            let p = &mut props[slots.gun];
            p.position = [hx + sn * GUN_FWD, bob + GUN_UP, hz + cs * GUN_FWD];
            p.yaw = face + std::f32::consts::PI;
            p.emissive = Some(hero_em * 0.5);
        }
        let n = self.foes.len();
        let mut drawn = [0usize; POOLS];
        for i in 0..n {
            let k = &self.kinds[self.foes.kind[i] as usize];
            let mut m = match k.model {
                MODEL_MIX => usize::from(self.foes.uid[i].is_multiple_of(4)),
                v => v as usize,
            };
            if slots.pool_cap[m] == 0 {
                m = 0; // that model is missing: draw it as the ghost
            }
            if drawn[m] >= slots.pool_cap[m] {
                continue;
            }
            drawn[m] += 1;
            let idx = slots.pool0[m] + drawn[m] - 1;
            let p = &mut props[idx];
            let age = self.foes.age[i];
            let pop = (age / 0.25).min(1.0);
            let wob = (age * 9.0 + self.foes.uid[i] as f32).sin();
            let mk = if slots.glb_foe { FOE_MODEL_K } else { 2.2 };
            let d = k.radius * k.scale * mk * (0.4 + 0.6 * pop);
            let (sx, sy) = (1.0 - 0.07 * wob, 1.0 + 0.1 * wob);
            let (x, z) = (self.foes.x[i], self.foes.z[i]);
            p.enabled = true;
            // The ghost floats a little; the others shamble on the ground.
            let float = if slots.glb_foe && (m == 0 || k.model == 3) {
                0.12 + 0.06 * wob
            } else {
                0.0
            };
            p.position = [x, if slots.glb_foe { float } else { d * 0.4 }, z];
            p.yaw = (hx - x).atan2(hz - z);
            p.scale = [d * sx, d * sy, d * sx];
            let fl = self.foes.flash[i] * 10.0;
            let glow = if k.elite {
                0.35 + 0.25 * (self.anim_t * 4.0).sin()
            } else {
                0.06
            };
            p.emissive = Some(glow + 2.2 * fl);
            p.emissive_color = Some(if fl > 0.0 { [1.0, 1.0, 1.0] } else { k.linear });
            if slots.glb_foe {
                let base = POOL_TINT[m];
                p.color = Some([
                    base[0] * k.tint[0] / 255,
                    base[1] * k.tint[1] / 255,
                    base[2] * k.tint[2] / 255,
                ]);
            } else {
                p.color = Some(k.color);
            }
        }
        for m in 0..POOLS {
            for j in drawn[m]..slots.drawn[m] {
                props[slots.pool0[m] + j].enabled = false;
            }
        }
        slots.drawn = drawn;
        self.slots = slots;
        let gn = self.gems.len();
        for i in 0..self.spec.caps.gems as usize {
            let p = &mut props[slots.gem0 + i];
            if i >= gn {
                if p.enabled {
                    p.enabled = false;
                }
                continue;
            }
            let g = &self.gems[i];
            p.enabled = true;
            let bob = 0.08 * (self.anim_t * 5.0 + i as f32).sin();
            if slots.glb_gem {
                p.position = [g.x, 0.12 + bob, g.z];
                p.yaw = self.anim_t * 2.2 + i as f32;
            } else {
                p.position = [g.x, 0.3 + bob, g.z];
            }
        }
        let sn = self.shots.len();
        for i in 0..self.spec.caps.shots as usize {
            let p = &mut props[slots.shot0 + i];
            if i >= sn {
                if p.enabled {
                    p.enabled = false;
                }
                continue;
            }
            let s = &self.shots[i];
            let wc = self
                .spec
                .weapons
                .get(s.w as usize)
                .map_or([255, 190, 90], |w| w.color.map(|c| c.min(255) as u8));
            p.enabled = true;
            p.position = [s.x, 0.55, s.z];
            p.yaw = s.vx.atan2(s.vz);
            p.color = Some(wc.map(u32::from));
            p.emissive_color = Some(lin(wc.map(u32::from)));
            p.scale = match s.kind {
                1 => [s.r * 0.9, s.r * 0.9, s.r * 1.4],
                2 => {
                    let pulse = 1.0 + 0.15 * (self.anim_t * 14.0 + i as f32).sin();
                    [s.r * 1.7 * pulse; 3]
                }
                _ => [s.r * 0.8, s.r * 0.8, s.r * 2.0],
            };
        }
        self.build_extras(&slots);
        for c in &mut self.doc.points {
            c.positions.clear();
        }
        for p in &self.fx {
            let b = ((p.age / p.life * 3.0) as usize).min(2);
            self.doc.points[p.kind as usize * 3 + b]
                .positions
                .push([p.x, p.y, p.z]);
        }
        let streak = self.spec.fx.trails;
        let nshots = self.spec.caps.shots as usize;
        for (i, l) in self.doc.lines.iter_mut().take(nshots).enumerate() {
            l.points.clear();
            if let (true, Some(s)) = (streak, self.shots.get(i)) {
                let wc = self
                    .spec
                    .weapons
                    .get(s.w as usize)
                    .map_or([255, 190, 90], |w| w.color.map(|c| c.min(255) as u8));
                let len = if s.kind == 2 { 0.16 } else { 0.05 };
                l.points.push([s.x - s.vx * len, 0.58, s.z - s.vz * len]);
                l.points.push([s.x, 0.58, s.z]);
                l.color = [wc[0], wc[1], wc[2], 150];
                l.width = if s.kind == 2 { 6.0 } else { 3.0 };
            }
        }
        let lock = if self.aim_mode != 0 {
            self.lock_foe()
        } else {
            None
        };
        let aim_dir = if self.aim_mode != 0 && lock.is_none() {
            self.aim_dir
        } else {
            None
        };
        let (head, tail) = self.doc.lines.split_at_mut(slots.ln_aim);
        if let (Some(ring), Some(aim)) = (head.get_mut(slots.ln_ring), tail.first_mut()) {
            ring.points.clear();
            aim.points.clear();
            if let Some(j) = lock {
                let r = self.kinds[self.foes.kind[j] as usize].radius
                    + 0.3
                    + 0.07 * (self.anim_t * 7.0).sin();
                let spin = self.anim_t * 1.5;
                for k in 0..=LOCK_SEG {
                    let a = spin + k as f32 / LOCK_SEG as f32 * std::f32::consts::TAU;
                    ring.points.push([
                        self.foes.x[j] + a.sin() * r,
                        0.07,
                        self.foes.z[j] + a.cos() * r,
                    ]);
                }
            }
            if let Some(a) = aim_dir {
                let (sn, cs) = a.sin_cos();
                aim.points.push([hx + sn * 1.2, 0.3, hz + cs * 1.2]);
                aim.points.push([hx + sn * 4.5, 0.3, hz + cs * 4.5]);
            }
        }
        let cam = &mut self.doc.cameras[0];
        cam.position = eye;
        cam.target = target;
        cam.fov = fov;
        let l = &mut self.doc.lights[0];
        l.position = [hx, 9.0, hz + 3.0];
        self.doc.lights[1].position = [hx, 2.4, hz + 0.8];
    }

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

    pub fn hud(&self, width: u32, height: u32, _menu: bool) -> DrawList {
        let mut list = DrawList {
            clear: [0, 0, 0, 0],
            quads: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
        };
        let (w, h) = (width.max(1) as f32, height.max(1) as f32);
        self.size.set((w, h));
        let s = hud_scale(w, h);
        if self.flow.phase == FlowPhase::Title || self.flow.phase == FlowPhase::Result {
            return list;
        }
        let text =
            |list: &mut DrawList, t: &str, x: f32, y: f32, size: f32, c: [u8; 4], a: TextAlign| {
                list.texts
                    .push(TextQuad::new(t, x, y, size * s, c).aligned(a));
            };
        // Hurt vignette.
        if self.hurt_flash > 0.0 {
            list.quads.push(Quad::new(
                0.0,
                0.0,
                w,
                h,
                [255, 40, 60, (self.hurt_flash * 70.0) as u8],
            ));
        }
        // XP bar across the top.
        let frac = (self.xp / self.xp_need()).clamp(0.0, 1.0);
        list.quads
            .push(Quad::new(0.0, 0.0, w, 12.0 * s, [8, 8, 24, 190]));
        list.quads.push(Quad::new(
            0.0,
            0.0,
            w * frac,
            12.0 * s,
            [110, 220, 255, 255],
        ));
        // A sheen on the top half and a bright tip where the bar ends.
        list.quads
            .push(Quad::new(0.0, 0.0, w * frac, 4.5 * s, [220, 250, 255, 120]));
        if frac > 0.01 {
            list.quads.push(Quad::new(
                w * frac - 3.0 * s,
                0.0,
                3.0 * s,
                12.0 * s,
                [255, 255, 255, 255],
            ));
        }
        list.quads.push(Quad::new(
            6.0 * s,
            13.0 * s,
            62.0 * s,
            21.0 * s,
            [8, 8, 24, 150],
        ));
        text(
            &mut list,
            &list_lv(self.lang_ja, self.level),
            12.0 * s,
            15.0 * s,
            15.0,
            [255, 240, 170, 255],
            TextAlign::Left,
        );
        // HP bar under it.
        let (bx, by, bw, bh) = (30.0 * s, 40.0 * s, (w * 0.40).min(200.0 * s), 16.0 * s);
        let hf = (self.hp / self.mods.hp_max).clamp(0.0, 1.0);
        list.quads.push(Quad::new(
            bx - 2.0 * s,
            by - 2.0 * s,
            bw + 4.0 * s,
            bh + 4.0 * s,
            [8, 8, 24, 200],
        ));
        let low = hf <= 0.3 && self.hp > 0.0;
        let beat = if low {
            (self.anim_t * 8.0).sin() * 0.5 + 0.5
        } else {
            0.0
        };
        let hc = if hf > 0.5 {
            [120, 235, 150, 255]
        } else if hf > 0.25 {
            [255, 200, 90, 255]
        } else {
            [
                255,
                (90.0 + 120.0 * beat) as u8,
                (100.0 + 60.0 * beat) as u8,
                255,
            ]
        };
        list.quads.push(Quad::new(bx, by, bw * hf, bh, hc));
        list.quads
            .push(Quad::new(bx, by, bw * hf, bh * 0.4, [255, 255, 255, 80]));
        text(
            &mut list,
            "♥",
            bx - 22.0 * s,
            by - 3.0 * s,
            20.0,
            [255, (90.0 + 120.0 * beat) as u8, 110, 255],
            TextAlign::Left,
        );
        if low {
            // A faint red breathing at the screen edge: the bar is nearly gone.
            list.quads.push(Quad::new(
                0.0,
                0.0,
                w,
                h,
                [255, 30, 50, (18.0 + 26.0 * beat) as u8],
            ));
        }
        text(
            &mut list,
            &format!("{}", self.hp.max(0.0).ceil() as i32),
            bx + 6.0 * s,
            by - 1.0 * s,
            12.0,
            [20, 20, 40, 255],
            TextAlign::Left,
        );
        text(
            &mut list,
            &format!("★ {} {}", self.kills, self.tr("KO", "たおした")),
            bx - 22.0 * s,
            by + 22.0 * s,
            14.0,
            [255, 235, 160, 235],
            TextAlign::Left,
        );
        // Clock: time left until dawn.
        let left = (self.spec.run.secs - self.t).max(0.0).ceil() as u32;
        let pw = 96.0 * s;
        list.quads.push(Quad::new(
            w * 0.5 - pw * 0.5,
            14.0 * s,
            pw,
            40.0 * s,
            [8, 8, 24, 150],
        ));
        let prog = (self.t / self.spec.run.secs.max(1.0)).clamp(0.0, 1.0);
        list.quads.push(Quad::new(
            w * 0.5 - pw * 0.5 + 6.0 * s,
            47.0 * s,
            (pw - 12.0 * s) * prog,
            3.0 * s,
            [255, 190 + (60.0 * prog) as u8, 110, 255],
        ));
        // The clock warms toward dawn and flickers in the last ten seconds.
        let urgent = left <= 10 && (self.anim_t * 6.0).sin() > 0.0;
        let clock = if urgent {
            [255, 150, 120, 255]
        } else {
            [
                255,
                240 - (40.0 * prog) as u8,
                200 - (60.0 * prog) as u8,
                255,
            ]
        };
        text(
            &mut list,
            &format!("{}:{:02}", left / 60, left % 60),
            w * 0.5,
            17.0 * s,
            26.0,
            clock,
            TextAlign::Center,
        );
        // Floating damage numbers.
        if self.spec.fx.damage_numbers {
            for d in &self.dmg {
                let t = d.age / DMG_LIFE;
                let Some((sx, sy)) = self.project([d.x, 0.9 + t * 0.9, d.z], w, h) else {
                    continue;
                };
                let a = ((1.0 - t * t) * 255.0) as u8;
                let pop = 1.0 + 0.6 * (1.0 - (d.age * 9.0).min(1.0));
                let label = format!("{}", d.val);
                text(
                    &mut list,
                    &label,
                    sx + 1.5 * s,
                    sy + 1.5 * s,
                    15.0 * pop,
                    [10, 10, 30, a / 2],
                    TextAlign::Center,
                );
                text(
                    &mut list,
                    &label,
                    sx,
                    sy,
                    15.0 * pop,
                    [255, 240, 190, a],
                    TextAlign::Center,
                );
            }
        }
        self.hud_loadout(&mut list, w, h, s);
        // The lock-on arrow over the locked foe.
        if self.aim_mode != 0 && self.flow.phase == FlowPhase::Play {
            if let Some(j) = self.lock_foe() {
                if let Some((sx, sy)) = self.project([self.foes.x[j], 1.35, self.foes.z[j]], w, h) {
                    let bob = (self.anim_t * 7.0).sin() * 3.0 * s;
                    text(
                        &mut list,
                        "▼",
                        sx,
                        sy - 12.0 * s + bob,
                        18.0,
                        [255, 96, 96, 255],
                        TextAlign::Center,
                    );
                }
            }
        }
        // Pause button.
        if self.flow.phase == FlowPhase::Play {
            let r = pause_rect(w, s);
            list.quads
                .push(Quad::new(r.x, r.y, r.w, r.h, [8, 8, 24, 150]));
            for dx in [0.32_f32, 0.58] {
                list.quads.push(Quad::new(
                    r.x + r.w * dx,
                    r.y + r.h * 0.26,
                    r.w * 0.11,
                    r.h * 0.48,
                    [255, 255, 255, 230],
                ));
            }
        }
        // The thumb marker: a soft round dot (a square read as a stray grey box).
        if let Some((fx, fy)) = self.finger {
            text(
                &mut list,
                "●",
                fx,
                fy - 12.0 * s,
                22.0,
                [255, 255, 255, 70],
                TextAlign::Center,
            );
            if self.spec.input.mode == "stick" {
                text(
                    &mut list,
                    "○",
                    self.origin.0,
                    self.origin.1 - 22.0 * s,
                    44.0,
                    [255, 255, 255, 50],
                    TextAlign::Center,
                );
            }
        }
        if !self.cards.is_empty() {
            self.hud_cards(&mut list, w, h, s);
        } else if self.flow.phase == FlowPhase::Pause {
            self.hud_pause(&mut list, w, h, s);
        }
        list
    }

    fn hud_cards(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        list.quads.push(Quad::new(0.0, 0.0, w, h, [6, 6, 20, 200]));
        list.texts.push(
            TextQuad::new(
                if self.swap_in.is_some() {
                    self.tr("WEAPON FOUND!", "ぶきをみつけた！")
                } else {
                    self.tr("LEVEL UP!", "レベルアップ！")
                },
                w * 0.5,
                h * 0.09,
                32.0 * s,
                [255, 240, 160, 255],
            )
            .aligned(TextAlign::Center),
        );
        list.texts.push(
            TextQuad::new(
                if self.swap_in.is_some() {
                    self.tr(
                        "Slots full: drop one?",
                        "スロットがいっぱい：どれをすてる？",
                    )
                } else {
                    self.tr("Pick one", "ひとつえらんでね")
                },
                w * 0.5,
                h * 0.09 + 40.0 * s,
                14.0 * s,
                [255, 255, 255, 190],
            )
            .aligned(TextAlign::Center),
        );
        for (c, r) in self.cards.iter().zip(card_rects(w, h, s, self.cards.len())) {
            let (tag, name, desc, accent) = self.card_text(*c);
            let a = [accent[0], accent[1], accent[2], 255];
            list.quads.push(Quad::new(
                r.x - 3.0 * s,
                r.y - 3.0 * s,
                r.w + 6.0 * s,
                r.h + 6.0 * s,
                a,
            ));
            list.quads
                .push(Quad::new(r.x, r.y, r.w, r.h, [28, 24, 56, 250]));
            list.quads.push(Quad::new(r.x, r.y, 8.0 * s, r.h, a));
            // An icon badge: the card's glyph on a tinted tile.
            let portrait = h >= w;
            let bs = if portrait {
                (r.h * 0.62).min(76.0 * s)
            } else {
                56.0 * s
            };
            let (bx, by) = if portrait {
                (r.x + 18.0 * s, r.y + (r.h - bs) * 0.5)
            } else {
                (r.x + 18.0 * s, r.y + 14.0 * s)
            };
            list.quads.push(Quad::new(
                bx,
                by,
                bs,
                bs,
                [a[0] / 3, a[1] / 3, a[2] / 3, 255],
            ));
            list.quads
                .push(Quad::new(bx, by, bs, 3.0 * s, [a[0], a[1], a[2], 200]));
            list.texts.push(
                TextQuad::new(
                    self.card_glyph(*c),
                    bx + bs * 0.5,
                    by + bs * 0.5 - bs * 0.30,
                    bs * 0.6,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Center),
            );
            let (tx, ty) = if portrait {
                (bx + bs + 14.0 * s, r.y + 9.0 * s)
            } else {
                (r.x + 22.0 * s, by + bs + 12.0 * s)
            };
            let (cur, max) = self.card_pips(*c);
            if max > 1 {
                let pips: String = (0..max)
                    .map(|k| match k.cmp(&cur) {
                        std::cmp::Ordering::Less => '●',
                        std::cmp::Ordering::Equal => '◆',
                        std::cmp::Ordering::Greater => '○',
                    })
                    .collect();
                list.texts.push(
                    TextQuad::new(&pips, r.x + r.w - 12.0 * s, r.y + 8.0 * s, 12.0 * s, a)
                        .aligned(TextAlign::Right),
                );
            }
            list.texts.push(TextQuad::new(&tag, tx, ty, 12.0 * s, a));
            list.texts.push(TextQuad::new(
                &name,
                tx,
                ty + 17.0 * s,
                22.0 * s,
                [255, 255, 255, 255],
            ));
            list.texts.push(TextQuad::new(
                &desc,
                tx,
                ty + 47.0 * s,
                14.0 * s,
                [225, 225, 245, 230],
            ));
        }
    }

    fn hud_pause(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        list.quads.push(Quad::new(0.0, 0.0, w, h, [6, 6, 20, 205]));
        list.texts.push(
            TextQuad::new(
                self.tr("PAUSED", "ポーズ"),
                w * 0.5,
                h * 0.16,
                30.0 * s,
                [255, 255, 255, 255],
            )
            .aligned(TextAlign::Center),
        );
        for (b, r) in pause_buttons(w, h, s) {
            let (label, fill, ink) = match b {
                PauseBtn::Resume => (
                    self.tr("RESUME", "つづける"),
                    [255, 170, 80, 255],
                    [30, 14, 20, 255],
                ),
                PauseBtn::Aim => (
                    match self.aim_mode {
                        0 => self.tr("AIM: AUTO", "ねらい: オート"),
                        1 => self.tr("AIM: TAP", "ねらい: タップ"),
                        _ => self.tr("AIM: HYBRID", "ねらい: ミックス"),
                    },
                    [40, 34, 74, 245],
                    [150, 235, 255, 255],
                ),
                PauseBtn::Retry => (
                    self.tr("RETRY", "リトライ"),
                    [40, 34, 74, 245],
                    [255, 255, 255, 255],
                ),
                PauseBtn::Title => (
                    self.tr("TITLE", "タイトル"),
                    [40, 34, 74, 245],
                    [255, 255, 255, 255],
                ),
            };
            list.quads.push(Quad::new(r.x, r.y, r.w, r.h, fill));
            list.texts.push(
                TextQuad::new(
                    label,
                    r.x + r.w * 0.5,
                    r.y + r.h * 0.5 - 9.0 * s,
                    18.0 * s,
                    ink,
                )
                .aligned(TextAlign::Center),
            );
        }
    }
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
mod tests {
    use super::*;
    use crate::input::{PointerEvent, PointerPhase};

    const DT: f32 = 1.0 / 60.0;

    fn fresh() -> SurvivorArena {
        SurvivorArena::new(SurvivorSpec::default())
    }

    fn started() -> SurvivorArena {
        let mut g = fresh();
        g.confirm();
        g
    }

    /// Never moves, takes the first card.
    fn idle(g: &mut SurvivorArena, secs: f32) {
        let mut n = 0;
        while g.flow.phase == FlowPhase::Play && g.secs() < secs && n < 60 * 400 {
            if g.card_count() > 0 {
                g.choose(0);
            }
            g.tick(DT);
            n += 1;
        }
    }

    /// A kiting bot: away from the crowd, toward gems, off the walls. It may only
    /// use what a thumb can do (a direction), never touch the sim.
    fn kite_step(g: &mut SurvivorArena) {
        let half = g.spec.arena.half;
        let (hx, hz) = g.hero();
        let (mut dx, mut dz) = (0.0f32, 0.0f32);
        let f = g.foes();
        for i in 0..f.len() {
            let (ox, oz) = (hx - f.x[i], hz - f.z[i]);
            let d2 = ox * ox + oz * oz;
            if d2 < 36.0 && d2 > 1e-4 {
                let d = d2.sqrt();
                let w = 1.0 / d2;
                dx += ox / d * w * 4.0;
                dz += oz / d * w * 4.0;
            }
        }
        if let Some((gx, gz)) = g.gem_positions().min_by(|a, b| {
            let da = (a.0 - hx).powi(2) + (a.1 - hz).powi(2);
            let db = (b.0 - hx).powi(2) + (b.1 - hz).powi(2);
            da.total_cmp(&db)
        }) {
            let (ox, oz) = (gx - hx, gz - hz);
            let d = ox.hypot(oz).max(0.1);
            dx += ox / d * 0.35;
            dz += oz / d * 0.35;
        }
        let m = half - 3.5;
        if hx.abs() > m {
            dx -= hx.signum() * 2.0;
        }
        if hz.abs() > m {
            dz -= hz.signum() * 2.0;
        }
        g.walk(dx, -dz);
    }

    fn kite(g: &mut SurvivorArena, secs: f32) {
        kite_each(g, secs, |_| {});
    }

    fn kite_each(g: &mut SurvivorArena, secs: f32, mut each: impl FnMut(&SurvivorArena)) {
        let mut n = 0;
        while g.flow.phase == FlowPhase::Play && g.secs() < secs && n < 60 * 400 {
            if g.card_count() > 0 {
                // Deepen what is held (the sensible player), else take the first card; a
                // swap offer is declined.
                let id = |i: usize| g.card_id(i).unwrap_or_default();
                let held = |c: &str| {
                    c.strip_prefix("weapon:")
                        .is_some_and(|w| g.weapon_level(w) > 0)
                };
                let at = (0..g.card_count())
                    .find(|i| held(&id(*i)))
                    .or_else(|| (0..g.card_count()).find(|i| id(*i) == "keep"))
                    .unwrap_or(0);
                g.choose(at);
            }
            kite_step(g);
            g.tick(DT);
            each(g);
            n += 1;
        }
        g.walk(0.0, 0.0);
    }

    #[test]
    #[ignore]
    fn timeline() {
        for seed in 1..=3u32 {
            let mut spec = SurvivorSpec::default();
            spec.director.seed = seed;
            let mut g = SurvivorArena::new(spec);
            g.confirm();
            let mut t = 10.0;
            while g.flow.phase == FlowPhase::Play && g.secs() < 400.0 {
                kite(&mut g, t);
                eprintln!(
                    "seed{seed} t={:.0} lv={} kills={} foes={} hp={:.0}",
                    g.secs(),
                    g.level(),
                    g.kills(),
                    g.foes().len(),
                    g.hp()
                );
                t += 10.0;
            }
            eprintln!(
                "seed{seed} end t={:.1} {} win={}",
                g.secs(),
                g.phase_name(),
                g.won()
            );
        }
    }

    fn kind_of(g: &SurvivorArena, id: &str) -> u8 {
        g.spec
            .enemies
            .types
            .iter()
            .position(|t| t.id == id)
            .unwrap() as u8
    }

    /// A world with no director spawns, for staging a fight by hand.
    fn quiet(start_weapon: &str) -> SurvivorArena {
        let mut spec = SurvivorSpec::default();
        spec.hero.start_weapon = start_weapon.into();
        spec.hero.max_hp = 10_000.0;
        spec.director.start_rate = 0.0;
        spec.director.end_rate = 0.0;
        spec.director.elite_first = 0.0;
        let mut g = SurvivorArena::new(spec);
        g.confirm();
        g
    }

    #[test]
    fn drops_are_seeded_capped_pooled_and_switchable() {
        let run = |enabled: bool| {
            let mut spec = SurvivorSpec::default();
            spec.drops.enabled = enabled;
            spec.drops.tables[0].chance = 1.0;
            let mut g = SurvivorArena::new(spec);
            g.confirm();
            let cap = g.spec.drops.cap as usize;
            let ptr = g.pickups.as_ptr();
            let mut most = 0;
            kite_each(&mut g, 40.0, |g| {
                most = most.max(g.pickups.len());
                assert!(g.pickups.len() <= cap, "pickups are capped");
            });
            assert_eq!(ptr, g.pickups.as_ptr(), "no reallocation");
            (g.kills(), most, g.pickups.len(), g.hp().to_bits())
        };
        let (a, b) = (run(true), run(true));
        assert_eq!(a, b, "a seed drops the same things twice");
        assert!(a.1 >= 8, "a storm of drops fills the pool ({})", a.1);
        assert_eq!(run(false).1, 0, "switched off: nothing drops");
    }

    #[test]
    fn the_pity_timer_forces_a_drop() {
        let mut spec = SurvivorSpec::default();
        spec.drops.tables[0].chance = 0.0;
        spec.drops.pity_kills = 5;
        let mut g = SurvivorArena::new(spec);
        g.confirm();
        let k = g.kinds[0];
        for _ in 0..4 {
            g.drop_loot(1.0, 1.0, &k);
        }
        assert!(g.pickups.is_empty());
        g.drop_loot(1.0, 1.0, &k);
        assert_eq!(g.pickups.len(), 1);
    }

    #[test]
    fn the_elite_arrives_and_always_leaves_a_chest_with_a_weapon() {
        let mut g = quiet("bolt");
        g.next_elite = 45.0;
        g.t = 46.0;
        g.tick(DT);
        assert_eq!(g.state_value()["elites"], 1);
        assert!(g
            .hud(390, 844, false)
            .texts
            .iter()
            .any(|t| t.text.starts_with('☠')));
        let j = (0..g.foes.len())
            .find(|&i| g.kinds[g.foes.kind[i] as usize].elite)
            .unwrap();
        g.foes.hp[j] = 0.0;
        g.tick(DT);
        assert!(
            g.pickups.iter().any(|p| p.item == CHEST),
            "the elite leaves a chest"
        );
        g.build_doc();
        assert!(g.doc.props.iter().any(|p| p.enabled && p.name == "chest"));
        let (cx, cz) = (g.pickups[0].x, g.pickups[0].z);
        (g.hx, g.hz) = (cx, cz);
        g.tick(DT);
        assert!(
            g.pickups.iter().all(|p| p.item != CHEST),
            "touching opens it"
        );
        assert!(g.held_weapons() >= 2, "the chest guarantees a weapon");
        assert!(g.state_value()["toast"]
            .as_str()
            .is_some_and(|t| !t.is_empty()));
    }

    #[test]
    fn weapon_pickups_fill_the_slots_then_offer_a_swap() {
        let mut g = quiet("bolt");
        let max = g.spec.levels.max_weapons;
        for _ in 0..500 {
            if g.held_weapons() >= max {
                break;
            }
            g.grant_weapon();
        }
        assert_eq!(g.held_weapons(), max, "slots fill up");
        assert!(g.cards.is_empty());
        for _ in 0..2000 {
            g.grant_weapon();
            if !g.cards.is_empty() {
                break;
            }
        }
        assert!(g.swap_in.is_some(), "a full bar offers a swap");
        assert!(matches!(g.cards.last(), Some(Card::Keep)));
        assert!(g.cards.iter().any(|c| matches!(c, Card::Swap(_))));
        let incoming = g.swap_in.unwrap() as usize;
        assert_eq!(g.weapon_lv[incoming], 0);
        let hud = g.hud(390, 844, false);
        assert!(hud.texts.iter().any(|t| t.text == "WEAPON FOUND!"));
        // Swap the first offer in.
        let Some(Card::Swap(old)) = g.cards.first().copied() else {
            panic!("the first card is a swap");
        };
        g.choose(0);
        assert!(g.cards.is_empty() && g.swap_in.is_none());
        assert_eq!(g.weapon_lv[old as usize], 0, "the old weapon is gone");
        assert_eq!(g.weapon_lv[incoming], 1, "the new one is in");
        assert_eq!(g.held_weapons(), max);
        // And keeping refuses the new one.
        for _ in 0..2000 {
            g.grant_weapon();
            if !g.cards.is_empty() {
                break;
            }
        }
        let before: Vec<u32> = g.weapon_lv.clone();
        let last = g.cards.len() - 1;
        g.choose(last);
        assert_eq!(before, g.weapon_lv);
        assert!(g.cards.is_empty());
    }

    #[test]
    fn every_weapon_hurts_a_crowd() {
        let ids: Vec<String> = SurvivorSpec::default()
            .weapons
            .iter()
            .map(|w| w.id.clone())
            .collect();
        for id in ids {
            let mut g = quiet(&id);
            for k in 0..8 {
                let a = k as f32 * 0.785;
                g.foes.spawn(0, a.sin() * 2.6, a.cos() * 2.6, 1000.0);
            }
            let hp0: f32 = g.foes.hp.iter().sum();
            for _ in 0..360 {
                g.tick(DT);
            }
            let hp1: f32 = g.foes.hp.iter().sum();
            assert!(hp1 < hp0 - 15.0, "{id} dealt only {:.1} in 6 s", hp0 - hp1);
        }
    }

    #[test]
    fn each_weapon_builds_its_own_effect() {
        let mut g = quiet("zap");
        for k in 0..4 {
            g.foes.spawn(0, 2.0 + k as f32 * 0.8, 0.0, 1000.0);
        }
        let mut zaps = 0;
        for _ in 0..180 {
            g.tick(DT);
            zaps = zaps.max(g.zaps.len());
        }
        assert!(zaps > 0, "lightning is drawn");
        g.build_doc();
        let mut p = quiet("pulse");
        p.foes.spawn(0, 1.5, 0.0, 1000.0);
        let mut rings = 0;
        let mut far = 0.0f32;
        for _ in 0..200 {
            p.tick(DT);
            rings = rings.max(p.rings.len());
            if !p.foes.x.is_empty() {
                far = far.max(p.foes.x[0]);
            }
        }
        assert!(rings > 0 && far > 1.6, "the pulse shoves (far {far})");
        let mut b = quiet("blades");
        b.foes.spawn(0, 1.9, 0.0, 1000.0);
        for _ in 0..60 {
            b.tick(DT);
        }
        b.build_doc();
        let live = b
            .doc
            .props
            .iter()
            .filter(|p| p.enabled && p.name == "blade")
            .count();
        assert_eq!(live, 2, "two blades circle");
        let mut w = quiet("wisps");
        w.foes.spawn(0, 0.0, 6.0, 1000.0);
        w.foes.spawn(0, 1.0, 6.4, 1000.0);
        let mut shots = 0;
        for _ in 0..120 {
            w.tick(DT);
            shots = shots.max(w.shots.len());
        }
        assert!(shots >= 1, "wisps fly");
        let mut f = quiet("petals");
        f.foes.spawn(0, 0.0, 3.0, 1000.0);
        f.tick(DT);
        assert!(f.shots.len() >= 4, "a fan of petals ({})", f.shots.len());
    }

    #[test]
    fn a_shield_blocks_hits_and_frenzy_speeds_the_weapons() {
        let mut g = quiet("bolt");
        g.hp = 100.0;
        g.shield_t = 3.0;
        g.foes.spawn(0, 0.0, 0.0, 1000.0);
        for _ in 0..60 {
            g.tick(DT);
        }
        assert_eq!(g.hp, 100.0, "the shield takes the hit");
        g.shield_t = 0.0;
        for _ in 0..60 {
            g.tick(DT);
        }
        assert!(g.hp < 100.0, "then it wears off");
        let slow = g.cd_scale();
        g.frenzy_t = 5.0;
        g.frenzy_amt = 0.45;
        assert!(g.cd_scale() < slow * 0.6);
    }

    #[test]
    fn items_do_what_they_say() {
        let mut g = quiet("bolt");
        let id = |g: &SurvivorArena, kind: PickupKind| {
            g.spec
                .drops
                .items
                .iter()
                .position(|i| i.kind == kind)
                .unwrap()
        };
        g.hp = 10.0;
        let heal = id(&g, PickupKind::Heal);
        g.apply_item(heal);
        assert!(g.hp > 10.0);
        let xp0 = g.xp;
        let dust = id(&g, PickupKind::Xp);
        g.apply_item(dust);
        assert!(g.xp > xp0 || g.level > 1);
        let shield = id(&g, PickupKind::Shield);
        g.apply_item(shield);
        assert!(g.shield_t > 0.0);
        let frenzy = id(&g, PickupKind::Frenzy);
        g.apply_item(frenzy);
        assert!(g.frenzy_t > 0.0);
        g.foes.spawn(0, 2.0, 0.0, 1.0);
        let bomb = id(&g, PickupKind::Bomb);
        g.apply_item(bomb);
        g.tick(DT);
        assert_eq!(g.foes.len(), 0, "the bomb clears what is near");
        g.gems.push(Gem {
            x: 9.0,
            z: 9.0,
            v: 1.0,
            fly: false,
        });
        let magnet = id(&g, PickupKind::Magnet);
        g.apply_item(magnet);
        assert!(g.gems.iter().all(|m| m.fly), "a magnet calls every gem");
    }

    #[test]
    fn synergies_switch_on_with_a_weapon_and_a_passive() {
        let mut spec = SurvivorSpec::default();
        spec.synergies.clear();
        let mut plain = SurvivorArena::new(spec);
        plain.confirm();
        let mut g = started();
        let hi = |g: &SurvivorArena| {
            g.spec
                .passives
                .iter()
                .position(|p| p.id == "haste")
                .unwrap()
        };
        let (a, b) = (hi(&plain), hi(&g));
        plain.passive_lv[a] = 3;
        plain.recompute();
        g.passive_lv[b] = 3;
        g.recompute();
        assert_eq!(plain.syn_mask, 0);
        assert_ne!(g.syn_mask, 0);
        assert!(g.weapon_stats[0].cooldown < plain.weapon_stats[0].cooldown);
        assert!(g.hud(390, 844, false).texts.iter().any(|t| t.text == "★"));
    }

    #[test]
    fn shooters_throw_orbs_and_the_pools_hold() {
        let mut g = quiet("bolt");
        let k = kind_of(&g, "shooter");
        g.foes.spawn(k, 0.0, 6.0, 1000.0);
        let (op, zp, rp) = (g.orbs.as_ptr(), g.zaps.as_ptr(), g.rings.as_ptr());
        let mut most = 0;
        for _ in 0..600 {
            g.tick(DT);
            most = most.max(g.orbs.len());
            assert!(g.orbs.len() <= ORB_CAP);
        }
        assert!(most >= 1, "the shooter throws");
        assert!(g.hp < 10_000.0, "an orb can hurt");
        assert_eq!(
            (op, zp, rp),
            (g.orbs.as_ptr(), g.zaps.as_ptr(), g.rings.as_ptr())
        );
    }

    #[test]
    fn the_whole_cast_shows_up_and_the_loadout_is_drawn() {
        let mut g = started();
        let mut seen = [false; 5];
        let mut elite_hud = false;
        kite_each(&mut g, 70.0, |g| {
            for i in 0..g.foes.len() {
                seen[g.foes.kind[i] as usize] = true;
            }
            elite_hud |= g
                .hud(390, 844, false)
                .texts
                .iter()
                .any(|t| t.text.starts_with('☠'));
        });
        assert_eq!(
            seen, [true; 5],
            "ghoul, runner, shooter, tank and warden all show up"
        );
        assert!(elite_hud, "the elite has a health bar");
        let hud = g.hud(390, 844, false);
        assert!(hud.texts.iter().any(|t| t.text == "●"), "the bolt chip");
        let w = g.state_value()["weapons"].as_array().unwrap().len();
        assert!(w >= 1);
    }

    #[test]
    fn each_weapon_alone_holds_the_early_game() {
        let ids: Vec<String> = SurvivorSpec::default()
            .weapons
            .iter()
            .map(|w| w.id.clone())
            .collect();
        for id in ids {
            let mut spec = SurvivorSpec::default();
            spec.hero.start_weapon = id.clone();
            let mut g = SurvivorArena::new(spec);
            g.confirm();
            kite(&mut g, 30.0);
            assert!(g.secs() >= 30.0, "{id} died at {:.0}s", g.secs());
            assert!(g.kills() >= 25, "{id} only killed {}", g.kills());
        }
    }

    fn merge(a: &mut Value, b: &Value) {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                for (k, v) in y {
                    merge(x.entry(k.clone()).or_insert(Value::Null), v);
                }
            }
            (a, b) => *a = b.clone(),
        }
    }

    /// `KAGRA_TUNE='{"director":{"elite_first":60}}' cargo test --release sweep -- --ignored
    /// --nocapture`: kiting bots over eight seeds with a patch on the default world.
    #[test]
    #[ignore]
    fn sweep() {
        let mut v = fresh().world_value()["game"].clone();
        if let Ok(t) = std::env::var("KAGRA_TUNE") {
            merge(&mut v, &serde_json::from_str(&t).unwrap());
        }
        let secs: f32 = std::env::var("KAGRA_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000.0);
        let mut wins = 0;
        for seed in 1..=8u32 {
            v["director"]["seed"] = json!(seed);
            let mut g = SurvivorArena::new(SurvivorSpec::from_value(&v).unwrap());
            g.confirm();
            kite(&mut g, secs);
            wins += u32::from(g.won());
            eprintln!(
                "seed{seed} t={:.0} lv={} kills={} win={} weapons={} pickups_left={}",
                g.secs(),
                g.level(),
                g.kills(),
                g.won(),
                g.weapon_values(),
                g.pickups.len()
            );
        }
        eprintln!("seed wins {wins}/8");
    }

    /// `cargo test --release -p kagra-shared --lib perf_probe -- --ignored --nocapture`: the
    /// per-frame load of a busy late run on each quality tier (sim + doc build time, live props,
    /// points and lines handed to the renderer).
    #[test]
    #[ignore]
    fn perf_probe() {
        for (name, q) in [("low", Some("low")), ("mid", None), ("high", Some("high"))] {
            let mut spec = SurvivorSpec::default();
            spec.hero.max_hp = 1.0e6;
            spec.director.seed = 3;
            let mut g = SurvivorArena::new(spec);
            g.picture_quality = q.map(String::from);
            g.apply_quality();
            g.build_doc();
            g.confirm();
            for _ in 0..3 {
                g.grant_weapon();
            }
            let (mut props, mut pts, mut lines, mut max_props, mut n) =
                (0usize, 0usize, 0usize, 0usize, 0u32);
            let mut ns = 0u128;
            let mut foes = 0usize;
            while g.secs() < 119.0 && g.flow.phase == FlowPhase::Play {
                if g.card_count() > 0 {
                    g.choose(0);
                }
                kite_step(&mut g);
                let t0 = std::time::Instant::now();
                g.tick(DT);
                let dt = t0.elapsed().as_nanos();
                if g.secs() > 70.0 {
                    ns += dt;
                    n += 1;
                    let live = g.doc.props.iter().filter(|p| p.enabled).count();
                    props += live;
                    max_props = max_props.max(live);
                    pts += g
                        .doc
                        .points
                        .iter()
                        .map(|p| p.positions.len())
                        .sum::<usize>();
                    lines += g.doc.lines.iter().map(|l| l.points.len()).sum::<usize>();
                    foes += g.foes.len();
                }
            }
            if std::env::var("KAGRA_PROPS").is_ok() {
                let mut by: std::collections::BTreeMap<String, usize> = Default::default();
                for p in g.doc.props.iter().filter(|p| p.enabled) {
                    let key = format!(
                        "{} {}",
                        p.name.trim_end_matches(char::is_numeric),
                        p.gltf.clone().unwrap_or_default()
                    );
                    *by.entry(key).or_default() += 1;
                }
                eprintln!("{name} live props: {by:?}");
            }
            // What one render() costs on the CPU before the GPU sees a vertex (native release:
            // a phone in wasm is several times slower).
            let time = |label: &str, f: &mut dyn FnMut()| {
                let t0 = std::time::Instant::now();
                for _ in 0..200 {
                    f();
                }
                eprintln!(
                    "{name}:   {label} {:.1} us",
                    t0.elapsed().as_nanos() as f64 / 200.0 / 1000.0
                );
            };
            time("doc.clone()", &mut || {
                std::hint::black_box(g.doc.clone());
            });
            time("gpu_mesh_key", &mut || {
                std::hint::black_box(g.doc.gpu_mesh_key());
            });
            time("gltf_slots", &mut || {
                std::hint::black_box(g.doc.gltf_slots());
            });
            time("compile_scene", &mut || {
                std::hint::black_box(g.doc.compile_scene(0.45));
            });
            let fp = crate::profile::FrameProfile::from_world(&g.doc);
            eprintln!(
                "{name}:   gpu draws {} triangles {} texture bytes {}",
                fp.draws, fp.triangles, fp.texture_bytes
            );
            let n = n.max(1) as usize;
            eprintln!(
                "{name}: t={:.0}s avg tick {:.1} us | foes {} | live props avg {} max {} of {} | points {} | line pts {}",
                g.secs(),
                ns as f64 / n as f64 / 1000.0,
                foes / n,
                props / n,
                max_props,
                g.doc.props.len(),
                pts / n,
                lines / n
            );
        }
    }

    /// `cargo test --release -p kagra-shared --features render --lib render_probe -- --ignored --nocapture`
    /// draws a busy late frame on the software GPU and times it with one picture feature
    /// off at a time. Software numbers are not a phone's, but the ratios show which pass or
    /// prop family is worth cutting (fill-rate bound work scales the same way).
    #[cfg(feature = "render")]
    #[test]
    #[ignore]
    fn render_probe() {
        use crate::render::Renderer;
        let (w, h) = (390u32, 844u32);
        let mut spec = SurvivorSpec::default();
        spec.hero.max_hp = 1.0e6;
        spec.director.seed = 3;
        let mut g = SurvivorArena::new(spec);
        g.apply_quality();
        g.build_doc();
        g.confirm();
        for _ in 0..3 {
            g.grant_weapon();
        }
        while g.secs() < 100.0 && g.flow.phase == FlowPhase::Play {
            if g.card_count() > 0 {
                g.choose(0);
            }
            kite_step(&mut g);
            g.tick(DT);
        }
        let Ok(mut r) = pollster::block_on(Renderer::new_offscreen(w, h)) else {
            eprintln!("no adapter");
            return;
        };
        let hud = crate::scene::DrawList::default();
        let base = g.doc.clone();
        let run = |label: &str, doc: &WorldDoc, rr: &mut Renderer| {
            let _ = rr.draw_world_doc_with_hud(doc, &hud);
            let _ = rr.read_rgba();
            let t0 = std::time::Instant::now();
            for _ in 0..4 {
                rr.draw_world_doc_with_hud(doc, &hud).unwrap();
                let _ = rr.read_rgba();
            }
            eprintln!(
                "{label:<22} {:7.1} ms/frame",
                t0.elapsed().as_secs_f64() * 250.0
            );
        };
        run("base", &base, &mut r);
        let mut d = base.clone();
        d.bloom = Some(0.0);
        run("no bloom", &d, &mut r);
        let mut d = base.clone();
        d.lut = None;
        run("no lut", &d, &mut r);
        let mut d = base.clone();
        d.fxaa = Some(false);
        run("no fxaa", &d, &mut r);
        let mut d = base.clone();
        d.bloom = Some(0.0);
        d.lut = None;
        d.fxaa = Some(false);
        d.vignette = Some(0.0);
        run("no post at all", &d, &mut r);
        let mut d = base.clone();
        d.env_map = None;
        d.ibl = Some(0.0);
        run("no env/ibl", &d, &mut r);
        let mut d = base.clone();
        d.lights.truncate(1);
        run("one light", &d, &mut r);
        let mut d = base.clone();
        d.points.clear();
        d.lines.clear();
        run("no points/lines", &d, &mut r);
        for prefix in [
            "pick", "foe", "gem", "wall", "column", "decor", "shot", "hero", "floor", "ring",
            "loot", "blade", "chest",
        ] {
            let mut d = base.clone();
            let mut n = 0;
            for p in d
                .props
                .iter_mut()
                .filter(|p| p.name.starts_with(prefix) || p.id.starts_with(prefix))
            {
                if p.enabled {
                    n += 1;
                }
                p.enabled = false;
            }
            if n > 0 {
                run(&format!("no {prefix} ({n})"), &d, &mut r);
            }
        }
        let mut d = base.clone();
        for p in d.props.iter_mut() {
            p.enabled = false;
        }
        run("no props", &d, &mut r);
        for (sw, sh) in [(585u32, 1266u32), (780, 1688), (294, 633)] {
            if let Ok(mut r2) = pollster::block_on(Renderer::new_offscreen(sw, sh)) {
                run(&format!("base @{sw}x{sh}"), &base, &mut r2);
            }
        }
    }

    fn ev(phase: PointerPhase, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            id: 1,
            x,
            y,
            phase,
            pressure: 1.0,
        }
    }

    #[test]
    fn the_default_spec_is_valid_and_strict() {
        let spec = SurvivorSpec::default();
        spec.validate().unwrap();
        let mut v = fresh().world_value()["game"].clone();
        v["hero"]["bogus"] = json!(1);
        assert!(
            SurvivorSpec::from_value(&v).is_err(),
            "unknown keys are refused"
        );
        let mut bad = SurvivorSpec::default();
        bad.hero.max_hp = -1.0;
        assert!(bad.validate().is_err());
        let mut bad = SurvivorSpec::default();
        let step = bad.weapons[0].levels[0].clone();
        bad.weapons[0].levels = vec![step; 10];
        assert!(bad.validate().is_err());
    }

    #[test]
    fn the_baked_fixture_is_the_default_world() {
        let want = fresh().world_value();
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/survivor_arena_world.json"
        );
        if std::env::var("KAGRA_BAKE").is_ok() {
            std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
        }
        let got: Value =
            serde_json::from_str(include_str!("../tests/fixtures/survivor_arena_world.json"))
                .unwrap();
        assert_eq!(
            got, want,
            "run `KAGRA_BAKE=1 cargo test baked` to refresh it"
        );
        assert!(is_survivor_dump(&got.to_string()));
    }

    #[test]
    fn a_dump_round_trips_through_the_registry() {
        let json = include_str!("../tests/fixtures/survivor_arena_world.json");
        let kit = crate::kit_host::load(json).unwrap().unwrap();
        assert_eq!(kit.name(), "survivor arena");
        assert_eq!(kit.phase_name(), "title");
        assert!(kit.dump_json().contains("\"sim\""));
        assert!(kit.uses_swipe());
    }

    #[test]
    fn it_runs_the_same_twice() {
        let run = || {
            let mut g = started();
            kite(&mut g, 40.0);
            (
                g.kills(),
                g.level(),
                g.hero(),
                g.foes().len(),
                g.state_json(),
            )
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn a_run_starts_with_one_weapon_and_a_full_bar() {
        let g = started();
        assert_eq!(g.flow.phase, FlowPhase::Play);
        assert_eq!(g.level(), 1);
        assert!((g.hp() - g.hp_max()).abs() < 1e-3);
        assert_eq!(g.weapon_level(&g.spec.hero.start_weapon), 1);
    }

    #[test]
    fn the_blaster_points_where_the_shot_goes() {
        let mut g = started();
        g.foes.spawn(0, g.hx + 3.0, g.hz - 4.0, 999.0);
        for _ in 0..120 {
            g.tick(DT);
            if !g.shots.is_empty() {
                break;
            }
        }
        let s = g.shots[0];
        let aim = s.vx.atan2(s.vz);
        let turn = (g.face - aim + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        assert!(
            turn.abs() < 0.05,
            "the hero faces {} but the shot flies {aim}",
            g.face
        );
        // It leaves from the muzzle (one or two sim steps of flight allowed), along the aim.
        let (dx, dz) = (s.x - g.hx, s.z - g.hz);
        let d = dx.hypot(dz);
        assert!(d > 0.6 && d < MUZZLE + 0.5, "spawned {d} m from the hero");
        assert!((dx.atan2(dz) - aim).abs() < 0.05);
        // It keeps facing the target while it holds the aim, even if the thumb drags the other way.
        g.key_dir = (-1.0, 0.0);
        g.tick(DT);
        assert!((g.face - aim).abs() < 0.05);
    }

    #[test]
    fn effects_fill_pools_inside_their_caps_and_never_change_the_run() {
        let run = |fx: bool| {
            let mut g = started();
            g.spec.fx.particles = fx;
            g.spec.fx.damage_numbers = fx;
            g.spec.fx.trails = fx;
            g.spec.fx.shake = if fx { 1.0 } else { 0.0 };
            let (mut peak, mut peak_dmg, mut seen_points) = (0, 0, 0);
            kite_each(&mut g, 40.0, |g| {
                peak = peak.max(g.fx.len());
                peak_dmg = peak_dmg.max(g.dmg.len());
                assert!(g.fx.len() <= g.fx_cap);
                assert!(g.dmg.len() <= DMG_CAP[g.tier]);
            });
            g.build_doc();
            seen_points += g
                .doc
                .points
                .iter()
                .map(|c| c.positions.len())
                .sum::<usize>();
            let again = g.doc.points.len();
            (
                g.kills(),
                (g.hp() * 100.0) as i32,
                peak,
                peak_dmg,
                seen_points,
                again,
            )
        };
        let (k1, h1, peak, peak_dmg, _, clouds) = run(true);
        let (k0, h0, none, none_dmg, pts, _) = run(false);
        assert_eq!((k1, h1), (k0, h0), "effects changed the sim");
        assert!(peak > 5 && peak_dmg > 0, "peak {peak} dmg {peak_dmg}");
        assert_eq!((none, none_dmg, pts), (0, 0, 0));
        assert_eq!(clouds, FX_LOOK.len() * 3);
    }

    fn finger(g: &mut SurvivorArena, id: u32, x: f32, y: f32, phase: PointerPhase) {
        g.touch_ev(
            PointerEvent {
                id,
                x,
                y,
                phase,
                pressure: 1.0,
            },
            390,
            844,
        );
    }

    /// Foe `uid` as a screen pixel.
    fn on_screen(g: &SurvivorArena, uid: u32) -> (f32, f32) {
        let j = (0..g.foes.len()).find(|&i| g.foes.uid[i] == uid).unwrap();
        g.project([g.foes.x[j], 0.45, g.foes.z[j]], 390.0, 844.0)
            .unwrap()
    }

    /// Tick until a shot exists and return its yaw.
    fn first_shot_yaw(g: &mut SurvivorArena) -> f32 {
        for _ in 0..120 {
            g.tick(DT);
            if let Some(s) = g.shots.first() {
                return s.vx.atan2(s.vz);
            }
        }
        panic!("no shot");
    }

    fn bearing(g: &SurvivorArena, uid: u32) -> f32 {
        let j = (0..g.foes.len()).find(|&i| g.foes.uid[i] == uid).unwrap();
        (g.foes.x[j] - g.hx).atan2(g.foes.z[j] - g.hz)
    }

    fn yaw_gap(a: f32, b: f32) -> f32 {
        let d = (a - b).rem_euclid(std::f32::consts::TAU);
        d.min(std::f32::consts::TAU - d)
    }

    /// A near foe east and a farther one west, both inside the bolt's range.
    fn two_foes() -> (SurvivorArena, u32, u32) {
        let mut g = started();
        g.weapon_cd.iter_mut().for_each(|c| *c = 5.0);
        g.foes.spawn(0, g.hx + 2.5, g.hz, 9999.0);
        g.foes.spawn(0, g.hx - 5.5, g.hz + 1.0, 9999.0);
        let (near, far) = (g.foes.uid[0], g.foes.uid[1]);
        g.tick(DT);
        (g, near, far)
    }

    #[test]
    fn a_tap_with_the_other_thumb_locks_a_foe_and_the_shot_follows_it() {
        let (mut g, near, far) = two_foes();
        assert_eq!(g.aim_name(), "hybrid");
        finger(&mut g, 1, 195.0, 700.0, PointerPhase::Begin); // thumb one steers
        let (x, y) = on_screen(&g, far);
        finger(&mut g, 2, x, y, PointerPhase::Begin); // thumb two taps the far foe
        finger(&mut g, 2, x, y, PointerPhase::End);
        assert_eq!(g.locked(), Some(far));
        g.weapon_cd.iter_mut().for_each(|c| *c = 0.0);
        let yaw = first_shot_yaw(&mut g);
        assert!(
            yaw_gap(yaw, bearing(&g, far)) < 0.2,
            "shot ignored the lock"
        );
        assert!(yaw_gap(yaw, bearing(&g, near)) > 1.0);
        // The blaster points the same way.
        assert!(yaw_gap(g.face, yaw) < 1e-3);
        // Tap it again: released, the nearest is back.
        finger(&mut g, 2, x, y, PointerPhase::Begin);
        assert_eq!(g.locked(), None);
    }

    #[test]
    fn the_lock_falls_back_to_the_nearest_when_the_target_dies() {
        let (mut g, near, far) = two_foes();
        let (x, y) = on_screen(&g, far);
        finger(&mut g, 1, 195.0, 700.0, PointerPhase::Begin);
        finger(&mut g, 2, x, y, PointerPhase::Begin);
        assert_eq!(g.locked(), Some(far));
        let j = (0..g.foes.len()).find(|&i| g.foes.uid[i] == far).unwrap();
        g.foes.hp[j] = 0.0;
        g.tick(DT);
        g.shots.clear();
        g.weapon_cd.iter_mut().for_each(|c| *c = 0.0);
        let yaw = first_shot_yaw(&mut g);
        assert_eq!(g.locked(), None);
        assert!(yaw_gap(yaw, bearing(&g, near)) < 0.2, "did not fall back");
    }

    #[test]
    fn a_quick_tap_of_the_lone_finger_also_locks() {
        let (mut g, _, far) = two_foes();
        let (x, y) = on_screen(&g, far);
        finger(&mut g, 1, x, y, PointerPhase::Begin);
        g.tick(DT);
        finger(&mut g, 1, x, y, PointerPhase::End);
        assert_eq!(g.locked(), Some(far));
        // A long drag never locks.
        g.set_aim("hybrid");
        finger(&mut g, 1, x, y, PointerPhase::Begin);
        for _ in 0..30 {
            g.tick(DT);
        }
        finger(&mut g, 1, x, y, PointerPhase::End);
        assert_eq!(g.locked(), None);
    }

    #[test]
    fn auto_ignores_taps_and_the_ground_tap_aims_a_direction() {
        let (mut g, near, far) = two_foes();
        g.set_aim("auto");
        let (x, y) = on_screen(&g, far);
        finger(&mut g, 1, 195.0, 700.0, PointerPhase::Begin);
        finger(&mut g, 2, x, y, PointerPhase::Begin);
        assert_eq!(g.locked(), None);
        g.weapon_cd.iter_mut().for_each(|c| *c = 0.0);
        let yaw = first_shot_yaw(&mut g);
        assert!(yaw_gap(yaw, bearing(&g, near)) < 0.2);

        // Manual: a ground tap aims that way even with foes around.
        let (mut g, near, _) = two_foes();
        g.set_aim("manual");
        finger(&mut g, 1, 195.0, 700.0, PointerPhase::Begin);
        let (gx, gz) = (g.hx, g.hz + 6.0); // north of the hero
        let (sx, sy) = g.project([gx, 0.0, gz], 390.0, 844.0).unwrap();
        finger(&mut g, 2, sx, sy, PointerPhase::Begin);
        assert_eq!(g.locked(), None);
        let a = g.aim_dir.expect("ground aim");
        assert!(yaw_gap(a, 0.0) < 0.1, "aim yaw {a}");
        g.weapon_cd.iter_mut().for_each(|c| *c = 0.0);
        let yaw = first_shot_yaw(&mut g);
        assert!(yaw_gap(yaw, a) < 1e-3);
        assert!(yaw_gap(yaw, bearing(&g, near)) > 0.5);
        assert!(yaw_gap(g.face, yaw) < 1e-3);
    }

    #[test]
    fn hybrid_uses_the_ground_aim_only_when_nothing_is_near() {
        let mut g = started();
        g.weapon_cd.iter_mut().for_each(|c| *c = 0.0);
        g.aim_dir = Some(1.0);
        let yaw = first_shot_yaw(&mut g);
        assert!(yaw_gap(yaw, 1.0) < 1e-3, "empty field: fires along the aim");
        let (mut g, near, _) = two_foes();
        g.aim_dir = Some(1.0);
        g.weapon_cd.iter_mut().for_each(|c| *c = 0.0);
        let yaw = first_shot_yaw(&mut g);
        assert!(yaw_gap(yaw, bearing(&g, near)) < 0.2, "foes in range win");
    }

    #[test]
    fn the_pause_card_cycles_the_aim_and_the_doc_stays_inside_its_pools() {
        let mut g = started();
        g.pause();
        for want in ["auto", "manual", "hybrid"] {
            let (w, h) = (390.0, 844.0);
            let r = pause_buttons(w, h, hud_scale(w, h))
                .into_iter()
                .find(|(b, _)| *b == PauseBtn::Aim)
                .unwrap()
                .1;
            assert!(g.pointer(r.x + r.w * 0.5, r.y + r.h * 0.5, 390, 844));
            assert_eq!(g.aim_name(), want);
            assert!(g.state_value()["aim"] == want);
        }
        g.resume_from_page();
        let (mut g, _, far) = two_foes();
        finger(&mut g, 1, 195.0, 700.0, PointerPhase::Begin);
        let (x, y) = on_screen(&g, far);
        finger(&mut g, 2, x, y, PointerPhase::Begin);
        g.build_doc();
        let ring = g.doc.lines.iter().find(|l| l.id == "lock-ring").unwrap();
        assert_eq!(ring.points.len(), LOCK_SEG + 1);
        assert!(
            g.doc.lines.len()
                == g.spec.caps.shots as usize
                    + 2
                    + g.spec.drops.cap as usize
                    + ZAP_CAP
                    + RING_CAP
                    + 2
        );
        g.tick(DT);
        assert!(g.hud(390, 844, false).texts.iter().any(|t| t.text == "▼"));
    }

    #[test]
    fn the_spec_validates_the_aim_scheme() {
        let mut s = SurvivorSpec::default();
        assert_eq!(s.input.aim, "hybrid");
        assert!(s.validate().is_ok());
        s.input.aim = "psychic".into();
        assert!(s.validate().is_err());
    }

    #[test]
    fn a_damage_number_lands_on_its_foe() {
        let mut g = started();
        g.foes.spawn(0, g.hx + 2.0, g.hz - 3.0, 999.0);
        for _ in 0..180 {
            g.tick(DT);
            if !g.dmg.is_empty() {
                break;
            }
        }
        assert!(!g.dmg.is_empty());
        let d = g.dmg[0];
        let (w, h) = (390.0, 844.0);
        g.hud(390, 844, false);
        let (sx, sy) = g
            .project([d.x, 0.0, d.z], w, h)
            .expect("in front of the camera");
        assert!(
            (0.0..w).contains(&sx) && (0.0..h).contains(&sy),
            "{sx},{sy}"
        );
        // The hero projects to where the camera looks: upper half, centred.
        let (hx, hy) = g.project([g.hx, 0.0, g.hz], w, h).unwrap();
        assert!((hx - w * 0.5).abs() < 4.0 && hy < h * 0.6, "{hx},{hy}");
        let list = g.hud(390, 844, false);
        assert!(list.texts.iter().any(|t| t.text == format!("{}", d.val)));
    }

    #[test]
    fn auto_attack_kills_and_drops_gems() {
        let mut g = started();
        kite(&mut g, 20.0);
        assert!(g.kills() > 3, "kills {}", g.kills());
        assert!(g.gem_positions().count() > 0 || g.level() > 1);
    }

    #[test]
    fn levelling_up_freezes_the_run_until_a_card_is_taken() {
        let mut g = started();
        let mut n = 0;
        while g.card_count() == 0 && n < 60 * 120 && g.flow.phase == FlowPhase::Play {
            g.walk(1.0, 0.0);
            g.tick(DT);
            n += 1;
        }
        assert_eq!(g.card_count(), 3, "three cards offered");
        let ids: Vec<_> = (0..3).map(|i| g.card_id(i).unwrap()).collect();
        assert!(
            ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2],
            "{ids:?}"
        );
        let (t, p) = (g.secs(), g.hero());
        for _ in 0..30 {
            g.tick(DT);
        }
        assert_eq!(g.secs(), t, "the clock waits on the cards");
        assert_eq!(g.hero(), p);
        let before = g.level();
        g.choose(1);
        assert_eq!(g.card_count(), 0);
        g.tick(DT);
        assert!(g.secs() > t);
        assert_eq!(g.level(), before);
    }

    #[test]
    fn taking_a_weapon_or_passive_card_levels_it() {
        let mut g = started();
        let mut taken = 0;
        let mut n = 0;
        while taken < 3 && n < 60 * 300 && g.flow.phase == FlowPhase::Play {
            kite_step(&mut g);
            if g.card_count() > 0 {
                let id = g.card_id(0).unwrap();
                let before = match id.split_once(':') {
                    Some(("weapon", w)) => g.weapon_level(w),
                    Some(("passive", p)) => g.passive_level(p),
                    _ => 0,
                };
                g.choose(0);
                let after = match id.split_once(':') {
                    Some(("weapon", w)) => g.weapon_level(w),
                    Some(("passive", p)) => g.passive_level(p),
                    _ => before + 1,
                };
                assert_eq!(after, before + 1, "{id}");
                taken += 1;
            }
            g.tick(DT);
            n += 1;
        }
        assert!(taken >= 3, "only {taken} level ups");
    }

    #[test]
    fn standing_still_gets_you_swarmed_and_scored() {
        let mut g = started();
        idle(&mut g, 200.0);
        assert_eq!(g.flow.phase, FlowPhase::Result, "idle must die");
        assert!(!g.won());
        assert!(g.secs() < g.spec.run.secs);
        let v = g.state_value();
        assert_eq!(v["result"]["run"]["win"], 0);
        assert!(v["result"]["run"]["score"].as_u64().unwrap() > 0);
        assert_eq!(v["death"], "swarmed");
    }

    #[test]
    fn a_kiting_thumb_lasts_longer_than_standing_still() {
        let mut a = started();
        idle(&mut a, 200.0);
        let mut b = started();
        kite(&mut b, 200.0);
        assert!(
            b.secs() > a.secs() + 15.0 || b.won(),
            "kite {:.1}s idle {:.1}s",
            b.secs(),
            a.secs()
        );
    }

    #[test]
    fn a_decent_run_can_reach_dawn() {
        // Several kiting runs, one seed each: the finale must be reachable.
        let mut wins = 0;
        for seed in 1..=6u32 {
            let mut spec = SurvivorSpec::default();
            spec.director.seed = seed;
            let mut g = SurvivorArena::new(spec);
            g.confirm();
            kite(&mut g, 1000.0);
            wins += u32::from(g.won());
        }
        assert!(
            (1..6).contains(&wins),
            "{wins}/6 reached dawn: balance is off"
        );
    }

    #[test]
    fn pools_stay_inside_their_caps_and_never_reallocate() {
        let mut g = started();
        let (fx, fc) = (g.foes().x.as_ptr(), g.foes().cap());
        let (sp, gp) = (g.shots.as_ptr(), g.gems.as_ptr());
        let (pk, ob, sc) = (g.pickups.as_ptr(), g.orbs.as_ptr(), g.scratch.as_ptr());
        let mut most = 0;
        kite_each(&mut g, 100.0, |g| {
            most = most.max(g.foes().len());
            assert!(g.foes().len() <= fc);
            assert!(g.shots.len() <= g.spec.caps.shots as usize);
            assert!(g.gems.len() <= g.spec.caps.gems as usize);
        });
        assert!(most > 12, "a swarm should build ({most})");
        assert_eq!(fx, g.foes().x.as_ptr());
        assert_eq!(sp, g.shots.as_ptr());
        assert_eq!(gp, g.gems.as_ptr());
        assert_eq!(pk, g.pickups.as_ptr());
        assert_eq!(ob, g.orbs.as_ptr());
        assert_eq!(sc, g.scratch.as_ptr());
        assert_eq!(g.document().props.len(), g.doc.props.len());
    }

    #[test]
    fn the_picture_keeps_a_fixed_prop_list() {
        let mut g = started();
        g.build_doc();
        let n0 = g.document().props.len();
        assert!(n0 > 100, "{n0}");
        kite(&mut g, 30.0);
        g.build_doc();
        assert_eq!(g.document().props.len(), n0);
        assert!(g.doc.props.iter().filter(|p| p.name == "decor").count() > 10);
        assert!(g
            .document()
            .props
            .iter()
            .any(|p| p.enabled && p.name == "foe"));
        assert!(!g.document().cameras.is_empty());
    }

    #[test]
    fn a_finger_drags_the_hero_toward_it() {
        let mut g = started();
        let (w, h) = (800u32, 450u32);
        g.hud(w, h, false);
        let p0 = g.hero();
        let right = |x: f32| ev(PointerPhase::Begin, x, 225.0);
        g.touch_ev(right(700.0), w, h);
        for _ in 0..90 {
            g.tick(DT);
        }
        let p1 = g.hero();
        assert!(p1.0 > p0.0 + 2.0, "followed right: {p0:?} -> {p1:?}");
        g.touch_ev(ev(PointerPhase::End, 700.0, 225.0), w, h);
        let rest = g.hero();
        for _ in 0..60 {
            g.tick(DT);
        }
        let d = (g.hero().0 - rest.0).hypot(g.hero().1 - rest.1);
        assert!(d < 1.0, "the hero stops when the finger lifts ({d})");
    }

    #[test]
    fn a_finger_that_picked_a_card_does_not_also_steer() {
        let mut g = started();
        let mut n = 0;
        while g.card_count() == 0 && n < 60 * 120 {
            g.tick(DT);
            n += 1;
        }
        assert!(g.card_count() > 0);
        let (w, h) = (800u32, 450u32);
        g.hud(w, h, false);
        let s = hud_scale(w as f32, h as f32);
        let r = card_rects(w as f32, h as f32, s, g.card_count())[0];
        let (cx, cy) = (r.x + r.w * 0.5, r.y + r.h * 0.5);
        let ev0 = ev(PointerPhase::Begin, cx, cy);
        g.touch_ev(ev0, w, h);
        assert!(g.pointer(cx, cy, w, h) || g.card_count() == 0);
        assert_eq!(g.card_count(), 0, "a tap on a card takes it");
        let p = g.hero();
        g.touch_ev(ev(PointerPhase::Move, cx + 200.0, cy), w, h);
        for _ in 0..30 {
            g.tick(DT);
        }
        let d = (g.hero().0 - p.0).hypot(g.hero().1 - p.1);
        assert!(d < 0.6, "muted until it lifts ({d})");
    }

    #[test]
    fn pause_freezes_and_resumes() {
        let mut g = started();
        for _ in 0..30 {
            g.tick(DT);
        }
        g.pause();
        assert_eq!(g.flow.phase, FlowPhase::Pause);
        let t = g.secs();
        for _ in 0..30 {
            g.tick(DT);
        }
        assert_eq!(g.secs(), t);
        g.resume_from_page();
        g.tick(DT);
        assert!(g.secs() > t);
    }

    #[test]
    fn result_feeds_the_meta_layer() {
        let mut g = started();
        idle(&mut g, 200.0);
        let r = g.result_value();
        let run = &r["run"];
        for k in ["score", "time", "kills", "level", "win"] {
            assert!(run.get(k).is_some(), "{k}");
        }
        assert_eq!(r["sort"], "score");
        assert!(r["columns"]
            .as_array()
            .unwrap()
            .iter()
            .all(|c| run.get(c.as_str().unwrap()).is_some()));
    }

    #[test]
    fn the_session_drives_it_end_to_end() {
        use crate::session::SharedSession;
        let json = include_str!("../tests/fixtures/survivor_arena_world.json");
        let mut s = SharedSession::default();
        s.load_game_json(json).unwrap();
        s.create_surface(800, 450);
        assert!(s.survivor_mut().is_some() && s.hop().is_none());
        assert!(s.uses_swipe());
        s.set_direct_play(true);
        // A tap on the title starts the run.
        s.push_pointer(ev(PointerPhase::Begin, 400.0, 225.0));
        s.push_pointer(ev(PointerPhase::End, 400.0, 225.0));
        s.request_frame();
        assert_eq!(s.survivor_mut().unwrap().phase_name(), "play");
        // Hold a finger to the right: the hero follows over some frames.
        s.push_pointer(ev(PointerPhase::Begin, 700.0, 225.0));
        for _ in 0..90 {
            s.request_frame();
        }
        assert!(s.survivor_mut().unwrap().hero().0 > 1.0);
        // Hidden tab: pause, then back.
        s.pause_run();
        assert_eq!(s.survivor_mut().unwrap().phase_name(), "pause");
        s.resume_run();
        assert_eq!(s.survivor_mut().unwrap().phase_name(), "play");
        s.set_lang("ja");
    }

    #[test]
    fn the_shipped_manifest_matches_the_default_spec() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../games/survivor_arena_minimal.json"
        );
        let m: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let heroes: Vec<&Value> = m["meta"]["catalog"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["slot"] == "hero")
            .collect();
        let d = heroes
            .iter()
            .find(|i| i["price"] == 0)
            .expect("a default hero");
        let spec = SurvivorSpec::default();
        assert_eq!(d["patch"]["hero"]["skin"], json!(spec.hero.skin));
        assert_eq!(d["patch"]["hero"]["start_passives"], json!([]));
        // Every earn key is a key the kit reports.
        let run = fresh().result_value()["run"].clone();
        for k in m["meta"]["earn"]["per"].as_object().unwrap().keys() {
            assert!(run.get(k).is_some(), "earn.per.{k} is not in result.run");
        }
        // Every hero perk names a real passive.
        for it in heroes {
            if let Some(sp) = it["patch"]["hero"]["start_passives"].as_array() {
                for p in sp {
                    let id = p["id"].as_str().unwrap();
                    assert!(spec.passives.iter().any(|x| x.id == id), "{id}");
                }
            }
        }
    }
}

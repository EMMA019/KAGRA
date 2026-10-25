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
use crate::game_core::enemies::{weighted_pick, EnemySpec, EnemyType, SwarmStats};
use crate::game_core::swarm::{Budget, Foes, Grid, Rng};
use crate::game_core::{tick_frame, Core, Kit};
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::world_doc::{WorldCamera, WorldDoc};

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
    fn new(en: &str, ja: &str) -> Self {
        Self {
            en: en.into(),
            ja: ja.into(),
        }
    }

    fn get(&self, ja: bool) -> &str {
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
}

impl Default for InputSpec {
    fn default() -> Self {
        Self {
            mode: "follow".into(),
            thumb_offset_m: 2.0,
            dead_zone_m: 0.3,
            full_speed_m: 2.2,
            stick_px: 70.0,
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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponSpec {
    pub id: String,
    pub name: Loc,
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
    pub weapons: Vec<WeaponSpec>,
    pub passives: Vec<PassiveSpec>,
    /// Sound cues for the page (`game.audio`: file names under the bundle). Passed through.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<Value>,
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

fn lv(en: &str, ja: &str) -> WeaponLevel {
    WeaponLevel {
        desc: Loc::new(en, ja),
        ..WeaponLevel::default()
    }
}

impl Default for SurvivorSpec {
    /// The shipped step-1 game: one hero, the moon bolt, the ghoul swarm, six passives.
    fn default() -> Self {
        let bolt = WeaponSpec {
            id: "bolt".into(),
            name: Loc::new("Moon bolt", "つきのたま"),
            kind: WeaponKind::Projectile,
            base: WeaponBase::default(),
            levels: vec![
                WeaponLevel {
                    count: 1,
                    ..lv("+1 bolt per cast", "いちどに +1こ")
                },
                WeaponLevel {
                    damage: 3.0,
                    ..lv("Damage +3", "ダメージ +3")
                },
                WeaponLevel {
                    cooldown: -0.15,
                    count: 1,
                    ..lv("Casts faster, +1 bolt", "はやくなげる、+1こ")
                },
                WeaponLevel {
                    pierce: 2,
                    damage: 2.0,
                    ..lv("Pierces two more, +2 damage", "ぬける +2、ダメージ +2")
                },
            ],
        };
        let ghoul = EnemyType {
            id: "ghoul".into(),
            look: "ghoul".into(),
            color: [120, 240, 150],
            swarm: Some(SwarmStats {
                hp: 4.0,
                speed: 1.7,
                touch_dmg: 6.0,
                ..SwarmStats::default()
            }),
            ..EnemyType::default()
        };
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
                types: vec![ghoul],
                ..EnemySpec::default()
            },
            weapons: vec![bolt],
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
        self.enemies.validate(w, &["ghoul"])?;
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
        }
    }

    fn add(&mut self, l: &WeaponLevel) {
        self.damage += l.damage;
        self.cooldown += l.cooldown;
        self.speed += l.speed;
        self.range += l.range;
        self.count += l.count;
        self.pierce += l.pierce;
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Card {
    Weapon(u8),
    Passive(u8),
    Heal,
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
    foe0: usize,
    /// Zombie pool start and size (size 0: no zombie model, every foe is a ghost).
    zombie0: usize,
    zombie_cap: usize,
    ghost_cap: usize,
    gem0: usize,
    shot0: usize,
    glb_foe: bool,
    glb_gem: bool,
    /// How many foe props each pool drew last frame (to hide the leftovers).
    drawn_ghost: usize,
    drawn_zombie: usize,
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
/// Per kind: sRGB colour, sprite radius (m) and opacity for the young, middle and old third
/// of its life. One point cloud per kind and third, refilled in place each frame.
const FX_LOOK: [([u8; 3], [f32; 3], [u8; 3]); 6] = [
    ([185, 240, 255], [0.11, 0.08, 0.05], [255, 235, 190]),
    ([185, 165, 255], [0.16, 0.26, 0.34], [210, 130, 60]),
    ([140, 225, 150], [0.16, 0.26, 0.34], [210, 130, 60]),
    ([255, 225, 110], [0.10, 0.08, 0.05], [255, 230, 160]),
    ([255, 225, 160], [0.28, 0.18, 0.10], [255, 200, 110]),
    ([255, 85, 95], [0.16, 0.12, 0.08], [255, 230, 160]),
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
    key_dir: (f32, f32),
    size: Cell<(f32, f32)>,
    // presentation
    lang_ja: bool,
    sfx: bool,
    cue_t: [f32; 4],
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
                Kind {
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
            key_dir: (0.0, 0.0),
            size: Cell::new((390.0, 844.0)),
            lang_ja: false,
            sfx: true,
            cue_t: [0.0; 4],
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
        for c in &mut self.cue_t {
            *c = (*c - dt).max(0.0);
        }
        self.hurt_flash = (self.hurt_flash - dt * 3.0).max(0.0);
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
        self.move_hero(dt);
        self.spawn(dt);
        self.grid.rebuild(&self.foes.x, &self.foes.z);
        self.move_foes(dt);
        self.fire(dt);
        self.move_shots(dt);
        self.reap();
        self.collect(dt);
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
            if k.unlock <= self.t && m < 8 {
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
            if self.foes.spawn(kind as u8, pos.0, pos.1, hp).is_none() {
                break; // the cap is the budget: the rest of this step's births are dropped
            }
        }
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
            let (mut vx, mut vz) = (dx / d * sp, dz / d * sp);
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

    fn fire(&mut self, dt: f32) {
        for wi in 0..self.spec.weapons.len() {
            if self.weapon_lv[wi] == 0 {
                continue;
            }
            self.weapon_cd[wi] -= dt;
            if self.weapon_cd[wi] > 0.0 {
                continue;
            }
            let st = self.weapon_stats[wi];
            let Some((j, _)) =
                self.grid
                    .nearest(&self.foes.x, &self.foes.z, self.hx, self.hz, st.range)
            else {
                self.weapon_cd[wi] = 0.1; // nothing in range: look again soon
                continue;
            };
            self.weapon_cd[wi] = (st.cooldown * self.mods.cooldown).max(0.08);
            let (dx, dz) = (self.foes.x[j] - self.hx, self.foes.z[j] - self.hz);
            let base = dx.atan2(dz);
            // Turn to the target first: the blaster points where the shot goes, and the
            // shot leaves from the muzzle (nearer if the foe is already in the way).
            self.face = base;
            self.aim_hold = AIM_HOLD;
            let reach = MUZZLE.min(dx.hypot(dz) * 0.6);
            let n = st.count;
            for k in 0..n {
                if self.shots.len() >= self.spec.caps.shots as usize {
                    break;
                }
                let a = base + (k as f32 - (n - 1) as f32 * 0.5) * st.spread;
                self.shots.push(Shot {
                    x: self.hx + a.sin() * reach,
                    z: self.hz + a.cos() * reach,
                    vx: a.sin() * st.speed,
                    vz: a.cos() * st.speed,
                    life: st.range / st.speed,
                    dmg: st.damage * self.mods.damage,
                    pierce: st.pierce,
                    r: st.size,
                    hits: [0; 4],
                    n: 0,
                });
            }
            self.cue("shoot", 0, 0.12);
            let (sn, cs) = base.sin_cos();
            let at = [self.hx + sn * reach, MUZZLE_Y, self.hz + cs * reach];
            self.burst(FX_MUZZLE, 1, at, [0.0, 0.0], 0.0, 0.0, 0.0, 0.09);
            self.burst(FX_SPARK, 2, at, [sn, cs], 5.0, 0.5, 0.5, 0.18);
        }
    }

    fn move_shots(&mut self, dt: f32) {
        let lim = self.spec.arena.half + 1.0;
        let mut hit_cue = false;
        for si in (0..self.shots.len()).rev() {
            let s = &mut self.shots[si];
            s.x += s.vx * dt;
            s.z += s.vz * dt;
            s.life -= dt;
            if s.life <= 0.0 || s.x.abs() > lim || s.z.abs() > lim {
                self.shots.swap_remove(si);
                continue;
            }
            let (sx, sz, sr) = (s.x, s.z, s.r);
            let (hits, nh) = (s.hits, s.n);
            let mut found: Option<usize> = None;
            let (foes, kinds) = (&self.foes, &self.kinds);
            self.grid.query(sx, sz, sr + MAX_FOE_R, |j| {
                if found.is_some() {
                    return;
                }
                let rs = sr + kinds[foes.kind[j] as usize].radius;
                let (dx, dz) = (foes.x[j] - sx, foes.z[j] - sz);
                if dx * dx + dz * dz < rs * rs && !hits[..nh as usize].contains(&foes.uid[j]) {
                    found = Some(j);
                }
            });
            if let Some(j) = found {
                let s = &mut self.shots[si];
                let k = 1.0 / s.vx.hypot(s.vz).max(1e-3);
                self.foes.hp[j] -= s.dmg;
                self.foes.flash[j] = 0.1;
                let kb = self.spec.fx.knockback;
                self.foes.x[j] += s.vx * k * kb;
                self.foes.z[j] += s.vz * k * kb;
                hit_cue = true;
                let (dirx, dirz, dmg_val) = (-s.vx * k, -s.vz * k, s.dmg);
                let (fx_x, fx_z) = (self.foes.x[j], self.foes.z[j]);
                if self.spec.fx.damage_numbers {
                    if self.dmg.len() >= DMG_CAP[self.tier] {
                        self.dmg.remove(0);
                    }
                    self.dmg.push(DmgNum {
                        x: fx_x,
                        z: fx_z,
                        val: dmg_val.round().max(1.0) as i32,
                        age: 0.0,
                    });
                }
                self.burst(FX_SPARK, 4, [sx, 0.6, sz], [dirx, dirz], 5.5, 0.6, 1.8, 0.3);
                let s = &mut self.shots[si];
                if (s.n as usize) < s.hits.len() {
                    s.hits[s.n as usize] = self.foes.uid[j];
                    s.n += 1;
                }
                s.pierce -= 1;
                if s.pierce < 0 {
                    self.shots.swap_remove(si);
                }
            }
        }
        if hit_cue {
            self.cue("hit", 1, 0.07);
        }
    }

    fn reap(&mut self) {
        for i in (0..self.foes.len()).rev() {
            if self.foes.hp[i] > 0.0 {
                continue;
            }
            let k = self.kinds[self.foes.kind[i] as usize];
            self.kills += 1;
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
        let mut pool: Vec<(Card, u32)> = Vec::with_capacity(16);
        for (i, w) in self.spec.weapons.iter().enumerate() {
            let cur = self.weapon_lv[i];
            if cur > 0 && (cur as usize) < w.levels.len() + 1 {
                pool.push((Card::Weapon(i as u8), lv.weapon_weight.max(1)));
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
        match card {
            Card::Weapon(i) => self.weapon_lv[i as usize] += 1,
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
        self.pending_levels = self.pending_levels.saturating_sub(1);
        self.cue("pick", 2, 0.1);
    }

    /// (kind label, name, description, accent colour) of a card, in the page language.
    /// A glyph from the bundled UI font for a card.
    fn card_glyph(&self, c: Card) -> &'static str {
        match c {
            Card::Weapon(_) => "★",
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
            Card::Weapon(i) => (
                self.weapon_lv[i as usize] as usize,
                self.spec.weapons[i as usize].levels.len() + 1,
            ),
            Card::Passive(i) => (
                self.passive_lv[i as usize] as usize,
                self.spec.passives[i as usize].max_level as usize,
            ),
            Card::Heal => (0, 0),
        }
    }

    fn card_text(&self, c: Card) -> (String, String, String, [u8; 3]) {
        match c {
            Card::Weapon(i) => {
                let w = &self.spec.weapons[i as usize];
                let cur = self.weapon_lv[i as usize] as usize;
                let d = w
                    .levels
                    .get(cur - 1)
                    .map(|l| l.desc.get(self.lang_ja).to_string())
                    .unwrap_or_default();
                (
                    format!("{} LV {}", self.tr("WEAPON", "ぶき"), cur + 1),
                    w.name.get(self.lang_ja).to_string(),
                    d,
                    [255, 170, 80],
                )
            }
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
                }
            }
            PointerPhase::Move => {
                if self.finger_id == Some(ev.id) && !self.mute_finger {
                    self.finger = Some((ev.x, ev.y));
                }
            }
            PointerPhase::End | PointerPhase::Cancel if self.finger_id == Some(ev.id) => {
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

fn hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PauseBtn {
    Resume,
    Retry,
    Title,
}

fn pause_rect(w: f32, s: f32) -> Quad {
    let side = 46.0 * s;
    Quad::new(w - side - 8.0 * s, 22.0 * s, side, side, [0, 0, 0, 0])
}

fn pause_buttons(w: f32, h: f32, s: f32) -> Vec<(PauseBtn, Quad)> {
    let bw = (w * 0.7).min(320.0 * s);
    let bh = 54.0 * s;
    let x = (w - bw) * 0.5;
    let y0 = h * 0.36;
    [PauseBtn::Resume, PauseBtn::Retry, PauseBtn::Title]
        .into_iter()
        .enumerate()
        .map(|(i, b)| {
            (
                b,
                Quad::new(x, y0 + i as f32 * (bh + 14.0 * s), bw, bh, [0; 4]),
            )
        })
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
            "hit": cue("retro_explosion_02.wav", 0.14, 1.6),
            "hurt": cue("retro_die_01.wav", 0.6, 1.0),
            "gem": cue("retro_coin_01.wav", 0.25, 1.3),
            "levelup": cue("power_up_02.wav", 0.7, 1.0),
            "pick": {"file": "assets/audio/kenney-interface-sounds/confirmation_001.wav", "volume": 0.7, "rate": 1.0},
            "win": cue("power_up_04.wav", 0.8, 1.0),
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
/// Kenney Platformer Kit jewel (CC0): the XP gem.
/// Kenney Blaster Kit (CC0): the hero's blaster. The model's muzzle is at -Z.
const BLASTER_GLB: &str = "assets/models/kenney/blaster-kit/blaster-a.glb";
/// Where the blaster sits in front of the hero (metres along the aim, and up) and how
/// big it is drawn. The muzzle is `GUN_FWD + 0.37 * GUN_SCALE` ahead of the hero.
const GUN_FWD: f32 = 0.86;
const GUN_UP: f32 = 0.9;
const GUN_SCALE: f32 = 1.3;
/// Height of the muzzle above the floor (m).
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
const GHOST_TINT: [u32; 3] = [150, 150, 225];
const ZOMBIE_TINT: [u32; 3] = [205, 225, 190];
const HERO_MODEL_SCALE: f32 = 2.2;

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
        let glb_zombie = glb_foe && has(ZOMBIE_GLB);
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
        self.slots.ghost_cap = if glb_zombie { cap * 85 / 100 + 4 } else { cap };
        self.slots.zombie_cap = if glb_zombie { cap * 40 / 100 + 4 } else { 0 };
        let foe_prop = |id: String, path: &str| {
            let mut p = if glb_foe {
                glb_prop(
                    &id,
                    path,
                    "foe",
                    [0.0, -50.0, 0.0],
                    [1.0; 3],
                    if path == ZOMBIE_GLB {
                        ZOMBIE_TINT
                    } else {
                        GHOST_TINT
                    },
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
        self.slots.foe0 = doc.props.len();
        for i in 0..self.slots.ghost_cap {
            doc.props.push(foe_prop(format!("foe-{i}"), GHOST_GLB));
        }
        self.slots.zombie0 = doc.props.len();
        for i in 0..self.slots.zombie_cap {
            doc.props.push(foe_prop(format!("zombie-{i}"), ZOMBIE_GLB));
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
        let (mut ng, mut nz) = (0usize, 0usize);
        for i in 0..n {
            let zombie = slots.zombie_cap > 0 && self.foes.uid[i].is_multiple_of(4);
            let idx = if zombie {
                if nz >= slots.zombie_cap {
                    continue;
                }
                nz += 1;
                slots.zombie0 + nz - 1
            } else {
                if ng >= slots.ghost_cap {
                    continue;
                }
                ng += 1;
                slots.foe0 + ng - 1
            };
            let p = &mut props[idx];
            let k = &self.kinds[self.foes.kind[i] as usize];
            let age = self.foes.age[i];
            let pop = (age / 0.25).min(1.0);
            let wob = (age * 9.0 + self.foes.uid[i] as f32).sin();
            let m = if slots.glb_foe { FOE_MODEL_K } else { 2.2 };
            let d = k.radius * m * (0.4 + 0.6 * pop);
            let (sx, sy) = (1.0 - 0.07 * wob, 1.0 + 0.1 * wob);
            let (x, z) = (self.foes.x[i], self.foes.z[i]);
            p.enabled = true;
            // The ghost floats a little; the zombie shambles on the ground.
            let float = if slots.glb_foe && !zombie {
                0.12 + 0.06 * wob
            } else {
                0.0
            };
            p.position = [x, if slots.glb_foe { float } else { d * 0.4 }, z];
            p.yaw = (hx - x).atan2(hz - z);
            p.scale = [d * sx, d * sy, d * sx];
            let fl = self.foes.flash[i] * 10.0;
            p.emissive = Some(0.06 + 2.2 * fl);
            p.emissive_color = Some(if fl > 0.0 { [1.0, 1.0, 1.0] } else { k.linear });
            if !slots.glb_foe {
                p.color = Some(k.color);
            }
        }
        for j in ng..slots.drawn_ghost {
            props[slots.foe0 + j].enabled = false;
        }
        for j in nz..slots.drawn_zombie {
            props[slots.zombie0 + j].enabled = false;
        }
        slots.drawn_ghost = ng;
        slots.drawn_zombie = nz;
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
            p.enabled = true;
            p.position = [s.x, 0.55, s.z];
            p.yaw = s.vx.atan2(s.vz);
            p.scale = [s.r * 0.8, s.r * 0.8, s.r * 2.0];
        }
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
        for (i, l) in self.doc.lines.iter_mut().enumerate() {
            l.points.clear();
            if let (true, Some(s)) = (streak, self.shots.get(i)) {
                l.points.push([s.x - s.vx * 0.05, 0.58, s.z - s.vz * 0.05]);
                l.points.push([s.x, 0.58, s.z]);
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
            "cards": self.card_values(),
            "win": self.win,
            "death": self.core.stats.death,
            "death_text": self.death_text,
            "revives": 0,
            "result": if self.flow.phase == FlowPhase::Result { self.result_value() } else { Value::Null },
            "pos": [self.hx, self.hz],
        })
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
        // The thumb marker.
        if let Some((fx, fy)) = self.finger {
            let d = 16.0 * s;
            list.quads.push(Quad::new(
                fx - d * 0.5,
                fy - d * 0.5,
                d,
                d,
                [255, 255, 255, 70],
            ));
            if self.spec.input.mode == "stick" {
                let o = self.origin;
                let d2 = 44.0 * s;
                list.quads.push(Quad::new(
                    o.0 - d2 * 0.5,
                    o.1 - d2 * 0.5,
                    d2,
                    d2,
                    [255, 255, 255, 40],
                ));
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
                self.tr("LEVEL UP!", "レベルアップ！"),
                w * 0.5,
                h * 0.09,
                32.0 * s,
                [255, 240, 160, 255],
            )
            .aligned(TextAlign::Center),
        );
        list.texts.push(
            TextQuad::new(
                self.tr("Pick one", "ひとつえらんでね"),
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
                let at = (0..g.card_count())
                    .find(|i| g.card_id(*i).is_some_and(|c| c.starts_with("weapon")))
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

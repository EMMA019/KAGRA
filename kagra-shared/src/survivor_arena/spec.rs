//! The survivor spec: every tunable, its defaults and its checks (`SurvivorSpec` is the `game` block of the world file).

use super::*;

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
    /// How dark the night is: 0 = the old blue-lit arena, 1 = dim moonlight (darker floor,
    /// walls and fill, a tighter key light, closer fog) where the pale foes glow out of it.
    pub dark: f32,
}

impl Default for ArenaSpec {
    fn default() -> Self {
        Self {
            half: 13.0,
            floor: [34, 46, 66],
            wall: [84, 70, 128],
            dark: 1.0,
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
    /// `stick` (a fixed on-screen joystick bottom-left, analog, the default) or `drag` (the hero
    /// goes to the finger). The old `mode` key and `follow` still read as `drag`.
    #[serde(rename = "move", alias = "mode")]
    pub move_scheme: String,
    /// Follow: the hero aims this far (m) above the finger, so the thumb does not cover it.
    pub thumb_offset_m: f32,
    /// Follow: closer than this (m) the hero stands still.
    pub dead_zone_m: f32,
    /// Follow: at this distance (m) the hero runs at full speed.
    pub full_speed_m: f32,
    /// Stick: radius in pixels at 400 px of short screen side.
    pub stick_px: f32,
    /// Stick: the inner fraction of the radius that does not move the hero.
    pub stick_dead: f32,
    /// Who picks the target: `auto` (nearest foe), `manual` (tap a foe to lock it, tap the
    /// ground to aim that way) or `hybrid` (a locked foe first, else nearest).
    pub aim: String,
    /// Manual and hybrid: a tap picks a foe within this many pixels (at 400 px short side).
    pub tap_px: f32,
}

impl Default for InputSpec {
    fn default() -> Self {
        Self {
            move_scheme: "stick".into(),
            thumb_offset_m: 2.0,
            dead_zone_m: 0.3,
            full_speed_m: 2.2,
            stick_px: 70.0,
            stick_dead: 0.12,
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
    /// A warden should take about this many seconds to bring down at the hero's current
    /// firepower (half of it landing), never less than its own HP and never more than
    /// `elite_hp_cap` times that.
    pub elite_fight_secs: f32,
    pub elite_hp_cap: f32,
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
            elite_fight_secs: 14.0,
            elite_hp_cap: 4.0,
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
    /// Fraction of the triangles of the foe models kept on the low, mid and high picture
    /// tiers (vertex clustering at load; 1 = full detail). A crowd of small figures does not
    /// need a thousand triangles each.
    pub foe_detail: [f32; 3],
    /// Body motion of the foes (bob, squash, sway, lean, a flinch on a hit), each model its own
    /// way and each foe its own phase: 0 = still figures, 1 = as tuned, up to 2. Picture only.
    pub foe_motion: f32,
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
            foe_detail: [0.5, 0.65, 1.0],
            foe_motion: 1.0,
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

pub(super) fn default_weapon_glyph() -> String {
    "★".into()
}

pub(super) fn default_weapon_color() -> [u32; 3] {
    [255, 170, 80]
}

pub(super) fn passive(
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

pub(super) fn range(who: &str, name: &str, v: f32, lo: f32, hi: f32) -> Result<(), String> {
    if v.is_finite() && (lo..=hi).contains(&v) {
        Ok(())
    } else {
        Err(format!("{who}: {name} must be {lo}..={hi}"))
    }
}

pub(super) fn id_ok(id: &str) -> bool {
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
    pub(super) fn validate_content(&self, w: &str) -> Result<(), String> {
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
        for (i, d) in self.fx.foe_detail.iter().enumerate() {
            range(w, &format!("fx.foe_detail[{i}]"), *d, 0.2, 1.0)?;
        }
        range(w, "fx.foe_motion", self.fx.foe_motion, 0.0, 2.0)?;
        range(
            w,
            "director.elite_fight_secs",
            self.director.elite_fight_secs,
            0.0,
            120.0,
        )?;
        range(
            w,
            "director.elite_hp_cap",
            self.director.elite_hp_cap,
            1.0,
            20.0,
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
        range(w, "arena.dark", self.arena.dark, 0.0, 1.0)?;
        range(w, "hero.speed", self.hero.speed, 1.0, 12.0)?;
        range(w, "hero.max_hp", self.hero.max_hp, 1.0, 10_000.0)?;
        range(w, "hero.radius", self.hero.radius, 0.1, 1.0)?;
        range(w, "hero.pickup_radius", self.hero.pickup_radius, 0.3, 10.0)?;
        range(w, "hero.hurt_cd", self.hero.hurt_cd, 0.0, 5.0)?;
        if !matches!(self.input.move_scheme.as_str(), "drag" | "follow" | "stick") {
            return Err(format!("{w}: input.move is stick|drag"));
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
        range(w, "input.stick_dead", self.input.stick_dead, 0.0, 0.6)?;
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

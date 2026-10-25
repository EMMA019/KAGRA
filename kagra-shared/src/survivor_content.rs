//! Content tables for the survivor arena (Moonlit Swarm): the drop tables and pickups, the foe
//! roster rows, the weapon x passive synergies, and the shipped defaults for all of them.
//!
//! Everything here is plain data the game JSON can replace (`game.drops`, `game.foes`,
//! `game.synergies`, plus the new `game.weapons` kinds). The sim in [`crate::survivor_arena`]
//! reads these tables; it never hard-codes an id.

use crate::game_core::enemies::{EnemyType, SwarmStats};
use crate::survivor_arena::{lv, Loc, WeaponBase, WeaponKind, WeaponLevel, WeaponSpec};
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------- foes -----

/// How a foe moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FoeAi {
    /// Walks straight at the hero.
    #[default]
    Chase,
    /// Keeps `hold_m` away and throws a slow orb every `shoot_every` seconds.
    Shoot,
}

/// Survivor-only extras for one enemy type (`foes[i]` goes with `enemies.types[i]` by `id`).
/// The shared `EnemyType` keeps hp / speed / radius / contact damage / xp / unlock time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FoeSpec {
    /// The `enemies.types[].id` this row belongs to.
    pub id: String,
    pub ai: FoeAi,
    /// Model size multiplier on top of the radius.
    pub scale: f32,
    /// Albedo tint of the model (the kit's models are pale and burn out under the moon).
    pub tint: [u32; 3],
    /// Sideways weave, 0 (straight) .. 1 (a strong zigzag).
    pub weave: f32,
    /// Flat damage taken off each bolt, pellet, blade and zap hit (never below 30% of the hit).
    /// Blasts, pulses and bombs ignore it.
    pub armor: f32,
    /// Share of knockback and shove it ignores, 0..1.
    pub knock_resist: f32,
    /// Shooter: the distance (m) it likes to keep.
    pub hold_m: f32,
    /// Shooter: seconds between orbs.
    pub shoot_every: f32,
    /// Shooter: damage of one orb.
    pub shot_dmg: f32,
    /// Shooter: metres per second of an orb.
    pub shot_speed: f32,
    /// Which `drops.tables[].id` it rolls on death (empty: nothing drops).
    pub table: String,
    /// Multiplies that table's chance.
    pub drop_mul: f32,
    /// Not part of the weighted crowd: the director sends it on a timer (`director.elite_*`).
    pub elite: bool,
}

impl Default for FoeSpec {
    fn default() -> Self {
        Self {
            id: "ghoul".into(),
            ai: FoeAi::Chase,
            scale: 1.0,
            tint: [255, 255, 255],
            weave: 0.0,
            armor: 0.0,
            knock_resist: 0.0,
            hold_m: 6.0,
            shoot_every: 2.6,
            shot_dmg: 7.0,
            shot_speed: 4.6,
            table: "common".into(),
            drop_mul: 1.0,
            elite: false,
        }
    }
}

// ------------------------------------------------------------ drops -----

/// What a pickup does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickupKind {
    /// Grants a weapon, or upgrades one (a swap choice when the slots are full).
    Weapon,
    /// Heals `amount` of max HP.
    Heal,
    /// Pulls every gem and pickup to the hero.
    Magnet,
    /// `amount` XP at once.
    Xp,
    /// `amount` damage to every foe within `area` metres (armor ignored).
    Bomb,
    /// No damage taken for `secs`.
    Shield,
    /// Weapons cool down `amount` faster for `secs`.
    Frenzy,
}

/// One row of the pickup catalogue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ItemRow {
    pub id: String,
    pub kind: PickupKind,
    /// 0 common, 1 uncommon, 2 rare. Elite tables ask for a minimum.
    pub rarity: u8,
    /// Draw weight inside a table.
    pub weight: u32,
    pub amount: f32,
    pub secs: f32,
    pub area: f32,
    pub color: [u32; 3],
    /// One glyph from the bundled UI font, drawn over the pickup and on the HUD chip.
    pub glyph: String,
    pub name: Loc,
}

impl Default for ItemRow {
    fn default() -> Self {
        Self {
            id: "heal".into(),
            kind: PickupKind::Heal,
            rarity: 0,
            weight: 10,
            amount: 0.3,
            secs: 0.0,
            area: 0.0,
            color: [255, 255, 255],
            glyph: "★".into(),
            name: Loc::default(),
        }
    }
}

/// A drop table: what a dying foe may leave behind.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DropTable {
    pub id: String,
    /// Chance of a drop per death, before the foe's `drop_mul`. 1.0 always drops.
    pub chance: f32,
    /// Items inside one drop (a chest holds this many).
    pub rolls: u32,
    /// Only items of at least this rarity are drawn.
    pub rarity_min: u8,
    /// The drop is a treasure chest: it opens on touch and gives all `rolls` at once.
    pub chest: bool,
    /// An item id the first roll always is ("" none).
    pub guarantee: String,
}

impl Default for DropTable {
    fn default() -> Self {
        Self {
            id: "common".into(),
            chance: 0.07,
            rolls: 1,
            rarity_min: 0,
            chest: false,
            guarantee: String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DropSpec {
    /// Off: nothing ever drops (the step-1 game).
    pub enabled: bool,
    /// Pickups alive at once; a drop that does not fit is dropped.
    pub cap: u32,
    /// Seconds a pickup lies there (chests last four times as long).
    pub life: f32,
    /// The last seconds it blinks.
    pub blink: f32,
    /// Pickups within this many metres of the hero (times the Magnet passive) fly to it.
    pub pull_m: f32,
    /// This many kills without a drop makes the next kill drop (0 off).
    pub pity_kills: u32,
    pub items: Vec<ItemRow>,
    pub tables: Vec<DropTable>,
}

impl Default for DropSpec {
    fn default() -> Self {
        let row = |id: &str,
                   kind: PickupKind,
                   rarity: u8,
                   weight: u32,
                   amount: f32,
                   secs: f32,
                   area: f32,
                   color: [u32; 3],
                   glyph: &str,
                   en: &str,
                   ja: &str| ItemRow {
            id: id.into(),
            kind,
            rarity,
            weight,
            amount,
            secs,
            area,
            color,
            glyph: glyph.into(),
            name: Loc::new(en, ja),
        };
        Self {
            enabled: true,
            cap: 16,
            life: 16.0,
            blink: 4.0,
            pull_m: 2.4,
            pity_kills: 24,
            items: vec![
                row(
                    "heal",
                    PickupKind::Heal,
                    0,
                    30,
                    0.30,
                    0.0,
                    0.0,
                    [120, 235, 150],
                    "♥",
                    "Rice ball",
                    "おにぎり",
                ),
                row(
                    "magnet",
                    PickupKind::Magnet,
                    0,
                    22,
                    0.0,
                    0.0,
                    0.0,
                    [110, 200, 255],
                    "∪",
                    "Magnet",
                    "じしゃく",
                ),
                row(
                    "dust",
                    PickupKind::Xp,
                    0,
                    16,
                    4.0,
                    0.0,
                    0.0,
                    [150, 160, 255],
                    "◇",
                    "Moon dust",
                    "つきのこ",
                ),
                row(
                    "bomb",
                    PickupKind::Bomb,
                    1,
                    12,
                    14.0,
                    0.0,
                    8.0,
                    [255, 120, 90],
                    "◎",
                    "Moon bomb",
                    "つきのボム",
                ),
                row(
                    "shield",
                    PickupKind::Shield,
                    1,
                    12,
                    0.0,
                    6.0,
                    0.0,
                    [120, 220, 255],
                    "□",
                    "Shield",
                    "バリア",
                ),
                row(
                    "frenzy",
                    PickupKind::Frenzy,
                    1,
                    12,
                    0.45,
                    8.0,
                    0.0,
                    [255, 200, 90],
                    "↑",
                    "Frenzy",
                    "ハッスル",
                ),
                row(
                    "weapon",
                    PickupKind::Weapon,
                    2,
                    14,
                    0.0,
                    0.0,
                    0.0,
                    [255, 170, 80],
                    "★",
                    "Weapon",
                    "ぶき",
                ),
            ],
            tables: vec![
                DropTable::default(),
                DropTable {
                    id: "elite".into(),
                    chance: 1.0,
                    rolls: 2,
                    rarity_min: 1,
                    chest: true,
                    guarantee: "weapon".into(),
                },
            ],
        }
    }
}

// ---------------------------------------------------------- synergy -----

/// A bonus a weapon gains while a passive is at `min_level` or more. `damage`, `cooldown` and
/// `area` are fractions added to the multiplier (`cooldown` -0.15 is 15% faster); `count` and
/// `pierce` are added.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SynergySpec {
    pub weapon: String,
    pub passive: String,
    pub min_level: u32,
    pub name: Loc,
    pub damage: f32,
    pub cooldown: f32,
    pub count: i32,
    pub pierce: i32,
    pub area: f32,
}

impl Default for SynergySpec {
    fn default() -> Self {
        Self {
            weapon: "bolt".into(),
            passive: "haste".into(),
            min_level: 3,
            name: Loc::default(),
            damage: 0.0,
            cooldown: 0.0,
            count: 0,
            pierce: 0,
            area: 0.0,
        }
    }
}

// --------------------------------------------------------- defaults -----

/// The shipped foe roster: the ghoul crowd, plus a fast runner, a slow armoured tank, a
/// shooter and the warden (an elite sent on a timer).
pub fn default_enemies() -> Vec<EnemyType> {
    let t = |id: &str, look: &str, color: [u32; 3], s: SwarmStats| EnemyType {
        id: id.into(),
        look: look.into(),
        color,
        swarm: Some(s),
        ..EnemyType::default()
    };
    vec![
        t(
            "ghoul",
            "ghoul",
            [120, 240, 150],
            SwarmStats {
                hp: 4.0,
                speed: 1.7,
                touch_dmg: 6.0,
                ..SwarmStats::default()
            },
        ),
        t(
            "runner",
            "vampire",
            [255, 110, 130],
            SwarmStats {
                hp: 3.0,
                speed: 2.7,
                radius: 0.34,
                touch_dmg: 4.0,
                xp: 1.0,
                unlock_secs: 20.0,
                weight: 4,
            },
        ),
        t(
            "shooter",
            "skeleton",
            [190, 140, 255],
            SwarmStats {
                hp: 6.0,
                speed: 1.5,
                radius: 0.38,
                touch_dmg: 4.0,
                xp: 2.0,
                unlock_secs: 34.0,
                weight: 2,
            },
        ),
        t(
            "tank",
            "keeper",
            [255, 170, 90],
            SwarmStats {
                hp: 16.0,
                speed: 1.0,
                radius: 0.62,
                touch_dmg: 10.0,
                xp: 4.0,
                unlock_secs: 45.0,
                weight: 2,
            },
        ),
        t(
            "warden",
            "zombie",
            [255, 215, 90],
            SwarmStats {
                hp: 50.0,
                speed: 1.25,
                radius: 0.8,
                touch_dmg: 14.0,
                xp: 12.0,
                unlock_secs: 0.0,
                weight: 0,
            },
        ),
    ]
}

pub fn default_foes() -> Vec<FoeSpec> {
    let f = |id: &str| FoeSpec {
        id: id.into(),
        ..FoeSpec::default()
    };
    vec![
        FoeSpec {
            tint: [255, 255, 255],
            ..f("ghoul")
        },
        FoeSpec {
            scale: 1.0,
            tint: [255, 200, 215],
            weave: 0.55,
            drop_mul: 0.6,
            ..f("runner")
        },
        FoeSpec {
            ai: FoeAi::Shoot,
            tint: [215, 195, 255],
            hold_m: 6.0,
            drop_mul: 1.5,
            ..f("shooter")
        },
        FoeSpec {
            tint: [255, 215, 170],
            armor: 1.0,
            knock_resist: 0.8,
            drop_mul: 2.2,
            ..f("tank")
        },
        FoeSpec {
            scale: 1.0,
            tint: [255, 235, 150],
            armor: 0.5,
            knock_resist: 0.9,
            table: "elite".into(),
            drop_mul: 1.0,
            elite: true,
            ..f("warden")
        },
    ]
}

pub fn default_synergies() -> Vec<SynergySpec> {
    let s = |weapon: &str, passive: &str, en: &str, ja: &str| SynergySpec {
        weapon: weapon.into(),
        passive: passive.into(),
        min_level: 3,
        name: Loc::new(en, ja),
        ..SynergySpec::default()
    };
    vec![
        SynergySpec {
            cooldown: -0.15,
            ..s("bolt", "haste", "Rapid moon", "はやづき")
        },
        SynergySpec {
            pierce: 1,
            ..s("bolt", "might", "Moonstrike", "つきぬき")
        },
        SynergySpec {
            count: 1,
            ..s("petals", "might", "Heavy bloom", "おもいはな")
        },
        SynergySpec {
            area: 0.25,
            ..s("blades", "swift", "Whirlwind", "つむじかぜ")
        },
        SynergySpec {
            count: 1,
            ..s("wisps", "growth", "Bloom swarm", "ふえるウィスプ")
        },
        SynergySpec {
            cooldown: -0.2,
            ..s("zap", "haste", "Static", "せいでんき")
        },
        SynergySpec {
            area: 0.3,
            damage: 0.2,
            ..s("pulse", "vitality", "Bulwark", "まもりのなみ")
        },
    ]
}

fn wl(en: &str, ja: &str, damage: f32, cooldown: f32, count: i32, pierce: i32) -> WeaponLevel {
    WeaponLevel {
        damage,
        cooldown,
        count,
        pierce,
        ..lv(en, ja)
    }
}

/// The shipped weapons: the moon bolt (unchanged) and five that feel different. `area` is the
/// orbit radius of the blades, the blast of a wisp, the jump of a zap, the ring of a pulse.
pub fn default_weapons() -> Vec<WeaponSpec> {
    let bolt = WeaponSpec {
        id: "bolt".into(),
        name: Loc::new("Moon bolt", "つきのたま"),
        desc: Loc::new(
            "Throws bolts at the nearest foe",
            "いちばんちかいてきになげる",
        ),
        glyph: "●".into(),
        color: [255, 190, 90],
        kind: WeaponKind::Projectile,
        base: WeaponBase::default(),
        levels: vec![
            wl("+1 bolt per cast", "いちどに +1こ", 0.0, 0.0, 1, 0),
            wl("Damage +3", "ダメージ +3", 3.0, 0.0, 0, 0),
            wl(
                "Casts faster, +1 bolt",
                "はやくなげる、+1こ",
                0.0,
                -0.15,
                1,
                0,
            ),
            wl(
                "Pierces two more, +2 damage",
                "ぬける +2、ダメージ +2",
                2.0,
                0.0,
                0,
                2,
            ),
        ],
    };
    let petals = WeaponSpec {
        id: "petals".into(),
        name: Loc::new("Moon petals", "つきのはなびら"),
        desc: Loc::new(
            "A fan of petals, deadly up close",
            "ちかくでつよい、はなびらのたば",
        ),
        glyph: "※".into(),
        color: [255, 150, 200],
        kind: WeaponKind::Spread,
        base: WeaponBase {
            damage: 2.4,
            cooldown: 1.0,
            speed: 15.0,
            range: 5.0,
            count: 4,
            spread_deg: 9.0,
            pierce: 0,
            size: 0.2,
            area: 0.0,
        },
        levels: vec![
            wl("+1 petal", "はなびら +1", 0.0, 0.0, 1, 0),
            wl("Damage +1.5", "ダメージ +1.5", 1.5, 0.0, 0, 0),
            wl(
                "Casts faster, +1 petal",
                "はやくなる、+1まい",
                0.0,
                -0.2,
                1,
                0,
            ),
            wl(
                "+2 petals, pierces one",
                "はなびら +2、ぬける +1",
                0.0,
                0.0,
                2,
                1,
            ),
        ],
    };
    let blades = WeaponSpec {
        id: "blades".into(),
        name: Loc::new("Crescent blades", "みかづきのやいば"),
        desc: Loc::new(
            "Blades circle you and cut what they touch",
            "まわりをまわる、さわるとダメージ",
        ),
        glyph: "†".into(),
        color: [150, 220, 255],
        kind: WeaponKind::Orbit,
        base: WeaponBase {
            damage: 3.0,
            cooldown: 0.45,
            speed: 3.2,
            range: 3.0,
            count: 2,
            spread_deg: 0.0,
            pierce: 0,
            size: 0.55,
            area: 1.9,
        },
        levels: vec![
            wl("+1 blade", "やいば +1", 0.0, 0.0, 1, 0),
            WeaponLevel {
                damage: 2.0,
                ..lv("Damage +2", "ダメージ +2")
            },
            WeaponLevel {
                speed: 0.9,
                area: 0.3,
                ..lv("Spins faster, wider circle", "はやくまわる、おおきなわ")
            },
            wl(
                "+1 blade, damage +2",
                "やいば +1、ダメージ +2",
                2.0,
                0.0,
                1,
                0,
            ),
        ],
    };
    let wisps = WeaponSpec {
        id: "wisps".into(),
        name: Loc::new("Seeker wisps", "おいかけウィスプ"),
        desc: Loc::new(
            "Wisps curve toward foes and burst",
            "てきをおいかけてはじける",
        ),
        glyph: "☆".into(),
        color: [190, 150, 255],
        kind: WeaponKind::Missile,
        base: WeaponBase {
            damage: 5.0,
            cooldown: 1.6,
            speed: 7.0,
            range: 11.0,
            count: 2,
            spread_deg: 40.0,
            pierce: 0,
            size: 0.3,
            area: 1.5,
        },
        levels: vec![
            wl("+1 wisp", "ウィスプ +1", 0.0, 0.0, 1, 0),
            wl("Damage +3", "ダメージ +3", 3.0, 0.0, 0, 0),
            wl("Casts faster", "はやくなる", 0.0, -0.3, 0, 0),
            WeaponLevel {
                count: 1,
                area: 0.5,
                ..lv("+1 wisp, bigger burst", "ウィスプ +1、おおきなばくはつ")
            },
        ],
    };
    let zap = WeaponSpec {
        id: "zap".into(),
        name: Loc::new("Thunder thread", "いなずまのいと"),
        desc: Loc::new(
            "Lightning jumps from foe to foe",
            "てきからてきへとびうつる",
        ),
        glyph: "∠".into(),
        color: [255, 245, 130],
        kind: WeaponKind::Chain,
        base: WeaponBase {
            damage: 6.0,
            cooldown: 1.1,
            speed: 1.0,
            range: 7.5,
            count: 3,
            spread_deg: 0.0,
            pierce: 0,
            size: 0.3,
            area: 3.2,
        },
        levels: vec![
            wl("Jumps to +1 foe", "とびうつる +1たい", 0.0, 0.0, 1, 0),
            wl("Damage +3", "ダメージ +3", 3.0, 0.0, 0, 0),
            wl("Casts faster", "はやくなる", 0.0, -0.25, 0, 0),
            WeaponLevel {
                count: 2,
                area: 1.0,
                ..lv("+2 jumps, longer reach", "とびうつる +2、とおくまで")
            },
        ],
    };
    let pulse = WeaponSpec {
        id: "pulse".into(),
        name: Loc::new("Moon pulse", "つきのはどう"),
        desc: Loc::new(
            "A ring that shoves the crowd back",
            "まわりをはじきとばすリング",
        ),
        glyph: "○".into(),
        color: [120, 235, 220],
        kind: WeaponKind::Pulse,
        base: WeaponBase {
            damage: 5.0,
            cooldown: 2.4,
            speed: 1.0,
            range: 2.6,
            count: 1,
            spread_deg: 0.0,
            pierce: 0,
            size: 0.3,
            area: 2.6,
        },
        levels: vec![
            wl("Damage +3", "ダメージ +3", 3.0, 0.0, 0, 0),
            WeaponLevel {
                area: 0.5,
                ..lv("Wider ring", "リングがひろがる")
            },
            wl("Pulses faster", "はやくなる", 0.0, -0.5, 0, 0),
            WeaponLevel {
                damage: 4.0,
                area: 0.5,
                ..lv("Damage +4, wider ring", "ダメージ +4、ひろがる")
            },
        ],
    };
    vec![bolt, petals, blades, wisps, zap, pulse]
}

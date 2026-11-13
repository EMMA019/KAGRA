//! Survivor loop for the VRM horde prototype scenes (dev only).
//!
//! This is the *game-specific* half of the horde prototype: spawning,
//! auto-fire, contact damage, kills and game over for worlds that carry
//! a `survivor` spec ([`crate::world_doc::SurvivorSpec`]). The generic
//! half stays in the shared drivers — `world_play`'s seek pass moves
//! the bodies, the NPC phase pass animates them, `world_doc` owns the
//! spec and the pool flags (`survivor_enemy` / `survivor_speed`).
//!
//! Boundary, for the shipping version: everything in this file is what
//! a kit (a `KitHost` game, the way Moonlit Swarm is built) would own.
//! Only the spec/flag plumbing in `world_doc` and the one `tick` /
//! `build_hud` hook in `world_play` belong to the engine side, and both
//! are inert on worlds without a `survivor` block.
//!
//! Prototype simplifications, on purpose:
//! - Hitscan fire. The shot itself is sold by a one-frame tracer
//!   (`doc.lines`) plus a muzzle point sprite (`doc.points`), both
//!   maintained by the loop under fixed ids and faded in ~60 ms.
//! - The rifle is a rigid prop the loop re-seats in the player's
//!   hands every step (body-level attach: it follows position and
//!   facing, not the hand bones), so the aim clips finally hold a
//!   visible gun.
//! - No death clip in the Mixamo set, so a killed enemy freezes grey
//!   (the renderer's `ko` convention), sinks through the floor over
//!   about a second, and recycles into the spawn pool.
//! - Parked pool members wait under the floor (`PARK_Y`); the opaque
//!   ground plane hides them.
//! - Difficulty pressure is the alive-cap ramp plus a spawn
//!   interval that eases toward its floor, time-scaled enemy HP
//!   and speed once past an opening grace, and a boss on a fixed
//!   schedule: a scaled-up pool member (the hulk cut when the
//!   scene authors one) whose every return is tougher.

use crate::game::GamePhase;
use crate::game_core::stick;
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};
use crate::strokes::{FatLine, PointCloud};
use crate::world_doc::{load_skinned_parts, SurvivorSpec, WorldDoc, WorldWalker};
use std::collections::HashMap;

/// Off-stage parking depth for pool members (meters under the floor).
const PARK_Y: f32 = -100.0;
/// Seconds a killed enemy stays on stage, sinking, before recycling.
const DYING_S: f32 = 1.1;
/// Sink speed while dying (m/s).
const SINK_RATE: f32 = 0.9;
/// Red-flash time after a hit (the renderer's `hurt` name convention).
const HURT_S: f32 = 0.09;
/// Seek stop distance used when a walker authors none.
const DEFAULT_STOP: f32 = 1.7;
/// Cruise speed handed to `seek` when a pool walker authors none.
const DEFAULT_SPEED: f32 = 1.35;
/// Player facing turn rate (rad/s). Aim and movement facings ease at
/// this rate instead of snapping, so a target switch reads as a fast
/// turn of the head/body rather than a teleport of the whole pose.
/// A half-turn takes ~0.35 s.
const TURN_RATE: f32 = 9.0;
/// How far beyond `fire_range` the current target stays locked
/// (hysteresis so two near-equal zombies do not trade the aim — and
/// the facing — back and forth every shot). The lock is for facing
/// only out there: shots are held until the target is truly in
/// `fire_range`, so no rounds are spent on a shape in the fog.
const TARGET_STICK: f32 = 1.25;
/// A rival zombie this close (meters) is an immediate threat: it
/// steals the aim from a farther lock even mid-kill, because it is
/// the one about to bite.
const THREAT_R: f32 = 3.0;
/// Or a rival at less than this fraction of the lock's distance
/// steals the aim — the gun re-points at what matters instead of
/// finishing a distant body while the near ones close in.
const THREAT_FRAC: f32 = 0.55;
/// Floating-stick geometry, mirroring Moonlit Swarm's kit stick
/// (`survivor_arena` input / `game_core::stick`): base radius derives
/// from a 70 px stick at the arena HUD scale, dead zone 0.12.
const STICK_PX: f32 = 70.0;
const STICK_DEAD: f32 = 0.12;
/// Doc ids of the drop point clouds (XP gems / heal packs). Drops
/// ride the same per-frame doc plumbing as the shot FX.
const GEM_ID: &str = "fx:gems";
const HEAL_ID: &str = "fx:heals";
/// Chance a kill also drops a heal pack.
const HEAL_CHANCE: f32 = 0.12;
/// HP restored by one heal pack.
const HEAL_AMOUNT: f32 = 20.0;
/// Oldest drops are culled past this many (a long run must not grow
/// the point clouds without bound).
const MAX_DROPS: usize = 48;
/// Distance at which a drop is collected by walking over it.
const COLLECT_R: f32 = 1.1;
/// Drops inside this distance drift toward the player.
const MAGNET_R: f32 = 3.0;
/// Drift speed of a magneted drop (m/s).
const MAGNET_SPEED: f32 = 6.5;
/// Seconds the tracer line / muzzle sprite stay up after a shot.
const FX_S: f32 = 0.06;
/// Doc id of the tracer entry in `doc.lines`.
const TRACER_ID: &str = "fx:tracer";
/// Doc id of the muzzle-flash entry in `doc.points`.
const FLASH_ID: &str = "fx:flash";
/// Prop id of the rifle the loop keeps in the player's hands. Scenes
/// that author no such prop simply skip the attach.
pub const RIFLE_PROP_ID: &str = "prop:rifle";
/// Rifle seat relative to the player origin (meters): chest height,
/// forward and to the right of the body axis.
const RIFLE_H: f32 = 1.30;
const RIFLE_FWD: f32 = 0.30;
const RIFLE_RIGHT: f32 = 0.10;
/// Extra yaw on the rifle prop so the Kenney blaster's barrel axis
/// matches the walker's facing convention (tuned from screenshots).
const RIFLE_YAW_OFF: f32 = 0.0;
/// Muzzle offset from the player origin along the facing (the rifle
/// seat plus the barrel length), and its height.
const MUZZLE_FWD: f32 = 0.74;
const MUZZLE_H: f32 = RIFLE_H + 0.05;
/// Doc id of the rocket-impact burst entry in `doc.points`.
const BOOM_ID: &str = "fx:boom";
/// Doc id of the equipped gun's glow point (rides the muzzle, tinted
/// by weapon so the held gun reads at a glance).
const GUN_ID: &str = "fx:gun";
/// Doc id of the accent ring under the player — soldier and zombies
/// share a grey palette, so the ring (plus a marker over the head)
/// is what keeps the player findable in a brawl.
const RING_ID: &str = "fx:ring";
/// Rocket splash radius (meters) around the impact point.
const SPLASH_R: f32 = 2.3;
/// Doc id of the boss marker entry in `doc.points` (a red ring at
/// its feet plus a bobbing point over its head — the scaled body
/// alone can read as a near regular at a glance in the dusk).
const BOSS_ID: &str = "fx:boss";
/// Doc id of the tap-lock marker entry in `doc.points` (an amber
/// ring at the locked foe's feet plus a point over its head — the
/// player ring / boss ring trick, in the lock's own colour).
const LOCK_ID: &str = "fx:lock";
/// Screen-space pick radius of a target tap, in HUD pixels before
/// the HUD scale — Moonlit's `input.tap_px` default, so a tap
/// means the same fat-finger tolerance in both games.
const TAP_PX: f32 = 44.0;
/// Chest height a tap projects a foe at (Moonlit projects its
/// chibi foes at 0.45; these bodies are human-sized).
const TAP_CHEST_Y: f32 = 1.1;
/// Burst refill: while the stage stands emptier than this share
/// of the alive cap, fresh bodies arrive at [`REFILL_MULT`] of the
/// usual interval, so a cleared field refills as a wave instead of
/// a trickle (the lull a still-frame review caught).
const REFILL_UNDER: f32 = 0.4;
/// Spawn-interval multiple while the burst refill runs.
const REFILL_MULT: f32 = 0.35;
/// Sound-cue throttle slots (Moonlit's `cue_t` pattern: a name, a
/// slot, a minimum gap, so a burst of hits is a smear of sound,
/// not a buzz). The names are Moonlit's cue vocabulary, and the
/// scene's `game.audio` block maps them to the shared pool.
const CUE_SHOOT: usize = 0;
const CUE_HIT: usize = 1;
const CUE_KILL: usize = 2;
const CUE_HURT: usize = 3;
const CUE_GEM: usize = 4;
const CUE_EVENT: usize = 5;
/// XP gems a boss bursts into on top of the standard one, left in
/// a ring around the corpse.
const BOSS_GEMS: u32 = 6;
/// The boss lumbers: its cruise is this much under a regular's, so
/// its threat is bulk and bite, not pace (the time curve still
/// applies on top).
const BOSS_SPEED_MULT: f32 = 0.9;
/// Splash damage as a multiple of the shot's damage (the direct
/// target takes its own hit on top).
const SPLASH_MULT: f32 = 1.5;
/// Half-width of the laser's piercing corridor (meters).
const LASER_CORRIDOR: f32 = 0.85;

/// The gun in the player's hands. Kills sometimes drop another one;
/// walking over a drop swaps (the old gun falls where the player
/// stands, offset behind so the swap is a choice, not a loop).
/// Numbers are deliberately coarse — the *feel* must differ at a
/// glance: rate, punch, tracer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weapon {
    Rifle,
    MachineGun,
    Laser,
    Rocket,
}

impl Weapon {
    /// HUD label (ASCII only).
    pub fn name(self) -> &'static str {
        match self {
            Weapon::Rifle => "RIFLE",
            Weapon::MachineGun => "MACHINE GUN",
            Weapon::Laser => "LASER GUN",
            Weapon::Rocket => "ROCKET LAUNCHER",
        }
    }
    /// Fire-interval multiple of the scene's base interval.
    fn interval_mult(self) -> f32 {
        match self {
            Weapon::Rifle => 1.0,
            Weapon::MachineGun => 0.45,
            Weapon::Laser => 2.3,
            Weapon::Rocket => 1.9,
        }
    }
    /// Per-hit damage multiple of the scene's base damage.
    fn damage_mult(self) -> f32 {
        match self {
            Weapon::Rifle => 1.0,
            Weapon::MachineGun => 0.45,
            Weapon::Laser => 2.6,
            Weapon::Rocket => 1.1,
        }
    }
    /// Tracer line color / width — the shot's signature.
    fn tracer(self) -> ([u8; 4], f32) {
        match self {
            Weapon::Rifle => ([255, 224, 140, 255], 3.0),
            Weapon::MachineGun => ([255, 170, 60, 255], 1.6),
            Weapon::Laser => ([120, 240, 255, 255], 5.0),
            Weapon::Rocket => ([255, 110, 60, 255], 4.0),
        }
    }
    /// Seconds the tracer / flash stay up (the laser lingers).
    fn fx_seconds(self) -> f32 {
        match self {
            Weapon::Laser => 0.13,
            Weapon::Rocket => 0.1,
            _ => FX_S,
        }
    }
    /// Tint of the ground drop and the muzzle glow point.
    fn drop_color(self) -> [u8; 4] {
        match self {
            Weapon::Rifle => [220, 230, 240, 255],
            Weapon::MachineGun => [255, 180, 60, 255],
            Weapon::Laser => [110, 240, 255, 255],
            Weapon::Rocket => [255, 128, 0, 255],
        }
    }
    /// Scale of the shared blaster prop while this gun is held (one
    /// prop model serves all four; size is its silhouette cue).
    fn prop_scale(self) -> f32 {
        match self {
            Weapon::Rifle => 1.0,
            Weapon::MachineGun => 0.85,
            Weapon::Laser => 1.05,
            Weapon::Rocket => 1.5,
        }
    }
    /// Doc id of this weapon's ground-drop point cloud.
    fn cloud_id(self) -> &'static str {
        match self {
            Weapon::Rifle => "fx:wpn_rifle",
            Weapon::MachineGun => "fx:wpn_mg",
            Weapon::Laser => "fx:wpn_laser",
            Weapon::Rocket => "fx:wpn_rocket",
        }
    }
}

/// Drop rotation: what the next weapon drop will be. The held gun
/// is skipped at drop time, so a drop is always a real choice.
const WEAPON_CYCLE: [Weapon; 4] = [
    Weapon::MachineGun,
    Weapon::Laser,
    Weapon::Rocket,
    Weapon::Rifle,
];
const ALL_WEAPONS: [Weapon; 4] = [
    Weapon::Rifle,
    Weapon::MachineGun,
    Weapon::Laser,
    Weapon::Rocket,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Parked,
    Alive,
    Dying,
}

#[derive(Clone, Debug)]
struct EnemyState {
    mode: Mode,
    hp: f32,
    hurt_t: f32,
    dying_t: f32,
    base_name: String,
    /// True while this pool member walks as the boss: scaled body,
    /// boss hit points, heavier bite, a guaranteed payout.
    boss: bool,
    /// Cruise speed fixed at activation (authored speed × jitter,
    /// × [`BOSS_SPEED_MULT`] for the boss). The per-step time curve
    /// multiplies this, so `speed0` itself never drifts.
    speed0: f32,
}

/// What a killed enemy leaves on the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DropKind {
    /// XP gem: banks toward the next level.
    Gem,
    /// Heal pack: restores HP on pickup.
    Heal,
    /// A gun: walking over it swaps the held weapon for this one.
    Weapon(Weapon),
}

#[derive(Clone, Copy, Debug)]
struct Drop {
    kind: DropKind,
    pos: [f32; 3],
}

/// One level-up reward. Three distinct ones are offered per level,
/// drawn from the whole pool on a rotation so builds vary by run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// Fire interval ×0.88.
    RapidFire,
    /// Shot damage +8.
    DamageUp,
    /// Max HP +25 and heal 25.
    Vitality,
    /// Contact damage taken ×0.85.
    Armor,
    /// Pickup pull radius grows.
    Magnet,
}

const CHOICE_POOL: [Choice; 5] = [
    Choice::RapidFire,
    Choice::DamageUp,
    Choice::Vitality,
    Choice::Armor,
    Choice::Magnet,
];

impl Choice {
    fn name(self) -> &'static str {
        match self {
            Choice::RapidFire => "RAPID FIRE",
            Choice::DamageUp => "HEAVY ROUNDS",
            Choice::Vitality => "VITALITY",
            Choice::Armor => "PLATING",
            Choice::Magnet => "MAGNET COIL",
        }
    }

    fn desc(self) -> &'static str {
        match self {
            Choice::RapidFire => "Fire rate +12%",
            Choice::DamageUp => "Damage +8",
            Choice::Vitality => "Max HP +25, heal 25",
            Choice::Armor => "Damage taken -15%",
            Choice::Magnet => "Pickup pull grows",
        }
    }
}

/// XP gems needed to leave `level` (level 1 → 5, then +4 a level):
/// the first choice lands inside the first half minute of contact.
pub fn xp_need(level: u32) -> u32 {
    5 + level.saturating_sub(1) * 4
}

/// HUD scale of the arena kits (`survivor_arena::hud_scale`); the
/// stick borrows it so the ported stick feels identical.
fn stick_hud_scale(w: f32, h: f32) -> f32 {
    (w.min(h) / 400.0).clamp(0.9, 2.6)
}

/// Floating-stick base radius at a viewport, as the arena kits
/// compute it (`stick_px` × scale × 1.25).
fn stick_radius(w: f32, h: f32) -> f32 {
    STICK_PX * stick_hud_scale(w, h) * 1.25
}

/// Where the resting stick hint sits (bottom-left, clear of the
/// edges), same formula as the arena kits.
fn stick_center(w: f32, h: f32) -> (f32, f32) {
    let r = stick_radius(w, h);
    (
        (r * 1.35).max(w * 0.2),
        h - r * 1.45 - 12.0 * stick_hud_scale(w, h),
    )
}

/// The three level-up card rects (x, y, w, h in HUD pixels). One
/// layout shared by the HUD painter and the tap hit-test, so what
/// the player sees is exactly what they can tap. Sized on the
/// arena kits' HUD scale (`stick_hud_scale`, Moonlit's own), so
/// the cards read at phone size like the kit's do.
fn level_card_rects(w: f32, h: f32) -> [[f32; 4]; 3] {
    let s = stick_hud_scale(w, h);
    let cw = (w - 36.0 * s).min(340.0 * s);
    let ch = 80.0 * s;
    let gap = 14.0 * s;
    let x = (w - cw) / 2.0;
    let y0 = h * 0.32;
    [
        [x, y0, cw, ch],
        [x, y0 + ch + gap, cw, ch],
        [x, y0 + 2.0 * (ch + gap), cw, ch],
    ]
}

/// Run state for one survivor world. Built from the authored doc by
/// [`SurvivorLoop::from_doc`]; `world_play` rebuilds it on `start()`.
#[derive(Clone, Debug)]
pub struct SurvivorLoop {
    /// Player hit points left.
    pub hp: f32,
    /// Player hit points at run start (the spec value).
    pub hp_max: f32,
    /// Enemies killed this run.
    pub kills: u32,
    /// Seconds survived (only advances while the loop steps).
    pub elapsed: f32,
    /// Enemies on stage after the last step (HUD / tests).
    pub alive_now: u32,
    /// True once the player fell. `world_play` turns this into the
    /// Complete phase; the loop itself just stops changing the world.
    pub over: bool,
    /// Shots actually fired this run. Waste accounting: the number
    /// to compare against kills when judging the auto-aim.
    pub shots: u32,
    /// XP gems banked toward the next level.
    pub gems: u32,
    /// Current level (the run starts at 1).
    pub level: u32,
    /// True while the level-up cards are on screen. `world_play`
    /// freezes the whole world while this holds (nothing moves, so
    /// choosing is safe), and taps route to the cards, not the stick.
    pub level_pending: bool,
    /// The three rewards offered by the pending level-up.
    pub choices: [Choice; 3],
    /// The gun currently held. Kills can drop another; walking
    /// over the drop swaps this out (the old gun drops behind).
    pub weapon: Weapon,
    /// Bosses that have walked in this run. The next one is due at
    /// `next_boss_t` and takes `boss_hp_growth`^(this count) on top
    /// of the base boss multiple.
    pub bosses_spawned: u32,
    /// Elapsed time at which the next boss is due. Seeded from the
    /// spec's `boss_first_s`; not advanced while the pool has no
    /// parked member to give, so a full stage delays, never skips.
    next_boss_t: f32,
    /// Rotation cursor for weapon drops (see `WEAPON_CYCLE`).
    weapon_cycle: usize,
    /// Fire-interval multiplier from Rapid Fire picks.
    fire_mult: f32,
    /// Flat shot-damage bonus from Heavy Rounds picks.
    dmg_bonus: f32,
    /// Contact-damage multiplier from Plating picks (lower is better).
    armor_mult: f32,
    /// Extra magnet / collect reach from Magnet Coil picks (meters).
    magnet_r: f32,
    drops: Vec<Drop>,
    /// Floating touch stick (the arena kits' ぷにコン, ported): the
    /// finger that owns it, where its base landed and where the
    /// thumb is now. Inert for keyboard / pad players.
    stick_id: Option<u32>,
    stick_origin: (f32, f32),
    stick_finger: Option<(f32, f32)>,
    stick_size: (f32, f32),
    fire_cd: f32,
    fire_phase: f32,
    /// Lazily resolved fire-clip duration (outer None = not tried).
    fire_dur: Option<Option<f32>>,
    /// Currently locked target (walker id), kept while it stays
    /// alive and near range so the aim does not flicker.
    aim_id: Option<String>,
    /// The foe the player tapped to lock (walker id), Moonlit's
    /// hybrid aim ported: while it lives it wins the gun over the
    /// auto-aim whenever it is in reach; a tap on it again, a tap
    /// on bare ground, or its death releases it.
    tap_lock: Option<String>,
    /// A tap that still needs its pick resolved: `touch` has no
    /// doc or camera, so it parks the pixel here and `world_play`
    /// (which owns both) resolves it the same event.
    pending_tap: Option<(f32, f32)>,
    /// Per-slot sound-cue throttle timers (seconds left).
    cue_t: [f32; 6],
    /// True once this run has started its music. The loop is
    /// rebuilt per run, so the flag re-arms itself on a restart.
    bgm_on: bool,
    /// Smoothed player facing actually written to the walker. Eases
    /// toward the aim / movement facing at [`TURN_RATE`].
    face_yaw: f32,
    /// False until `face_yaw` is seeded from the authored yaw.
    face_seeded: bool,
    /// Time left on the tracer / muzzle flash entries.
    fx_t: f32,
    spawn_t: f32,
    rng: u32,
    enemies: HashMap<String, EnemyState>,
}

impl Default for SurvivorLoop {
    fn default() -> Self {
        Self {
            hp: 0.0,
            hp_max: 0.0,
            kills: 0,
            elapsed: 0.0,
            alive_now: 0,
            over: false,
            shots: 0,
            gems: 0,
            level: 1,
            level_pending: false,
            choices: [Choice::RapidFire, Choice::DamageUp, Choice::Vitality],
            weapon: Weapon::Rifle,
            bosses_spawned: 0,
            next_boss_t: f32::MAX,
            weapon_cycle: 0,
            fire_mult: 1.0,
            dmg_bonus: 0.0,
            armor_mult: 1.0,
            magnet_r: 0.0,
            drops: Vec::new(),
            stick_id: None,
            stick_origin: (0.0, 0.0),
            stick_finger: None,
            stick_size: (1.0, 1.0),
            fire_cd: 0.0,
            fire_phase: 0.0,
            fire_dur: None,
            aim_id: None,
            tap_lock: None,
            pending_tap: None,
            cue_t: [0.0; 6],
            bgm_on: false,
            face_yaw: 0.0,
            face_seeded: false,
            fx_t: 0.0,
            spawn_t: 0.0,
            rng: 0x9E37_79B9,
            enemies: HashMap::new(),
        }
    }
}

impl SurvivorLoop {
    /// Fresh run state from the authored doc: HP from the spec, every
    /// `survivor_enemy` walker parked in the pool under its authored
    /// name. Worlds without a spec get an inert default.
    pub fn from_doc(doc: &WorldDoc) -> Self {
        let mut state = Self::default();
        let Some(spec) = &doc.survivor else {
            return state;
        };
        state.hp = spec.player_hp;
        state.hp_max = spec.player_hp;
        state.next_boss_t = if spec.boss_first_s > 0.0 {
            spec.boss_first_s
        } else {
            f32::MAX
        };
        state.rng ^= (doc.walkers.len() as u32).wrapping_mul(0x85EB_CA6B);
        if state.rng == 0 {
            state.rng = 0x9E37_79B9;
        }
        for w in &doc.walkers {
            if w.survivor_enemy {
                state.enemies.insert(
                    w.id.clone(),
                    EnemyState {
                        mode: Mode::Parked,
                        hp: 0.0,
                        hurt_t: 0.0,
                        dying_t: 0.0,
                        base_name: w.name.clone(),
                        boss: false,
                        speed0: 0.0,
                    },
                );
            }
        }
        state
    }

    /// Raw pointer feed for the floating stick and the level-up
    /// cards. `world_play` routes every phase here on survivor
    /// worlds (session pushes pointers to kits the same way); the
    /// stick half mirrors `survivor_arena`'s `touch_ev` stick mode:
    /// a thumb landing in the lower-left zone grows its own base
    /// where it lands, the base follows past the rim, and letting go
    /// hands control back to keys / pad.
    pub fn touch(&mut self, ev: PointerEvent, w: u32, h: u32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        self.stick_size = (w, h);
        if self.level_pending {
            // The world is frozen; a tap on a card is the only input
            // that means anything. One exception: a stick release
            // still counts. The thumb that steered here lifts to tap
            // a card, and a release swallowed by the freeze would
            // leave the stick held — the hero would walk on by
            // himself once the pick closes the cards. (Moonlit's kit
            // clears its finger on End/Cancel the same way, cards or
            // no cards.)
            match ev.phase {
                PointerPhase::Begin => {
                    for (i, r) in level_card_rects(w, h).iter().enumerate() {
                        if ev.x >= r[0]
                            && ev.x <= r[0] + r[2]
                            && ev.y >= r[1]
                            && ev.y <= r[1] + r[3]
                        {
                            let pick = self.choices[i];
                            self.apply_choice(pick);
                            break;
                        }
                    }
                }
                PointerPhase::End | PointerPhase::Cancel => {
                    if self.stick_id == Some(ev.id) {
                        self.stick_id = None;
                        self.stick_finger = None;
                    }
                }
                PointerPhase::Move => {}
            }
            return;
        }
        match ev.phase {
            PointerPhase::Begin => {
                if self.stick_id.is_none() && ev.x < w * 0.55 && ev.y > h * 0.4 {
                    let r = stick_radius(w, h);
                    self.stick_id = Some(ev.id);
                    self.stick_finger = Some((ev.x, ev.y));
                    self.stick_origin =
                        stick::land(ev.x, ev.y, r, 4.0 * stick_hud_scale(w, h), w, h);
                } else if self.stick_id != Some(ev.id) {
                    // Any other thumb — a second finger while the
                    // stick is held, or a first finger outside the
                    // stick zone — is a tap, never a stick (Moonlit's
                    // stick-mode branch, verbatim semantics): it
                    // picks the target. The pick itself needs the
                    // doc and the camera, which the loop does not
                    // own, so the pixel is parked for `world_play`
                    // to resolve on this same event.
                    self.pending_tap = Some((ev.x, ev.y));
                }
            }
            PointerPhase::Move => {
                if self.stick_id == Some(ev.id) {
                    self.stick_finger = Some((ev.x, ev.y));
                    let r = stick_radius(w, h);
                    self.stick_origin = stick::follow(self.stick_origin, (ev.x, ev.y), r);
                }
            }
            PointerPhase::End | PointerPhase::Cancel => {
                if self.stick_id == Some(ev.id) {
                    self.stick_id = None;
                    self.stick_finger = None;
                }
            }
        }
    }

    /// The stick's push as a `WalkInput` pair (lx, lz), or `None`
    /// while no thumb is down (keys / pad then steer, untouched).
    /// Screen y grows downward, `WalkInput` lz grows up-screen:
    /// hence the negation, same as the shell stick and the kits.
    pub fn stick_vector(&self) -> Option<(f32, f32)> {
        let f = self.stick_finger?;
        let (w, h) = self.stick_size;
        let (ax, ay) = stick::analog(self.stick_origin, f, stick_radius(w, h), STICK_DEAD);
        Some((ax, -ay))
    }

    /// Let go of the floating stick: no thumb steers until a new one
    /// lands. The level-up cards call this on the way in and on the
    /// pick (Moonlit's `mute_after_tap`): a thumb held — or a release
    /// lost — across the freeze must not walk the hero once the cards
    /// close, and a stale `stick_id` must not lock out the next thumb.
    fn drop_stick(&mut self) {
        self.stick_id = None;
        self.stick_finger = None;
    }

    /// Queue a sound cue through the mixer's throttle, Moonlit's
    /// `cue` pattern: one name per slot per gap, so the per-frame
    /// callers (a laser raking a crowd, contact damage every step)
    /// smear into a rhythm instead of a buzz. The names are the
    /// arena kit's vocabulary; the scene's `game.audio` block maps
    /// them onto the shared sound pool in the shell.
    fn cue(&mut self, name: &str, slot: usize, gap: f32) {
        if self.cue_t[slot] <= 0.0 {
            self.cue_t[slot] = gap;
            crate::mixer::cue_sfx(name);
        }
    }

    /// Take a tap parked by [`SurvivorLoop::touch`], if any.
    /// `world_play` resolves it with the doc and the camera on the
    /// same pointer event.
    pub fn take_pending_tap(&mut self) -> Option<(f32, f32)> {
        self.pending_tap.take()
    }

    /// The walker id a tap has locked, while the lock holds.
    pub fn tap_locked(&self) -> Option<&str> {
        self.tap_lock.as_deref()
    }

    /// Resolve a target tap at a canvas pixel, Moonlit's
    /// `tap_select` with the world loop's own bookkeeping: the
    /// living foe nearest the tap (projected at chest height,
    /// inside [`TAP_PX`] of screen) is locked — tapping the locked
    /// foe again releases it — and a tap that hits no foe clears
    /// the lock (the kits also swing their manual aim that way;
    /// this loop has no manual aim, so the clear is all of it).
    /// A pick or a clear both chirp `pick`, as the kit does.
    pub fn tap_select(
        &mut self,
        doc: &WorldDoc,
        view: crate::game_core::view::View,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) {
        let reach = TAP_PX * stick_hud_scale(w, h);
        let mut best: Option<(&str, f32)> = None;
        for wk in &doc.walkers {
            if !self
                .enemies
                .get(&wk.id)
                .is_some_and(|e| e.mode == Mode::Alive)
            {
                continue;
            }
            let chest = [wk.position[0], wk.position[1] + TAP_CHEST_Y, wk.position[2]];
            let Some((sx, sy)) = view.project(chest, w, h) else {
                continue;
            };
            let d = (sx - x).hypot(sy - y);
            if d < reach && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((wk.id.as_str(), d));
            }
        }
        match best {
            Some((id, _)) if self.tap_lock.as_deref() == Some(id) => {
                self.tap_lock = None;
            }
            Some((id, _)) => {
                self.tap_lock = Some(id.to_string());
            }
            None => {
                self.tap_lock = None;
            }
        }
        self.cue("pick", CUE_EVENT, 0.1);
    }

    /// Apply one level-up reward and close the cards. The effects
    /// are arithmetic on the run's stats — fire cadence, damage,
    /// survivability — never cosmetic.
    fn apply_choice(&mut self, pick: Choice) {
        match pick {
            Choice::RapidFire => self.fire_mult *= 0.88,
            Choice::DamageUp => self.dmg_bonus += 8.0,
            Choice::Vitality => {
                self.hp_max += 25.0;
                self.hp = (self.hp + 25.0).min(self.hp_max);
            }
            Choice::Armor => self.armor_mult *= 0.85,
            Choice::Magnet => self.magnet_r += 1.2,
        }
        self.level_pending = false;
        // Whatever the stick held before the freeze is stale now.
        self.drop_stick();
    }

    /// Roll the three offers for a fresh level-up: a window of three
    /// over the pool, rotated by the run's rng so the set varies.
    fn roll_choices(&mut self) {
        let start = (next_rng(&mut self.rng) % CHOICE_POOL.len() as u32) as usize;
        self.choices = [
            CHOICE_POOL[start],
            CHOICE_POOL[(start + 1) % CHOICE_POOL.len()],
            CHOICE_POOL[(start + 2) % CHOICE_POOL.len()],
        ];
    }

    /// Effective fire interval (spec × Rapid Fire picks).
    fn fire_interval(&self, spec: &SurvivorSpec) -> f32 {
        spec.fire_interval * self.fire_mult * self.weapon.interval_mult()
    }

    /// Effective per-shot damage (spec + Heavy Rounds picks).
    fn shot_damage(&self, spec: &SurvivorSpec) -> f32 {
        (spec.shot_damage + self.dmg_bonus) * self.weapon.damage_mult()
    }

    /// The next gun in the drop rotation, skipping the one in
    /// hand, so a drop is always a real choice.
    fn roll_weapon(&mut self) -> Weapon {
        let mut kind = WEAPON_CYCLE[self.weapon_cycle % WEAPON_CYCLE.len()];
        self.weapon_cycle += 1;
        if kind == self.weapon {
            kind = WEAPON_CYCLE[self.weapon_cycle % WEAPON_CYCLE.len()];
            self.weapon_cycle += 1;
        }
        kind
    }

    /// Damage one living enemy. On a kill: stamp the death and
    /// roll the payout where the body fell — always an XP gem,
    /// sometimes a heal pack, sometimes another gun. A boss pays
    /// out in full instead: a gun for certain, plus a burst of
    /// gems ringed around the corpse.
    fn apply_hit(&mut self, doc: &mut WorldDoc, i: usize, dmg: f32, spec: &SurvivorSpec) {
        let id = doc.walkers[i].id.clone();
        let mut killed = false;
        let mut was_boss = false;
        if let Some(e) = self.enemies.get_mut(&id) {
            if e.mode != Mode::Alive {
                return;
            }
            e.hp -= dmg;
            if e.hp <= 0.0 {
                killed = true;
                was_boss = e.boss;
            } else {
                e.hurt_t = HURT_S;
            }
        }
        if !killed {
            // A landed hit ticks; a kill thuds lower (and rarer).
            self.cue("hit", CUE_HIT, 0.07);
            doc.walkers[i].name = "hurt".into();
            return;
        }
        self.cue("kill", CUE_KILL, 0.1);
        self.kills += 1;
        if let Some(e) = self.enemies.get_mut(&id) {
            e.mode = Mode::Dying;
            e.dying_t = 0.0;
            e.hurt_t = 0.0;
        }
        let wp = doc.walkers[i].position;
        let w = &mut doc.walkers[i];
        w.name = "ko".into();
        w.seek = 0.0;
        let at = [wp[0], wp[1] + 0.35, wp[2]];
        self.drops.push(Drop {
            kind: DropKind::Gem,
            pos: at,
        });
        if rng_unit(&mut self.rng) < HEAL_CHANCE {
            self.drops.push(Drop {
                kind: DropKind::Heal,
                pos: at,
            });
        }
        if was_boss {
            for n in 0..BOSS_GEMS {
                let a = n as f32 / BOSS_GEMS as f32 * std::f32::consts::TAU
                    + rng_unit(&mut self.rng) * 0.6;
                let r = 0.7 + 0.5 * rng_unit(&mut self.rng);
                self.drops.push(Drop {
                    kind: DropKind::Gem,
                    pos: [wp[0] + a.cos() * r, wp[1] + 0.35, wp[2] + a.sin() * r],
                });
            }
            let kind = self.roll_weapon();
            self.drops.push(Drop {
                kind: DropKind::Weapon(kind),
                pos: at,
            });
        } else if rng_unit(&mut self.rng) < spec.weapon_drop_chance {
            let kind = self.roll_weapon();
            self.drops.push(Drop {
                kind: DropKind::Weapon(kind),
                pos: at,
            });
        }
        while self.drops.len() > MAX_DROPS {
            self.drops.remove(0);
        }
    }
}

/// Park every pool member off-stage (called once on world build, so
/// the seed doc — and therefore every restart — starts parked too).
/// Authored names are kept: parking is positional only. The boss's
/// bulk is loop state, never doc state, so parking also strips the
/// scale back off — a restart can never inherit a giant zombie.
/// The rifle prop, when authored, is seated in the player's hands
/// so even the title frame shows it.
pub fn park_pool(doc: &mut WorldDoc) {
    if doc.survivor.is_none() {
        return;
    }
    for w in &mut doc.walkers {
        if w.survivor_enemy {
            w.position[1] = PARK_Y;
            w.seek = 0.0;
            w.scale = None;
        }
    }
    if let Some(pp) = player_pos(doc) {
        let yaw = doc
            .player
            .as_ref()
            .or_else(|| doc.walkers.first())
            .map(|w| w.yaw)
            .unwrap_or(0.0);
        place_rifle(doc, pp, yaw, Weapon::Rifle);
    }
}

/// Ease `cur` toward `want` along the shortest arc, moving at most
/// `max_step` radians. The result is not renormalized (callers only
/// feed it to sin/cos and back in here, which is wrap-safe).
fn turn_toward(cur: f32, want: f32, max_step: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut d = (want - cur).rem_euclid(tau);
    if d > std::f32::consts::PI {
        d -= tau;
    }
    cur + d.clamp(-max_step.abs(), max_step.abs())
}

/// Unit facing / right vectors for a yaw (the walker convention:
/// facing is `(sin yaw, cos yaw)` in XZ, right is facing × up).
fn facing(yaw: f32) -> ([f32; 2], [f32; 2]) {
    let (s, c) = (yaw.sin(), yaw.cos());
    ([s, c], [-c, s])
}

/// Seat the rifle prop in the player's hands (body-level attach).
/// No-op on scenes that author no rifle prop.
fn place_rifle(doc: &mut WorldDoc, pp: [f32; 3], yaw: f32, weapon: Weapon) {
    let Some(prop) = doc.props.iter_mut().find(|p| p.id == RIFLE_PROP_ID) else {
        return;
    };
    let (f, r) = facing(yaw);
    prop.position = [
        pp[0] + f[0] * RIFLE_FWD + r[0] * RIFLE_RIGHT,
        pp[1] + RIFLE_H,
        pp[2] + f[1] * RIFLE_FWD + r[1] * RIFLE_RIGHT,
    ];
    prop.yaw = yaw + RIFLE_YAW_OFF;
    // One blaster model serves every gun; its scale is the held
    // gun's silhouette cue (the rocket reads chunky, the MG small).
    let s = weapon.prop_scale();
    prop.scale = [s, s, s];
    prop.enabled = true;
}

/// Muzzle world position for a shot fired by a player at `pp`
/// facing `yaw` (rifle seat plus barrel length).
fn muzzle(pp: [f32; 3], yaw: f32) -> [f32; 3] {
    let (f, r) = facing(yaw);
    [
        pp[0] + f[0] * MUZZLE_FWD + r[0] * RIFLE_RIGHT,
        pp[1] + MUZZLE_H,
        pp[2] + f[1] * MUZZLE_FWD + r[1] * RIFLE_RIGHT,
    ]
}

/// Drop the tracer / flash entries (shot FX lifetime over, or the
/// run ended and the frozen frame must not keep a stray beam).
fn clear_fx(doc: &mut WorldDoc) {
    doc.lines.retain(|l| l.id != TRACER_ID);
    doc.points.retain(|p| p.id != FLASH_ID && p.id != BOOM_ID);
}

fn next_rng(rng: &mut u32) -> u32 {
    let mut x = *rng;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *rng = x;
    x
}

fn rng_unit(rng: &mut u32) -> f32 {
    (next_rng(rng) >> 8) as f32 / (1u32 << 24) as f32
}

fn player_id(doc: &WorldDoc) -> Option<String> {
    doc.player
        .as_ref()
        .map(|w| w.id.clone())
        .or_else(|| doc.walkers.first().map(|w| w.id.clone()))
}

fn player_pos(doc: &WorldDoc) -> Option<[f32; 3]> {
    doc.player
        .as_ref()
        .or_else(|| doc.walkers.first())
        .map(|w| w.position)
}

/// Mutate the player walker in both of its dump slots (`player` and
/// the matching `walkers` entry), mirroring `world_play::write_player`.
fn with_player(doc: &mut WorldDoc, f: impl Fn(&mut WorldWalker)) {
    let Some(id) = player_id(doc) else {
        return;
    };
    if let Some(p) = doc.player.as_mut() {
        f(p);
    }
    for w in &mut doc.walkers {
        if w.id == id {
            f(w);
        }
    }
}

fn dist_xz(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[2] - b[2];
    (dx * dx + dz * dz).sqrt()
}

/// XZ distance from point `p` to the segment `a`→`b` (the laser's
/// piercing corridor test).
fn seg_dist_xz(a: [f32; 3], b: [f32; 3], p: [f32; 3]) -> f32 {
    let abx = b[0] - a[0];
    let abz = b[2] - a[2];
    let len2 = abx * abx + abz * abz;
    let t = if len2 > 1e-8 {
        (((p[0] - a[0]) * abx + (p[2] - a[2]) * abz) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let cx = a[0] + abx * t;
    let cz = a[2] + abz * t;
    ((p[0] - cx).powi(2) + (p[2] - cz).powi(2)).sqrt()
}

/// Alive cap at `elapsed`: start cap, +1 per `cap_ramp_s`, at most the
/// spec ceiling and the pool size.
fn alive_cap(spec: &SurvivorSpec, pool: usize, elapsed: f32) -> u32 {
    let ramped = if spec.cap_ramp_s > 0.0 {
        (elapsed / spec.cap_ramp_s).floor().max(0.0) as u32
    } else {
        0
    };
    spec.cap_start
        .saturating_add(ramped)
        .min(spec.cap_max)
        .min(pool as u32)
}

/// Spawn interval at `elapsed`, easing toward its floor.
fn spawn_interval(spec: &SurvivorSpec, elapsed: f32) -> f32 {
    if spec.spawn_ramp_s <= 0.0 {
        return spec.spawn_interval_min.max(0.05);
    }
    let frac = (elapsed / spec.spawn_ramp_s).clamp(0.0, 1.0);
    (spec.spawn_interval + (spec.spawn_interval_min - spec.spawn_interval) * frac).max(0.05)
}

/// The interval the *next* spawn waits: [`spawn_interval`], cut
/// to [`REFILL_MULT`] of itself while the stage stands emptier
/// than [`REFILL_UNDER`] of the cap. A wiped field refills as a
/// wave over a couple of seconds instead of trickling back one
/// body per full interval (the empty-screen lull after a clear).
pub fn spawn_pacing(spec: &SurvivorSpec, elapsed: f32, alive: u32, cap: u32) -> f32 {
    let base = spawn_interval(spec, elapsed);
    if cap > 0 && (alive as f32) < cap as f32 * REFILL_UNDER {
        (base * REFILL_MULT).max(0.05)
    } else {
        base
    }
}

/// Enemy HP multiple at `elapsed`: the authored HP holds through
/// the grace, then grows linearly until its ceiling. Applied at
/// activation — a body already on stage keeps the HP it walked in
/// with, so the curve reads as tougher arrivals, not a mid-fight
/// refill.
pub fn enemy_hp_scale(spec: &SurvivorSpec, elapsed: f32) -> f32 {
    let over = (elapsed - spec.scale_grace_s).max(0.0);
    (1.0 + over * spec.hp_scale_rate).min(spec.hp_scale_max.max(1.0))
}

/// Enemy speed multiple at `elapsed`: same shape as the HP curve,
/// gentler on both rate and ceiling. Applied every step to
/// whatever is walking, so the whole field quickens together
/// instead of only the new arrivals.
pub fn enemy_speed_scale(spec: &SurvivorSpec, elapsed: f32) -> f32 {
    let over = (elapsed - spec.scale_grace_s).max(0.0);
    (1.0 + over * spec.speed_scale_rate).min(spec.speed_scale_max.max(1.0))
}

/// Hit points of the nth boss (0-based): the boss multiple of a
/// time-scaled regular, compounded by the growth per return.
fn boss_hp(spec: &SurvivorSpec, elapsed: f32, n: u32) -> f32 {
    spec.enemy_hp
        * enemy_hp_scale(spec, elapsed)
        * spec.boss_hp_mult
        * spec.boss_hp_growth.powi(n as i32)
}

/// Fire-clip duration for the player's model, resolved once. `None`
/// when the model has no resolvable skinned clip (phase then just
/// holds, which the sampler tolerates).
fn resolve_fire_dur(doc: &WorldDoc, spec: &SurvivorSpec) -> Option<f32> {
    let player = doc.player.as_ref().or(doc.walkers.first())?;
    let model = player
        .gltf
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(player.model.trim());
    if model.is_empty() {
        return None;
    }
    let parts = load_skinned_parts(model)?;
    let skin = parts.iter().find(|p| !p.skin_joints.is_empty())?;
    crate::gltf_load::walker_clip_info(skin, Some(spec.fire_clip.as_str()), "idle")
        .map(|(_, dur)| dur)
}

/// Activate one parked pool member at the spawn ring. False when
/// the pool is exhausted. The boss takes the hulk cut when the
/// scene authors one (its authored name says so) and falls back
/// to the first parked member otherwise.
fn activate(
    doc: &mut WorldDoc,
    state: &mut SurvivorLoop,
    spec: &SurvivorSpec,
    pp: [f32; 3],
    boss: bool,
) -> bool {
    let half = doc.half.max(4.0);
    // First parked member, in authored order (deterministic).
    let parked = |state: &SurvivorLoop, w: &WorldWalker| {
        state
            .enemies
            .get(&w.id)
            .is_some_and(|e| e.mode == Mode::Parked)
    };
    let idx = if boss {
        doc.walkers
            .iter()
            .position(|w| {
                parked(state, w)
                    && state
                        .enemies
                        .get(&w.id)
                        .is_some_and(|e| e.base_name.contains("hulk"))
            })
            .or_else(|| doc.walkers.iter().position(|w| parked(state, w)))
    } else {
        doc.walkers.iter().position(|w| parked(state, w))
    };
    let Some(idx) = idx else {
        return false;
    };
    let angle = rng_unit(&mut state.rng) * std::f32::consts::TAU;
    let x = (pp[0] + angle.cos() * spec.spawn_radius).clamp(-half + 2.0, half - 2.0);
    let z = (pp[2] + angle.sin() * spec.spawn_radius).clamp(-half + 2.0, half - 2.0);
    let ground_y = doc.height_at(x, z);
    let w = &mut doc.walkers[idx];
    w.position[0] = x;
    w.position[2] = z;
    w.position[1] = ground_y;
    let base_speed = if w.survivor_speed > 0.0 {
        w.survivor_speed
    } else {
        DEFAULT_SPEED
    };
    let speed0 = base_speed
        * (0.9 + 0.2 * rng_unit(&mut state.rng))
        * if boss { BOSS_SPEED_MULT } else { 1.0 };
    w.seek = speed0 * enemy_speed_scale(spec, state.elapsed);
    w.scale = if boss {
        Some([spec.boss_scale, spec.boss_scale, spec.boss_scale])
    } else {
        None
    };
    w.clip = rng_unit(&mut state.rng);
    w.anim_blend = 1.0;
    if let Some(mc) = w.move_clip.clone() {
        w.clip_name = Some(mc);
    } else if w.clip_name.is_none() {
        w.clip_name = Some("walk".into());
    }
    let id = w.id.clone();
    if let Some(e) = state.enemies.get_mut(&id) {
        e.mode = Mode::Alive;
        e.hp = if boss {
            boss_hp(spec, state.elapsed, state.bosses_spawned)
        } else {
            spec.enemy_hp * enemy_hp_scale(spec, state.elapsed)
        };
        e.boss = boss;
        e.speed0 = speed0;
        e.hurt_t = 0.0;
        e.dying_t = 0.0;
        let base = e.base_name.clone();
        doc.walkers[idx].name = base;
    }
    if boss {
        state.bosses_spawned += 1;
    }
    true
}

/// Advance the loop one step. Returns true on the step the player
/// falls (and on every later step, so the caller can latch game over).
pub fn step(doc: &mut WorldDoc, state: &mut SurvivorLoop, spec: &SurvivorSpec, dt: f32) -> bool {
    if state.over || dt <= 0.0 {
        return state.over;
    }
    if state.level_pending {
        // Choosing a reward: the world holds still (world_play also
        // skips its walker step), so nothing here may advance.
        return false;
    }
    state.elapsed += dt;
    state.fire_cd -= dt;
    for c in &mut state.cue_t {
        *c = (*c - dt).max(0.0);
    }
    if !state.bgm_on {
        // First live step of the run: the music starts with the
        // fighting, not the title (the loop is rebuilt per run, so
        // a restart re-arms this). The scene's `game.audio` block
        // names the loop file, as in the arena kit.
        state.bgm_on = true;
        crate::mixer::play_bgm("bgm", 0.5);
    }
    let Some(pp) = player_pos(doc) else {
        return false;
    };
    if !state.face_seeded {
        state.face_yaw = doc
            .player
            .as_ref()
            .or_else(|| doc.walkers.first())
            .map(|w| w.yaw)
            .unwrap_or(0.0);
        state.face_seeded = true;
    }
    // Shot FX lifetime: the tracer / flash live for FX_S, then the
    // entries come back out of the doc.
    if state.fx_t > 0.0 {
        state.fx_t -= dt;
        if state.fx_t <= 0.0 {
            clear_fx(doc);
        }
    }
    let pool = state.enemies.len();

    // --- The boss keeps its own schedule, off the cap: when it
    // comes due, a parked member walks in scaled up. A full pool
    // delays the entrance (the due time simply stays due), it
    // never cancels it. ---
    if state.elapsed >= state.next_boss_t && activate(doc, state, spec, pp, true) {
        state.cue("elite", CUE_EVENT, 0.5);
        state.next_boss_t = if spec.boss_interval_s > 0.0 {
            state.elapsed + spec.boss_interval_s
        } else {
            f32::MAX
        };
    }

    // --- Spawning: top the stage up to the (ramping) alive cap. ---
    let mut alive = state
        .enemies
        .values()
        .filter(|e| e.mode == Mode::Alive)
        .count() as u32;
    let cap = alive_cap(spec, pool, state.elapsed);
    state.spawn_t -= dt;
    while alive < cap && state.spawn_t <= 0.0 {
        if !activate(doc, state, spec, pp, false) {
            break;
        }
        alive += 1;
        state.spawn_t += spawn_pacing(spec, state.elapsed, alive, cap);
    }
    if state.spawn_t < 0.0 {
        state.spawn_t = 0.0;
    }
    state.alive_now = alive;

    // --- The speed curve walks the whole field: every living
    // enemy's cruise is its activation speed under the current
    // multiple, so time quickens the ones already closing in, not
    // just the next arrivals. ---
    let speed_scale = enemy_speed_scale(spec, state.elapsed);
    for w in &mut doc.walkers {
        let speed0 = state
            .enemies
            .get(&w.id)
            .filter(|e| e.mode == Mode::Alive)
            .map(|e| e.speed0);
        if let Some(speed0) = speed0 {
            w.seek = speed0 * speed_scale;
        }
    }

    // --- Target selection. The tap lock outranks the auto-aim
    // (Moonlit's hybrid rule): while the tapped foe lives, it is
    // the target whenever it is inside the stick band, and no
    // closer rival steals from it — the player chose this one.
    // The lock itself survives a foe stepping out of the band
    // (the gun then falls back to the nearest, as the kit's
    // `pick_aim` does) and is released only by its death, a
    // re-tap on it, or a tap on bare ground. Without a tap lock,
    // the standing rule runs: keep the current target while it
    // lives and stays near reach — but a much closer living enemy
    // steals the aim, because rounds spent finishing a distant
    // body while a near one bites are rounds wasted. Only living
    // enemies are ever aimed at (no corpse fire). ---
    let mut nearest: Option<(usize, f32)> = None;
    for (i, w) in doc.walkers.iter().enumerate() {
        if !state
            .enemies
            .get(&w.id)
            .is_some_and(|e| e.mode == Mode::Alive)
        {
            continue;
        }
        let d = dist_xz(pp, w.position);
        if d <= spec.fire_range && nearest.is_none_or(|(_, bd)| d < bd) {
            nearest = Some((i, d));
        }
    }
    let mut target: Option<(usize, f32)> = None;
    if let Some(lock_id) = state.tap_lock.clone() {
        let locked = doc
            .walkers
            .iter()
            .enumerate()
            .find(|(_, w)| w.id == lock_id)
            .filter(|(_, w)| {
                state
                    .enemies
                    .get(&w.id)
                    .is_some_and(|e| e.mode == Mode::Alive)
            });
        match locked {
            Some((i, w)) => {
                let d = dist_xz(pp, w.position);
                if d <= spec.fire_range * TARGET_STICK {
                    target = Some((i, d));
                }
            }
            None => state.tap_lock = None,
        }
    }
    if target.is_none() {
        if let Some(aid) = state.aim_id.as_deref() {
            if let Some((i, w)) = doc.walkers.iter().enumerate().find(|(_, w)| w.id == aid) {
                let d = dist_xz(pp, w.position);
                if state
                    .enemies
                    .get(&w.id)
                    .is_some_and(|e| e.mode == Mode::Alive)
                    && d <= spec.fire_range * TARGET_STICK
                {
                    target = Some((i, d));
                    if let Some((ni, nd)) = nearest {
                        let steal =
                            ni != i && (nd < d * THREAT_FRAC || (nd <= THREAT_R && d > THREAT_R));
                        if steal {
                            target = Some((ni, nd));
                        }
                    }
                }
            }
        }
    }
    if target.is_none() {
        target = nearest;
    }
    state.aim_id = target.map(|(i, _)| doc.walkers[i].id.clone());

    // --- Facing: the aim while locked, otherwise the facing the
    // walker step just chose (movement direction, or the last one
    // when idle). The written yaw eases at TURN_RATE either way, so
    // neither a target switch nor a direction change snaps. ---
    let desired = if let Some((ti, _)) = target {
        let tp = doc.walkers[ti].position;
        (tp[0] - pp[0]).atan2(tp[2] - pp[2])
    } else {
        doc.player
            .as_ref()
            .or_else(|| doc.walkers.first())
            .map(|w| w.yaw)
            .unwrap_or(state.face_yaw)
    };
    state.face_yaw = turn_toward(state.face_yaw, desired, TURN_RATE * dt);
    let face_yaw = state.face_yaw;
    place_rifle(doc, pp, face_yaw, state.weapon);
    // The held gun's glow point rides the muzzle (tinted by
    // weapon), and an accent ring plus a bobbing head marker keep
    // the grey-on-grey player findable in a brawl — the toon
    // outline in this engine covers props, not skinned walkers,
    // so the marker rides the point-cloud plumbing instead.
    doc.points.retain(|p| p.id != GUN_ID && p.id != RING_ID);
    doc.points.push(PointCloud {
        id: GUN_ID.into(),
        positions: vec![muzzle(pp, face_yaw)],
        color: state.weapon.drop_color(),
        size: 0.26,
    });
    let mut ring: Vec<[f32; 3]> = (0..14)
        .map(|i| {
            let a = i as f32 / 14.0 * std::f32::consts::TAU;
            [pp[0] + a.cos() * 0.62, pp[1] + 0.06, pp[2] + a.sin() * 0.62]
        })
        .collect();
    ring.push([
        pp[0],
        pp[1] + 2.3 + (state.elapsed * 2.0).sin() * 0.08,
        pp[2],
    ]);
    doc.points.push(PointCloud {
        id: RING_ID.into(),
        positions: ring,
        color: [110, 255, 190, 255],
        size: 0.2,
    });
    // The boss wears the same trick in red: a wide ring at its
    // feet plus a point bobbing over its head, so the big body
    // reads out of the grey crowd before it is on top of the
    // player.
    doc.points.retain(|p| p.id != BOSS_ID);
    if let Some(bp) = doc
        .walkers
        .iter()
        .find(|w| {
            state
                .enemies
                .get(&w.id)
                .is_some_and(|e| e.boss && e.mode == Mode::Alive)
        })
        .map(|w| w.position)
    {
        let r = 0.62 * spec.boss_scale;
        let mut ring: Vec<[f32; 3]> = (0..18)
            .map(|i| {
                let a = i as f32 / 18.0 * std::f32::consts::TAU;
                [bp[0] + a.cos() * r, bp[1] + 0.06, bp[2] + a.sin() * r]
            })
            .collect();
        ring.push([
            bp[0],
            bp[1] + 2.3 * spec.boss_scale + (state.elapsed * 2.0).sin() * 0.1,
            bp[2],
        ]);
        doc.points.push(PointCloud {
            id: BOSS_ID.into(),
            positions: ring,
            color: [255, 84, 60, 255],
            size: 0.3,
        });
    }
    // The tap-locked foe wears the same trick in amber, so the
    // player's chosen target reads out of the crowd the same way
    // the boss does (and the player, in mint, in theirs).
    doc.points.retain(|p| p.id != LOCK_ID);
    if let Some(lp) = state.tap_lock.as_deref().and_then(|id| {
        doc.walkers
            .iter()
            .find(|w| {
                w.id == id
                    && state
                        .enemies
                        .get(&w.id)
                        .is_some_and(|e| e.mode == Mode::Alive)
            })
            .map(|w| w.position)
    }) {
        let mut ring: Vec<[f32; 3]> = (0..12)
            .map(|i| {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                [lp[0] + a.cos() * 0.55, lp[1] + 0.06, lp[2] + a.sin() * 0.55]
            })
            .collect();
        ring.push([
            lp[0],
            lp[1] + 2.2 + (state.elapsed * 2.0).sin() * 0.08,
            lp[2],
        ]);
        doc.points.push(PointCloud {
            id: LOCK_ID.into(),
            positions: ring,
            color: [255, 255, 255, 255],
            size: 0.26,
        });
    }

    // --- Auto-fire at the locked target, but only inside
    // `fire_range`: a lock held in the stick band aims the body
    // without spending rounds (and without the fire clip, which
    // would read as shooting at nothing). ---
    let in_range = target.is_some_and(|(_, d)| d <= spec.fire_range);
    if let Some((ti, _)) = target.filter(|_| in_range) {
        let tp = doc.walkers[ti].position;
        if state.fire_dur.is_none() {
            state.fire_dur = Some(resolve_fire_dur(doc, spec));
        }
        state.fire_phase += dt;
        if let Some(Some(dur)) = state.fire_dur {
            if dur > 1e-6 {
                state.fire_phase = state.fire_phase.rem_euclid(dur);
            }
        }
        let phase = state.fire_phase;
        let clip = spec.fire_clip.clone();
        with_player(doc, |p| {
            p.yaw = face_yaw;
            p.face = face_yaw;
            p.clip_name = Some(clip.clone());
            p.clip = phase;
            p.anim_blend += (1.0 - p.anim_blend) * (dt * 8.0).min(1.0);
        });
        if state.fire_cd <= 0.0 {
            state.fire_cd = state.fire_interval(spec);
            state.fire_phase = 0.0;
            state.shots += 1;
            state.cue("shoot", CUE_SHOOT, 0.08);
            let weapon = state.weapon;
            let dmg = state.shot_damage(spec);
            let from = muzzle(pp, face_yaw);
            // The shot's shape depends on the gun: a single hit, a
            // piercing beam down the aim line, or a rocket whose
            // impact splashes everything standing nearby.
            let mut hits: Vec<(usize, f32)> = Vec::new();
            let mut beam_end = [tp[0], tp[1] + 1.1, tp[2]];
            let mut boom: Option<[f32; 3]> = None;
            match weapon {
                Weapon::Rifle | Weapon::MachineGun => hits.push((ti, dmg)),
                Weapon::Laser => {
                    let dx = tp[0] - pp[0];
                    let dz = tp[2] - pp[2];
                    let len = (dx * dx + dz * dz).sqrt().max(1e-4);
                    let end = [
                        pp[0] + dx / len * spec.fire_range,
                        pp[1],
                        pp[2] + dz / len * spec.fire_range,
                    ];
                    beam_end = [end[0], tp[1] + 1.1, end[2]];
                    for (i, w) in doc.walkers.iter().enumerate() {
                        if !state
                            .enemies
                            .get(&w.id)
                            .is_some_and(|e| e.mode == Mode::Alive)
                        {
                            continue;
                        }
                        if seg_dist_xz(pp, end, w.position) <= LASER_CORRIDOR {
                            hits.push((i, dmg));
                        }
                    }
                    if hits.is_empty() {
                        hits.push((ti, dmg));
                    }
                }
                Weapon::Rocket => {
                    boom = Some([tp[0], tp[1] + 0.6, tp[2]]);
                    for (i, w) in doc.walkers.iter().enumerate() {
                        if !state
                            .enemies
                            .get(&w.id)
                            .is_some_and(|e| e.mode == Mode::Alive)
                        {
                            continue;
                        }
                        if dist_xz(tp, w.position) <= SPLASH_R {
                            let d = if i == ti {
                                dmg * (1.0 + SPLASH_MULT)
                            } else {
                                dmg * SPLASH_MULT
                            };
                            hits.push((i, d));
                        }
                    }
                    if hits.is_empty() {
                        hits.push((ti, dmg));
                    }
                }
            }
            // Sell the shot: a tracer beam muzzle → hit point and a
            // muzzle point sprite, tinted and sized by the gun, plus
            // the rocket's impact burst when there is one.
            let (tracer_color, tracer_width) = weapon.tracer();
            doc.lines.retain(|l| l.id != TRACER_ID);
            doc.lines.push(FatLine {
                id: TRACER_ID.into(),
                points: vec![from, beam_end],
                color: tracer_color,
                width: tracer_width,
            });
            doc.points.retain(|p| p.id != FLASH_ID && p.id != BOOM_ID);
            doc.points.push(PointCloud {
                id: FLASH_ID.into(),
                positions: vec![from],
                color: [255, 190, 80, 255],
                size: 0.3,
            });
            if let Some(b) = boom {
                doc.points.push(PointCloud {
                    id: BOOM_ID.into(),
                    positions: vec![b],
                    color: [255, 150, 70, 255],
                    size: 1.5,
                });
                state.cue("boom", CUE_EVENT, 0.2);
            }
            state.fx_t = weapon.fx_seconds();
            for (i, hd) in hits {
                state.apply_hit(doc, i, hd, spec);
            }
        }
    } else {
        state.fire_phase = 0.0;
        // No lock in range: still write the eased facing so movement
        // turns the walker step chose do not snap either.
        with_player(doc, |p| {
            p.yaw = face_yaw;
            p.face = face_yaw;
        });
    }

    // --- Contact damage, hurt flashes, dying bodies. ---
    let mut damage = 0.0f32;
    let ids: Vec<String> = doc.walkers.iter().map(|w| w.id.clone()).collect();
    for id in ids {
        let Some((mode, is_boss)) = state.enemies.get(&id).map(|e| (e.mode, e.boss)) else {
            continue;
        };
        let Some(w) = doc.walkers.iter_mut().find(|w| w.id == id) else {
            continue;
        };
        match mode {
            Mode::Alive => {
                let stop = if w.seek_stop > 0.0 {
                    w.seek_stop
                } else {
                    DEFAULT_STOP
                };
                // The boss's bulk reaches further than a regular's
                // stop ring: what touches the player is what bites.
                let reach = stop
                    + 0.35
                    + if is_boss {
                        (spec.boss_scale - 1.0).max(0.0) * 0.8
                    } else {
                        0.0
                    };
                if dist_xz(pp, w.position) <= reach {
                    let mult = if is_boss { spec.boss_contact_mult } else { 1.0 };
                    damage += spec.contact_dps * mult * dt;
                }
                if let Some(e) = state.enemies.get_mut(&id) {
                    if e.hurt_t > 0.0 {
                        e.hurt_t -= dt;
                        if e.hurt_t <= 0.0 && w.name == "hurt" {
                            w.name = e.base_name.clone();
                        }
                    }
                }
            }
            Mode::Dying => {
                w.position[1] -= SINK_RATE * dt;
                let done = if let Some(e) = state.enemies.get_mut(&id) {
                    e.dying_t += dt;
                    e.dying_t >= DYING_S
                } else {
                    false
                };
                if done {
                    w.position[1] = PARK_Y;
                    w.seek = 0.0;
                    // The boss's bulk is loop state: parking strips
                    // it, so the member recycles as a regular.
                    w.scale = None;
                    if let Some(e) = state.enemies.get_mut(&id) {
                        e.mode = Mode::Parked;
                        e.boss = false;
                        w.name = e.base_name.clone();
                    }
                }
            }
            Mode::Parked => {}
        }
    }
    // --- Drops: magnet drift, then pickups by walking over them.
    // A banked gem can trip the level-up; the cards freeze the
    // world from the *next* step, so the killing field stays as it
    // was at the moment of the choice. ---
    let collect_r = COLLECT_R + state.magnet_r * 0.5;
    let magnet_r = MAGNET_R + state.magnet_r;
    let mut banked = false;
    let mut healed = false;
    let mut swapped: Option<Weapon> = None;
    state.drops.retain_mut(|d| {
        let dist = dist_xz(pp, d.pos);
        if dist <= collect_r {
            match d.kind {
                DropKind::Gem => {
                    state.gems += 1;
                    banked = true;
                }
                DropKind::Heal => {
                    state.hp = (state.hp + HEAL_AMOUNT).min(state.hp_max);
                    healed = true;
                }
                DropKind::Weapon(w) => {
                    swapped = Some(std::mem::replace(&mut state.weapon, w));
                }
            }
            return false;
        }
        // Guns do not magnet: picking one up is a decision, and a
        // swapped-out gun must stay where it was left.
        if !matches!(d.kind, DropKind::Weapon(_)) && dist <= magnet_r && dist > 1e-4 {
            let pull = (MAGNET_SPEED * dt / dist).min(1.0);
            d.pos[0] += (pp[0] - d.pos[0]) * pull;
            d.pos[2] += (pp[2] - d.pos[2]) * pull;
        }
        true
    });
    if let Some(old) = swapped {
        state.cue("pick", CUE_EVENT, 0.15);
        // The old gun falls behind the player — outside collect
        // reach, so the swap cannot instantly undo itself; walking
        // back to it swaps again.
        let (f, _) = facing(state.face_yaw);
        state.drops.push(Drop {
            kind: DropKind::Weapon(old),
            pos: [pp[0] - f[0] * 1.7, pp[1] + 0.35, pp[2] - f[1] * 1.7],
        });
        while state.drops.len() > MAX_DROPS {
            state.drops.remove(0);
        }
    }
    // Pickups chirp as they land (after the drop loop, so the
    // throttle sees at most one of each per step).
    if banked {
        state.cue("gem", CUE_GEM, 0.06);
    }
    if healed {
        state.cue("pickup", CUE_EVENT, 0.15);
    }
    if banked && !state.level_pending && state.gems >= xp_need(state.level) {
        state.gems -= xp_need(state.level);
        state.level += 1;
        state.roll_choices();
        state.level_pending = true;
        state.cue("levelup", CUE_EVENT, 0.3);
        // The cards are modal: the steering thumb is let go now, so
        // neither a thumb left resting nor a release the page never
        // delivered survives into the resumed run.
        state.drop_stick();
    }
    sync_drops(doc, state);

    state.hp -= damage * state.armor_mult;
    if damage > 0.0 {
        // Teeth on skin, throttled: contact is continuous, the
        // sound lands in beats.
        state.cue("hurt", CUE_HURT, 0.45);
    }
    if state.hp <= 0.0 {
        state.hp = 0.0;
        state.over = true;
        state.tap_lock = None;
        crate::mixer::stop_bgm(0.6);
        state.cue("gameover", CUE_EVENT, 0.5);
        clear_fx(doc);
        doc.points.retain(|p| {
            p.id != GEM_ID
                && p.id != HEAL_ID
                && p.id != BOSS_ID
                && p.id != LOCK_ID
                && !ALL_WEAPONS.iter().any(|w| w.cloud_id() == p.id)
        });
        return true;
    }
    false
}

/// Republish the drop point clouds from the loop's drop list (the
/// doc is rebuilt from scratch every frame, so the entries mirror
/// the list exactly; culled or collected drops simply vanish).
fn sync_drops(doc: &mut WorldDoc, state: &SurvivorLoop) {
    doc.points.retain(|p| {
        p.id != GEM_ID && p.id != HEAL_ID && !ALL_WEAPONS.iter().any(|w| w.cloud_id() == p.id)
    });
    let gems: Vec<[f32; 3]> = state
        .drops
        .iter()
        .filter(|d| d.kind == DropKind::Gem)
        .map(|d| d.pos)
        .collect();
    if !gems.is_empty() {
        doc.points.push(PointCloud {
            id: GEM_ID.into(),
            positions: gems,
            color: [255, 232, 54, 255],
            size: 0.4,
        });
    }
    let heals: Vec<[f32; 3]> = state
        .drops
        .iter()
        .filter(|d| d.kind == DropKind::Heal)
        .map(|d| d.pos)
        .collect();
    if !heals.is_empty() {
        doc.points.push(PointCloud {
            id: HEAL_ID.into(),
            positions: heals,
            color: [255, 120, 140, 255],
            size: 0.45,
        });
    }
    // One cloud per weapon kind, so each ground gun carries its
    // own tint (amber MG, cyan laser, orange rocket, pale
    // rifle) and reads before the player commits to the swap.
    for w in ALL_WEAPONS {
        let pts: Vec<[f32; 3]> = state
            .drops
            .iter()
            .filter(|d| d.kind == DropKind::Weapon(w))
            .map(|d| d.pos)
            .collect();
        if !pts.is_empty() {
            doc.points.push(PointCloud {
                id: w.cloud_id().into(),
                positions: pts,
                color: w.drop_color(),
                size: 0.55,
            });
        }
    }
}

fn fmt_time(s: f32) -> String {
    let t = s.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
}

/// Survivor HUD: HP bar + time / kills while playing, a game-over
/// panel on Complete. ASCII only, so no font-subset concerns.
/// Layout and sizes ride the arena kits' HUD scale
/// (`stick_hud_scale` — Moonlit's `hud_scale`), so the cluster
/// reads at the kit's phone size instead of the smaller world
/// scale this HUD started on.
pub fn build_hud(state: &SurvivorLoop, phase: GamePhase, width: u32, height: u32) -> DrawList {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let s = stick_hud_scale(w, h);
    let pad = 14.0 * s;
    // The shell draws its own band across the very top of the page
    // (title / phase line), so the top HUD cluster starts below it
    // instead of sliding underneath: a fixed safe-area offset on
    // top of the pad, both in HUD-scale units.
    let top = pad + 36.0 * s;
    let mut quads = Vec::new();
    let mut texts = Vec::new();
    match phase {
        GamePhase::Playing => {
            let bar_w = (w * 0.46).min(235.0 * s);
            let bar_h = 20.0 * s;
            let frac = if state.hp_max > 0.0 {
                (state.hp / state.hp_max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            // The kit's framed bar: a dark rim, the well, the fill,
            // a sheen on the fill's top half, the number inside.
            quads.push(Quad::new(
                pad - 2.5 * s,
                top - 2.5 * s,
                bar_w + 5.0 * s,
                bar_h + 5.0 * s,
                [10, 10, 18, 215],
            ));
            quads.push(Quad::new(pad, top, bar_w, bar_h, [24, 20, 20, 220]));
            let fill = if frac > 0.35 {
                [96, 200, 110, 255]
            } else {
                [226, 64, 64, 255]
            };
            quads.push(Quad::new(pad, top, bar_w * frac, bar_h, fill));
            quads.push(Quad::new(
                pad,
                top,
                bar_w * frac,
                bar_h * 0.4,
                [255, 255, 255, 60],
            ));
            texts.push(
                TextQuad::new(
                    &format!("HP {}", state.hp.ceil() as i32),
                    pad + 7.0 * s,
                    top + 2.5 * s,
                    14.5 * s,
                    [16, 18, 16, 255],
                )
                .with_face(TextFace::Display),
            );
            texts.push(
                TextQuad::new(
                    &format!("TIME {}", fmt_time(state.elapsed)),
                    w - pad,
                    top - 2.0 * s,
                    19.0 * s,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Right)
                .with_face(TextFace::Display),
            );
            texts.push(
                TextQuad::new(
                    &format!("KILLS {}", state.kills),
                    w - pad,
                    top + 24.0 * s,
                    19.0 * s,
                    [240, 196, 72, 255],
                )
                .aligned(TextAlign::Right)
                .with_face(TextFace::Display),
            );
            if state
                .enemies
                .values()
                .any(|e| e.boss && e.mode == Mode::Alive)
            {
                texts.push(
                    TextQuad::new(
                        "BOSS",
                        w - pad,
                        top + 50.0 * s,
                        19.0 * s,
                        [255, 84, 60, 255],
                    )
                    .aligned(TextAlign::Right)
                    .with_face(TextFace::Display),
                );
            }
            texts.push(
                TextQuad::new(
                    &format!("LV {}", state.level),
                    pad,
                    top + bar_h + 9.0 * s,
                    16.0 * s,
                    [150, 220, 255, 255],
                )
                .with_face(TextFace::Display),
            );
            texts.push(
                TextQuad::new(
                    &format!("XP {}/{}", state.gems, xp_need(state.level)),
                    pad + 64.0 * s,
                    top + bar_h + 9.0 * s,
                    16.0 * s,
                    [255, 232, 54, 255],
                )
                .with_face(TextFace::Display),
            );
            texts.push(
                TextQuad::new(
                    state.weapon.name(),
                    pad,
                    top + bar_h + 33.0 * s,
                    16.0 * s,
                    state.weapon.drop_color(),
                )
                .with_face(TextFace::Display),
            );
            if state.level_pending {
                // The level-up cards. The rects are the tap targets
                // (`level_card_rects` is shared with the hit-test).
                quads.push(Quad::new(0.0, 0.0, w, h, [6, 8, 10, 150]));
                texts.push(
                    TextQuad::new(
                        "LEVEL UP!",
                        w * 0.5,
                        h * 0.32 - 66.0 * s,
                        32.0 * s,
                        [240, 196, 72, 255],
                    )
                    .aligned(TextAlign::Center)
                    .with_face(TextFace::Display),
                );
                for (i, r) in level_card_rects(w, h).iter().enumerate() {
                    quads.push(Quad::new(r[0], r[1], r[2], r[3], [18, 22, 30, 240]));
                    quads.push(Quad::new(r[0], r[1], r[2], 5.0 * s, [240, 196, 72, 255]));
                    let c = state.choices[i];
                    texts.push(
                        TextQuad::new(
                            c.name(),
                            r[0] + 18.0 * s,
                            r[1] + 13.0 * s,
                            21.0 * s,
                            [255, 255, 255, 255],
                        )
                        .with_face(TextFace::Display),
                    );
                    texts.push(
                        TextQuad::new(
                            c.desc(),
                            r[0] + 18.0 * s,
                            r[1] + 48.0 * s,
                            16.0 * s,
                            [170, 200, 220, 255],
                        )
                        .with_face(TextFace::Display),
                    );
                }
            } else {
                // The floating stick: live (base + jelly knob at the
                // thumb) while a finger is down, the resting hint at
                // its home corner otherwise — the arena kits' draw.
                let rad = stick_radius(w, h);
                let s = stick_hud_scale(w, h);
                let look = stick::StickLook::default();
                let mut tmp = DrawList::default();
                match state.stick_finger {
                    Some(f) => {
                        let k = stick::knob(state.stick_origin, f, rad);
                        stick::draw(&mut tmp, state.stick_origin, k, rad, s, true, &look);
                    }
                    None => {
                        let c = stick_center(w, h);
                        stick::draw(&mut tmp, c, c, rad, s, false, &look);
                    }
                }
                quads.extend(tmp.quads);
            }
        }
        GamePhase::Complete => {
            quads.push(Quad::new(0.0, 0.0, w, h, [8, 10, 8, 150]));
            quads.push(Quad::new(0.0, h * 0.24, w, h * 0.34, [12, 14, 12, 220]));
            texts.push(
                TextQuad::new("GAME OVER", w * 0.5, h * 0.30, 44.0 * s, [240, 96, 84, 255])
                    .aligned(TextAlign::Center)
                    .with_face(TextFace::Display),
            );
            texts.push(
                TextQuad::new(
                    &format!(
                        "SURVIVED {}   KILLS {}",
                        fmt_time(state.elapsed),
                        state.kills
                    ),
                    w * 0.5,
                    h * 0.42,
                    22.0 * s,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Center)
                .with_face(TextFace::Display),
            );
            let score = state.kills * 250 + state.elapsed as u32;
            let bar = (score.min(2400) as f32 / 2400.0).clamp(0.08, 1.0);
            quads.push(Quad::new(
                w * 0.22,
                h * 0.50,
                w * 0.56 * bar,
                16.0 * s,
                [240, 196, 72, 255],
            ));
        }
        GamePhase::Title => {
            quads.push(Quad::new(0.0, 0.0, w, h, [10, 14, 12, 150]));
            texts.push(
                TextQuad::new(
                    "SURVIVOR",
                    w * 0.5,
                    h * 0.34,
                    38.0 * s,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Center)
                .with_face(TextFace::Display),
            );
        }
    }
    DrawList {
        // Dusk tone matching the survivor scene's authored `sky`
        // (the default outdoor blue reads as a cheerful noon).
        clear: [86, 68, 70, 255],
        quads,
        texts,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world_play::WorldPlay;
    use serde_json::json;

    /// Player at the origin + `n` parked pool zombies, survivor spec
    /// inline. Mirrors the world_play test dumps.
    fn survivor_dump(n: usize, spec: serde_json::Value) -> String {
        let player = json!({
            "id": "walker:player", "type": "walker", "name": "player",
            "position": [0.0, 0.0, 0.0], "yaw": 0.0, "face": 0.0,
            "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
            "clip": 0.0, "clip_name": "rifle_idle", "move_clip": "rifle_walk"
        });
        let mut walkers = vec![player.clone()];
        for i in 0..n {
            walkers.push(json!({
                "id": format!("walker:z{i}"), "type": "walker",
                "name": format!("zombie_{i}"),
                "position": [0.0, -100.0, 0.0], "yaw": 0.0, "face": 0.0,
                "on_ground": true, "model": "capsule", "gltf": "walk_skinned.gltf",
                "clip": 0.0, "clip_name": "walk", "move_clip": "walk",
                "seek_stop": 1.7, "survivor_enemy": true, "survivor_speed": 1.4
            }));
        }
        json!({
            "version": 1, "half": 30.0, "floor_y": 0.0, "gravity": 9.8,
            "coins": 0, "player": player, "walkers": walkers,
            "props": [], "lights": [], "cameras": [{
                "id": "camera:main", "type": "camera", "name": "main",
                "fov": 50.0, "position": [0.0, 3.0, 11.5],
                "target": [0.0, 1.0, -3.0]
            }],
            "survivor": spec
        })
        .to_string()
    }

    fn enemy<'a>(play: &'a WorldPlay, id: &str) -> &'a WorldWalker {
        play.doc.walkers.iter().find(|w| w.id == id).unwrap()
    }

    #[test]
    fn pool_spawns_up_to_cap_and_closes_in() {
        // shot_damage 0: the horde stays alive so the cap is observable
        // (with a live gun the spawns would be shot down mid-test).
        let dump = survivor_dump(
            6,
            json!({ "cap_start": 3, "cap_max": 3, "spawn_radius": 12.0, "shot_damage": 0.0 }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        // ~4 s: three spawns (interval 1.1 s), all walking in.
        for _ in 0..240 {
            play.tick(dt);
        }
        assert_eq!(play.survivor_loop.alive_now, 3);
        for i in 0..3 {
            let z = enemy(&play, &format!("walker:z{i}"));
            assert!(z.seek > 0.0, "spawned enemy must seek, z{i}");
            assert!(z.position[1] > -1.0, "spawned enemy must be on stage");
            let d = (z.position[0].powi(2) + z.position[2].powi(2)).sqrt();
            assert!(d < 12.0, "seeker must have closed in from the ring, d={d}");
        }
        // The rest of the pool stays parked off-stage.
        let z5 = enemy(&play, "walker:z5");
        assert_eq!(z5.position[1], PARK_Y);
        assert_eq!(z5.seek, 0.0);
    }

    #[test]
    fn auto_fire_kills_and_recycles() {
        // One enemy, weak: 2 shots (34 dmg vs 50 hp). It spawns inside
        // fire range and is shot down without ever reaching the player.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 8.0,
                "enemy_hp": 50.0, "fire_interval": 0.1
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..600 {
            play.tick(dt);
            if play.survivor_loop.kills >= 1 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.kills, 1, "auto-fire must kill the enemy");
        assert_eq!(play.survivor_loop.hp, play.survivor_loop.hp_max);
        // Player faced the target and played the fire clip.
        let p = play.doc.player.as_ref().unwrap();
        assert_eq!(p.clip_name.as_deref(), Some("rifle_fire"));
        // Recycle: the parked body spawns again and is shot down a
        // second time — a second kill proves the pool looped.
        for _ in 0..1800 {
            play.tick(dt);
            if play.survivor_loop.kills >= 2 {
                break;
            }
        }
        assert!(
            play.survivor_loop.kills >= 2,
            "pool must recycle (kills {})",
            play.survivor_loop.kills
        );
    }

    #[test]
    fn contact_damage_ends_the_run() {
        // Fragile player, harmless gun: the first zombie walks in and
        // mauls the player to game over.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 6.0,
                "player_hp": 5.0, "shot_damage": 0.0, "contact_dps": 10.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        let mut over = false;
        for _ in 0..3600 {
            play.tick(dt);
            if play.survivor_loop.over {
                over = true;
                break;
            }
        }
        assert!(over, "contact damage must end the run");
        assert_eq!(play.survivor_loop.hp, 0.0);
        assert_eq!(play.game.phase, GamePhase::Complete);
        let hud = play.build_hud(960, 720);
        assert!(
            hud.texts.iter().any(|t| t.text.contains("GAME OVER")),
            "result HUD must show the game-over panel"
        );
    }

    #[test]
    fn cap_ramps_with_survival_time() {
        let spec: SurvivorSpec = serde_json::from_value(json!({
            "cap_start": 2, "cap_max": 5, "cap_ramp_s": 10.0
        }))
        .unwrap();
        assert_eq!(alive_cap(&spec, 18, 0.0), 2);
        assert_eq!(alive_cap(&spec, 18, 25.0), 4);
        assert_eq!(alive_cap(&spec, 18, 999.0), 5);
        assert_eq!(alive_cap(&spec, 3, 999.0), 3, "pool size clamps the cap");
    }

    #[test]
    fn difficulty_curves_hold_the_grace_then_climb_to_their_caps() {
        // The shipped curve numbers (spec defaults = the scene's):
        // a 30 s grace where the authored numbers hold exactly,
        // then HP +1%/s to a ×5 ceiling, speed +0.16%/s to ×1.45.
        let spec: SurvivorSpec = serde_json::from_value(json!({})).unwrap();
        assert_eq!(enemy_hp_scale(&spec, 0.0), 1.0);
        assert_eq!(enemy_hp_scale(&spec, 30.0), 1.0);
        assert_eq!(enemy_speed_scale(&spec, 30.0), 1.0);
        assert!((enemy_hp_scale(&spec, 90.0) - 1.6).abs() < 1e-4);
        assert!((enemy_hp_scale(&spec, 180.0) - 2.5).abs() < 1e-4);
        assert_eq!(enemy_hp_scale(&spec, 430.0), 5.0);
        assert_eq!(enemy_hp_scale(&spec, 3600.0), 5.0);
        assert!((enemy_speed_scale(&spec, 90.0) - 1.096).abs() < 1e-4);
        assert_eq!(enemy_speed_scale(&spec, 400.0), 1.45);
        assert_eq!(enemy_speed_scale(&spec, 3600.0), 1.45);
    }

    #[test]
    fn spawn_density_ramps_and_the_cap_stays_inside_the_draw_budget() {
        // The shipped scene's density numbers: spawns start at
        // 0.5 s and ease to a 0.22 s floor over two minutes, while
        // the alive cap climbs to 19 (the draw-budget ceiling) —
        // density comes from turnover, not from flooding the stage.
        let spec: SurvivorSpec = serde_json::from_value(json!({
            "cap_start": 13, "cap_max": 19, "cap_ramp_s": 10.0,
            "spawn_interval": 0.5, "spawn_interval_min": 0.22,
            "spawn_ramp_s": 120.0
        }))
        .unwrap();
        assert!((spawn_interval(&spec, 0.0) - 0.5).abs() < 1e-6);
        assert!((spawn_interval(&spec, 60.0) - 0.36).abs() < 1e-6);
        assert!((spawn_interval(&spec, 120.0) - 0.22).abs() < 1e-6);
        assert!((spawn_interval(&spec, 999.0) - 0.22).abs() < 1e-6);
        assert_eq!(alive_cap(&spec, 20, 0.0), 13);
        assert_eq!(alive_cap(&spec, 20, 60.0), 19);
        assert_eq!(alive_cap(&spec, 20, 999.0), 19);
        assert_eq!(alive_cap(&spec, 18, 999.0), 18, "the pool clamps the cap");
    }

    #[test]
    fn boss_walks_in_on_schedule_pays_out_and_returns_tougher() {
        // No regular spawns (cap 0): the boss schedule is the only
        // way in. The gun starts silent (damage 0) so the entrance,
        // the bulk and the bite can be measured before the payout.
        let dump = survivor_dump(
            4,
            json!({
                "cap_start": 0, "cap_max": 0, "spawn_radius": 8.0,
                "enemy_hp": 100.0, "shot_damage": 0.0, "fire_interval": 0.05,
                "contact_dps": 5.0, "weapon_drop_chance": 0.0,
                "boss_first_s": 2.0, "boss_interval_s": 3.0,
                "boss_hp_mult": 3.0, "boss_hp_growth": 1.6, "boss_scale": 1.9
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        let boss_of = |play: &WorldPlay| {
            play.survivor_loop
                .enemies
                .iter()
                .find(|(_, e)| e.boss && e.mode == Mode::Alive)
                .map(|(id, e)| (id.clone(), e.hp))
        };
        // Nothing before the schedule; the boss at ~2 s.
        for _ in 0..60 {
            play.tick(dt);
        }
        assert!(boss_of(&play).is_none(), "no boss before its time");
        for _ in 0..120 {
            play.tick(dt);
            if boss_of(&play).is_some() {
                break;
            }
        }
        let (boss_id, boss_hp0) = boss_of(&play).expect("the boss walks in on schedule");
        assert_eq!(play.survivor_loop.bosses_spawned, 1);
        assert!(
            (boss_hp0 - 300.0).abs() < 1e-3,
            "boss hp = enemy hp x boss mult inside the grace, got {boss_hp0}"
        );
        let bw = enemy(&play, &boss_id);
        assert_eq!(
            bw.scale,
            Some([1.9, 1.9, 1.9]),
            "the boss body is scaled up"
        );
        assert!(
            play.doc.points.iter().any(|p| p.id == BOSS_ID),
            "the boss marker publishes"
        );
        let hud = play.build_hud(960, 720);
        assert!(
            hud.texts.iter().any(|t| t.text == "BOSS"),
            "the HUD calls the boss out"
        );
        // The bite: drop the boss on the player for one second.
        // (Inside its stop ring the seek pass holds it in place.)
        let pp = play.doc.player.as_ref().unwrap().position;
        for w in &mut play.doc.walkers {
            if w.id == boss_id {
                w.position = pp;
            }
        }
        let hp_before = play.survivor_loop.hp;
        for _ in 0..60 {
            play.tick(dt);
        }
        let bitten = hp_before - play.survivor_loop.hp;
        assert!(
            (bitten - 12.5).abs() < 1.5,
            "one second of boss contact = contact_dps x boss mult (12.5), got {bitten}"
        );
        // Kill it from a distance (no magnet or pickup can reach
        // the payout): a gun for certain at drop chance 0, plus a
        // burst of gems around the corpse.
        for w in &mut play.doc.walkers {
            if !w.survivor_enemy {
                w.position = [pp[0] + 10.0, 0.0, pp[2]];
            }
        }
        if let Some(p) = play.doc.player.as_mut() {
            p.position = [pp[0] + 10.0, 0.0, pp[2]];
        }
        play.survivor_loop.enemies.get_mut(&boss_id).unwrap().hp = 10.0;
        if let Some(s) = play.doc.survivor.as_mut() {
            s.shot_damage = 34.0;
        }
        for _ in 0..300 {
            play.tick(dt);
            if play.survivor_loop.kills >= 1 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.kills, 1, "the boss falls");
        let gems = play
            .survivor_loop
            .drops
            .iter()
            .filter(|d| d.kind == DropKind::Gem)
            .count();
        assert_eq!(
            gems,
            BOSS_GEMS as usize + 1,
            "the burst plus the standard gem"
        );
        assert!(
            play.survivor_loop
                .drops
                .iter()
                .any(|d| matches!(d.kind, DropKind::Weapon(_))),
            "a boss always drops a gun"
        );
        // The corpse recycles at regular size...
        if let Some(s) = play.doc.survivor.as_mut() {
            s.shot_damage = 0.0;
        }
        for _ in 0..90 {
            play.tick(dt);
        }
        assert_eq!(
            enemy(&play, &boss_id).scale,
            None,
            "the recycled body is regular-sized again"
        );
        // ...and one interval after the first entrance, a tougher
        // boss walks in: hp x the growth factor.
        for _ in 0..360 {
            play.tick(dt);
            if play.survivor_loop.bosses_spawned >= 2 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.bosses_spawned, 2);
        let (_, hp2) = boss_of(&play).expect("the second boss walks in");
        assert!(
            (hp2 - 480.0).abs() < 1e-3,
            "second boss hp = 300 x 1.6 growth, got {hp2}"
        );
    }

    #[test]
    fn turn_toward_takes_the_short_way() {
        assert!((turn_toward(0.0, std::f32::consts::PI, 0.5) - 0.5).abs() < 1e-6);
        // 3.0 -> -3.0 is +0.28 the short way (across the wrap), not -6.
        assert!((turn_toward(3.0, -3.0, 0.5) - 3.2832).abs() < 1e-3);
        assert!((turn_toward(0.0, -std::f32::consts::FRAC_PI_2, 0.1) + 0.1).abs() < 1e-6);
        assert!((turn_toward(0.0, 0.05, 0.5) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn facing_eases_toward_the_target_instead_of_snapping() {
        // One unkillable enemy inside range: the player must end up
        // facing it, but never by more than TURN_RATE per step.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 6.0,
                "shot_damage": 0.0, "fire_interval": 5.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        let mut prev = play.doc.player.as_ref().unwrap().yaw;
        for _ in 0..600 {
            play.tick(dt);
            let yaw = play.doc.player.as_ref().unwrap().yaw;
            let mut d = (yaw - prev).rem_euclid(std::f32::consts::TAU);
            if d > std::f32::consts::PI {
                d -= std::f32::consts::TAU;
            }
            assert!(
                d.abs() <= TURN_RATE * dt + 1e-3,
                "one step must not turn more than the rate allows, d={d}"
            );
            prev = yaw;
        }
        // The enemy has reached the stop ring and holds still there;
        // the player faces it.
        let z = enemy(&play, "walker:z0");
        let p = play.doc.player.as_ref().unwrap();
        let aim = (z.position[0] - p.position[0]).atan2(z.position[2] - p.position[2]);
        let mut d = (p.yaw - aim).rem_euclid(std::f32::consts::TAU);
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        assert!(d.abs() < 0.1, "facing must converge on the target, d={d}");
    }

    #[test]
    fn fire_fx_tracer_and_flash_appear_and_fade() {
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 8.0,
                "fire_interval": 5.0, "enemy_hp": 100.0, "shot_damage": 34.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        // The first step already has a target in range and fires.
        // (The doc also carries the loop's standing point clouds —
        // gun glow, player ring — so the FX are counted by id.)
        play.tick(dt);
        let tracer = play.doc.lines.iter().find(|l| l.id == TRACER_ID);
        assert!(tracer.is_some(), "a shot must draw a tracer");
        assert_eq!(tracer.unwrap().points.len(), 2, "tracer is muzzle->target");
        assert_eq!(
            play.doc.points.iter().filter(|p| p.id == FLASH_ID).count(),
            1,
            "a shot must flash the muzzle"
        );
        // No second shot for 5 s, so the FX must fade out by itself.
        for _ in 0..10 {
            play.tick(dt);
        }
        assert!(
            play.doc.lines.iter().all(|l| l.id != TRACER_ID),
            "tracer must fade out"
        );
        assert!(
            play.doc.points.iter().all(|p| p.id != FLASH_ID),
            "flash must fade out"
        );
    }

    #[test]
    fn weapon_numbers_differ_by_kind() {
        // The four guns must feel different on paper before they
        // feel different in the hand: rate and punch trade off.
        let spec = SurvivorSpec::default();
        let mut l = SurvivorLoop::default();
        let base_i = l.fire_interval(&spec);
        let base_d = l.shot_damage(&spec);
        l.weapon = Weapon::MachineGun;
        assert!(
            l.fire_interval(&spec) < base_i * 0.6,
            "MG cycles far faster"
        );
        assert!(l.shot_damage(&spec) < base_d, "MG hits softer per round");
        l.weapon = Weapon::Laser;
        assert!(
            l.fire_interval(&spec) > base_i * 2.0,
            "laser charges slowly"
        );
        assert!(l.shot_damage(&spec) > base_d * 2.0, "laser hits far harder");
        l.weapon = Weapon::Rocket;
        assert!(l.fire_interval(&spec) > base_i, "rocket reloads slower");
        assert!(l.shot_damage(&spec) > base_d, "rocket hits harder");
    }

    #[test]
    fn kills_can_drop_a_gun_and_walking_over_it_swaps() {
        let dump = survivor_dump(
            1,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 5.0,
                "spawn_interval": 0.05, "spawn_interval_min": 0.05,
                "enemy_hp": 34.0, "shot_damage": 34.0,
                "fire_interval": 0.05, "contact_dps": 0.0,
                "weapon_drop_chance": 1.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..45 {
            play.tick(dt);
        }
        assert!(
            play.survivor_loop.kills >= 1,
            "the zombie must fall (the pool may recycle and fall again)"
        );
        let gun_pos = play
            .survivor_loop
            .drops
            .iter()
            .find(|d| matches!(d.kind, DropKind::Weapon(_)))
            .map(|d| d.pos);
        let gun_pos = gun_pos.expect("a kill at 100% must drop a gun");
        let kind = play
            .survivor_loop
            .drops
            .iter()
            .find(|d| matches!(d.kind, DropKind::Weapon(_)))
            .map(|d| d.kind);
        assert_eq!(
            kind,
            Some(DropKind::Weapon(Weapon::MachineGun)),
            "the first rotation drop is the machine gun"
        );
        // Walk onto the drop: the MG is taken, the rifle falls.
        if let Some(p) = play.doc.player.as_mut() {
            p.position = gun_pos;
        }
        for w in &mut play.doc.walkers {
            if !w.survivor_enemy {
                w.position = gun_pos;
            }
        }
        for _ in 0..10 {
            play.tick(dt);
        }
        assert_eq!(play.survivor_loop.weapon, Weapon::MachineGun);
        assert!(
            play.survivor_loop
                .drops
                .iter()
                .any(|d| d.kind == DropKind::Weapon(Weapon::Rifle)),
            "the swapped-out rifle must lie on the ground"
        );
        let hud = play.build_hud(960, 720);
        assert!(
            hud.texts.iter().any(|t| t.text == "MACHINE GUN"),
            "the HUD must name the held gun"
        );
    }

    #[test]
    fn laser_pierces_everything_down_the_line() {
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 2, "cap_max": 2, "spawn_radius": 6.0,
                "spawn_interval": 0.05, "spawn_interval_min": 0.05,
                "enemy_hp": 100000.0, "shot_damage": 10.0,
                "fire_interval": 0.05, "contact_dps": 0.0,
                "weapon_drop_chance": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.survivor_loop.weapon = Weapon::Laser;
        let dt = 1.0 / 60.0;
        for _ in 0..150 {
            play.tick(dt);
        }
        // Line both zombies up on the aim axis, one behind the
        // other, then let one beam go.
        let ids: Vec<String> = play
            .doc
            .walkers
            .iter()
            .filter(|w| w.survivor_enemy)
            .map(|w| w.id.clone())
            .collect();
        for (n, w) in play
            .doc
            .walkers
            .iter_mut()
            .filter(|w| w.survivor_enemy)
            .enumerate()
        {
            w.position = [0.0, 0.0, -4.0 - 3.0 * n as f32];
        }
        for _ in 0..45 {
            play.tick(dt);
        }
        for id in &ids {
            let hp = play.survivor_loop.enemies.get(id).map(|e| e.hp);
            assert!(
                hp.is_some_and(|hp| hp < 100000.0),
                "the beam must reach {id} too, hp={hp:?}"
            );
        }
    }

    #[test]
    fn rocket_splashes_the_cluster_not_the_field() {
        let dump = survivor_dump(
            3,
            json!({
                "cap_start": 3, "cap_max": 3, "spawn_radius": 6.0,
                "spawn_interval": 0.05, "spawn_interval_min": 0.05,
                "enemy_hp": 100000.0, "shot_damage": 10.0,
                "fire_interval": 0.05, "contact_dps": 0.0,
                "weapon_drop_chance": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.survivor_loop.weapon = Weapon::Rocket;
        let dt = 1.0 / 60.0;
        for _ in 0..150 {
            play.tick(dt);
        }
        let spots = [[0.0, 0.0, -4.0], [0.8, 0.0, -4.6], [0.0, 0.0, 9.0]];
        let mut ids = Vec::new();
        for (n, w) in play
            .doc
            .walkers
            .iter_mut()
            .filter(|w| w.survivor_enemy)
            .enumerate()
        {
            w.position = spots[n];
            ids.push(w.id.clone());
        }
        // Baselines after the teleport: the warm-up fire may have
        // nicked anyone, so splash is judged on the delta from here.
        let hp = |play: &WorldPlay, i: usize| play.survivor_loop.enemies.get(&ids[i]).map(|e| e.hp);
        let base: Vec<Option<f32>> = (0..3).map(|i| hp(&play, i)).collect();
        for _ in 0..40 {
            play.tick(dt);
        }
        assert!(
            hp(&play, 0) < base[0],
            "the direct target takes the rocket, hp={:?} vs base {:?}",
            hp(&play, 0),
            base[0]
        );
        assert!(
            hp(&play, 1) < base[1],
            "the neighbour inside the splash takes it too, hp={:?} vs base {:?}",
            hp(&play, 1),
            base[1]
        );
        assert_eq!(
            hp(&play, 2),
            base[2],
            "the far zombie is outside the splash and untouched"
        );
    }

    #[test]
    fn survivor_camera_override_comes_from_the_spec() {
        let dump = survivor_dump(1, json!({ "cam_distance": 20.0, "cam_height": 11.0 }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..10 {
            play.tick(dt);
        }
        let cam = &play.doc.cameras[0];
        let pp = play.doc.player.as_ref().unwrap().position;
        let dx = cam.position[0] - pp[0];
        let dz = cam.position[2] - pp[2];
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            (dist - 20.0).abs() < 0.6,
            "camera must sit at the spec distance, got {dist}"
        );
        assert!(
            (cam.position[1] - 11.0).abs() < 0.6,
            "camera must sit at the spec height, got {}",
            cam.position[1]
        );
    }

    #[test]
    fn shipped_tuning_reaches_a_camping_player() {
        // The exact numbers the staging scene ships: a player who
        // never moves must feel the pressure and eventually fall.
        // (Guards the round-3 distance redesign for the raised
        // camera: spawn 16.5 m, fire reach 12 m — combat inside the
        // view, contact reachable.)
        let dump = survivor_dump(
            20,
            json!({
                "player_hp": 100.0, "fire_range": 12.0, "fire_interval": 0.28,
                "shot_damage": 34.0, "enemy_hp": 100.0, "contact_dps": 5.0,
                "cap_start": 13, "cap_max": 19, "cap_ramp_s": 10.0,
                "spawn_interval": 0.5, "spawn_interval_min": 0.22,
                "spawn_ramp_s": 120.0, "spawn_radius": 16.5
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        let mut over = false;
        for t in 0..(60 * 120) {
            play.tick(dt);
            // A real camper still taps a level-up card eventually;
            // this guard is about pressure, not the choice, so lift
            // the freeze without taking a reward.
            play.survivor_loop.level_pending = false;
            if t == 60 * 15 {
                assert!(
                    play.survivor_loop.alive_now >= 6,
                    "the screen must be crowded early, alive={}",
                    play.survivor_loop.alive_now
                );
            }
            if play.survivor_loop.over {
                over = true;
                break;
            }
        }
        assert!(over, "a camping player must eventually be overrun");
        assert_eq!(play.survivor_loop.hp, 0.0);
        assert!(play.survivor_loop.kills > 0, "the gun must still kill");
    }

    #[test]
    fn rifle_prop_follows_the_player() {
        let mut v: serde_json::Value = serde_json::from_str(&survivor_dump(2, json!({}))).unwrap();
        v["props"] = json!([{
            "id": RIFLE_PROP_ID, "type": "prop", "name": "rifle",
            "position": [0.0, 1.3, 5.0], "yaw": 0.0,
            "model": "blaster", "gltf": "assets/model/blaster-a.glb"
        }]);
        let mut play = WorldPlay::from_json(&v.to_string()).unwrap();
        play.start();
        for _ in 0..30 {
            play.tick(1.0 / 60.0);
        }
        let p = play.doc.player.as_ref().unwrap();
        let prop = play
            .doc
            .props
            .iter()
            .find(|q| q.id == RIFLE_PROP_ID)
            .unwrap();
        assert!(prop.enabled, "the rifle stays enabled");
        assert!(
            (prop.position[1] - (p.position[1] + RIFLE_H)).abs() < 0.01,
            "rifle sits at chest height, y={}",
            prop.position[1]
        );
        let dx = prop.position[0] - p.position[0];
        let dz = prop.position[2] - p.position[2];
        let horizontal = (dx * dx + dz * dz).sqrt();
        assert!(
            (horizontal - (RIFLE_FWD * RIFLE_FWD + RIFLE_RIGHT * RIFLE_RIGHT).sqrt()).abs() < 0.02,
            "rifle sits at its forward/right offset, d={horizontal}"
        );
        assert!(
            (prop.yaw - p.yaw).abs() < 1e-4,
            "rifle yaw follows the player's facing"
        );
    }

    #[test]
    fn no_spec_world_is_untouched() {
        // A world without the survivor block: the loop is inert and the
        // generic HUD stays in charge.
        let dump = {
            let mut v: serde_json::Value =
                serde_json::from_str(&survivor_dump(2, json!({}))).unwrap();
            v.as_object_mut().unwrap().remove("survivor");
            v.to_string()
        };
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        for _ in 0..120 {
            play.tick(1.0 / 60.0);
        }
        assert_eq!(play.survivor_loop.kills, 0);
        assert_eq!(play.survivor_loop.alive_now, 0);
        // Pool members keep their authored (parked) places; nothing
        // was activated.
        assert_eq!(enemy(&play, "walker:z0").seek, 0.0);
    }

    fn ptr(id: u32, x: f32, y: f32, phase: PointerPhase) -> PointerEvent {
        PointerEvent {
            id,
            x,
            y,
            phase,
            pressure: 1.0,
        }
    }

    #[test]
    fn stick_lands_in_the_zone_and_steers_the_player() {
        // No enemies: the stick alone is under test (390x844 phone).
        let dump = survivor_dump(2, json!({ "cap_start": 0, "cap_max": 0 }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..5 {
            play.tick(dt);
        }
        // A thumb outside the zone (top-right) is not a stick.
        play.survivor_touch(ptr(3, 350.0, 100.0, PointerPhase::Begin), 390, 844);
        assert!(play.survivor_loop.stick_vector().is_none());
        play.survivor_touch(ptr(3, 350.0, 100.0, PointerPhase::End), 390, 844);
        // Land in the lower-left zone, then push straight up.
        play.survivor_touch(ptr(7, 100.0, 600.0, PointerPhase::Begin), 390, 844);
        let v = play.survivor_loop.stick_vector().expect("stick engaged");
        assert!(v.0.abs() < 1e-5 && v.1.abs() < 1e-5, "centered: {v:?}");
        play.survivor_touch(ptr(7, 100.0, 470.0, PointerPhase::Move), 390, 844);
        let v = play.survivor_loop.stick_vector().expect("stick held");
        assert!(v.0.abs() < 0.05 && v.1 > 0.95, "full push up: {v:?}");
        // Held up, the player walks (away from the chase camera).
        let z0 = play.doc.player.as_ref().unwrap().position[2];
        for _ in 0..60 {
            play.tick(dt);
        }
        let z1 = play.doc.player.as_ref().unwrap().position[2];
        assert!(z1 < z0 - 0.5, "player must walk up-screen: {z0} -> {z1}");
        // Release hands control back: no vector, no more walking.
        play.survivor_touch(ptr(7, 100.0, 470.0, PointerPhase::End), 390, 844);
        assert!(play.survivor_loop.stick_vector().is_none());
        let z2 = play.doc.player.as_ref().unwrap().position[2];
        for _ in 0..30 {
            play.tick(dt);
        }
        let z3 = play.doc.player.as_ref().unwrap().position[2];
        assert!(
            (z3 - z2).abs() < 0.05,
            "released means stopped: {z2} -> {z3}"
        );
    }

    #[test]
    fn stick_draws_in_the_hud_while_playing() {
        let dump = survivor_dump(2, json!({ "cap_start": 0, "cap_max": 0 }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        play.tick(1.0 / 60.0);
        let rest = play.build_hud(960, 720).quads.len();
        play.survivor_touch(ptr(7, 200.0, 500.0, PointerPhase::Begin), 960, 720);
        play.survivor_touch(ptr(7, 260.0, 470.0, PointerPhase::Move), 960, 720);
        let live = play.build_hud(960, 720).quads.len();
        assert!(
            live > rest,
            "the live stick (base + jelly rows) draws more than the resting hint: {rest} -> {live}"
        );
    }

    #[test]
    fn fire_is_held_beyond_fire_range() {
        // Spawn ring at 12 m, fire reach 10.5 m: the zombie is
        // inside the lock's stick band (13.1 m) from the first step,
        // so the old code spent rounds on it immediately. Now the
        // gun must hold until the target truly closes in.
        let dump = survivor_dump(
            1,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 12.0,
                "fire_range": 10.5, "shot_damage": 0.0, "fire_interval": 0.1
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            play.tick(dt);
        }
        assert_eq!(
            play.survivor_loop.shots, 0,
            "no rounds beyond fire_range, even with a lock"
        );
        // It closes to ~8 m within two more seconds and fire opens.
        for _ in 0..150 {
            play.tick(dt);
        }
        assert!(
            play.survivor_loop.shots > 0,
            "fire must open once the target is in range"
        );
    }

    #[test]
    fn dying_enemies_are_never_shot() {
        // One-shot kill (34 dmg vs 34 hp). The body spends 1.1 s
        // dying inside fire range; not one more round may go into it.
        let dump = survivor_dump(
            1,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 8.0,
                "enemy_hp": 34.0, "fire_interval": 0.1
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..600 {
            play.tick(dt);
            if play.survivor_loop.kills >= 1 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.kills, 1);
        assert_eq!(play.survivor_loop.shots, 1, "one shot, one kill");
        for _ in 0..60 {
            play.tick(dt);
            assert!(play.survivor_loop.aim_id.is_none(), "no aim at a corpse");
        }
        assert_eq!(
            play.survivor_loop.shots, 1,
            "the dying body must not be shot again"
        );
    }

    #[test]
    fn a_closer_threat_steals_the_aim() {
        // Two unkillable zombies. The aim locks onto one at range;
        // teleporting the other to biting distance must steal the
        // aim (and therefore the damage) to the near threat.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 2, "cap_max": 2, "spawn_radius": 9.0,
                "spawn_interval": 0.05, "spawn_interval_min": 0.05,
                "enemy_hp": 100000.0, "shot_damage": 1.0,
                "fire_interval": 0.05, "contact_dps": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..40 {
            play.tick(dt);
        }
        let locked = play.survivor_loop.aim_id.clone().expect("aim locked");
        let other = if locked == "walker:z0" {
            "walker:z1"
        } else {
            "walker:z0"
        };
        // Stage it: the lock's target out at 10 m, the rival at 2 m.
        for w in &mut play.doc.walkers {
            if w.id == locked {
                w.position = [0.0, 0.0, -10.0];
            } else if w.id == other {
                w.position = [2.0, 0.0, 0.0];
            }
        }
        let hp_of = |play: &WorldPlay, id: &str| play.survivor_loop.enemies[id].hp;
        let far_hp0 = hp_of(&play, &locked);
        let near_hp0 = hp_of(&play, other);
        for _ in 0..20 {
            play.tick(dt);
        }
        assert_eq!(
            play.survivor_loop.aim_id.as_deref(),
            Some(other),
            "the biting zombie must own the aim"
        );
        assert!(
            hp_of(&play, other) < near_hp0,
            "the near threat takes the damage now"
        );
        assert_eq!(
            hp_of(&play, &locked),
            far_hp0,
            "the far body stops being shot while the threat is near"
        );
    }

    #[test]
    fn kills_drop_gems_and_walking_over_collects_them() {
        // One-shot kill inside range: a gem is left where it fell.
        let dump = survivor_dump(
            1,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 6.0,
                "enemy_hp": 34.0, "fire_interval": 0.1
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..600 {
            play.tick(dt);
            if play.survivor_loop.kills >= 1 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.kills, 1);
        let gem = play
            .survivor_loop
            .drops
            .iter()
            .find(|d| d.kind == DropKind::Gem)
            .expect("a kill drops a gem")
            .pos;
        assert!(
            play.doc
                .points
                .iter()
                .any(|p| p.id == GEM_ID && !p.positions.is_empty()),
            "gems publish to the doc point cloud"
        );
        // Too far to collect from the origin spawn point? The gem
        // sits where the zombie died; walk the player onto it.
        assert_eq!(play.survivor_loop.gems, 0);
        with_player(&mut play.doc, |p| {
            p.position = [gem[0], 0.0, gem[2]];
        });
        play.tick(dt);
        assert_eq!(play.survivor_loop.gems, 1, "walking over banks the gem");
        assert!(
            play.survivor_loop
                .drops
                .iter()
                .all(|d| d.kind != DropKind::Gem),
            "the collected gem is gone"
        );
    }

    #[test]
    fn heal_packs_restore_hp_and_magnet_pulls_drops_in() {
        let dump = survivor_dump(1, json!({ "cap_start": 0, "cap_max": 0 }));
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        play.tick(dt);
        // A heal pack at the player's feet: +20 HP, capped at max.
        play.survivor_loop.hp = 50.0;
        play.survivor_loop.drops.push(Drop {
            kind: DropKind::Heal,
            pos: [0.0, 0.35, 0.0],
        });
        play.tick(dt);
        assert_eq!(play.survivor_loop.hp, 70.0);
        // A gem 2.5 m out is inside the magnet pull: it drifts in
        // and is collected without the player moving.
        play.survivor_loop.drops.push(Drop {
            kind: DropKind::Gem,
            pos: [2.5, 0.35, 0.0],
        });
        for _ in 0..90 {
            play.tick(dt);
            if play.survivor_loop.gems >= 1 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.gems, 1, "magnet feeds the gem home");
        assert!(play.survivor_loop.drops.is_empty());
    }

    #[test]
    fn level_up_freezes_the_world_and_the_pick_applies() {
        // One live zombie for the freeze check, and one banked gem
        // short of the first level: collecting it must raise the
        // cards.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 9.0,
                "shot_damage": 0.0, "contact_dps": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            play.tick(dt);
        }
        play.survivor_loop.gems = xp_need(1) - 1;
        play.survivor_loop.drops.push(Drop {
            kind: DropKind::Gem,
            pos: [0.0, 0.35, 0.0],
        });
        play.tick(dt);
        assert!(play.survivor_loop.level_pending, "the cards come up");
        assert_eq!(play.survivor_loop.level, 2);
        assert_eq!(play.survivor_loop.gems, 0, "the need was spent");
        // Frozen: neither the zombie nor the clock advances.
        let zpos = enemy(&play, "walker:z0").position;
        let t0 = play.survivor_loop.elapsed;
        for _ in 0..30 {
            play.tick(dt);
        }
        assert_eq!(enemy(&play, "walker:z0").position, zpos, "world frozen");
        assert_eq!(play.survivor_loop.elapsed, t0, "clock frozen");
        // Tap the Heavy Rounds card: damage grows by the flat bonus.
        // (The offers themselves are pinned here so the tap target
        // does not depend on the run's rng rotation.)
        play.survivor_loop.choices = [Choice::RapidFire, Choice::DamageUp, Choice::Vitality];
        let idx = play
            .survivor_loop
            .choices
            .iter()
            .position(|c| *c == Choice::DamageUp)
            .expect("damage is among the offers");
        let spec = play.doc.survivor.clone().unwrap();
        let before = play.survivor_loop.shot_damage(&spec);
        let r = level_card_rects(960.0, 720.0)[idx];
        play.survivor_touch(
            ptr(9, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, PointerPhase::Begin),
            960,
            720,
        );
        assert!(
            !play.survivor_loop.level_pending,
            "the pick closes the cards"
        );
        assert_eq!(play.survivor_loop.shot_damage(&spec), before + 8.0);
        // And the world breathes again.
        for _ in 0..30 {
            play.tick(dt);
        }
        assert!(play.survivor_loop.elapsed > t0, "the run resumes");
    }

    #[test]
    fn release_during_level_up_cards_does_not_leave_the_stick_held() {
        // Phone report: after picking up items the hero walks on by
        // himself. The path: the thumb is on the stick when the last
        // gem banks the level, the cards freeze the world, and the
        // player lifts the thumb to tap a card. That release (or a
        // pointer cancel in the same window) must still count, or
        // the stale stick vector walks the hero forever once the
        // pick closes the cards.
        for phase in [PointerPhase::End, PointerPhase::Cancel] {
            let dump = survivor_dump(
                2,
                json!({
                    "cap_start": 1, "cap_max": 1, "spawn_radius": 9.0,
                    "shot_damage": 0.0, "contact_dps": 0.0
                }),
            );
            let mut play = WorldPlay::from_json(&dump).unwrap();
            play.start();
            let dt = 1.0 / 60.0;
            for _ in 0..30 {
                play.tick(dt);
            }
            // Thumb down in the stick zone, pushed up-right.
            play.survivor_touch(ptr(1, 200.0, 550.0, PointerPhase::Begin), 960, 720);
            play.survivor_touch(ptr(1, 260.0, 480.0, PointerPhase::Move), 960, 720);
            assert!(
                play.survivor_loop.stick_vector().is_some(),
                "the stick steers ({phase:?})"
            );
            // The last gem banks: the cards come up mid-hold.
            play.survivor_loop.gems = xp_need(1) - 1;
            play.survivor_loop.drops.push(Drop {
                kind: DropKind::Gem,
                pos: [0.0, 0.35, 0.0],
            });
            play.tick(dt);
            assert!(play.survivor_loop.level_pending, "the cards come up");
            // The thumb lifts while the world is frozen...
            play.survivor_touch(ptr(1, 260.0, 480.0, phase), 960, 720);
            // ...and a fresh tap picks the first card.
            let r = level_card_rects(960.0, 720.0)[0];
            play.survivor_touch(
                ptr(2, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, PointerPhase::Begin),
                960,
                720,
            );
            assert!(
                !play.survivor_loop.level_pending,
                "the pick closes the cards ({phase:?})"
            );
            assert!(
                play.survivor_loop.stick_vector().is_none(),
                "no thumb is down: the hero must not walk on by himself ({phase:?})"
            );
            assert!(
                play.survivor_loop.stick_finger.is_none(),
                "the stick hand is empty after the pick ({phase:?})"
            );
        }
    }

    #[test]
    fn a_thumb_held_through_the_cards_does_not_walk_after_the_pick() {
        // The other half of the phone report: the thumb never lifts
        // (or its release is lost), another finger taps a card. The
        // pick must drop the stick, and the next thumb must land.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 9.0,
                "shot_damage": 0.0, "contact_dps": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            play.tick(dt);
        }
        play.survivor_touch(ptr(1, 200.0, 550.0, PointerPhase::Begin), 960, 720);
        play.survivor_touch(ptr(1, 260.0, 480.0, PointerPhase::Move), 960, 720);
        assert!(
            play.survivor_loop.stick_vector().is_some(),
            "the stick steers before the freeze"
        );
        play.survivor_loop.gems = xp_need(1) - 1;
        play.survivor_loop.drops.push(Drop {
            kind: DropKind::Gem,
            pos: [0.0, 0.35, 0.0],
        });
        play.tick(dt);
        assert!(play.survivor_loop.level_pending, "the cards come up");
        // No release for id 1. Another finger picks a card.
        let r = level_card_rects(960.0, 720.0)[0];
        play.survivor_touch(
            ptr(2, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0, PointerPhase::Begin),
            960,
            720,
        );
        assert!(
            !play.survivor_loop.level_pending,
            "the pick closes the cards"
        );
        assert!(
            play.survivor_loop.stick_vector().is_none(),
            "no walk after the pick"
        );
        let before = play.doc.player.as_ref().unwrap().position;
        for _ in 0..20 {
            play.tick(dt);
        }
        assert_eq!(
            play.doc.player.as_ref().unwrap().position,
            before,
            "the hero stays put"
        );
        // The still-held thumb's moves no longer steer...
        play.survivor_touch(ptr(1, 300.0, 450.0, PointerPhase::Move), 960, 720);
        assert!(play.survivor_loop.stick_vector().is_none());
        // ...and a fresh thumb lands even though id 1 never ended.
        play.survivor_touch(ptr(3, 200.0, 550.0, PointerPhase::Begin), 960, 720);
        play.survivor_touch(ptr(3, 260.0, 480.0, PointerPhase::Move), 960, 720);
        assert!(
            play.survivor_loop.stick_vector().is_some(),
            "a new thumb takes the stick"
        );
    }

    #[test]
    fn walk_over_pickups_do_not_touch_the_stick() {
        // Weapon and heal drops are collected by walking over them —
        // no tap, no freeze. Neither pickup may conjure or clear a
        // stick vector: with no thumb down the hero stays put, and
        // that must still be true right after the swap / heal lands.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 9.0,
                "shot_damage": 0.0, "contact_dps": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..30 {
            play.tick(dt);
        }
        assert!(play.survivor_loop.stick_vector().is_none());
        let gun = play.survivor_loop.weapon;
        play.survivor_loop.drops.push(Drop {
            kind: DropKind::Weapon(Weapon::Laser),
            pos: [0.0, 0.35, 0.0],
        });
        play.survivor_loop.hp = 40.0;
        play.survivor_loop.drops.push(Drop {
            kind: DropKind::Heal,
            pos: [0.0, 0.35, 0.0],
        });
        for _ in 0..10 {
            play.tick(dt);
        }
        assert_ne!(play.survivor_loop.weapon, gun, "the swap landed");
        assert!(play.survivor_loop.hp > 40.0, "the heal landed");
        assert!(
            play.survivor_loop.stick_vector().is_none(),
            "pickups leave an untouched stick untouched"
        );
    }

    #[test]
    fn every_choice_moves_a_real_number() {
        let spec: SurvivorSpec = serde_json::from_value(json!({
            "fire_interval": 0.3, "shot_damage": 30.0, "player_hp": 100.0
        }))
        .unwrap();
        let mut state = SurvivorLoop {
            hp: 40.0,
            hp_max: 100.0,
            level_pending: true,
            ..Default::default()
        };
        state.apply_choice(Choice::RapidFire);
        assert!((state.fire_interval(&spec) - 0.3 * 0.88).abs() < 1e-6);
        state.apply_choice(Choice::DamageUp);
        assert_eq!(state.shot_damage(&spec), 38.0);
        state.apply_choice(Choice::Vitality);
        assert_eq!(state.hp_max, 125.0);
        assert_eq!(state.hp, 65.0, "vitality heals what it adds");
        let armor = state.armor_mult;
        state.apply_choice(Choice::Armor);
        assert!((state.armor_mult - armor * 0.85).abs() < 1e-6);
        let magnet = state.magnet_r;
        state.apply_choice(Choice::Magnet);
        assert!(state.magnet_r > magnet);
        assert!(!state.level_pending, "each pick closes the cards");
    }

    /// Project a world point through the play's live camera, the
    /// same math `tap_select` picks with.
    fn project(play: &WorldPlay, p: [f32; 3], w: f32, h: f32) -> (f32, f32) {
        let cam = &play.doc.cameras[0];
        let view = crate::game_core::view::View {
            eye: cam.position,
            target: cam.target,
            fov: cam.fov,
        };
        view.project(p, w, h).expect("the point is on screen")
    }

    fn two_foe_stage() -> WorldPlay {
        // Two unkillable zombies: one walks in close-right, one
        // holds far-left (teleported after the spawn, so the stage
        // geometry is exact). The gun ticks at both; nothing dies.
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 2, "cap_max": 2, "spawn_radius": 9.0,
                "spawn_interval": 0.05, "spawn_interval_min": 0.05,
                "enemy_hp": 100000.0, "shot_damage": 10.0,
                "fire_interval": 0.05, "contact_dps": 0.0,
                "weapon_drop_chance": 0.0,
                "cam_distance": 14.5, "cam_height": 10.8
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            play.tick(dt);
        }
        assert_eq!(play.survivor_loop.alive_now, 2, "both zombies walk in");
        for w in &mut play.doc.walkers {
            match w.id.as_str() {
                "walker:z0" => w.position = [2.5, 0.0, -3.0],
                "walker:z1" => w.position = [-4.0, 0.0, -7.0],
                _ => {}
            }
        }
        play
    }

    #[test]
    fn a_tap_locks_the_tapped_foe_over_the_auto_aim() {
        let mut play = two_foe_stage();
        let dt = 1.0 / 60.0;
        // Auto-aim alone takes the near zombie.
        for _ in 0..10 {
            play.tick(dt);
        }
        assert_eq!(
            play.survivor_loop.aim_id.as_deref(),
            Some("walker:z0"),
            "auto-aim starts on the nearest foe"
        );
        // Tap the far zombie's chest: the lock takes it, and the
        // gun follows the lock, not the nearer body.
        let z1 = enemy(&play, "walker:z1").position;
        let (sx, sy) = project(&play, [z1[0], 1.1, z1[2]], 960.0, 720.0);
        play.survivor_touch(ptr(9, sx, sy, PointerPhase::Begin), 960, 720);
        assert_eq!(play.survivor_loop.tap_locked(), Some("walker:z1"));
        for _ in 0..10 {
            play.tick(dt);
        }
        assert_eq!(
            play.survivor_loop.aim_id.as_deref(),
            Some("walker:z1"),
            "the tap lock outranks the nearer auto-aim target"
        );
        assert!(
            play.doc.points.iter().any(|p| p.id == LOCK_ID),
            "the locked foe wears the amber marker"
        );
        // A tap on bare sky clears the lock (no foe up there).
        play.survivor_touch(ptr(9, 480.0, 30.0, PointerPhase::Begin), 960, 720);
        assert_eq!(play.survivor_loop.tap_locked(), None);
        // Tap the foe again to lock, again to release (the toggle).
        let z1 = enemy(&play, "walker:z1").position;
        let (sx, sy) = project(&play, [z1[0], 1.1, z1[2]], 960.0, 720.0);
        play.survivor_touch(ptr(9, sx, sy, PointerPhase::Begin), 960, 720);
        assert_eq!(play.survivor_loop.tap_locked(), Some("walker:z1"));
        play.survivor_touch(ptr(9, sx, sy, PointerPhase::Begin), 960, 720);
        assert_eq!(play.survivor_loop.tap_locked(), None);
        // Lock once more, then let the gun finish it: death
        // releases the lock and takes the marker down.
        play.survivor_touch(ptr(9, sx, sy, PointerPhase::Begin), 960, 720);
        assert_eq!(play.survivor_loop.tap_locked(), Some("walker:z1"));
        play.survivor_loop.enemies.get_mut("walker:z1").unwrap().hp = 5.0;
        for _ in 0..300 {
            play.tick(dt);
            if play.survivor_loop.kills >= 1 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.kills, 1, "the locked foe falls");
        // The release lands on the next step's selection pass.
        for _ in 0..5 {
            play.tick(dt);
        }
        assert_eq!(play.survivor_loop.tap_locked(), None);
        assert!(
            play.doc.points.iter().all(|p| p.id != LOCK_ID),
            "the marker leaves with the lock"
        );
    }

    #[test]
    fn a_second_finger_tap_picks_while_the_stick_is_held() {
        let mut play = two_foe_stage();
        let dt = 1.0 / 60.0;
        for _ in 0..10 {
            play.tick(dt);
        }
        // The steering thumb lands in the zone and holds.
        play.survivor_touch(ptr(1, 200.0, 550.0, PointerPhase::Begin), 960, 720);
        assert!(play.survivor_loop.stick_vector().is_some(), "stick held");
        // The other thumb taps the far zombie: a pick, not a stick,
        // and the steering thumb keeps its stick.
        let z1 = enemy(&play, "walker:z1").position;
        let (sx, sy) = project(&play, [z1[0], 1.1, z1[2]], 960.0, 720.0);
        play.survivor_touch(ptr(2, sx, sy, PointerPhase::Begin), 960, 720);
        assert_eq!(play.survivor_loop.tap_locked(), Some("walker:z1"));
        assert!(
            play.survivor_loop.stick_vector().is_some(),
            "the tap must not steal the steering thumb"
        );
        // Lifting the steering thumb frees the stick; the lock stays.
        play.survivor_touch(ptr(1, 200.0, 550.0, PointerPhase::End), 960, 720);
        assert!(play.survivor_loop.stick_vector().is_none());
        assert_eq!(play.survivor_loop.tap_locked(), Some("walker:z1"));
    }

    #[test]
    fn burst_refill_fills_a_cleared_stage_faster() {
        // The pacing numbers: burst at 35% of the base interval
        // while the stage is under 40% of the cap, base otherwise.
        let spec: SurvivorSpec = serde_json::from_value(json!({
            "spawn_interval": 2.0, "spawn_interval_min": 2.0, "spawn_ramp_s": 0.0
        }))
        .unwrap();
        assert!((spawn_pacing(&spec, 0.0, 0, 6) - 0.7).abs() < 1e-6);
        assert!((spawn_pacing(&spec, 0.0, 2, 6) - 0.7).abs() < 1e-6);
        assert!((spawn_pacing(&spec, 0.0, 3, 6) - 2.0).abs() < 1e-6);
        assert!((spawn_pacing(&spec, 0.0, 0, 0) - 2.0).abs() < 1e-6);
        // The stage itself: fill to cap, park everyone by hand
        // (a wipe), and count what 1.5 s of refills bring back —
        // the burst lands three, the old single pace would land one.
        let dump = survivor_dump(
            8,
            json!({
                "cap_start": 6, "cap_max": 6, "spawn_radius": 9.0,
                "spawn_interval": 2.0, "spawn_interval_min": 2.0,
                "spawn_ramp_s": 0.0, "enemy_hp": 100000.0,
                "shot_damage": 0.0, "contact_dps": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..1200 {
            play.tick(dt);
            if play.survivor_loop.alive_now >= 6 {
                break;
            }
        }
        assert_eq!(play.survivor_loop.alive_now, 6, "the stage fills");
        for e in play.survivor_loop.enemies.values_mut() {
            e.mode = Mode::Parked;
        }
        // Zero the spawn clock so the refill phase is exact: the
        // burst then lands spawns at 0 s, 0.7 s and 1.4 s.
        play.survivor_loop.spawn_t = 0.0;
        for _ in 0..90 {
            play.tick(dt);
        }
        assert!(
            play.survivor_loop.alive_now >= 3,
            "burst refill must return a wave, alive={}",
            play.survivor_loop.alive_now
        );
    }

    #[test]
    fn the_run_starts_its_music_and_cues_its_shots() {
        crate::mixer::reset_mixer();
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 8.0,
                "enemy_hp": 100000.0, "shot_damage": 10.0,
                "fire_interval": 0.05, "contact_dps": 0.0,
                "weapon_drop_chance": 0.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        for _ in 0..180 {
            play.tick(dt);
        }
        assert!(play.survivor_loop.shots > 0, "the gun must have fired");
        let cues: serde_json::Value = serde_json::from_str(&crate::mixer::cues_json()).unwrap();
        assert_eq!(cues["bgm_name"], "bgm", "the run starts its music");
        assert!(
            cues["bgm_gain"].as_f64().unwrap() > 0.5,
            "the music fades in, gain={}",
            cues["bgm_gain"]
        );
        let shots = cues["oneshots"].as_array().unwrap();
        assert!(
            shots.iter().any(|s| s == "shoot"),
            "shots must chirp: {shots:?}"
        );
        assert!(
            shots.iter().any(|s| s == "hit"),
            "hits must tick: {shots:?}"
        );
    }

    #[test]
    fn contact_cues_hurt_and_death_stings_and_stops_the_music() {
        crate::mixer::reset_mixer();
        let dump = survivor_dump(
            2,
            json!({
                "cap_start": 1, "cap_max": 1, "spawn_radius": 6.0,
                "player_hp": 5.0, "shot_damage": 0.0, "contact_dps": 10.0
            }),
        );
        let mut play = WorldPlay::from_json(&dump).unwrap();
        play.start();
        let dt = 1.0 / 60.0;
        // The page drains the cue queue every frame, and the mixer
        // holds at most 8 at a time — so the test drains as it goes
        // and keeps the whole run's shots.
        let mut heard: Vec<String> = Vec::new();
        let mut last_gain = 1.0f64;
        for i in 0..3600 {
            play.tick(dt);
            if i % 30 == 29 {
                let cues: serde_json::Value =
                    serde_json::from_str(&crate::mixer::cues_json()).unwrap();
                heard.extend(
                    cues["oneshots"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().to_string()),
                );
                last_gain = cues["bgm_gain"].as_f64().unwrap();
            }
            if play.survivor_loop.over {
                break;
            }
        }
        assert!(play.survivor_loop.over, "the mauling ends the run");
        // Let the death fade run out on the mixer.
        for i in 0..90 {
            play.tick(dt);
            if i % 30 == 29 {
                let cues: serde_json::Value =
                    serde_json::from_str(&crate::mixer::cues_json()).unwrap();
                heard.extend(
                    cues["oneshots"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().to_string()),
                );
                last_gain = cues["bgm_gain"].as_f64().unwrap();
            }
        }
        assert!(
            heard.iter().any(|s| s == "hurt"),
            "contact must growl: {heard:?}"
        );
        assert!(
            heard.iter().any(|s| s == "gameover"),
            "death must sting: {heard:?}"
        );
        assert!(
            last_gain < 0.01,
            "the music stops with the run, gain={last_gain}"
        );
    }

    #[test]
    fn the_shipped_scene_carries_its_audio_block() {
        // The fixture is the scene the pack ships: its `game.audio`
        // block is what the page and the native plan both read.
        // Every cue the loop can emit must map to a file that
        // exists in the repo's shared pool.
        let world: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/vrm_verify_survivor_world.json"
        ))
        .unwrap();
        let spec = crate::audio_plan::spec_of(&world).expect("the scene names its sounds");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        for f in crate::audio_plan::sfx_files(Some(&spec)) {
            assert!(root.join(&f).is_file(), "sfx file must exist: {f}");
        }
        let cues = crate::audio_plan::Cues {
            muted: false,
            master: 0.8,
            bgm: 0.7,
            sfx: 1.0,
            bgm_name: "bgm".into(),
            bgm_gain: 1.0,
            oneshots: [
                "shoot", "hit", "kill", "hurt", "gem", "pickup", "pick", "levelup", "boom",
                "elite", "gameover",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        };
        let plan = crate::audio_plan::plan(Some(&spec), &cues);
        assert_eq!(
            plan.shots.len(),
            11,
            "every cue the loop emits must resolve to a file"
        );
        assert_eq!(
            plan.bgm.expect("the loop has a file").file,
            "assets/audio/oga-flowerbed-fields/flowerbed_fields.mp3"
        );
    }

    #[test]
    fn hud_text_rides_the_arena_kit_scale() {
        // The HUD alignment with Moonlit: at a 720-wide canvas the
        // HUD scale is 1.8 (the kit's own), where the old world
        // scale gave 1.0 — clock and cards grow by that ratio.
        let state = SurvivorLoop {
            hp: 80.0,
            hp_max: 100.0,
            ..Default::default()
        };
        let hud = build_hud(&state, GamePhase::Playing, 720, 1280);
        let time = hud
            .texts
            .iter()
            .find(|t| t.text.starts_with("TIME"))
            .expect("the clock is drawn");
        assert!(
            (time.size - 19.0 * 1.8).abs() < 0.01,
            "clock size rides the kit scale, got {}",
            time.size
        );
        let mut pending = state.clone();
        pending.level_pending = true;
        let hud = build_hud(&pending, GamePhase::Playing, 720, 1280);
        let name = hud
            .texts
            .iter()
            .find(|t| t.text == "RAPID FIRE")
            .expect("the card names are drawn");
        assert!((name.size - 21.0 * 1.8).abs() < 0.01);
        let rects = level_card_rects(720.0, 1280.0);
        assert!(
            (rects[0][3] - 80.0 * 1.8).abs() < 0.01,
            "cards are kit-height, got {}",
            rects[0][3]
        );
    }
}

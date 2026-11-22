//! Combat shared by top-down action kits: bodies, attack volumes, projectiles.
//!
//! The split that matters: a body's **hurt volume** (a foot-centred cylinder the
//! body *is hit through*) is separate from a weapon's **attack volume** (a sector
//! or circle that exists only at the moment of the blow). A long sword or a wide
//! zombie arm never widens the hurt volume, so a hit always has a readable cause.
//!
//! This module owns the facts only — damage, poise, hitstun, knockback velocity,
//! brief invulnerability, projectiles and the [`HitEvent`]s they produce. The kit
//! turns events into sound, juice, XP and drops. Stage geometry stays with the
//! kit: callers pass a `blocked` test for walls and push bodies out of solids
//! themselves after [`Body::integrate`].
//!
//! Design source: `docs/../action-game-v2-design.md` §2 (in the workspace root).

/// Which side a body fights on. Attacks only land on the other faction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Faction {
    Hero,
    Enemy,
}

/// What a body is doing. Timed states count down in [`Body::integrate`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyState {
    /// Free to act and to be hit.
    Active,
    /// A short flinch. Knockback still applies.
    Hitstun,
    /// Blown off its feet (`hit_fly`). No seeking until it ends.
    Launched,
    /// Out of the fight. Never a target again.
    Dead,
}

/// How one body takes each element: a damage multiplier and a status
/// duration multiplier per [`Element`] (indexed by `Element::index`).
/// Combat mechanics never branch on the element — they read this table,
/// so a "thunder-proof, fire-weak" body is data, not a code path. The
/// default is all 1.0 (every element lands exactly as authored).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affinity {
    /// Damage multiplier for Strike / Burst / Chain / Tick alike.
    pub damage: [f32; 4],
    /// Status duration multiplier, applied after `status_resist`.
    /// 0 is immunity: the ailment never lands at all.
    pub status: [f32; 4],
}

impl Default for Affinity {
    fn default() -> Self {
        Self {
            damage: [1.0; 4],
            status: [1.0; 4],
        }
    }
}

impl Affinity {
    /// The neutral table: every element lands as authored.
    pub const IDENTITY: Affinity = Affinity {
        damage: [1.0; 4],
        status: [1.0; 4],
    };
}

/// One combatant. `id` is a stable uid: array order may change, ids never do,
/// so a swing can remember whom it already hit.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub id: u64,
    pub faction: Faction,
    pub x: f32,
    pub z: f32,
    /// Facing yaw, the walkers' convention (`atan2(dx, dz)`).
    pub facing: f32,
    /// Hurt cylinder: radius on the ground, height above the feet.
    pub hurt_radius: f32,
    pub hurt_height: f32,
    pub hp: f32,
    pub max_hp: f32,
    /// Knockback divisor. A mob is 1.0, a brute ~3.0, a boss ~6.0.
    pub mass: f32,
    /// Knockback velocity (m/s), integrated with decay.
    pub vx: f32,
    pub vz: f32,
    pub state: BodyState,
    /// Seconds left in `Hitstun` / `Launched`.
    pub state_t: f32,
    /// No hit lands before this time (post-hit or dodge invulnerability).
    pub invuln_until: f32,
    /// White-flash display until this time. Display only; the sim ignores it.
    pub flash_until: f32,
    /// Flinch resistance. While poise remains, small hits do not stun.
    pub poise: f32,
    pub poise_max: f32,
    /// Status ailments riding on this body (a burn ticking, a slow).
    pub status: Status,
    /// How much of an ailment's duration this body shrugs off (0..1):
    /// mobs 0, brutes ~0.3, bosses ~0.6 (set by the kit / stage).
    pub status_resist: f32,
    /// Per-element damage / status multipliers (the kit's kind table).
    pub affinity: Affinity,
    /// Post-hit invulnerability the body grants itself (s): the hero
    /// 0.7, enemies 0 — a crowd gets no shared i-frame window, so a
    /// combo always lands every cut it connects.
    pub invuln_on_hit: f32,
    /// Until this time the body takes damage and poise loss but never
    /// flinches (a boss mid-swing). A poise break earned under armour
    /// is deferred: it lands when the armour ends (see
    /// [`Body::settle_armor`]).
    pub armor_until: f32,
    /// How long a poise break (or a launching blow) puts this body
    /// down. Mobs use [`LAUNCHED_SECS`]; heavy kinds and bosses longer.
    pub down_secs: f32,
}

impl Body {
    pub fn new(id: u64, faction: Faction, x: f32, z: f32, hp: f32, hurt_radius: f32) -> Self {
        Self {
            id,
            faction,
            x,
            z,
            facing: 0.0,
            hurt_radius,
            hurt_height: 1.8,
            hp,
            max_hp: hp,
            mass: 1.0,
            vx: 0.0,
            vz: 0.0,
            state: BodyState::Active,
            state_t: 0.0,
            invuln_until: 0.0,
            flash_until: 0.0,
            poise: 0.0,
            poise_max: 0.0,
            status: Status::default(),
            status_resist: 0.0,
            affinity: Affinity::IDENTITY,
            invuln_on_hit: 0.0,
            armor_until: 0.0,
            down_secs: LAUNCHED_SECS,
        }
    }

    pub fn alive(&self) -> bool {
        self.state != BodyState::Dead
    }

    /// Can this body be hit at `now`? The dead and the invulnerable cannot.
    pub fn hittable(&self, now: f32) -> bool {
        self.alive() && now >= self.invuln_until
    }

    /// Advance knockback and timed states by one fixed step. The caller runs
    /// static collision (buildings, arena bounds) on the result afterwards.
    pub fn integrate(&mut self, dt: f32) {
        if self.vx != 0.0 || self.vz != 0.0 {
            self.x += self.vx * dt;
            self.z += self.vz * dt;
            // Exponential-feeling decay: fast at first, gone in about
            // half a second (tuned down from 8/s on 2026-11-14 so slides
            // read on a phone screen; design §2.8).
            let keep = (1.0 - KNOCKBACK_DECAY * dt).max(0.0);
            self.vx *= keep;
            self.vz *= keep;
            if self.vx.hypot(self.vz) < KNOCKBACK_STOP {
                self.vx = 0.0;
                self.vz = 0.0;
            }
        }
        match self.state {
            BodyState::Hitstun | BodyState::Launched => {
                self.state_t -= dt;
                if self.state_t <= 0.0 {
                    self.state = BodyState::Active;
                    self.state_t = 0.0;
                }
            }
            _ => {}
        }
    }

    /// Back into the fight at full health (a recycled crowd body).
    pub fn revive(&mut self, x: f32, z: f32) {
        self.x = x;
        self.z = z;
        self.hp = self.max_hp;
        self.vx = 0.0;
        self.vz = 0.0;
        self.state = BodyState::Active;
        self.state_t = 0.0;
        self.invuln_until = 0.0;
        self.armor_until = 0.0;
        self.flash_until = 0.0;
        self.poise = self.poise_max;
        // A recycled body never comes back still burning or slowed.
        self.status = Status::default();
    }

    /// Land a poise break that was earned while armoured, once the
    /// armour has ended: poise sitting at or below zero on a standing
    /// body means the break was deferred by [`Body::armor_until`].
    /// The kit calls this once per step for bodies that can wear
    /// armour; it is a no-op for everyone else.
    pub fn settle_armor(&mut self, now: f32) {
        if self.alive()
            && self.state == BodyState::Active
            && now >= self.armor_until
            && self.poise_max > 0.0
            && self.poise <= 0.0
        {
            self.poise = self.poise_max;
            self.state = BodyState::Launched;
            self.state_t = self.down_secs;
        }
    }

    /// The speed multiplier a slow imposes at `now` (1.0 when not slowed).
    /// Movement, walk-cycle phase and action timers all scale by this one
    /// number, so a slowed body's animation and its hit timing stay in
    /// step. Knockback slide is *not* scaled: a shove is a shove.
    pub fn speed_scale(&self, now: f32) -> f32 {
        if now < self.status.slow_until {
            self.status.slow.max(SLOW_FLOOR)
        } else {
            1.0
        }
    }

    /// Advance the burn by one tick when one is due. Burn damage ignores
    /// invulnerability (a body on fire burns, i-frames or not) and never
    /// stuns, launches or knocks back — it only flashes. Returns the tick
    /// as a [`HitKind::Tick`] event; a tick can kill.
    pub fn tick_status(&mut self, now: f32) -> Option<HitEvent> {
        if !self.alive() {
            return None;
        }
        let st = self.status;
        if st.burn_dps <= 0.0 || now < st.burn_next || st.burn_next > st.burn_until {
            return None;
        }
        let damage = st.burn_dps * BURN_TICK_SECS * self.affinity.damage[Element::Fire.index()];
        self.hp -= damage;
        self.flash_until = now + FLASH_SECS;
        self.status.burn_next += BURN_TICK_SECS;
        let killed = self.hp <= 0.0;
        if killed {
            self.hp = 0.0;
            self.state = BodyState::Dead;
            self.state_t = 0.0;
        }
        Some(HitEvent {
            target: self.id,
            damage,
            killed,
            fly: false,
            dir: (0.0, 0.0),
            hitstop: 0.0,
            element: Element::Fire,
            kind: HitKind::Tick,
        })
    }
}

/// An element, as a label: display colour, sound and resistance tables
/// read it. Combat *mechanics* never branch on it — they branch on the
/// [`Effect`] a hit carries, so a "fire" hit that burns and a "fire" hit
/// that does something else both work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Neutral,
    Fire,
    Ice,
    Thunder,
}

impl Element {
    /// Every element, in spell-table order (the kit's ladder index).
    pub const ALL: [Element; 4] = [
        Element::Neutral,
        Element::Fire,
        Element::Ice,
        Element::Thunder,
    ];

    /// This element's index in [`Element::ALL`] (and in the kit's tables).
    pub fn index(self) -> usize {
        match self {
            Element::Neutral => 0,
            Element::Fire => 1,
            Element::Ice => 2,
            Element::Thunder => 3,
        }
    }
}

/// What one hit leaves behind. Copy, so kit tables can be `const`.
/// One attack carries exactly one effect; combinations come from pairing
/// a weapon's effect with a spell's, never from stacking two here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    None,
    /// Burn: `dps` for `secs`, dealt in fixed [`BURN_TICK_SECS`] slices.
    Burn {
        dps: f32,
        secs: f32,
    },
    /// Slow: time runs at `factor` (0..1) for `secs`.
    Slow {
        factor: f32,
        secs: f32,
    },
    /// Chain: the hit jumps to the nearest not-yet-hit opposing body,
    /// `jumps` times, each jump dealing the attack's damage × `scale`.
    Chain {
        jumps: u8,
        range: f32,
        scale: f32,
    },
}

impl Effect {
    /// The element this effect presents as (for colour, sound, events).
    pub fn element(&self) -> Element {
        match self {
            Effect::None => Element::Neutral,
            Effect::Burn { .. } => Element::Fire,
            Effect::Slow { .. } => Element::Ice,
            Effect::Chain { .. } => Element::Thunder,
        }
    }
}

/// The ailments on one body. Times read the same run clock as
/// `invuln_until` / `flash_until`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    /// Burn damage per second (0 = not burning).
    pub burn_dps: f32,
    /// The burn ends at this time.
    pub burn_until: f32,
    /// The next burn tick lands at this time.
    pub burn_next: f32,
    /// Slow speed factor (1.0 = not slowed).
    pub slow: f32,
    /// The slow ends at this time.
    pub slow_until: f32,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            burn_dps: 0.0,
            burn_until: 0.0,
            burn_next: 0.0,
            slow: 1.0,
            slow_until: 0.0,
        }
    }
}

impl Status {
    /// Put an effect on this body. Reapplication never stacks numbers:
    /// a burn keeps the stronger dps and the later end, a slow keeps the
    /// stronger (smaller) factor and the later end. Durations shrink by
    /// the body's `resist` and scale by its affinity's status multiplier
    /// for the effect's element (`time_scale`); a scaled duration of
    /// zero or less means immunity — the ailment never lands. A chain
    /// leaves nothing behind.
    pub fn inflict(&mut self, effect: &Effect, resist: f32, time_scale: f32, now: f32) {
        let keep = (1.0 - resist.clamp(0.0, 1.0)) * time_scale.max(0.0);
        match *effect {
            Effect::None | Effect::Chain { .. } => {}
            Effect::Burn { dps, secs } => {
                if secs * keep <= 0.0 {
                    return;
                }
                let until = now + secs * keep;
                if self.burn_dps <= 0.0 || now >= self.burn_until {
                    // A fresh burn: the first tick is one period out.
                    self.burn_dps = dps;
                    self.burn_until = until;
                    self.burn_next = now + BURN_TICK_SECS;
                } else {
                    self.burn_dps = self.burn_dps.max(dps);
                    self.burn_until = self.burn_until.max(until);
                }
            }
            Effect::Slow { factor, secs } => {
                if secs * keep <= 0.0 {
                    return;
                }
                let until = now + secs * keep;
                if now >= self.slow_until {
                    self.slow = factor;
                    self.slow_until = until;
                } else {
                    self.slow = self.slow.min(factor);
                    self.slow_until = self.slow_until.max(until);
                }
            }
        }
    }
}

/// Where a hit came from, so the kit can voice and score each kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    /// A direct blow (a swing, a bolt's touch).
    Strike,
    /// A burst victim (not the body the bolt touched).
    Burst,
    /// A chain jump's victim.
    Chain,
    /// A status tick (burn damage).
    Tick,
}

/// Seconds between burn damage slices.
pub const BURN_TICK_SECS: f32 = 0.5;
/// A slow never takes a body below this speed factor.
pub const SLOW_FLOOR: f32 = 0.35;
/// Chain jumps flinch but never shove.
pub const CHAIN_KNOCKBACK: f32 = 0.0;
/// Poise damage of one chain jump.
pub const CHAIN_POISE: f32 = 5.0;

/// Knockback velocity decay (per second) and the speed below which it stops.
pub const KNOCKBACK_DECAY: f32 = 6.0;
pub const KNOCKBACK_STOP: f32 = 0.15;
/// How long a hit flashes the body white.
pub const FLASH_SECS: f32 = 0.1;

/// One attack's numbers. The volume is a sector of `radius` around the attacker
/// with half-angle `half_angle` (radians); a `half_angle` of PI or more is a
/// full circle (a nova, an explosion).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackDef {
    pub radius: f32,
    pub half_angle: f32,
    pub damage: f32,
    /// Knockback launch speed (m/s) before the target's mass divides it.
    pub knockback: f32,
    /// Hitstop the kit should apply when this lands (the strongest hit of a
    /// swing wins; hits never add up).
    pub hitstop: f32,
    /// Hitstun seconds for a target with no poise left.
    pub hitstun: f32,
    pub poise_damage: f32,
    /// Launch the target (`hit_fly`) instead of a plain flinch.
    pub fly: bool,
    /// The ailment this blow leaves on whatever it hurts.
    pub effect: Effect,
}

impl AttackDef {
    /// A sword swing: a wide sector in front of the attacker.
    pub const fn sector(radius: f32, half_angle_deg: f32, damage: f32) -> Self {
        Self {
            radius,
            half_angle: half_angle_deg.to_radians(),
            damage,
            knockback: 4.5,
            hitstop: 0.035,
            hitstun: 0.2,
            poise_damage: 10.0,
            fly: false,
            effect: Effect::None,
        }
    }
}

/// The fact of one landed hit. The kit adds sound, juice and rewards.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitEvent {
    pub target: u64,
    pub damage: f32,
    pub killed: bool,
    /// The target was launched (`hit_fly`), not just flinched.
    pub fly: bool,
    /// Unit direction of the knockback.
    pub dir: (f32, f32),
    pub hitstop: f32,
    /// The element the hit presented as.
    pub element: Element,
    /// Where the hit came from (a blow, a burst, a chain jump, a tick).
    pub kind: HitKind,
}

/// Apply one hit to `body`. Direction `dir` points from attacker to target.
/// Returns the event, or `None` when the body could not be hit at `now`.
pub fn apply_hit(body: &mut Body, def: &AttackDef, dir: (f32, f32), now: f32) -> Option<HitEvent> {
    if !body.hittable(now) {
        return None;
    }
    let element = def.effect.element();
    // The body's affinity scales the blow before anything reads it:
    // the HP loss, the kill check and the event all see the scaled
    // damage, so a resisted hit is smaller in every sense at once.
    let damage = def.damage * body.affinity.damage[element.index()];
    body.hp -= damage;
    body.flash_until = now + FLASH_SECS;
    // A landed hit opens the body's own i-frame window (the hero's
    // 0.7 s; enemies carry 0 and are never gated by it).
    if body.invuln_on_hit > 0.0 {
        body.invuln_until = now + body.invuln_on_hit;
    }
    let kb = def.knockback / body.mass.max(0.1);
    body.vx += dir.0 * kb;
    body.vz += dir.1 * kb;
    let killed = body.hp <= 0.0;
    let mut fly = false;
    if killed {
        body.hp = 0.0;
        body.state = BodyState::Dead;
        body.state_t = 0.0;
    } else if now < body.armor_until {
        // Armoured: the damage and the poise loss land, the flinch
        // does not. Poise may sit at or below zero here — the break
        // itself is deferred to `settle_armor` when the armour ends.
        if body.poise > 0.0 {
            body.poise -= def.poise_damage;
        }
    } else if body.poise > 0.0 {
        body.poise -= def.poise_damage;
        if body.poise <= 0.0 {
            body.poise = body.poise_max;
            fly = true;
            body.state = BodyState::Launched;
            body.state_t = body.down_secs;
        }
        // Otherwise the blow lands but the body does not flinch.
    } else if def.fly {
        fly = true;
        body.state = BodyState::Launched;
        body.state_t = body.down_secs;
    } else {
        body.state = BodyState::Hitstun;
        body.state_t = def.hitstun;
    }
    if !killed {
        // The ailment lands even when poise ate the flinch: a brute that
        // shrugs off the stagger still catches fire.
        let time_scale = body.affinity.status[element.index()];
        body.status
            .inflict(&def.effect, body.status_resist, time_scale, now);
    }
    Some(HitEvent {
        target: body.id,
        damage,
        killed,
        fly,
        dir,
        hitstop: def.hitstop,
        element,
        kind: HitKind::Strike,
    })
}

/// Follow a landed hit's chain effect: from `origin` (a body the attack
/// just hurt), jump to the nearest hittable opposing body within
/// `range + its hurt radius` that is not in `already`, then from that
/// body onward, up to `jumps` times. Each jump deals `def.damage × scale`
/// with a plain flinch and no knockback, and carries no effect of its own
/// (a chain never chains). Ties go to the smaller uid, and events come
/// back in hop order, so a step replays exactly. A no-op unless
/// `def.effect` is [`Effect::Chain`]. Jumped-to bodies are pushed into
/// `already` as they are hit.
pub fn chain_from(
    origin: u64,
    def: &AttackDef,
    faction: Faction,
    bodies: &mut [Body],
    already: &mut Vec<u64>,
    now: f32,
) -> Vec<HitEvent> {
    let Effect::Chain {
        jumps,
        range,
        scale,
    } = def.effect
    else {
        return Vec::new();
    };
    let jump_def = AttackDef {
        radius: 0.0,
        half_angle: 0.0,
        damage: def.damage * scale,
        knockback: CHAIN_KNOCKBACK,
        hitstop: 0.0,
        hitstun: 0.2,
        poise_damage: CHAIN_POISE,
        fly: false,
        // The chain presents as thunder, so the target's thunder
        // affinity is what scales a jump (the effect itself leaves
        // nothing behind, as before).
        effect: def.effect,
    };
    let mut events = Vec::new();
    let mut cur = origin;
    for _ in 0..jumps {
        let Some(ci) = bodies.iter().position(|b| b.id == cur) else {
            break;
        };
        let (cx, cz) = (bodies[ci].x, bodies[ci].z);
        let mut best: Option<usize> = None;
        let mut best_key = (f32::MAX, u64::MAX);
        for (i, b) in bodies.iter().enumerate() {
            if b.faction == faction || !b.hittable(now) || already.contains(&b.id) {
                continue;
            }
            let d = (b.x - cx).hypot(b.z - cz);
            if d > range + b.hurt_radius {
                continue;
            }
            if (d, b.id) < best_key {
                best_key = (d, b.id);
                best = Some(i);
            }
        }
        let Some(i) = best else { break };
        let b = &mut bodies[i];
        let (dx, dz) = (b.x - cx, b.z - cz);
        let d = dx.hypot(dz);
        let dir = if d > 1e-4 {
            (dx / d, dz / d)
        } else {
            (0.0, 1.0)
        };
        let Some(mut ev) = apply_hit(b, &jump_def, dir, now) else {
            break;
        };
        ev.kind = HitKind::Chain;
        ev.element = Element::Thunder;
        already.push(ev.target);
        cur = ev.target;
        events.push(ev);
    }
    events
}

/// Seconds a launched body stays down before it can act again.
pub const LAUNCHED_SECS: f32 = 0.55;

/// One swing of a weapon: an [`AttackDef`] placed at the attacker's position
/// and facing, remembering whom it already hit so one swing lands once per
/// target even while its active window spans two fixed steps.
#[derive(Clone, Debug)]
pub struct Swing {
    pub id: u64,
    pub def: AttackDef,
    hit: Vec<u64>,
    /// A swing's chain fires once, however many steps its window spans.
    chained: bool,
}

impl Swing {
    pub fn new(id: u64, def: AttackDef) -> Self {
        Self {
            id,
            def,
            hit: Vec::new(),
            chained: false,
        }
    }

    /// Test the volume once, from `(x, z)` facing `facing`, against every body
    /// of the opposing faction. Hits come back sorted by target uid, so a step
    /// replays exactly.
    pub fn strike(
        &mut self,
        x: f32,
        z: f32,
        facing: f32,
        faction: Faction,
        bodies: &mut [Body],
        now: f32,
    ) -> Vec<HitEvent> {
        let (fx, fz) = (facing.sin(), facing.cos());
        let mut events = Vec::new();
        for body in bodies.iter_mut() {
            if body.faction == faction || !body.hittable(now) || self.hit.contains(&body.id) {
                continue;
            }
            let (dx, dz) = (body.x - x, body.z - z);
            let d = dx.hypot(dz);
            if d > self.def.radius + body.hurt_radius {
                continue;
            }
            if self.def.half_angle < std::f32::consts::PI && d > body.hurt_radius {
                // Angle to the body's centre, with slack for its radius: a wide
                // body at the sector's edge still counts when its bulk is in.
                let cos = (dx * fx + dz * fz) / d.max(1e-5);
                let ang = cos.clamp(-1.0, 1.0).acos();
                let slack = (body.hurt_radius / d.max(body.hurt_radius)).asin();
                if ang > self.def.half_angle + slack {
                    continue;
                }
            }
            let dir = if d > 1e-4 { (dx / d, dz / d) } else { (fx, fz) };
            if let Some(ev) = apply_hit(body, &self.def, dir, now) {
                self.hit.push(body.id);
                events.push(ev);
            }
        }
        if !events.is_empty() && !self.chained && matches!(self.def.effect, Effect::Chain { .. }) {
            // The chain starts from the struck body nearest the attacker,
            // never jumps back onto a body this swing already cut, and
            // its victims join the hit set so a later step of the same
            // swing cannot cut them again.
            self.chained = true;
            let origin = events
                .iter()
                .filter_map(|e| {
                    bodies
                        .iter()
                        .find(|b| b.id == e.target)
                        .map(|b| (e.target, (b.x - x).hypot(b.z - z)))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
                .map(|(id, _)| id);
            if let Some(origin) = origin {
                let def = self.def;
                let mut already = self.hit.clone();
                let chained = chain_from(origin, &def, faction, bodies, &mut already, now);
                for ev in &chained {
                    self.hit.push(ev.target);
                }
                events.extend(chained);
            }
        }
        events.sort_by_key(|e| e.target);
        events
    }
}

/// Which stretch of a [`Telegraph`] the attack is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelegraphPhase {
    /// The tell: the attack is coming but cannot land yet.
    Windup,
    /// The blow is live: `strike` can land.
    Active,
    /// The whiff / follow-through: vulnerable, cannot land.
    Recover,
    /// Spent (or cancelled).
    Done,
}

/// A telegraphed attack's shape: three timed stretches around one
/// [`AttackDef`]. The numbers are the kit's (an enemy table, a boss
/// table); this type only owns the clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TelegraphDef {
    /// The tell's length (s). Readable attacks keep this ≥ 0.55.
    pub windup: f32,
    /// How long the blow stays live (s).
    pub active: f32,
    /// The follow-through (s) after the blow.
    pub recover: f32,
    /// The blow itself: volume, damage, ailment.
    pub def: AttackDef,
    /// Wear armour through the active stretch and the first 0.2 s of
    /// recovery (the kit turns this into the attacker's `armor_until`;
    /// the state machine only carries the flag).
    pub armor: bool,
}

/// One telegraphed attack in flight: windup → active → recover, timed
/// by the caller. The caller passes a *scaled* `dt` (a slowed attacker's
/// `speed_scale` multiplies it), so a frozen tell really does take
/// longer. The blow rides a [`Swing`], so a moving origin — a charge,
/// a growing ring — still lands at most once per target.
#[derive(Clone, Debug)]
pub struct Telegraph {
    pub def: TelegraphDef,
    /// Seconds inside the current phase.
    pub t: f32,
    pub phase: TelegraphPhase,
    pub swing: Swing,
}

impl Telegraph {
    pub fn new(id: u64, def: TelegraphDef) -> Self {
        Self {
            swing: Swing::new(id, def.def),
            def,
            t: 0.0,
            phase: TelegraphPhase::Windup,
        }
    }

    /// Advance the clock. Returns the phase just entered, if the step
    /// crossed a boundary; a step crosses at most one (the fixed step
    /// is far shorter than any stretch worth telegraphing).
    pub fn step(&mut self, dt: f32) -> Option<TelegraphPhase> {
        if self.phase == TelegraphPhase::Done {
            return None;
        }
        self.t += dt;
        let limit = match self.phase {
            TelegraphPhase::Windup => self.def.windup,
            TelegraphPhase::Active => self.def.active,
            TelegraphPhase::Recover => self.def.recover,
            TelegraphPhase::Done => return None,
        };
        if self.t < limit {
            return None;
        }
        self.t -= limit;
        self.phase = match self.phase {
            TelegraphPhase::Windup => TelegraphPhase::Active,
            TelegraphPhase::Active => TelegraphPhase::Recover,
            TelegraphPhase::Recover | TelegraphPhase::Done => TelegraphPhase::Done,
        };
        Some(self.phase)
    }

    /// Land the blow from `(x, z)`: only while the active stretch runs.
    /// Safe to call every step — the swing remembers its victims.
    pub fn strike(
        &mut self,
        x: f32,
        z: f32,
        facing: f32,
        faction: Faction,
        bodies: &mut [Body],
        now: f32,
    ) -> Vec<HitEvent> {
        if self.phase != TelegraphPhase::Active {
            return Vec::new();
        }
        self.swing.strike(x, z, facing, faction, bodies, now)
    }

    /// How far the tell has wound up, 0..1 (1 once it is over): the
    /// ground warning fills by this.
    pub fn progress(&self) -> f32 {
        match self.phase {
            TelegraphPhase::Windup if self.def.windup > 0.0 => {
                (self.t / self.def.windup).clamp(0.0, 1.0)
            }
            _ => 1.0,
        }
    }

    /// Call the attack off (a break, a phase change): it will never land.
    pub fn cancel(&mut self) {
        self.phase = TelegraphPhase::Done;
    }
}

/// One rung of a charged shot: hold the button at least `hold_secs` and the
/// release fires with these numbers instead of the tap's. Tier 0 is the tap
/// itself (`hold_secs` 0). Damage, radius, blast and knockback all grow
/// together, so a charged shot reads as *bigger and stronger*, never just
/// faster — and holding is never pointless: the top rungs trade the wait
/// for area, bursting on impact where the tap only ever touches one body.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChargeTier {
    /// Seconds the button must be held to reach this tier.
    pub hold_secs: f32,
    pub damage: f32,
    /// Projectile radius (m). Also its drawn size, so growth is visible.
    pub radius: f32,
    pub knockback: f32,
    /// Impact burst radius (m): on a body or wall hit, every opposing body
    /// within this distance of the impact point takes the hit too. 0 is a
    /// plain single-target bolt (the tap).
    pub blast: f32,
    /// The ailment the bolt's hits leave behind (its burst victims too,
    /// and the origin of its chain, when the effect is one).
    pub effect: Effect,
}

/// A charged attack's tiers, weakest first. The kit owns the table (its
/// balance); this type owns the lookup, so every charged weapon in every kit
/// answers "how long was that held?" the same way.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChargeLadder {
    tiers: &'static [ChargeTier],
}

impl ChargeLadder {
    pub const fn new(tiers: &'static [ChargeTier]) -> Self {
        Self { tiers }
    }

    /// How many tiers the ladder has (at least the tap tier).
    pub fn len(&self) -> usize {
        self.tiers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tiers.is_empty()
    }

    /// The tier a hold of `held_secs` has reached: the last rung whose
    /// threshold has passed. Holding past the top rung stays on it.
    pub fn tier(&self, held_secs: f32) -> usize {
        let mut t = 0;
        for (i, rung) in self.tiers.iter().enumerate() {
            if held_secs >= rung.hold_secs {
                t = i;
            }
        }
        t
    }

    /// The numbers of one tier (clamped to the ladder).
    pub fn get(&self, tier: usize) -> ChargeTier {
        self.tiers[tier.min(self.tiers.len().saturating_sub(1))]
    }

    /// Charge progress 0..=1 across the whole ladder, for a growing visual:
    /// 0 at the tap, 1 when the top rung is reached.
    pub fn progress(&self, held_secs: f32) -> f32 {
        let top = self.tiers.last().map(|r| r.hold_secs).unwrap_or(0.0);
        if top <= 0.0 {
            return 1.0;
        }
        (held_secs / top).clamp(0.0, 1.0)
    }
}

/// One stage of a melee combo, at tempo 1.0 (the weapon's own pace is
/// applied by [`Weapon::stage`]). `hit_at` is the moment the blow lands,
/// measured from the animation it plays; the phase `hit_at / total` is
/// a property of the motion, so it survives every tempo unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComboStage {
    /// The stage's full length (s) at tempo 1.0.
    pub total: f32,
    /// When the blow lands (s from the stage's start), at tempo 1.0.
    pub hit_at: f32,
    /// The blow itself: volume, damage, and the ailment it leaves.
    pub def: AttackDef,
    /// Step-in speed (m/s) while the swing starts.
    pub step_speed: f32,
}

/// A melee weapon: a combo's stages at tempo 1.0 plus the pace this
/// weapon plays them at. The kit owns the table (its balance); this
/// type owns the tempo lookup, so every kit answers "what does stage
/// *i* look like in this weapon's hands?" the same way: `total` and
/// `hit_at` shrink by the same factor, the phase never moves, and the
/// blow itself (damage, reach, ailment) is read as authored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weapon {
    /// Stable id: state JSON and drop tables key on it.
    pub id: &'static str,
    stages: &'static [ComboStage],
    /// Playback pace: 1.25 is a quarter faster than the authored combo.
    pub tempo: f32,
}

impl Weapon {
    pub const fn new(id: &'static str, stages: &'static [ComboStage], tempo: f32) -> Self {
        Self { id, stages, tempo }
    }

    /// How many stages the combo has.
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// Stage `i` at this weapon's tempo (clamped to the last stage):
    /// total and hit_at divided by the tempo, the blow as authored.
    pub fn stage(&self, i: usize) -> ComboStage {
        let mut s = self.stages[i.min(self.stages.len().saturating_sub(1))];
        s.total /= self.tempo;
        s.hit_at /= self.tempo;
        s
    }
}

/// The branch moves a combo tree offers, one slot per chain stage:
/// `Some` where a stage branches, `None` where it does not. The
/// payload is the kit's — a melee tree carries the branch's own
/// [`ComboStage`], a kit whose branches are casts carries a move
/// id — combat owns the shape (one optional branch per stage) and
/// the input machine ([`ComboChain`]), never what a move does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComboTree<B: Copy + 'static> {
    branches: &'static [Option<B>],
}

impl<B: Copy + 'static> ComboTree<B> {
    pub const fn new(branches: &'static [Option<B>]) -> Self {
        Self { branches }
    }

    /// How many stages the tree covers (the chain's length).
    pub fn len(&self) -> usize {
        self.branches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    /// The branch declared at `stage`, if it declares one.
    pub fn branch(&self, stage: usize) -> Option<B> {
        self.branches.get(stage).copied().flatten()
    }

    /// Whether `stage` declares a branch at all.
    pub fn has_branch(&self, stage: usize) -> bool {
        self.branch(stage).is_some()
    }
}

/// What taking a branch press did, told by [`ComboChain::choose_branch`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchTake {
    /// The branch is chosen: it fires when the current stage ends,
    /// in the next stage's slot.
    Armed,
    /// The press landed in the **last** stage: there is no next
    /// slot, so the branch converts the finisher's recovery at
    /// once — the kit starts the branch move now and ends the chain.
    ConvertNow,
    /// The current stage declares no branch (or no window is open):
    /// nothing was chosen. A guard, not a path — the kit only takes
    /// the press when a branch exists.
    NoBranch,
}

/// What a finished stage gives way to, decided by [`ComboChain::advance`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComboAdvance {
    /// The plain next stage (index).
    Next(usize),
    /// The branch chosen during stage `usize`: it fires in the slot
    /// after that stage, and the combo ends with it.
    Branch(usize),
    /// The combo is over.
    End,
}

/// One branched combo in flight: the stage swinging, whether its
/// reservation window has opened, and what the player has reserved
/// inside it. This machine owns the input interpretation every
/// branched combo shares — the Musou shape: the plain button walks
/// the chain, and the charge button pressed *inside a swing's
/// window* branches at that stage.
///
/// - The window opens when the stage's blow lands. A charge press
///   that landed *before* the window is not a branch press: the kit
///   sweeps it (drops it unfired) on the opening step this machine
///   reports — the standing rule that keeps a bolt meant before the
///   swing from detonating inside it.
/// - A plain press inside the window reserves the next stage; a
///   charge press inside the window chooses the current stage's
///   branch. The branch outranks the reservation.
/// - A kit with two plain buttons (a wide cut and a diagonal cut,
///   say) tags its reservation with [`ComboChain::queue_next_tag`]:
///   the machine carries the tag untouched and the kit reads it
///   back with [`ComboChain::queued_tag`] when the stage ends.
///   The untagged [`ComboChain::queue_next`] is tag 0.
/// - A branch chosen before the last stage fires in the next
///   stage's slot (the plain stage there never happens). A branch
///   chosen *in* the last stage has no slot left: it converts the
///   finisher's recovery on the spot ([`BranchTake::ConvertNow`]).
///
/// The kit owns the buttons, the clocks and every move: it polls
/// its buttons, feeds the machine, and acts on what it answers.
/// A branch move is not a chain stage — starting one ends the
/// chain, and presses during it are the kit's ordinary business.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComboChain {
    stage: usize,
    active: bool,
    window_open: bool,
    queued: bool,
    queued_tag: u8,
    branch: Option<usize>,
}

impl ComboChain {
    pub fn new() -> Self {
        Self::default()
    }

    /// A combo is in flight (between [`ComboChain::begin`] and
    /// [`ComboChain::end`]).
    pub fn active(&self) -> bool {
        self.active
    }

    /// The stage swinging now (0-based).
    pub fn stage(&self) -> usize {
        self.stage
    }

    /// Start stage `stage`: a fresh window, nothing reserved, no
    /// branch chosen. The kit calls this as each plain stage begins.
    pub fn begin(&mut self, stage: usize) {
        self.stage = stage;
        self.active = true;
        self.window_open = false;
        self.queued = false;
        self.queued_tag = 0;
        self.branch = None;
    }

    /// Feed the blow's landing: the reservation window opens on the
    /// first step `hit_landed` is true. Returns true on that step
    /// only — the step the kit sweeps a stale charge press.
    pub fn open_window(&mut self, hit_landed: bool) -> bool {
        if self.active && hit_landed && !self.window_open {
            self.window_open = true;
            return true;
        }
        false
    }

    /// The current stage's window is open.
    pub fn window_open(&self) -> bool {
        self.window_open
    }

    /// A plain press inside the window: reserve the next stage.
    /// (A press fed before the window opens is the kit's buffer to
    /// hold; it never reserves from outside the window.)
    pub fn queue_next(&mut self) {
        self.queue_next_tag(0);
    }

    /// A plain press inside the window, tagged: reserve the next
    /// stage and remember which plain button asked for it. A kit
    /// whose plain button comes in two flavours (wide / diagonal)
    /// reads the tag back with [`ComboChain::queued_tag`] at the
    /// stage's end and plays that flavour's move next. The last
    /// tag queued inside the window stands.
    pub fn queue_next_tag(&mut self, tag: u8) {
        if self.active && self.window_open {
            self.queued = true;
            self.queued_tag = tag;
        }
    }

    /// The next stage is reserved.
    pub fn queued(&self) -> bool {
        self.queued
    }

    /// The tag the reservation carries (0 when the press was fed
    /// through [`ComboChain::queue_next`], or when nothing is
    /// reserved). Read it before [`ComboChain::advance`]: advancing
    /// consumes the reservation and its tag with it.
    pub fn queued_tag(&self) -> u8 {
        self.queued_tag
    }

    /// A charge press inside the window: choose the current stage's
    /// branch. `has_branch` is the tree's answer for this stage,
    /// `is_last` whether it is the chain's last.
    pub fn choose_branch(&mut self, has_branch: bool, is_last: bool) -> BranchTake {
        if !self.active || !self.window_open || !has_branch {
            return BranchTake::NoBranch;
        }
        if is_last {
            return BranchTake::ConvertNow;
        }
        self.branch = Some(self.stage);
        BranchTake::Armed
    }

    /// The stage whose branch was chosen, if one was.
    pub fn branch(&self) -> Option<usize> {
        self.branch
    }

    /// The current stage ended: what follows. A chosen branch
    /// outranks a reservation; a reservation walks the chain;
    /// anything else ends the combo. The choice is consumed either
    /// way — the kit starts the next stage with [`ComboChain::begin`],
    /// a branch move (or the end) with [`ComboChain::end`].
    pub fn advance(&mut self, stages: usize) -> ComboAdvance {
        let out = if let Some(b) = self.branch {
            ComboAdvance::Branch(b)
        } else if self.queued && self.stage + 1 < stages {
            ComboAdvance::Next(self.stage + 1)
        } else {
            ComboAdvance::End
        };
        self.queued = false;
        self.queued_tag = 0;
        self.branch = None;
        self.window_open = false;
        out
    }

    /// The combo is over or cancelled (a branch move started, a
    /// dodge rolled out, the run reset): nothing is swinging,
    /// nothing is reserved.
    pub fn end(&mut self) {
        self.active = false;
        self.window_open = false;
        self.queued = false;
        self.queued_tag = 0;
        self.branch = None;
    }
}

/// A bolt of magic (or any travelling shot): a small circle flying straight.
/// It dies on the first wall or the first opposing body it touches. A bolt
/// with a `blast` radius bursts where it lands: the impact hurts every
/// opposing body around the point, not just the one it touched.
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub id: u64,
    pub faction: Faction,
    pub x: f32,
    pub z: f32,
    /// Height of the bolt's centre; a body is hit when this is under its
    /// `hurt_height`.
    pub y: f32,
    pub dx: f32,
    pub dz: f32,
    pub speed: f32,
    pub radius: f32,
    pub damage: f32,
    pub knockback: f32,
    /// Impact burst radius (m), 0 for a plain single-target bolt. The burst
    /// goes off on a body hit and on a wall hit, never on a range fizzle.
    pub blast: f32,
    /// The ailment the bolt's hits carry: the direct hit, the burst and
    /// the chain all read it.
    pub effect: Effect,
    /// Flight left (m) before it fizzles out.
    pub range_left: f32,
    pub alive: bool,
}

/// What one [`Projectile::step`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoltOutcome {
    /// Every body hurt this step: the direct hit first (if any), then the
    /// burst's victims, sorted by target after the direct hit.
    pub events: Vec<HitEvent>,
    /// Where the bolt ended in an impact — a body's position or the wall
    /// point — when it did. `None` while still flying, and after a range
    /// fizzle (a fizzle never bursts).
    pub impact: Option<(f32, f32)>,
}

impl Projectile {
    /// The hit a burst (or the bolt's own touch) deals: the bolt's damage
    /// and knockback, a standard flinch, no launch — and the bolt's own
    /// effect, so a burst's victims burn or slow like the direct hit.
    fn burst_def(&self) -> AttackDef {
        AttackDef {
            radius: 0.0,
            half_angle: 0.0,
            damage: self.damage,
            knockback: self.knockback,
            hitstop: 0.0,
            hitstun: 0.2,
            poise_damage: 5.0,
            fly: false,
            effect: self.effect,
        }
    }

    /// Hurt every opposing body within `blast` of the bolt's position,
    /// knocked back away from the burst centre. `direct` names the body the
    /// bolt itself just hit (it is not hurt twice). No-op at blast 0.
    pub fn detonate(&self, bodies: &mut [Body], now: f32, direct: Option<u64>) -> Vec<HitEvent> {
        if self.blast <= 0.0 {
            return Vec::new();
        }
        let def = self.burst_def();
        let mut events = Vec::new();
        for body in bodies.iter_mut() {
            if body.faction == self.faction || !body.hittable(now) || self.y > body.hurt_height {
                continue;
            }
            if Some(body.id) == direct {
                continue;
            }
            let (dx, dz) = (body.x - self.x, body.z - self.z);
            let d = dx.hypot(dz);
            if d > self.blast + body.hurt_radius {
                continue;
            }
            let dir = if d > 1e-4 {
                (dx / d, dz / d)
            } else {
                (self.dx, self.dz)
            };
            if let Some(mut ev) = apply_hit(body, &def, dir, now) {
                ev.kind = HitKind::Burst;
                events.push(ev);
            }
        }
        events.sort_by_key(|e| e.target);
        events
    }

    /// The chain a bolt's impact sets off, when its effect is one: from
    /// the body it touched (or, for a wall burst, the burst victim
    /// nearest the impact point) outward past the burst — a thunder bolt
    /// reaches bodies its blast never touched. Nobody hit, no chain.
    fn chain_events(
        &self,
        bodies: &mut [Body],
        now: f32,
        prior: &[HitEvent],
        direct: Option<u64>,
    ) -> Vec<HitEvent> {
        if !matches!(self.effect, Effect::Chain { .. }) || prior.is_empty() {
            return Vec::new();
        }
        let origin = match direct {
            Some(id) => id,
            None => {
                let mut best: Option<(f32, u64)> = None;
                for e in prior {
                    if let Some(b) = bodies.iter().find(|b| b.id == e.target) {
                        let d = (b.x - self.x).hypot(b.z - self.z);
                        if best.map(|(bd, _)| d < bd).unwrap_or(true) {
                            best = Some((d, e.target));
                        }
                    }
                }
                match best {
                    Some((_, id)) => id,
                    None => return Vec::new(),
                }
            }
        };
        let mut already: Vec<u64> = prior.iter().map(|e| e.target).collect();
        let def = self.burst_def();
        chain_from(origin, &def, self.faction, bodies, &mut already, now)
    }

    /// Advance one step. `blocked(x, z)` reports solid stage geometry (already
    /// expanded by the bolt's radius). Returns what the step did: the hits
    /// (direct + burst) and the impact point, if the bolt ended in one.
    pub fn step(
        &mut self,
        dt: f32,
        bodies: &mut [Body],
        now: f32,
        blocked: impl Fn(f32, f32) -> bool,
    ) -> BoltOutcome {
        let mut out = BoltOutcome::default();
        if !self.alive {
            return out;
        }
        let travel = self.speed * dt;
        self.x += self.dx * travel;
        self.z += self.dz * travel;
        self.range_left -= travel;
        if self.range_left <= 0.0 {
            // A fizzle: the bolt dies quietly and never bursts.
            self.alive = false;
            return out;
        }
        if blocked(self.x, self.z) {
            self.alive = false;
            out.impact = Some((self.x, self.z));
            out.events = self.detonate(bodies, now, None);
            let chained = self.chain_events(bodies, now, &out.events, None);
            out.events.extend(chained);
            return out;
        }
        // The first body touched along the flight wins; scan order is uid
        // order, and bodies are stored near enough to it for a prototype.
        let mut best: Option<usize> = None;
        let mut best_d = f32::MAX;
        for (i, body) in bodies.iter().enumerate() {
            if body.faction == self.faction || !body.hittable(now) || self.y > body.hurt_height {
                continue;
            }
            let d = (body.x - self.x).hypot(body.z - self.z);
            if d <= self.radius + body.hurt_radius && d < best_d {
                best = Some(i);
                best_d = d;
            }
        }
        let Some(i) = best else {
            return out;
        };
        let def = self.burst_def();
        let direct = bodies[i].id;
        if let Some(ev) = apply_hit(&mut bodies[i], &def, (self.dx, self.dz), now) {
            out.events.push(ev);
        }
        self.alive = false;
        out.impact = Some((self.x, self.z));
        out.events.extend(self.detonate(bodies, now, Some(direct)));
        let chained = self.chain_events(bodies, now, &out.events, Some(direct));
        out.events.extend(chained);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mob(id: u64, x: f32, z: f32) -> Body {
        Body::new(id, Faction::Enemy, x, z, 30.0, 0.42)
    }

    fn swing_def() -> AttackDef {
        AttackDef::sector(2.3, 65.0, 10.0)
    }

    #[test]
    fn a_sector_hits_the_front_and_not_the_back() {
        let mut bodies = vec![mob(1, 0.0, 1.5), mob(2, 0.0, -1.5), mob(3, 1.6, 1.6)];
        let mut sw = Swing::new(1, swing_def());
        // Facing +z (yaw 0): mob 1 ahead, mob 2 behind, mob 3 at 45 degrees.
        let evs = sw.strike(0.0, 0.0, 0.0, Faction::Hero, &mut bodies, 0.0);
        let ids: Vec<u64> = evs.iter().map(|e| e.target).collect();
        assert_eq!(ids, vec![1, 3]);
        assert_eq!(bodies[0].hp, 20.0);
        assert_eq!(bodies[1].hp, 30.0, "behind the swing is safe");
        assert_eq!(bodies[0].state, BodyState::Hitstun);
    }

    #[test]
    fn one_swing_lands_once_per_target_across_steps() {
        let mut bodies = vec![mob(1, 0.0, 1.5)];
        let mut sw = Swing::new(1, swing_def());
        let first = sw.strike(0.0, 0.0, 0.0, Faction::Hero, &mut bodies, 0.0);
        let second = sw.strike(0.0, 0.0, 0.0, Faction::Hero, &mut bodies, 0.02);
        assert_eq!(first.len(), 1);
        assert!(second.is_empty(), "the same swing cannot hit twice");
        assert_eq!(bodies[0].hp, 20.0);
    }

    #[test]
    fn a_circle_hits_all_around() {
        let mut bodies = vec![mob(1, 0.0, -1.0), mob(2, -1.0, 0.0)];
        let mut def = swing_def();
        def.half_angle = std::f32::consts::PI;
        let mut sw = Swing::new(9, def);
        let evs = sw.strike(0.0, 0.0, 0.0, Faction::Hero, &mut bodies, 0.0);
        assert_eq!(evs.len(), 2);
    }

    #[test]
    fn invulnerability_and_death_gate_hits() {
        let mut b = mob(1, 0.0, 1.0);
        b.invuln_until = 1.0;
        assert!(apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.5).is_none());
        let ev = apply_hit(&mut b, &swing_def(), (0.0, 1.0), 1.0).unwrap();
        assert!(!ev.killed);
        let mut lethal = swing_def();
        lethal.damage = 50.0;
        let ev = apply_hit(&mut b, &lethal, (0.0, 1.0), 1.1).unwrap();
        assert!(ev.killed);
        assert_eq!(b.state, BodyState::Dead);
        assert!(apply_hit(&mut b, &swing_def(), (0.0, 1.0), 1.2).is_none());
    }

    #[test]
    fn knockback_is_a_velocity_that_decays_and_slides() {
        let mut b = mob(1, 0.0, 0.0);
        apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.0).unwrap();
        assert!((b.vz - 4.5).abs() < 1e-4, "launch speed before mass");
        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            b.integrate(dt);
        }
        assert!(b.z > 0.3 && b.z < 0.8, "slid and stopped: {}", b.z);
        assert_eq!((b.vx, b.vz), (0.0, 0.0));
        assert_eq!(b.state, BodyState::Active, "the flinch ended");
    }

    #[test]
    fn heavy_bodies_knock_back_less_and_poise_eats_flinches() {
        let mut brute = mob(1, 0.0, 1.0);
        brute.hp = 100.0;
        brute.max_hp = 100.0;
        brute.mass = 3.0;
        brute.poise = 25.0;
        brute.poise_max = 25.0;
        let ev = apply_hit(&mut brute, &swing_def(), (0.0, 1.0), 0.0).unwrap();
        assert!((brute.vz - 1.5).abs() < 1e-4, "mass divides knockback");
        assert_eq!(brute.state, BodyState::Active, "poise ate the flinch");
        assert!(!ev.fly);
        apply_hit(&mut brute, &swing_def(), (0.0, 1.0), 0.1).unwrap();
        apply_hit(&mut brute, &swing_def(), (0.0, 1.0), 0.2).unwrap();
        assert_eq!(brute.state, BodyState::Launched, "poise broke: launched");
        assert_eq!(brute.poise, 25.0, "poise refills after the break");
    }

    #[test]
    fn a_flying_hit_launches_instead_of_flinching() {
        let mut b = mob(1, 0.0, 1.0);
        let mut def = swing_def();
        def.fly = true;
        let ev = apply_hit(&mut b, &def, (0.0, 1.0), 0.0).unwrap();
        assert!(ev.fly);
        assert_eq!(b.state, BodyState::Launched);
    }

    #[test]
    fn a_projectile_hits_the_first_body_and_dies_on_walls() {
        let mut bodies = vec![mob(1, 0.0, 3.0), mob(2, 0.0, 5.0)];
        let mut p = Projectile {
            id: 1,
            faction: Faction::Hero,
            x: 0.0,
            z: 0.0,
            y: 1.1,
            dx: 0.0,
            dz: 1.0,
            speed: 18.0,
            radius: 0.28,
            damage: 12.0,
            knockback: 2.5,
            blast: 0.0,
            effect: Effect::None,
            range_left: 16.0,
            alive: true,
        };
        let dt = 1.0 / 60.0;
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p.step(dt, &mut bodies, step as f32 * dt, |_, _| false);
            if !out.events.is_empty() {
                break;
            }
        }
        assert_eq!(out.events.len(), 1, "blast 0 touches one body only");
        assert_eq!(out.events[0].target, 1);
        assert!(out.impact.is_some());
        assert_eq!(bodies[0].hp, 18.0);
        assert_eq!(bodies[1].hp, 30.0, "no piercing");
        assert!(!p.alive);

        // A wall between muzzle and target eats the bolt.
        let mut p2 = Projectile {
            alive: true,
            ..p.clone()
        };
        p2.x = 0.0;
        p2.z = 0.0;
        let mut bodies = vec![mob(1, 0.0, 3.0)];
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p2.step(dt, &mut bodies, step as f32 * dt, |_, z| z > 1.5);
            if !p2.alive {
                break;
            }
        }
        assert!(out.events.is_empty());
        assert!(out.impact.is_some(), "a wall is an impact");
        assert_eq!(bodies[0].hp, 30.0);
    }

    #[test]
    fn a_blast_bolt_hurts_the_cluster_not_just_the_first_body() {
        // Three mobs in a knot, one standing apart: the bolt touches the
        // first, the burst takes the knot, the loner is untouched.
        let mut bodies = vec![
            mob(1, 0.0, 3.0),
            mob(2, 0.9, 3.4),
            mob(3, -0.8, 3.5),
            mob(4, 0.0, 7.0),
        ];
        let mut p = Projectile {
            id: 1,
            faction: Faction::Hero,
            x: 0.0,
            z: 0.0,
            y: 1.1,
            dx: 0.0,
            dz: 1.0,
            speed: 18.0,
            radius: 0.45,
            damage: 21.0,
            knockback: 4.0,
            blast: 1.2,
            effect: Effect::None,
            range_left: 16.0,
            alive: true,
        };
        let dt = 1.0 / 60.0;
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p.step(dt, &mut bodies, step as f32 * dt, |_, _| false);
            if !out.events.is_empty() {
                break;
            }
        }
        let mut ids: Vec<u64> = out.events.iter().map(|e| e.target).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3], "direct hit plus the whole knot");
        assert_eq!(bodies[0].hp, 9.0, "the direct body is hurt once, not twice");
        assert_eq!(bodies[1].hp, 9.0);
        assert_eq!(bodies[2].hp, 9.0);
        assert_eq!(bodies[3].hp, 30.0, "outside the burst is safe");
        // Burst knockback points away from the burst centre: mob 2 stands
        // to the right of the impact and is pushed further right.
        assert!(bodies[1].vx > 0.0, "radial knockback: {}", bodies[1].vx);
        assert!(bodies[2].vx < 0.0, "radial knockback: {}", bodies[2].vx);
    }

    #[test]
    fn a_blast_goes_off_on_a_wall_but_not_on_a_fizzle() {
        let mut p = Projectile {
            id: 1,
            faction: Faction::Hero,
            x: 0.0,
            z: 0.0,
            y: 1.1,
            dx: 0.0,
            dz: 1.0,
            speed: 18.0,
            radius: 0.66,
            damage: 34.0,
            knockback: 6.0,
            blast: 2.3,
            effect: Effect::None,
            range_left: 16.0,
            alive: true,
        };
        let dt = 1.0 / 60.0;
        // The bolt dies on the wall at z > 1.5, next to a mob it never
        // touched: the burst still gets him.
        let mut bodies = vec![mob(1, 0.8, 2.2)];
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p.step(dt, &mut bodies, step as f32 * dt, |_, z| z > 1.5);
            if !p.alive {
                break;
            }
        }
        assert_eq!(out.events.len(), 1, "the wall burst hurts the bystander");
        assert!(out.events[0].killed, "34 damage against 30 HP kills");
        assert!(out.impact.is_some());

        // Out of range in open air: a fizzle, no burst, nobody hurt. The
        // mob stands just past the expiry point, inside the would-be blast
        // (1.8 m from where the bolt dies) but clear of the bolt itself.
        let mut p2 = Projectile {
            range_left: 0.5,
            ..p.clone()
        };
        p2.x = 0.0;
        p2.z = 0.0;
        p2.alive = true;
        let mut bodies = vec![mob(1, 0.0, 2.4)];
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p2.step(dt, &mut bodies, step as f32 * dt, |_, _| false);
            if !p2.alive {
                break;
            }
        }
        assert!(out.events.is_empty());
        assert!(out.impact.is_none(), "a fizzle is not an impact");
        assert_eq!(bodies[0].hp, 30.0);
    }

    #[test]
    fn charge_tiers_grow_with_hold_time() {
        const LADDER: ChargeLadder = ChargeLadder::new(&[
            ChargeTier {
                hold_secs: 0.0,
                damage: 12.0,
                radius: 0.28,
                knockback: 2.5,
                blast: 0.0,
                effect: Effect::None,
            },
            ChargeTier {
                hold_secs: 0.45,
                damage: 21.0,
                radius: 0.45,
                knockback: 4.0,
                blast: 1.2,
                effect: Effect::None,
            },
            ChargeTier {
                hold_secs: 0.95,
                damage: 34.0,
                radius: 0.66,
                knockback: 6.0,
                blast: 2.3,
                effect: Effect::None,
            },
        ]);
        assert_eq!(LADDER.len(), 3);
        // A tap and a short hold are tier 0; thresholds are inclusive.
        assert_eq!(LADDER.tier(0.0), 0);
        assert_eq!(LADDER.tier(0.44), 0);
        assert_eq!(LADDER.tier(0.45), 1);
        assert_eq!(LADDER.tier(0.94), 1);
        assert_eq!(LADDER.tier(0.95), 2);
        // Holding forever stays on the top rung.
        assert_eq!(LADDER.tier(30.0), 2);
        assert_eq!(LADDER.get(2).damage, 34.0);
        assert_eq!(LADDER.get(99).radius, 0.66, "get clamps to the top rung");
        // Every rung is strictly bigger and stronger than the last.
        for w in LADDER.tiers.windows(2) {
            assert!(w[1].damage > w[0].damage);
            assert!(w[1].radius > w[0].radius);
            assert!(w[1].knockback > w[0].knockback);
            assert!(w[1].blast > w[0].blast, "area grows with the charge too");
        }
        assert_eq!(LADDER.get(0).blast, 0.0, "the tap stays single-target");
        assert_eq!(LADDER.progress(0.0), 0.0);
        assert_eq!(LADDER.progress(0.95), 1.0);
        assert_eq!(LADDER.progress(9.0), 1.0);
    }

    /// A two-stage combo at the phase the action kits measure their
    /// clips at (0.40 / 0.33 of the stage).
    const TEST_STAGES: &[ComboStage] = &[
        ComboStage {
            total: 0.36,
            hit_at: 0.144,
            def: AttackDef {
                radius: 1.6,
                half_angle: 0.9,
                damage: 10.0,
                knockback: 5.0,
                hitstop: 0.045,
                hitstun: 0.2,
                poise_damage: 10.0,
                fly: false,
                effect: Effect::None,
            },
            step_speed: 2.0,
        },
        ComboStage {
            total: 0.34,
            hit_at: 0.113,
            def: AttackDef {
                radius: 1.6,
                half_angle: 0.9,
                damage: 14.0,
                knockback: 8.5,
                hitstop: 0.09,
                hitstun: 0.2,
                poise_damage: 10.0,
                fly: true,
                effect: Effect::Burn {
                    dps: 3.0,
                    secs: 2.5,
                },
            },
            step_speed: 3.2,
        },
    ];

    #[test]
    fn a_weapons_tempo_scales_time_never_phase() {
        for tempo in [0.8f32, 1.0, 1.25] {
            let w = Weapon::new("test", TEST_STAGES, tempo);
            assert_eq!(w.len(), 2);
            for (i, &raw) in TEST_STAGES.iter().enumerate() {
                let s = w.stage(i);
                assert!((s.total - raw.total / tempo).abs() < 1e-6);
                assert!((s.hit_at - raw.hit_at / tempo).abs() < 1e-6);
                // The phase is the motion's, not the weapon's.
                assert!(
                    (s.hit_at / s.total - raw.hit_at / raw.total).abs() < 1e-6,
                    "tempo {tempo} moved stage {i}'s phase"
                );
                // The blow itself is read as authored.
                assert_eq!(s.def, raw.def);
                assert_eq!(s.step_speed, raw.step_speed);
            }
        }
        // Past the table, the last stage answers (at tempo).
        let fast = Weapon::new("test", TEST_STAGES, 1.25);
        assert_eq!(fast.stage(9), fast.stage(1));
    }

    #[test]
    fn revive_restores_a_recycled_body() {
        let mut b = mob(1, 0.0, 0.0);
        let mut lethal = swing_def();
        lethal.damage = 99.0;
        apply_hit(&mut b, &lethal, (0.0, 1.0), 0.0).unwrap();
        assert_eq!(b.state, BodyState::Dead);
        b.revive(5.0, -5.0);
        assert_eq!(b.state, BodyState::Active);
        assert_eq!(b.hp, b.max_hp);
        assert_eq!((b.x, b.z), (5.0, -5.0));
    }

    fn burn_def(dps: f32, secs: f32) -> AttackDef {
        let mut def = swing_def();
        def.damage = 0.0;
        def.knockback = 0.0;
        def.effect = Effect::Burn { dps, secs };
        def
    }

    #[test]
    fn burn_ticks_fixed_damage_and_ignores_invuln() {
        let mut b = mob(1, 0.0, 1.0);
        apply_hit(&mut b, &burn_def(4.0, 2.0), (0.0, 1.0), 0.0).unwrap();
        assert_eq!(b.hp, 30.0, "the burn itself deals nothing up front");
        // The inflicting hit's own flinch is not what is being measured.
        b.state = BodyState::Active;
        // Invulnerable the whole time: the fire does not care.
        b.invuln_until = 99.0;
        let mut total = 0.0;
        let mut ticks = 0;
        for k in 1..=4 {
            let ev = b
                .tick_status(0.5 * k as f32)
                .expect("a tick lands every half second");
            assert_eq!(ev.kind, HitKind::Tick);
            assert_eq!(ev.element, Element::Fire);
            assert!(!ev.killed);
            total += ev.damage;
            ticks += 1;
            // A tick never stuns, launches or shoves.
            assert_eq!(b.state, BodyState::Active);
            assert_eq!((b.vx, b.vz), (0.0, 0.0));
        }
        assert_eq!(ticks, 4);
        assert_eq!(total, 8.0, "dps 4 for 2 s is 8 damage in 4 slices");
        assert_eq!(b.hp, 22.0);
        assert!(b.tick_status(2.5).is_none(), "the burn is over");
    }

    #[test]
    fn burn_refresh_takes_the_stronger_and_never_stacks() {
        let mut st = Status::default();
        st.inflict(
            &Effect::Burn {
                dps: 3.0,
                secs: 2.0,
            },
            0.0,
            1.0,
            0.0,
        );
        st.inflict(
            &Effect::Burn {
                dps: 5.0,
                secs: 1.0,
            },
            0.0,
            1.0,
            0.5,
        );
        assert_eq!(st.burn_dps, 5.0, "the stronger burn wins, not 3 + 5");
        assert_eq!(st.burn_until, 2.0, "and the end is the later one");
        // A weaker refresh still extends the end.
        st.inflict(
            &Effect::Burn {
                dps: 1.0,
                secs: 4.0,
            },
            0.0,
            1.0,
            1.0,
        );
        assert_eq!(st.burn_dps, 5.0);
        assert_eq!(st.burn_until, 5.0);
    }

    #[test]
    fn a_burn_tick_can_kill() {
        let mut b = mob(1, 0.0, 1.0);
        b.hp = 1.0;
        apply_hit(&mut b, &burn_def(4.0, 2.0), (0.0, 1.0), 0.0).unwrap();
        let ev = b.tick_status(0.5).expect("the tick lands");
        assert!(ev.killed);
        assert_eq!(ev.kind, HitKind::Tick);
        assert_eq!(b.state, BodyState::Dead);
        assert!(b.tick_status(1.0).is_none(), "the dead do not burn");
    }

    #[test]
    fn slow_scales_and_has_a_floor() {
        let mut b = mob(1, 0.0, 1.0);
        let mut def = swing_def();
        def.damage = 0.0;
        def.effect = Effect::Slow {
            factor: 0.4,
            secs: 1.0,
        };
        apply_hit(&mut b, &def, (0.0, 1.0), 0.0).unwrap();
        assert_eq!(b.speed_scale(0.5), 0.4);
        assert_eq!(b.speed_scale(1.0), 1.0, "the slow ends on time");
        // A stronger slow refreshes but never goes under the floor.
        def.effect = Effect::Slow {
            factor: 0.1,
            secs: 1.0,
        };
        apply_hit(&mut b, &def, (0.0, 1.0), 0.5).unwrap();
        assert_eq!(b.status.slow, 0.1, "the stronger factor is kept");
        assert_eq!(b.speed_scale(0.6), SLOW_FLOOR, "but the floor holds");
    }

    #[test]
    fn status_resist_shortens_duration() {
        let mut b = mob(1, 0.0, 1.0);
        b.status_resist = 0.5;
        apply_hit(&mut b, &burn_def(4.0, 2.0), (0.0, 1.0), 0.0).unwrap();
        assert_eq!(b.status.burn_until, 1.0, "half the burn at resist 0.5");
        let mut def = swing_def();
        def.damage = 0.0;
        def.effect = Effect::Slow {
            factor: 0.5,
            secs: 2.0,
        };
        apply_hit(&mut b, &def, (0.0, 1.0), 0.0).unwrap();
        assert_eq!(b.status.slow_until, 1.0, "half the slow too");
    }

    #[test]
    fn revive_clears_status() {
        let mut b = mob(1, 0.0, 0.0);
        apply_hit(&mut b, &burn_def(4.0, 2.0), (0.0, 1.0), 0.0).unwrap();
        assert!(b.status.burn_dps > 0.0);
        b.revive(5.0, -5.0);
        assert_eq!(b.status, Status::default(), "no recycled burning");
        assert_eq!(b.speed_scale(0.5), 1.0);
    }

    fn chain_def(damage: f32, jumps: u8, range: f32, scale: f32) -> AttackDef {
        let mut def = swing_def();
        def.damage = damage;
        def.effect = Effect::Chain {
            jumps,
            range,
            scale,
        };
        def
    }

    #[test]
    fn chain_hops_nearest_unhit_in_order() {
        // A row of mobs 1.5 m apart; the origin is hurt already, so the
        // chain starts from it and walks down the row, nearest first.
        let mut bodies = vec![
            mob(1, 0.0, 0.0),
            mob(2, 1.5, 0.0),
            mob(3, 3.0, 0.0),
            mob(4, 4.5, 0.0),
        ];
        let def = chain_def(10.0, 2, 3.0, 0.6);
        let mut already = vec![1];
        let evs = chain_from(1, &def, Faction::Hero, &mut bodies, &mut already, 0.0);
        let ids: Vec<u64> = evs.iter().map(|e| e.target).collect();
        assert_eq!(ids, vec![2, 3], "two hops, nearest first, in hop order");
        for e in &evs {
            assert_eq!(e.kind, HitKind::Chain);
            assert_eq!(e.element, Element::Thunder);
            assert_eq!(e.damage, 6.0, "damage x scale, no decay per hop");
        }
        assert_eq!(bodies[1].hp, 24.0);
        assert_eq!(bodies[2].hp, 24.0);
        assert_eq!(bodies[3].hp, 30.0, "the third hop never happens");
        assert_eq!(already, vec![1, 2, 3], "victims join the hit set");
    }

    #[test]
    fn chain_skips_bodies_the_attack_already_hit() {
        // The mob right next to the origin was caught by the burst: the
        // chain must skip it and reach the one past it.
        let mut bodies = vec![mob(1, 0.0, 0.0), mob(2, 1.0, 0.0), mob(3, 2.5, 0.0)];
        let def = chain_def(10.0, 1, 3.0, 0.6);
        let mut already = vec![1, 2];
        let evs = chain_from(1, &def, Faction::Hero, &mut bodies, &mut already, 0.0);
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].target, 3);
        assert_eq!(bodies[1].hp, 30.0, "already hit means already hit");
        // A def with no chain effect is a no-op.
        let mut already = vec![1];
        assert!(chain_from(
            1,
            &swing_def(),
            Faction::Hero,
            &mut bodies,
            &mut already,
            0.0
        )
        .is_empty());
    }

    #[test]
    fn a_swing_chains_once_across_two_steps() {
        // In a line away from the attacker: the swing's circle reaches
        // only the first mob; the second is chain-range from it, the
        // third is out of everyone's reach.
        let mut bodies = vec![mob(1, 0.0, 1.5), mob(2, 0.0, 3.5), mob(3, 0.0, 5.5)];
        let mut def = chain_def(10.0, 1, 3.0, 0.6);
        def.radius = 2.3;
        def.half_angle = std::f32::consts::PI; // the whole circle, both steps
        let mut sw = Swing::new(1, def);
        let first = sw.strike(0.0, 0.0, 0.0, Faction::Hero, &mut bodies, 0.0);
        let chains1 = first.iter().filter(|e| e.kind == HitKind::Chain).count();
        assert_eq!(chains1, 1, "the cut lands, the chain jumps once: {first:?}");
        let second = sw.strike(0.0, 0.0, 0.0, Faction::Hero, &mut bodies, 0.02);
        assert!(
            second.iter().all(|e| e.kind != HitKind::Chain),
            "one swing, one chain: {second:?}"
        );
        assert_eq!(bodies[1].hp, 24.0, "the middle mob took the one jump");
        assert_eq!(bodies[2].hp, 30.0, "the far mob is out of chain range");
    }

    #[test]
    fn burst_carries_the_effect() {
        // A burn bolt: the direct hit and both burst victims catch fire.
        let mut bodies = vec![mob(1, 0.0, 3.0), mob(2, 0.9, 3.4), mob(3, -0.8, 3.5)];
        let mut p = Projectile {
            id: 1,
            faction: Faction::Hero,
            x: 0.0,
            z: 0.0,
            y: 1.1,
            dx: 0.0,
            dz: 1.0,
            speed: 18.0,
            radius: 0.45,
            damage: 16.0,
            knockback: 3.0,
            blast: 1.2,
            effect: Effect::Burn {
                dps: 5.0,
                secs: 3.0,
            },
            range_left: 16.0,
            alive: true,
        };
        let dt = 1.0 / 60.0;
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p.step(dt, &mut bodies, step as f32 * dt, |_, _| false);
            if !out.events.is_empty() {
                break;
            }
        }
        assert_eq!(out.events.len(), 3);
        assert_eq!(out.events[0].kind, HitKind::Strike, "the touch is a strike");
        assert_eq!(out.events[1].kind, HitKind::Burst, "the rest are the burst");
        for b in &bodies {
            assert_eq!(b.status.burn_dps, 5.0, "every victim burns");
            assert_eq!(b.hp, 14.0);
        }
    }

    #[test]
    fn a_thunder_bolt_chains_past_its_blast() {
        // Direct hit, one burst victim, and a third mob well outside the
        // blast that only the chain can reach.
        let mut bodies = vec![mob(1, 0.0, 3.0), mob(2, 0.9, 3.3), mob(3, 0.0, 6.5)];
        let mut p = Projectile {
            id: 1,
            faction: Faction::Hero,
            x: 0.0,
            z: 0.0,
            y: 1.1,
            dx: 0.0,
            dz: 1.0,
            speed: 18.0,
            radius: 0.66,
            damage: 28.0,
            knockback: 5.0,
            blast: 1.6,
            effect: Effect::Chain {
                jumps: 4,
                range: 4.0,
                scale: 0.6,
            },
            range_left: 16.0,
            alive: true,
        };
        let dt = 1.0 / 60.0;
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p.step(dt, &mut bodies, step as f32 * dt, |_, _| false);
            if !out.events.is_empty() {
                break;
            }
        }
        let chains: Vec<&HitEvent> = out
            .events
            .iter()
            .filter(|e| e.kind == HitKind::Chain)
            .collect();
        assert_eq!(chains.len(), 1, "one hop past the blast: {:?}", out.events);
        assert_eq!(chains[0].target, 3);
        assert!((bodies[2].hp - (30.0 - 28.0 * 0.6)).abs() < 1e-4);
    }

    // ---- affinity / armour / i-frames (the enemy stage's additions) ----

    fn thunder_def(damage: f32) -> AttackDef {
        let mut def = AttackDef::sector(2.0, 60.0, damage);
        def.effect = Effect::Chain {
            jumps: 1,
            range: 3.0,
            scale: 0.6,
        };
        def
    }

    #[test]
    fn affinity_scales_damage_and_status_time() {
        // Thunder 1.3: a 10-damage thunder hit lands for 13.
        let mut b = mob(1, 0.0, 1.0);
        b.affinity.damage[Element::Thunder.index()] = 1.3;
        let ev = apply_hit(&mut b, &thunder_def(10.0), (0.0, 1.0), 0.0).unwrap();
        assert!((ev.damage - 13.0).abs() < 1e-4);
        assert!((b.hp - 17.0).abs() < 1e-4);
        // Burn time 0 is immunity: the ailment never lands at all.
        let mut b = mob(2, 0.0, 1.0);
        b.affinity.status[Element::Fire.index()] = 0.0;
        apply_hit(&mut b, &burn_def(4.0, 2.0), (0.0, 1.0), 0.0).unwrap();
        assert_eq!(b.status.burn_dps, 0.0, "immune bodies do not burn");
        assert!(b.tick_status(0.5).is_none());
        // Burn time 1.5 stretches the burn: 2 s authored runs 3 s.
        let mut b = mob(3, 0.0, 1.0);
        b.affinity.status[Element::Fire.index()] = 1.5;
        apply_hit(&mut b, &burn_def(4.0, 2.0), (0.0, 1.0), 0.0).unwrap();
        assert!((b.status.burn_until - 3.0).abs() < 1e-4);
        // And a fire affinity scales the ticks themselves.
        let mut b = mob(4, 0.0, 1.0);
        b.affinity.damage[Element::Fire.index()] = 1.2;
        apply_hit(&mut b, &burn_def(4.0, 2.0), (0.0, 1.0), 0.0).unwrap();
        let tick = b.tick_status(0.5).unwrap();
        assert!((tick.damage - 4.0 * BURN_TICK_SECS * 1.2).abs() < 1e-4);
    }

    #[test]
    fn affinity_default_changes_nothing() {
        let mut b = mob(1, 0.0, 1.0);
        assert_eq!(b.affinity, Affinity::IDENTITY);
        let ev = apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.0).unwrap();
        assert_eq!(ev.damage, 10.0);
        assert_eq!(b.hp, 20.0);
    }

    #[test]
    fn invuln_on_hit_gates_the_next_hit() {
        let mut b = mob(1, 0.0, 1.0);
        b.hp = 100.0;
        b.max_hp = 100.0;
        b.invuln_on_hit = 0.7;
        apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.0).unwrap();
        assert!(
            apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.69).is_none(),
            "0.69 s later the window is still shut"
        );
        assert!(
            apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.70).is_some(),
            "at 0.70 s the next hit lands"
        );
        // A body with no window (every enemy) is never gated by it.
        let mut e = mob(2, 0.0, 1.0);
        apply_hit(&mut e, &swing_def(), (0.0, 1.0), 0.0).unwrap();
        assert!(apply_hit(&mut e, &swing_def(), (0.0, 1.0), 0.01).is_some());
    }

    #[test]
    fn armor_takes_damage_but_never_flinches() {
        let mut b = mob(1, 0.0, 1.0);
        b.hp = 100.0;
        b.max_hp = 100.0;
        b.poise = 25.0;
        b.poise_max = 25.0;
        b.armor_until = 1.0;
        let ev = apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.5).unwrap();
        assert_eq!(b.hp, 90.0, "the damage lands");
        assert_eq!(b.poise, 15.0, "and the poise loss lands");
        assert_eq!(b.state, BodyState::Active, "but the body never flinches");
        assert!(!ev.fly);
    }

    #[test]
    fn a_break_under_armor_waits_for_the_armor_to_end() {
        let mut b = mob(1, 0.0, 1.0);
        b.hp = 200.0;
        b.max_hp = 200.0;
        b.poise = 5.0;
        b.poise_max = 25.0;
        b.down_secs = 1.2;
        b.armor_until = 1.0;
        apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.5).unwrap();
        assert_eq!(b.state, BodyState::Active, "no break under armour");
        assert!(b.poise <= 0.0, "the debt is recorded");
        b.settle_armor(0.9);
        assert_eq!(b.state, BodyState::Active, "armour still up at 0.9");
        b.settle_armor(1.0);
        assert_eq!(b.state, BodyState::Launched, "the break lands at 1.0");
        assert!((b.state_t - 1.2).abs() < 1e-4, "for the body's down_secs");
        assert_eq!(b.poise, 25.0, "poise refills as the break starts");
    }

    #[test]
    fn down_secs_sets_the_break_length() {
        let mut b = mob(1, 0.0, 1.0);
        b.hp = 200.0;
        b.max_hp = 200.0;
        b.poise = 5.0;
        b.poise_max = 25.0;
        b.down_secs = 0.9;
        let ev = apply_hit(&mut b, &swing_def(), (0.0, 1.0), 0.0).unwrap();
        assert!(ev.fly);
        assert_eq!(b.state, BodyState::Launched);
        assert!((b.state_t - 0.9).abs() < 1e-4);
        // A launching blow on a poise-less body uses it too.
        let mut m = mob(2, 0.0, 1.0);
        m.down_secs = 0.8;
        let mut fly_def = swing_def();
        fly_def.fly = true;
        apply_hit(&mut m, &fly_def, (0.0, 1.0), 0.0).unwrap();
        assert!((m.state_t - 0.8).abs() < 1e-4);
    }

    // ---- telegraphs ----

    fn grab_def() -> TelegraphDef {
        TelegraphDef {
            windup: 0.30,
            active: 0.10,
            recover: 0.50,
            def: AttackDef::sector(0.8, 60.0, 6.0),
            armor: false,
        }
    }

    #[test]
    fn telegraph_runs_windup_active_recover() {
        let mut tg = Telegraph::new(1, grab_def());
        let dt = 1.0 / 60.0;
        let mut hero = vec![Body::new(0, Faction::Hero, 0.0, 0.5, 100.0, 0.38)];
        // Windup: steps pass, nothing can land, progress climbs.
        for _ in 0..17 {
            assert_eq!(tg.step(dt), None);
            assert!(tg
                .strike(0.0, 0.0, 0.0, Faction::Enemy, &mut hero, 0.0)
                .is_empty());
        }
        assert!(tg.progress() > 0.9);
        assert_eq!(tg.step(dt), Some(TelegraphPhase::Active));
        assert_eq!(tg.progress(), 1.0);
        // Active: the blow lands.
        let evs = tg.strike(0.0, 0.0, 0.0, Faction::Enemy, &mut hero, 0.0);
        assert_eq!(evs.len(), 1);
        assert_eq!(hero[0].hp, 94.0);
        // Recover after the active stretch, then Done.
        let mut seen = Vec::new();
        for _ in 0..40 {
            if let Some(p) = tg.step(dt) {
                seen.push(p);
            }
        }
        assert_eq!(seen, vec![TelegraphPhase::Recover, TelegraphPhase::Done]);
        assert_eq!(tg.step(dt), None, "done is terminal");
        // A cancelled telegraph never lands.
        let mut tg = Telegraph::new(2, grab_def());
        tg.cancel();
        assert_eq!(tg.phase, TelegraphPhase::Done);
    }

    #[test]
    fn telegraph_hits_once_while_moving() {
        // A charge: the origin moves every step through the active
        // stretch, the victim is struck exactly once.
        let mut def = grab_def();
        def.active = 0.30;
        let mut tg = Telegraph::new(1, def);
        let dt = 1.0 / 60.0;
        while tg.phase == TelegraphPhase::Windup {
            tg.step(dt);
        }
        let mut hero = vec![Body::new(0, Faction::Hero, 0.0, 1.2, 100.0, 0.38)];
        let mut hits = 0;
        let mut z = 0.2;
        while tg.phase == TelegraphPhase::Active {
            z += 3.0 * dt;
            hits += tg.strike(0.0, z, 0.0, Faction::Enemy, &mut hero, 0.0).len();
            tg.step(dt);
        }
        assert_eq!(hits, 1);
        assert_eq!(hero[0].hp, 94.0);
    }

    #[test]
    fn a_growing_ring_hits_once() {
        // A shockwave: the swing's radius grows between steps; the
        // ring still lands at most once per body.
        let mut def = AttackDef::sector(0.5, 360.0, 10.0);
        def.half_angle = std::f32::consts::PI;
        let mut sw = Swing::new(7, def);
        let mut bodies = vec![mob(1, 3.0, 0.0), mob(2, 6.0, 0.0)];
        let mut hits = 0;
        for step in 1..=8 {
            sw.def.radius = 0.5 + step as f32;
            hits += sw
                .strike(0.0, 0.0, 0.0, Faction::Hero, &mut bodies, 0.0)
                .len();
        }
        assert_eq!(hits, 2, "each body once, as the ring reaches it");
        assert_eq!(bodies[0].hp, 20.0);
        assert_eq!(bodies[1].hp, 20.0);
    }

    #[test]
    fn slowed_telegraph_takes_longer() {
        // The kit passes dt × speed_scale: at 0.4 the windup's real
        // duration is 1 / 0.4 times the authored one.
        let mut tg = Telegraph::new(1, grab_def());
        let dt = 1.0 / 60.0;
        let mut steps: usize = 0;
        while tg.phase == TelegraphPhase::Windup {
            tg.step(dt * 0.4);
            steps += 1;
        }
        let want = (0.30 / (dt * 0.4)).ceil() as usize;
        assert!(
            steps.abs_diff(want) <= 1,
            "windup took {steps} slowed steps, want ~{want}"
        );
    }

    #[test]
    fn enemy_projectiles_hit_only_the_hero() {
        // An enemy bolt flies through its own side and lands on the hero.
        let mut bodies = vec![
            Body::new(0, Faction::Hero, 0.0, 3.0, 100.0, 0.38),
            mob(1, 0.0, 1.0),
            mob(2, 0.0, 2.0),
        ];
        let mut p = Projectile {
            id: 9,
            faction: Faction::Enemy,
            x: 0.0,
            z: 0.0,
            y: 1.0,
            dx: 0.0,
            dz: 1.0,
            speed: 11.0,
            radius: 0.45,
            damage: 14.0,
            knockback: 6.0,
            blast: 0.0,
            effect: Effect::None,
            range_left: 14.0,
            alive: true,
        };
        let dt = 1.0 / 60.0;
        let mut out = BoltOutcome::default();
        for step in 0..60 {
            out = p.step(dt, &mut bodies, step as f32 * dt, |_, _| false);
            if !out.events.is_empty() {
                break;
            }
        }
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].target, 0, "only the hero is hit");
        assert_eq!(bodies[0].hp, 86.0);
        assert_eq!(bodies[1].hp, 30.0, "its own side is untouched");
        assert_eq!(bodies[2].hp, 30.0);
    }

    // ---- the combo tree and its chain machine ----

    /// A three-stage tree with a branch at every stage, payloads
    /// standing in for a kit's move ids.
    const TREE: ComboTree<u8> = ComboTree::new(&[Some(10), Some(20), Some(30)]);

    #[test]
    fn a_tree_declares_one_optional_branch_per_stage() {
        assert_eq!(TREE.len(), 3);
        assert!(!TREE.is_empty());
        assert_eq!(TREE.branch(0), Some(10));
        assert_eq!(TREE.branch(2), Some(30));
        assert_eq!(TREE.branch(3), None, "past the table there is nothing");
        assert!(TREE.has_branch(1));
        let sparse: ComboTree<u8> = ComboTree::new(&[None, Some(7)]);
        assert!(!sparse.has_branch(0));
        assert_eq!(sparse.branch(1), Some(7));
    }

    #[test]
    fn the_window_opens_once_when_the_blow_lands() {
        let mut c = ComboChain::new();
        assert!(!c.active());
        assert!(!c.open_window(true), "no combo, no window");
        c.begin(0);
        assert!(c.active());
        assert!(!c.open_window(false), "the blow has not landed yet");
        assert!(!c.window_open());
        assert!(c.open_window(true), "the opening step reports itself");
        assert!(c.window_open());
        assert!(!c.open_window(true), "and only that step");
    }

    #[test]
    fn a_reservation_only_counts_inside_the_window() {
        let mut c = ComboChain::new();
        c.begin(0);
        c.queue_next();
        assert!(
            !c.queued(),
            "before the window a press is not a reservation"
        );
        c.open_window(true);
        c.queue_next();
        assert!(c.queued());
        assert_eq!(c.advance(3), ComboAdvance::Next(1));
        assert!(!c.queued(), "the reservation was consumed");
    }

    #[test]
    fn a_tagged_reservation_carries_its_tag_to_the_stage_end() {
        let mut c = ComboChain::new();
        c.begin(0);
        c.queue_next_tag(1);
        assert!(!c.queued(), "before the window a tag does not reserve");
        c.open_window(true);
        c.queue_next();
        assert_eq!(c.queued_tag(), 0, "the plain reservation is tag 0");
        c.queue_next_tag(1);
        assert!(c.queued());
        assert_eq!(c.queued_tag(), 1, "the last tag queued stands");
        assert_eq!(c.advance(7), ComboAdvance::Next(1));
        assert_eq!(c.queued_tag(), 0, "advancing consumed the tag");
        c.begin(3);
        assert_eq!(c.queued_tag(), 0, "a fresh stage starts untagged");
    }

    #[test]
    fn a_branch_chosen_mid_chain_fires_in_the_next_slot_and_outranks() {
        let mut c = ComboChain::new();
        c.begin(0);
        c.open_window(true);
        c.queue_next();
        assert_eq!(
            c.choose_branch(true, false),
            BranchTake::Armed,
            "stage 1 has a branch and is not the last"
        );
        assert_eq!(c.branch(), Some(0));
        // The branch outranks the reservation made in the same window.
        assert_eq!(c.advance(3), ComboAdvance::Branch(0));
        assert_eq!(c.branch(), None, "the choice was consumed");
    }

    #[test]
    fn a_branch_in_the_last_stage_converts_on_the_spot() {
        let mut c = ComboChain::new();
        c.begin(2);
        c.open_window(true);
        assert_eq!(c.choose_branch(true, true), BranchTake::ConvertNow);
        assert_eq!(c.branch(), None, "nothing is armed: it fires now");
    }

    #[test]
    fn a_stage_without_a_branch_refuses_the_press() {
        let mut c = ComboChain::new();
        c.begin(1);
        assert_eq!(
            c.choose_branch(true, false),
            BranchTake::NoBranch,
            "the window is shut"
        );
        c.open_window(true);
        assert_eq!(c.choose_branch(false, false), BranchTake::NoBranch);
        assert_eq!(c.branch(), None);
        // With nothing chosen and nothing reserved, the combo ends.
        assert_eq!(c.advance(3), ComboAdvance::End);
    }

    #[test]
    fn an_unreserved_last_stage_ends_and_end_clears_everything() {
        let mut c = ComboChain::new();
        c.begin(2);
        c.open_window(true);
        c.queue_next();
        assert_eq!(
            c.advance(3),
            ComboAdvance::End,
            "there is no next stage to reserve"
        );
        c.begin(1);
        c.open_window(true);
        c.choose_branch(true, false);
        c.end();
        assert!(!c.active());
        assert!(!c.window_open());
        assert!(!c.queued());
        assert_eq!(c.branch(), None);
        assert_eq!(
            c.advance(3),
            ComboAdvance::End,
            "an ended chain stays ended"
        );
    }
}

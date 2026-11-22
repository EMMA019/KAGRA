//! The companion follower core (meta round M2, META-DESIGN §2.1):
//! one ally who walks with the hero. The part owns ONLY the
//! following — where the companion stands, how fast she closes the
//! gap, when a cast roots her feet, and the aura question. Role
//! behaviour (what the cast DOES, when it may fire, what it costs)
//! is kit data declared on top, the way `mission_cycle` leaves the
//! reward offer to the kit.
//!
//! The companion is not a combat body: no hitbox, no health, no
//! aggro target, no collision — nothing in the combat sim can see
//! her, and she can never die, block, or be blocked. Her position
//! is presentation state the kit mirrors into a walker each frame.
//!
//! The numbers are the design's shared shape, one set for every
//! role: trail 1.7 m behind the hero's facing, at a side offset so
//! she walks beside his line rather than hiding in it, cruise at
//! the hero's pace ×1.05, catch up at ×1.5 once the anchor is more
//! than 6 m away, and ease to a stop over the last stretch — her
//! pace decays in proportion to the remaining distance all the
//! way in, so she settles like a released stick instead of
//! pulsing against a dead zone.
//!
//! She is DRIVEN the hero's way (Saka's direction, 2026-11-19 —
//! his read of her gliding-mannequin follow was that she did not
//! move like him): each step forms a desired velocity toward the
//! anchor — the follower's stick — and integrates it exactly the
//! way the hero integrates his input, eased over the last stretch
//! the way a feathered stick eases him. Her reported speed is her
//! real displacement over the step, the number the hero's own
//! animator is fed, so a kit running her through the same
//! `Animator` keeps her clip phase in step with the ground she
//! actually covered. Her heading follows her velocity while she
//! moves and holds when she stops, again like his facing.

/// The shared follower shape (META-DESIGN §2.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompanionSpec {
    /// How far behind the hero (along his facing) the anchor sits.
    pub anchor_dist: f32,
    /// The anchor's side offset (radians): the rear anchor is
    /// rotated this far toward the hero's left shoulder, so she
    /// trails beside his line instead of hiding in it.
    pub anchor_side: f32,
    /// Cruising pace as a multiple of the hero's current pace.
    pub walk_mult: f32,
    /// Catch-up pace as a multiple of the hero's current pace.
    pub catchup_mult: f32,
    /// Anchor distance past which the catch-up pace applies.
    pub catchup_dist: f32,
    /// The rest radius (m): inside this distance of the anchor
    /// she is home and stands still. Deliberately tiny — the
    /// arrive ramp has already decayed her pace to a creep
    /// (≈0.1 m/s) by the time she crosses it, so the stop is the
    /// smooth tail of a continuous decay. A wide hard dead zone
    /// over a full-pace approach is what read as stop-motion
    /// (Saka's device read, 2026-11-19): burst, snap-stop, burst.
    pub dead_zone: f32,
    /// The arrive ramp (m): inside this distance of the anchor
    /// the desired speed scales down in proportion to the
    /// remaining distance — the follower's feathered stick, so a
    /// half-metre adjustment is a stroll at a stroll's cadence,
    /// not a full-pace burst with a hard stop.
    pub arrive: f32,
}

/// The design's follower numbers.
pub const COMPANION_SPEC: CompanionSpec = CompanionSpec {
    anchor_dist: 1.7,
    anchor_side: 0.6109, // 35° toward the hero's left shoulder
    walk_mult: 1.05,
    catchup_mult: 1.5,
    catchup_dist: 6.0,
    dead_zone: 0.05,
    arrive: 1.5,
};

/// One companion's runtime: position, facing, the pace she is
/// actually making (the kit feeds it to her animator's locomotion),
/// the cast lock that roots her, and the ability clock the kit's
/// role logic reads and re-arms.
#[derive(Clone, Debug)]
pub struct Companion {
    pub spec: CompanionSpec,
    pub x: f32,
    pub z: f32,
    /// Facing (radians, the world convention: forward is
    /// `(sin, cos)`).
    pub facing: f32,
    /// Her real ground speed over the last step: the distance she
    /// actually covered divided by `dt` (0 at rest) — the same
    /// displacement measure the hero's animator is fed, so the kit
    /// can hand it straight to hers.
    pub speed: f32,
    /// Seconds left of the cast in flight; while it runs she does
    /// not move. The kit applies the cast's effect on the step this
    /// reaches zero (see [`Companion::step`]'s return).
    pub casting: f32,
    /// The ability clock: seconds until the role may act again.
    /// Counts down always (casting or not); the kit re-arms it when
    /// a cast starts.
    pub timer: f32,
}

impl Default for Companion {
    fn default() -> Self {
        Self::new()
    }
}

impl Companion {
    pub fn new() -> Self {
        Self::with_spec(COMPANION_SPEC)
    }

    pub fn with_spec(spec: CompanionSpec) -> Self {
        Self {
            spec,
            x: 0.0,
            z: 0.0,
            facing: 0.0,
            speed: 0.0,
            casting: 0.0,
            timer: 0.0,
        }
    }

    /// The anchor point for a hero at `hero` facing `hero_facing`:
    /// `anchor_dist` behind him along his facing, rotated
    /// `anchor_side` toward his left shoulder.
    pub fn anchor(&self, hero: (f32, f32), hero_facing: f32) -> (f32, f32) {
        let a = hero_facing + std::f32::consts::PI + self.spec.anchor_side;
        (
            hero.0 + a.sin() * self.spec.anchor_dist,
            hero.1 + a.cos() * self.spec.anchor_dist,
        )
    }

    /// Put her on the anchor (a run's start, a stage change): no
    /// walk-in from wherever she last stood.
    pub fn place(&mut self, hero: (f32, f32), hero_facing: f32) {
        let (ax, az) = self.anchor(hero, hero_facing);
        self.x = ax;
        self.z = az;
        self.facing = hero_facing;
        self.speed = 0.0;
        self.casting = 0.0;
        self.timer = 0.0;
    }

    /// Begin a cast: her feet root for `cast_secs`, and the ability
    /// clock re-arms to `cooldown` (measured from the cast's start,
    /// so a role's cycle is start-to-start).
    pub fn start_cast(&mut self, cast_secs: f32, cooldown: f32) {
        self.casting = cast_secs.max(0.0);
        self.timer = cooldown.max(0.0);
    }

    /// Is a cast in flight?
    pub fn is_casting(&self) -> bool {
        self.casting > 0.0
    }

    /// May the role act? (The clock has run out and no cast is in
    /// flight — the kit still applies its own conditions: combat
    /// window, target in range, hero hurt.)
    pub fn ready(&self) -> bool {
        self.timer <= 0.0 && self.casting <= 0.0
    }

    /// The aura question (the buffer's whole mechanic): is the hero
    /// within `radius` of her?
    pub fn hero_in_aura(&self, hero: (f32, f32), radius: f32) -> bool {
        (hero.0 - self.x).hypot(hero.1 - self.z) <= radius
    }

    /// Advance one step, the hero's way: form the desired
    /// velocity — toward the anchor at the follow pace, eased
    /// over the arrive ramp — integrate it for `dt` (never past
    /// the anchor), take her heading from the velocity, and
    /// report the speed she actually made good. `hero_speed` is
    /// the hero's current full pace (his base speed with every
    /// multiplier the kit applies — the follower's paces are
    /// multiples of it, so she matches a buffed hero and dawdles
    /// with a slowed one). Returns `true` on exactly the step a
    /// cast completes (its countdown crosses zero): the kit
    /// lands the cast's effect then.
    pub fn step(&mut self, dt: f32, hero: (f32, f32), hero_facing: f32, hero_speed: f32) -> bool {
        if dt <= 0.0 {
            return false;
        }
        self.timer = (self.timer - dt).max(0.0);
        let mut cast_done = false;
        if self.casting > 0.0 {
            self.casting -= dt;
            self.speed = 0.0;
            // A cast faces where the hero faces: she casts at his
            // fight, not at her anchor.
            self.facing = hero_facing;
            if self.casting <= 0.0 {
                self.casting = 0.0;
                cast_done = true;
            }
            return cast_done;
        }
        let (ax, az) = self.anchor(hero, hero_facing);
        let (dx, dz) = (ax - self.x, az - self.z);
        let d = dx.hypot(dz);
        if d <= self.spec.dead_zone {
            self.speed = 0.0;
            // Home. The arrive ramp has decayed her pace to a
            // creep by this radius, so this stop is the tail of a
            // continuous decay — never a step down from pace (the
            // stop-motion Saka called out). At rest she also keeps
            // her own heading: the hero keeps his when the stick
            // centres, so she does not snap to his facing every
            // time she stops.
            return false;
        }
        let mult = if d > self.spec.catchup_dist {
            self.spec.catchup_mult
        } else {
            self.spec.walk_mult
        };
        // The desired velocity: the follow pace, eased over the
        // arrive ramp like a feathered stick (full pace at the
        // ramp's edge, a stroll inside it).
        let pace = (hero_speed * mult).max(0.0) * (d / self.spec.arrive.max(f32::EPSILON)).min(1.0);
        let (vx, vz) = (dx / d * pace, dz / d * pace);
        // Integrate like the hero: velocity × dt, clamped so she
        // lands on the anchor rather than stepping past it.
        let step_len = (pace * dt).min(d);
        let (ox, oz) = (self.x, self.z);
        self.x += dx / d * step_len;
        self.z += dz / d * step_len;
        if step_len > 0.0 {
            // Her heading is her velocity's, as his is his input's.
            self.facing = vx.atan2(vz);
        }
        // The speed is the real displacement over the step — the
        // measure the hero's animator is fed (his disp / dt) — so
        // an eased or anchor-clamped step reports the pace her
        // clips should play at, not the pace she was commanded.
        self.speed = (self.x - ox).hypot(self.z - oz) / dt;
        cast_done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn anchor_sits_behind_the_heros_left_shoulder() {
        let c = Companion::new();
        // Facing +Z (facing 0): the anchor is 1.7 m away, behind
        // him (−Z) and off to his left (−X at this facing).
        let (ax, az) = c.anchor((3.0, 4.0), 0.0);
        let d = (ax - 3.0).hypot(az - 4.0);
        assert!((d - 1.7).abs() < 1e-5, "anchor distance {d}");
        assert!(az < 4.0, "behind him: az {az}");
        assert!(ax < 3.0, "off his left shoulder: ax {ax}");
        // Facing PI (toward −Z): the offset rotates with him —
        // still 1.7 m, still behind, now toward +X.
        let (ax, az) = c.anchor((3.0, 4.0), std::f32::consts::PI);
        let d = (ax - 3.0).hypot(az - 4.0);
        assert!((d - 1.7).abs() < 1e-5, "anchor distance {d}");
        assert!(az > 4.0, "behind him: az {az}");
        assert!(ax > 3.0, "off his left shoulder: ax {ax}");
    }

    #[test]
    fn she_converges_on_the_anchor_and_rests_in_the_dead_zone() {
        let mut c = Companion::new();
        c.x = 10.0;
        c.z = 10.0;
        let hero = (0.0, 0.0);
        for _ in 0..600 {
            c.step(DT, hero, 0.0, 3.0);
        }
        let (ax, az) = c.anchor(hero, 0.0);
        let d = (ax - c.x).hypot(az - c.z);
        assert!(d <= c.spec.dead_zone, "resting distance {d}");
        assert_eq!(c.speed, 0.0, "at rest in the dead zone");
        // And she stays: more steps do not drift her out of it.
        for _ in 0..60 {
            c.step(DT, hero, 0.0, 3.0);
        }
        let d2 = (ax - c.x).hypot(az - c.z);
        assert!(d2 <= c.spec.dead_zone);
    }

    #[test]
    fn cruise_and_catchup_paces_are_the_spec_multiples() {
        let mut c = Companion::new();
        let (ax, az) = c.anchor((0.0, 0.0), 0.0);
        // 3 m from the anchor (outside the arrive ramp): cruise
        // at hero pace x1.05.
        c.x = ax;
        c.z = az - 3.0;
        c.step(0.1, (0.0, 0.0), 0.0, 4.0);
        assert!((c.speed - 4.2).abs() < 1e-4, "cruise {}", c.speed);
        // 10 m from the anchor: catch-up at x1.5.
        c.x = ax;
        c.z = az - 10.0;
        c.step(0.1, (0.0, 0.0), 0.0, 4.0);
        assert!((c.speed - 6.0).abs() < 1e-4, "catch-up {}", c.speed);
    }

    #[test]
    fn the_last_stretch_eases_like_a_feathered_stick() {
        let mut c = Companion::new();
        let (ax, az) = c.anchor((0.0, 0.0), 0.0);
        // Halfway down the arrive ramp (0.75 m out): half the
        // cruise pace, not a full-pace burst.
        c.x = ax;
        c.z = az - 0.75;
        c.step(0.1, (0.0, 0.0), 0.0, 4.0);
        let want = 4.0 * 1.05 * 0.5;
        assert!((c.speed - want).abs() < 1e-4, "eased {}", c.speed);
    }

    #[test]
    fn her_velocity_decays_continuously_into_rest() {
        // The stop-motion guard (Saka's device read, 2026-11-19):
        // approaching a still hero from inside the arrive ramp,
        // her speed must decay step by step to nothing — no
        // square wave, no snap from pace to zero at a dead-zone
        // edge.
        let mut c = Companion::new();
        let (ax, az) = c.anchor((0.0, 0.0), 0.0);
        c.x = ax;
        c.z = az - 1.2; // inside the 1.5 m ramp from the first step
        let dt = 1.0 / 60.0;
        let mut speeds = Vec::new();
        for _ in 0..240 {
            c.step(dt, (0.0, 0.0), 0.0, 4.0);
            speeds.push(c.speed);
        }
        assert_eq!(*speeds.last().unwrap(), 0.0, "she comes to rest");
        // She passes through the walk band on the way down — the
        // ease crosses gaits instead of cutting across them.
        assert!(
            speeds.iter().any(|&s| s > 0.3 && s < 3.0),
            "she eases through the walk band: {speeds:?}"
        );
        for w in speeds.windows(2) {
            let (prev, next) = (w[0], w[1]);
            if prev > 0.2 {
                assert!(next >= prev * 0.85, "a step in the decay: {prev} -> {next}");
            }
            if next == 0.0 && prev > 0.0 {
                assert!(prev < 0.2, "the stop itself is a creep: {prev} -> 0");
            }
        }
    }

    #[test]
    fn the_reported_speed_is_the_real_displacement() {
        let mut c = Companion::new();
        let (ax, az) = c.anchor((0.0, 0.0), 0.0);
        // A long step whose commanded travel overshoots the
        // anchor: she lands ON it, and the reported speed is the
        // distance she actually covered over dt — not the pace.
        c.x = ax;
        c.z = az - 0.5;
        c.step(1.0, (0.0, 0.0), 0.0, 4.0);
        assert!(
            (c.x - ax).abs() < 1e-6 && (c.z - az).abs() < 1e-6,
            "landed on the anchor"
        );
        assert!(
            (c.speed - 0.5).abs() < 1e-6,
            "displacement speed {}",
            c.speed
        );
    }

    #[test]
    fn her_heading_follows_her_velocity_and_holds_at_rest() {
        let mut c = Companion::new();
        let (ax, az) = c.anchor((0.0, 0.0), 0.0);
        // Due "north" of the anchor (+Z of it): she travels −Z,
        // and her heading is that travel's: PI in the convention.
        c.x = ax;
        c.z = az + 4.0;
        c.step(0.1, (0.0, 0.0), 0.0, 4.0);
        assert!(
            (c.facing - std::f32::consts::PI).abs() < 1e-5,
            "heading {}",
            c.facing
        );
        // Settled inside the dead zone, she keeps that heading:
        // the step does not overwrite her facing with the hero's
        // (the hero keeps his own heading at rest too).
        let (ax, az) = c.anchor((0.0, 0.0), -2.0);
        c.x = ax;
        c.z = az;
        c.step(0.1, (0.0, 0.0), -2.0, 4.0);
        assert_eq!(c.speed, 0.0);
        assert!(
            (c.facing - std::f32::consts::PI).abs() < 1e-5,
            "heading held {}",
            c.facing
        );
    }

    #[test]
    fn a_cast_roots_her_and_completes_once() {
        let mut c = Companion::new();
        c.x = 5.0;
        c.z = 5.0;
        c.start_cast(0.9, 10.0);
        assert!(c.is_casting());
        assert!(!c.ready());
        let mut completions = 0;
        for _ in 0..120 {
            let was_casting = c.is_casting();
            if c.step(DT, (0.0, 0.0), 0.0, 4.0) {
                completions += 1;
            }
            if was_casting {
                assert_eq!((c.x, c.z), (5.0, 5.0), "rooted while casting");
            }
        }
        assert_eq!(completions, 1, "the cast lands exactly once");
        assert!(!c.is_casting());
        // The clock re-armed at the cast's start: 10 s from then,
        // not from completion.
        assert!(c.timer > 7.9 && c.timer < 8.2, "timer {}", c.timer);
        for _ in 0..600 {
            c.step(DT, (0.0, 0.0), 0.0, 4.0);
        }
        assert!(c.ready());
    }

    #[test]
    fn aura_boundary_is_the_radius() {
        let mut c = Companion::new();
        c.x = 0.0;
        c.z = 0.0;
        assert!(c.hero_in_aura((5.0, 0.0), 5.0));
        assert!(c.hero_in_aura((3.0, 4.0), 5.0));
        assert!(!c.hero_in_aura((5.01, 0.0), 5.0));
    }
}

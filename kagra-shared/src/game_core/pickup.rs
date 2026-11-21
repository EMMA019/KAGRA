//! Pickups: items lying in the world that the hero collects by walking
//! over them — placed fixtures (a heal on a doorstep, a rune at the end of
//! an alley) and combat drops (a heal orb a mob left behind). Kits own the
//! item *kinds* and the effects; this part owns the shared bookkeeping:
//! spawn order and caps, magnet drift, lifetimes with a warning blink, and
//! the walk-over collection test.
//!
//! Collection is **edge-triggered**: an item is taken on the step the hero
//! walks *into* its reach, never on a later step he spends standing on
//! it. A level-triggered test re-fires under his feet — a swapped-out
//! sword whose rest had lifted was re-taken on the spot, swapping back
//! and forth forever (Saka device check, 2026-11-14). Two softenings
//! keep the old feel where it mattered: an item the kit refuses (a heal
//! at full HP) keeps its edge and is taken on the first step it is
//! wanted, even underfoot; and an item still resting (`no_pick_until`)
//! spends its edge, so the rest lifting under his feet fires nothing —
//! he steps out and back in to take it.
//!
//! The edge alone did not survive contact with combat: footwork and
//! knockback cross a drop's reach over and over without ever leaving
//! the fight, and every honest re-entry re-took a swapped-out sword —
//! refreshing its lifetime and re-dressing the hero's model each time,
//! so the sword at his feet never disappeared and every swap hitched
//! (Saka device check, 2026-11-15, the recurrence). A drop can
//! therefore carry a **re-arm distance** (`arm_radius`): it cannot be
//! taken until the hero has dwelt that far away from it, steering
//! under his own power, for a short spell. Combat crossings stay
//! inside the distance and never re-arm it; a knockback's brief
//! excursion does not dwell; and a crowd bulldozing a passive hero
//! past the line accrues nothing, because none of it is his own
//! movement. A deliberate walk clear of the fight and back re-arms
//! it, and the return entry takes it exactly once.
//!
//! The magnet numbers match the survivor kits (`world_survivor` pulls gems
//! and heals from 3.0 m at 6.5 m/s): a drop slides to the hero once he is
//! close, but a *placed* item (magnet 0) is a destination, not a reward —
//! you walk to the doorstep to take the snack.

/// How fast a magnetised drop slides toward the hero, metres per second.
/// Same value as `world_survivor`'s drop magnet (city_action design §3.6:
/// align with the survivor's shipped numbers, not the draft's guess).
pub const MAGNET_SPEED: f32 = 6.5;

/// A drop with less lifetime than this left is expiring and should blink.
pub const BLINK_SECS: f32 = 3.0;

/// How long the hero must stay beyond an item's re-arm distance,
/// steering under his own power, before the item arms (seconds). A
/// dwell, not an instant, and only his own movement counts: a
/// knockback's brief throw past the distance — or a crowd bulldozing
/// a passive hero — cannot arm a drop he never actually left.
const ARM_DWELL_SECS: f32 = 0.75;

/// One item in the world.
#[derive(Clone, Debug)]
pub struct Pickup<K: Copy> {
    /// Spawn-order id; rendering phases off it so neighbours don't bob in
    /// sync.
    pub id: u64,
    pub kind: K,
    pub x: f32,
    pub z: f32,
    /// Collection radius of the item itself (the hero's radius is added).
    pub radius: f32,
    /// Pull radius in metres: a drop within this distance of the hero
    /// slides toward him. 0 = never pulled (placed items).
    pub magnet: f32,
    /// Seconds of life left. `None` = placed: stays until taken.
    pub ttl: Option<f32>,
    pub age: f32,
    /// Run-clock time before which the item cannot be taken — a swapped-out
    /// weapon rests a beat so the swap cannot ping-pong.
    pub no_pick_until: f32,
    /// Whether the hero was inside the collection reach at the last
    /// step's test. Collection fires on the outside→inside edge only
    /// (see the module docs); this is the edge's memory.
    pub was_inside: bool,
    /// Re-arm distance in metres (0 = no gate): the item cannot be
    /// taken until the hero has once been this far away from it.
    pub arm_radius: f32,
    /// Whether a gated item has been armed by distance yet. Ungated
    /// items are born armed.
    pub armed: bool,
    /// Seconds the hero has *continuously* spent beyond the re-arm
    /// distance while moving under his own power. Arming needs that
    /// dwell, not an instant out there: a knockback can throw the
    /// hero past the distance for a few frames, and a grappling crowd
    /// can bulldoze a passive hero clear across the street — neither
    /// is him leaving the fight, so neither may arm the drop.
    pub arm_for: f32,
    /// Index into the kit's placed-spot table, for spots that respawn.
    pub spot: Option<u16>,
}

/// The world's live pickups, in spawn order.
#[derive(Clone, Debug)]
pub struct Pickups<K: Copy> {
    pub items: Vec<Pickup<K>>,
    next_id: u64,
    /// Maximum number of *dropped* (ttl-bearing) items alive at once;
    /// spawning past it evicts the oldest drop. Placed items never count
    /// toward the cap and are never evicted.
    pub cap: usize,
}

impl<K: Copy> Default for Pickups<K> {
    fn default() -> Self {
        Pickups {
            items: Vec::new(),
            next_id: 1,
            cap: usize::MAX,
        }
    }
}

impl<K: Copy> Pickups<K> {
    pub fn new(cap: usize) -> Self {
        Pickups {
            cap,
            ..Pickups::default()
        }
    }

    /// Put an item in the world and return its id. A dropped item (ttl
    /// `Some`) that would exceed the drop cap evicts the oldest drop first.
    /// Spawn's argument list is the item's whole description; a
    /// parameter struct would just move the same names one hop away.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        &mut self,
        kind: K,
        x: f32,
        z: f32,
        radius: f32,
        magnet: f32,
        ttl: Option<f32>,
        spot: Option<u16>,
    ) -> u64 {
        if ttl.is_some() {
            while self.items.iter().filter(|p| p.ttl.is_some()).count() >= self.cap {
                let Some(oldest) = self.items.iter().position(|p| p.ttl.is_some()) else {
                    break;
                };
                self.items.remove(oldest);
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Pickup {
            id,
            kind,
            x,
            z,
            radius,
            magnet,
            ttl,
            age: 0.0,
            no_pick_until: 0.0,
            was_inside: false,
            arm_radius: 0.0,
            armed: true,
            arm_for: 0.0,
            spot,
        });
        id
    }

    /// Seconds-remaining lookup for the just-spawned item's no-pick rest:
    /// kits set it right after spawn (a swap drop).
    pub fn set_no_pick_until(&mut self, id: u64, until: f32) {
        if let Some(p) = self.items.iter_mut().find(|p| p.id == id) {
            p.no_pick_until = until;
        }
    }

    /// Gate the just-spawned item behind a re-arm distance (see the
    /// module docs): it starts unarmed — the hero is standing on it —
    /// and arms on the first step he is beyond `radius` from it.
    pub fn set_arm_radius(&mut self, id: u64, radius: f32) {
        if let Some(p) = self.items.iter_mut().find(|p| p.id == id) {
            p.arm_radius = radius;
            p.armed = radius <= 0.0;
            p.arm_for = 0.0;
        }
    }

    /// Whether a placed spot currently has its item in the world.
    pub fn spot_present(&self, spot: u16) -> bool {
        self.items.iter().any(|p| p.spot == Some(spot))
    }

    /// An expiring drop in its last seconds blinks out of existence.
    pub fn blinking(p: &Pickup<K>) -> bool {
        matches!(p.ttl, Some(left) if left < BLINK_SECS)
    }

    /// Advance everything one step: age, expire, drift magnetised drops
    /// toward the hero, and collect what he walks *into* (and `accept`s —
    /// a full-HP hero walks past a heal and leaves it for later).
    /// `hero_active` is whether the hero is moving under his own power
    /// this step (his steering is deflected): a gated drop's re-arm
    /// dwell only counts while it is true, so a crowd bulldozing a
    /// passive hero around the map cannot arm his drop for him.
    /// Returns the collected items, in spawn order.
    pub fn step(
        &mut self,
        dt: f32,
        now: f32,
        hero: (f32, f32),
        hero_r: f32,
        hero_active: bool,
        accept: impl Fn(K) -> bool,
    ) -> Vec<Pickup<K>> {
        let mut taken = Vec::new();
        let mut i = 0;
        while i < self.items.len() {
            let expired = {
                let p = &mut self.items[i];
                p.age += dt;
                if let Some(ttl) = &mut p.ttl {
                    *ttl -= dt;
                    *ttl <= 0.0
                } else {
                    false
                }
            };
            if expired {
                self.items.remove(i);
                continue;
            }
            let p = &mut self.items[i];
            let dx = hero.0 - p.x;
            let dz = hero.1 - p.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let reach = hero_r + p.radius;
            if p.magnet > 0.0 && dist > reach && dist <= p.magnet {
                let step = (MAGNET_SPEED * dt).min(dist - reach);
                p.x += dx / dist * step;
                p.z += dz / dist * step;
            }
            let dist = ((hero.0 - p.x) * (hero.0 - p.x) + (hero.1 - p.z) * (hero.1 - p.z)).sqrt();
            // A gated drop arms once the hero has dwelt clear of it
            // *under his own power* (module docs): combat crossings
            // inside the arm radius never count, a knockback's brief
            // excursion does not dwell, and a crowd bulldozing a
            // passive hero past the line accrues nothing either.
            // Coming back inside the radius resets the dwell.
            if p.arm_radius > 0.0 && !p.armed {
                if dist > p.arm_radius {
                    if hero_active {
                        p.arm_for += dt;
                        if p.arm_for >= ARM_DWELL_SECS {
                            p.armed = true;
                        }
                    }
                } else {
                    p.arm_for = 0.0;
                }
            }
            // The collection test, edge-triggered (module docs): fire
            // only on the step the hero enters the reach. A resting item
            // spends the entry — when its rest lifts under his feet
            // nothing fires. An unaccepted item keeps the entry, so a
            // heal waited on is still taken the step it is wanted.
            let inside = dist <= reach;
            if inside && !p.was_inside && p.armed && now >= p.no_pick_until && accept(p.kind) {
                taken.push(self.items.remove(i));
                continue;
            }
            if inside {
                if now < p.no_pick_until {
                    p.was_inside = true;
                }
            } else {
                p.was_inside = false;
            }
            i += 1;
        }
        taken
    }

    /// Remove every item (a field reset re-places the spots itself).
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn placed_items_are_not_capped_and_drops_evict_the_oldest() {
        let mut p: Pickups<u8> = Pickups::new(2);
        p.spawn(90, 0.0, 0.0, 0.5, 0.0, None, Some(0)); // placed
        p.spawn(1, 1.0, 0.0, 0.5, 3.0, Some(18.0), None);
        p.spawn(2, 2.0, 0.0, 0.5, 3.0, Some(18.0), None);
        p.spawn(3, 3.0, 0.0, 0.5, 3.0, Some(18.0), None); // evicts kind 1
        let kinds: Vec<u8> = p.items.iter().map(|i| i.kind).collect();
        assert_eq!(kinds, vec![90, 2, 3]);
        assert!(p.spot_present(0));
    }

    #[test]
    fn a_magnetised_drop_slides_in_and_is_collected() {
        let mut p: Pickups<u8> = Pickups::new(8);
        p.spawn(1, 2.5, 0.0, 0.45, 3.0, Some(18.0), None);
        // Out of reach but inside the magnet radius: it drifts, not taken.
        let taken = p.step(DT, 0.0, (0.0, 0.0), 0.38, true, |_| true);
        assert!(taken.is_empty());
        assert!(p.items[0].x < 2.5, "the drop slid toward the hero");
        // Given time it arrives and is collected.
        let mut got = Vec::new();
        for n in 0..120 {
            got = p.step(DT, n as f32 * DT, (0.0, 0.0), 0.38, true, |_| true);
            if !got.is_empty() {
                break;
            }
        }
        assert_eq!(got.len(), 1);
        assert!(p.items.is_empty());
    }

    #[test]
    fn a_placed_item_never_drifts() {
        let mut p: Pickups<u8> = Pickups::new(8);
        p.spawn(1, 2.0, 0.0, 0.45, 0.0, None, Some(0));
        for n in 0..60 {
            let taken = p.step(DT, n as f32 * DT, (0.0, 0.0), 0.38, true, |_| true);
            assert!(taken.is_empty());
        }
        assert_eq!(p.items[0].x, 2.0);
    }

    #[test]
    fn an_unaccepted_item_is_left_standing() {
        let mut p: Pickups<u8> = Pickups::new(8);
        p.spawn(7, 0.2, 0.0, 0.5, 0.0, None, Some(0));
        let taken = p.step(DT, 0.0, (0.0, 0.0), 0.38, true, |_| false);
        assert!(taken.is_empty());
        assert_eq!(p.items.len(), 1);
    }

    #[test]
    fn drops_expire_and_blink_in_their_last_seconds() {
        let mut p: Pickups<u8> = Pickups::new(8);
        p.spawn(1, 9.0, 0.0, 0.45, 3.0, Some(2.0), None);
        assert!(Pickups::blinking(&p.items[0]));
        for _ in 0..130 {
            p.step(DT, 0.0, (0.0, 0.0), 0.38, true, |_| true);
        }
        assert!(p.items.is_empty(), "gone once its 2 s are up");
        let mut q: Pickups<u8> = Pickups::new(8);
        q.spawn(1, 9.0, 0.0, 0.45, 3.0, Some(18.0), None);
        assert!(!Pickups::blinking(&q.items[0]), "fresh drops do not blink");
    }

    #[test]
    fn a_rest_lifting_under_the_hero_collects_nothing_until_reentry() {
        // The hero is already standing on the item when its rest lifts:
        // the rest spent the entry edge, so nothing fires under his
        // feet — he must step out and back in (the swap ping-pong fix).
        let mut p: Pickups<u8> = Pickups::new(8);
        let id = p.spawn(1, 0.0, 0.0, 0.55, 0.0, None, None);
        p.set_no_pick_until(id, 1.0);
        let taken = p.step(DT, 0.5, (0.0, 0.0), 0.38, true, |_| true);
        assert!(taken.is_empty(), "still resting at t=0.5");
        let taken = p.step(DT, 1.5, (0.0, 0.0), 0.38, true, |_| true);
        assert!(taken.is_empty(), "the rest lifted under his feet: no fire");
        let taken = p.step(DT, 1.6, (0.0, 0.0), 0.38, true, |_| true);
        assert!(taken.is_empty(), "nor on any later standing step");
        // Out of reach, then back in: exactly one collection.
        let taken = p.step(DT, 1.7, (5.0, 0.0), 0.38, true, |_| true);
        assert!(taken.is_empty());
        let taken = p.step(DT, 1.8, (0.0, 0.0), 0.38, true, |_| true);
        assert_eq!(taken.len(), 1, "re-entry collects");
        assert!(p.items.is_empty());
    }

    #[test]
    fn an_arm_gated_drop_waits_for_a_clear_exit_and_return() {
        // The recurrence (2026-11-15): a swap drop gated at 2.5 m.
        // Standing on it takes nothing; combat footwork — out of the
        // 0.93 m reach and back, staying inside the arm radius —
        // takes nothing either, however often it crosses.
        let mut p: Pickups<u8> = Pickups::new(8);
        let id = p.spawn(1, 0.0, 0.0, 0.55, 0.0, Some(25.0), None);
        p.set_arm_radius(id, 2.5);
        let mut now = 0.0f32;
        let mut step = |p: &mut Pickups<u8>, hero: (f32, f32)| {
            now += DT;
            p.step(DT, now, hero, 0.38, true, |_| true)
        };
        assert!(step(&mut p, (0.0, 0.0)).is_empty(), "underfoot: unarmed");
        for _ in 0..6 {
            assert!(step(&mut p, (1.6, 0.0)).is_empty());
            assert!(
                step(&mut p, (0.0, 0.0)).is_empty(),
                "a combat crossing inside the arm radius re-takes nothing"
            );
            assert!(step(&mut p, (-1.6, 0.0)).is_empty());
            assert!(step(&mut p, (0.0, 0.0)).is_empty());
        }
        // A clear exit arms it (without taking it — he is out of
        // reach), but only once the absence has dwelt: a brief
        // excursion past the distance (a knockback) arms nothing,
        // and neither does being *carried* out there passively —
        // the dwell only counts while he steers himself.
        for _ in 0..30 {
            assert!(step(&mut p, (3.2, 0.0)).is_empty());
        }
        assert!(
            step(&mut p, (0.0, 0.0)).is_empty(),
            "0.5 s away is not yet armed"
        );
        let mut now2 = 100.0f32;
        for _ in 0..120 {
            now2 += DT;
            assert!(
                p.step(DT, now2, (3.2, 0.0), 0.38, false, |_| true)
                    .is_empty(),
                "carried out there, he arms nothing"
            );
        }
        assert!(
            step(&mut p, (0.0, 0.0)).is_empty(),
            "a passive absence never armed it"
        );
        for _ in 0..60 {
            assert!(step(&mut p, (3.2, 0.0)).is_empty(), "arming is not taking");
        }
        let taken = step(&mut p, (0.0, 0.0));
        assert_eq!(taken.len(), 1, "the return entry takes it, once");
        assert!(p.items.is_empty());
    }

    #[test]
    fn an_unaccepted_item_keeps_its_edge_while_he_stands_on_it() {
        let mut p: Pickups<u8> = Pickups::new(8);
        p.spawn(7, 0.0, 0.0, 0.5, 0.0, None, Some(0));
        // Standing on it, unwanted: it waits, and its edge is not spent.
        for n in 0..30 {
            let taken = p.step(DT, n as f32 * DT, (0.0, 0.0), 0.38, true, |_| false);
            assert!(taken.is_empty());
        }
        // The first step it is wanted, the standing hero takes it.
        let taken = p.step(DT, 1.0, (0.0, 0.0), 0.38, true, |_| true);
        assert_eq!(taken.len(), 1);
    }
}

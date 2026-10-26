//! The survivor kit's tests (headless: sim, input, HUD, the baked fixture, balance bots).

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
fn a_full_pickup_pool_still_gets_the_warden_chest_and_its_bar_clears_the_chips() {
    let mut g = quiet("bolt");
    g.next_elite = 45.0;
    g.t = 46.0;
    g.tick(DT);
    // Fill the pool with ordinary pickups far from the hero.
    let cap = g.spec.drops.cap as usize;
    let half = g.spec.arena.half - 1.0;
    while g.pickups.len() < cap {
        let k = g.pickups.len() as f32;
        g.pickups.push(Pickup {
            x: -half,
            z: -half + k * 0.1,
            item: 0,
            table: 0,
            age: k,
            fly: false,
        });
    }
    // The warden's bar starts right of the weapon chips on a narrow phone.
    let s = hud_scale(390.0, 844.0);
    let chips_end = 8.0 * s + g.spec.levels.max_weapons as f32 * 38.0 * s;
    let l = g.hud(390, 844, false);
    let bar = l
        .quads
        .iter()
        .find(|q| q.color == [255, 110, 90, 255])
        .expect("the warden bar");
    assert!(
        bar.x >= chips_end,
        "bar at {} under chips to {chips_end}",
        bar.x
    );
    let j = (0..g.foes.len())
        .find(|&i| g.kinds[g.foes.kind[i] as usize].elite)
        .unwrap();
    g.foes.hp[j] = 0.0;
    g.tick(DT);
    assert!(
        g.pickups.iter().any(|p| p.item == CHEST),
        "the chest is not lost"
    );
    assert!(g.pickups.len() <= cap);
    assert!(
        g.pickups
            .iter()
            .all(|p| p.item == CHEST || p.age < (cap - 1) as f32 + 0.5),
        "the oldest ordinary pickup made room"
    );
}

#[test]
fn the_warden_is_a_fight_that_grows_with_firepower_but_stays_capped() {
    let elite_hp = |g: &mut SurvivorArena| {
        g.next_elite = 45.0;
        g.t = 46.0;
        g.tick(DT);
        let j = (0..g.foes.len())
            .find(|&i| g.kinds[g.foes.kind[i] as usize].elite)
            .unwrap();
        g.foes.hp[j]
    };
    let mut g = quiet("bolt");
    let base = g.kinds.iter().find(|k| k.elite).unwrap().hp;
    let weak = elite_hp(&mut g);
    assert!(weak >= base, "never below its own hp");
    let mut s = quiet("bolt");
    for i in 0..s.weapon_lv.len() {
        s.weapon_lv[i] = 6;
    }
    let strong = elite_hp(&mut s);
    assert!(
        strong > weak,
        "more firepower, more warden: {strong} vs {weak}"
    );
    let cap = base * (1.0 + s.spec.director.hp_per_min * s.t / 60.0) * s.spec.director.elite_hp_cap;
    assert!(
        strong <= cap + 1e-3,
        "capped so it stays fair: {strong} <= {cap}"
    );
}

#[test]
fn a_chest_drifts_to_the_hero_from_afar_and_opens_from_a_wider_reach() {
    let mut g = quiet("bolt");
    g.pickups.clear();
    let (hx, hz) = (g.hx, g.hz);
    g.pickups.push(Pickup {
        x: hx + 5.0,
        z: hz,
        item: CHEST,
        table: 0,
        age: 0.0,
        fly: false,
    });
    g.tick(DT);
    assert!(g.pickups[0].fly, "pulled in from 5 m without any magnet");
    g.pickups.clear();
    g.pickups.push(Pickup {
        x: hx + 1.6,
        z: hz,
        item: CHEST,
        table: 0,
        age: 0.0,
        fly: false,
    });
    g.tick(DT);
    assert!(
        g.pickups.iter().all(|p| p.item != CHEST),
        "opens from 1.6 m"
    );
}

#[test]
fn an_off_screen_chest_gets_an_edge_arrow() {
    let mut g = quiet("bolt");
    g.pickups.clear();
    let (hx, hz) = (g.hx, g.hz);
    g.pickups.push(Pickup {
        x: hx + 11.0,
        z: hz,
        item: CHEST,
        table: 0,
        age: 0.0,
        fly: false,
    });
    g.build_doc();
    let hud = g.hud(390, 844, false);
    assert!(hud.texts.iter().any(|t| t.text.starts_with('宝')));
    assert!(hud.texts.iter().any(|t| t.text == "→"));
    g.pickups.clear();
    let hud = g.hud(390, 844, false);
    assert!(hud.texts.iter().all(|t| !t.text.starts_with('宝')));
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
    assert!(hud.texts.iter().any(|t| t.text == "弾"), "the bolt chip");
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

/// Pile `n` foes of every unlocked kind (tough, so they stay) in a ring on the hero,
/// and fill the gem pool: the worst frame the swarm can make.
fn pile_crowd(g: &mut SurvivorArena, n_foes: usize) {
    let unlocked: Vec<usize> = (0..g.kinds.len())
        .filter(|&i| !g.kinds[i].elite && g.kinds[i].unlock <= g.t)
        .collect();
    let mut rng = Rng::new(9);
    while g.foes.len() < n_foes {
        let kind = unlocked[rng.next_u32() as usize % unlocked.len()];
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(1.5, 11.0);
        let hp = g.kinds[kind].hp * 40.0;
        if !g.add_foe(kind, (g.hx + a.sin() * r, g.hz + a.cos() * r), hp) {
            break;
        }
    }
    for _ in 0..g.spec.caps.gems as usize {
        if g.gems.len() < g.spec.caps.gems as usize {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let r = rng.range(1.0, 10.0);
            g.gems.push(Gem {
                x: g.hx + a.sin() * r,
                z: g.hz + a.cos() * r,
                v: 1.0,
                fly: false,
            });
        }
    }
}

/// `cargo test --release -p kagra-shared --lib crowd_probe -- --ignored --nocapture`
/// is the worst frame: hundreds of foes of every kind on the hero, gems and damage
/// numbers at their caps, orbit blades out, particles flying. Per-phase native cost of
/// one step, and what the renderer is handed.
#[test]
#[ignore]
fn crowd_probe() {
    let n_foes: usize = std::env::var("KAGRA_FOES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(250);
    for (name, q) in [("low", Some("low")), ("mid", None), ("high", Some("high"))] {
        let mut spec = SurvivorSpec::default();
        spec.hero.max_hp = 1.0e9;
        spec.director.seed = 5;
        let mut g = SurvivorArena::new(spec);
        g.picture_quality = q.map(String::from);
        g.apply_quality();
        g.build_doc();
        g.confirm();
        for _ in 0..3 {
            g.grant_weapon();
        }
        // Run to a busy minute, then pile the crowd in a ring around the hero.
        while g.secs() < 90.0 && g.flow.phase == FlowPhase::Play {
            if g.card_count() > 0 {
                g.choose(0);
            }
            kite_step(&mut g);
            g.tick(DT);
        }
        pile_crowd(&mut g, n_foes);
        let at = [g.hx, 0.4, g.hz];
        let time = |label: &str, g: &mut SurvivorArena, f: &dyn Fn(&mut SurvivorArena)| {
            let t0 = std::time::Instant::now();
            for _ in 0..120 {
                f(g);
            }
            eprintln!(
                "{name} {label:<14} {:8.1} us",
                t0.elapsed().as_nanos() as f64 / 120.0 / 1000.0
            );
        };
        eprintln!(
            "{name}: foes {} gems {} fx cap {} dmg cap {}",
            g.foes.len(),
            g.gems.len(),
            g.fx_cap,
            DMG_CAP[g.tier]
        );
        time("grid.rebuild", &mut g, &|g| {
            g.grid.rebuild(&g.foes.x, &g.foes.z)
        });
        time("move_foes", &mut g, &|g| g.move_foes(0.0));
        time("fire", &mut g, &|g| g.fire(0.0));
        time("step_blades", &mut g, &|g| g.step_blades(0.0));
        time("reap", &mut g, &|g| g.reap());
        time("collect", &mut g, &|g| g.collect(0.0));
        time("step_fx", &mut g, &|g| {
            g.burst(FX_GLINT, 6, at, [0.0, 0.0], 3.0, 1.0, 3.0, 0.7);
            g.step_fx(0.0)
        });
        time("whole step", &mut g, &|g| g.step_sim(DT));
        time("build_doc", &mut g, &|g| g.build_doc());
        let doc = g.doc.clone();
        let live = doc.props.iter().filter(|p| p.enabled).count();
        let pts: usize = doc.points.iter().map(|p| p.positions.len()).sum();
        let fp = crate::profile::FrameProfile::from_world(&doc);
        eprintln!(
            "{name}: live props {live} points {pts} gpu draws {} triangles {}",
            fp.draws, fp.triangles
        );
        {
            let meshes = doc.compile_meshes();
            let scene = doc.compile_scene(0.45);
            for b in &scene.batches {
                if let Some((_, m)) = meshes.iter().find(|(id, _)| *id == b.mesh) {
                    eprintln!(
                        "{name}:   batch mesh {:?} x{:<4} {:6} tris each = {}",
                        b.mesh,
                        b.instances.len(),
                        m.indices.len() / 3,
                        m.indices.len() / 3 * b.instances.len()
                    );
                }
            }
            let mut by: std::collections::BTreeMap<String, usize> = Default::default();
            for p in doc.props.iter().filter(|p| p.enabled) {
                *by.entry(
                    p.gltf
                        .clone()
                        .unwrap_or_else(|| format!("prim:{:?}", p.kind)),
                )
                .or_default() += 1;
            }
            eprintln!("{name}:   live by model {by:?}");
        }
        time("doc.clone", &mut g, &|g| {
            std::hint::black_box(g.doc.clone());
        });
        time("compile_scene", &mut g, &|g| {
            std::hint::black_box(g.doc.compile_scene(0.45));
        });
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
    if let Some(n) = std::env::var("KAGRA_CROWD")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        pile_crowd(&mut g, n);
        g.build_doc();
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
        "pick", "foe", "gem", "wall", "column", "decor", "shot", "hero", "floor", "ring", "loot",
        "blade", "chest",
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

#[test]
fn the_blaster_is_held_only_with_a_gun_weapon_and_petals_have_no_barrel_streak() {
    let gun = |g: &SurvivorArena| {
        g.doc
            .props
            .iter()
            .find(|p| p.name == "gun")
            .map(|p| p.enabled)
    };
    for (start, held) in [
        ("bolt", true),
        ("petals", true),
        ("wisps", true),
        ("blades", false),
        ("zap", false),
        ("pulse", false),
    ] {
        let mut g = quiet(start);
        g.build_doc();
        g.tick(DT);
        if gun(&g).is_none() {
            return; // the model is not in this build: nothing to check
        }
        assert_eq!(gun(&g), Some(held), "{start}");
    }
    let mut g = quiet("petals");
    g.build_doc();
    g.shots.push(Shot {
        x: g.hx + 3.0,
        z: g.hz,
        vx: 15.0,
        vz: 0.0,
        life: 1.0,
        dmg: 1.0,
        pierce: 0,
        r: 0.2,
        hits: [0; 4],
        n: 0,
        kind: 1,
        w: 0,
        area: 0.0,
    });
    g.build_doc();
    assert!(g.doc.lines[0].points.is_empty(), "a petal leaves no streak");
}

#[test]
fn graphics_hints_thin_the_picture_and_never_touch_the_sim() {
    use crate::kit_host::KitHost;
    let play = |atmo: f32, particles: f32| {
        let mut g = fresh();
        g.apply_quality();
        g.build_doc();
        g.confirm();
        KitHost::quality_hints(&mut g, atmo, particles);
        let mut fx_seen = 0usize;
        for _ in 0..60 * 40 {
            if g.card_count() > 0 {
                g.choose(0);
            }
            kite_step(&mut g);
            g.tick(DT);
            fx_seen = fx_seen.max(g.fx.len());
        }
        let decor = g
            .doc
            .props
            .iter()
            .filter(|p| p.name == "decor" && p.enabled)
            .count();
        (g.state_json(), decor, fx_seen)
    };
    assert!(KitHost::supports_quality_tiers(&fresh()));
    let (full, decor_full, fx_full) = play(1.0, 1.0);
    let (thin, decor_thin, fx_thin) = play(0.3, 0.4);
    assert_eq!(full, thin, "tiers are picture only: same run, same state");
    assert!(decor_full > 10 && decor_thin < decor_full * 4 / 10 + 2);
    assert!(decor_thin > 0, "a thin ring, not an empty one");
    assert!(fx_thin < fx_full, "fewer sparks ({fx_thin} vs {fx_full})");
}

/// Close-ups of the hero for each starting weapon, to see what it holds.
/// `KAGRA_CLOSEUP_DIR=/tmp/x cargo test --release -p kagra-shared --features render --lib closeup_probe -- --ignored --nocapture`
#[cfg(feature = "render")]
#[test]
#[ignore = "writes PNGs for a human to look at"]
fn closeup_probe() {
    use crate::render::Renderer;
    let dir = std::env::var("KAGRA_CLOSEUP_DIR").unwrap_or_else(|_| "/tmp/closeup".into());
    std::fs::create_dir_all(&dir).unwrap();
    let Ok(mut r) = pollster::block_on(Renderer::new_offscreen(390, 520)) else {
        eprintln!("no adapter");
        return;
    };
    for start in ["bolt", "petals", "blades", "wisps", "zap", "pulse"] {
        let mut g = quiet(start);
        g.apply_quality();
        g.build_doc();
        g.confirm();
        for _ in 0..90 {
            g.tick(DT);
        }
        let mut d = g.doc.clone();
        let (hx, hz) = (g.hx, g.hz);
        let near = std::env::var("KAGRA_CLOSEUP_NEAR").is_ok();
        d.cameras[0].position = if near {
            [hx + 1.6, 1.8, hz + 2.0]
        } else {
            [hx, 5.5, hz + 4.0]
        };
        d.cameras[0].target = [hx, if near { 0.9 } else { 0.4 }, hz];
        d.cameras[0].fov = if near { 40.0 } else { 45.0 };
        let _ = r.draw_world_doc_with_hud(&d, &crate::scene::DrawList::default());
        let px = r.read_rgba().unwrap();
        let mut out = b"P6\n390 520\n255\n".to_vec();
        for c in px.chunks(4) {
            out.extend_from_slice(&c[..3]);
        }
        std::fs::write(format!("{dir}/{start}.ppm"), out).unwrap();
        let on: Vec<_> = d
            .props
            .iter()
            .filter(|p| p.enabled && (p.name == "gun" || p.name == "blade" || p.name == "hero"))
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect();
        eprintln!("{start}: {on:?}");
    }
}

/// `cargo test --release -p kagra-shared --features render --lib lod_look_probe -- --ignored --nocapture`
/// writes the same crowd close up with full detail and thinned (`/tmp/closeup/lod-*.ppm`).
#[cfg(feature = "render")]
#[test]
#[ignore = "writes PNGs for a human to look at"]
fn lod_look_probe() {
    use crate::render::Renderer;
    let Ok(mut r) = pollster::block_on(Renderer::new_offscreen(640, 400)) else {
        eprintln!("no adapter");
        return;
    };
    std::fs::create_dir_all("/tmp/closeup").unwrap();
    for (name, keep) in [("full", 1.0f32), ("mid", 0.65), ("low", 0.5)] {
        let mut spec = SurvivorSpec::default();
        spec.fx.foe_detail = [keep; 3];
        spec.hero.max_hp = 1.0e9;
        let mut g = SurvivorArena::new(spec);
        g.apply_quality();
        g.build_doc();
        g.confirm();
        while g.secs() < 80.0 {
            if g.card_count() > 0 {
                g.choose(0);
            }
            kite_step(&mut g);
            g.tick(DT);
        }
        pile_crowd(&mut g, 14);
        for _ in 0..3 {
            g.tick(DT);
        }
        g.build_doc();
        let mut d = g.doc.clone();
        let (hx, hz) = (g.hx, g.hz);
        d.cameras[0].position = [hx, 6.0, hz + 5.0];
        d.cameras[0].target = [hx, 0.4, hz];
        d.cameras[0].fov = 45.0;
        let _ = r.draw_world_doc_with_hud(&d, &crate::scene::DrawList::default());
        let px = r.read_rgba().unwrap();
        let mut out = b"P6\n640 400\n255\n".to_vec();
        for c in px.chunks(4) {
            out.extend_from_slice(&c[..3]);
        }
        std::fs::write(format!("/tmp/closeup/lod-{name}.ppm"), out).unwrap();
    }
}

#[test]
fn crowd_kills_melt_gems_and_repeat_hits_count_up_one_number() {
    let mut g = quiet("bolt");
    g.gems.clear();
    let (hx, hz) = (g.hx + 6.0, g.hz);
    g.gems.push(Gem {
        x: hx,
        z: hz,
        v: 2.0,
        fly: false,
    });
    let j = g.foes.len();
    assert!(g.add_foe(0, (hx, hz), 0.0));
    g.tick(DT);
    assert_eq!(g.foes.len(), j, "the foe died");
    let near: Vec<_> = g.gems.iter().filter(|m| (m.x - hx).abs() < 1.0).collect();
    assert_eq!(near.len(), 1, "the new gem melted into the resting one");
    assert!(near[0].v >= 3.0, "worth the sum: {}", near[0].v);
    // A gem that is already flying to the hero is not a merge target.
    g.gems.clear();
    g.gems.push(Gem {
        x: hx,
        z: hz,
        v: 2.0,
        fly: true,
    });
    assert!(g.add_foe(0, (hx, hz), 0.0));
    g.tick(DT);
    assert!(g.gems.len() >= 2 || g.gems.iter().all(|m| m.fly));

    let mut g = quiet("bolt");
    g.dmg.clear();
    assert!(g.add_foe(0, (g.hx + 6.0, g.hz), 1.0e6));
    let j = g.foes.len() - 1;
    g.damage_foe(j, 5.0, (0.0, 0.0), 0.0, false);
    g.damage_foe(j, 7.0, (0.0, 0.0), 0.0, false);
    assert_eq!(g.dmg.len(), 1, "one number");
    assert_eq!(g.dmg[0].val, 12);
}

#[test]
fn the_fixed_stick_is_analog_has_a_dead_zone_and_leaves_the_other_thumb_free() {
    let (w, h) = (390u32, 844u32);
    let at = |g: &mut SurvivorArena, id, x, y, phase| {
        g.touch_ev(
            PointerEvent {
                id,
                x,
                y,
                phase,
                pressure: 1.0,
            },
            w,
            h,
        )
    };
    let run = |dx: f32, dy: f32| -> (f32, f32) {
        let mut g = started();
        g.foes.clear();
        g.budget = Default::default();
        g.spec.director.start_rate = 0.0;
        g.spec.director.end_rate = 0.0;
        g.hud(w, h, false);
        let (cx, cy) = g.stick_center(w as f32, h as f32);
        let r = g.stick_radius(w as f32, h as f32);
        // Land anywhere in the thumb zone; the push is measured from the base.
        // Land inside the zone, then slide to the target: dragging past the rim is fine.
        at(&mut g, 1, cx, cy, PointerPhase::Begin);
        at(&mut g, 1, cx + dx * r, cy + dy * r, PointerPhase::Move);
        let p0 = g.hero();
        for _ in 0..60 {
            g.tick(DT);
        }
        let p1 = g.hero();
        (p1.0 - p0.0, p1.1 - p0.1)
    };
    let still = run(0.05, 0.0);
    assert!(still.0.hypot(still.1) < 0.05, "dead zone: {still:?}");
    let half = run(0.55, 0.0);
    let full = run(1.0, 0.0);
    assert!(
        half.0 > 0.5 && full.0 > half.0 * 1.4,
        "analog: {half:?} {full:?}"
    );
    assert!(full.1.abs() < 0.2, "straight right");
    let up = run(0.0, -1.0);
    assert!(up.1 < -1.0, "up the screen is -z: {up:?}");
    let beyond = run(3.0, 0.0);
    assert!(
        (beyond.0 - full.0).abs() < 0.3,
        "the rim clamps: {beyond:?} vs {full:?}"
    );

    // The stick finger never taps; a finger outside the zone does, with the stick held.
    let (mut g, _, far) = two_foes();
    g.hud(w, h, false);
    let (cx, cy) = g.stick_center(w as f32, h as f32);
    at(&mut g, 1, cx, cy, PointerPhase::Begin);
    at(&mut g, 1, cx, cy, PointerPhase::End);
    assert_eq!(g.locked(), None, "a stick tap is not a foe tap");
    at(&mut g, 1, cx + 5.0, cy, PointerPhase::Begin);
    let (x, y) = on_screen(&g, far);
    at(&mut g, 2, x, y, PointerPhase::Begin);
    at(&mut g, 2, x, y, PointerPhase::End);
    assert_eq!(g.locked(), Some(far), "the other thumb locks it");
    assert!(g.finger.is_some(), "the stick thumb still steers");
    // The base is drawn under the thumb, smooth (one-pixel rows), and follows past the rim.
    let l = g.hud(w, h, false);
    let (bx, by) = g.origin;
    assert!(l.quads.iter().any(|q| q.h <= 1.0 && q.contains(bx, by)));
    let r = g.stick_radius(w as f32, h as f32);
    at(&mut g, 1, bx + 3.0 * r, by, PointerPhase::Move);
    assert!(
        (g.origin.0 - (bx + 2.0 * r)).abs() < 0.5,
        "the base follows: {:?}",
        g.origin
    );
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
fn arena_dark_dims_the_night_and_zero_keeps_the_old_look() {
    let doc_at = |dark: f32| {
        let mut spec = SurvivorSpec::default();
        spec.arena.dark = dark;
        let mut g = SurvivorArena::new(spec);
        g.build_doc();
        g.doc.clone()
    };
    let (old, night) = (doc_at(0.0), doc_at(1.0));
    assert_eq!(old.exposure, Some(1.0));
    assert!(night.exposure.unwrap() < 0.9);
    assert!(night.lights[0].radius < old.lights[0].radius);
    let floor = |d: &WorldDoc| d.props.iter().find(|p| p.name == "floor").unwrap().color;
    assert!(floor(&night).unwrap()[0] < floor(&old).unwrap()[0]);
    let mut bad = SurvivorSpec::default();
    bad.arena.dark = 1.5;
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
    let got: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/survivor_arena_world.json"
    ))
    .unwrap();
    assert_eq!(
        got, want,
        "run `KAGRA_BAKE=1 cargo test baked` to refresh it"
    );
    assert!(is_survivor_dump(&got.to_string()));
}

#[test]
fn a_dump_round_trips_through_the_registry() {
    let json = include_str!("../../tests/fixtures/survivor_arena_world.json");
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
    g.spec.input.move_scheme = "drag".into();
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
            == g.spec.caps.shots as usize + 2 + g.spec.drops.cap as usize + ZAP_CAP + RING_CAP + 2
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
    g.spec.input.move_scheme = "drag".into();
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
    let json = include_str!("../../tests/fixtures/survivor_arena_world.json");
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
    // Land a thumb, slide it to the right and hold: the hero follows over some frames.
    s.push_pointer(ev(PointerPhase::Begin, 200.0, 340.0));
    s.push_pointer(ev(PointerPhase::Move, 320.0, 340.0));
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

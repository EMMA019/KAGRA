use super::*;
use crate::kit_host::KitHost;

const DT: f32 = 1.0 / 60.0;

fn started() -> StationEscape {
    let mut g = StationEscape::new(StationEscapeSpec::default());
    let _ = g.hud(390, 844, false);
    g.confirm();
    g
}

/// Past the story card.
fn running() -> StationEscape {
    let mut g = started();
    g.walk(0.0, 0.0, true);
    g.tick(DT);
    g.walk(0.0, 0.0, false);
    g.tick(DT);
    assert_eq!(g.beat, Beat::Run);
    g
}

fn key(g: &mut StationEscape, x: f32, jump: bool, dash: bool, steps: usize) {
    for _ in 0..steps {
        g.walk(x, 0.0, jump);
        g.buttons(dash, false);
        g.tick(DT);
    }
}

/// A simple player: heads for each x in `route` in turn, jumps at edges, walls and spikes,
/// dashes when a fall would miss, waits for lasers. Returns true at the clear card.
fn bot(g: &mut StationEscape, route: &[f32], secs: f32) -> bool {
    let spec = g.spec.body.clone();
    let mut leg = 0;
    let mut hold = 0_u32;
    let mut dash_wait = 0_u32;
    let steps = (secs / DT) as usize;
    for _ in 0..steps {
        if g.beat == Beat::Clear {
            return true;
        }
        if matches!(g.beat, Beat::Down(_)) {
            key(g, 0.0, false, false, 1);
            continue;
        }
        while leg + 1 < route.len() && (g.body.x - route[leg]).abs() < 0.5 {
            leg += 1;
        }
        let goal = route[leg];
        let dir = if goal >= g.body.x { 1.0 } else { -1.0 };
        let (x, y) = (g.body.x, g.body.y);
        let st = g.st().clone();
        let solids = g.solids.clone();
        let under = |px: f32, below: f32| {
            solids
                .iter()
                .any(|s| px > s.x0 && px < s.x1 && s.y1 <= y + 0.05 && s.y1 >= y - below)
        };
        let wall = solids.iter().any(|s| {
            !s.one_way
                && s.y1 > y + 0.1
                && s.y0 < y + spec.height
                && if dir > 0.0 {
                    s.x0 >= x && s.x0 < x + spec.half_w + 0.6
                } else {
                    s.x1 <= x && s.x1 > x - spec.half_w - 0.6
                }
        });
        // A ledge above that the route climbs to.
        let climb = solids.iter().any(|s| {
            s.y1 > y + 0.5
                && s.y1 < y + 2.2
                && (goal - x).abs() < 6.0
                && ((s.x0 - 0.3)..(s.x1 + 0.3)).contains(&goal)
                && ((x + dir * 1.2) > s.x0 - 1.5 && (x + dir * 1.2) < s.x1 + 1.5)
        });
        let edge = !under(x + dir * 0.7, 0.6);
        let spike = st.spikes.iter().any(|s| {
            (s.x - (x + dir * 1.2)).abs() < 0.7
                || (x + dir * 1.2 > s.x && x + dir * 1.2 < s.x + s.w)
        });
        let laser_near = st.lasers.iter().any(|l| {
            let d = (l.x - x) * dir;
            d > 0.2
                && d < 2.2
                && l.y < y + spec.height
                && l.y + l.h > y
                && (l.state(g.t).0 || l.state(g.t + 0.45).0 || l.state(g.t + 0.2).0)
        });
        // A live drone ahead at body height: dash through it. A bolt coming: jump it.
        let drone_ahead = st.drones.iter().enumerate().any(|(i, d)| {
            let (dx, dy, _) = d.at(g.t);
            g.foes.alive[i]
                && (dx - x) * dir > 0.0
                && (dx - x) * dir < 2.4
                && dy - y > -0.4
                && dy - y < 1.5
        });
        let bolt_coming = g.foes.bolts.iter().flatten().any(|b| {
            (b[0] - x) * dir > 0.0
                && (b[0] - x) * dir < 2.6
                && b[2] * dir < 0.0
                && b[1] > y
                && b[1] < y + spec.height + 0.2
        });
        let mut mx = dir;
        let mut jump = hold > 0;
        let mut dash = false;
        if g.body.grounded {
            // Nothing to jump to yet (a lift is away): wait at the edge.
            let reach = (1..=16).any(|k| under(x + dir * k as f32 * 0.3, 2.5))
                || solids
                    .iter()
                    .any(|s| s.y1 > y + 0.3 && s.y1 < y + 2.2 && (s.x0 - x).abs() < 4.0);
            let lift = st.blocks.iter().any(|b| b.kind == "move");
            if laser_near || (edge && !reach && lift) {
                mx = 0.0;
            } else if (edge || wall || spike || climb || bolt_coming) && hold == 0 {
                jump = true;
                hold = 30;
            }
        } else if g.body.vy < 1.0 && dash_wait == 0 {
            // Near the top of a jump: is there a surface below that it comes down on without a
            // dash? Each surface is reached when the fall has dropped to its top; moving ones
            // move meanwhile.
            let g_fall = spec.gravity * spec.fall_gravity;
            let vy0 = g.body.vy.max(0.0);
            let lands = solids.iter().any(|s| {
                let drop = y - s.y1;
                if drop < -0.05 {
                    return false;
                }
                let t_up = vy0 / spec.gravity;
                let t = t_up + (2.0 * (drop + vy0 * vy0 / (2.0 * spec.gravity)) / g_fall).sqrt();
                let lx = x + dir * spec.run * t;
                let (x0, x1) = (s.x0 + s.vel.0 * t, s.x1 + s.vel.0 * t);
                lx + spec.half_w > x0 && lx - spec.half_w < x1
            });
            if !lands {
                dash = true;
                dash_wait = 40;
            }
        }
        if drone_ahead && dash_wait == 0 {
            dash = true;
            dash_wait = 25;
        }
        hold = hold.saturating_sub(1);
        dash_wait = dash_wait.saturating_sub(1);
        if std::env::var("SE_DEBUG").is_ok() {
            eprintln!("x {:.2} y {:.2} g {} vy {:.1} falls {} mx {mx} jump {jump} dash {dash} laser {laser_near} spike {spike} edge {edge} wall {wall} climb {climb}", x, y, g.body.grounded, g.body.vy, g.falls);
        }
        key(g, mx, jump, dash, 1);
    }
    g.beat == Beat::Clear
}

#[test]
fn the_default_spec_is_valid_and_strict() {
    StationEscapeSpec::default().validate().unwrap();
    let v = StationEscape::new(StationEscapeSpec::default()).world_value()["game"].clone();
    let mut bad = v.clone();
    bad["bogus"] = json!(1);
    assert!(StationEscapeSpec::from_value(&bad).is_err());
    let mut bad = v.clone();
    bad["stages"][0]["blocks"][0]["kind"] = json!("lava");
    assert!(StationEscapeSpec::from_value(&bad).is_err());
    let mut bad = v;
    bad["stages"][0]["kill_y"] = json!(10.0);
    assert!(StationEscapeSpec::from_value(&bad).is_err());
}

#[test]
fn spawns_checkpoints_and_exits_stand_on_something() {
    let spec = StationEscapeSpec::default();
    for (i, st) in spec.stages.iter().enumerate() {
        let on = |p: [f32; 2]| {
            st.blocks
                .iter()
                .any(|b| p[0] > b.x && p[0] < b.x + b.w && (b.y + b.h - p[1]).abs() < 0.01)
        };
        assert!(on(st.spawn), "stage {i} spawn");
        assert!(on(st.exit), "stage {i} exit");
        for c in &st.checkpoints {
            assert!(on(*c), "stage {i} checkpoint {c:?}");
        }
    }
}

#[test]
fn no_drone_patrols_over_a_spawn_or_a_checkpoint() {
    for (i, st) in StationEscapeSpec::default().stages.iter().enumerate() {
        let mut safe = vec![st.spawn];
        safe.extend(st.checkpoints.iter().copied());
        for p in safe {
            for d in &st.drones {
                let over = p[0] > d.x0.min(d.x1) - 1.5 && p[0] < d.x0.max(d.x1) + 1.5;
                let level = d.y > p[1] - 0.5 && d.y < p[1] + 2.0;
                assert!(
                    !(over && level),
                    "stage {}: a drone patrols over {p:?}",
                    i + 1
                );
            }
        }
    }
}

/// Emma's playtest: the dash gap in stage 1 had no run-up after the spikes. A sloppy human
/// (jumps half a metre before the edge, dashes a little after the top) still makes it, from
/// a stand-still just past the spikes.
#[test]
fn the_stage_one_dash_gap_forgives_a_sloppy_jump() {
    let mut g = running();
    let st = g.st().clone();
    let sp = &st.spikes[0];
    let edge = st
        .blocks
        .iter()
        .filter(|b| b.x < 56.0 && b.x + b.w > sp.x + sp.w)
        .map(|b| b.x + b.w)
        .fold(f32::MIN, f32::max);
    for early in [0.2_f32, 0.5] {
        g.respawn();
        g.body.reset_to(sp.x + sp.w + 0.4, 0.0);
        key(&mut g, 0.0, false, false, 2);
        let falls = g.falls;
        let mut jumped = false;
        let mut dashed = false;
        let mut peak_t = 0;
        for _ in 0..240 {
            let jump = !jumped && g.body.x >= edge - early;
            jumped |= jump;
            if jumped && !g.body.grounded && g.body.vy <= 0.0 {
                peak_t += 1;
            }
            let dash = jumped && !dashed && peak_t == 4;
            dashed |= dash;
            key(&mut g, 1.0, jumped, dash, 1);
            if g.body.grounded && jumped && g.body.x > edge + 1.0 {
                break;
            }
        }
        assert_eq!(g.falls, falls, "jumping {early} m early fell");
        assert!(g.body.x > edge + 1.0, "landed across: {}", g.body.x);
    }
}

#[test]
fn the_story_card_waits_for_a_tap() {
    let mut g = started();
    assert_eq!(g.beat, Beat::Intro);
    key(&mut g, 1.0, false, false, 60);
    assert_eq!(g.beat, Beat::Intro);
    assert_eq!(g.body.x, g.st().spawn[0], "frozen behind the card");
    g.touch(
        PointerEvent {
            id: 1,
            x: 200.0,
            y: 400.0,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        },
        390,
        844,
        0.0,
    );
    g.tick(DT);
    assert_eq!(g.beat, Beat::Run);
}

#[test]
fn the_left_thumb_runs_and_the_right_jumps_and_dashes() {
    let mut g = running();
    let ev = |id, phase, x, y| PointerEvent {
        id,
        x,
        y,
        phase,
        pressure: 1.0,
    };
    let x0 = g.body.x;
    g.touch(ev(1, PointerPhase::Begin, 90.0, 700.0), 390, 844, 0.0);
    g.touch(ev(1, PointerPhase::Move, 170.0, 700.0), 390, 844, 0.0);
    key(&mut g, 0.0, false, false, 30);
    assert!(g.body.x > x0 + 1.5, "stick right runs right");
    let [(jc, _), (dc, _)] = g.buttons_at(390.0, 844.0);
    g.touch(ev(2, PointerPhase::Begin, jc.0, jc.1), 390, 844, 0.0);
    key(&mut g, 0.0, false, false, 10);
    assert!(g.body.y > 1.0, "jump: {}", g.body.y);
    g.touch(ev(3, PointerPhase::Begin, dc.0, dc.1), 390, 844, 0.0);
    g.tick(DT);
    assert!(g.body.dashing(), "dash");
}

#[test]
fn cells_checkpoints_lasers_and_falls() {
    let mut g = running();
    let c = g.st().cells[0];
    g.body.reset_to(c[0], 0.0);
    key(&mut g, 0.0, false, false, 2);
    assert!(g.got[0]);
    let cp = g.st().checkpoints[0];
    g.body.reset_to(cp[0], 0.0);
    key(&mut g, 0.0, false, false, 2);
    assert_eq!(g.check, Some(0));
    // Into the laser while it is on.
    let l = g.st().lasers[0].clone();
    g.t = 0.0 - l.phase;
    g.body.reset_to(l.x + 0.05, l.y);
    key(&mut g, 0.0, false, false, 1);
    assert!(matches!(g.beat, Beat::Down(_)), "the laser");
    key(&mut g, 0.0, false, false, 60);
    assert_eq!(g.beat, Beat::Run);
    assert!((g.body.x - cp[0]).abs() < 0.01, "back at the checkpoint");
    // Off the edge into the pit.
    g.body.reset_to(13.0, 0.0);
    key(&mut g, 0.0, false, false, 90);
    assert_eq!(g.falls, 2);
}

#[test]
fn crumbling_ledges_fall_and_come_back_and_lifts_carry() {
    let mut g = running();
    g.load_stage(1);
    g.beat = Beat::Run;
    let i = g
        .st()
        .blocks
        .iter()
        .position(|b| b.kind == "crumble")
        .unwrap();
    let b = g.st().blocks[i].clone();
    g.body.reset_to(b.x + b.w * 0.5, b.y + b.h);
    key(&mut g, 0.0, false, false, 2);
    assert!(g.crumbles[i].since.is_some());
    key(&mut g, 0.0, false, false, 40);
    assert!(g.crumbles[i].gone > 0.0, "it fell");
    assert!(!g.body.grounded || g.body.y < b.y);
    let m = g.st().blocks.iter().position(|b| b.kind == "move").unwrap();
    let mb = g.st().blocks[m].clone();
    g.respawn();
    g.t = 0.0;
    g.update_solids();
    g.body.reset_to(mb.x + mb.w * 0.5, mb.y + mb.h);
    let x0 = g.body.x;
    key(&mut g, 0.0, false, false, 60);
    assert!(
        g.body.x > x0 + 1.5,
        "the lift carried the hero: {} -> {}",
        x0,
        g.body.x
    );
}

/// The boss fight: wait in the middle, dash into the Warden when it swoops close, run away
/// from bolts' landing spot a little. Returns true at the clear card.
fn boss_bot(g: &mut StationEscape, secs: f32) -> bool {
    let exit = g.st().exit[0];
    let steps = (secs / DT) as usize;
    let mut dash_wait = 0_u32;
    let mut pace_to = 19.0_f32;
    for _ in 0..steps {
        if g.beat == Beat::Clear {
            return true;
        }
        let w = g.warden.clone().unwrap();
        let (x, y) = (g.body.x, g.body.y);
        let mut dash = false;
        let mut mx;
        if w.dead() {
            mx = (exit - x).signum();
        } else {
            let to = w.x - x;
            let near = to.abs() < 2.6 && (w.y - y) < 1.8;
            if w.beat == boss::Beat::Swoop && near && dash_wait == 0 {
                mx = to.signum();
                dash = true;
                dash_wait = 30;
            } else {
                // Keep moving under it, so aimed bolts land where it was.
                if (x - pace_to).abs() < 0.4 {
                    pace_to = if pace_to > 14.0 { 9.0 } else { 19.0 };
                }
                mx = (pace_to - x).signum();
                // A bolt that would meet us on this course: turn round.
                let run = g.spec.body.run;
                let hits_us = |dir: f32| {
                    g.foes.bolts.iter().flatten().any(|b| {
                        (1..30).any(|k| {
                            let t = k as f32 * 0.03;
                            let (bx, by) = (b[0] + b[2] * t, b[1] + b[3] * t);
                            let hx = x + dir * run * t;
                            (bx - hx).abs() < 0.7 && by > y - 0.2 && by < y + 1.4
                        })
                    })
                };
                if hits_us(mx) {
                    mx = -mx;
                    pace_to = if mx > 0.0 { 19.0 } else { 9.0 };
                }
            }
        }
        dash_wait = dash_wait.saturating_sub(1);
        let falls = g.falls;
        key(g, mx, false, dash, 1);
        if std::env::var("SE_DEBUG").is_ok() && g.falls > falls {
            let bolt = g.foes.bolts.iter().flatten().count();
            eprintln!("fell at x {x:.2} y {y:.2}: warden {:?} at ({:.2}, {:.2}) hp {} bolts {bolt} dashing {}", w.beat, w.x, w.y, w.hp, g.body.dashing());
        }
    }
    g.beat == Beat::Clear
}

fn tap_on(g: &mut StationEscape) {
    g.walk(0.0, 0.0, true);
    g.tick(DT);
    g.walk(0.0, 0.0, false);
    g.tick(DT);
}

#[test]
fn a_player_clears_every_stage_and_the_run_ends() {
    let mut g = running();
    let routes: [&[f32]; 5] = [
        &[80.0],
        &[29.0, 37.5, 32.5, 38.5, 59.5, 59.8, 64.0, 67.5, 90.0],
        &[90.0],
        &[11.8, 11.9, 16.0, 28.2, 31.5, 35.0, 90.0],
        &[],
    ];
    for (i, route) in routes.iter().enumerate() {
        assert_eq!(g.stage, i);
        let cleared = if route.is_empty() {
            boss_bot(&mut g, 150.0)
        } else {
            bot(&mut g, route, 150.0)
        };
        assert!(
            cleared,
            "stage {}: stuck at {:?} after {} falls",
            i + 1,
            (g.body.x, g.body.y),
            g.falls
        );
        let tl = *g.tallies.last().unwrap();
        assert!(tl.score >= 100);
        eprintln!(
            "stage {}: {:.0} s, {} falls, {} drones",
            i + 1,
            tl.secs,
            tl.falls,
            tl.wrecks
        );
        tap_on(&mut g);
        if i + 1 < routes.len() {
            tap_on(&mut g);
        }
    }
    for _ in 0..30 {
        g.tick(DT);
    }
    assert_eq!(g.phase_name(), "result");
    let v = g.state_value();
    assert_eq!(v["win"], true);
    assert!(
        g.tallies.iter().map(|t| t.wrecks).sum::<u32>() > 0,
        "drones met"
    );
}

#[test]
fn the_hero_and_the_buttons_stay_on_screen_either_way_up() {
    let mut g = running();
    key(&mut g, 1.0, false, false, 60);
    for (w, h) in [(390_u32, 844_u32), (844, 390)] {
        let hud = g.hud(w, h, false);
        g.rebuild();
        let cam = &g.document().cameras[0];
        let view = View {
            eye: cam.position,
            target: cam.target,
            fov: cam.fov,
        };
        for p in [
            [g.body.x, g.body.y, 0.0],
            [g.body.x, g.body.y + 1.1, 0.0],
            [g.body.x + 5.0, g.body.y, 0.0],
        ] {
            let (sx, sy) = view.project(p, w as f32, h as f32).unwrap();
            assert!(
                (0.0..=w as f32).contains(&sx) && (0.0..=h as f32).contains(&sy),
                "{w}x{h}: {p:?} at ({sx}, {sy})"
            );
        }
        let [(jc, jr), (dc, _)] = g.buttons_at(w as f32, h as f32);
        assert!(jc.0 + jr <= w as f32 && jc.1 + jr <= h as f32 && dc.0 > w as f32 * 0.5);
        assert!(hud.quads.iter().any(|q| q.radius > 0.0));
    }
}

#[test]
fn the_gpu_model_set_never_changes_in_a_stage() {
    let mut g = running();
    g.rebuild();
    let key0 = g.document().gpu_mesh_key();
    for i in 0..600 {
        key(&mut g, 1.0, i % 40 < 20, i % 90 == 0, 1);
        g.rebuild();
        assert_eq!(g.document().gpu_mesh_key(), key0, "step {i}");
    }
}

#[test]
fn a_link_can_start_at_a_later_stage() {
    let mut v = StationEscape::new(StationEscapeSpec::default()).world_value();
    v["stage"] = json!("3");
    let mut g = StationEscape::from_json(&v.to_string()).unwrap();
    g.confirm();
    assert_eq!(g.stage, 2);
    v["stage"] = json!(99);
    let g = StationEscape::from_json(&v.to_string()).unwrap();
    assert_eq!(g.stage, g.spec.stages.len() - 1);
}

#[test]
fn the_device_keeps_records_and_opens_the_stages_reached() {
    let mut g = running();
    assert_eq!(
        g.save_json().unwrap(),
        json!({"cleared": 0, "best_secs": [null, null, null, null, null]}).to_string()
    );
    g.t = 41.25;
    g.clear_stage();
    let saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    assert_eq!(saved["cleared"], 1);
    assert!((saved["best_secs"][0].as_f64().unwrap() - 41.3).abs() < 1e-3);
    // Next visit: the page hands the save back as `save`.
    let mut world = StationEscape::new(StationEscapeSpec::default()).world_value();
    world["save"] = saved;
    let mut g = StationEscape::from_json(&world.to_string()).unwrap();
    g.confirm();
    assert!(g.can_pick());
    g.walk(1.0, 0.0, false);
    g.tick(DT);
    assert_eq!(g.stage, 1, "right picks the next stage reached");
    g.walk(0.0, 0.0, false);
    g.tick(DT);
    g.walk(1.0, 0.0, false);
    g.tick(DT);
    assert_eq!(g.stage, 0, "stage 3 is not reached: wraps to 1");
    let hud = g.hud(390, 844, false);
    assert!(
        hud.texts.iter().any(|t| t.text.contains("0:41")),
        "the record shows"
    );
    // A save that does not fit is ignored, not an error.
    world["save"] = json!({"cleared": "lots", "best_secs": [-1, "x"]});
    let g = StationEscape::from_json(&world.to_string()).unwrap();
    assert_eq!(g.progress.cleared, 0);
    assert!((0..5).all(|i| g.progress.record(i).is_none()));
}

#[test]
fn the_exit_stays_shut_until_the_warden_is_down() {
    let mut g = running();
    g.load_stage(4);
    g.beat = Beat::Run;
    let e = g.st().exit;
    g.body.reset_to(e[0], e[1]);
    key(&mut g, 0.0, false, false, 2);
    assert_ne!(g.beat, Beat::Clear, "shut");
    g.warden.as_mut().unwrap().hp = 1;
    assert!(boss_bot(&mut g, 60.0), "the bot brings it down and leaves");
    let v = g.state_value();
    assert_eq!(v["boss_hp"], 0);
    assert!(g.tallies.last().unwrap().score >= g.spec.boss_score);
}

#[test]
fn the_warden_is_in_sight_from_the_start_of_the_fight() {
    let mut g = running();
    g.load_stage(4);
    g.beat = Beat::Run;
    for _ in 0..90 {
        key(&mut g, 0.0, false, false, 1);
        if matches!(g.beat, Beat::Down(_)) {
            break;
        }
    }
    let w = g.warden.clone().unwrap();
    let _ = g.hud(390, 844, false);
    g.rebuild();
    let cam = &g.document().cameras[0];
    let view = View {
        eye: cam.position,
        target: cam.target,
        fov: cam.fov,
    };
    let (sx, sy) = view.project([w.x, w.y, 0.0], 390.0, 844.0).unwrap();
    assert!(
        (0.0..=390.0).contains(&sx) && (0.0..=844.0).contains(&sy),
        "warden at ({sx}, {sy})"
    );
}

#[test]
fn pause_freezes_the_stage() {
    let mut g = running();
    key(&mut g, 1.0, false, false, 10);
    g.pause_run();
    let (t, x) = (g.t, g.body.x);
    key(&mut g, 1.0, false, false, 30);
    assert_eq!((g.t, g.body.x), (t, x));
    g.resume_run();
    key(&mut g, 1.0, false, false, 2);
    assert!(g.t > t);
}

#[test]
fn the_baked_fixture_is_the_default_world() {
    let want = StationEscape::new(StationEscapeSpec::default()).world_value();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/station_escape_world.json"
    );
    if std::env::var("KAGRA_BAKE").is_ok() {
        std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
    }
    let got: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default())
        .unwrap_or(Value::Null);
    assert_eq!(
        got, want,
        "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib station_escape::` to refresh it"
    );
    assert!(is_station_escape_dump(&got.to_string()));
    let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
    assert_eq!(loaded.name(), GAME_ID, "the session finds this kit");
}

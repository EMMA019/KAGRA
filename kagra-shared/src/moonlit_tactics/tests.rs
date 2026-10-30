use super::*;
use crate::kit_host::KitHost;

const DT: f32 = 1.0 / 60.0;

fn started() -> MoonlitTactics {
    let mut g = MoonlitTactics::new(MoonlitTacticsSpec::default());
    let _ = g.hud(390, 844, false);
    g.confirm();
    g
}

/// Run the clock until nothing is playing out (or a cap).
fn settle(g: &mut MoonlitTactics) {
    for _ in 0..3000 {
        g.tick(DT);
        if g.anims.is_empty() && g.mode != Mode::Enemy {
            break;
        }
    }
}

/// Past the story card into the first player phase.
fn playing() -> MoonlitTactics {
    let mut g = started();
    g.advance();
    settle(&mut g);
    assert_eq!(g.mode, Mode::Player);
    g
}

fn tap_screen(g: &mut MoonlitTactics, x: f32, y: f32) {
    g.touch(
        PointerEvent {
            id: 1,
            x,
            y,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        },
        390,
        844,
        0.0,
    );
}

/// Where a cell's middle is on a 390 x 844 screen.
fn cell_on_screen(g: &MoonlitTactics, c: Cell) -> (f32, f32) {
    let p = doc::cell_pos(&g.board, c, g.spec.tile);
    g.view().project(p, 390.0, 844.0).unwrap()
}

/// How much the dead could hurt hero `h` standing on `cell` next turn (a rough reach test:
/// a foe within its move plus its range), leaving out `spare` (a foe about to fall).
fn threat(b: &Board, h: usize, cell: Cell, spare: Option<usize>) -> i32 {
    b.living(Side::Foe)
        .filter(|&f| Some(f) != spare)
        .filter(|&f| {
            let k = b.class(f);
            grid::manhattan(b.units[f].at, cell) <= k.mv + k.range_max
        })
        .map(|f| (b.class(f).atk - b.class(h).def - b.cover(cell)).max(1))
        .sum()
}

/// A careful commander's choice for hero `h`: the best act it can reach, weighed against how
/// exposed the cell leaves it; with nothing worth doing, the safest cell.
fn careful_plan(b: &Board, h: usize) -> (Cell, Option<usize>) {
    let mut best: Option<(i32, Cell, Option<usize>)> = None;
    let hp = b.units[h].hp;
    for (cell, cost) in b.reach(h) {
        let mut options: Vec<(i32, Option<usize>)> = vec![(0, None)];
        for t in b.targets_from(h, cell) {
            if b.units[t].side == Side::Hero {
                let max = b.classes[b.units[t].class].hp;
                options.push(((max - b.units[t].hp).min(b.class(h).heal) * 8, Some(t)));
            } else {
                let ex = b.forecast(h, t, cell);
                let mut v = ex.dealt * 10 - ex.countered.unwrap_or(0) * 8;
                if ex.killed {
                    v += 120;
                }
                if b.units[t].leader {
                    v += 30;
                }
                options.push((v, Some(t)));
            }
        }
        for (v, t) in options {
            let spare =
                t.filter(|&t| b.units[t].side == Side::Foe && b.forecast(h, t, cell).killed);
            let danger = threat(b, h, cell, spare);
            let mut score = v - danger * 6 - cost;
            if danger >= hp {
                score -= 300;
            }
            if best.is_none_or(|(s, _, _)| score > s) {
                best = Some((score, cell, t));
            }
        }
    }
    best.map_or((b.units[h].at, None), |(_, c, t)| (c, t))
}

/// A greedy commander for the heroes: each one takes the best attack (or heal) it can reach,
/// the same way the dead choose, through real taps. Returns true when the map is won.
fn play_map(g: &mut MoonlitTactics, max_turns: u32) -> bool {
    while g.turn <= max_turns {
        match g.mode {
            Mode::Won => return true,
            Mode::Lost => return false,
            Mode::Player => {}
            _ => {
                settle(g);
                continue;
            }
        }
        let ready: Vec<usize> = g.board.living(Side::Hero).filter(|&h| g.ready(h)).collect();
        if std::env::var("MT_DEBUG").is_ok() && ready.len() == g.board.living(Side::Hero).count() {
            let show = |side| {
                g.board
                    .living(side)
                    .map(|u| {
                        format!(
                            "{}{:?}:{}",
                            g.board.class(u).id,
                            g.board.units[u].at,
                            g.board.units[u].hp
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            eprintln!(
                "turn {}: heroes {} | foes {}",
                g.turn,
                show(Side::Hero),
                show(Side::Foe)
            );
        }
        if ready.is_empty() {
            settle(g);
            continue;
        }
        let h = ready[0];
        let (dest, target) = careful_plan(&g.board, h);
        let (sx, sy) = cell_on_screen(g, g.board.units[h].at);
        tap_screen(g, sx, sy);
        assert_eq!(g.sel, Some(h), "the tap selected the hero");
        if dest != g.board.units[h].at {
            let (dx, dy) = cell_on_screen(g, dest);
            tap_screen(g, dx, dy);
            settle(g);
        }
        match target {
            Some(t) => {
                let (tx, ty) = cell_on_screen(g, g.board.units[t].at);
                tap_screen(g, tx, ty);
            }
            None => g.press(Btn::Wait),
        }
        settle(g);
    }
    g.mode == Mode::Won
}

#[test]
fn the_default_spec_is_valid_and_strict() {
    MoonlitTacticsSpec::default().validate().unwrap();
    let v = MoonlitTactics::new(MoonlitTacticsSpec::default()).world_value()["game"].clone();
    let mut bad = v.clone();
    bad["bogus"] = json!(1);
    assert!(MoonlitTacticsSpec::from_value(&bad).is_err());
    let mut bad = v.clone();
    bad["maps"][0]["units"][0]["class"] = json!("dragon");
    assert!(MoonlitTacticsSpec::from_value(&bad).is_err());
    let mut bad = v;
    bad["maps"][0]["rows"][0] = json!("..X....");
    assert!(MoonlitTacticsSpec::from_value(&bad).is_err());
}

#[test]
fn tap_select_move_and_attack_through_the_screen() {
    let mut g = playing();
    let h = g.board.living(Side::Hero).next().unwrap();
    let (sx, sy) = cell_on_screen(&g, g.board.units[h].at);
    tap_screen(&mut g, sx, sy);
    assert_eq!(g.sel, Some(h));
    assert!(g.reach.len() > 3, "blue cells");
    // Move one cell up, if it is free.
    let at = g.board.units[h].at;
    let dest = g.reach.iter().map(|(c, _)| *c).find(|c| *c != at).unwrap();
    let (dx, dy) = cell_on_screen(&g, dest);
    tap_screen(&mut g, dx, dy);
    assert_eq!(g.board.units[h].at, dest);
    assert!(g.moved_from.is_some());
    // Undo puts it back and keeps it selected.
    settle(&mut g);
    g.press(Btn::Undo);
    assert_eq!(g.board.units[h].at, at);
    assert_eq!(g.sel, Some(h));
    // A foe next door is attacked with one tap (no move).
    let f = g.board.living(Side::Foe).next().unwrap();
    let next_to = (at.0, at.1 - 1);
    g.board.units[f].at = next_to;
    let hp = g.board.units[f].hp;
    let (fx, fy) = cell_on_screen(&g, next_to);
    tap_screen(&mut g, fx, fy);
    assert!(g.board.units[f].hp < hp, "struck");
    assert!(g.board.units[h].acted && g.sel.is_none());
    assert!(!g.anims.is_empty(), "the strike plays out");
}

#[test]
fn ending_the_turn_lets_the_dead_move_then_it_is_turn_two() {
    let mut g = playing();
    let before: Vec<Cell> = g
        .board
        .living(Side::Foe)
        .map(|f| g.board.units[f].at)
        .collect();
    let r = g
        .buttons(390.0, 844.0)
        .into_iter()
        .find(|(b, _)| *b == Btn::EndTurn)
        .unwrap()
        .1;
    tap_screen(&mut g, r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    settle(&mut g);
    assert_eq!(g.turn, 2);
    assert_eq!(g.mode, Mode::Player);
    let after: Vec<Cell> = g
        .board
        .living(Side::Foe)
        .map(|f| g.board.units[f].at)
        .collect();
    assert_ne!(before, after, "the dead moved");
    assert!(
        g.board.living(Side::Hero).all(|h| !g.board.units[h].acted),
        "a fresh turn"
    );
}

#[test]
fn a_commander_wins_both_maps_and_the_run_ends() {
    let mut g = playing();
    assert!(
        play_map(&mut g, 20),
        "map 1 lost or stalled at turn {}",
        g.turn
    );
    let t1 = *g.tallies.last().unwrap();
    eprintln!(
        "map 1: {} turns, {} standing, {} kills",
        t1.turns, t1.survivors, t1.kills
    );
    g.advance();
    assert_eq!((g.map_i, g.mode), (1, Mode::Intro));
    g.advance();
    settle(&mut g);
    assert!(
        play_map(&mut g, 25),
        "map 2 lost or stalled at turn {}",
        g.turn
    );
    let t2 = *g.tallies.last().unwrap();
    eprintln!(
        "map 2: {} turns, {} standing, {} kills",
        t2.turns, t2.survivors, t2.kills
    );
    g.advance();
    for _ in 0..30 {
        g.tick(DT);
    }
    assert_eq!(g.phase_name(), "result");
    assert_eq!(g.state_value()["win"], true);
    assert!(g.core.stats.score >= 2 * g.spec.map_score);
}

#[test]
fn the_keeper_falls_and_the_map_is_won() {
    let mut g = playing();
    g.load_map(1);
    g.advance();
    settle(&mut g);
    let k = g.board.units.iter().position(|u| u.leader).unwrap();
    g.board.units[k].hp = 0;
    g.tick(DT);
    assert_eq!(g.mode, Mode::Won);
}

#[test]
fn the_device_keeps_the_maps_won_and_the_fewest_turns() {
    let mut g = playing();
    assert_eq!(
        g.save_json().unwrap(),
        json!({"cleared": 0, "best_turns": [null, null]}).to_string()
    );
    g.turn = 6;
    for f in g.board.living(Side::Foe).collect::<Vec<_>>() {
        g.board.units[f].hp = 0;
    }
    g.tick(DT);
    assert_eq!(g.mode, Mode::Won);
    assert!(g.new_record);
    let saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    assert_eq!(saved, json!({"cleared": 1, "best_turns": [6.0, null]}));
    // Next visit: the page hands the save back; the first card opens the map select.
    let mut world = MoonlitTactics::new(MoonlitTacticsSpec::default()).world_value();
    world["save"] = saved;
    let mut g = MoonlitTactics::from_json(&world.to_string()).unwrap();
    let _ = g.hud(390, 844, false);
    g.confirm();
    assert!(g.can_pick());
    let hud = g.hud(390, 844, false);
    assert!(hud.texts.iter().any(|t| t.text.contains("FEWEST TURNS 6")));
    assert!(hud.texts.iter().any(|t| t.text == "▶"));
    tap_screen(&mut g, 380.0, 420.0);
    assert_eq!(
        (g.map_i, g.mode),
        (1, Mode::Intro),
        "a tap on the right picks map 2"
    );
    g.walk(1.0, 0.0, false);
    assert_eq!(g.map_i, 0, "the keys wrap round the maps reached");
    tap_screen(&mut g, 195.0, 420.0);
    assert_eq!(g.mode, Mode::Player, "a tap in the middle begins");
    // A link can start on map 2; an odd save is ignored.
    world["save"] = json!({"cleared": "lots"});
    world["map"] = json!("2");
    let g = MoonlitTactics::from_json(&world.to_string()).unwrap();
    assert_eq!((g.map_i, g.progress.cleared), (1, 0));
}

#[test]
fn losing_everyone_ends_the_run() {
    let mut g = playing();
    for h in g.board.living(Side::Hero).collect::<Vec<_>>() {
        g.board.units[h].hp = 0;
    }
    g.tick(DT);
    assert_eq!(g.mode, Mode::Lost);
    g.advance();
    for _ in 0..30 {
        g.tick(DT);
    }
    assert_eq!(g.phase_name(), "result");
    assert_eq!(g.state_value()["win"], false);
}

#[test]
fn the_board_fits_between_the_bars_either_way_up() {
    let mut g = playing();
    for (w, h) in [(390_u32, 844_u32), (844, 390)] {
        let _ = g.hud(w, h, false);
        g.rebuild();
        let (bw, bh) = g.board.size();
        let s = hud_scale(w as f32, h as f32);
        for c in [(0, 0), (bw - 1, 0), (0, bh - 1), (bw - 1, bh - 1)] {
            let p = doc::cell_pos(&g.board, c, g.spec.tile);
            let (sx, sy) = g.view().project(p, w as f32, h as f32).unwrap();
            assert!(
                (0.0..=w as f32).contains(&sx) && sy > 70.0 * s && sy < h as f32 - 60.0 * s,
                "{w}x{h}: cell {c:?} at ({sx}, {sy})"
            );
        }
    }
}

#[test]
fn the_gpu_model_set_never_changes_in_a_map() {
    let mut g = playing();
    g.rebuild();
    let key = g.document().gpu_mesh_key();
    play_map(&mut g, 4);
    g.rebuild();
    assert_eq!(g.document().gpu_mesh_key(), key);
}

#[test]
fn the_baked_fixture_is_the_default_world() {
    let want = MoonlitTactics::new(MoonlitTacticsSpec::default()).world_value();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/moonlit_tactics_world.json"
    );
    if std::env::var("KAGRA_BAKE").is_ok() {
        std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
    }
    let got: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default())
        .unwrap_or(Value::Null);
    assert_eq!(
        got, want,
        "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib moonlit_tactics::` to refresh it"
    );
    assert!(is_moonlit_tactics_dump(&got.to_string()));
    let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
    assert_eq!(loaded.name(), GAME_ID, "the session finds this kit");
}

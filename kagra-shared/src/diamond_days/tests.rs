use super::*;
use crate::kit_host::KitHost;
use team::Stat;

const DT: f32 = 1.0 / 60.0;
const W: u32 = 390;
const H: u32 = 844;

fn started() -> DiamondDays {
    let mut g = DiamondDays::new(DiamondDaysSpec::default());
    let _ = g.hud(W, H, false);
    g.confirm();
    g
}

fn tap(g: &mut DiamondDays, x: f32, y: f32) {
    g.touch(
        PointerEvent {
            id: 1,
            x,
            y,
            phase: PointerPhase::Begin,
            pressure: 1.0,
        },
        W,
        H,
        0.0,
    );
}

fn press(g: &mut DiamondDays, b: hud::Btn) {
    let r = g
        .hud_buttons(W as f32, H as f32)
        .into_iter()
        .find(|(x, _)| *x == b)
        .map(|(_, r)| r)
        .unwrap_or_else(|| panic!("no {b:?} in {:?}", g.mode));
    tap(g, r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
}

/// Every text in the HUD, layers included.
fn all_texts(list: &crate::scene::DrawList) -> impl Iterator<Item = &crate::scene::TextQuad> {
    list.flatten().into_iter().flat_map(|l| l.texts.iter())
}

fn run(g: &mut DiamondDays, secs: f32) {
    for _ in 0..(secs / DT) as usize {
        g.tick(DT);
    }
}

/// Play one week through the screen: clear notices, pick the first card or watch a game
/// (with orders left to the team), and wait for the next week.
fn play_week(g: &mut DiamondDays) {
    let week = (g.s.year, g.s.week);
    for _ in 0..4000 {
        match g.mode {
            Mode::Notice => tap(g, 195.0, 400.0),
            Mode::Plan if (g.s.year, g.s.week) != week => return,
            Mode::Plan => {
                for p in &mut g.s.team.players {
                    p.auto_raise();
                }
                press(g, hud::Btn::Card(0))
            }
            Mode::Match {
                asking: Some(_), ..
            } => press(g, hud::Btn::Call(None)),
            Mode::Player(_) => press(g, hud::Btn::Back),
            Mode::Match { .. } => press(g, hud::Btn::Skip),
            Mode::Roster => tap(g, 10.0, 10.0),
            Mode::Walk { .. } => run(g, 0.3),
        }
        if g.flow.phase != FlowPhase::Play {
            return;
        }
    }
    panic!("stuck in {:?}", g.mode);
}

#[test]
fn the_default_spec_is_valid_and_strict() {
    let spec = DiamondDaysSpec::default();
    assert!(spec.validate().is_ok());
    let mut v = serde_json::to_value(&spec).unwrap();
    v["ring"] = json!(["rest", "rest"]);
    assert!(DiamondDaysSpec::from_value(&v).is_err());
    let mut v = serde_json::to_value(&spec).unwrap();
    v["oops"] = json!(1);
    assert!(DiamondDaysSpec::from_value(&v).is_err());
}

#[test]
fn a_card_walks_the_coach_and_the_square_happens() {
    let mut g = started();
    assert_eq!(g.mode, Mode::Notice, "the welcome card");
    tap(&mut g, 195.0, 400.0);
    assert_eq!(g.mode, Mode::Plan);
    let steps = g.s.hand[0].steps as usize;
    let xp_before: u32 = g
        .s
        .team
        .players
        .iter()
        .map(|p| p.xp.iter().map(|&x| x as u32).sum::<u32>() + p.contact as u32 + p.speed as u32)
        .sum();
    press(&mut g, hud::Btn::Card(0));
    assert!(matches!(g.mode, Mode::Walk { .. }));
    assert_eq!(g.s.hand.len(), 5, "a new card is drawn");
    run(&mut g, 2.0);
    assert_eq!(g.s.pos, steps % g.spec.ring.len());
    assert_eq!(g.mode, Mode::Notice);
    assert_eq!(g.s.week, 2, "the week moved on");
    let sq = g.spec.ring[g.s.pos];
    if let Square::Train(_) = sq {
        let after: u32 =
            g.s.team
                .players
                .iter()
                .map(|p| {
                    p.xp.iter().map(|&x| x as u32).sum::<u32>() + p.contact as u32 + p.speed as u32
                })
                .sum();
        assert!(after > xp_before);
    }
    let hud = g.hud(W, H, false);
    assert!(all_texts(&hud).next().is_some());
}

#[test]
fn a_game_stops_at_chances_and_pinches_for_the_coachs_call() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    // Jump to the week of the first practice game.
    g.s.week = 6;
    g.begin_week();
    assert!(matches!(g.mode, Mode::Match { .. }));
    let mut calls = 0;
    for _ in 0..20000 {
        match g.mode {
            Mode::Match {
                asking: Some(spot), ..
            } => {
                let hud = g.hud(W, H, true);
                let prompt = match spot {
                    Spot::Chance => "chance",
                    Spot::Pinch => "pinch",
                };
                assert!(all_texts(&hud).any(|t| t.text.contains(prompt)));
                // Take the first call on offer.
                let first = g
                    .hud_buttons(W as f32, H as f32)
                    .into_iter()
                    .find_map(|(b, _)| matches!(b, hud::Btn::Call(Some(_))).then_some(b))
                    .unwrap();
                press(&mut g, first);
                calls += 1;
            }
            Mode::Match { .. } => run(&mut g, 0.3),
            _ => break,
        }
    }
    assert!(calls >= 1, "at least one chance or pinch in a game");
    assert!(g.game.is_none());
    assert_eq!(g.mode, Mode::Notice);
    assert_eq!(g.s.wins + g.s.losses, 1);
    assert_eq!(g.s.week, 7);
}

#[test]
fn the_coach_spends_points_and_sets_each_players_practice() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.team.players[0].xp = [40; 5];
    press(&mut g, hud::Btn::Roster);
    press(&mut g, hud::Btn::Row(0));
    assert_eq!(g.mode, Mode::Player(0));
    let p0 = g.s.team.players[0].clone();
    let st = if p0.pitcher() {
        Stat::Control
    } else {
        Stat::Contact
    };
    press(&mut g, hud::Btn::Raise(st));
    assert_eq!(g.s.team.players[0].stat(st), p0.stat(st) + 1);
    press(&mut g, hud::Btn::Menu(team::Drill::Running));
    assert_eq!(g.s.team.players[0].menu, team::Drill::Running);
    press(&mut g, hud::Btn::AutoRaise);
    assert!(Stat::ALL
        .iter()
        .all(|&st| !g.s.team.players[0].can_raise(st)));
    let hud = g.hud(W, H, false);
    assert!(all_texts(&hud).any(|t| t.text.contains(&p0.given)));
    press(&mut g, hud::Btn::Back);
    assert_eq!(g.mode, Mode::Roster);
}

#[test]
fn scouting_brings_a_middle_schooler_next_april() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    let scout = g
        .spec
        .ring
        .iter()
        .position(|s| *s == Square::Scout)
        .unwrap();
    g.s.pos = (scout + g.spec.ring.len() - 1) % g.spec.ring.len();
    g.s.hand[0].steps = 1;
    press(&mut g, hud::Btn::Card(0));
    run(&mut g, 1.0);
    assert_eq!(g.s.scouted.len(), 1);
    let name = g.s.scouted[0].full_name();
    g.s.week = board::WEEKS;
    g.end_week();
    assert!(g
        .s
        .team
        .players
        .iter()
        .any(|p| p.full_name() == name || p.given == g.s.team.players.last().unwrap().given));
    assert!(g.s.scouted.is_empty());
    assert_eq!(g.s.year, 2);
}

#[test]
fn a_whole_year_turns_over_and_three_years_end_the_run() {
    let mut g = started();
    let founding = g.s.team.players.len();
    let thirds = g.s.team.players.iter().filter(|p| p.grade == 3).count();
    let mut weeks = 0;
    while g.s.year == 1 && weeks < 60 {
        play_week(&mut g);
        weeks += 1;
    }
    assert_eq!(g.s.year, 2, "a year is 48 weeks: {weeks}");
    assert!(
        g.s.wins + g.s.losses >= 5,
        "practice games and tournaments played"
    );
    assert_eq!(g.s.results.len(), 1, "the year's result is kept");
    assert!(g.s.team.players.len() >= founding - thirds + 4);
    assert!(g.s.team.players.iter().all(|p| (1..=3).contains(&p.grade)));
    let mut guard = 0;
    while g.flow.phase == FlowPhase::Play && guard < 200 {
        play_week(&mut g);
        guard += 1;
    }
    assert_eq!(g.phase_name(), "result", "graduation after three years");
    assert!(g.flow.score > 0);
}

#[test]
fn the_run_is_kept_on_the_device_and_resumes() {
    let mut g = started();
    for _ in 0..3 {
        play_week(&mut g);
    }
    let saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    assert_eq!(saved["week"], g.s.week);
    let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    world["save"] = saved;
    let mut h = DiamondDays::from_json(&world.to_string()).unwrap();
    h.confirm();
    assert_eq!(h.s.week, g.s.week);
    assert_eq!(h.s.team, g.s.team);
    // A save that does not fit starts fresh.
    world["save"] = json!({"year": "x"});
    let mut k = DiamondDays::from_json(&world.to_string()).unwrap();
    k.confirm();
    assert_eq!((k.s.year, k.s.week), (1, 1));
}

#[test]
fn a_link_can_start_at_a_game_week() {
    let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    world["week"] = json!("6");
    let mut g = DiamondDays::from_json(&world.to_string()).unwrap();
    g.confirm();
    assert_eq!(g.s.week, 6);
    tap(&mut g, 195.0, 400.0);
    assert!(matches!(g.mode, Mode::Match { .. }), "{:?}", g.mode);
}

#[test]
fn an_injury_lasts_its_weeks_and_saves_keep_the_week_whole() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    // Hurt this week for two weeks: still out next week, back the week after.
    g.s.team.players[0].hurt = 2;
    g.s.healed = (g.s.year, g.s.week);
    g.end_week();
    g.begin_week();
    assert_eq!(g.s.team.players[0].hurt, 1, "out next week too");
    g.end_week();
    g.begin_week();
    assert_eq!(g.s.team.players[0].hurt, 0);
    // A save while the coach walks keeps the week as it was before the card.
    g.mode = Mode::Plan;
    let (pos, hand) = (g.s.pos, g.s.hand.clone());
    press(&mut g, hud::Btn::Card(0));
    g.tick(DT);
    let saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    assert_eq!(saved["pos"], pos);
    assert_eq!(saved["hand"], serde_json::to_value(&hand).unwrap());
    run(&mut g, 2.0);
    let saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    assert_eq!(saved["pos"], g.s.pos, "after landing the save moves on");
}

#[test]
fn the_title_screen_keeps_a_saved_run_and_calls_come_back_in_a_new_run() {
    let mut g = started();
    for _ in 0..2 {
        play_week(&mut g);
    }
    let saved = g.save_json().unwrap();
    let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    world["save"] = serde_json::from_str(&saved).unwrap();
    let h = DiamondDays::from_json(&world.to_string()).unwrap();
    assert_eq!(h.phase_name(), "title");
    assert_eq!(
        h.save_json().unwrap(),
        saved,
        "the title keeps the run on the device"
    );
    let fresh = DiamondDays::new(DiamondDaysSpec::default());
    assert_eq!(
        fresh.save_json(),
        None,
        "nothing to keep: the device copy is left alone"
    );
    // A new run asks for calls again.
    g.auto_orders = true;
    g.flow.finish(0, 0, "x");
    g.confirm();
    assert!(!g.auto_orders);
}

#[test]
fn the_roster_opens_and_scrolls() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    press(&mut g, hud::Btn::Roster);
    assert_eq!(g.mode, Mode::Roster);
    let hud = g.hud(W, H, false);
    let first = &g.s.team.players[0].family;
    assert!(all_texts(&hud).any(|t| t.text.contains(first.as_str())));
    press(&mut g, hud::Btn::RosterDown);
    assert_eq!(g.roster_scroll, 4);
    // A tap outside the list closes it.
    tap(&mut g, 4.0, 400.0);
    assert_eq!(g.mode, Mode::Plan);
}

#[test]
fn the_model_set_stays_fixed_through_a_week() {
    let mut g = started();
    let key = g.document().gpu_mesh_key();
    for _ in 0..3 {
        play_week(&mut g);
        assert_eq!(g.document().gpu_mesh_key(), key);
    }
}

#[test]
fn the_baked_fixture_is_the_default_world() {
    let want = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/diamond_days_world.json"
    );
    if std::env::var("KAGRA_BAKE").is_ok() {
        std::fs::write(path, serde_json::to_string_pretty(&want).unwrap() + "\n").unwrap();
    }
    let got: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_default())
        .unwrap_or(Value::Null);
    assert_eq!(
        got, want,
        "run `KAGRA_BAKE=1 cargo test -p kagra-shared --lib diamond_days::` to refresh it"
    );
    assert!(is_diamond_days_dump(&got.to_string()));
    let loaded = crate::kit_host::load(&got.to_string()).unwrap().unwrap();
    assert_eq!(loaded.name(), GAME_ID, "the session finds this kit");
}

/// Three years played by a bot that always takes the first card: how far it gets.
/// `cargo test -p kagra-shared --lib diamond_days::tests::balance_probe -- --ignored --nocapture`
#[test]
#[ignore]
fn balance_probe() {
    for seed in 0..4 {
        let mut g = DiamondDays::new(DiamondDaysSpec {
            seed,
            ..DiamondDaysSpec::default()
        });
        g.confirm();
        let mut strengths = Vec::new();
        let mut guard = 0;
        while g.flow.phase == FlowPhase::Play && guard < 200 {
            if g.s.week == 12 {
                strengths.push(g.s.team.strength());
            }
            play_week(&mut g);
            guard += 1;
        }
        eprintln!(
            "seed {seed}: {}W {}L rep {:.0} strength {:?} results {:?}",
            g.s.wins,
            g.s.losses,
            g.s.reputation,
            strengths.iter().map(|s| s.round()).collect::<Vec<_>>(),
            g.s.results.iter().map(|r| r.0.clone()).collect::<Vec<_>>()
        );
    }
}

/// Pictures of every screen for review (`KAGRA_SHOTS=dir cargo test -p kagra-shared --lib
/// diamond_days::tests::screens_to_png`), drawn on the CPU at phone and desktop sizes.
#[test]
fn screens_to_png() {
    let Ok(dir) = std::env::var("KAGRA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shot = |g: &DiamondDays, name: &str| {
        for (w, h, tag) in [(390u32, 844u32, "phone"), (1280, 720, "desk")] {
            let t0 = std::time::Instant::now();
            // As a native shell draws it: the game's own title, pause and result cards.
            let list = g.hud(w, h, true);
            let built = t0.elapsed();
            let (q, t, x) = list.counts();
            eprintln!("{name} {tag}: {t} triangles, {q} quads, {x} texts, built in {built:?}");
            let pic = crate::raster2d::picture(&list, w, h);
            pic.save_png(&dir.join(format!("{name}_{tag}.png")))
                .unwrap();
        }
    };
    let mut g = DiamondDays::new(DiamondDaysSpec::default());
    g.set_lang("ja");
    shot(&g, "00_title");
    g.confirm();
    run(&mut g, 0.5);
    shot(&g, "01_notice");
    tap(&mut g, 195.0, 400.0);
    // A few weeks in.
    for _ in 0..3 {
        play_week(&mut g);
    }
    run(&mut g, 0.5);
    shot(&g, "02_plan");
    press(&mut g, hud::Btn::Card(0));
    run(&mut g, 0.3);
    shot(&g, "03_walk");
    for _ in 0..600 {
        if g.mode == Mode::Notice {
            break;
        }
        run(&mut g, 0.1);
    }
    shot(&g, "04_week_notice");
    while g.mode == Mode::Notice {
        tap(&mut g, 195.0, 400.0);
    }
    press(&mut g, hud::Btn::Roster);
    shot(&g, "05_roster");
    for p in &mut g.s.team.players {
        p.xp = [60, 50, 40, 30, 20];
    }
    press(&mut g, hud::Btn::Row(0));
    shot(&g, "06_player");
    press(&mut g, hud::Btn::Back);
    press(&mut g, hud::Btn::Close);
    g.s.week = 6;
    g.begin_week();
    let mut asked = false;
    let mut mid = false;
    for _ in 0..20000 {
        match g.mode {
            Mode::Match {
                asking: Some(_), ..
            } => {
                if !asked {
                    shot(&g, "08_call");
                    asked = true;
                }
                press(&mut g, hud::Btn::Call(None));
            }
            Mode::Match { shown, t, .. } => {
                let gm = g.game.as_ref().unwrap();
                if !mid
                    && gm.inning >= 3
                    && t > 0.2
                    && matches!(gm.log[shown.saturating_sub(1)].kind, sim::PlayKind::Hit(_))
                {
                    shot(&g, "07_match");
                    mid = true;
                }
                run(&mut g, 0.05);
            }
            _ => break,
        }
        if asked && mid {
            break;
        }
    }
    press(&mut g, hud::Btn::Skip);
    shot(&g, "09_result");
    g.pause();
    shot(&g, "10_pause");
    g.flow.resume();
    g.s.results = vec![
        ("Summer round 3".into(), "夏 3回戦敗退".into()),
        ("Prefectural champions".into(), "県大会優勝".into()),
        ("Summer Stadium champions".into(), "夏の大舞台 優勝".into()),
    ];
    g.s.wins = 41;
    g.s.losses = 12;
    g.finish_run();
    shot(&g, "11_record");
    // A kept run on the title.
    let mut back = DiamondDays::new(DiamondDaysSpec::default());
    back.set_lang("ja");
    back.resume = Some(g.s.clone());
    back.resume.as_mut().unwrap().year = 2;
    back.resume.as_mut().unwrap().week = 14;
    shot(&back, "12_title_continue");
}

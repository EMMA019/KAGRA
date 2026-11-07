use super::*;
use crate::kit_host::KitHost;

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
            Mode::Plan => press(g, hud::Btn::Card(0)),
            Mode::Match { asking: true, .. } => press(g, hud::Btn::Order(None)),
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
    assert!(!hud.texts.is_empty());
}

#[test]
fn practice_game_week_plays_a_game_with_orders() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    // Jump to the week before the first practice game.
    g.s.week = 6;
    g.begin_week();
    assert!(matches!(g.mode, Mode::Match { .. }));
    // Watch until we bat and the coach is asked.
    for _ in 0..2000 {
        if matches!(g.mode, Mode::Match { asking: true, .. }) {
            break;
        }
        run(&mut g, 0.2);
    }
    assert!(matches!(g.mode, Mode::Match { asking: true, .. }));
    let hud = g.hud(W, H, true);
    assert!(hud.texts.iter().any(|t| t.text.contains("order")));
    press(&mut g, hud::Btn::Order(Some(Order::Small)));
    assert_eq!(g.game.as_ref().unwrap().order, Order::Small);
    press(&mut g, hud::Btn::Skip);
    assert!(g.game.is_none());
    assert_eq!(g.mode, Mode::Notice);
    assert_eq!(g.s.wins + g.s.losses, 1);
    assert_eq!(g.s.week, 7);
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
fn the_roster_opens_and_scrolls() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    press(&mut g, hud::Btn::Roster);
    assert_eq!(g.mode, Mode::Roster);
    let hud = g.hud(W, H, false);
    let first = &g.s.team.players[0].family;
    assert!(hud.texts.iter().any(|t| t.text.contains(first.as_str())));
    press(&mut g, hud::Btn::RosterDown);
    assert_eq!(g.roster_scroll, 4);
    tap(&mut g, 195.0, 400.0);
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

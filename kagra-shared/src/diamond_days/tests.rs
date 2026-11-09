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

fn touch(g: &mut DiamondDays, x: f32, y: f32, phase: PointerPhase) {
    g.touch(
        PointerEvent {
            id: 1,
            x,
            y,
            phase,
            pressure: 1.0,
        },
        W,
        H,
        0.0,
    );
}

/// A finger down and up on one spot. A scene change takes no taps while it covers the
/// screen, so wait for it first, as a player does.
fn tap(g: &mut DiamondDays, x: f32, y: f32) {
    for _ in 0..40 {
        if g.transition.as_ref().is_none_or(|t| t.openness() >= 0.6) {
            break;
        }
        run(g, 0.05);
    }
    touch(g, x, y, PointerPhase::Begin);
    touch(g, x, y, PointerPhase::End);
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

/// Skip a game to its last moment, then past it to the result.
fn skip_whole_game(g: &mut DiamondDays) {
    press(g, hud::Btn::Skip);
    assert!(
        matches!(g.mode, Mode::Match { .. }),
        "the skip stops on the last moment"
    );
    press(g, hud::Btn::Skip);
}

/// Play one week through the screen: clear notices, pick the first card or watch a game
/// (with orders left to the team), and wait for the next week.
fn play_week(g: &mut DiamondDays) {
    let week = (g.s.year, g.s.week);
    for _ in 0..4000 {
        match g.mode {
            Mode::Notice => tap(g, 195.0, 400.0),
            Mode::Ask { .. } => press(g, hud::Btn::Choose(0)),
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
            Mode::Lineup { pick: Some(_) } => press(g, hud::Btn::Back),
            Mode::Lineup { .. } => press(g, hud::Btn::Close),
            Mode::CampTalk { .. } => press(g, hud::Btn::Choose(0)),
            Mode::Epilogue { schools: true } => press(g, hud::Btn::Back),
            Mode::Epilogue { .. } => press(g, hud::Btn::EndStory),
            Mode::Walk { .. } => run(g, 0.3),
            Mode::MatchIntro => press(g, hud::Btn::PlayBall),
            Mode::MatchEnd => press(g, hud::Btn::Continue),
            Mode::Bracket | Mode::Decide(_) => press(g, hud::Btn::Choose(0)),
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
    // Game day is its own screen: the opponent and our order, then the coach starts it.
    assert_eq!(g.mode, Mode::MatchIntro);
    g.set_lang("en");
    let hud = g.hud(W, H, true);
    assert!(all_texts(&hud).any(|t| t.text == "PLAY BALL!"));
    // Their ace is on the card, and he is the one who starts.
    assert!(all_texts(&hud).any(|t| t.text == "THEIR ACE"));
    let ace = g.pending_rival.as_ref().unwrap().arms[0].name.clone();
    assert!(all_texts(&hud).any(|t| t.text == ace));
    press(&mut g, hud::Btn::PlayBall);
    assert!(matches!(g.mode, Mode::Match { .. }));
    let them = g.game.as_ref().map(|gm| 1 - gm.us).unwrap();
    assert_eq!(g.game.as_ref().unwrap().sides[them].arms[0].name, ace);
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
                    Spot::Bench(_) => {
                        press(&mut g, hud::Btn::Call(None));
                        continue;
                    }
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
    // The result screen, then back to practice.
    assert_eq!(g.mode, Mode::MatchEnd);
    let hud = g.hud(W, H, true);
    assert!(all_texts(&hud).any(|t| t.text == "VICTORY!" || t.text == "DEFEAT"));
    assert!(all_texts(&hud).any(|t| t.text.starts_with("Game experience +")));
    assert_eq!(g.s.wins + g.s.losses, 1);
    assert_eq!(
        g.s.week, 7,
        "the week already moved on (a save here is past the game)"
    );
    press(&mut g, hud::Btn::Continue);
    assert!(matches!(g.mode, Mode::Plan | Mode::Notice), "{:?}", g.mode);
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
fn a_whole_year_turns_over_and_the_coach_may_retire_after_three() {
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
    // `play_week` picks "retire" at the three-year milestone.
    assert_eq!(g.phase_name(), "result", "retired at the first milestone");
    assert_eq!(g.s.history.len(), 3, "each year is in the career record");
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
    assert_eq!(g.mode, Mode::MatchIntro);
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
    run(&mut g, 0.6);
    assert!(
        g.roster.offset > 50.0,
        "the button glides the list: {}",
        g.roster.offset
    );
    // A finger drags the list; a drag opens nothing.
    let before = g.roster.offset;
    touch(&mut g, 195.0, 600.0, PointerPhase::Begin);
    for k in 1..=10 {
        touch(&mut g, 195.0, 600.0 + k as f32 * 8.0, PointerPhase::Move);
    }
    touch(&mut g, 195.0, 680.0, PointerPhase::End);
    assert_eq!(g.mode, Mode::Roster);
    assert!(
        g.roster.offset < before - 40.0,
        "{} -> {}",
        before,
        g.roster.offset
    );
    run(&mut g, 2.0);
    assert!(g.roster.offset >= 0.0 && g.roster.offset <= g.roster.max() + 0.01);
    // A row shown in the window opens on a tap; rows scrolled away do not answer.
    let rows: Vec<_> = g
        .hud_buttons(W as f32, H as f32)
        .into_iter()
        .filter(|(b, _)| matches!(b, hud::Btn::Row(_)))
        .collect();
    assert!(!rows.is_empty());
    let view = hud::list_view(g.stage(W as f32, H as f32).h);
    let st = g.stage(W as f32, H as f32);
    let v = st.rect(view[0], view[1], view[2], view[3]);
    assert!(rows
        .iter()
        .all(|(_, r)| r[1] >= v[1] - 0.5 && r[1] + r[3] <= v[1] + v[3] + 0.5));
    let (b, r) = rows[0];
    tap(&mut g, r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    let hud::Btn::Row(i) = b else { unreachable!() };
    assert_eq!(g.mode, Mode::Player(i));
    press(&mut g, hud::Btn::Back);
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
        // Back on the board (a game in between builds the ballpark, then lets it go).
        run(&mut g, 0.05);
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
    // Past the curtain that opens back onto the board.
    run(&mut g, 1.6);
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
    // Back on the board in a quiet week (week 4 opens the spring tournament).
    g.notes.clear();
    g.s.cup = None;
    g.game = None;
    g.s.week = 2;
    g.mode = Mode::Plan;
    press(&mut g, hud::Btn::Roster);
    shot(&g, "05_roster");
    press(&mut g, hud::Btn::RosterDown);
    run(&mut g, 0.12);
    shot(&g, "05b_roster_moving");
    run(&mut g, 1.0);
    g.roster.offset = 0.0;
    for p in &mut g.s.team.players {
        p.xp = [60, 50, 40, 30, 20];
    }
    press(&mut g, hud::Btn::Row(0));
    shot(&g, "06_player");
    press(&mut g, hud::Btn::AutoRaise);
    run(&mut g, 0.35);
    shot(&g, "06b_growth");
    run(&mut g, 3.0);
    press(&mut g, hud::Btn::Back);
    press(&mut g, hud::Btn::Close);
    g.s.week = 6;
    g.begin_week();
    run(&mut g, 0.3);
    shot(&g, "07_curtain");
    run(&mut g, 1.2);
    shot(&g, "07a_game_day");
    press(&mut g, hud::Btn::PlayBall);
    let (mut asked, mut pitch, mut field) = (false, false, false);
    for _ in 0..40000 {
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
                let play = &gm.log[shown.saturating_sub(1)];
                let pre = g.pitch_time(shown);
                let n = play.pitches().len();
                // A pitch on its way in, the count already running.
                if !pitch
                    && gm.inning >= 2
                    && n >= 3
                    && t > PITCH_SECS * 2.6
                    && t < PITCH_SECS * 2.7
                {
                    shot(&g, "07b_pitch");
                    pitch = true;
                }
                // A hit, on the field.
                if !field
                    && gm.inning >= 3
                    && t > pre + 0.25
                    && matches!(play.kind, sim::PlayKind::Hit(_))
                {
                    shot(&g, "07_match");
                    field = true;
                }
                run(&mut g, 0.02);
            }
            _ => break,
        }
        if asked && pitch && field {
            break;
        }
    }
    press(&mut g, hud::Btn::Skip);
    shot(&g, "09_finale");
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
    // A long career: twelve years at two schools.
    let bests = [
        ("Summer round 1", "夏 1回戦敗退"),
        ("Summer round 3", "夏 3回戦敗退"),
        ("Prefectural champions", "県大会優勝"),
        ("Summer Stadium round 2", "夏の大舞台 2回戦"),
        ("Summer Stadium champions", "夏の大舞台 優勝"),
    ];
    g.s.history = (1..=12u32)
        .map(|y| {
            let (best, best_ja) = bests[(y as usize * 7 + 3) % bests.len()];
            YearLog {
                year: y,
                school: if y <= 6 { "Hinata" } else { "Minamino" }.into(),
                school_ja: if y <= 6 {
                    "日向高校"
                } else {
                    "南野高校"
                }
                .into(),
                best: best.into(),
                best_ja: best_ja.into(),
                wins: 10 + y % 4,
                losses: 4 - y % 3,
            }
        })
        .collect();
    g.s.career_wins = 120;
    g.s.career_losses = 40;
    g.s.chapter = 5;
    g.s.reputation = 72.0;
    // The milestone every three years.
    g.s.epilogue = true;
    g.notes.clear();
    g.mode = Mode::Epilogue { schools: false };
    shot(&g, "17_milestone");
    g.s.epilogue = false;
    // A talk on an event square.
    let mut talk = DiamondDays::new(DiamondDaysSpec::default());
    talk.set_lang("ja");
    talk.confirm();
    talk.notes.clear();
    talk.spec.happenings.retain(|h| h.id == "night_swing");
    talk.s.pos = talk
        .spec
        .ring
        .iter()
        .position(|s| *s == board::Square::Event)
        .unwrap();
    talk.land(board::Card {
        steps: 1,
        focus: None,
        star: None,
    });
    shot(&talk, "15_talk");
    press(&mut talk, hud::Btn::Choose(0));
    shot(&talk, "16_talk_reply");
    // The year's turn: graduation and the first-years, with faces.
    let mut spring = DiamondDays::new(DiamondDaysSpec::default());
    spring.set_lang("ja");
    spring.confirm();
    spring.notes.clear();
    spring.s.week = board::WEEKS;
    spring.end_week();
    assert!(spring
        .notes
        .iter()
        .any(|n| n.title == "Graduation" && !n.faces.is_empty()));
    assert!(spring
        .notes
        .iter()
        .any(|n| n.title == "New faces" && !n.faces.is_empty()));
    spring.mode = Mode::Notice;
    shot(&spring, "13_graduation");
    spring.notes.remove(0);
    shot(&spring, "14_new_faces");
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

#[test]
fn a_talk_event_waits_for_the_coachs_answer() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.spec.happenings.retain(|h| h.id == "night_swing");
    g.s.pos = g
        .spec
        .ring
        .iter()
        .position(|s| *s == board::Square::Event)
        .unwrap();
    let week = g.s.week;
    g.land(board::Card {
        steps: 1,
        focus: None,
        star: None,
    });
    let Mode::Ask { who, .. } = g.mode else {
        panic!("a talk, not {:?}", g.mode)
    };
    let hud = g.hud(W, H, false);
    let name = g.s.team.players[who].family.clone();
    assert!(all_texts(&hud).any(|t| t.text.contains(&name)));
    let xp: u16 = g.s.team.players[who].xp.iter().sum();
    press(&mut g, hud::Btn::Choose(0));
    assert!(g.s.team.players[who].xp.iter().sum::<u16>() > xp);
    assert_eq!(g.s.week, week + 1);
    assert_eq!(g.mode, Mode::Notice);
    assert_eq!(g.notes[0].faces.len(), 1);
}

#[test]
fn a_pitchers_page_fits_above_the_practice_menu_on_a_short_screen() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    let i = g.s.team.players.iter().position(|p| p.pitcher()).unwrap();
    g.mode = Mode::Player(i);
    // A 720p desktop: the shortest canvas (700 units).
    let buttons = g.hud_buttons(1280.0, 720.0);
    let raise_bottom = buttons
        .iter()
        .filter(|(b, _)| matches!(b, hud::Btn::Raise(_)))
        .map(|(_, r)| r[1] + r[3])
        .fold(0.0, f32::max);
    let menu_top = buttons
        .iter()
        .filter(|(b, _)| matches!(b, hud::Btn::Menu(_)))
        .map(|(_, r)| r[1])
        .fold(f32::MAX, f32::min);
    assert!(raise_bottom < menu_top, "{raise_bottom} vs {menu_top}");
}

#[test]
fn abilities_lineup_rivals_and_the_next_chapter() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    // The order can be edited, and an old save without the new fields still loads.
    press(&mut g, hud::Btn::Roster);
    press(&mut g, hud::Btn::Lineup);
    assert!(matches!(g.mode, Mode::Lineup { pick: None }));
    let before = g.s.team.resolved()[0].0;
    press(&mut g, hud::Btn::Nudge(0, 1));
    assert!(!g.s.team.card.order.is_empty(), "the coach's order is kept");
    assert_ne!(g.s.team.resolved()[0].0, before);
    let saved = g.save_json().unwrap();
    let mut old: Value = serde_json::from_str(&saved).unwrap();
    old.as_object_mut().unwrap().remove("rivals");
    old.as_object_mut().unwrap().remove("slate");
    old.as_object_mut().unwrap().remove("chapter");
    old["team"]["players"][0]
        .as_object_mut()
        .unwrap()
        .remove("abilities");
    old["team"]["players"][0]
        .as_object_mut()
        .unwrap()
        .remove("study");
    let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    world["save"] = old;
    let mut resumed = DiamondDays::from_json(&world.to_string()).unwrap();
    resumed.confirm();
    assert_eq!(resumed.s.rivals.len(), 10, "a circuit is filled in");
    assert_eq!(resumed.chapter(), 1);
    assert!(resumed.s.team.players[0].abilities.is_empty());

    // A deep autumn run is invited to March on selection day in January.
    g.s.week = board::SELECTION_WEEK;
    g.s.selection_year = 0;
    g.s.autumn_wins = 3;
    g.s.year = 1;
    g.mode = Mode::Plan;
    g.game = None;
    g.notes.clear();
    g.begin_week();
    assert!(g.s.spring_in, "the autumn sends them to March");
    assert!(g.notes.iter().any(|n| n.title == "Selection day"));

    // A practice school comes back the same week next year.
    g.notes.clear();
    g.s.week = 22;
    g.mode = Mode::Plan;
    g.game = None;
    g.begin_week();
    assert_eq!(g.mode, Mode::MatchIntro);
    let rival = g.s.rivals[g.rival_idx.unwrap()].name.clone();
    press(&mut g, hud::Btn::PlayBall);
    skip_whole_game(&mut g);
    g.notes.clear();
    g.mode = Mode::Plan;
    g.s.year = 2;
    g.s.week = 22;
    g.game = None;
    g.begin_week();
    let again = g.s.rivals[g.rival_idx.unwrap()].name.clone();
    assert_eq!(again, rival, "year 2 is a rematch");

    // Exam week benches someone who is not durable.
    for p in &mut g.s.team.players {
        p.abilities.retain(|a| *a != team::Ability::Durable);
        p.study = 0;
        p.mood = team::Mood::Awful;
    }
    g.s.week = board::EXAM_WEEK;
    g.s.exam_year = 0;
    g.s.year = 1;
    g.game = None;
    g.notes.clear();
    g.mode = Mode::Plan;
    g.begin_week();
    assert!(g.s.team.players.iter().any(|p| p.study == 2));
    assert!(g.notes.iter().any(|n| n.title == "Exams"));

    // After three years the coach can stay for another chapter: the career carries on.
    g.notes.clear();
    g.s.epilogue = false;
    g.s.year = g.spec.years;
    g.s.week = board::WEEKS;
    g.mode = Mode::Plan;
    let rep = g.s.reputation;
    let grounds = g.s.grounds;
    g.end_week();
    assert!(g.s.epilogue);
    while matches!(g.mode, Mode::Notice) {
        tap(&mut g, 195.0, 400.0);
    }
    assert!(matches!(g.mode, Mode::Epilogue { schools: false }));
    press(&mut g, hud::Btn::Stay);
    assert_eq!(g.s.year, 1);
    assert_eq!(g.chapter(), 2);
    assert!(!g.s.epilogue);
    assert_eq!(g.s.reputation, rep);
    assert!(g.s.grounds >= grounds);
    assert_eq!(g.spec.years, 3);
}

/// Like the long-running school managers: the years never run out. Every three years is
/// a milestone (stay, move, retire); the year count, the record and the fame carry on.
#[test]
fn the_career_runs_on_past_every_milestone() {
    let mut g = started();
    g.set_lang("ja");
    let mut milestones = 0;
    for year in 1..=7u32 {
        assert_eq!(g.career_year(), year);
        g.notes.clear();
        g.game = None;
        g.mode = Mode::Plan;
        g.s.week = board::WEEKS;
        g.s.wins += 2;
        g.s.losses += 1;
        g.end_week();
        while matches!(g.mode, Mode::Notice) {
            tap(&mut g, 195.0, 400.0);
        }
        if matches!(g.mode, Mode::Epilogue { .. }) {
            milestones += 1;
            let hud = g.hud(W, H, false);
            assert!(all_texts(&hud).any(|t| t.text == "ひと区切り"));
            press(&mut g, hud::Btn::Stay);
            while matches!(g.mode, Mode::Notice) {
                tap(&mut g, 195.0, 400.0);
            }
        }
        assert_eq!(g.flow.phase, FlowPhase::Play, "year {year}");
    }
    assert_eq!(milestones, 2);
    assert_eq!(g.career_year(), 8);
    assert_eq!(g.chapter(), 3);
    let years: Vec<u32> = g.s.history.iter().map(|l| l.year).collect();
    assert_eq!(years, (1..=7).collect::<Vec<_>>());
    assert!(g.s.history.iter().all(|l| (l.wins, l.losses) == (2, 1)));
    assert!(g.s.slate.iter().all(|e| e.chapter == 3), "old fixtures go");
    let hud = g.hud(W, H, false);
    assert!(all_texts(&hud).any(|t| t.text == "8年目"));
    // The record survives a reload.
    let saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    world["save"] = saved;
    let mut h = DiamondDays::from_json(&world.to_string()).unwrap();
    h.confirm();
    assert_eq!(h.s.history, g.s.history);
    assert_eq!(h.career_year(), 8);
    // Fame names the school.
    assert_eq!(board::rank_title(0.0).1, "無名校");
    assert_eq!(board::rank_title(100.0).1, "伝説校");
}

#[test]
fn a_later_chapter_reloads_without_the_new_coach_intro() {
    let mut g = started();
    g.notes.clear();
    g.s.year = g.spec.years;
    g.s.week = board::WEEKS;
    g.mode = Mode::Plan;
    g.end_week();
    while matches!(g.mode, Mode::Notice) {
        tap(&mut g, 195.0, 400.0);
    }
    press(&mut g, hud::Btn::Stay);
    assert_eq!((g.s.year, g.s.week, g.s.wins, g.s.losses), (1, 1, 0, 0));
    let saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    world["save"] = saved;
    let mut h = DiamondDays::from_json(&world.to_string()).unwrap();
    h.confirm();
    assert!(
        !h.notes.iter().any(|n| n.title == "A new coach"),
        "{:?}",
        h.notes.iter().map(|n| &n.title).collect::<Vec<_>>()
    );
    // A brand-new career still gets it.
    let fresh = started();
    assert!(fresh.notes.iter().any(|n| n.title == "A new coach"));
}

#[test]
fn the_old_school_keeps_its_own_colours_after_a_move() {
    let mut g = started();
    let old = g.school_en().to_string();
    g.move_picks = vec![0];
    let joined = g.s.rivals[0].name.clone();
    let joined_color = g.s.rivals[0].color;
    g.continue_move(0);
    assert_eq!(g.school_en(), joined);
    let slot = &g.s.rivals[0];
    assert_eq!(slot.name, old);
    assert_eq!(slot.face, old);
    let c = art::team_color(&old);
    assert_eq!(slot.color, [c[0], c[1], c[2]]);
    assert!(slot.color != joined_color || c[..3] == joined_color);
}

#[test]
fn no_more_calls_from_the_bench_picker_plays_on() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    press(&mut g, hud::Btn::PlayBall);
    for _ in 0..20000 {
        match g.mode {
            Mode::Match {
                asking: Some(Spot::Chance),
                ..
            } if g
                .game
                .as_ref()
                .is_some_and(|gm| !gm.sides[gm.us].bench.is_empty()) =>
            {
                press(&mut g, hud::Btn::OpenBench(0));
                assert!(matches!(
                    g.mode,
                    Mode::Match {
                        asking: Some(Spot::Bench(0)),
                        ..
                    }
                ));
                press(&mut g, hud::Btn::Auto);
                assert!(matches!(g.mode, Mode::Match { asking: None, .. }));
                assert!(g.auto_orders);
                // And it never asks again this game.
                for _ in 0..20000 {
                    match g.mode {
                        Mode::Match {
                            asking: Some(_), ..
                        } => panic!("asked after No more calls"),
                        Mode::Match { .. } => run(&mut g, 0.3),
                        _ => return,
                    }
                }
                return;
            }
            Mode::Match {
                asking: Some(_), ..
            } => press(&mut g, hud::Btn::Call(None)),
            Mode::Match { .. } => run(&mut g, 0.3),
            _ => break,
        }
    }
    panic!("no chance with a bench in the game");
}

/// While a plate appearance's pitches play, the screen is as of that plate appearance: its
/// half inning on the count box and the caption, and no runs or hit it has not shown yet,
/// even though the sim has already moved on (a third out flips the half).
#[test]
fn the_pitches_show_the_game_as_it_was_before_the_play() {
    let mut g = started();
    g.set_lang("ja");
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    press(&mut g, hud::Btn::PlayBall);
    let label = |inning: u8, top: bool| format!("{}回{}", inning, if top { "表" } else { "裏" });
    let mut checked = (false, false);
    for _ in 0..400 {
        let gm = g.game.as_mut().unwrap();
        if gm.over {
            break;
        }
        if gm.spot().is_some() {
            gm.call(None);
        }
        gm.step();
        let gm = g.game.as_ref().unwrap();
        let Some(i) = gm.log.iter().rposition(|q| q.is_pa()) else {
            continue;
        };
        let pl = gm.log[i].clone();
        let third_out = pl.outs0 == 2
            && matches!(
                pl.kind,
                sim::PlayKind::Strikeout | sim::PlayKind::GroundOut | sim::PlayKind::FlyOut
            );
        let hit = matches!(pl.kind, sim::PlayKind::Hit(_));
        if !(third_out && !checked.0 || hit && !checked.1) {
            continue;
        }
        let hits_now = gm.hits[pl.side as usize];
        g.mode = Mode::Match {
            shown: i + 1,
            t: PITCH_SECS * 0.5,
            asking: None,
        };
        let hud = g.hud(W, H, false);
        let texts: Vec<String> = all_texts(&hud).map(|t| t.text.clone()).collect();
        let here = label(pl.inning, pl.side == 0);
        assert!(
            texts.iter().any(|t| t.contains(&here)),
            "{here} not in {texts:?}"
        );
        if third_out {
            let gm = g.game.as_ref().unwrap();
            if gm.inning != pl.inning || gm.top != (pl.side == 0) {
                let next = label(gm.inning, gm.top);
                assert!(
                    !texts.iter().any(|t| t.contains(&next)),
                    "{next} shown early"
                );
            }
            checked.0 = true;
        }
        if hit {
            assert!(
                texts.iter().any(|t| *t == (hits_now - 1).to_string()),
                "the hit waits for the ball"
            );
            checked.1 = true;
        }
        if checked == (true, true) {
            return;
        }
    }
    panic!("no third out and hit found: {checked:?}");
}

/// The coach can call any batter, not only chances and pinches.
#[test]
fn the_coach_calls_the_next_batter_on_request() {
    let mut g = started();
    g.set_lang("ja");
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    press(&mut g, hud::Btn::PlayBall);
    press(&mut g, hud::Btn::Orders);
    assert!(g.want_call);
    for _ in 0..2000 {
        if let Mode::Match {
            asking: Some(spot), ..
        } = g.mode
        {
            assert!(matches!(spot, Spot::Chance | Spot::Pinch));
            assert!(!g.want_call, "the request is used up");
            let hud = g.hud(W, H, false);
            assert!(all_texts(&hud).any(|t| t.text.contains("作戦は？")));
            press(&mut g, hud::Btn::Call(None));
            return;
        }
        run(&mut g, 0.1);
    }
    panic!("the call never came");
}

/// Raising a stat pops the gain over its row, and the new grade when the letter changes.
#[test]
fn a_raise_pops_up_on_the_players_page() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    press(&mut g, hud::Btn::Roster);
    let i = 0;
    g.s.team.players[i].xp = [900, 900, 900, 900, 900];
    press(&mut g, hud::Btn::Row(i));
    press(&mut g, hud::Btn::AutoRaise);
    assert!(!g.pops.is_empty());
    run(&mut g, 0.4);
    let hud = g.hud(W, H, false);
    assert!(all_texts(&hud).any(|t| t.text.starts_with('+')));
    run(&mut g, 3.0);
    assert!(g.pops.is_empty());
}

/// Every game adds to the players' numbers, and the club's hits are theirs. At the year's
/// end the best bat and arm are named, the season starts again, and pros are remembered.
#[test]
fn players_keep_their_numbers_and_the_year_names_its_best() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    press(&mut g, hud::Btn::PlayBall);
    skip_whole_game(&mut g);
    let rep = g.report.clone().unwrap();
    let ours: u32 = g.s.team.players.iter().map(|p| p.season.hits as u32).sum();
    assert_eq!(
        ours, rep.hits[rep.us] as u32,
        "every hit belongs to somebody"
    );
    let played: Vec<_> =
        g.s.team
            .players
            .iter()
            .filter(|p| p.season.games == 1)
            .collect();
    assert!(played.len() >= 9, "the nine (and whoever pitched) played");
    assert!(g.s.team.players.iter().all(|p| p.career == p.season));
    // The year's end.
    press(&mut g, hud::Btn::Continue);
    for p in &mut g.s.team.players {
        p.season.ab = p.season.ab.max(10);
        p.season.faced = p.season.faced.max(20);
    }
    if let Some(p) = g.s.team.players.iter_mut().find(|p| p.grade == 3) {
        p.contact = 99;
        p.power = 99;
        p.speed = 99;
        p.field = 99;
        p.arm = 99;
    }
    g.s.reputation = 100.0;
    g.notes.clear();
    g.game = None;
    g.mode = Mode::Plan;
    g.s.week = board::WEEKS;
    g.end_week();
    assert!(g.notes.iter().any(|n| n.title == "Season awards"));
    assert!(g
        .s
        .team
        .players
        .iter()
        .all(|p| p.season == team::Stats::default()));
    assert!(
        g.s.team.players.iter().any(|p| p.career.games >= 1),
        "careers carry on"
    );
    assert!(!g.s.pros.is_empty(), "a star at a famous school goes pro");
    assert_eq!(
        team::Stats {
            ab: 16,
            hits: 5,
            ..Default::default()
        }
        .avg(),
        ".313"
    );
    assert_eq!(team::Stats::default().avg(), "---");
}

/// A call asked for does not carry into the next game; a scene change takes no taps while
/// it covers the screen; walks, sacrifices and steals are not at bats, and a steal is not
/// a batter faced.
#[test]
fn calls_wipes_subs_and_the_scorebook() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    // A raw tap on Play Ball while the curtain is still closed does nothing.
    let r = g
        .hud_buttons(W as f32, H as f32)
        .into_iter()
        .find(|(b, _)| *b == hud::Btn::PlayBall)
        .map(|(_, r)| r)
        .unwrap();
    touch(
        &mut g,
        r[0] + r[2] * 0.5,
        r[1] + r[3] * 0.5,
        PointerPhase::Begin,
    );
    assert_eq!(g.mode, Mode::MatchIntro, "covered: no tap");
    press(&mut g, hud::Btn::PlayBall);
    assert!(matches!(g.mode, Mode::Match { .. }));
    // The first pitch waits for the iris.
    let shown = |g: &DiamondDays| match g.mode {
        Mode::Match { shown, .. } => shown,
        _ => usize::MAX,
    };
    let before = shown(&g);
    g.tick(DT);
    assert_eq!(shown(&g), before, "the game waits under the iris");
    // Ask for a call, then skip the game: the request is gone.
    press(&mut g, hud::Btn::Orders);
    assert!(g.want_call);
    press(&mut g, hud::Btn::Skip);
    assert!(!g.want_call, "a call asked for does not carry over");
    // The scorebook.
    let mk = |kind| sim::Play {
        batter: "A".into(),
        kind,
        ..sim::Play::note("", "")
    };
    for k in [
        sim::PlayKind::Walk,
        sim::PlayKind::SacFly,
        sim::PlayKind::Bunt,
        sim::PlayKind::Steal,
        sim::PlayKind::CaughtStealing,
    ] {
        assert!(!mk(k).is_at_bat(), "{k:?} is not an at bat");
    }
    assert!(mk(sim::PlayKind::Hit(1)).is_at_bat() && mk(sim::PlayKind::Strikeout).is_at_bat());
    assert!(!mk(sim::PlayKind::Steal).faced_batter());
    assert!(mk(sim::PlayKind::Walk).faced_batter());
}

/// A starter taken out for a pinch hitter keeps what he did before.
#[test]
fn a_starter_taken_out_keeps_his_line() {
    for seed in 0..12u32 {
        let mut g = DiamondDays::new(DiamondDaysSpec {
            seed,
            ..DiamondDaysSpec::default()
        });
        let _ = g.hud(W, H, false);
        g.confirm();
        tap(&mut g, 195.0, 400.0);
        g.s.week = 6;
        g.begin_week();
        press(&mut g, hud::Btn::PlayBall);
        let mut subbed = None;
        for _ in 0..20000 {
            match g.mode {
                Mode::Match {
                    asking: Some(Spot::Chance),
                    ..
                } if subbed.is_none()
                    && g.game.as_ref().is_some_and(|gm| {
                        !gm.sides[gm.us].bench.is_empty()
                            && gm.log.iter().any(|q| {
                                q.is_pa()
                                    && q.side as usize == gm.us
                                    && q.batter == gm.batter_now().name
                            })
                    }) =>
                {
                    let starter = g.game.as_ref().unwrap().batter_now().name.clone();
                    press(&mut g, hud::Btn::OpenBench(0));
                    press(&mut g, hud::Btn::UseBench(0));
                    subbed = Some(starter);
                }
                Mode::Match {
                    asking: Some(_), ..
                } => press(&mut g, hud::Btn::Call(None)),
                Mode::Match { .. } => run(&mut g, 0.3),
                _ => break,
            }
        }
        let Some(starter) = subbed else { continue };
        let rep = g.report.clone().unwrap();
        let ours: u32 = g.s.team.players.iter().map(|p| p.season.hits as u32).sum();
        assert_eq!(ours, rep.hits[rep.us] as u32, "hits add up with a sub");
        let p =
            g.s.team
                .players
                .iter()
                .find(|p| p.family == starter)
                .unwrap();
        assert_eq!(p.season.games, 1, "{starter} keeps his game");
        return;
    }
    panic!("no game with a pinch hit for a starter who had batted");
}

/// The game in 3D with the HUD over it, rendered offscreen (a GPU or lavapipe):
/// `KAGRA_SHOTS3D=<dir> cargo test --features render --lib match_in_3d -- --nocapture`.
#[cfg(feature = "render")]
#[test]
fn match_in_3d_to_png() {
    let Ok(dir) = std::env::var("KAGRA_SHOTS3D") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shot = |g: &DiamondDays, name: &str| {
        let hud = g.hud(W, H, false);
        let t0 = std::time::Instant::now();
        let doc = g.document().clone();
        let rgba = crate::render::render_world_doc_with_hud(&doc, W, H, &hud).unwrap();
        eprintln!(
            "{name}: {} props, rendered in {:?}",
            doc.props.len(),
            t0.elapsed()
        );
        let mut c = crate::raster2d::Canvas::new(W, H, [0, 0, 0, 255]);
        c.rgba = rgba;
        c.save_png(&dir.join(format!("{name}.png"))).unwrap();
    };
    let night = std::env::var("KAGRA_NIGHT").is_ok();
    let mut g = started();
    g.set_lang("ja");
    tap(&mut g, 195.0, 400.0);
    // A spring game by day, an autumn one under the lights.
    let weeks: Vec<u8> = if night { (25..=36).collect() } else { vec![6] };
    for w in weeks {
        g.s.week = w;
        g.begin_week();
        if g.mode == Mode::MatchIntro {
            break;
        }
        g.mode = Mode::Plan;
    }
    press(&mut g, hud::Btn::PlayBall);
    run(&mut g, 0.6);
    let find = |g: &DiamondDays, id: &str| {
        g.document()
            .props
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.position)
    };
    let (mut pitch, mut field, mut idle, mut grounder) = (false, false, false, false);
    let mut called = false;
    for _ in 0..60000 {
        match g.mode {
            Mode::Match {
                asking: Some(_), ..
            } => {
                if !idle {
                    shot(&g, "3d_idle");
                    idle = true;
                }
                press(&mut g, hud::Btn::Call(None));
            }
            Mode::Match { .. } => g.tick(DT),
            _ => break,
        }
        let shadow = find(&g, "ball-shadow");
        if let (false, Some(b), None) = (pitch, find(&g, "ball"), shadow) {
            if b[2] < -6.0 && b[2] > -12.0 {
                shot(&g, "3d_pitch");
                pitch = true;
            }
        }
        if let (false, Some(b), Some(_)) = (field, find(&g, "ball"), shadow) {
            if b[1] > 6.0 {
                shot(&g, "3d_field");
                field = true;
            }
        }
        // A ground ball on its way across to first.
        let kind = match (&g.mode, g.game.as_ref()) {
            (Mode::Match { shown, .. }, Some(gm)) if *shown > 0 => {
                gm.log.get(shown - 1).map(|p| p.kind)
            }
            _ => None,
        };
        if let (false, Some(sim::PlayKind::GroundOut), Some(b), Some(_)) =
            (grounder, kind, find(&g, "ball"), shadow)
        {
            if b[0] > 8.0 && b[1] > 1.5 {
                // The first baseman is on the bag, stretching toward the throw: the
                // ball heads for a glove held up off the ground.
                let bag = [19.4f32, 0.0, -19.4];
                let near_bag = g
                    .document()
                    .props
                    .iter()
                    .filter(|p| p.id.starts_with("f2:"))
                    .map(|p| p.position)
                    .fold(f32::MAX, |m, q| m.min((q[0] - bag[0]).hypot(q[2] - bag[2])));
                assert!(near_bag < 2.5, "first baseman at the bag: {near_bag}");
                shot(&g, "3d_grounder");
                grounder = true;
            }
        }
        // A pitch with the other bench's call in force (their chip over the picture).
        let their = match (&g.mode, g.game.as_ref()) {
            (Mode::Match { shown, .. }, Some(gm)) if *shown > 0 => {
                gm.log.get(shown - 1).and_then(|p| p.their_call)
            }
            _ => None,
        };
        if let (false, Some(_), Some(b), None) = (called, their, find(&g, "ball"), shadow) {
            if b[2] < -4.0 {
                shot(&g, "3d_call");
                called = true;
            }
        }
        if pitch && field && grounder && called {
            break;
        }
    }
    assert!(pitch && field, "pitch {pitch} field {field}");
}

/// Each plate appearance keeps who was on which bag, and the 3D picture puts those runners
/// there (not the top of the order).
#[test]
fn the_runners_on_screen_are_the_ones_on_base() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    press(&mut g, hud::Btn::PlayBall);
    let mut seen = 0;
    for _ in 0..60000 {
        match g.mode {
            Mode::Match {
                asking: Some(_), ..
            } => press(&mut g, hud::Btn::Call(None)),
            Mode::Match { .. } => g.tick(DT),
            _ => break,
        }
        if let Some(gm) = g.game.as_ref() {
            for pl in gm.log.iter().filter(|p| p.is_pa()) {
                for b in 0..3 {
                    assert_eq!(
                        pl.bases0 & (1 << b) != 0,
                        !pl.runners0[b].is_empty(),
                        "{}: bag {b}",
                        pl.en
                    );
                }
            }
            if gm.bases.iter().any(|x| x.is_some()) {
                let doc = g.document();
                let names = gm.runner_names();
                for (b, n) in names.iter().enumerate() {
                    let drawn = doc
                        .props
                        .iter()
                        .any(|p| p.id.starts_with(&format!("run{b}:")));
                    if !n.is_empty()
                        && matches!(
                            g.mode,
                            Mode::Match {
                                asking: Some(_),
                                ..
                            }
                        )
                    {
                        assert!(drawn, "runner on bag {b} drawn");
                        seen += 1;
                    }
                }
            }
        }
    }
    assert!(
        seen > 0,
        "a game with someone on base while the coach was asked"
    );
}

/// The nine in the field are nine different players, the pitcher on the mound once.
#[test]
fn every_fielder_is_someone_else() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    press(&mut g, hud::Btn::PlayBall);
    let gm = g.game.as_ref().unwrap();
    for side in 0..2 {
        let pitcher = gm.sides[side].arms[0].name.clone();
        let names: Vec<String> = (0..9)
            .filter_map(|i| DiamondDays::fielder_name(gm, side, i, &pitcher))
            .collect();
        assert_eq!(names.len(), 9, "{names:?}");
        let mut uniq = names.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), 9, "{names:?}");
        assert_eq!(names.iter().filter(|n| **n == pitcher).count(), 1);
        // Everyone in the order is out there.
        for b in &gm.sides[side].bats {
            assert!(names.contains(&b.name) || b.name == pitcher, "{}", b.name);
        }
    }
}

/// On a ground ball out the first baseman takes the bag and stretches: the throw arrives
/// at a glove held up, not at his shins.
#[test]
fn the_throw_to_first_meets_a_raised_glove() {
    let bag = [19.4f32, 0.0, -19.4];
    for seed in 0..8u32 {
        let mut g = DiamondDays::new(DiamondDaysSpec {
            seed,
            ..DiamondDaysSpec::default()
        });
        let _ = g.hud(W, H, false);
        g.confirm();
        tap(&mut g, 195.0, 400.0);
        g.s.week = 6;
        g.begin_week();
        press(&mut g, hud::Btn::PlayBall);
        let mut nearest: Option<(f32, f32)> = None;
        for _ in 0..60000 {
            match g.mode {
                Mode::Match {
                    asking: Some(_), ..
                } => press(&mut g, hud::Btn::Call(None)),
                Mode::Match { .. } => g.tick(DT),
                _ => break,
            }
            let kind = match (&g.mode, g.game.as_ref()) {
                (Mode::Match { shown, .. }, Some(gm)) if *shown > 0 => {
                    gm.log.get(shown - 1).map(|p| p.kind)
                }
                _ => None,
            };
            if kind != Some(sim::PlayKind::GroundOut) {
                if nearest.is_some() {
                    break;
                }
                continue;
            }
            let doc = g.document();
            if !doc.props.iter().any(|p| p.id == "ball-shadow") {
                continue;
            }
            if let Some(b) = doc.props.iter().find(|p| p.id == "ball") {
                let d = (b.position[0] - bag[0]).hypot(b.position[2] - bag[2]);
                if nearest.is_none_or(|(nd, _)| d < nd) {
                    nearest = Some((d, b.position[1]));
                }
            }
        }
        let Some((d, y)) = nearest else { continue };
        if d > 2.5 {
            // Fielded by the first baseman himself: no throw.
            continue;
        }
        assert!(y > 0.6, "the throw arrives at {y} m");
        return;
    }
    panic!("no ground ball out thrown to first");
}

/// Opening the game again never drops you straight into a game: a kept run greets the coach
/// with where it stands first, even when this week is a game day. The `?week=` test switch
/// only starts new runs.
#[test]
fn opening_a_kept_run_says_where_it_stands_first() {
    let world_with = |save: Value, week: Option<u8>| {
        let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
        world["save"] = save;
        if let Some(w) = week {
            world["week"] = json!(w);
        }
        let mut h = DiamondDays::from_json(&world.to_string()).unwrap();
        h.set_lang("ja");
        h
    };
    let kept = |week: u8| {
        let mut g = started();
        tap(&mut g, 195.0, 400.0);
        g.s.week = week;
        serde_json::from_str::<Value>(&g.save_json().unwrap()).unwrap()
    };
    // A game week: the greeting first, the game day after the tap.
    let mut h = world_with(kept(6), None);
    let _ = h.hud(W, H, false);
    h.confirm();
    assert_eq!(h.mode, Mode::Notice);
    assert_eq!(h.notes[0].title_ja, "おかえりなさい、監督");
    assert!(
        h.notes[0].lines[1].1.contains("練習試合"),
        "{:?}",
        h.notes[0].lines
    );
    while h.mode == Mode::Notice {
        tap(&mut h, 195.0, 400.0);
    }
    assert_eq!(h.mode, Mode::MatchIntro);
    // A practice week: the greeting, then the cards.
    let mut h = world_with(kept(2), None);
    let _ = h.hud(W, H, false);
    h.confirm();
    assert_eq!(h.notes[0].title_ja, "おかえりなさい、監督");
    assert!(h.notes[0].lines[1].1.contains("練習から"));
    while h.mode == Mode::Notice {
        tap(&mut h, 195.0, 400.0);
    }
    assert_eq!(h.mode, Mode::Plan);
    // Camp and exam weeks say so, and open on them after the tap.
    for (week, says, title) in [
        (board::CAMP_WEEK, "夏合宿", None),
        (board::EXAM_WEEK, "テスト週間", Some("テスト")),
    ] {
        let mut h = world_with(kept(week), None);
        let _ = h.hud(W, H, false);
        h.confirm();
        assert_eq!(h.notes[0].title_ja, "おかえりなさい、監督");
        assert!(
            h.notes[0].lines[1].1.contains(says),
            "{:?}",
            h.notes[0].lines
        );
        tap(&mut h, 195.0, 400.0);
        if let Some(t) = title {
            assert_eq!(h.notes.first().map(|n| n.title_ja.as_str()), Some(t));
        } else {
            assert!(
                h.mode == Mode::Notice || matches!(h.mode, Mode::CampTalk { .. }),
                "camp opens: {:?}",
                h.mode
            );
        }
    }
    // `?week=` does not move a kept run.
    let mut h = world_with(kept(2), Some(6));
    h.confirm();
    assert_eq!(h.s.week, 2);
    // A new run still opens on the new coach.
    let mut g = DiamondDays::new(DiamondDaysSpec::default());
    g.set_lang("ja");
    let _ = g.hud(W, H, false);
    g.confirm();
    assert_eq!(g.notes[0].title_ja, "新しい監督");
}

#[test]
fn an_old_save_without_a_record_still_loads() {
    let g = started();
    let mut saved: Value = serde_json::from_str(&g.save_json().unwrap()).unwrap();
    let obj = saved.as_object_mut().unwrap();
    obj.remove("history");
    obj.remove("year_from");
    let mut world = DiamondDays::new(DiamondDaysSpec::default()).world_value();
    world["save"] = saved;
    let mut h = DiamondDays::from_json(&world.to_string()).unwrap();
    h.confirm();
    assert!(h.s.history.is_empty());
    assert_eq!(h.career_year(), 1);
}

#[test]
fn the_team_button_is_drawn_when_no_games_are_left() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = board::WEEKS;
    g.s.summer_in = false;
    g.s.autumn_in = false;
    let hud = g.hud(W, H, false);
    assert!(all_texts(&hud).any(|t| t.text == "TEAM"));
}

/// The words `word` sit on a layer above the paper (the first clipped layer with shapes,
/// which in a game is only ever the confetti over the picture).
fn words_over_paper(list: &crate::scene::DrawList, word: &str) {
    let layers = list.flatten();
    let paper = layers
        .iter()
        .position(|l| l.clip.is_some() && !l.mesh.verts.is_empty())
        .expect("paper in the air");
    let at = layers
        .iter()
        .position(|l| l.texts.iter().any(|t| t.text.contains(word)))
        .unwrap_or_else(|| panic!("no {word}"));
    assert!(at > paper, "{word} on layer {at}, paper on {paper}");
}

/// Games played straight through in the sim from today's match-up, by seed.
fn games_to_the_end(
    g: &DiamondDays,
    seeds: std::ops::Range<u32>,
    we_bat_first: bool,
) -> Vec<sim::Game> {
    let gm = g.game.as_ref().expect("a game on");
    let (us, them) = (gm.sides[gm.us].clone(), gm.sides[1 - gm.us].clone());
    seeds
        .map(|seed| {
            let mut x = sim::Game::new(us.clone(), them.clone(), we_bat_first, seed);
            let mut guard = 0;
            while !x.over && guard < 4000 {
                if x.spot().is_some() {
                    x.call(None);
                }
                x.step();
                guard += 1;
            }
            assert!(x.over);
            x
        })
        .collect()
}

fn on_game_day() -> DiamondDays {
    let mut g = started();
    g.set_lang("ja");
    tap(&mut g, 195.0, 400.0);
    g.s.week = 6;
    g.begin_week();
    assert_eq!(g.mode, Mode::MatchIntro);
    press(&mut g, hud::Btn::PlayBall);
    g
}

/// The last line of a game is its moment: a walk-off brings the bench out to the hero, any
/// other end lines both nines up across the plate to bow, and the line holds long enough
/// to see it.
#[test]
fn a_game_ends_with_a_walk_off_party_or_a_bow() {
    let mut g = on_game_day();
    let games = games_to_the_end(&g, 0..400, false);
    let ends: Vec<_> = games.iter().map(|x| g.finale(x, x.log.len())).collect();
    assert!(
        ends.iter().all(|e| e.is_some()),
        "every finished game has an end"
    );
    // Before the last line there is no finale.
    assert!(games.iter().all(|x| g.finale(x, x.log.len() - 1).is_none()));
    let walk = ends
        .iter()
        .position(|e| matches!(e, Some(hud::Finale::WalkOff { .. })))
        .expect("a walk-off in 400 games");
    let bow = ends
        .iter()
        .position(|e| matches!(e, Some(hud::Finale::LineUp)))
        .expect("a game that ends with the bow");
    // A walk-off: the home side won on its last play, and that batter is the hero.
    let x = &games[walk];
    let last = x.log.iter().rev().find(|q| q.is_pa()).unwrap();
    assert_eq!(last.side, 1);
    assert!(x.runs(1) > x.runs(0));
    let Some(hud::Finale::WalkOff { hero }) = &ends[walk] else {
        unreachable!()
    };
    assert_eq!(*hero, last.batter);

    let ids = |g: &DiamondDays| -> Vec<String> {
        let mut v: Vec<String> = g
            .match_world()
            .props
            .iter()
            .filter_map(|p| p.id.split(':').next().map(str::to_string))
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let at = |g: &mut DiamondDays, x: &sim::Game, t: f32| {
        g.game = Some(x.clone());
        g.mode = Mode::Match {
            shown: x.log.len(),
            t,
            asking: None,
        };
    };
    at(&mut g, &games[walk], 0.0);
    let hold = g.line_time(games[walk].log.len());
    assert!(hold >= g.spec.line_secs * 6.0 - 1e-3, "{hold}");
    at(&mut g, &games[walk], hold * 0.8);
    let seen = ids(&g);
    assert!(seen.contains(&"hero".to_string()), "{seen:?}");
    let mates = seen.iter().filter(|i| i.starts_with("mate")).count();
    assert!(mates >= 10, "the bench pours out: {mates}");
    // By the end they ring the hero, close by.
    let doc = g.match_world();
    let hero = doc
        .props
        .iter()
        .find(|p| p.id.starts_with("hero:"))
        .unwrap()
        .position;
    let near = doc
        .props
        .iter()
        .filter(|p| p.id.starts_with("mate") && p.id.ends_with(":1"))
        .filter(|p| (p.position[0] - hero[0]).hypot(p.position[2] - hero[2]) < 3.5)
        .count();
    assert!(near >= 10, "around him: {near}");
    let hud_now = g.hud(W, H, false);
    let texts: Vec<String> = all_texts(&hud_now).map(|t| t.text.clone()).collect();
    assert!(texts.iter().any(|t| t == "サヨナラ！"), "{texts:?}");
    words_over_paper(&hud_now, "サヨナラ！");
    // Batting first, a walk-off is theirs: the title says we lost it.
    let away = games_to_the_end(&g, 0..400, true);
    let lost = away
        .iter()
        .find(|x| matches!(g.finale(x, x.log.len()), Some(hud::Finale::WalkOff { .. })))
        .expect("a walk-off against us in 400 games");
    at(&mut g, lost, hold * 0.5);
    let texts: Vec<String> = all_texts(&g.hud(W, H, false))
        .map(|t| t.text.clone())
        .collect();
    assert!(texts.iter().any(|t| t == "サヨナラ負け…"), "{texts:?}");

    at(&mut g, &games[bow], 1.0);
    let seen = ids(&g);
    for side in 0..2 {
        for j in 0..9 {
            assert!(seen.contains(&format!("row{side}-{j}")), "{seen:?}");
        }
    }
    assert!(seen.contains(&"ump".to_string()));
    let texts: Vec<String> = all_texts(&g.hud(W, H, false))
        .map(|t| t.text.clone())
        .collect();
    assert!(texts.iter().any(|t| t == "ゲームセット"), "{texts:?}");
}

/// A home run holds long enough for its moment, and ours carries his number here.
#[test]
fn a_home_run_has_its_moment_and_his_number() {
    let mut g = on_game_day();
    let games = games_to_the_end(&g, 0..60, false);
    let (x, i) = games
        .iter()
        .find_map(|x| {
            x.log
                .iter()
                .position(|q| q.kind == sim::PlayKind::Hit(4) && q.side as usize == x.us)
                .map(|i| (x, i))
        })
        .expect("one of ours goes out in 60 games");
    let name = x.log[i].batter.clone();
    for p in &mut g.s.team.players {
        if p.family == name {
            p.career.hr = 11;
        }
    }
    assert_eq!(g.career_homers(x, i), 12);
    g.game = Some(x.clone());
    assert!(g.line_pace(i + 1) >= 3.0);
    // On screen while the result holds: the words, his number, and paper in the air.
    let t = g.pitch_time(i + 1) + g.spec.line_secs * 0.6;
    g.mode = Mode::Match {
        shown: i + 1,
        t,
        asking: None,
    };
    let hud = g.hud(W, H, false);
    let texts: Vec<String> = all_texts(&hud).map(|t| t.text.clone()).collect();
    assert!(texts.iter().any(|t| t == "ホームラン！"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t.contains("高校通算12号")),
        "{texts:?}"
    );
    // The paper flies under the words, not over them.
    words_over_paper(&hud, "ホームラン！");
    words_over_paper(&hud, "高校通算12号");
}

/// The big moments in 3D with the HUD, for a look (`KAGRA_SHOTS3D=dir`): a home run of
/// ours, a walk-off and the bow at the end.
#[cfg(feature = "render")]
#[test]
fn big_moments_in_3d_to_png() {
    let Ok(dir) = std::env::var("KAGRA_SHOTS3D") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut g = on_game_day();
    let shot = |g: &mut DiamondDays, x: &sim::Game, shown: usize, t: f32, name: &str| {
        g.game = Some(x.clone());
        g.mode = Mode::Match {
            shown,
            t,
            asking: None,
        };
        g.clock = t;
        g.transition = None;
        g.doc = g.match_world();
        let hud = g.hud(W, H, false);
        let rgba = crate::render::render_world_doc_with_hud(&g.doc, W, H, &hud).unwrap();
        let mut c = crate::raster2d::Canvas::new(W, H, [0, 0, 0, 255]);
        c.rgba = rgba;
        c.save_png(&dir.join(format!("{name}.png"))).unwrap();
    };
    let games = games_to_the_end(&g, 0..400, false);
    if let Some((x, i)) = games.iter().find_map(|x| {
        x.log
            .iter()
            .position(|q| q.kind == sim::PlayKind::Hit(4) && q.side as usize == x.us)
            .map(|i| (x, i))
    }) {
        g.game = Some(x.clone());
        let t = g.pitch_time(i + 1) + g.spec.line_secs * 0.5;
        shot(&mut g, x, i + 1, t, "3d_homer");
    }
    for (x, name) in [
        (
            games
                .iter()
                .find(|x| matches!(g.finale(x, x.log.len()), Some(hud::Finale::WalkOff { .. }))),
            "3d_walkoff",
        ),
        (
            games.iter().find(|x| {
                matches!(g.finale(x, x.log.len()), Some(hud::Finale::LineUp))
                    && x.runs(x.us) > x.runs(1 - x.us)
            }),
            "3d_bow",
        ),
    ] {
        let Some(x) = x else { continue };
        g.game = Some(x.clone());
        let hold = g.line_time(x.log.len());
        let n = x.log.len();
        shot(&mut g, x, n, hold * 0.75, name);
        if name == "3d_bow" {
            shot(&mut g, x, n, hold * 0.32, "3d_bow_low");
        }
    }
    // The ceremonies of a tournament: the school song, the stands, the ring at the mound.
    let won = games.iter().find(|x| {
        x.runs(x.us) > x.runs(1 - x.us)
            && matches!(g.finale(x, x.log.len()), Some(hud::Finale::LineUp))
    });
    let lost = games.iter().find(|x| x.runs(x.us) < x.runs(1 - x.us));
    for (x, cup, k, name) in [
        (won, Fixture::Summer(3), 0.85, "3d_song"),
        (lost, Fixture::Summer(3), 0.97, "3d_stands"),
        (won, Fixture::Summer(6), 0.7, "3d_crown"),
    ] {
        let Some(x) = x else { continue };
        g.s.cup = Some(cup);
        g.game = Some(x.clone());
        let hold = g.line_time(x.log.len());
        shot(&mut g, x, x.log.len(), hold * k, name);
    }
    g.s.cup = None;
}

/// Drive the screens with the first choice everywhere until `stop` holds (notes are read
/// as they come, and handed to `seen`).
fn drive(
    g: &mut DiamondDays,
    steps: usize,
    stop: impl Fn(&DiamondDays) -> bool,
    seen: &mut Vec<String>,
) {
    for _ in 0..steps {
        if stop(g) {
            return;
        }
        if let Some(n) = g.notes.first() {
            if !seen.contains(&n.title) {
                seen.push(n.title.clone());
            }
        }
        match g.mode {
            Mode::Notice => tap(g, 195.0, 400.0),
            Mode::Ask { .. } | Mode::CampTalk { .. } => press(g, hud::Btn::Choose(0)),
            Mode::Plan => {
                for p in &mut g.s.team.players {
                    p.auto_raise();
                }
                press(g, hud::Btn::Card(0))
            }
            Mode::Match {
                asking: Some(_), ..
            } => press(g, hud::Btn::Call(None)),
            Mode::Match { .. } => press(g, hud::Btn::Skip),
            Mode::Walk { .. } => run(g, 0.3),
            Mode::MatchIntro => press(g, hud::Btn::PlayBall),
            Mode::MatchEnd => press(g, hud::Btn::Continue),
            Mode::Bracket | Mode::Decide(_) => press(g, hud::Btn::Choose(0)),
            _ => panic!("unexpected {:?}", g.mode),
        }
    }
    panic!("never got there; stuck in {:?}", g.mode);
}

/// A tournament is a run of game days: the board waits, the draw shows our path, each
/// win stays on the same week, and the day before is rest, tune up, or study.
#[test]
fn a_tournament_is_a_run_of_game_days_on_its_draw() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = 4;
    g.mode = Mode::Plan;
    g.begin_week();
    assert_eq!(g.s.cup, Some(Fixture::SpringPref(1)));
    assert_eq!(g.s.cup_path.len(), 4, "a school drawn for every round");
    assert!(g.notes.iter().any(|n| n.title_ja == "春季県大会"));
    tap(&mut g, 195.0, 400.0);
    assert_eq!(g.mode, Mode::Bracket);
    let hud = g.hud(W, H, false);
    let texts: Vec<String> = all_texts(&hud).map(|t| t.text.clone()).collect();
    assert!(texts
        .iter()
        .any(|t| t.contains("決勝") || t.contains("final")));
    // Study them: the next school plays a little weaker, and the game is on.
    for p in &mut g.s.team.players {
        p.stamina = 50;
    }
    press(&mut g, hud::Btn::Choose(2));
    assert_eq!(g.mode, Mode::MatchIntro);
    assert!(g.s.studied);
    let first = g.s.cup_path[0] as usize;
    assert_eq!(
        g.rival_idx,
        Some(first),
        "round 1 is the school drawn for it"
    );
    // Play rounds until it is over: a win keeps the week; the end moves past it.
    let week = g.s.week;
    let mut seen = Vec::new();
    loop {
        drive(&mut g, 400, |g| g.mode == Mode::MatchEnd, &mut seen);
        let won = g.report.as_ref().is_some_and(|r| r.won);
        if g.s.cup.is_some() {
            assert!(won && g.s.week == week, "a win stays on the week");
            press(&mut g, hud::Btn::Continue);
            assert_eq!(g.mode, Mode::Bracket, "back to the draw");
            assert_eq!(g.s.cup_scores.len(), g.s.cup.unwrap().round() as usize - 1);
        } else {
            assert_eq!(g.s.week, board::cup_end(Fixture::SpringPref(1)));
            break;
        }
    }
}

/// Lose in the summer and the third-years retire on the spot: a word from each, they
/// leave the team (to graduate in March), and the new team names a captain.
#[test]
fn the_summer_ends_the_third_years_and_the_new_team_names_a_captain() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    let thirds: Vec<String> =
        g.s.team
            .players
            .iter()
            .filter(|p| p.grade == 3)
            .map(|p| p.family.clone())
            .collect();
    assert!(!thirds.is_empty());
    // A summer against a much stronger field, so it ends (a win would go on).
    for s in &mut g.s.rivals {
        s.strength = 90.0;
    }
    g.s.week = 13;
    g.mode = Mode::Plan;
    g.begin_week();
    assert!(matches!(g.s.cup, Some(Fixture::Summer(_))));
    let mut seen = Vec::new();
    drive(
        &mut g,
        3000,
        |g| g.mode == Mode::Decide(Decide::Captain),
        &mut seen,
    );
    assert!(seen.iter().any(|t| t == "Farewell, seniors"), "{seen:?}");
    assert!(g.s.team.players.iter().all(|p| p.grade < 3));
    let gone: Vec<String> = g.s.seniors.iter().map(|p| p.family.clone()).collect();
    assert_eq!(gone, thirds);
    assert!(
        g.s.team.players.len() >= 10,
        "nine and a spare, whatever it takes"
    );
    // The captain: one of the oldest left, his way of leading.
    let cands = g.captain_candidates();
    assert!(!cands.is_empty() && cands.len() <= 3);
    let mut ids: Vec<usize> = cands.iter().map(|(i, _)| *i).collect();
    ids.dedup();
    assert_eq!(ids.len(), cands.len(), "three different players");
    let (pick, _) = cands[1.min(cands.len() - 1)];
    let name = g.s.team.players[pick].family.clone();
    press(&mut g, hud::Btn::Choose(1.min(cands.len() - 1)));
    assert_eq!(g.s.captain, name);
    assert!(!g.s.need_captain);
    assert_eq!(g.captain_name(), Some(name));
    // March: the retired seniors graduate.
    g.notes.clear();
    g.s.week = board::WEEKS;
    g.mode = Mode::Plan;
    g.end_week();
    assert!(g.s.seniors.is_empty());
    let grad = g
        .notes
        .iter()
        .find(|n| n.title == "Graduation")
        .expect("a graduation");
    for fam in &thirds {
        assert!(grad.faces.iter().any(|(f, _)| f == fam));
    }
}

/// Before the summer the coach gives out number 1, and number 1 starts when rested.
#[test]
fn number_one_is_the_coachs_call_and_he_starts() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = board::NUMBERS_WEEK;
    g.mode = Mode::Plan;
    g.begin_week();
    assert_eq!(g.mode, Mode::Decide(Decide::Ace));
    let cands = g.ace_candidates();
    assert!(cands.len() >= 2);
    let last = cands.len() - 1;
    let pick = cands[last];
    press(&mut g, hud::Btn::Choose(last));
    let fam = g.s.team.players[pick].family.clone();
    assert_eq!(g.s.team.number_one, fam);
    assert_eq!(g.s.week, board::NUMBERS_WEEK + 1);
    assert!(g.notes.iter().any(|n| n.title == "Summer numbers"));
    for p in &mut g.s.team.players {
        p.stamina = 100;
        p.hurt = 0;
        p.study = 0;
    }
    assert_eq!(g.s.team.ace(), Some(pick), "number 1 takes the ball");
    g.s.team.players[pick].stamina = 20;
    assert_ne!(g.s.team.ace(), Some(pick), "not on a tired arm");
}

/// Winter has no games with other schools, and its training counts for more.
#[test]
fn winter_is_for_training() {
    for w in board::WINTER {
        assert!(board::fixture(w).is_none());
    }
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.week = *board::WINTER.start();
    g.mode = Mode::Plan;
    g.begin_week();
    assert!(g.notes.iter().any(|n| n.title == "Winter"));
}

/// The season's screens as pictures (`KAGRA_SHOTS=dir`): the first day, the draw, number
/// 1, the farewell to the seniors and the captain.
#[test]
fn season_screens_to_png() {
    let Ok(dir) = std::env::var("KAGRA_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shot = |g: &DiamondDays, name: &str| {
        let list = g.hud(390, 844, false);
        crate::raster2d::picture(&list, 390, 844)
            .save_png(&dir.join(format!("{name}.png")))
            .unwrap();
    };
    let mut g = DiamondDays::new(DiamondDaysSpec::default());
    g.set_lang("ja");
    let _ = g.hud(W, H, false);
    g.confirm();
    run(&mut g, 1.5);
    shot(&g, "s0_first_day");
    tap(&mut g, 195.0, 400.0);
    g.s.week = board::NUMBERS_WEEK;
    g.mode = Mode::Plan;
    g.begin_week();
    run(&mut g, 1.5);
    shot(&g, "s1_numbers");
    press(&mut g, hud::Btn::Choose(0));
    run(&mut g, 0.5);
    shot(&g, "s2_draw_note");
    g.notes.clear();
    for s in &mut g.s.rivals {
        s.strength = 90.0;
    }
    g.s.week = 13;
    g.mode = Mode::Plan;
    g.begin_week();
    tap(&mut g, 195.0, 400.0);
    run(&mut g, 1.5);
    shot(&g, "s3_bracket");
    let mut seen = Vec::new();
    drive(
        &mut g,
        3000,
        |g| {
            g.mode == Mode::Notice
                && g.notes
                    .first()
                    .is_some_and(|n| n.title == "Farewell, seniors")
        },
        &mut seen,
    );
    run(&mut g, 1.5);
    shot(&g, "s4_farewell");
    drive(
        &mut g,
        50,
        |g| g.mode == Mode::Decide(Decide::Captain),
        &mut seen,
    );
    run(&mut g, 1.5);
    shot(&g, "s5_captain");
}

/// The ceremonies: a tournament win plays the school song (ours with its words) while
/// the winners face the field; a loss runs to the stands; winning a final makes a ring at
/// the mound; and our chance strikes up the band.
#[test]
fn tournament_ceremonies_school_song_stands_and_the_ring() {
    let mut g = on_game_day();
    let games = games_to_the_end(&g, 0..200, false);
    let texts = |g: &DiamondDays| -> Vec<String> {
        all_texts(&g.hud(W, H, false))
            .map(|t| t.text.clone())
            .collect()
    };
    let show = |g: &mut DiamondDays, x: &sim::Game, k: f32| {
        g.game = Some(x.clone());
        let n = x.log.len();
        let t = g.line_time(n) * k;
        g.mode = Mode::Match {
            shown: n,
            t,
            asking: None,
        };
    };
    let won_plain = games
        .iter()
        .find(|x| {
            x.runs(x.us) > x.runs(1 - x.us)
                && matches!(g.finale(x, x.log.len()), Some(hud::Finale::LineUp))
        })
        .expect("a win that ends on a bow");
    let lost = games
        .iter()
        .find(|x| x.runs(x.us) < x.runs(1 - x.us))
        .expect("a loss");
    // A tournament win, not the final: the school song, the winners facing the field.
    g.s.cup = Some(Fixture::Summer(3));
    show(&mut g, won_plain, 0.85);
    assert!(
        texts(&g).iter().any(|t| t.contains("校歌")),
        "{:?}",
        texts(&g)
    );
    let doc = g.match_world();
    let ours = g.s.team.players.len().min(9);
    assert!(ours > 0);
    let row = format!("row{}-0:", won_plain.us);
    // The head turns with the body: the biggest ball of the figure.
    let head = doc
        .props
        .iter()
        .filter(|p| p.id.starts_with(&row) && p.model == "sphere")
        .max_by(|a, b| a.scale[0].total_cmp(&b.scale[0]))
        .unwrap();
    assert!(
        (head.yaw.abs() - std::f32::consts::PI).abs() < 0.3,
        "facing the field: {}",
        head.yaw
    );
    // The camera stays on them for the song.
    let cam = &doc.cameras[0];
    assert!(
        cam.target[0].abs() < 3.0,
        "the camera stays on the song: {:?}",
        cam.target
    );
    // A tournament loss: off to the stands.
    show(&mut g, lost, 0.85);
    assert!(
        texts(&g).iter().any(|t| t.contains("応援席")),
        "{:?}",
        texts(&g)
    );
    let doc = g.match_world();
    let row = format!("row{}-0:", lost.us);
    let ran = doc
        .props
        .iter()
        .filter(|p| p.id.starts_with(&row))
        .map(|p| p.position[0].abs())
        .fold(0.0f32, f32::max);
    assert!(ran > 10.0, "they ran to their stands: {ran}");
    // And the camera goes with them.
    let cam = &doc.cameras[0];
    assert!(
        cam.target[0].abs() > 10.0,
        "the camera follows: {:?}",
        cam.target
    );
    // Winning the final: everyone at the mound in a ring, the title on top.
    g.s.cup = Some(Fixture::Summer(6));
    show(&mut g, won_plain, 0.8);
    assert_eq!(
        g.finale(won_plain, won_plain.log.len()),
        Some(hud::Finale::Crown)
    );
    assert!(texts(&g).iter().any(|t| t == "優勝！"));
    let doc = g.match_world();
    let near_mound = (0..9)
        .filter(|i| {
            doc.props
                .iter()
                .filter(|p| p.id.starts_with(&format!("ring{i}:")))
                .any(|p| (p.position[0]).hypot(p.position[2] + 18.44) < 3.0)
        })
        .count();
    assert_eq!(near_mound, 9, "the whole nine at the mound");
    // Our chance: the band plays.
    g.s.cup = None;
    let (x, i) = games
        .iter()
        .find_map(|x| {
            x.log
                .iter()
                .position(|q| {
                    q.is_pa()
                        && q.side as usize == x.us
                        && q.bases0 & 0b110 != 0
                        && !q.pitches().is_empty()
                })
                .map(|i| (x, i))
        })
        .expect("a chance of ours");
    g.game = Some(x.clone());
    g.mode = Mode::Match {
        shown: i + 1,
        t: 0.1,
        asking: None,
    };
    assert!(texts(&g).iter().any(|t| t.contains("チャンステーマ")));
}

/// The rival ace waits late in our prefectural draw, a year older and faster every year,
/// and the draw says so.
#[test]
fn the_rival_ace_waits_in_our_half_and_grows() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    let (ai, name) = g.rival_ace().expect("a rival ace");
    assert_eq!(g.s.rivals[ai].name_ja, "星見学園");
    let mut velo = Vec::new();
    for year in 1..=3u8 {
        g.s.year = year;
        g.notes.clear();
        g.s.cup = None;
        g.s.week = 13;
        g.s.summer_in = true;
        g.s.seeded = false;
        g.mode = Mode::Plan;
        g.begin_week();
        let at = g.s.cup_path.iter().position(|&i| i as usize == ai);
        assert_eq!(at, Some(4), "the semifinal of six rounds");
        let said: Vec<String> = g
            .notes
            .iter()
            .flat_map(|n| n.lines.iter().map(|l| l.1.clone()))
            .collect();
        assert!(said.iter().any(|l| l.contains(name)), "{said:?}");
        let side = g.rival_side(Fixture::Summer(5), ai);
        assert_eq!(side.arms[0].name, name);
        assert!(side.bats.iter().any(|b| b.name == name), "he bats too");
        velo.push(side.arms[0].velo);
    }
    assert!(velo[0] < velo[1] && velo[1] < velo[2], "{velo:?}");
    // His last summer ends in August of his third year: no autumn, no practice games.
    g.s.year = 3;
    g.s.week = 24;
    assert!(g.rival_ace().is_none());
    let side = g.rival_side(Fixture::Autumn(4), ai);
    assert_ne!(side.arms[0].name, name);
    g.s.year = 2;
    assert!(
        g.rival_ace().is_some(),
        "a second-year still pitches in the autumn"
    );
}

/// Kotori grows a year with the club, and her own summer ends with the third year's.
#[test]
fn kotoris_last_summer_ends_with_a_letter() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.s.year = g.spec.years;
    g.notes.clear();
    g.retire_seniors();
    assert!(g.notes.iter().any(|n| n.title == "A letter from Kotori"));
    // Only in the first chapter's third year.
    let mut h = started();
    tap(&mut h, 195.0, 400.0);
    h.s.year = 2;
    h.notes.clear();
    h.retire_seniors();
    assert!(!h.notes.iter().any(|n| n.title == "A letter from Kotori"));
}

/// The captain the coach named is the captain at his farewell, and number 1 is the ace.
#[test]
fn the_named_captain_and_number_one_say_goodbye_as_such() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    let thirds: Vec<usize> = (0..g.s.team.players.len())
        .filter(|&i| {
            g.s.team.players[i].grade == 3 && g.s.team.players[i].role == team::Role::Regular
        })
        .collect();
    let i = *thirds.first().expect("a third-year with no founding role");
    let fam = g.s.team.players[i].family.clone();
    for p in &mut g.s.team.players {
        if p.role == team::Role::Captain {
            p.role = team::Role::Regular;
        }
    }
    g.s.captain = fam.clone();
    g.notes.clear();
    g.retire_seniors();
    let n = g
        .notes
        .iter()
        .find(|n| n.title == "Farewell, seniors")
        .unwrap();
    let k = n.faces.iter().position(|(f, _)| *f == fam).unwrap();
    assert_eq!(n.captions[k].1, "主将");
    assert!(n
        .lines
        .iter()
        .any(|l| l.1.starts_with(&fam) && l.1.contains("あとは頼んだ")));
}

/// A school from another prefecture shows on game day with its own name and colour.
#[test]
fn a_school_from_elsewhere_shows_on_game_day() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    g.notes.clear();
    g.s.cup = Some(Fixture::National(1));
    g.s.cup_path = (0..5).map(|r| AWAY + 3 + r).collect();
    g.prepare_game(Fixture::National(1));
    assert_eq!(g.mode, Mode::MatchIntro);
    let (en, _, _) = g.path_school(AWAY + 3);
    g.transition = None;
    let texts: Vec<String> = all_texts(&g.hud(W, H, false))
        .map(|t| t.text.clone())
        .collect();
    assert!(texts.contains(&en), "{en} in {texts:?}");
    // Its crest and name, not the stand-in for an unknown school.
    assert!(!texts.iter().any(|t| t == "Rival"), "{texts:?}");
    assert!(texts.iter().any(|t| *t == en[..1]), "the crest letter");
}

/// The board is a road running to the right: the squares ahead carry their step
/// numbers, so a card's number is where it lands; walking slides the road along.
#[test]
fn the_board_is_a_road_with_the_steps_ahead_numbered() {
    let mut g = started();
    g.set_lang("ja");
    tap(&mut g, 195.0, 400.0);
    g.notes.clear();
    g.mode = Mode::Plan;
    g.transition = None;
    let texts = |g: &DiamondDays| -> Vec<String> {
        all_texts(&g.hud(W, H, false))
            .map(|t| t.text.clone())
            .collect()
    };
    let now = texts(&g);
    for d in 1..=6 {
        assert!(now.contains(&d.to_string()), "step {d} in {now:?}");
    }
    // Every step number sits on the screen, 6 included, left to right.
    let hud_now = g.hud(W, H, false);
    let xs: Vec<f32> = (1..=6)
        .map(|d| {
            all_texts(&hud_now)
                .filter(|t| t.text == d.to_string() && t.size < 14.0)
                .map(|t| t.x)
                .next()
                .unwrap_or(f32::MAX)
        })
        .collect();
    assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
    assert!(xs[5] < W as f32 - 8.0, "step 6 on the screen: {xs:?}");
    // The six squares ahead are on screen by name, in order.
    let n = g.spec.ring.len();
    for d in 1..=6 {
        let sq = g.spec.ring[(g.s.pos + d) % n];
        assert!(
            now.iter().any(|t| t == hud::square_label(sq, true)),
            "{:?} ahead",
            sq
        );
    }
    assert!(now.iter().any(|t| t.contains("カードの数字")));
    // Walking: the numbers go, the steps left show.
    press(&mut g, hud::Btn::Card(0));
    assert!(matches!(g.mode, Mode::Walk { .. }));
    let walking = texts(&g);
    assert!(walking
        .iter()
        .any(|t| t.starts_with("あと") && t.ends_with("マス")));
    assert!(!walking.iter().any(|t| t.contains("カードの数字")));
}

/// The milestone's notes under each way on fit the screen in English too.
#[test]
fn the_milestone_notes_fit_the_screen() {
    let mut g = started();
    g.notes.clear();
    g.s.epilogue = true;
    g.mode = Mode::Epilogue { schools: false };
    for lang in ["en", "ja"] {
        g.set_lang(lang);
        let hud = g.hud(W, H, false);
        let st = g.stage(W as f32, H as f32);
        for t in all_texts(&hud).filter(|t| t.size < 13.0 * st.k + 0.5) {
            let w = crate::font::measure_text(&t.text, t.size);
            let (l, r) = match t.align {
                crate::scene::TextAlign::Center => (t.x - w * 0.5, t.x + w * 0.5),
                crate::scene::TextAlign::Right => (t.x - w, t.x),
                _ => (t.x, t.x + w),
            };
            assert!(
                l >= -1.0 && r <= W as f32 + 1.0,
                "{lang}: {:?} runs off ({l}..{r})",
                t.text
            );
        }
    }
}

/// "Skip" plays the rest at once, the calls left to the team, and stops on the game's last
/// moment (the bow, the song, the walk-off) so it is still seen; a second press ends it.
#[test]
fn skip_stops_on_the_last_moment_then_ends_the_game() {
    let mut g = on_game_day();
    press(&mut g, hud::Btn::Orders);
    assert!(g.want_call);
    press(&mut g, hud::Btn::Skip);
    let Mode::Match {
        shown,
        asking: None,
        t,
    } = g.mode.clone()
    else {
        panic!("still in the game: {:?}", g.mode)
    };
    let gm = g.game.as_ref().unwrap();
    assert!(gm.over);
    assert_eq!(shown, gm.log.len());
    assert!(g.finale(gm, shown).is_some(), "on the last moment");
    assert_eq!(t, 0.0, "it plays from its start");
    assert!(!g.want_call);
    assert!(g.report.is_none());
    press(&mut g, hud::Btn::Skip);
    assert_eq!(g.mode, Mode::MatchEnd);
    assert!(g.report.is_some());
}

/// "To the change" plays on to the next half-inning's first batter, or stops sooner for a
/// call the coach has to make.
#[test]
fn to_change_plays_to_the_next_half_inning() {
    let mut g = on_game_day();
    let half = |gm: &Game, shown: usize| {
        gm.log[..shown]
            .iter()
            .rev()
            .find(|q| q.is_pa())
            .map(|q| (q.inning, q.side))
    };
    let mut changes = 0;
    for _ in 0..200 {
        match g.mode.clone() {
            Mode::Match {
                asking: Some(_), ..
            } => g.give_call(None),
            Mode::Match {
                shown,
                asking: None,
                ..
            } => {
                let gm = g.game.as_ref().unwrap();
                if g.finale(gm, shown).is_some() {
                    break;
                }
                // Before the first batter, the top of the first is on screen.
                let before = half(gm, shown).or(Some((1, 0)));
                press(&mut g, hud::Btn::ToChange);
                let Mode::Match {
                    shown: now, asking, ..
                } = g.mode.clone()
                else {
                    panic!("left the game: {:?}", g.mode)
                };
                let gm = g.game.as_ref().unwrap();
                if asking.is_some() || g.finale(gm, now).is_some() {
                    continue;
                }
                let line = &gm.log[now - 1];
                assert!(line.is_pa(), "stops on a batter: {}", line.ja);
                assert_ne!(Some((line.inning, line.side)), before, "a new half");
                assert_eq!(
                    half(gm, now - 1),
                    before,
                    "the first batter of the next half"
                );
                changes += 1;
            }
            _ => break,
        }
    }
    assert!(changes >= 6, "{changes} changes");
    assert!(g
        .finale(g.game.as_ref().unwrap(), g.game.as_ref().unwrap().log.len())
        .is_some());
}

/// A tap during the walk puts the coach on his square at once.
#[test]
fn a_tap_skips_the_walk() {
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    assert_eq!(g.mode, Mode::Plan);
    let steps = g.s.hand[0].steps as usize;
    press(&mut g, hud::Btn::Card(0));
    assert!(matches!(g.mode, Mode::Walk { .. }));
    let texts: Vec<String> = all_texts(&g.hud(W, H, false))
        .map(|t| t.text.clone())
        .collect();
    assert!(texts.iter().any(|t| t.contains("Tap to skip")), "{texts:?}");
    tap(&mut g, 195.0, 300.0);
    assert!(!matches!(g.mode, Mode::Walk { .. }));
    assert_eq!(g.s.pos, steps % g.spec.ring.len());
    assert_eq!(g.s.week, 2, "the week moved on");
}

/// A home side ahead after the top of the ninth does not bat: its board shows an X there.
#[test]
fn the_home_side_that_does_not_bat_gets_an_x() {
    let g = on_game_day();
    let games = games_to_the_end(&g, 0..200, false);
    let mut seen = 0;
    for x in &games {
        let last = x.line[1].len() - 1;
        let cell = x.cell(1, last).unwrap();
        if x.home_sat_out() {
            assert_eq!(cell, "X");
            assert!(x.runs(1) > x.runs(0));
            seen += 1;
        } else {
            assert_ne!(cell, "X");
        }
        assert_ne!(x.cell(0, x.line[0].len() - 1).unwrap(), "X");
    }
    assert!(seen > 10, "{seen} of {} games", games.len());
}

/// The phone's back button closes the screen on top; on the board it is left to the shell,
/// which pauses the run.
#[test]
fn back_closes_the_screen_on_top() {
    let back = crate::session::BACK_KEY;
    let mut g = started();
    tap(&mut g, 195.0, 400.0);
    assert_eq!(g.mode, Mode::Plan);
    assert!(!g.on_key(back, true), "the board: the shell pauses");
    g.mode = Mode::Player(0);
    assert!(g.on_key(back, true));
    assert_eq!(g.mode, Mode::Roster);
    assert!(g.on_key(back, true));
    assert_eq!(g.mode, Mode::Plan);
    // The order's player picker unwinds one level: the picker, then the order.
    g.mode = Mode::Lineup { pick: Some(0) };
    assert!(g.on_key(back, true));
    assert_eq!(g.mode, Mode::Lineup { pick: None });
    assert!(g.on_key(back, true));
    assert_eq!(g.mode, Mode::Roster);
    g.mode = Mode::Plan;
    g.pause_run();
    assert!(!g.on_key(back, true), "paused: the shell resumes");
}

/// Several notices waiting can be closed at once; one alone has only the tap.
#[test]
fn queued_notices_close_at_once() {
    let mut g = started();
    g.set_lang("ja");
    assert_eq!(g.mode, Mode::Notice);
    let has = |g: &DiamondDays| {
        g.hud_buttons(W as f32, H as f32)
            .iter()
            .any(|(b, _)| *b == hud::Btn::SkipNotes)
    };
    for k in 0..3 {
        g.note(&format!("Note {k}"), &format!("お知らせ{k}"), vec![]);
    }
    assert!(g.notes.len() >= 4);
    assert!(has(&g));
    let texts: Vec<String> = all_texts(&g.hud(W, H, false))
        .map(|t| t.text.clone())
        .collect();
    assert!(
        texts.iter().any(|t| t.starts_with("まとめて送る")),
        "{texts:?}"
    );
    if let Ok(dir) = std::env::var("KAGRA_SHOTS") {
        crate::raster2d::picture(&g.hud(W, H, false), W, H)
            .save_png(&std::path::Path::new(&dir).join("01b_notices_phone.png"))
            .unwrap();
    }
    let week = g.s.week;
    press(&mut g, hud::Btn::SkipNotes);
    assert!(g.notes.is_empty() || g.s.week != week, "all closed");
    assert_ne!(g.mode, Mode::Notice);
    // A single notice: only the tap.
    g.note("One", "ひとつ", vec![]);
    g.mode = Mode::Notice;
    assert!(!has(&g));
}

/// The match buttons' labels fit inside them in English too (the longer words), with a
/// call asked for ("NEXT BATTER").
#[test]
fn match_button_labels_fit_in_english() {
    let mut g = on_game_day();
    g.set_lang("en");
    // Past the opening iris, as a player sees it.
    run(&mut g, 1.6);
    press(&mut g, hud::Btn::Orders);
    assert!(g.want_call);
    let list = g.hud(W, H, false);
    if let Ok(dir) = std::env::var("KAGRA_SHOTS") {
        crate::raster2d::picture(&list, W, H)
            .save_png(&std::path::Path::new(&dir).join("07c_match_en_phone.png"))
            .unwrap();
    }
    let texts: Vec<_> = all_texts(&list).cloned().collect();
    let mut checked = 0;
    for (b, r) in g.hud_buttons(W as f32, H as f32) {
        let cx = r[0] + r[2] * 0.5;
        let label = texts
            .iter()
            .filter(|t| (t.x - cx).abs() < 0.5 && t.y >= r[1] && t.y <= r[1] + r[3])
            .max_by(|a, b| a.size.total_cmp(&b.size));
        let Some(t) = label else { continue };
        let wide = crate::font::measure_text(&t.text, t.size);
        assert!(wide <= r[2], "{b:?} '{}' {wide:.0} > {:.0}", t.text, r[2]);
        checked += 1;
    }
    assert!(checked >= 4, "{checked} labels found");
}

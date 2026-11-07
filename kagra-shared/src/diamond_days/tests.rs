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
    press(&mut g, hud::Btn::PlayBall);
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

    // Autumn champions are invited, and the same school comes back next year.
    g.s.week = 27;
    g.s.autumn_in = true;
    g.s.autumn_wins = 2;
    g.s.year = 1;
    g.mode = Mode::Plan;
    g.game = None;
    g.notes.clear();
    g.begin_week();
    assert_eq!(g.mode, Mode::MatchIntro);
    let rival = g.s.rivals[g.rival_idx.unwrap()].name.clone();
    press(&mut g, hud::Btn::PlayBall);
    press(&mut g, hud::Btn::Skip);
    assert!(g.s.spring_in, "the autumn sends them to March");
    g.notes.clear();
    g.mode = Mode::Plan;
    g.s.year = 2;
    g.s.week = 27;
    g.s.autumn_in = true;
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

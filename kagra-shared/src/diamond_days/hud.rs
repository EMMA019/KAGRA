//! The screens, drawn as illustrations on a 400-unit-wide stage (phone first; a desktop
//! shows it centred over the backdrop): the week's board with the hand of cards and the
//! manager, the notice card, a game on the field with the scoreboard and the coach's calls,
//! the roster and one player's page.

mod broadcast;
mod season;
mod stadium;
#[cfg(test)]
pub(in crate::diamond_days) use stadium::Finale;

use super::art::{self, INK, INK_SOFT, ORANGE, OURS, WHITE};
use super::board::{self, Fixture, Square};
use super::sim::{Spot, Tactic};
use super::team::{self, grade, Drill, Stat};
use super::{DiamondDays, Mode};
use crate::flow::FlowPhase;
use crate::game_core::portrait::{Expr, Outfit};
use crate::hud_art::{Painter, Stage};
use crate::scene::{DrawList, TextAlign};
use crate::vector::{self, shade, Paint, Rgba};

/// A button on the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Card(usize),
    Roster,
    RosterUp,
    RosterDown,
    /// A row of the roster: that player's page.
    Row(usize),
    /// Close the roster.
    Close,
    /// An answer in a talk.
    Choose(usize),
    /// A call at a chance or a pinch; None lets them play.
    Call(Option<Tactic>),
    /// No more calls this game.
    Auto,
    Skip,
    /// Play on to the end of this half-inning (a call still stops it).
    ToChange,
    /// Close every notice waiting, not one at a time.
    SkipNotes,
    /// A player's page: raise a stat, spend everything, change the practice menu, back.
    Raise(Stat),
    AutoRaise,
    Menu(Drill),
    Back,
    /// Open the batting order from the roster.
    Lineup,
    /// Clear a saved order.
    AutoFill,
    /// A slot in the order (reorder screen, or the one being filled).
    Slot(usize),
    /// Move that slot up (-1) or down (1).
    Nudge(usize, i8),
    /// Next defensive position for that slot.
    Cycle(usize),
    /// Put this roster index into the selected slot.
    Assign(usize),
    /// Open the bench: 0 pinch-hit, 1 pinch-run, 2 defense.
    OpenBench(u8),
    UseBench(u8),
    Stay,
    MoveSchool,
    School(u8),
    EndStory,
    /// Start the game from the pre-game screen.
    PlayBall,
    /// Leave the result screen for practice.
    Continue,
    /// Double speed in a game.
    Speed,
    /// Call the next plate appearance, whoever bats.
    Orders,
}

fn push_choices(b: &mut Vec<(Btn, [f32; 4])>, hh: f32, n: usize) {
    let n = n.min(3);
    for k in 0..n {
        let y = hh - 70.0 - (n - k) as f32 * 64.0;
        b.push((Btn::Choose(k), [24.0, y, 352.0, 54.0]));
    }
}

fn drill_label(d: Drill, ja: bool) -> &'static str {
    match (d, ja) {
        (Drill::Batting, false) => "BAT",
        (Drill::Pitching, false) => "PITCH",
        (Drill::Fielding, false) => "FIELD",
        (Drill::Running, false) => "RUN",
        (Drill::Strength, false) => "POWER",
        (Drill::Batting, true) => "打撃",
        (Drill::Pitching, true) => "投球",
        (Drill::Fielding, true) => "守備",
        (Drill::Running, true) => "走塁",
        (Drill::Strength, true) => "筋力",
    }
}

/// Design-unit layout of the screen that is showing (shared by drawing and taps).
struct Layout {
    st: Stage,
    buttons: Vec<(Btn, [f32; 4])>,
}

pub(super) const ROW_H: f32 = 58.0;
pub(super) const ROSTER_TOP: f32 = 124.0;

/// The window onto the roster (and the player picker): rows scroll inside it.
pub(super) fn list_view(hh: f32) -> [f32; 4] {
    [10.0, ROSTER_TOP, 380.0, (hh - ROSTER_TOP - 72.0).max(ROW_H)]
}

/// Height of `n` rows.
pub(super) fn list_content(n: usize) -> f32 {
    (n as f32 * (ROW_H + 4.0) - 4.0).max(0.0)
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> Option<[f32; 4]> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

/// Lines that fit `width` design units at `size` ([`crate::text`]: Japanese line-breaking
/// rules, words and numbers kept whole).
fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
    crate::text::wrap(text, size, width)
}

impl DiamondDays {
    pub(super) fn stage(&self, w: f32, h: f32) -> Stage {
        Stage::fit(w, h, 400.0, 700.0)
    }

    fn layout(&self, w: f32, h: f32) -> Layout {
        let st = self.stage(w, h);
        let hh = st.h;
        let mut b = Vec::new();
        match self.mode {
            Mode::Plan => {
                let y = hh - 112.0;
                for i in 0..self.s.hand.len().min(5) {
                    b.push((Btn::Card(i), [10.0 + i as f32 * 77.5, y, 70.0, 98.0]));
                }
                b.push((Btn::Roster, [302.0, 72.0, 88.0, 44.0]));
            }
            Mode::Ask { h, .. } => {
                let n = self
                    .spec
                    .happenings
                    .get(h)
                    .map(|x| x.choices.len())
                    .unwrap_or(0);
                push_choices(&mut b, hh, n);
            }
            Mode::CampTalk { .. } => {
                push_choices(&mut b, hh, board::camp_talk().choices.len());
            }
            Mode::Bracket => push_choices(&mut b, hh, 3),
            Mode::Notice if self.notes.len() > 1 => {
                if let Some((y, tall)) = self.notice_box(hh) {
                    b.push((Btn::SkipNotes, [214.0, y + tall - 82.0, 146.0, 36.0]));
                }
            }
            Mode::Decide(d) => {
                let n = match d {
                    super::Decide::Ace => self.ace_candidates().len(),
                    super::Decide::Captain => self.captain_candidates().len(),
                };
                push_choices(&mut b, hh, n);
            }
            Mode::Roster => {
                b.push((Btn::RosterUp, [296.0, 78.0, 42.0, 34.0]));
                b.push((Btn::RosterDown, [344.0, 78.0, 42.0, 34.0]));
                // Rows answer only where they show in the window.
                let view = list_view(hh);
                for (i, r) in self.list_rows(hh) {
                    if let Some(hit) = overlap(r, view) {
                        b.push((Btn::Row(i), hit));
                    }
                }
                b.push((Btn::Lineup, [10.0, hh - 60.0, 88.0, 48.0]));
                b.push((Btn::AutoRaise, [106.0, hh - 60.0, 156.0, 48.0]));
                b.push((Btn::Close, [270.0, hh - 60.0, 120.0, 48.0]));
            }
            Mode::Lineup { pick } => {
                if pick.is_some() {
                    let view = list_view(hh);
                    for (i, r) in self.list_rows(hh) {
                        if let Some(hit) = overlap(r, view) {
                            b.push((Btn::Assign(i), hit));
                        }
                    }
                    b.push((Btn::RosterUp, [10.0, hh - 60.0, 70.0, 48.0]));
                    b.push((Btn::RosterDown, [88.0, hh - 60.0, 70.0, 48.0]));
                    b.push((Btn::Back, [270.0, hh - 60.0, 120.0, 48.0]));
                } else {
                    let n = self.s.team.resolved().len().min(9);
                    for i in 0..n {
                        let y = 78.0 + i as f32 * 52.0;
                        b.push((Btn::Nudge(i, -1), [8.0, y, 36.0, 46.0]));
                        b.push((Btn::Slot(i), [48.0, y, 220.0, 46.0]));
                        b.push((Btn::Cycle(i), [274.0, y, 70.0, 46.0]));
                        b.push((Btn::Nudge(i, 1), [350.0, y, 40.0, 46.0]));
                    }
                    b.push((Btn::AutoFill, [10.0, hh - 60.0, 160.0, 48.0]));
                    b.push((Btn::Close, [270.0, hh - 60.0, 120.0, 48.0]));
                }
            }
            Mode::Epilogue { schools } => {
                if schools {
                    for k in 0..self.move_picks.len().min(3) {
                        let y = hh * 0.42 + k as f32 * 64.0;
                        b.push((Btn::School(k as u8), [24.0, y, 352.0, 54.0]));
                    }
                    b.push((Btn::Back, [24.0, hh - 80.0, 352.0, 48.0]));
                } else {
                    let y = hh * 0.47 + 34.0;
                    b.push((Btn::Stay, [24.0, y, 352.0, 50.0]));
                    b.push((Btn::MoveSchool, [24.0, y + 80.0, 352.0, 50.0]));
                    b.push((Btn::EndStory, [24.0, y + 160.0, 352.0, 50.0]));
                }
            }
            Mode::Match { asking, shown, .. } => match asking {
                Some(spot) => {
                    let g = self.game.as_ref();
                    let panel = self.call_panel(hh);
                    let per = 3usize;
                    let bw = (panel[2] - 32.0 - (per as f32 - 1.0) * 8.0) / per as f32;
                    let place = |i: usize, bw: f32| {
                        let (row, col) = (i / per, i % per);
                        [
                            panel[0] + 16.0 + col as f32 * (bw + 8.0),
                            panel[1] + 78.0 + row as f32 * 56.0,
                            bw,
                            48.0,
                        ]
                    };
                    match spot {
                        Spot::Bench(_) => {
                            let n = g.map(|g| g.sides[g.us].bench.len()).unwrap_or(0).min(6);
                            for i in 0..n {
                                b.push((Btn::UseBench(i as u8), place(i, bw)));
                            }
                            b.push((Btn::Call(None), place(n, bw)));
                        }
                        Spot::Chance | Spot::Pinch => {
                            let tactics = match spot {
                                Spot::Chance => Tactic::CHANCE.as_slice(),
                                Spot::Pinch => Tactic::PINCH.as_slice(),
                                Spot::Bench(_) => &[],
                            };
                            let mut list: Vec<Btn> = tactics
                                .iter()
                                .copied()
                                .filter(|t| g.is_some_and(|g| t.possible(g)))
                                .map(|t| Btn::Call(Some(t)))
                                .collect();
                            let bench = g.map(|g| g.sides[g.us].bench.len()).unwrap_or(0);
                            if bench > 0 && matches!(spot, Spot::Chance) {
                                list.push(Btn::OpenBench(0));
                                if g.is_some_and(|g| g.bases.iter().any(|b| b.is_some())) {
                                    list.push(Btn::OpenBench(1));
                                }
                            }
                            if bench > 0 && matches!(spot, Spot::Pinch) {
                                list.push(Btn::OpenBench(2));
                            }
                            list.push(Btn::Call(None));
                            for (i, btn) in list.into_iter().enumerate() {
                                b.push((btn, place(i, bw)));
                            }
                        }
                    }
                    b.push((
                        Btn::Auto,
                        [panel[0] + panel[2] - 136.0, panel[1] + 16.0, 120.0, 34.0],
                    ));
                }
                None => {
                    b.push((Btn::Speed, [10.0, hh - 62.0, 86.0, 46.0]));
                    // On the last moment only the way to the result is left.
                    if self.at_finale(shown) {
                        b.push((Btn::Skip, [198.0, hh - 62.0, 192.0, 46.0]));
                    } else {
                        b.push((Btn::Orders, [104.0, hh - 62.0, 86.0, 46.0]));
                        b.push((Btn::ToChange, [198.0, hh - 62.0, 92.0, 46.0]));
                        b.push((Btn::Skip, [298.0, hh - 62.0, 92.0, 46.0]));
                    }
                }
            },
            Mode::MatchIntro => {
                b.push((Btn::Lineup, [20.0, hh - 76.0, 150.0, 58.0]));
                b.push((Btn::PlayBall, [180.0, hh - 76.0, 200.0, 58.0]));
            }
            Mode::MatchEnd => {
                b.push((Btn::Continue, [70.0, hh - 82.0, 260.0, 58.0]));
            }
            Mode::Player(i) => {
                let p = &self.s.team.players[i];
                let stats: Vec<Stat> = Stat::ALL
                    .iter()
                    .copied()
                    .filter(|st| p.pitcher() || !st.pitching())
                    .collect();
                // The rows fit between the points panel and the practice menu (nine rows
                // for a pitcher on the shortest 700-unit canvas).
                let my = hh - 128.0;
                let n = stats.len().max(1) as f32;
                let step = ((my - 34.0 - 252.0) / n).clamp(28.0, 40.0);
                for (k, st) in stats.into_iter().enumerate() {
                    b.push((
                        Btn::Raise(st),
                        [334.0, 252.0 + k as f32 * step, 48.0, step - 6.0],
                    ));
                }
                for (k, d) in Drill::ALL.into_iter().enumerate() {
                    b.push((Btn::Menu(d), [10.0 + k as f32 * 77.0, my, 72.0, 38.0]));
                }
                b.push((Btn::AutoRaise, [10.0, hh - 62.0, 200.0, 48.0]));
                b.push((Btn::Back, [270.0, hh - 62.0, 120.0, 48.0]));
            }
            _ => {}
        }
        Layout { st, buttons: b }
    }

    /// The roster rows in the window now (player, the row's whole rectangle).
    fn list_rows(&self, hh: f32) -> Vec<(usize, [f32; 4])> {
        let view = list_view(hh);
        (0..self.s.team.players.len())
            .map(|i| {
                let y = ROSTER_TOP + i as f32 * (ROW_H + 4.0) - self.roster.offset;
                (i, [10.0, y, 380.0, ROW_H])
            })
            .filter(|(_, r)| overlap(*r, view).is_some())
            .collect()
    }

    fn call_panel(&self, hh: f32) -> [f32; 4] {
        let r = broadcast::broadcast_rect(hh);
        [14.0, r[1] + r[3] * 0.2, 372.0, 210.0]
    }

    /// The buttons that apply now, with their screen rectangles.
    pub(super) fn hud_buttons(&self, w: f32, h: f32) -> Vec<(Btn, [f32; 4])> {
        let l = self.layout(w, h);
        l.buttons
            .into_iter()
            .map(|(b, r)| (b, l.st.rect(r[0], r[1], r[2], r[3])))
            .collect()
    }

    pub fn hud(&self, w: u32, h: u32, menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let mut list = DrawList {
            clear: [40, 60, 100, 255],
            ..Default::default()
        };
        self.own_menu.set(menu);
        let l = self.layout(w, h);
        let mut p = Painter::new(&mut list, l.st);
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            art::ballpark(&mut p, self.clock);
            if menu {
                match self.flow.phase {
                    FlowPhase::Result => self.draw_record(&mut p),
                    _ => self.draw_title(&mut p),
                }
            }
            return list;
        }
        let month = board::month(self.s.week);
        match &self.mode {
            Mode::Match { shown, t, asking } => {
                self.draw_match(&mut p, *shown, *t, *asking, &l);
            }
            Mode::MatchIntro => self.draw_match_intro(&mut p, &l),
            Mode::Bracket => self.draw_bracket(&mut p, &l),
            Mode::MatchEnd => self.draw_match_end(&mut p, &l),
            Mode::Roster => {
                art::backdrop(&mut p, month, self.clock);
                self.draw_bar(&mut p);
                self.draw_roster(&mut p, &l);
            }
            Mode::Lineup { pick } => {
                art::backdrop(&mut p, month, self.clock);
                self.draw_lineup(&mut p, *pick, &l);
            }
            Mode::Epilogue { schools } => {
                // The ballpark at dusk behind the milestone.
                art::ballpark(&mut p, self.clock);
                let [x0, y0, w, h] = p.st.screen;
                p.fill(
                    &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
                    Paint::vertical(y0, y0 + h, [10, 14, 34, 120], [10, 14, 34, 215]),
                );
                self.draw_epilogue(&mut p, *schools, &l);
            }
            Mode::Player(i) => {
                art::backdrop(&mut p, month, self.clock);
                self.draw_player(&mut p, *i, &l);
            }
            _ => {
                art::backdrop(&mut p, month, self.clock);
                self.draw_bar(&mut p);
                self.draw_next(&mut p);
                self.draw_board(&mut p);
                self.draw_manager(&mut p);
                if matches!(self.mode, Mode::Plan) {
                    self.draw_hand(&mut p, &l);
                }
            }
        }
        if let (Mode::Notice, Some(_)) = (&self.mode, self.notes.first()) {
            self.draw_notice(&mut p);
        }
        if let Mode::Ask { h, who } = self.mode {
            self.draw_ask(&mut p, h, who, &l);
        }
        if let Mode::CampTalk { who } = self.mode {
            self.draw_camp_talk(&mut p, who, &l);
        }
        if let Mode::Decide(d) = self.mode {
            self.draw_decide(&mut p, d, &l);
        }
        if !self.pops.is_empty() {
            p.layer();
            self.pops.draw(&mut p);
        }
        if let Some(t) = &self.transition {
            t.draw(&mut p);
        }
        if menu && self.flow.phase == FlowPhase::Pause {
            self.draw_pause(&mut p);
        }
        list
    }

    /// The stat rows on a player's page (design units), for the words that pop up there.
    pub(super) fn raise_rows(&self, w: f32, h: f32) -> Vec<(Stat, [f32; 4])> {
        self.layout(w, h)
            .buttons
            .into_iter()
            .filter_map(|(b, r)| match b {
                Btn::Raise(st) => Some((st, r)),
                _ => None,
            })
            .collect()
    }

    // ---- our own cards, when the host has no HTML ones (Android, desktop)

    fn draw_title(&self, p: &mut Painter) {
        let ja = self.lang_ja;
        let hh = p.st.h;
        let y = hh * 0.16;
        p.glow(200.0, y + 40.0, 230.0, [255, 232, 180, 150]);
        super::sprites::ball(p, 12.0, y + 8.0, 34.0);
        if ja {
            p.text_outlined(
                "白球デイズ",
                200.0,
                y,
                56.0,
                WHITE,
                [30, 40, 90, 255],
                3.5,
                TextAlign::Center,
            );
            // The display face in gold, ringed in dark so it reads on the sky.
            for (dx, dy) in [(-1.6, 0.0), (1.6, 0.0), (0.0, -1.6), (0.0, 1.6), (1.2, 1.8)] {
                p.text_display(
                    "DIAMOND DAYS",
                    200.0 + dx,
                    y + 72.0 + dy,
                    22.0,
                    [60, 34, 24, 255],
                    TextAlign::Center,
                );
            }
            p.text_display(
                "DIAMOND DAYS",
                200.0,
                y + 72.0,
                22.0,
                [255, 206, 110, 255],
                TextAlign::Center,
            );
        } else {
            p.text_outlined(
                "Diamond Days",
                200.0,
                y + 8.0,
                46.0,
                WHITE,
                [30, 40, 90, 255],
                3.0,
                TextAlign::Center,
            );
        }
        p.text_outlined(
            self.tr(
                "Summer after summer. Your calls.",
                "— 何度でも夏を、きみの采配で —",
            ),
            200.0,
            y + 108.0,
            16.0,
            WHITE,
            [20, 30, 70, 255],
            2.4,
            TextAlign::Center,
        );
        // A kept run waits: say where it stopped.
        let (face, outfit) = art::manager();
        let by = hh - 282.0;
        let msg = match &self.resume {
            Some(s) => {
                let m = board::month(s.week);
                let wk = (s.week - 1) % 4 + 1;
                if ja {
                    format!(
                        "監督、待ってましたよ！ {}年目 {m}月 第{wk}週、{}勝{}敗からです。",
                        super::career_year(s, self.spec.years),
                        s.wins,
                        s.losses
                    )
                } else {
                    format!(
                        "Welcome back, coach! Year {}, {} week {wk}, {}-{}.",
                        super::career_year(s, self.spec.years),
                        month_en(m),
                        s.wins,
                        s.losses
                    )
                }
            }
            None => self
                .tr(
                    "Nice to meet you, coach! The Summer Stadium, however many years it takes!",
                    "はじめまして、監督！ 何年かかっても、夏の大舞台へ行きましょう！",
                )
                .to_string(),
        };
        p.portrait(8.0, by - 6.0, 110.0, &face, Expr::Smile, &outfit, 1.0);
        p.bubble(
            118.0,
            by + 6.0,
            272.0,
            86.0,
            112.0,
            by + 50.0,
            [255, 250, 236, 255],
            [240, 206, 150, 255],
        );
        p.text(
            self.tr("Kotori", "七瀬ことり"),
            132.0,
            by + 12.0,
            11.0,
            [214, 100, 120, 255],
            TextAlign::Left,
        );
        for (k, line) in wrap(&msg, 14.0, 246.0).iter().take(3).enumerate() {
            p.text(
                line,
                132.0,
                by + 29.0 + k as f32 * 19.0,
                14.0,
                INK,
                TextAlign::Left,
            );
        }
        let label = match (&self.resume, ja) {
            (Some(_), true) => "つづきから",
            (Some(_), false) => "CONTINUE",
            (None, true) => "はじめる",
            (None, false) => "START",
        };
        let pulse = 1.0 + (self.clock * 3.0).sin() * 0.03;
        let (bw, bh) = (220.0 * pulse, 58.0 * pulse);
        p.button(
            200.0 - bw * 0.5,
            hh - 132.0 - bh * 0.5,
            bw,
            bh,
            label,
            22.0,
            ORANGE,
            true,
        );
        p.text_shadow(
            self.tr("Tap anywhere", "どこかをタップ"),
            200.0,
            hh - 92.0,
            12.0,
            WHITE,
            [20, 24, 50, 200],
            TextAlign::Center,
        );
        p.text(
            self.tr(
                "All names are fictional",
                "名前・人物・団体はすべて架空のものです",
            ),
            200.0,
            hh - 24.0,
            10.0,
            [230, 230, 240, 220],
            TextAlign::Center,
        );
    }

    /// The coach retires: the career year by year (the latest years when it is long), the
    /// whole record, and a new start.
    fn draw_record(&self, p: &mut Painter) {
        let ja = self.lang_ja;
        let hh = p.st.h;
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::Solid([12, 16, 34, 120]),
        );
        let top = (hh * 0.1).max(56.0);
        let hist = &self.s.history;
        let fit = (((hh - top - 290.0) / 42.0).floor().max(3.0) as usize).min(10);
        let shown = &hist[hist.len().saturating_sub(fit)..];
        let rows = shown.len().max(1) as f32;
        let tall = 116.0 + rows * 42.0 + 132.0;
        p.panel(
            20.0,
            top,
            360.0,
            tall,
            24.0,
            [255, 253, 246, 255],
            [244, 236, 220, 255],
            18.0,
        );
        let years = hist.len().max(1);
        let ribbon = if ja {
            format!("{years}年間の夏")
        } else if years == 1 {
            "One summer".to_string()
        } else {
            format!("{years} summers")
        };
        p.ribbon(
            200.0,
            top - 22.0,
            270.0,
            50.0,
            &ribbon,
            18.0,
            [255, 150, 60, 255],
        );
        let mut y = top + 44.0;
        if hist.len() > shown.len() {
            let more = hist.len() - shown.len();
            let line = if ja {
                format!("…ほか{more}年")
            } else {
                format!("... {more} earlier years")
            };
            p.text(&line, 360.0, y - 10.0, 12.0, INK_SOFT, TextAlign::Right);
            y += 8.0;
        }
        for log in shown {
            let yr = if ja {
                format!("{}年目", log.year)
            } else {
                format!("Year {}", log.year)
            };
            p.text(&yr, 44.0, y, 14.0, INK_SOFT, TextAlign::Left);
            p.text(
                if ja { &log.best_ja } else { &log.best },
                120.0,
                y - 2.0,
                16.0,
                INK,
                TextAlign::Left,
            );
            let sub = if ja {
                format!("{}  {}勝{}敗", log.school_ja, log.wins, log.losses)
            } else {
                format!("{}  {}-{}", log.school, log.wins, log.losses)
            };
            p.text(&sub, 120.0, y + 18.0, 12.0, INK_SOFT, TextAlign::Left);
            if log.tier() < 3 {
                super::sprites::medal(p, 334.0, y - 8.0, log.tier());
            }
            y += 42.0;
        }
        let (w_all, l_all) = (
            self.s.career_wins + self.s.wins,
            self.s.career_losses + self.s.losses,
        );
        let crowns = hist.iter().filter(|l| l.tier() == 0).count();
        let rec = match (ja, crowns) {
            (true, 0) => format!("通算 {w_all}勝{l_all}敗"),
            (true, n) => format!("通算 {w_all}勝{l_all}敗  日本一{n}回"),
            (false, 0) => format!("{w_all} wins, {l_all} losses"),
            (false, n) => format!("{w_all}-{l_all}, {n} national titles"),
        };
        p.text(&rec, 200.0, y + 4.0, 20.0, ORANGE, TextAlign::Center);
        if !self.s.pros.is_empty() {
            let pros = if ja {
                format!("プロ入りOB {}人", self.s.pros.len())
            } else {
                format!("{} former players went pro", self.s.pros.len())
            };
            p.text(&pros, 200.0, y + 30.0, 14.0, INK_SOFT, TextAlign::Center);
        }
        let (face, outfit) = art::manager();
        p.portrait(
            28.0,
            top + tall - 118.0,
            104.0,
            &face,
            Expr::Smile,
            &outfit,
            1.0,
        );
        let thanks = if ja {
            format!("監督、{years}年間ありがとうございました！")
        } else {
            format!("Thank you for {years} years, coach!")
        };
        for (k, line) in wrap(&thanks, 14.0, 230.0).iter().take(2).enumerate() {
            p.text(
                line,
                140.0,
                top + tall - 92.0 + k as f32 * 20.0,
                14.0,
                INK,
                TextAlign::Left,
            );
        }
        // Always shown: the clock stands still on this card.
        p.text(
            self.tr("Tap to start a new career", "タップで新しい監督生活へ"),
            200.0,
            top + tall + 18.0,
            15.0,
            WHITE,
            TextAlign::Center,
        );
    }

    fn draw_pause(&self, p: &mut Painter) {
        p.layer();
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::Solid([10, 14, 30, 170]),
        );
        let y = p.st.h * 0.5 - 90.0;
        p.panel(
            60.0,
            y,
            280.0,
            180.0,
            24.0,
            [255, 253, 246, 255],
            [244, 236, 220, 255],
            16.0,
        );
        p.text(
            self.tr("Paused", "ひと休み"),
            200.0,
            y + 34.0,
            26.0,
            INK,
            TextAlign::Center,
        );
        p.text(
            self.tr("Your run is kept.", "ここまでの記録は保存されています"),
            200.0,
            y + 80.0,
            13.0,
            INK_SOFT,
            TextAlign::Center,
        );
        p.text(
            self.tr("Tap to go on", "タップで再開"),
            200.0,
            y + 124.0,
            17.0,
            ORANGE,
            TextAlign::Center,
        );
    }

    // ---- the week's screen

    fn draw_bar(&self, p: &mut Painter) {
        let ja = self.lang_ja;
        p.panel(
            10.0,
            10.0,
            380.0,
            54.0,
            16.0,
            [255, 255, 255, 245],
            [244, 240, 230, 245],
            10.0,
        );
        let m = board::month(self.s.week);
        let wk = (self.s.week - 1) % 4 + 1;
        let (top, date) = if ja {
            (
                format!("{}年目", self.career_year()),
                format!("{m}月 第{wk}週"),
            )
        } else {
            (
                format!("Year {}", self.career_year()),
                format!("{} week {wk}", month_en(m)),
            )
        };
        p.text(&top, 24.0, 15.0, 12.0, INK_SOFT, TextAlign::Left);
        p.text(&date, 24.0, 30.0, 21.0, INK, TextAlign::Left);
        let g = grade(self.s.team.strength().round() as u8);
        p.text(
            self.tr("TEAM", "戦力"),
            196.0,
            16.0,
            11.0,
            INK_SOFT,
            TextAlign::Center,
        );
        p.chip(196.0, 44.0, 26.0, g, grade_color(g));
        let stars = board::fame_stars(self.s.reputation) as usize;
        let (rank_en, rank_ja) = board::rank_title(self.s.reputation);
        p.text(
            self.tr(rank_en, rank_ja),
            242.0,
            16.0,
            11.0,
            INK_SOFT,
            TextAlign::Left,
        );
        for i in 0..5 {
            if i < stars {
                super::sprites::kenney_star(p, 288.0 + i as f32 * 18.0, 22.0, 16.0);
            } else {
                p.star(288.0 + i as f32 * 18.0, 22.0, 8.0, [206, 204, 214, 255]);
            }
        }
        let record = if ja {
            format!("{}勝{}敗", self.s.wins, self.s.losses)
        } else {
            format!("W{} L{}", self.s.wins, self.s.losses)
        };
        p.text(&record, 376.0, 36.0, 16.0, INK, TextAlign::Right);
    }

    fn next_fixture(&self) -> Option<(u8, Fixture)> {
        if let Some(f) = self.s.cup {
            return Some((self.s.week, f));
        }
        (self.s.week..=board::WEEKS).find_map(|wk| {
            board::fixture(wk)
                .filter(|f| self.plays_in(*f))
                .map(|f| (wk, f))
        })
    }

    fn draw_next(&self, p: &mut Painter) {
        p.panel(
            10.0,
            72.0,
            284.0,
            44.0,
            14.0,
            [44, 58, 104, 245],
            [26, 34, 68, 245],
            8.0,
        );
        p.button(
            302.0,
            72.0,
            88.0,
            44.0,
            self.tr("TEAM", "部員"),
            16.0,
            [60, 120, 210, 255],
            true,
        );
        let Some((wk, f)) = self.next_fixture() else {
            p.text(
                self.tr("No more games this year", "今年の試合は終わり"),
                24.0,
                85.0,
                15.0,
                WHITE,
                TextAlign::Left,
            );
            return;
        };
        let (en, ja) = Self::fixture_name(f);
        p.text(
            self.tr("NEXT GAME", "次の試合"),
            22.0,
            78.0,
            10.0,
            [180, 196, 240, 255],
            TextAlign::Left,
        );
        p.text(
            if self.lang_ja { &ja } else { &en },
            22.0,
            92.0,
            16.0,
            WHITE,
            TextAlign::Left,
        );
        let left = wk - self.s.week;
        let when = match (left, self.lang_ja) {
            (0, true) => "今週！".to_string(),
            (0, false) => "THIS WEEK".to_string(),
            (n, true) => format!("あと{n}週"),
            (n, false) => format!("in {n} wk"),
        };
        p.text(
            &when,
            284.0,
            84.0,
            17.0,
            [255, 190, 96, 255],
            TextAlign::Right,
        );
    }

    /// The practice ground (top) and the road the coach walks (below it), above Kotori
    /// and the cards.
    fn board_layout(&self, hh: f32) -> ([f32; 4], [f32; 4]) {
        let y_m = hh - 112.0 - 100.0;
        let road = [10.0, y_m - 128.0, 380.0, 120.0];
        let ground = [10.0, 124.0, 380.0, (road[1] - 8.0 - 124.0).max(110.0)];
        (ground, road)
    }

    /// The week's board: the club at practice on the school's ground, and under it the
    /// road of squares running to the right. The coach stands near the left; the squares
    /// ahead carry their step numbers (a card's number is where it lands), and as he walks
    /// the road slides by.
    fn draw_board(&self, p: &mut Painter) {
        let (ground, road) = self.board_layout(p.st.h);
        let team = &self.s.team;
        let mut crew: Vec<_> = team
            .players
            .iter()
            .filter(|pl| pl.can_play())
            .take(10)
            .map(|pl| (pl.menu, art::player_face(pl), art::mood_expr(pl.mood)))
            .collect();
        if crew.is_empty() {
            crew = team
                .players
                .iter()
                .take(6)
                .map(|pl| (pl.menu, art::player_face(pl), Expr::Sad))
                .collect();
        }
        art::school_ground(p, ground, &crew, self.clock);

        // The road: a dirt path between two strips of grass.
        let [rx, ry, rw, rh] = road;
        p.shadow(rx, ry, rw, rh, 16.0, 8.0, 70);
        p.fill(
            &vector::rounded_rect(rx, ry, rw, rh, 16.0),
            Paint::vertical(ry, ry + rh, [118, 178, 96, 255], [86, 146, 72, 255]),
        );
        let (py, ph) = (ry + 30.0, 72.0);
        p.fill(
            &vector::rounded_rect(rx + 6.0, py, rw - 12.0, ph, 12.0),
            Paint::vertical(py, py + ph, [214, 176, 128, 255], [186, 146, 100, 255]),
        );
        for y in [py + 2.0, py + ph - 2.0] {
            p.line(
                &[[rx + 12.0, y], [rx + rw - 12.0, y]],
                1.5,
                [255, 255, 255, 120],
            );
        }
        let n = self.spec.ring.len().max(1);
        let here = self.s.pos % n;
        // The coach's square and the six ahead all fit on the road: a 6 shows where it lands.
        let x_here = rx + 34.0;
        let pitch = (rw - 64.0) / 6.0;
        let (tw, th) = (pitch - 5.0, 60.0);
        let cy = py + ph * 0.5;
        let walking = matches!(self.mode, Mode::Walk { .. });
        let k = if walking {
            self.hop.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let slide = k * k * (3.0 - 2.0 * k) * pitch;
        // The steps a card can take, brighter where a card in hand lands.
        let in_hand = |d: usize| self.s.hand.iter().any(|c| c.steps as usize == d);
        p.clip_layer([rx + 2.0, ry, rw - 4.0, rh]);
        for d in -1i32..=7 {
            let i = (here as i32 + d).rem_euclid(n as i32) as usize;
            let x = x_here + d as f32 * pitch - slide;
            let sq = self.spec.ring[i];
            art::road_tile(
                p,
                [x, cy],
                tw,
                th,
                sq,
                square_label(sq, self.lang_ja),
                d == 0 && !walking,
            );
            if (1..=6).contains(&d) && matches!(self.mode, Mode::Plan) {
                let lit = in_hand(d as usize);
                p.dot(
                    x,
                    ry + 15.0,
                    10.0,
                    if lit {
                        [255, 196, 60, 255]
                    } else {
                        [40, 70, 50, 200]
                    },
                );
                p.text(
                    &d.to_string(),
                    x,
                    ry + 7.0,
                    13.0,
                    if lit { INK } else { [210, 230, 210, 255] },
                    TextAlign::Center,
                );
            }
        }
        p.layer();
        // The coach: on his square, hopping as the road slides.
        let lift = (k * std::f32::consts::PI).sin() * 18.0;
        art::coach_token(p, [x_here, cy - 6.0], 48.0, lift);
        if let Mode::Walk { left, .. } = &self.mode {
            let txt = if self.lang_ja {
                format!("あと{left}マス")
            } else {
                format!("{left} to go")
            };
            p.panel(
                rx + rw - 128.0,
                ry + 4.0,
                118.0,
                24.0,
                12.0,
                [255, 255, 255, 235],
                [246, 240, 228, 235],
                4.0,
            );
            p.text(&txt, rx + rw - 69.0, ry + 8.0, 14.0, INK, TextAlign::Center);
            p.text(
                self.tr("Tap to skip the walk", "タップでスキップ"),
                rx + rw * 0.5,
                py + ph + 3.0,
                11.0,
                [236, 248, 230, 255],
                TextAlign::Center,
            );
        } else if matches!(self.mode, Mode::Plan) {
            p.text(
                self.tr(
                    "A card moves you its number of squares",
                    "カードの数字のマスだけ進みます",
                ),
                rx + rw * 0.5,
                py + ph + 3.0,
                11.0,
                [236, 248, 230, 255],
                TextAlign::Center,
            );
        }
    }

    /// The manager and what she says this week.
    fn draw_manager(&self, p: &mut Painter) {
        let hh = p.st.h;
        let y = hh - 112.0 - 96.0;
        let (face, outfit) = art::manager();
        let tired = self
            .s
            .team
            .players
            .iter()
            .map(|q| 100 - q.stamina as u32)
            .sum::<u32>() as f32
            / self.s.team.players.len().max(1) as f32;
        let hurt = self.s.team.players.iter().find(|q| q.hurt > 0);
        let soon = self.next_fixture().map(|(wk, _)| wk - self.s.week);
        let (msg, expr) = if matches!(self.mode, Mode::Walk { .. }) {
            (
                self.tr("Here we go!", "それっ、進みますよ！").to_string(),
                Expr::Smile,
            )
        } else if tired > 55.0 {
            (
                self.tr(
                    "Everyone is worn out... a rest square would help.",
                    "みんな疲れてます……。休養のマスも大事ですよ。",
                )
                .to_string(),
                Expr::Sad,
            )
        } else if let Some(q) = hurt {
            (
                if self.lang_ja {
                    format!(
                        "{}くんがケガで休んでます。無理はさせないでくださいね。",
                        q.family
                    )
                } else {
                    format!("{} is hurt. Let him heal, coach.", q.family)
                },
                Expr::Serious,
            )
        } else if soon.is_some_and(|n| n <= 2) {
            (
                self.tr(
                    "A game is coming up! Let's get in shape!",
                    "もうすぐ試合です！ 調子を整えていきましょう！",
                )
                .to_string(),
                Expr::Fiery,
            )
        } else {
            (
                self.tr(
                    "Which card this week, coach?",
                    "今週はどのカードにしますか、監督？",
                )
                .to_string(),
                Expr::Happy,
            )
        };
        p.portrait(6.0, y - 4.0, 92.0, &face, expr, &outfit, 1.0);
        p.bubble(
            102.0,
            y + 8.0,
            288.0,
            76.0,
            98.0,
            y + 46.0,
            [255, 250, 236, 255],
            [240, 206, 150, 255],
        );
        p.text(
            self.tr("Kotori", "七瀬ことり"),
            116.0,
            y + 13.0,
            11.0,
            [214, 100, 120, 255],
            TextAlign::Left,
        );
        for (k, line) in wrap(&msg, 14.0, 260.0).iter().take(2).enumerate() {
            p.text(
                line,
                116.0,
                y + 30.0 + k as f32 * 21.0,
                14.0,
                INK,
                TextAlign::Left,
            );
        }
    }

    fn draw_hand(&self, p: &mut Painter, l: &Layout) {
        for (b, r) in &l.buttons {
            let Btn::Card(i) = *b else { continue };
            let Some(c) = self.s.hand.get(i) else {
                continue;
            };
            let [x, y, w, h] = *r;
            let col = c.focus.map(art::drill_color).unwrap_or([96, 112, 170, 255]);
            super::sprites::paper(p, x, y, w, h);
            p.outline(
                &vector::rounded_rect(x, y, w, h, 12.0),
                1.6,
                art::PAPER_EDGE,
            );
            let band = vector::rounded_rect(x + 5.0, y + 5.0, w - 10.0, 34.0, 9.0);
            p.fill(
                &band,
                Paint::vertical(y + 5.0, y + 39.0, shade(col, 0.2), shade(col, -0.12)),
            );
            p.text_display(
                &c.steps.to_string(),
                x + w * 0.5,
                y + 6.0,
                28.0,
                WHITE,
                TextAlign::Center,
            );
            if let Some(star) = &c.star {
                if let Some(pl) = self.s.team.players.iter().find(|q| &q.family == star) {
                    p.portrait(
                        x + w * 0.5 - 20.0,
                        y + 42.0,
                        40.0,
                        &art::player_face(pl),
                        Expr::Happy,
                        &Outfit::cap(OURS),
                        1.0,
                    );
                }
                p.text(
                    star,
                    x + w * 0.5,
                    y + h - 18.0,
                    11.0,
                    [110, 80, 40, 255],
                    TextAlign::Center,
                );
            } else if let Some(d) = c.focus {
                p.text(
                    &format!("{}×2", drill_label(d, self.lang_ja)),
                    x + w * 0.5,
                    y + h - 30.0,
                    13.0,
                    shade(col, -0.35),
                    TextAlign::Center,
                );
                p.text(
                    self.tr("steps", "マス"),
                    x + w * 0.5,
                    y + 50.0,
                    11.0,
                    INK_SOFT,
                    TextAlign::Center,
                );
            } else {
                p.text(
                    self.tr("steps", "マス進む"),
                    x + w * 0.5,
                    y + 58.0,
                    12.0,
                    INK_SOFT,
                    TextAlign::Center,
                );
            }
        }
    }

    /// The notice's lines as drawn: the first paragraph wraps at its bigger size.
    fn notice_lines(&self, n: &super::Note) -> Vec<String> {
        n.lines
            .iter()
            .enumerate()
            .flat_map(|(i, (en, jp))| {
                wrap(
                    if self.lang_ja { jp } else { en },
                    if i == 0 { 18.0 } else { 16.0 },
                    312.0,
                )
            })
            .collect()
    }

    /// The first notice's panel on a stage `stage_h` tall: its top and its height.
    fn notice_box(&self, stage_h: f32) -> Option<(f32, f32)> {
        let n = self.notes.first()?;
        let lines = self.notice_lines(n).len();
        // Faces in rows of four under the lines (who graduates, who joins).
        let faces_h = n.faces.len().min(12).div_ceil(4) as f32 * 92.0;
        // Lines from 46 below the top; the manager and the prompt take the last 104.
        let tall = (46.0 + lines as f32 * 25.0 + faces_h + 112.0)
            .max(220.0)
            .min(stage_h - 80.0);
        Some(((stage_h - tall) * 0.5, tall))
    }

    fn draw_notice(&self, p: &mut Painter) {
        let Some(n) = self.notes.first() else { return };
        let Some((y, tall)) = self.notice_box(p.st.h) else {
            return;
        };
        let ja = self.lang_ja;
        p.layer();
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::Solid([12, 16, 34, 185]),
        );
        let lines = self.notice_lines(n);
        let faces_h = n.faces.len().min(12).div_ceil(4) as f32 * 92.0;
        p.panel(
            20.0,
            y,
            360.0,
            tall,
            24.0,
            [255, 253, 246, 255],
            [244, 236, 220, 255],
            18.0,
        );
        super::sprites::paper(p, 20.0, y, 360.0, tall);
        let title = if ja { &n.title_ja } else { &n.title };
        super::sprites::banner(p, 70.0, y - 26.0, 260.0, 50.0);
        p.text_outlined(
            title,
            200.0,
            y - 12.0,
            18.0,
            WHITE,
            [90, 24, 28, 255],
            2.0,
            TextAlign::Center,
        );
        let max_lines = ((tall - 46.0 - 104.0 - faces_h) / 25.0).max(1.0) as usize;
        let fy = y + 46.0 + lines.len().min(max_lines) as f32 * 25.0 + 6.0;
        let fresh = n.title == "New faces";
        for (k, (fam, given)) in n.faces.iter().take(12).enumerate() {
            let (col, row) = (k % 4, k / 4);
            let in_row = (n.faces.len().min(12) - row * 4).min(4);
            let x = 200.0 - in_row as f32 * 41.0 + col as f32 * 82.0 + 9.0;
            let yy = fy + row as f32 * 92.0;
            let face = match n.roles.get(k).copied() {
                Some(team::Role::Ace) => art::ace_face(),
                Some(team::Role::Captain) => art::captain_face(),
                _ => art::face_of(&format!("{fam}{given}")),
            };
            let expr = if fresh { Expr::Happy } else { Expr::Smile };
            p.portrait(x, yy, 64.0, &face, expr, &Outfit::cap(OURS), 1.0);
            p.text(fam, x + 32.0, yy + 62.0, 12.0, INK, TextAlign::Center);
            if let Some((en, jp)) = n.captions.get(k) {
                p.text(
                    if ja { jp } else { en },
                    x + 32.0,
                    yy + 76.0,
                    art::TYPE_CAPTION,
                    ORANGE,
                    TextAlign::Center,
                );
            }
        }
        for (k, ln) in lines.iter().take(max_lines).enumerate() {
            // The first line (what happened) a little bigger.
            let (size, col) = if k == 0 {
                (18.0, INK)
            } else {
                (16.0, [52, 58, 76, 255])
            };
            p.text(
                ln,
                44.0,
                y + 44.0 + k as f32 * 25.0,
                size,
                col,
                TextAlign::Left,
            );
        }
        let (face, outfit) = art::manager();
        let good = n
            .lines
            .iter()
            .any(|(e, _)| e.contains('+') || e.contains("win") || e.contains("Win"));
        p.portrait(
            28.0,
            y + tall - 96.0,
            88.0,
            &face,
            if good { Expr::Smile } else { Expr::Happy },
            &outfit,
            1.0,
        );
        if (self.clock * 2.0) as i32 % 2 == 0 {
            p.text(
                self.tr("Tap to go on", "タップで次へ"),
                360.0,
                y + tall - 34.0,
                14.0,
                ORANGE,
                TextAlign::Right,
            );
        }
        // More cards waiting: close them all at once.
        let more = self.notes.len() - 1;
        if more > 0 {
            let label = if ja {
                format!("まとめて送る（あと{more}）")
            } else {
                format!("Skip all ({more} more)")
            };
            p.button(
                214.0,
                y + tall - 82.0,
                146.0,
                36.0,
                &label,
                13.0,
                [110, 116, 140, 255],
                true,
            );
        }
    }

    fn draw_ask(&self, p: &mut Painter, h: usize, who: usize, l: &Layout) {
        let Some(ev) = self.spec.happenings.get(h) else {
            return;
        };
        self.draw_talk(p, ev, who, l);
    }

    fn draw_camp_talk(&self, p: &mut Painter, who: usize, l: &Layout) {
        let ev = board::camp_talk();
        self.draw_talk(p, &ev, who, l);
    }

    /// A player talks to the coach: his face, what he says, the answers.
    fn draw_talk(&self, p: &mut Painter, ev: &board::Happening, who: usize, l: &Layout) {
        let Some(pl) = self.s.team.players.get(who) else {
            return;
        };
        let ja = self.lang_ja;
        p.layer();
        let [x0, y0, w, hgt] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + hgt], [x0, y0 + hgt]],
            Paint::Solid([12, 16, 34, 170]),
        );
        let hh = p.st.h;
        let first_btn = l.buttons.iter().map(|(_, r)| r[1]).fold(hh, f32::min);
        // The speaker, big, standing behind the speech box.
        let size = (first_btn - 210.0).clamp(180.0, 300.0);
        let box_y = first_btn - 150.0;
        let expr = match ev.who {
            board::Who::Pitcher => Expr::Serious,
            board::Who::Rookie => Expr::Surprised,
            _ if ev.id == "slump" => Expr::Sad,
            _ => Expr::Fiery,
        };
        p.glow(130.0, box_y - size * 0.45, size * 0.8, [255, 236, 190, 90]);
        p.portrait(
            130.0 - size * 0.5,
            box_y - size + 18.0,
            size,
            &art::player_face(pl),
            expr,
            &Outfit::cap(OURS),
            1.0,
        );
        p.ribbon(
            200.0,
            24.0,
            200.0,
            44.0,
            self.tr("A word with the coach", "部員との会話"),
            16.0,
            [90, 130, 210, 255],
        );
        p.panel_bordered(
            16.0,
            box_y,
            368.0,
            132.0,
            22.0,
            [255, 253, 246, 252],
            [246, 238, 222, 252],
            [230, 210, 170, 255],
            2.5,
            14.0,
        );
        let plate = format!("{} {}", pl.family, pl.given);
        let pw = crate::font::measure_text(&plate, 15.0) + 34.0;
        p.panel(
            32.0,
            box_y - 16.0,
            pw,
            32.0,
            16.0,
            shade(OURS, 0.15),
            shade(OURS, -0.1),
            6.0,
        );
        p.text(
            &plate,
            32.0 + pw * 0.5,
            box_y - 9.0,
            15.0,
            WHITE,
            TextAlign::Center,
        );
        let said = if ja { &ev.text_ja } else { &ev.text }.replace("{name}", &pl.family);
        for (k, line) in wrap(&said, 16.0, 330.0).iter().take(4).enumerate() {
            p.text(
                line,
                34.0,
                box_y + 26.0 + k as f32 * 25.0,
                16.0,
                INK,
                TextAlign::Left,
            );
        }
        for (b, r) in &l.buttons {
            let Btn::Choose(k) = *b else { continue };
            let Some(c) = ev.choices.get(k) else { continue };
            let col = if k == 0 {
                [60, 120, 210, 255]
            } else {
                [70, 160, 110, 255]
            };
            p.button(
                r[0],
                r[1],
                r[2],
                r[3],
                if ja { &c.text_ja } else { &c.text },
                17.0,
                col,
                true,
            );
        }
    }

    // ---- the roster and a player's page

    fn draw_roster(&self, p: &mut Painter, l: &Layout) {
        let ja = self.lang_ja;
        p.panel(
            10.0,
            72.0,
            380.0,
            46.0,
            14.0,
            [44, 58, 104, 245],
            [26, 34, 68, 245],
            8.0,
        );
        let title = if ja {
            format!("部員一覧  {}人", self.s.team.players.len())
        } else {
            format!("The club  {}", self.s.team.players.len())
        };
        p.text(&title, 24.0, 84.0, 18.0, WHITE, TextAlign::Left);
        self.draw_list(p, |me, p, i, r| me.draw_row(p, i, r));
        for (b, r) in &l.buttons {
            let [x, y, w, h] = *r;
            match *b {
                Btn::RosterUp => {
                    p.button(
                        x,
                        y,
                        w,
                        h,
                        "▲",
                        14.0,
                        [80, 100, 160, 255],
                        self.roster.offset > 0.5,
                    );
                }
                Btn::RosterDown => {
                    let more = self.roster.offset < self.roster.max() - 0.5;
                    p.button(x, y, w, h, "▼", 14.0, [80, 100, 160, 255], more);
                }
                Btn::Lineup => {
                    super::sprites::button(p, [x, y, w, h], self.tr("ORDER", "打順"), 15.0, false);
                }
                Btn::AutoRaise => {
                    super::sprites::button(p, [x, y, w, h], self.tr("SPEND", "育成"), 15.0, true);
                }
                Btn::Close => {
                    p.button(
                        x,
                        y,
                        w,
                        h,
                        self.tr("CLOSE", "とじる"),
                        16.0,
                        [110, 116, 140, 255],
                        true,
                    );
                }
                _ => {}
            }
        }
    }

    /// The rows in the list window, clipped to it, with a soft edge where they run on.
    fn draw_list(&self, p: &mut Painter, row: impl Fn(&Self, &mut Painter, usize, [f32; 4])) {
        let view = list_view(p.st.h);
        p.clip_layer(view);
        for (i, r) in self.list_rows(p.st.h) {
            row(self, p, i, r);
        }
        p.layer();
        // A thin bar shows where in the list the window is.
        let max = self.roster.max();
        if max > 1.0 {
            let frac = view[3] / (view[3] + max);
            let bar_h = (view[3] * frac).max(24.0);
            let k = (self.roster.offset / max).clamp(0.0, 1.0);
            let y = view[1] + (view[3] - bar_h) * k;
            p.fill(
                &vector::rounded_rect(view[0] + view[2] - 4.0, y, 4.0, bar_h, 2.0),
                Paint::Solid([255, 255, 255, 150]),
            );
        }
    }

    fn draw_row(&self, p: &mut Painter, i: usize, r: [f32; 4]) {
        let ja = self.lang_ja;
        let pl = &self.s.team.players[i];
        let [x, y, w, h] = r;
        let hurt = !pl.can_play();
        p.panel(
            x,
            y,
            w,
            h,
            14.0,
            [255, 255, 255, 250],
            [246, 242, 232, 250],
            6.0,
        );
        p.portrait(
            x + 4.0,
            y + 3.0,
            52.0,
            &art::player_face(pl),
            if hurt {
                Expr::Pain
            } else {
                art::mood_expr(pl.mood)
            },
            &Outfit::cap(OURS),
            1.0,
        );
        p.text(
            &format!("{} {}", pl.family, pl.given),
            x + 62.0,
            y + 8.0,
            16.0,
            INK,
            TextAlign::Left,
        );
        let sub = if ja {
            format!("{}年  {}", pl.grade, pl.pos.short(true))
        } else {
            format!("Yr {}  {}", pl.grade, pl.pos.short(false))
        };
        p.text(&sub, x + 62.0, y + 31.0, 12.0, INK_SOFT, TextAlign::Left);
        let (a, b) = if pl.pitcher() {
            (pl.pitch_rating(), pl.bat_rating())
        } else {
            (pl.bat_rating(), (pl.field as f32 + pl.arm as f32) * 0.5)
        };
        let (la, lb) = if pl.pitcher() {
            (self.tr("PIT", "投"), self.tr("BAT", "打"))
        } else {
            (self.tr("BAT", "打"), self.tr("DEF", "守"))
        };
        for (k, (lab, v)) in [(la, a), (lb, b)].into_iter().enumerate() {
            let cx = x + 210.0 + k as f32 * 44.0;
            let g = grade(v.round() as u8);
            p.text(lab, cx, y + 4.0, 10.0, INK_SOFT, TextAlign::Center);
            p.chip(cx, y + 34.0, 26.0, g, grade_color(g));
        }
        if hurt {
            p.text(
                if pl.study > 0 {
                    self.tr("grades", "赤点")
                } else {
                    self.tr("hurt", "ケガ")
                },
                x + 312.0,
                y + 10.0,
                13.0,
                [220, 70, 60, 255],
                TextAlign::Center,
            );
        } else {
            art::mood_icon(p, [x + 312.0, y + 20.0], 10.0, pl.mood);
        }
        p.gauge(
            x + 294.0,
            y + 38.0,
            36.0,
            8.0,
            pl.stamina as f32 / 100.0,
            stamina_color(pl.stamina),
            [226, 222, 212, 255],
        );
        let pts: u16 = pl.xp.iter().sum();
        p.text(
            &pts.to_string(),
            x + 370.0,
            y + 12.0,
            15.0,
            if pts >= 30 { ORANGE } else { INK },
            TextAlign::Right,
        );
        p.text(
            self.tr("pts", "pt"),
            x + 370.0,
            y + 33.0,
            10.0,
            INK_SOFT,
            TextAlign::Right,
        );
    }

    fn draw_lineup(&self, p: &mut Painter, pick: Option<usize>, l: &Layout) {
        let ja = self.lang_ja;
        let nine = self.s.team.resolved();
        let custom = !self.s.team.card.order.is_empty();
        p.panel(
            10.0,
            10.0,
            380.0,
            60.0,
            art::RADIUS,
            art::PAPER,
            art::PAPER_SHADE,
            10.0,
        );
        p.text(
            self.tr(
                if pick.is_some() {
                    "Who plays here?"
                } else if custom {
                    "Your order"
                } else {
                    "Auto order"
                },
                if pick.is_some() {
                    "この枠は誰？"
                } else if custom {
                    "あなたの打順"
                } else {
                    "おまかせ打順"
                },
            ),
            24.0,
            26.0,
            art::TYPE_TITLE,
            INK,
            TextAlign::Left,
        );
        if pick.is_some() {
            self.draw_list(p, |me, p, i, r| me.draw_row(p, i, r));
            for (b, r) in &l.buttons {
                let [x, y, w, h] = *r;
                match *b {
                    Btn::RosterUp => {
                        p.button(x, y, w, h, "▲", 14.0, art::NAVY, self.roster.offset > 0.5);
                    }
                    Btn::RosterDown => {
                        p.button(x, y, w, h, "▼", 14.0, art::NAVY, true);
                    }
                    Btn::Back => {
                        p.button(
                            x,
                            y,
                            w,
                            h,
                            self.tr("BACK", "もどる"),
                            16.0,
                            art::QUIET,
                            true,
                        );
                    }
                    _ => {}
                }
            }
            return;
        }
        for (b, r) in &l.buttons {
            let [x, y, w, h] = *r;
            match *b {
                Btn::Nudge(_, dir) => {
                    p.button(
                        x,
                        y,
                        w,
                        h,
                        if dir < 0 { "↑" } else { "↓" },
                        16.0,
                        art::NAVY,
                        true,
                    );
                }
                Btn::Slot(i) => {
                    let (name, pos) = nine
                        .get(i)
                        .map(|(pi, pos)| (self.s.team.players[*pi].family.as_str(), pos.short(ja)))
                        .unwrap_or(("-", "-"));
                    p.panel(x, y, w, h, 12.0, art::PAPER, art::PAPER_SHADE, 6.0);
                    p.text(
                        &format!("{}. {name}", i + 1),
                        x + 10.0,
                        y + 12.0,
                        art::TYPE_BUTTON,
                        INK,
                        TextAlign::Left,
                    );
                    let _ = pos;
                }
                Btn::Cycle(i) => {
                    let pos = nine.get(i).map(|(_, pos)| pos.short(ja)).unwrap_or("-");
                    p.button(x, y, w, h, pos, 16.0, OURS, true);
                }
                Btn::AutoFill => {
                    p.button(x, y, w, h, self.tr("AUTO", "おまかせ"), 16.0, ORANGE, true);
                }
                Btn::Close => {
                    p.button(x, y, w, h, self.tr("DONE", "決定"), 16.0, art::QUIET, true);
                }
                _ => {}
            }
        }
    }

    fn draw_epilogue(&self, p: &mut Painter, schools: bool, l: &Layout) {
        let ja = self.lang_ja;
        let hh = p.st.h;
        p.panel(
            16.0,
            36.0,
            368.0,
            hh * 0.36,
            art::RADIUS,
            art::PAPER,
            art::PAPER_SHADE,
            12.0,
        );
        p.outline(
            &vector::rounded_rect(16.0, 36.0, 368.0, hh * 0.36, art::RADIUS),
            1.5,
            art::PAPER_EDGE,
        );
        p.text(
            self.tr("A milestone", "ひと区切り"),
            200.0,
            52.0,
            art::TYPE_TITLE,
            INK,
            TextAlign::Center,
        );
        let (rank_en, rank_ja) = board::rank_title(self.s.reputation);
        let sub = if ja {
            format!(
                "{}年目  第{}期  {}勝{}敗  {rank_ja}",
                self.career_year(),
                self.chapter(),
                self.s.wins,
                self.s.losses,
            )
        } else {
            format!(
                "Year {}  chapter {}  {}-{}  {rank_en}",
                self.career_year(),
                self.chapter(),
                self.s.wins,
                self.s.losses,
            )
        };
        p.text(
            &sub,
            200.0,
            84.0,
            art::TYPE_BODY,
            INK_SOFT,
            TextAlign::Center,
        );
        let recent = &self.s.history[self.s.history.len().saturating_sub(3)..];
        for (i, log) in recent.iter().enumerate() {
            let line = if ja {
                format!("{}年目  {}", log.year, log.best_ja)
            } else {
                format!("Year {}  {}", log.year, log.best)
            };
            p.text(
                &line,
                36.0,
                118.0 + i as f32 * 26.0,
                art::TYPE_BUTTON,
                INK,
                TextAlign::Left,
            );
        }
        // The whole career so far, and the question.
        let (w_all, l_all) = (
            self.s.career_wins + self.s.wins,
            self.s.career_losses + self.s.losses,
        );
        let crowns = self.s.history.iter().filter(|l| l.tier() == 0).count();
        let career = match (ja, crowns) {
            (true, 0) => format!("通算 {w_all}勝{l_all}敗"),
            (true, n) => format!("通算 {w_all}勝{l_all}敗  日本一{n}回"),
            (false, 0) => format!("Career {w_all}-{l_all}"),
            (false, n) => format!("Career {w_all}-{l_all}, {n} national titles"),
        };
        let card_bottom = 36.0 + hh * 0.36;
        p.text(
            &career,
            200.0,
            200.0,
            art::TYPE_BUTTON,
            ORANGE,
            TextAlign::Center,
        );
        let ask = self.tr(
            "Three more years? The club, the town and the next first-years are waiting.",
            "次の3年はどうしますか？ 部も町も、次の新入生も待っています。",
        );
        for (k, line) in wrap(ask, 14.0, 320.0).iter().take(3).enumerate() {
            let y = 236.0 + k as f32 * 20.0;
            if y + 20.0 > card_bottom {
                break;
            }
            p.text(line, 40.0, y, 14.0, INK, TextAlign::Left);
        }
        let (face, outfit) = art::manager();
        p.portrait(280.0, 110.0, 90.0, &face, Expr::Smile, &outfit, 1.0);
        // The three ways on, each with what it means.
        if !schools {
            let top = hh * 0.47;
            p.text_outlined(
                self.tr("What next?", "これからどうする？"),
                200.0,
                top,
                18.0,
                WHITE,
                [16, 18, 30, 255],
                2.0,
                TextAlign::Center,
            );
            for (b, r) in &l.buttons {
                let what = match *b {
                    Btn::Stay => self.tr(
                        "Same school: fame and the grounds carry over; new first-years arrive.",
                        "評判も練習場もそのまま。次の新入生がやってくる。",
                    ),
                    Btn::MoveSchool => self.tr(
                        "A weaker school, from the start. The old one stays on the schedule.",
                        "弱い学校で一からやり直す。今の学校は対戦相手になる。",
                    ),
                    Btn::EndStory => self.tr(
                        "End the career here and keep the record.",
                        "監督人生をここで終えて、通算成績を残す。",
                    ),
                    _ => continue,
                };
                for (i, line) in wrap(what, 12.0, 340.0).iter().take(2).enumerate() {
                    p.text(
                        line,
                        200.0,
                        r[1] + r[3] + 4.0 + i as f32 * 14.0,
                        12.0,
                        [226, 232, 246, 255],
                        TextAlign::Center,
                    );
                }
            }
        }
        for (b, r) in &l.buttons {
            let [x, y, w, h] = *r;
            let (label, col) = match *b {
                Btn::Stay => (self.tr("Keep going here", "この学校で続ける"), ORANGE),
                Btn::MoveSchool => (
                    self.tr("Move to another school", "別の学校へ移る"),
                    [60, 120, 210, 255],
                ),
                Btn::EndStory => (self.tr("Retire here", "ここで引退する"), art::QUIET),
                Btn::School(k) => {
                    let name = self
                        .move_picks
                        .get(k as usize)
                        .and_then(|&i| self.s.rivals.get(i))
                        .map(|s| {
                            if ja {
                                s.name_ja.as_str()
                            } else {
                                s.name.as_str()
                            }
                        })
                        .unwrap_or("-");
                    p.button(x, y, w, h, name, art::TYPE_BUTTON, OURS, true);
                    continue;
                }
                Btn::Back => (self.tr("BACK", "もどる"), art::QUIET),
                _ => continue,
            };
            let _ = (schools, col);
            super::sprites::button(p, [x, y, w, h], label, 15.0, matches!(*b, Btn::Stay));
        }
    }

    fn draw_player(&self, p: &mut Painter, i: usize, l: &Layout) {
        let ja = self.lang_ja;
        let pl = &self.s.team.players[i];
        // The card: face, name, how he is.
        p.panel(
            10.0,
            10.0,
            380.0,
            152.0,
            22.0,
            [255, 253, 246, 255],
            [244, 236, 220, 255],
            12.0,
        );
        let band = vector::rounded_rect(10.0, 10.0, 380.0, 40.0, 22.0);
        let band = vector::clip_convex(
            &band,
            &[[0.0, 0.0], [400.0, 0.0], [400.0, 50.0], [0.0, 50.0]],
        );
        p.fill(
            &band,
            Paint::vertical(10.0, 50.0, shade(OURS, 0.12), shade(OURS, -0.1)),
        );
        p.portrait(
            16.0,
            26.0,
            134.0,
            &art::player_face(pl),
            if pl.hurt > 0 {
                Expr::Pain
            } else {
                art::mood_expr(pl.mood)
            },
            &Outfit::cap(OURS),
            1.0,
        );
        p.text_shadow(
            &format!("{} {}", pl.family, pl.given),
            160.0,
            18.0,
            21.0,
            WHITE,
            shade(OURS, -0.5),
            TextAlign::Left,
        );
        let sub = if ja {
            format!("{}年  {}", pl.grade, pl.pos.short(true))
        } else {
            format!("Year {}  {}", pl.grade, pl.pos.short(false))
        };
        p.text(&sub, 160.0, 60.0, 15.0, INK, TextAlign::Left);
        // This season's numbers in games.
        let s = pl.season;
        let line = if s.games == 0 {
            self.tr("No games yet this season", "今季はまだ試合なし")
                .to_string()
        } else if pl.pitcher() && s.faced > s.ab {
            if ja {
                format!("今季 {}試合 {}奪三振 {}失点", s.games, s.k, s.allowed)
            } else {
                format!("{} games, {} K, {} runs", s.games, s.k, s.allowed)
            }
        } else if ja {
            format!(
                "今季 打率{} {}安打 {}本 {}打点",
                s.avg(),
                s.hits,
                s.hr,
                s.rbi
            )
        } else {
            format!("{} avg, {} H, {} HR, {} RBI", s.avg(), s.hits, s.hr, s.rbi)
        };
        let line: String = wrap(&line, 11.0, 216.0)
            .into_iter()
            .next()
            .unwrap_or_default();
        p.text(&line, 160.0, 150.0, 11.0, INK_SOFT, TextAlign::Left);
        art::mood_icon(p, [170.0, 96.0], 10.0, pl.mood);
        p.text(pl.mood.label(ja), 186.0, 87.0, 14.0, INK, TextAlign::Left);
        p.text(
            self.tr("Stamina", "体力"),
            160.0,
            116.0,
            12.0,
            INK_SOFT,
            TextAlign::Left,
        );
        p.gauge(
            204.0,
            120.0,
            170.0,
            10.0,
            pl.stamina as f32 / 100.0,
            stamina_color(pl.stamina),
            [226, 222, 212, 255],
        );
        if pl.hurt > 0 || pl.study > 0 {
            let t = if pl.hurt > 0 {
                if ja {
                    format!("ケガ（あと{}週）", pl.hurt)
                } else {
                    format!("Hurt ({} wk)", pl.hurt)
                }
            } else if ja {
                format!("赤点（あと{}週）", pl.study)
            } else {
                format!("Red mark ({} wk)", pl.study)
            };
            p.text(&t, 160.0, 136.0, 13.0, [220, 70, 60, 255], TextAlign::Left);
        } else if pl.abilities.is_empty() {
            p.text(
                self.tr("No ability", "特技なし"),
                160.0,
                136.0,
                13.0,
                INK_SOFT,
                TextAlign::Left,
            );
        } else {
            let tags: Vec<&str> = pl.abilities.iter().map(|a| a.label(ja)).collect();
            p.text(
                &tags.join("  "),
                160.0,
                136.0,
                13.0,
                [176, 96, 24, 255],
                TextAlign::Left,
            );
        }
        // Points by drill.
        p.panel(
            10.0,
            170.0,
            380.0,
            72.0,
            18.0,
            [255, 255, 255, 240],
            [246, 242, 232, 240],
            8.0,
        );
        p.text(
            self.tr("Practice points", "練習ポイント"),
            24.0,
            176.0,
            11.0,
            INK_SOFT,
            TextAlign::Left,
        );
        for (k, d) in Drill::ALL.into_iter().enumerate() {
            let cx = 50.0 + k as f32 * 75.0;
            let col = art::drill_color(d);
            p.dot(cx, 206.0, 17.0, shade(col, -0.1));
            p.dot(cx, 204.0, 15.0, col);
            p.text_display(
                &pl.xp[d.index()].to_string(),
                cx,
                195.0,
                15.0,
                WHITE,
                TextAlign::Center,
            );
            p.text(
                drill_label(d, ja),
                cx,
                224.0,
                11.0,
                INK_SOFT,
                TextAlign::Center,
            );
        }
        // Stats to raise.
        let rows: Vec<&(Btn, [f32; 4])> = l
            .buttons
            .iter()
            .filter(|(b, _)| matches!(b, Btn::Raise(_)))
            .collect();
        if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
            let top = first.1[1] - 6.0;
            let bottom = last.1[1] + last.1[3] + 6.0;
            p.panel(
                10.0,
                top,
                380.0,
                bottom - top,
                18.0,
                [255, 255, 255, 240],
                [246, 242, 232, 240],
                8.0,
            );
        }
        for (b, r) in rows {
            let Btn::Raise(st) = *b else { continue };
            let [x, y, w, h] = *r;
            let v = pl.stat(st);
            let g = grade(v);
            // Text centred on the row, whatever its height.
            let ty = y + (h - 15.0) * 0.5 - 1.0;
            p.text(st.label(ja), 24.0, ty, 15.0, INK, TextAlign::Left);
            if st == Stat::Velo {
                p.text(&format!("{v}km/h"), 196.0, ty, 15.0, INK, TextAlign::Right);
            } else {
                p.chip(132.0, y + h * 0.5, (h - 4.0).min(24.0), g, grade_color(g));
                p.text(&v.to_string(), 176.0, ty, 15.0, INK, TextAlign::Right);
            }
            let frac = if st == Stat::Velo {
                (v as f32 - 100.0) / 60.0
            } else {
                v as f32 / 100.0
            };
            p.gauge(
                204.0,
                y + h * 0.5 - 4.5,
                60.0,
                9.0,
                frac,
                [52, 96, 186, 255],
                [226, 222, 212, 255],
            );
            let pool = st.pool();
            p.text(
                &format!("{} {}", pl.cost(st), drill_label(pool, ja)),
                324.0,
                y + (h - 11.0) * 0.5,
                11.0,
                art::drill_color(pool),
                TextAlign::Right,
            );
            p.button(x, y, w, h, "+", 18.0, [70, 160, 100, 255], pl.can_raise(st));
        }
        // The weekly practice menu, and the way out.
        let my = p.st.h - 128.0;
        p.text(
            self.tr("Own practice every week", "毎週の個人練習"),
            14.0,
            my - 20.0,
            12.0,
            WHITE,
            TextAlign::Left,
        );
        for (b, r) in &l.buttons {
            let [x, y, w, h] = *r;
            match *b {
                Btn::Menu(d) => {
                    let on = pl.menu == d;
                    let c = if on {
                        art::drill_color(d)
                    } else {
                        [120, 126, 150, 255]
                    };
                    p.button(x, y, w, h, drill_label(d, ja), 14.0, c, true);
                    if on {
                        p.outline(
                            &vector::rounded_rect(x - 2.0, y - 2.0, w + 4.0, h + 4.0, 18.0),
                            2.5,
                            WHITE,
                        );
                    }
                }
                Btn::AutoRaise => {
                    p.button(
                        x,
                        y,
                        w,
                        h,
                        self.tr("AUTO SPEND", "おまかせ育成"),
                        16.0,
                        ORANGE,
                        true,
                    );
                }
                Btn::Back => {
                    p.button(
                        x,
                        y,
                        w,
                        h,
                        self.tr("BACK", "もどる"),
                        16.0,
                        [110, 116, 140, 255],
                        true,
                    );
                }
                _ => {}
            }
        }
    }

    /// The face of a club member by family name (the game's sides only carry that).
    fn club_face(&self, family: &str) -> crate::game_core::portrait::Face {
        self.s
            .team
            .players
            .iter()
            .find(|q| q.family == family)
            .map(art::player_face)
            .unwrap_or_else(|| art::face_of(family))
    }

    fn side_color(&self, g: &super::sim::Game, side: usize) -> Rgba {
        if side == g.us {
            OURS
        } else {
            let name = &g.sides[side].name;
            self.s
                .rivals
                .iter()
                .find(|s| s.name == *name || s.name_ja == g.sides[side].name_ja)
                .map(|s| [s.color[0], s.color[1], s.color[2], 255])
                .or_else(|| {
                    // A school from another prefecture on our draw keeps its colour.
                    self.s
                        .cup_path
                        .iter()
                        .map(|&i| self.path_school(i))
                        .find(|(en, _, _)| en == name)
                        .map(|(_, _, c)| c)
                })
                .unwrap_or_else(|| art::team_color(name))
        }
    }
}

pub(in crate::diamond_days) fn square_label(sq: Square, ja: bool) -> &'static str {
    match (sq, ja) {
        (Square::Event, true) => "イベント",
        (Square::Train(d), false) => drill_label(d, false),
        (Square::Rest, false) => "REST",
        (Square::Event, false) => "EVENT",
        (Square::Spirit, false) => "SPIRIT",
        (Square::Scout, false) => "SCOUT",
        (Square::Film, false) => "FILM",
        (Square::Grounds, false) => "GROUNDS",
        _ => sq.label(ja),
    }
}

pub fn grade_color(g: &str) -> Rgba {
    match g {
        "S" => [236, 80, 140, 255],
        "A" => [240, 110, 70, 255],
        "B" => [244, 160, 50, 255],
        "C" => [236, 200, 60, 255],
        "D" => [120, 190, 100, 255],
        "E" => [90, 150, 220, 255],
        "F" => [130, 130, 170, 255],
        _ => [110, 110, 120, 255],
    }
}

fn stamina_color(s: u8) -> Rgba {
    if s > 60 {
        [90, 190, 110, 255]
    } else if s > 30 {
        [240, 180, 60, 255]
    } else {
        [230, 90, 70, 255]
    }
}

pub(super) fn month_en(m: u8) -> &'static str {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m.clamp(1, 12) - 1) as usize]
}

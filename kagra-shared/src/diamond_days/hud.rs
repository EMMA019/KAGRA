//! The screens, drawn as illustrations on a 400-unit-wide stage (phone first; a desktop
//! shows it centred over the backdrop): the week's board with the hand of cards and the
//! manager, the notice card, a game on the field with the scoreboard and the coach's calls,
//! the roster and one player's page.

use super::art::{self, INK, INK_SOFT, ORANGE, OURS, WHITE};
use super::board::{self, Fixture, Square};
use super::sim::{PlayKind, Spot, Tactic};
use super::team::{grade, Drill, Pos, Stat};
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
    /// A call at a chance or a pinch; None lets them play.
    Call(Option<Tactic>),
    /// No more calls this game.
    Auto,
    Skip,
    /// A player's page: raise a stat, spend everything, change the practice menu, back.
    Raise(Stat),
    AutoRaise,
    Menu(Drill),
    Back,
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

const ROW_H: f32 = 58.0;
const ROSTER_TOP: f32 = 124.0;

/// Lines that fit `width` design units at `size` (Japanese breaks anywhere, English at spaces).
fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let ascii = para.is_ascii();
        let pieces: Vec<String> = if ascii {
            para.split(' ').map(|w| format!("{w} ")).collect()
        } else {
            para.chars().map(|c| c.to_string()).collect()
        };
        for piece in pieces {
            let trial = format!("{line}{piece}");
            let keep = !ascii && "、。！？」）".contains(piece.as_str());
            if !line.is_empty()
                && crate::font::measure_text(trial.trim_end(), size) > width
                && !keep
            {
                out.push(line.trim_end().to_string());
                line = piece.trim_start().to_string();
            } else {
                line = trial;
            }
        }
        if !line.trim().is_empty() {
            out.push(line.trim_end().to_string());
        }
    }
    out
}

impl DiamondDays {
    fn stage(&self, w: f32, h: f32) -> Stage {
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
            Mode::Roster => {
                b.push((Btn::RosterUp, [296.0, 78.0, 42.0, 34.0]));
                b.push((Btn::RosterDown, [344.0, 78.0, 42.0, 34.0]));
                let rows = self.roster_rows(hh);
                for k in 0..rows {
                    let i = self.roster_scroll + k;
                    if i >= self.s.team.players.len() {
                        break;
                    }
                    b.push((
                        Btn::Row(i),
                        [10.0, ROSTER_TOP + k as f32 * (ROW_H + 4.0), 380.0, ROW_H],
                    ));
                }
                b.push((Btn::AutoRaise, [10.0, hh - 60.0, 210.0, 48.0]));
                b.push((Btn::Close, [270.0, hh - 60.0, 120.0, 48.0]));
            }
            Mode::Match { asking, .. } => match asking {
                Some(spot) => {
                    let g = self.game.as_ref();
                    let list: Vec<Option<Tactic>> = match spot {
                        Spot::Chance => Tactic::CHANCE.to_vec(),
                        Spot::Pinch => Tactic::PINCH.to_vec(),
                    }
                    .into_iter()
                    .filter(|t| g.is_some_and(|g| t.possible(g)))
                    .map(Some)
                    .chain(std::iter::once(None))
                    .collect();
                    let panel = self.call_panel(hh);
                    let per = 3usize;
                    let bw = (panel[2] - 32.0 - (per as f32 - 1.0) * 8.0) / per as f32;
                    for (i, t) in list.into_iter().enumerate() {
                        let (row, col) = (i / per, i % per);
                        b.push((
                            Btn::Call(t),
                            [
                                panel[0] + 16.0 + col as f32 * (bw + 8.0),
                                panel[1] + 78.0 + row as f32 * 56.0,
                                bw,
                                48.0,
                            ],
                        ));
                    }
                    b.push((
                        Btn::Auto,
                        [panel[0] + panel[2] - 136.0, panel[1] + 16.0, 120.0, 34.0],
                    ));
                }
                None => b.push((Btn::Skip, [270.0, hh - 58.0, 120.0, 46.0])),
            },
            Mode::Player(i) => {
                let p = &self.s.team.players[i];
                let stats: Vec<Stat> = Stat::ALL
                    .iter()
                    .copied()
                    .filter(|st| p.pitcher() || !st.pitching())
                    .collect();
                for (k, st) in stats.into_iter().enumerate() {
                    b.push((Btn::Raise(st), [334.0, 252.0 + k as f32 * 40.0, 48.0, 32.0]));
                }
                let my = hh - 128.0;
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

    fn roster_rows(&self, hh: f32) -> usize {
        (((hh - ROSTER_TOP - 72.0) / (ROW_H + 4.0)) as usize).max(3)
    }

    fn call_panel(&self, hh: f32) -> [f32; 4] {
        let fh = (hh * 0.4).clamp(240.0, 340.0);
        [14.0, 150.0 + fh * 0.18, 372.0, 210.0]
    }

    /// The buttons that apply now, with their screen rectangles.
    pub(super) fn hud_buttons(&self, w: f32, h: f32) -> Vec<(Btn, [f32; 4])> {
        let l = self.layout(w, h);
        l.buttons
            .into_iter()
            .map(|(b, r)| (b, l.st.rect(r[0], r[1], r[2], r[3])))
            .collect()
    }

    pub fn hud(&self, w: u32, h: u32, _menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let mut list = DrawList {
            clear: [40, 60, 100, 255],
            ..Default::default()
        };
        let l = self.layout(w, h);
        let mut p = Painter::new(&mut list, l.st);
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            art::ballpark(&mut p, self.clock);
            return list;
        }
        let month = board::month(self.s.week);
        match &self.mode {
            Mode::Match { shown, t, asking } => {
                self.draw_match(&mut p, *shown, *t, *asking, &l);
            }
            Mode::Roster => {
                art::backdrop(&mut p, month, self.clock);
                self.draw_bar(&mut p);
                self.draw_roster(&mut p, &l);
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
        list
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
            (format!("{}年目", self.s.year), format!("{m}月 第{wk}週"))
        } else {
            (
                format!("Year {}", self.s.year),
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
        let stars = (self.s.reputation / 20.0).ceil().clamp(1.0, 5.0) as usize;
        p.text(
            self.tr("Fame", "評判"),
            242.0,
            16.0,
            11.0,
            INK_SOFT,
            TextAlign::Left,
        );
        for i in 0..5 {
            let c = if i < stars {
                art::GOLD
            } else {
                [206, 204, 214, 255]
            };
            p.star(288.0 + i as f32 * 18.0, 22.0, 8.0, c);
        }
        let record = if ja {
            format!("{}勝{}敗", self.s.wins, self.s.losses)
        } else {
            format!("W{} L{}", self.s.wins, self.s.losses)
        };
        p.text(&record, 376.0, 36.0, 16.0, INK, TextAlign::Right);
    }

    fn next_fixture(&self) -> Option<(u8, Fixture)> {
        (self.s.week..=board::WEEKS)
            .find_map(|wk| board::fixture(wk).map(|f| (wk, f)))
            .filter(|(_, f)| match f {
                Fixture::Summer(_) => self.s.summer_in,
                Fixture::National(_) => self.s.national_in,
                Fixture::Autumn(_) => self.s.autumn_in,
                Fixture::Practice => true,
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
        let r = [302.0, 72.0, 88.0, 44.0];
        p.button(
            r[0],
            r[1],
            r[2],
            r[3],
            self.tr("TEAM", "部員"),
            16.0,
            [60, 120, 210, 255],
            true,
        );
    }

    fn board_rect(&self, hh: f32) -> [f32; 4] {
        let y_m = hh - 112.0 - 100.0;
        [10.0, 124.0, 380.0, (y_m - 132.0).max(250.0)]
    }

    fn draw_board(&self, p: &mut Painter) {
        let r = self.board_rect(p.st.h);
        p.panel_bordered(
            r[0],
            r[1],
            r[2],
            r[3],
            24.0,
            [130, 190, 112, 255],
            [98, 160, 88, 255],
            [255, 255, 255, 150],
            2.5,
            12.0,
        );
        let (tw, th) = (54.0, 42.0);
        let inner = [r[0] + 8.0, r[1] + 8.0, r[2] - 16.0, r[3] - 16.0];
        let n = self.spec.ring.len();
        let centers = art::tile_centers(inner, n, tw, th);
        // The little ballpark in the middle, the club in their places.
        let field = [
            inner[0] + tw + 8.0,
            inner[1] + th + 8.0,
            inner[2] - 2.0 * tw - 16.0,
            inner[3] - 2.0 * th - 16.0,
        ];
        let team = &self.s.team;
        let players: Vec<_> = team
            .ace()
            .map(|ace| {
                let mut v = Vec::new();
                let mut seen: Vec<Pos> = Vec::new();
                for &i in std::iter::once(&ace).chain(team.lineup(ace).iter()) {
                    let pl = &team.players[i];
                    let pos = if i == ace { Pos::P } else { pl.pos };
                    if pos == Pos::P && i != ace || seen.contains(&pos) {
                        continue;
                    }
                    seen.push(pos);
                    let slot = Pos::FIELD.iter().position(|q| *q == pos).unwrap_or(0);
                    v.push((slot, art::player_face(pl), art::mood_expr(pl.mood)));
                }
                v
            })
            .unwrap_or_default();
        art::mini_field(p, field, &players);
        let here = self.s.pos;
        for (i, c) in centers.iter().enumerate() {
            let sq = self.spec.ring[i];
            let label = square_label(sq, self.lang_ja);
            art::tile(
                p,
                *c,
                tw,
                th,
                sq,
                label,
                i == here && matches!(self.mode, Mode::Plan),
            );
        }
        // The coach: on his square, or hopping to the next.
        let (pos, lift) = match &self.mode {
            Mode::Walk { .. } => {
                let a = centers[here % n];
                let b = centers[(here + 1) % n];
                let k = self.hop.clamp(0.0, 1.0);
                let e = k * k * (3.0 - 2.0 * k);
                (
                    [a[0] + (b[0] - a[0]) * e, a[1] + (b[1] - a[1]) * e],
                    (k * std::f32::consts::PI).sin() * 16.0,
                )
            }
            _ => (centers[here % n], 0.0),
        };
        art::coach_token(p, pos, 46.0, lift);
        if let Mode::Walk { left, .. } = &self.mode {
            let txt = if self.lang_ja {
                format!("あと{left}マス")
            } else {
                format!("{left} to go")
            };
            p.panel(
                r[0] + r[2] * 0.5 - 60.0,
                r[1] + r[3] - 64.0,
                120.0,
                30.0,
                15.0,
                [255, 255, 255, 230],
                [246, 240, 228, 230],
                6.0,
            );
            p.text(
                &txt,
                r[0] + r[2] * 0.5,
                r[1] + r[3] - 58.0,
                15.0,
                INK,
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
            p.panel_bordered(
                x,
                y,
                w,
                h,
                12.0,
                [255, 253, 246, 255],
                [242, 234, 216, 255],
                [216, 202, 176, 255],
                2.0,
                10.0,
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

    fn draw_notice(&self, p: &mut Painter) {
        let Some(n) = self.notes.first() else { return };
        let ja = self.lang_ja;
        p.layer();
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::Solid([12, 16, 34, 150]),
        );
        let lines: Vec<String> = n
            .lines
            .iter()
            .flat_map(|(en, jp)| wrap(if ja { jp } else { en }, 16.0, 312.0))
            .collect();
        // Lines from 46 below the top; the manager and the prompt take the last 104.
        let tall = (46.0 + lines.len() as f32 * 25.0 + 112.0)
            .max(220.0)
            .min(p.st.h - 80.0);
        let y = (p.st.h - tall) * 0.5;
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
        let title = if ja { &n.title_ja } else { &n.title };
        p.ribbon(
            200.0,
            y - 20.0,
            250.0,
            48.0,
            title,
            19.0,
            [255, 150, 60, 255],
        );
        let max_lines = ((tall - 46.0 - 104.0) / 25.0).max(1.0) as usize;
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
                        self.roster_scroll > 0,
                    );
                }
                Btn::RosterDown => {
                    let more =
                        self.roster_scroll + self.roster_rows(p.st.h) < self.s.team.players.len();
                    p.button(x, y, w, h, "▼", 14.0, [80, 100, 160, 255], more);
                }
                Btn::Row(i) => self.draw_row(p, i, *r),
                Btn::AutoRaise => {
                    p.button(
                        x,
                        y,
                        w,
                        h,
                        self.tr("SPEND FOR ALL", "全員おまかせ育成"),
                        16.0,
                        ORANGE,
                        true,
                    );
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

    fn draw_row(&self, p: &mut Painter, i: usize, r: [f32; 4]) {
        let ja = self.lang_ja;
        let pl = &self.s.team.players[i];
        let [x, y, w, h] = r;
        let hurt = pl.hurt > 0;
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
                self.tr("hurt", "ケガ"),
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
        if pl.hurt > 0 {
            let t = if ja {
                format!("ケガ（あと{}週）", pl.hurt)
            } else {
                format!("Hurt ({} wk)", pl.hurt)
            };
            p.text(&t, 160.0, 136.0, 13.0, [220, 70, 60, 255], TextAlign::Left);
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
            p.text(st.label(ja), 24.0, y + 7.0, 15.0, INK, TextAlign::Left);
            if st == Stat::Velo {
                p.text(
                    &format!("{v}km/h"),
                    196.0,
                    y + 7.0,
                    15.0,
                    INK,
                    TextAlign::Right,
                );
            } else {
                p.chip(132.0, y + h * 0.5, 24.0, g, grade_color(g));
                p.text(&v.to_string(), 176.0, y + 7.0, 15.0, INK, TextAlign::Right);
            }
            let frac = if st == Stat::Velo {
                (v as f32 - 100.0) / 60.0
            } else {
                v as f32 / 100.0
            };
            p.gauge(
                204.0,
                y + 12.0,
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
                y + 9.0,
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

    // ---- a game

    fn draw_match(&self, p: &mut Painter, shown: usize, t: f32, asking: Option<Spot>, l: &Layout) {
        let Some(g) = &self.game else { return };
        let ja = self.lang_ja;
        let hh = p.st.h;
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::vertical(y0, y0 + h, [34, 44, 80, 255], [16, 20, 38, 255]),
        );
        let colors = [self.side_color(g, 0), self.side_color(g, 1)];
        // The header: the teams and the score.
        p.panel_bordered(
            10.0,
            10.0,
            380.0,
            50.0,
            14.0,
            [44, 56, 100, 255],
            [28, 36, 70, 255],
            [80, 92, 140, 255],
            1.5,
            8.0,
        );
        let mut x = 22.0;
        for (side, &col) in colors.iter().enumerate() {
            let name = if ja {
                &g.sides[side].name_ja
            } else {
                &g.sides[side].name
            };
            p.fill(
                &vector::rounded_rect(x, 24.0, 6.0, 22.0, 3.0),
                Paint::Solid(col),
            );
            p.text(name, x + 12.0, 24.0, 15.0, WHITE, TextAlign::Left);
            x += 22.0 + crate::font::measure_text(name, 15.0);
            if side == 0 {
                p.text("vs", x, 26.0, 12.0, [170, 180, 210, 255], TextAlign::Left);
                x += 22.0;
            }
        }
        p.text_display(
            &format!("{}-{}", g.runs(0), g.runs(1)),
            376.0,
            17.0,
            26.0,
            [255, 226, 130, 255],
            TextAlign::Right,
        );
        // The scoreboard.
        p.panel_bordered(
            10.0,
            66.0,
            380.0,
            64.0,
            12.0,
            [30, 74, 52, 255],
            [20, 54, 38, 255],
            [200, 210, 190, 255],
            2.0,
            8.0,
        );
        let innings = g.line[0].len().max(g.line[1].len()).max(9);
        let name_w = 74.0;
        let cell = (380.0 - name_w - 62.0) / innings as f32;
        for k in 0..innings {
            p.text(
                &(k + 1).to_string(),
                10.0 + name_w + (k as f32 + 0.5) * cell,
                69.0,
                10.0,
                [180, 220, 190, 255],
                TextAlign::Center,
            );
        }
        p.text(
            "R",
            352.0,
            69.0,
            10.0,
            [180, 220, 190, 255],
            TextAlign::Center,
        );
        p.text(
            "H",
            378.0,
            69.0,
            10.0,
            [180, 220, 190, 255],
            TextAlign::Center,
        );
        for (side, &col) in colors.iter().enumerate() {
            let yy = 84.0 + side as f32 * 22.0;
            let name = if ja {
                &g.sides[side].name_ja
            } else {
                &g.sides[side].name
            };
            p.fill(
                &vector::rounded_rect(16.0, yy + 2.0, 4.0, 15.0, 2.0),
                Paint::Solid(col),
            );
            let short: String = name.chars().take(5).collect();
            p.text(&short, 24.0, yy + 2.0, 12.0, WHITE, TextAlign::Left);
            for k in 0..innings {
                if let Some(v) = g.line[side].get(k) {
                    let live = k + 1 == g.line[side].len() && !g.over && (side == 0) == g.top;
                    p.text(
                        &v.to_string(),
                        10.0 + name_w + (k as f32 + 0.5) * cell,
                        yy,
                        14.0,
                        if live { [255, 236, 140, 255] } else { WHITE },
                        TextAlign::Center,
                    );
                }
            }
            p.text(
                &g.runs(side).to_string(),
                352.0,
                yy - 2.0,
                17.0,
                [255, 220, 120, 255],
                TextAlign::Center,
            );
            p.text(
                &g.hits[side].to_string(),
                378.0,
                yy,
                13.0,
                [210, 230, 210, 255],
                TextAlign::Center,
            );
        }
        // The field.
        let fh = (hh * 0.4).clamp(240.0, 340.0);
        let home = art::game_field(p, [10.0, 136.0, 380.0, fh], false);
        let bat_side = if g.top { 0 } else { 1 };
        let fld = colors[1 - bat_side];
        for (dist, ang) in art::FIELDERS {
            let c = art::field_pt(home, dist, ang);
            art::figure(p, c, fld, 9.0, false);
        }
        for (k, on) in g.bases.iter().enumerate() {
            if on.is_some() {
                let c = art::field_pt(
                    home,
                    if k == 1 { 38.8 } else { 27.4 },
                    [45.0, 0.0, -45.0][k],
                );
                art::figure(p, [c[0] + 6.0, c[1] - 4.0], colors[bat_side], 9.0, false);
            }
        }
        if !g.over {
            art::figure(
                p,
                [home.0 - 12.0, home.1 - 8.0],
                colors[bat_side],
                10.0,
                true,
            );
        }
        if let Some(play) = shown.checked_sub(1).and_then(|i| g.log.get(i)) {
            let k = (t / self.spec.line_secs.max(0.05)).min(1.0);
            art::ball_flight(p, home, play.kind, play.dir, k);
            let big = match play.kind {
                PlayKind::Hit(4) => Some((self.tr("HOME RUN!", "ホームラン！"), art::GOLD)),
                PlayKind::Strikeout => Some((self.tr("STRIKEOUT", "三振！"), WHITE)),
                PlayKind::DoublePlay => Some((self.tr("DOUBLE PLAY", "ゲッツー"), WHITE)),
                _ if play.runs > 0 => {
                    Some((self.tr("RUN SCORES!", "得点！"), [255, 210, 110, 255]))
                }
                _ => None,
            };
            if let Some((txt, c)) = big {
                if k < 0.95 {
                    p.text_outlined(
                        txt,
                        200.0,
                        136.0 + fh * 0.3,
                        34.0,
                        c,
                        [20, 20, 30, 255],
                        2.5,
                        TextAlign::Center,
                    );
                }
            }
        }
        // The count: inning, outs, runners.
        p.panel(
            18.0,
            144.0,
            104.0,
            56.0,
            12.0,
            [16, 20, 36, 225],
            [10, 12, 24, 225],
            0.0,
        );
        let half = if ja {
            format!("{}回{}", g.inning, if g.top { "表" } else { "裏" })
        } else {
            format!("{} {}", if g.top { "Top" } else { "Bot" }, g.inning)
        };
        p.text(&half, 28.0, 150.0, 14.0, WHITE, TextAlign::Left);
        p.text(
            "O",
            28.0,
            174.0,
            12.0,
            [200, 205, 220, 255],
            TextAlign::Left,
        );
        for k in 0..2 {
            let on = (k as u8) < g.outs;
            p.dot(
                48.0 + k as f32 * 16.0,
                183.0,
                6.0,
                if on {
                    [240, 80, 70, 255]
                } else {
                    [60, 64, 80, 255]
                },
            );
        }
        let (bx, by) = (98.0, 178.0);
        for (k, (dx, dy)) in [(10.0, 0.0), (0.0, -10.0), (-10.0, 0.0)]
            .into_iter()
            .enumerate()
        {
            let on = g.bases[k].is_some();
            let c = [bx + dx, by + dy];
            p.fill(
                &[
                    [c[0], c[1] - 6.0],
                    [c[0] + 6.0, c[1]],
                    [c[0], c[1] + 6.0],
                    [c[0] - 6.0, c[1]],
                ],
                Paint::Solid(if on {
                    [255, 200, 60, 255]
                } else {
                    [70, 76, 96, 255]
                }),
            );
        }
        // Who is up: the pitcher and the batter.
        let yd = 136.0 + fh + 8.0;
        if !g.over {
            let b = g.batter_now();
            let arm = g.pitcher_now();
            let cards = [
                (
                    self.tr("PITCHER", "投手"),
                    &arm.name,
                    colors[1 - bat_side],
                    format!("{:.0}km/h", arm.velo),
                    format!(
                        "{} {}",
                        self.tr("Ctl", "制球"),
                        grade(arm.control.round() as u8)
                    ),
                ),
                (
                    self.tr("BATTER", "打者"),
                    &b.name,
                    colors[bat_side],
                    format!(
                        "{} {}",
                        self.tr("Contact", "ミート"),
                        grade(b.contact.round() as u8)
                    ),
                    format!(
                        "{} {}",
                        self.tr("Power", "パワー"),
                        grade(b.power.round() as u8)
                    ),
                ),
            ];
            for (k, (lab, name, col, l1, l2)) in cards.into_iter().enumerate() {
                let x = 10.0 + k as f32 * 194.0;
                p.panel(
                    x,
                    yd,
                    186.0,
                    86.0,
                    14.0,
                    [255, 253, 246, 255],
                    [240, 232, 214, 255],
                    8.0,
                );
                p.fill(
                    &vector::rounded_rect(x + 8.0, yd + 10.0, 4.0, 22.0, 2.0),
                    Paint::Solid(col),
                );
                p.portrait(
                    x + 104.0,
                    yd + 4.0,
                    78.0,
                    &art::face_of(name),
                    Expr::Serious,
                    &Outfit::cap(col),
                    1.0,
                );
                p.text(lab, x + 18.0, yd + 8.0, 10.0, INK_SOFT, TextAlign::Left);
                p.text(name, x + 18.0, yd + 20.0, 16.0, INK, TextAlign::Left);
                p.text(&l1, x + 14.0, yd + 46.0, 12.0, INK, TextAlign::Left);
                p.text(&l2, x + 14.0, yd + 64.0, 12.0, INK_SOFT, TextAlign::Left);
            }
        }
        // Play-by-play.
        let ly = yd + 94.0;
        let lh = (hh - ly - 70.0).max(70.0);
        p.panel_bordered(
            10.0,
            ly,
            380.0,
            lh,
            14.0,
            [14, 18, 34, 235],
            [10, 12, 24, 235],
            [70, 80, 110, 255],
            1.5,
            6.0,
        );
        let rows = ((lh - 14.0) / 22.0).max(1.0) as usize;
        let lines: Vec<_> = g.log.iter().take(shown).collect();
        for (k, pl) in lines.iter().rev().take(rows).enumerate() {
            let a = 255u8.saturating_sub(k.min(7) as u8 * 36).max(90);
            let col = if pl.runs > 0 {
                [255, 214, 120, a]
            } else if pl.kind == PlayKind::Strikeout {
                [180, 200, 255, a]
            } else {
                [232, 236, 248, a]
            };
            let txt = if ja { &pl.ja } else { &pl.en };
            let one: String = wrap(txt, 13.5, 352.0)
                .into_iter()
                .next()
                .unwrap_or_default();
            p.text(
                &one,
                22.0,
                ly + 8.0 + (rows - 1 - k) as f32 * 22.0,
                13.5,
                col,
                TextAlign::Left,
            );
        }
        // The calls.
        if let Some(spot) = asking {
            p.layer();
            p.fill(
                &vector::rounded_rect(10.0, 136.0, 380.0, fh, 16.0),
                Paint::Solid([0, 0, 0, 110]),
            );
            let panel = self.call_panel(hh);
            let rows_n = l
                .buttons
                .iter()
                .filter(|(b, _)| matches!(b, Btn::Call(_)))
                .count()
                .div_ceil(3);
            let ph = 90.0 + rows_n as f32 * 56.0;
            p.panel(
                panel[0],
                panel[1],
                panel[2],
                ph,
                20.0,
                [255, 253, 246, 250],
                [240, 232, 214, 250],
                16.0,
            );
            let (en, jp, c) = match spot {
                Spot::Chance => (
                    "A chance! Your call?",
                    "チャンス！ 作戦は？",
                    [230, 150, 30, 255],
                ),
                Spot::Pinch => (
                    "In a pinch. Your call?",
                    "ピンチ！ どうする？",
                    [220, 70, 60, 255],
                ),
            };
            p.fill(
                &vector::rounded_rect(panel[0] + 16.0, panel[1] + 18.0, 6.0, 40.0, 3.0),
                Paint::Solid(c),
            );
            p.text(
                self.tr(en, jp),
                panel[0] + 30.0,
                panel[1] + 16.0,
                19.0,
                INK,
                TextAlign::Left,
            );
            if !g.over {
                let b = g.batter_now();
                let runners = g.bases.iter().filter(|x| x.is_some()).count();
                let sub = if ja {
                    format!("打者 {}  {}アウト  走者{}人", b.name, g.outs, runners)
                } else {
                    format!("{} up, {} out, {} on", b.name, g.outs, runners)
                };
                p.text(
                    &sub,
                    panel[0] + 30.0,
                    panel[1] + 44.0,
                    12.0,
                    INK_SOFT,
                    TextAlign::Left,
                );
            }
            for (b, r) in &l.buttons {
                let [x, y, w, h] = *r;
                match *b {
                    Btn::Call(Some(t)) => {
                        let col = match t {
                            Tactic::Hit | Tactic::Challenge => [200, 70, 60, 255],
                            Tactic::Bunt | Tactic::Squeeze => [60, 150, 100, 255],
                            Tactic::Steal => [220, 150, 40, 255],
                            _ => [60, 110, 200, 255],
                        };
                        p.button(x, y, w, h, t.label(ja), 16.0, col, true);
                    }
                    Btn::Call(None) => {
                        p.button(
                            x,
                            y,
                            w,
                            h,
                            self.tr("PLAY ON", "おまかせ"),
                            16.0,
                            [110, 116, 140, 255],
                            true,
                        );
                    }
                    Btn::Auto => {
                        p.button(
                            x,
                            y,
                            w,
                            h,
                            self.tr("NO MORE CALLS", "以降おまかせ"),
                            12.0,
                            [90, 96, 120, 255],
                            true,
                        );
                    }
                    _ => {}
                }
            }
        } else {
            for (b, r) in &l.buttons {
                if *b == Btn::Skip {
                    p.button(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        self.tr("TO THE END", "試合終了まで"),
                        14.0,
                        [110, 116, 140, 255],
                        true,
                    );
                }
            }
            p.text(
                self.tr("Tap to hurry", "タップで早送り"),
                20.0,
                hh - 46.0,
                12.0,
                [190, 200, 230, 255],
                TextAlign::Left,
            );
        }
    }

    fn side_color(&self, g: &super::sim::Game, side: usize) -> Rgba {
        if side == g.us {
            OURS
        } else {
            art::team_color(&g.sides[side].name)
        }
    }
}

fn square_label(sq: Square, ja: bool) -> &'static str {
    match (sq, ja) {
        (Square::Event, true) => "イベント",
        (Square::Train(d), false) => drill_label(d, false),
        (Square::Rest, false) => "REST",
        (Square::Event, false) => "EVENT",
        (Square::Spirit, false) => "SPIRIT",
        (Square::Scout, false) => "SCOUT",
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

fn month_en(m: u8) -> &'static str {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m.clamp(1, 12) - 1) as usize]
}

//! The HUD: the calendar bar, the hand of cards, the roster, the notice card, and the game
//! screen (scoreboard, bases, play-by-play, orders).

use super::board::{self, Fixture};
use super::sim::Order;
use super::team::{grade, Drill};
use super::{hud_scale, DiamondDays, Mode};
use crate::flow::FlowPhase;
use crate::hud_kit::{pill_button, round_panel, Card as Panel, CardStyle, Menu, Row};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};

const INK: [u8; 4] = [30, 34, 50, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

/// A button on the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Btn {
    Card(usize),
    Roster,
    RosterUp,
    RosterDown,
    /// An order for the inning; None leaves the rest to the team.
    Order(Option<Order>),
    Skip,
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

impl DiamondDays {
    /// The buttons that apply now, with their rectangles.
    pub(super) fn hud_buttons(&self, w: f32, h: f32) -> Vec<(Btn, [f32; 4])> {
        let s = hud_scale(w, h);
        let mut out = Vec::new();
        match self.mode {
            Mode::Plan => {
                let n = self.s.hand.len();
                let cw = ((w - 24.0 * s - (n as f32 - 1.0) * 8.0 * s) / n as f32).min(86.0 * s);
                let ch = cw * 1.35;
                let x0 = (w - n as f32 * cw - (n as f32 - 1.0) * 8.0 * s) * 0.5;
                for i in 0..n {
                    out.push((
                        Btn::Card(i),
                        [x0 + i as f32 * (cw + 8.0 * s), h - ch - 16.0 * s, cw, ch],
                    ));
                }
                out.push((Btn::Roster, [w - 112.0 * s, 64.0 * s, 100.0 * s, 36.0 * s]));
            }
            Mode::Roster => {
                out.push((Btn::RosterUp, [w - 70.0 * s, h * 0.2, 56.0 * s, 44.0 * s]));
                out.push((
                    Btn::RosterDown,
                    [w - 70.0 * s, h * 0.2 + 52.0 * s, 56.0 * s, 44.0 * s],
                ));
            }
            Mode::Match { asking, .. } => {
                let bh = 48.0 * s;
                if asking {
                    let bw = (w - 40.0 * s) / 4.0;
                    let y = h - bh - 18.0 * s;
                    let orders = [
                        Some(Order::Swing),
                        Some(Order::Normal),
                        Some(Order::Small),
                        None,
                    ];
                    for (i, o) in orders.into_iter().enumerate() {
                        out.push((
                            Btn::Order(o),
                            [12.0 * s + i as f32 * (bw + 5.0 * s), y, bw, bh],
                        ));
                    }
                } else {
                    out.push((Btn::Skip, [w - 120.0 * s, h - bh - 18.0 * s, 106.0 * s, bh]));
                }
            }
            _ => {}
        }
        out
    }

    pub fn hud(&self, w: u32, h: u32, _menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let s = hud_scale(w, h);
        let mut list = DrawList::default();
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            return list;
        }
        let ja = self.lang_ja;
        // The calendar bar.
        round_panel(
            &mut list,
            8.0 * s,
            8.0 * s,
            w - 16.0 * s,
            50.0 * s,
            14.0 * s,
            [255, 255, 255, 225],
            [40, 60, 110, 90],
            1.0 * s,
            (0.0, 3.0 * s, 50),
        );
        let m = board::month(self.s.week);
        let wk = (self.s.week - 1) % 4 + 1;
        let date = if ja {
            format!("{}年目 {m}月 第{wk}週", self.s.year)
        } else {
            format!("Year {}  {}  week {wk}", self.s.year, month_en(m))
        };
        list.texts
            .push(TextQuad::new(&date, 20.0 * s, 15.0 * s, 17.0 * s, INK));
        let record = if ja {
            format!("{}勝{}敗", self.s.wins, self.s.losses)
        } else {
            format!("W {}  L {}", self.s.wins, self.s.losses)
        };
        list.texts.push(TextQuad::new(
            &record,
            20.0 * s,
            36.0 * s,
            14.0 * s,
            [80, 90, 120, 255],
        ));
        let stars = (self.s.reputation / 20.0).ceil().clamp(1.0, 5.0) as usize;
        let rep = format!(
            "{} {}",
            self.tr("Fame", "評判"),
            "★".repeat(stars) + &"☆".repeat(5 - stars)
        );
        list.texts.push(
            TextQuad::new(&rep, w - 20.0 * s, 15.0 * s, 15.0 * s, [220, 150, 40, 255])
                .aligned(TextAlign::Right),
        );
        let strength = format!(
            "{} {}",
            self.tr("Team", "戦力"),
            grade(self.s.team.strength().round() as u8)
        );
        list.texts.push(
            TextQuad::new(
                &strength,
                w - 20.0 * s,
                36.0 * s,
                14.0 * s,
                [80, 90, 120, 255],
            )
            .aligned(TextAlign::Right),
        );
        match &self.mode {
            Mode::Match { shown, asking, .. } => {
                self.draw_match(&mut list, w, h, s, *shown, *asking)
            }
            Mode::Roster => self.draw_roster(&mut list, w, h, s),
            _ => {
                self.draw_next(&mut list, s);
                if matches!(self.mode, Mode::Plan) {
                    self.draw_hand(&mut list, w, h, s);
                }
            }
        }
        for (b, r) in self.hud_buttons(w, h) {
            let (label, fill) = match b {
                Btn::Card(_) => continue,
                Btn::Roster => (self.tr("TEAM", "部員"), [40, 90, 160, 235]),
                Btn::RosterUp => ("▲", [40, 60, 110, 230]),
                Btn::RosterDown => ("▼", [40, 60, 110, 230]),
                Btn::Order(Some(Order::Swing)) => (self.tr("SWING", "強攻"), [190, 70, 60, 240]),
                Btn::Order(Some(Order::Normal)) => (self.tr("NORMAL", "普通"), [60, 110, 190, 240]),
                Btn::Order(Some(Order::Small)) => (self.tr("SMALL", "小技"), [60, 150, 100, 240]),
                Btn::Order(None) => (self.tr("AUTO", "おまかせ"), [90, 90, 110, 240]),
                Btn::Skip => (self.tr("SKIP", "結果まで"), [80, 80, 100, 230]),
            };
            pill_button(
                &mut list,
                r[0],
                r[1],
                r[2],
                r[3],
                label,
                15.0 * s,
                fill,
                WHITE,
            );
        }
        if let (Mode::Notice, Some(n)) = (&self.mode, self.notes.first()) {
            let lines = n.lines.len() as f32;
            let tall = (110.0 + lines * 58.0) * s;
            let card = Panel::open(&mut list, w, h, s, tall, CardStyle::default());
            card.line(
                &mut list,
                if ja { &n.title_ja } else { &n.title },
                18.0,
                24.0,
                card.style.title,
            );
            for (i, (en, jp)) in n.lines.iter().enumerate() {
                card.body(
                    &mut list,
                    if ja { jp } else { en },
                    60.0 + i as f32 * 58.0,
                    15.0,
                    ja,
                );
            }
            card.prompt(
                &mut list,
                self.tr("Tap to go on", "タップで次へ"),
                self.clock,
                28.0,
            );
        }
        list
    }

    /// The next fixture under the bar.
    fn draw_next(&self, list: &mut DrawList, s: f32) {
        let next = (self.s.week..=board::WEEKS).find_map(|wk| board::fixture(wk).map(|f| (wk, f)));
        let Some((wk, f)) = next else {
            return;
        };
        let (en, ja) = Self::fixture_name(f);
        let left = wk - self.s.week;
        let skip = match f {
            Fixture::Summer(_) => !self.s.summer_in,
            Fixture::National(_) => !self.s.national_in,
            Fixture::Autumn(_) => !self.s.autumn_in,
            Fixture::Practice => false,
        };
        if skip {
            return;
        }
        let text = if self.lang_ja {
            if left == 0 {
                format!("今週：{ja}")
            } else {
                format!("次の試合：{ja}（あと{left}週）")
            }
        } else if left == 0 {
            format!("This week: {en}")
        } else {
            format!("Next: {en} (in {left} weeks)")
        };
        list.texts.push(TextQuad::new(
            &text,
            14.0 * s,
            70.0 * s,
            15.0 * s,
            [255, 255, 255, 240],
        ));
    }

    fn draw_hand(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        list.texts.push(
            TextQuad::new(
                self.tr("Pick a card for this week", "今週のカードを選ぼう"),
                w * 0.5,
                h - 160.0 * s,
                16.0 * s,
                [255, 255, 255, 240],
            )
            .aligned(TextAlign::Center),
        );
        for (b, r) in self.hud_buttons(w, h) {
            let Btn::Card(i) = b else {
                continue;
            };
            let c = self.s.hand[i];
            round_panel(
                list,
                r[0],
                r[1],
                r[2],
                r[3],
                10.0 * s,
                [255, 252, 240, 250],
                [200, 160, 60, 220],
                2.0 * s,
                (0.0, 4.0 * s, 70),
            );
            list.texts.push(
                TextQuad::new(
                    &format!("{}", c.steps),
                    r[0] + r[2] * 0.5,
                    r[1] + r[3] * 0.18,
                    r[2] * 0.55,
                    INK,
                )
                .aligned(TextAlign::Center)
                .with_face(TextFace::Display),
            );
            if let Some(d) = c.focus {
                let col = super::board::Square::Train(d).color();
                let mut q = Quad::new(
                    r[0] + 6.0 * s,
                    r[1] + r[3] - 30.0 * s,
                    r[2] - 12.0 * s,
                    22.0 * s,
                    [col[0] as u8, col[1] as u8, col[2] as u8, 255],
                );
                q.radius = 8.0 * s;
                list.quads.push(q);
                list.texts.push(
                    TextQuad::new(
                        &format!("{}×2", drill_label(d, self.lang_ja)),
                        r[0] + r[2] * 0.5,
                        r[1] + r[3] - 27.0 * s,
                        13.0 * s,
                        WHITE,
                    )
                    .aligned(TextAlign::Center),
                );
            }
        }
    }

    fn draw_roster(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let menu = Menu::layout(w - 70.0 * s, 70.0 * s, h - 20.0 * s, 460.0, s);
        let ja = self.lang_ja;
        let rows: Vec<Row> = self
            .s
            .team
            .players
            .iter()
            .map(|p| {
                let stats = if p.pitcher() {
                    format!(
                        "{}km {}{} {}{} {}{}",
                        p.velo,
                        self.tr("Ctl", "制"),
                        grade(p.control),
                        self.tr("Sta", "ス"),
                        grade(p.stamina_p),
                        self.tr("Brk", "変"),
                        grade(p.breaking)
                    )
                } else {
                    format!(
                        "{}{} {}{} {}{} {}{}",
                        self.tr("C", "ミ"),
                        grade(p.contact),
                        self.tr("P", "パ"),
                        grade(p.power),
                        self.tr("S", "走"),
                        grade(p.speed),
                        self.tr("F", "守"),
                        grade(p.field)
                    )
                };
                let status = if p.hurt > 0 {
                    self.tr("hurt", "ケガ").to_string()
                } else {
                    p.mood.label(ja).to_string()
                };
                Row {
                    icon: p.pos.short(ja).into(),
                    icon_color: [255, 220, 140, 255],
                    label: format!("{}{} {}  {stats}", p.grade, self.tr("", "年"), p.family),
                    right: status,
                    dim: p.hurt > 0,
                }
            })
            .collect();
        menu.draw(
            list,
            self.tr("The club", "部員"),
            &format!("{}", self.s.team.players.len()),
            &rows,
            self.roster_scroll,
            None,
        );
        list.texts.push(
            TextQuad::new(
                self.tr("Tap to close", "タップで閉じる"),
                w * 0.5,
                h - 16.0 * s,
                13.0 * s,
                [255, 255, 255, 200],
            )
            .aligned(TextAlign::Center),
        );
    }

    fn draw_match(&self, list: &mut DrawList, w: f32, h: f32, s: f32, shown: usize, asking: bool) {
        let Some(g) = &self.game else {
            return;
        };
        let ja = self.lang_ja;
        // The scoreboard.
        let (bx, by, bw, bh) = (8.0 * s, 64.0 * s, w - 16.0 * s, 78.0 * s);
        round_panel(
            list,
            bx,
            by,
            bw,
            bh,
            10.0 * s,
            [20, 60, 40, 240],
            [255, 255, 255, 120],
            1.5 * s,
            (0.0, 3.0 * s, 60),
        );
        let innings = g.line[0].len().max(9);
        let name_w = 82.0 * s;
        let cell = ((bw - name_w - 70.0 * s) / innings as f32).min(26.0 * s);
        for side in 0..2 {
            let y = by + 26.0 * s + side as f32 * 24.0 * s;
            let name = if ja {
                &g.sides[side].name_ja
            } else {
                &g.sides[side].name
            };
            let col = if side == g.us {
                [255, 230, 120, 255]
            } else {
                WHITE
            };
            list.texts
                .push(TextQuad::new(name, bx + 8.0 * s, y, 13.0 * s, col));
            for k in 0..innings {
                let v = g.line[side]
                    .get(k)
                    .map(|r| r.to_string())
                    .unwrap_or_default();
                list.texts.push(
                    TextQuad::new(
                        &v,
                        bx + name_w + (k as f32 + 0.5) * cell,
                        y,
                        13.0 * s,
                        WHITE,
                    )
                    .aligned(TextAlign::Center),
                );
            }
            list.texts.push(
                TextQuad::new(
                    &format!("{}", g.runs(side)),
                    bx + bw - 40.0 * s,
                    y - 2.0 * s,
                    17.0 * s,
                    [255, 240, 160, 255],
                )
                .aligned(TextAlign::Center),
            );
            list.texts.push(
                TextQuad::new(
                    &format!("{}", g.hits[side]),
                    bx + bw - 14.0 * s,
                    y,
                    12.0 * s,
                    [200, 230, 200, 255],
                )
                .aligned(TextAlign::Center),
            );
        }
        for k in 0..innings {
            list.texts.push(
                TextQuad::new(
                    &format!("{}", k + 1),
                    bx + name_w + (k as f32 + 0.5) * cell,
                    by + 7.0 * s,
                    11.0 * s,
                    [180, 220, 190, 255],
                )
                .aligned(TextAlign::Center),
            );
        }
        list.texts.push(
            TextQuad::new(
                "R",
                bx + bw - 40.0 * s,
                by + 7.0 * s,
                11.0 * s,
                [180, 220, 190, 255],
            )
            .aligned(TextAlign::Center),
        );
        list.texts.push(
            TextQuad::new(
                "H",
                bx + bw - 14.0 * s,
                by + 7.0 * s,
                11.0 * s,
                [180, 220, 190, 255],
            )
            .aligned(TextAlign::Center),
        );
        // Count: outs, the bases.
        let py = by + bh + 10.0 * s;
        round_panel(
            list,
            8.0 * s,
            py,
            150.0 * s,
            70.0 * s,
            10.0 * s,
            [255, 255, 255, 225],
            [40, 60, 110, 90],
            1.0 * s,
            (0.0, 2.0 * s, 40),
        );
        let half = if ja {
            format!("{}回{}", g.inning, if g.top { "表" } else { "裏" })
        } else {
            format!("{} {}", if g.top { "Top" } else { "Bot" }, g.inning)
        };
        list.texts
            .push(TextQuad::new(&half, 18.0 * s, py + 8.0 * s, 15.0 * s, INK));
        for k in 0..2 {
            let on = (k as u8) < g.outs;
            let mut q = Quad::new(
                18.0 * s + k as f32 * 18.0 * s,
                py + 40.0 * s,
                13.0 * s,
                13.0 * s,
                if on {
                    [230, 70, 60, 255]
                } else {
                    [200, 200, 210, 255]
                },
            );
            q.radius = 6.5 * s;
            list.quads.push(q);
        }
        list.texts.push(TextQuad::new(
            self.tr("OUT", "アウト"),
            58.0 * s,
            py + 40.0 * s,
            11.0 * s,
            [90, 90, 110, 255],
        ));
        let (cx, cy) = (120.0 * s, py + 38.0 * s);
        for (k, (dx, dy)) in [(14.0, 0.0), (0.0, -14.0), (-14.0, 0.0)].iter().enumerate() {
            let on = g.bases[k].is_some();
            let mut q = Quad::new(
                cx + dx * s - 6.0 * s,
                cy + dy * s - 6.0 * s,
                12.0 * s,
                12.0 * s,
                if on {
                    [240, 180, 40, 255]
                } else {
                    [200, 200, 210, 255]
                },
            );
            q.radius = 2.0 * s;
            list.quads.push(q);
        }
        // Who is up.
        if !g.over {
            let b = g.batter_now();
            let p = g.pitcher_now();
            let who = if ja {
                format!("打者 {}　投手 {}", b.name, p.name)
            } else {
                format!("At bat {}  Pitching {}", b.name, p.name)
            };
            list.texts.push(TextQuad::new(
                &who,
                168.0 * s,
                py + 10.0 * s,
                13.0 * s,
                WHITE,
            ));
        }
        // Play-by-play, the last lines shown.
        let lines: Vec<&super::sim::Play> = g.log.iter().take(shown).collect();
        let (ly, lh) = (h * 0.56, 26.0 * s);
        round_panel(
            list,
            8.0 * s,
            ly - 8.0 * s,
            w - 16.0 * s,
            lh * 5.0 + 16.0 * s,
            10.0 * s,
            [10, 14, 30, 210],
            [255, 255, 255, 60],
            1.0 * s,
            (0.0, 0.0, 0),
        );
        for (k, p) in lines.iter().rev().take(5).enumerate() {
            let a = 255 - (k as u8) * 40;
            let col = if p.runs > 0 {
                [255, 220, 120, a]
            } else {
                [235, 240, 255, a]
            };
            list.texts.push(TextQuad::new(
                if ja { &p.ja } else { &p.en },
                18.0 * s,
                ly + (4 - k) as f32 * lh,
                15.0 * s,
                col,
            ));
        }
        if asking {
            list.texts.push(
                TextQuad::new(
                    self.tr("Your order for this inning?", "この回の作戦は？"),
                    w * 0.5,
                    h - 96.0 * s,
                    17.0 * s,
                    WHITE,
                )
                .aligned(TextAlign::Center),
            );
        }
    }
}

fn month_en(m: u8) -> &'static str {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m.clamp(1, 12) - 1) as usize]
}

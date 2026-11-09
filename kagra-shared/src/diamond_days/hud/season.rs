//! The season's own screens: a tournament's draw (our path from the first round up to the
//! final, the scores so far, the next school, and how to spend the day before), and the
//! coach's decisions for the club (who wears number 1, who captains the new team).

use super::{wrap, Btn, Layout};
use crate::diamond_days::art::{self, INK, INK_SOFT, WHITE};
use crate::diamond_days::board::{self, Fixture};
use crate::diamond_days::team::grade;
use crate::diamond_days::{Decide, DiamondDays, AWAY};
use crate::game_core::portrait::Expr;
use crate::hud_art::Painter;
use crate::scene::TextAlign;
use crate::vector::{Paint, Rgba};

const GOLD: Rgba = [240, 196, 80, 255];

impl DiamondDays {
    /// A school on our path: its name (both languages) and colour.
    pub(in crate::diamond_days) fn path_school(&self, i: u8) -> (String, String, Rgba) {
        match self.s.rivals.get(i as usize) {
            Some(s) if i < AWAY => (
                s.name.clone(),
                s.name_ja.clone(),
                [s.color[0], s.color[1], s.color[2], 255],
            ),
            _ => {
                let (en, ja, c) =
                    board::away_school((i.saturating_sub(AWAY)) as usize, self.s.year);
                (en, ja, [c[0], c[1], c[2], 255])
            }
        }
    }

    /// What is at stake in this tournament, in a line.
    fn stakes(&self, f: Fixture) -> &'static str {
        match f {
            Fixture::Summer(_) => self.tr(
                "Lose once and the third-years' summer is over.",
                "負けたら、3年生の夏が終わる。",
            ),
            Fixture::National(_) => self.tr(
                "Every school here won its prefecture.",
                "ここにいるのは、県を勝ち抜いた学校だけ。",
            ),
            Fixture::SpringPref(_) => self.tr(
                "The last four are seeded in the summer.",
                "ベスト4に入れば、夏はシード。",
            ),
            Fixture::Autumn(_) => self.tr(
                "Go deep and March may call.",
                "勝ち進めば、春の招待舞台が見えてくる。",
            ),
            Fixture::Spring(_) | Fixture::Practice => {
                self.tr("The first big stage of the year.", "一年で最初の大舞台。")
            }
        }
    }

    /// The draw: our path up to the final (the final at the top), the scores so far, the
    /// next school lit, and three ways to spend the day before.
    pub(super) fn draw_bracket(&self, p: &mut Painter, l: &Layout) {
        let Some(f) = self.s.cup else { return };
        let ja = self.lang_ja;
        let hh = p.st.h;
        art::ballpark(p, self.clock);
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::vertical(y0, y0 + h, [10, 14, 34, 150], [10, 14, 34, 225]),
        );
        let (cup, cup_ja) = f.cup_name();
        let color = match f {
            Fixture::SpringPref(_) => [90, 160, 90, 255],
            Fixture::Autumn(_) => [210, 120, 40, 255],
            Fixture::Spring(_) => [220, 110, 150, 255],
            _ => [220, 80, 60, 255],
        };
        p.ribbon(
            200.0,
            22.0,
            330.0,
            48.0,
            if ja { cup_ja } else { cup },
            19.0,
            color,
        );
        p.text_outlined(
            self.stakes(f),
            200.0,
            78.0,
            14.0,
            WHITE,
            [16, 18, 30, 255],
            2.0,
            TextAlign::Center,
        );

        // The ladder: from the first round we play (bottom) to the final (top).
        let played = self.s.cup_scores.len() as u8;
        let first = f.round().saturating_sub(played).max(1);
        let rounds = f.rounds();
        let n = (rounds - first + 1) as usize;
        let top = 104.0;
        let first_btn = l.buttons.iter().map(|(_, r)| r[1]).fold(hh, f32::min);
        let bottom = first_btn - 64.0;
        let row = ((bottom - top) / n as f32).clamp(36.0, 62.0);
        let rh = row - 8.0;
        let (us_en, us_ja) = (self.school_en().to_string(), self.school_ja().to_string());
        // The spine the path climbs.
        let ys = |r: u8| top + (rounds - r) as f32 * row;
        p.line(
            &[[30.0, ys(first) + rh * 0.5], [30.0, ys(rounds) + rh * 0.5]],
            4.0,
            [255, 255, 255, 60],
        );
        let won_to = first + played; // the round we are at now
        p.line(
            &[[30.0, ys(first) + rh * 0.5], [30.0, ys(won_to) + rh * 0.5]],
            5.0,
            GOLD,
        );
        for r in first..=rounds {
            let y = ys(r);
            let at = f.at(r);
            let now = r == f.round();
            let done = r < f.round();
            let (rn, rn_ja) = at.round_name();
            let school = self
                .s
                .cup_path
                .get(r as usize - 1)
                .map(|&i| self.path_school(i));
            if now {
                p.glow(200.0, y + rh * 0.5, 190.0, [255, 214, 110, 70]);
            }
            p.panel_bordered(
                48.0,
                y,
                342.0,
                rh,
                12.0,
                if now {
                    [255, 251, 238, 250]
                } else {
                    [36, 44, 78, 235]
                },
                if now {
                    [246, 236, 214, 250]
                } else {
                    [24, 30, 58, 235]
                },
                if now { GOLD } else { [80, 92, 140, 255] },
                if now { 3.0 } else { 1.5 },
                6.0,
            );
            let ink = if now { INK } else { WHITE };
            let soft = if now { INK_SOFT } else { [180, 190, 220, 255] };
            // The node on the spine.
            p.dot(
                30.0,
                y + rh * 0.5,
                if now { 9.0 } else { 6.0 },
                if done || now {
                    GOLD
                } else {
                    [120, 130, 170, 255]
                },
            );
            let mid = y + rh * 0.5 - 8.0;
            p.text(
                if ja { &rn_ja } else { &rn },
                60.0,
                mid,
                15.0,
                if at.is_final() {
                    [214, 60, 60, 255]
                } else {
                    soft
                },
                TextAlign::Left,
            );
            if let Some((en, sja, c)) = &school {
                p.dot(150.0, y + rh * 0.5, 7.0, *c);
                let name = if ja { sja } else { en };
                let shown = if done || now {
                    name.clone()
                } else {
                    format!("{name}?")
                };
                p.text(&shown, 164.0, mid, 15.0, ink, TextAlign::Left);
            }
            let tag = match self.s.cup_scores.get((r - first) as usize) {
                Some(&(a, b)) if done => {
                    let mark = if a > b { "○" } else { "●" };
                    format!("{mark} {a}-{b}")
                }
                _ if now => self.tr("NEXT", "次の相手").to_string(),
                _ => String::new(),
            };
            p.text(
                &tag,
                378.0,
                mid,
                15.0,
                if now { [214, 60, 60, 255] } else { GOLD },
                TextAlign::Right,
            );
        }
        // Who we are, at the foot of the ladder.
        p.text_outlined(
            if ja { &us_ja } else { &us_en },
            30.0,
            ys(first) + rh + 6.0,
            12.0,
            [255, 236, 170, 255],
            [16, 18, 30, 255],
            1.6,
            TextAlign::Left,
        );

        // The ace's arm, for the day before.
        let ace = self
            .ace_candidates()
            .first()
            .and_then(|&i| self.s.team.players.get(i));
        if let Some(a) = ace {
            let y = first_btn - 50.0;
            let label = if ja {
                format!("エース {}　体力", a.family)
            } else {
                format!("Ace {}  stamina", a.family)
            };
            p.text(&label, 24.0, y, 13.0, WHITE, TextAlign::Left);
            let gw = 160.0;
            let k = a.stamina as f32 / 100.0;
            let col = if k > 0.6 {
                [90, 200, 120, 255]
            } else if k > 0.35 {
                [240, 190, 70, 255]
            } else {
                [230, 80, 70, 255]
            };
            p.gauge(216.0, y + 2.0, gw, 12.0, k, col, [40, 46, 70, 255]);
        }
        p.text(
            self.tr("The day before the game:", "試合前日の過ごし方は？"),
            200.0,
            first_btn - 28.0,
            14.0,
            WHITE,
            TextAlign::Center,
        );
        let labels = [
            self.tr("Rest: shake off the fatigue", "休養：疲れを取る"),
            self.tr("Tune up: lift the mood", "調整：調子を上げる"),
            self.tr("Study them: scout the next school", "対策：相手を研究する"),
        ];
        for (b, r) in &l.buttons {
            let Btn::Choose(k) = *b else { continue };
            let col = match k {
                0 => [70, 160, 110, 255],
                1 => [60, 120, 210, 255],
                _ => [200, 110, 50, 255],
            };
            p.button(r[0], r[1], r[2], r[3], labels[k.min(2)], 17.0, col, true);
        }
    }

    /// The coach decides for the club: Kotori asks, the answers are players.
    pub(super) fn draw_decide(&self, p: &mut Painter, d: Decide, l: &Layout) {
        let ja = self.lang_ja;
        p.layer();
        let [x0, y0, w, hgt] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + hgt], [x0, y0 + hgt]],
            Paint::Solid([12, 16, 34, 190]),
        );
        let hh = p.st.h;
        let first_btn = l.buttons.iter().map(|(_, r)| r[1]).fold(hh, f32::min);
        let (title, ask) = match d {
            Decide::Ace => (
                self.tr("Summer numbers", "背番号発表"),
                self.tr(
                    "Coach, number 1: the ace's number. Who do we give it to?",
                    "監督、背番号1……エースナンバーは誰に渡しますか？",
                ),
            ),
            Decide::Captain => (
                self.tr("The new team", "新チーム発足"),
                self.tr(
                    "The new team needs a captain. Who will lead them?",
                    "新チームの主将を決めましょう。誰に任せますか？",
                ),
            ),
        };
        p.ribbon(200.0, 24.0, 230.0, 46.0, title, 18.0, [90, 130, 210, 255]);
        let size = (first_btn - 250.0).clamp(150.0, 250.0);
        let box_y = first_btn - 140.0;
        let (face, outfit) = art::manager();
        p.glow(120.0, box_y - size * 0.45, size * 0.8, [255, 236, 190, 90]);
        p.portrait(
            120.0 - size * 0.5,
            box_y - size + 18.0,
            size,
            &face,
            Expr::Serious,
            &outfit,
            1.0,
        );
        p.panel_bordered(
            16.0,
            box_y,
            368.0,
            120.0,
            22.0,
            [255, 253, 246, 252],
            [246, 238, 222, 252],
            [230, 210, 170, 255],
            2.5,
            14.0,
        );
        p.text(
            self.tr("Kotori", "七瀬ことり"),
            34.0,
            box_y + 12.0,
            12.0,
            [214, 100, 120, 255],
            TextAlign::Left,
        );
        for (k, line) in wrap(ask, 16.0, 330.0).iter().take(3).enumerate() {
            p.text(
                line,
                34.0,
                box_y + 34.0 + k as f32 * 25.0,
                16.0,
                INK,
                TextAlign::Left,
            );
        }
        let labels: Vec<String> = match d {
            Decide::Ace => self
                .ace_candidates()
                .iter()
                .map(|&i| {
                    let q = &self.s.team.players[i];
                    if ja {
                        format!(
                            "{}（{}年）{}km 制球{}",
                            q.family,
                            q.grade,
                            q.velo,
                            grade(q.control)
                        )
                    } else {
                        format!(
                            "{} (yr {}) {} km/h, control {}",
                            q.family,
                            q.grade,
                            q.velo,
                            grade(q.control)
                        )
                    }
                })
                .collect(),
            Decide::Captain => self
                .captain_candidates()
                .iter()
                .map(|&(i, a)| {
                    let q = &self.s.team.players[i];
                    let style = match a {
                        crate::diamond_days::team::Ability::Mood => {
                            self.tr("leads with his voice", "声で引っ張る")
                        }
                        crate::diamond_days::team::Ability::Clutch => {
                            self.tr("holds the field together", "守りでまとめる")
                        }
                        _ => self.tr("leads by example", "実力で引っ張る"),
                    };
                    if ja {
                        format!(
                            "{}（{}年・{}）{style}",
                            q.family,
                            q.grade,
                            q.pos.short(true)
                        )
                    } else {
                        format!("{} ({}) {style}", q.family, q.pos.short(false))
                    }
                })
                .collect(),
        };
        for (b, r) in &l.buttons {
            let Btn::Choose(k) = *b else { continue };
            let Some(text) = labels.get(k) else { continue };
            let col = if k == 0 {
                [60, 120, 210, 255]
            } else {
                [70, 160, 110, 255]
            };
            p.button(r[0], r[1], r[2], r[3], text, 16.0, col, true);
        }
    }
}

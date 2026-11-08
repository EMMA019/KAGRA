//! The game part, apart from the practice board: the pre-game screen (the tournament, both
//! schools, our order), the game seen from behind the plate pitch by pitch with the count,
//! the field when the ball is hit, and the result screen (the score, the player of the game,
//! what the club learned).

use super::{grade_color, wrap, Btn, Layout};
use crate::diamond_days::art::{self, INK, INK_SOFT, ORANGE, OURS, WHITE};
use crate::diamond_days::board::{self, Fixture};
use crate::diamond_days::sim::{Game, Pitch, Play, PlayKind, Side, Spot, Tactic};
use crate::diamond_days::team::{grade, Pos};
use crate::diamond_days::{DiamondDays, PITCH_SECS};
use crate::game_core::chibi::{self, Facing, Kit, Pose};
use crate::game_core::portrait::{Expr, Face, Outfit};
use crate::hud_art::Painter;
use crate::scene::TextAlign;
use crate::vector::{self, shade, Paint, Rgba, P};

/// A nine's shape for the radar, 0..1 on each axis: contact, power, speed, fielding, and
/// the starting pitcher (speed, control, breaking ball).
fn team_axes(side: &Side) -> [f32; 5] {
    let n = side.bats.len().max(1) as f32;
    let avg =
        |f: fn(&crate::diamond_days::sim::Batter) -> f32| side.bats.iter().map(f).sum::<f32>() / n;
    // A square-root scale: a young club (ratings near 30) still shows half a shape, and
    // 90 reaches the rim, so growth shows from the first year.
    let scale = |v: f32| ((v - 10.0) / 80.0).clamp(0.0, 1.0).sqrt();
    let pitch = side
        .arms
        .first()
        .map(|a| {
            let velo = 10.0 + (a.velo - 105.0) / 45.0 * 70.0;
            scale(velo * 0.4 + a.control * 0.3 + a.breaking * 0.3)
        })
        .unwrap_or(0.3);
    [
        scale(avg(|b| b.contact)),
        scale(avg(|b| b.power)),
        scale(avg(|b| b.speed)),
        scale(side.field),
        pitch,
    ]
}

/// Where the latest line of a game is on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Phase {
    /// Between plate appearances (or a line of words): the next batter waits.
    Idle,
    /// Pitch `i` of the plate appearance, `k` 0..1 through it.
    Pitch { i: usize, k: f32 },
    /// The result, `k` 0..1 through its hold.
    Result { k: f32 },
}

const SCORE_H: f32 = 60.0;

/// The scoreboard as of a plate appearance on screen: its half inning, and (while its
/// pitches play) the runs and the hit it has not shown yet.
#[derive(Clone, Copy, Debug)]
struct AsOf {
    inning: u8,
    side: usize,
    runs: u8,
    hit: bool,
}

/// The broadcast rectangle for a screen `hh` design units tall.
pub(in crate::diamond_days) fn broadcast_rect(hh: f32) -> [f32; 4] {
    let ph = (hh - 74.0 - 236.0).clamp(300.0, 500.0);
    [10.0, 74.0, 380.0, ph]
}

fn hash(s: &str, i: usize) -> u64 {
    art::hash(s) ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

/// Where pitch `i` of `play` crossed the plate (zone units; inside -1..1 is a strike).
pub(super) fn location(play: &Play, i: usize, pitch: Pitch) -> P {
    let h = hash(&play.en, i);
    let f = |s: u32| ((h >> s) & 0xff) as f32 / 255.0;
    // Zone x toward the batter (a right-hander stands on the left from behind the plate).
    let inside = if art::bats_left(&play.batter) {
        1.0
    } else {
        -1.0
    };
    // The pitch that hits him is the last one, right at him.
    if play.kind == PlayKind::HitByPitch && i + 1 == play.pitches().len() {
        return [inside * 2.1, -0.1 + f(4) * 0.4];
    }
    let q = base_location(pitch, &f);
    // The coach's call puts the pitches where he asked: in on the hands, away, or low.
    let wide = |strike: f32, ball: f32| {
        if pitch == Pitch::Ball {
            ball
        } else {
            strike
        }
    };
    match play.approach() {
        Some(Tactic::Inside) => [inside * wide(0.35 + f(40) * 0.6, 1.2 + f(40) * 0.35), q[1]],
        Some(Tactic::Outside) => [-inside * wide(0.4 + f(40) * 0.6, 1.2 + f(40) * 0.35), q[1]],
        Some(Tactic::Low) => [q[0], wide(0.35 + f(48) * 0.6, 1.25 + f(48) * 0.3)],
        _ => q,
    }
}

fn base_location(pitch: Pitch, f: &dyn Fn(u32) -> f32) -> P {
    match pitch {
        Pitch::Ball => {
            let side = if f(0) < 0.5 { -1.0 } else { 1.0 };
            if f(8) < 0.55 {
                [side * (1.25 + f(16) * 0.35), f(24) * 1.6 - 0.8]
            } else {
                [f(16) * 1.6 - 0.8, side.abs() * (1.25 + f(24) * 0.3)]
            }
        }
        Pitch::Looking => [(f(0) - 0.5) * 1.8, (f(8) - 0.5) * 1.8],
        Pitch::Swinging => [(f(0) - 0.5) * 1.6, 0.4 + f(8) * 0.9],
        Pitch::Foul | Pitch::InPlay => [(f(0) - 0.5) * 1.2, (f(8) - 0.5) * 1.1],
    }
}

fn is_strike(pitch: Pitch, q: P) -> bool {
    !matches!(pitch, Pitch::Ball) && q[0].abs() <= 1.05 && q[1].abs() <= 1.05
        || matches!(pitch, Pitch::Looking)
}

/// Balls and strikes after `n` pitches.
fn count(pitches: &[Pitch], n: usize) -> (u8, u8) {
    let (mut b, mut s) = (0u8, 0u8);
    for p in pitches.iter().take(n) {
        match p {
            Pitch::Ball => b += 1,
            Pitch::Looking | Pitch::Swinging => s += 1,
            Pitch::Foul => s = (s + 1).min(2),
            Pitch::InPlay => {}
        }
    }
    (b.min(3), s.min(2))
}

/// Outs a result adds.
fn outs_from(kind: PlayKind) -> u8 {
    match kind {
        PlayKind::Strikeout
        | PlayKind::GroundOut
        | PlayKind::FlyOut
        | PlayKind::SacFly
        | PlayKind::CaughtStealing => 1,
        PlayKind::DoublePlay => 2,
        _ => 0,
    }
}

/// A small strike zone panel: the zone in thirds, and a numbered dot where each pitch
/// crossed (gold for strikes, blue for balls). Zone units run -1..1 inside the box.
fn draw_zone(p: &mut Painter, r: [f32; 4], marks: &[(P, bool)]) {
    let [x, y, w, h] = r;
    p.panel(x, y, w, h, 8.0, [14, 18, 34, 190], [10, 12, 24, 190], 0.0);
    // The zone sits inside with room for balls around it.
    let (zw, zh) = (w * 0.5, h * 0.5);
    let (zx, zy) = (x + (w - zw) * 0.5, y + (h - zh) * 0.5);
    let box_ = [[zx, zy], [zx + zw, zy], [zx + zw, zy + zh], [zx, zy + zh]];
    p.fill(&box_, Paint::Solid([255, 255, 255, 26]));
    for i in 1..3 {
        let fx = zx + zw * i as f32 / 3.0;
        let fy = zy + zh * i as f32 / 3.0;
        p.line(&[[fx, zy], [fx, zy + zh]], 0.8, [255, 255, 255, 70]);
        p.line(&[[zx, fy], [zx + zw, fy]], 0.8, [255, 255, 255, 70]);
    }
    p.outline(&box_, 1.4, [255, 255, 255, 190]);
    for (n, (q, strike)) in marks.iter().enumerate() {
        let c = [
            (zx + zw * (0.5 + q[0] * 0.5)).clamp(x + 6.0, x + w - 6.0),
            (zy + zh * (0.5 + q[1] * 0.5)).clamp(y + 6.0, y + h - 6.0),
        ];
        let col = if *strike {
            [255, 206, 60, 255]
        } else {
            [110, 196, 255, 255]
        };
        p.dot(c[0], c[1], 6.0, col);
        p.text(
            &format!("{}", n + 1),
            c[0],
            c[1] - 5.5,
            9.0,
            [20, 22, 36, 255],
            TextAlign::Center,
        );
    }
}

pub(super) fn in_play(kind: PlayKind) -> bool {
    matches!(
        kind,
        PlayKind::Hit(_)
            | PlayKind::GroundOut
            | PlayKind::FlyOut
            | PlayKind::DoublePlay
            | PlayKind::SacFly
            | PlayKind::Bunt
    )
}

impl DiamondDays {
    pub(super) fn phase(
        &self,
        g: &Game,
        shown: usize,
        t: f32,
        asking: Option<Spot>,
    ) -> (Phase, Option<usize>) {
        if asking.is_some() || shown == 0 {
            return (Phase::Idle, None);
        }
        let i = shown - 1;
        let Some(play) = g.log.get(i) else {
            return (Phase::Idle, None);
        };
        if !play.is_pa() {
            return (Phase::Idle, Some(i));
        }
        let n = play.pitches().len();
        let pre = n as f32 * PITCH_SECS;
        let t = if t.is_finite() { t } else { f32::MAX };
        if t < pre {
            let at = t / PITCH_SECS;
            let k = at.fract();
            return (
                Phase::Pitch {
                    i: (at as usize).min(n.saturating_sub(1)),
                    k,
                },
                Some(i),
            );
        }
        let hold = self.spec.line_secs * self.line_pace(shown);
        let k = ((t - pre) / hold.max(0.05)).clamp(0.0, 1.0);
        (Phase::Result { k }, Some(i))
    }

    pub(super) fn face_for(&self, g: &Game, side: usize, name: &str) -> Face {
        if side == g.us {
            self.club_face(name)
        } else {
            art::face_of(name)
        }
    }

    /// The score line across the top: both schools, the innings, runs and hits. `hold`
    /// leaves out runs not yet scored on screen (side, runs).
    fn draw_scoreboard(&self, p: &mut Painter, g: &Game, y: f32, as_of: Option<AsOf>) {
        let ja = self.lang_ja;
        let colors = [self.side_color(g, 0), self.side_color(g, 1)];
        p.panel_bordered(
            10.0,
            y,
            380.0,
            SCORE_H,
            12.0,
            [24, 64, 46, 255],
            [16, 46, 34, 255],
            [210, 220, 200, 255],
            2.0,
            8.0,
        );
        let innings = g.line[0].len().max(g.line[1].len()).max(9);
        let name_w = 84.0;
        let cell = (380.0 - name_w - 58.0) / innings as f32;
        for k in 0..innings {
            p.text(
                &(k + 1).to_string(),
                10.0 + name_w + (k as f32 + 0.5) * cell,
                y + 3.0,
                10.0,
                [170, 214, 184, 255],
                TextAlign::Center,
            );
        }
        p.text(
            "R",
            352.0,
            y + 3.0,
            10.0,
            [170, 214, 184, 255],
            TextAlign::Center,
        );
        p.text(
            "H",
            376.0,
            y + 3.0,
            10.0,
            [170, 214, 184, 255],
            TextAlign::Center,
        );
        for (side, &col) in colors.iter().enumerate() {
            let yy = y + 18.0 + side as f32 * 20.0;
            let name = if ja {
                &g.sides[side].name_ja
            } else {
                &g.sides[side].name
            };
            p.fill(
                &vector::rounded_rect(16.0, yy + 1.0, 4.0, 15.0, 2.0),
                Paint::Solid(col),
            );
            let short: String = name.chars().take(if ja { 4 } else { 9 }).collect();
            p.text(&short, 24.0, yy + 1.0, 12.5, WHITE, TextAlign::Left);
            let mine = as_of.filter(|a| a.side == side);
            let held = mine.map(|a| a.runs).unwrap_or(0);
            let held_hit = mine.is_some_and(|a| a.hit) as u16;
            // Innings shown: up to the plate appearance's half, not the half the game has
            // already moved on to.
            let cells = match as_of {
                Some(a) if side == a.side || side == 0 => a.inning as usize,
                Some(a) => a.inning.saturating_sub(1) as usize,
                None => g.line[side].len(),
            }
            .min(g.line[side].len());
            for k in 0..innings.min(cells) {
                if let Some(v) = g.line[side].get(k) {
                    let v = if k + 1 == cells {
                        v.saturating_sub(held)
                    } else {
                        *v
                    };
                    let live = match as_of {
                        Some(a) => a.side == side && k + 1 == a.inning as usize,
                        None => k + 1 == cells && !g.over && (side == 0) == g.top,
                    };
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
                &(g.runs(side).saturating_sub(held as u32)).to_string(),
                352.0,
                yy - 2.0,
                17.0,
                [255, 220, 110, 255],
                TextAlign::Center,
            );
            p.text(
                &g.hits[side].saturating_sub(held_hit).to_string(),
                376.0,
                yy,
                13.0,
                [210, 230, 210, 255],
                TextAlign::Center,
            );
        }
    }

    /// A game in progress.
    pub(super) fn draw_match(
        &self,
        p: &mut Painter,
        shown: usize,
        t: f32,
        asking: Option<Spot>,
        l: &Layout,
    ) {
        let Some(g) = &self.game else { return };
        let ja = self.lang_ja;
        let hh = p.st.h;
        let [x0, y0, w, h] = p.st.screen;
        // The 3D game shows through the band between the scoreboard and the lower panel
        // (`stadium`); the HUD paints only above and below it.
        let r = broadcast_rect(hh);
        let bg = Paint::vertical(y0, y0 + h, [30, 40, 74, 255], [12, 16, 32, 255]);
        let band = |a: f32, b: f32| [[x0, a], [x0 + w, a], [x0 + w, b], [x0, b]];
        p.fill(&band(y0, r[1]), bg);
        p.fill(&band(r[1] + r[3], y0 + h), bg);
        let colors = [self.side_color(g, 0), self.side_color(g, 1)];
        let (phase, line) = self.phase(g, shown, t, asking);
        let play = line.and_then(|i| g.log.get(i));
        let pa = play.filter(|pl| pl.is_pa());
        let pitches = pa.map(|pl| pl.pitches()).unwrap_or_default();

        // Who is up and the situation, from the line (before its result) or the game.
        let (bat_side, batter, pitcher, velo, mut outs, mut bases) = match (pa, phase) {
            (Some(pl), Phase::Pitch { .. }) | (Some(pl), Phase::Result { .. }) => (
                pl.side as usize,
                pl.batter.clone(),
                pl.pitcher.clone(),
                pl.velo,
                pl.outs0,
                pl.bases0,
            ),
            _ => {
                let s = g.batting();
                let b = g
                    .bases
                    .iter()
                    .enumerate()
                    .fold(0u8, |m, (i, x)| m | ((x.is_some() as u8) << i));
                (
                    s,
                    g.batter_now().name.clone(),
                    g.pitcher_now().name.clone(),
                    g.pitcher_now().velo,
                    g.outs,
                    b,
                )
            }
        };
        let fld_side = 1 - bat_side;
        let (mut balls, mut strikes) = (0u8, 0u8);
        // The half inning on screen: the plate appearance's, else the game's.
        let (inning, top) = match (pa, phase) {
            (Some(pl), Phase::Pitch { .. } | Phase::Result { .. }) if pl.inning > 0 => {
                (pl.inning, pl.side == 0)
            }
            _ => (g.inning, g.top),
        };
        let mut as_of = pa.filter(|pl| pl.inning > 0).map(|pl| AsOf {
            inning: pl.inning,
            side: pl.side as usize,
            runs: 0,
            hit: false,
        });
        if let (Some(pl), Phase::Pitch { i, k }) = (pa, phase) {
            let done = i + usize::from(k > 0.78);
            (balls, strikes) = count(&pitches, done);
            if let Some(a) = as_of.as_mut() {
                a.runs = pl.runs;
                a.hit = matches!(pl.kind, PlayKind::Hit(_));
            }
        }
        if matches!(phase, Phase::Idle) {
            as_of = None;
        }
        if let (Some(pl), Phase::Result { .. }) = (pa, phase) {
            outs = (outs + outs_from(pl.kind)).min(3);
            // Runners after: the next plate appearance's start, or the game now.
            let next = g.log[line.unwrap_or(0) + 1..]
                .iter()
                .find(|q| q.is_pa() && q.side == pl.side);
            bases = match next {
                Some(q) => q.bases0,
                None if g.batting() as u8 == pl.side => g
                    .bases
                    .iter()
                    .enumerate()
                    .fold(0u8, |m, (i, x)| m | ((x.is_some() as u8) << i)),
                None => 0,
            };
        }
        self.draw_scoreboard(p, g, 8.0, as_of);

        // Gold rules along the edges of the picture.
        for y in [r[1], r[1] + r[3]] {
            p.fill(&band(y - 1.5, y + 1.5), Paint::Solid([210, 170, 70, 255]));
        }

        // The count box, top left of the picture.
        self.draw_count(
            p,
            [r[0] + 8.0, r[1] + 8.0],
            (inning, top),
            balls,
            strikes,
            outs,
            bases,
        );
        // The strike zone, small in the corner: where each pitch of this turn crossed.
        if let (Some(pl), Phase::Pitch { i, k }) = (pa, phase) {
            let mut marks: Vec<(P, bool)> = (0..i)
                .filter(|j| !matches!(pitches[*j], Pitch::InPlay))
                .map(|j| {
                    let q = location(pl, j, pitches[j]);
                    (q, is_strike(pitches[j], q))
                })
                .collect();
            if k > 0.74 && !matches!(pitches[i], Pitch::InPlay) {
                let q = location(pl, i, pitches[i]);
                marks.push((q, is_strike(pitches[i], q)));
            }
            draw_zone(
                p,
                [r[0] + r[2] - 74.0, r[1] + r[3] - 92.0, 66.0, 84.0],
                &marks,
            );
        }
        // The calls in force for this batter: ours, and what the other bench called.
        if let (Some(pl), Phase::Pitch { .. } | Phase::Result { .. }) = (pa, phase) {
            let mut y = r[1] + r[3] - 34.0;
            let theirs = 1 - g.us;
            for (call, who_en, who_ja, col) in [
                (pl.their_call, "Them", "相手", colors[theirs]),
                (pl.call, "Call", "作戦", colors[g.us]),
            ] {
                let Some(t) = call else { continue };
                let text = if ja {
                    format!("{who_ja}：{}", t.label(true))
                } else {
                    format!("{who_en}: {}", t.label(false))
                };
                let w = crate::font::measure_text(&text, 13.0) + 26.0;
                p.panel(
                    r[0] + 8.0,
                    y,
                    w,
                    26.0,
                    13.0,
                    [16, 20, 36, 225],
                    [10, 12, 24, 225],
                    0.0,
                );
                p.fill(
                    &vector::rounded_rect(r[0] + 14.0, y + 7.0, 5.0, 12.0, 2.5),
                    Paint::Solid(col),
                );
                p.text(&text, r[0] + 24.0, y + 5.0, 13.0, WHITE, TextAlign::Left);
                y -= 32.0;
            }
        }
        if velo > 0.0 {
            if let (Some(_), Phase::Pitch { k, .. }) = (pa, phase) {
                if k > 0.55 {
                    let kmh = velo - (art::hash(&batter) % 9) as f32;
                    p.panel(
                        r[0] + r[2] - 98.0,
                        r[1] + 8.0,
                        90.0,
                        28.0,
                        8.0,
                        [16, 20, 36, 220],
                        [10, 12, 24, 220],
                        0.0,
                    );
                    p.text(
                        &format!("{kmh:.0} km/h"),
                        r[0] + r[2] - 53.0,
                        r[1] + 12.0,
                        16.0,
                        WHITE,
                        TextAlign::Center,
                    );
                }
            }
        }

        // What just happened, big over the picture.
        let mid = r[1] + r[3] * 0.36;
        match (pa, phase) {
            (Some(pl), Phase::Pitch { i, k }) if k > 0.78 => {
                let (txt, col) = match pitches[i] {
                    Pitch::Ball => (self.tr("BALL", "ボール"), [130, 210, 255, 255]),
                    Pitch::Looking => (self.tr("STRIKE", "ストライク"), [255, 220, 90, 255]),
                    Pitch::Swinging => (self.tr("SWING AND MISS", "空振り"), [255, 220, 90, 255]),
                    Pitch::Foul => (self.tr("FOUL", "ファウル"), WHITE),
                    Pitch::InPlay => ("", WHITE),
                };
                if !txt.is_empty() && !(i + 1 == pitches.len() && pl.kind != PlayKind::Steal) {
                    p.text_outlined(
                        txt,
                        200.0,
                        mid,
                        26.0,
                        col,
                        [16, 18, 30, 255],
                        2.4,
                        TextAlign::Center,
                    );
                }
            }
            (Some(pl), Phase::Result { k }) => {
                if let Some(ab) = pl.banner {
                    p.text_outlined(
                        ab.label(ja),
                        200.0,
                        r[1] + r[3] * 0.16,
                        32.0,
                        art::GOLD,
                        [20, 16, 40, 255],
                        3.0,
                        TextAlign::Center,
                    );
                }
                let walked = pl.en.contains("on purpose");
                let (txt, col) = match pl.kind {
                    PlayKind::Hit(4) => (self.tr("HOME RUN!", "ホームラン！"), art::GOLD),
                    PlayKind::Hit(3) => {
                        (self.tr("TRIPLE!", "スリーベース！"), [255, 210, 110, 255])
                    }
                    PlayKind::Hit(2) => (self.tr("DOUBLE!", "ツーベース！"), [255, 210, 110, 255]),
                    PlayKind::Hit(_) => (self.tr("BASE HIT!", "ヒット！"), [255, 230, 150, 255]),
                    PlayKind::Strikeout => (self.tr("STRIKEOUT!", "三振！"), WHITE),
                    PlayKind::Walk if walked => {
                        (self.tr("INTENTIONAL WALK", "敬遠"), [130, 210, 255, 255])
                    }
                    PlayKind::Walk => (self.tr("BALL FOUR", "フォアボール"), [130, 210, 255, 255]),
                    PlayKind::HitByPitch => (
                        self.tr("HIT BY PITCH", "デッドボール"),
                        [255, 150, 120, 255],
                    ),
                    PlayKind::DoublePlay => (self.tr("DOUBLE PLAY", "ゲッツー"), WHITE),
                    PlayKind::SacFly => (self.tr("SAC FLY", "犠牲フライ"), WHITE),
                    PlayKind::GroundOut => (self.tr("GROUND OUT", "ゴロでアウト"), WHITE),
                    PlayKind::FlyOut => (self.tr("FLY OUT", "フライでアウト"), WHITE),
                    PlayKind::Bunt => (self.tr("BUNT", "バント"), WHITE),
                    PlayKind::Steal => {
                        (self.tr("STOLEN BASE!", "盗塁成功！"), [255, 230, 150, 255])
                    }
                    PlayKind::CaughtStealing => (self.tr("CAUGHT STEALING", "盗塁失敗"), WHITE),
                    _ => ("", WHITE),
                };
                let size = if matches!(pl.kind, PlayKind::Hit(4)) {
                    40.0
                } else {
                    32.0
                };
                if !txt.is_empty() && k < 0.97 {
                    let pop = 1.0 + (1.0 - (k * 6.0).min(1.0)) * 0.25;
                    p.text_outlined(
                        txt,
                        200.0,
                        mid - 6.0,
                        size * pop,
                        col,
                        [16, 18, 30, 255],
                        3.0,
                        TextAlign::Center,
                    );
                }
                if pl.runs > 0 && k < 0.97 {
                    let msg = if ja {
                        format!("{}点が入る！", pl.runs)
                    } else {
                        format!(
                            "{} run{} scores!",
                            pl.runs,
                            if pl.runs > 1 { "s" } else { "" }
                        )
                    };
                    p.text_outlined(
                        &msg,
                        200.0,
                        mid + 38.0,
                        20.0,
                        [255, 214, 120, 255],
                        [16, 18, 30, 255],
                        2.0,
                        TextAlign::Center,
                    );
                }
                if outs >= 3 && k > 0.45 {
                    p.text_outlined(
                        self.tr("THREE OUTS", "スリーアウト チェンジ"),
                        200.0,
                        r[1] + r[3] * 0.62,
                        18.0,
                        WHITE,
                        [16, 18, 30, 255],
                        2.0,
                        TextAlign::Center,
                    );
                }
            }
            (None, _) => {
                if let Some(pl) = play {
                    // An inning, a pitching change, a shout.
                    let txt = if ja { &pl.ja } else { &pl.en };
                    let one: String = wrap(txt, 20.0, 340.0)
                        .into_iter()
                        .next()
                        .unwrap_or_default();
                    p.text_outlined(
                        &one,
                        200.0,
                        mid,
                        20.0,
                        WHITE,
                        [16, 18, 30, 255],
                        2.4,
                        TextAlign::Center,
                    );
                }
            }
            _ => {}
        }

        // The batter and the pitcher.
        let yd = r[1] + r[3] + 8.0;
        let ab_today = |name: &str| -> (u32, u32) {
            g.log
                .iter()
                .take(shown.saturating_sub(usize::from(matches!(phase, Phase::Pitch { .. }))))
                .filter(|q| q.batter == name && q.side as usize == bat_side)
                .fold((0, 0), |(ab, hi), q| {
                    let counts = q.is_at_bat();
                    (
                        ab + counts as u32,
                        hi + matches!(q.kind, PlayKind::Hit(_)) as u32,
                    )
                })
        };
        let thrown: usize = g
            .log
            .iter()
            .take(shown.saturating_sub(usize::from(matches!(phase, Phase::Pitch { .. }))))
            .filter(|q| q.pitcher == pitcher && q.side as usize == bat_side)
            .map(|q| q.pitches().len())
            .sum::<usize>()
            + match phase {
                Phase::Pitch { i, .. } => i + 1,
                _ => 0,
            };
        let (ab, hits) = ab_today(&batter);
        let bat = g.sides[bat_side].bats.iter().find(|b| b.name == batter);
        let arm = g.sides[fld_side].arms.iter().find(|a| a.name == pitcher);
        let cards: [(&str, &str, Rgba, String, String, usize); 2] = [
            (
                self.tr("PITCHER", "投手"),
                &pitcher,
                colors[fld_side],
                arm.map(|a| {
                    format!(
                        "{} {}  {} {}",
                        self.tr("Ctl", "制球"),
                        grade(a.control.round() as u8),
                        self.tr("Brk", "変化"),
                        grade(a.breaking.round() as u8)
                    )
                })
                .unwrap_or_default(),
                if ja {
                    format!("球数 {thrown}")
                } else {
                    format!("{thrown} pitches")
                },
                fld_side,
            ),
            (
                self.tr("BATTER", "打者"),
                &batter,
                colors[bat_side],
                bat.map(|b| {
                    format!(
                        "{} {}  {} {}",
                        self.tr("Con", "ミート"),
                        grade(b.contact.round() as u8),
                        self.tr("Pow", "パワー"),
                        grade(b.power.round() as u8)
                    )
                })
                .unwrap_or_default(),
                if ja {
                    format!("今日 {ab}打数{hits}安打")
                } else {
                    format!("Today {hits} for {ab}")
                },
                bat_side,
            ),
        ];
        for (k, (lab, name, col, l1, l2, side)) in cards.into_iter().enumerate() {
            let x = 10.0 + k as f32 * 194.0;
            p.panel(
                x,
                yd,
                186.0,
                72.0,
                14.0,
                [255, 253, 246, 255],
                [240, 232, 214, 255],
                8.0,
            );
            p.fill(
                &vector::rounded_rect(x + 8.0, yd + 9.0, 4.0, 20.0, 2.0),
                Paint::Solid(col),
            );
            let face = self.face_for(g, side, name);
            p.portrait(
                x + 120.0,
                yd + 2.0,
                64.0,
                &face,
                if k == 0 { Expr::Fiery } else { Expr::Serious },
                &Outfit::cap(col),
                1.0,
            );
            p.text(lab, x + 18.0, yd + 7.0, 10.0, INK_SOFT, TextAlign::Left);
            p.text(name, x + 18.0, yd + 18.0, 16.0, INK, TextAlign::Left);
            p.text(&l1, x + 12.0, yd + 40.0, 11.0, INK, TextAlign::Left);
            p.text(&l2, x + 12.0, yd + 55.0, 11.0, INK_SOFT, TextAlign::Left);
        }

        // The caption: the situation while he bats, then what happened.
        let cy = yd + 80.0;
        p.panel_bordered(
            10.0,
            cy,
            380.0,
            44.0,
            12.0,
            [14, 18, 34, 240],
            [10, 12, 24, 240],
            [70, 80, 110, 255],
            1.5,
            6.0,
        );
        let caption = match (pa, phase) {
            (Some(pl), Phase::Result { .. }) => {
                if ja {
                    pl.ja.clone()
                } else {
                    pl.en.clone()
                }
            }
            _ => {
                let on: Vec<&str> = (0..3)
                    .filter(|b| bases & (1 << b) != 0)
                    .map(|b| {
                        if ja {
                            ["一", "二", "三"][b]
                        } else {
                            ["1st", "2nd", "3rd"][b]
                        }
                    })
                    .collect();
                let runners = if on.is_empty() {
                    self.tr("bases empty", "ランナーなし").to_string()
                } else if ja {
                    format!("{}塁", on.join("・"))
                } else {
                    format!("on {}", on.join(", "))
                };
                if ja {
                    format!(
                        "{}回{}  {}アウト {}  打者 {}",
                        inning,
                        if top { "表" } else { "裏" },
                        outs.min(2),
                        runners,
                        batter
                    )
                } else {
                    format!(
                        "{} {}, {} out, {}. {} up.",
                        if top { "Top" } else { "Bot" },
                        inning,
                        outs.min(2),
                        runners,
                        batter
                    )
                }
            }
        };
        let one: String = wrap(&caption, 15.0, 356.0)
            .into_iter()
            .next()
            .unwrap_or_default();
        p.text(
            &one,
            22.0,
            cy + 13.0,
            15.0,
            [236, 240, 250, 255],
            TextAlign::Left,
        );
        // Earlier lines, fading, while there is room above the buttons.
        let room = ((hh - 70.0 - (cy + 50.0)) / 20.0).floor().max(0.0) as usize;
        let upto = match phase {
            Phase::Result { .. } => shown.saturating_sub(1),
            _ => shown,
        };
        let earlier: Vec<&Play> = g.log[..upto.min(g.log.len())]
            .iter()
            .rev()
            .filter(|q| q.kind != PlayKind::Note || q.banner.is_some())
            .take(room.min(4))
            .collect();
        for (k, q) in earlier.iter().enumerate() {
            let a = 200u8.saturating_sub(k as u8 * 40);
            let txt = if ja { &q.ja } else { &q.en };
            let one: String = wrap(txt, 13.0, 356.0)
                .into_iter()
                .next()
                .unwrap_or_default();
            p.text(
                &one,
                22.0,
                cy + 52.0 + k as f32 * 20.0,
                13.0,
                [200, 208, 230, a],
                TextAlign::Left,
            );
        }

        // The calls, or the speed and the skip.
        if let Some(spot) = asking {
            self.draw_calls(p, g, spot, l);
        } else {
            for (b, rr) in &l.buttons {
                match *b {
                    Btn::Skip => {
                        p.button(
                            rr[0],
                            rr[1],
                            rr[2],
                            rr[3],
                            self.tr("TO THE END", "試合終了まで"),
                            14.0,
                            [110, 116, 140, 255],
                            true,
                        );
                    }
                    Btn::Speed => {
                        p.button(
                            rr[0],
                            rr[1],
                            rr[2],
                            rr[3],
                            if self.fast {
                                self.tr("SPEED x2", "倍速 ON")
                            } else {
                                self.tr("SPEED x1", "倍速 OFF")
                            },
                            14.0,
                            if self.fast {
                                ORANGE
                            } else {
                                [80, 96, 150, 255]
                            },
                            true,
                        );
                    }
                    _ => {}
                }
            }
            for (b, rr) in &l.buttons {
                if *b == Btn::Orders {
                    p.button(
                        rr[0],
                        rr[1],
                        rr[2],
                        rr[3],
                        if self.want_call {
                            self.tr("NEXT BATTER", "次の打者で")
                        } else {
                            self.tr("CALL", "作戦")
                        },
                        15.0,
                        if self.want_call {
                            [230, 150, 30, 255]
                        } else {
                            [60, 110, 200, 255]
                        },
                        true,
                    );
                }
            }
            p.text(
                self.tr("Tap the picture: next", "画面タップで先へ"),
                200.0,
                hh - 76.0,
                11.0,
                [190, 200, 230, 255],
                TextAlign::Center,
            );
        }
    }

    /// The TV count box: inning, balls, strikes, outs, runners.
    #[allow(clippy::too_many_arguments)]
    fn draw_count(
        &self,
        p: &mut Painter,
        at: P,
        (inning, top): (u8, bool),
        balls: u8,
        strikes: u8,
        outs: u8,
        bases: u8,
    ) {
        let [x, y] = at;
        p.panel(
            x,
            y,
            118.0,
            66.0,
            10.0,
            [16, 20, 36, 228],
            [10, 12, 24, 228],
            0.0,
        );
        let half = if self.lang_ja {
            format!("{}回{}", inning, if top { "表" } else { "裏" })
        } else {
            format!("{} {}", if top { "Top" } else { "Bot" }, inning)
        };
        p.text(&half, x + 8.0, y + 4.0, 12.0, WHITE, TextAlign::Left);
        let rows: [(&str, u8, u8, Rgba); 3] = [
            ("B", balls, 3, [90, 220, 120, 255]),
            ("S", strikes, 2, [255, 210, 60, 255]),
            ("O", outs.min(2), 2, [240, 80, 70, 255]),
        ];
        for (k, (lab, n, max, col)) in rows.into_iter().enumerate() {
            let yy = y + 26.0 + k as f32 * 13.0;
            p.text(
                lab,
                x + 8.0,
                yy - 6.0,
                10.0,
                [200, 205, 220, 255],
                TextAlign::Left,
            );
            for j in 0..max {
                p.dot(
                    x + 26.0 + j as f32 * 12.0,
                    yy,
                    4.5,
                    if j < n { col } else { [56, 60, 78, 255] },
                );
            }
        }
        let (bx, by) = (x + 94.0, y + 38.0);
        for (k, (dx, dy)) in [(11.0, 0.0), (0.0, -11.0), (-11.0, 0.0)]
            .into_iter()
            .enumerate()
        {
            let on = bases & (1 << k) != 0;
            let c = [bx + dx, by + dy];
            p.fill(
                &[
                    [c[0], c[1] - 6.5],
                    [c[0] + 6.5, c[1]],
                    [c[0], c[1] + 6.5],
                    [c[0] - 6.5, c[1]],
                ],
                Paint::Solid(if on {
                    [255, 200, 60, 255]
                } else {
                    [64, 70, 92, 255]
                }),
            );
        }
    }

    /// The coach's call at a chance or a pinch, or the bench.
    fn draw_calls(&self, p: &mut Painter, g: &Game, spot: Spot, l: &Layout) {
        let ja = self.lang_ja;
        p.layer();
        let r = broadcast_rect(p.st.h);
        p.fill(
            &[
                [r[0], r[1]],
                [r[0] + r[2], r[1]],
                [r[0] + r[2], r[1] + r[3]],
                [r[0], r[1] + r[3]],
            ],
            Paint::Solid([0, 0, 0, 90]),
        );
        let panel = self.call_panel(p.st.h);
        let rows_n = l
            .buttons
            .iter()
            .filter(|(b, _)| matches!(b, Btn::Call(_) | Btn::OpenBench(_) | Btn::UseBench(_)))
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
        // A call the coach asked for, with nobody in scoring position, is not a chance.
        let risp = g.bases[1].is_some() || g.bases[2].is_some();
        let (en, jp, c) = match spot {
            Spot::Chance if !risp => ("Your call, coach?", "作戦は？", [60, 110, 200, 255]),
            Spot::Pinch if !risp => (
                "On defense: your call?",
                "守りの作戦は？",
                [60, 110, 200, 255],
            ),
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
            Spot::Bench(0) => ("Pinch hitter?", "代打は誰？", [90, 70, 160, 255]),
            Spot::Bench(1) => ("Pinch runner?", "代走は誰？", [90, 70, 160, 255]),
            Spot::Bench(_) => ("Who comes in?", "守備は誰？", [90, 70, 160, 255]),
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
        // What a good coach would call here, marked on its button.
        let advice = Tactic::advice(g, spot);
        for (b, rr) in &l.buttons {
            let [x, y, w, h] = *rr;
            match *b {
                Btn::Call(Some(t)) => {
                    let col = match t {
                        Tactic::Hit | Tactic::Challenge => [200, 70, 60, 255],
                        Tactic::Bunt | Tactic::Squeeze => [60, 150, 100, 255],
                        Tactic::Steal | Tactic::HitAndRun => [220, 150, 40, 255],
                        Tactic::Inside | Tactic::Outside | Tactic::Low => [40, 140, 160, 255],
                        Tactic::InfieldIn => [90, 130, 70, 255],
                        _ => [60, 110, 200, 255],
                    };
                    // The call, and what it trades in small type under it.
                    p.button(x, y, w, h, "", 15.0, col, true);
                    let cx = x + w * 0.5;
                    let label = t.label(ja);
                    let size = if label.chars().count() > 5 {
                        13.0
                    } else {
                        15.0
                    };
                    p.text(label, cx, y + 6.0, size, WHITE, TextAlign::Center);
                    p.text(
                        t.hint(ja),
                        cx,
                        y + h - 18.0,
                        10.5,
                        [255, 255, 255, 215],
                        TextAlign::Center,
                    );
                    if advice == Some(t) {
                        let tag = self.tr("TIP", "おすすめ");
                        p.panel(
                            x + w - 46.0,
                            y - 8.0,
                            50.0,
                            17.0,
                            8.0,
                            [255, 220, 90, 255],
                            [240, 170, 40, 255],
                            0.0,
                        );
                        p.text(tag, x + w - 21.0, y - 6.5, 10.0, INK, TextAlign::Center);
                    }
                }
                Btn::Call(None) => {
                    let back = matches!(spot, Spot::Bench(_));
                    p.button(
                        x,
                        y,
                        w,
                        h,
                        if back {
                            self.tr("BACK", "もどる")
                        } else {
                            self.tr("PLAY ON", "おまかせ")
                        },
                        16.0,
                        art::QUIET,
                        true,
                    );
                }
                Btn::OpenBench(k) => {
                    let label = match (k, ja) {
                        (0, false) => "PINCH HIT",
                        (1, false) => "PINCH RUN",
                        (_, false) => "DEFENSE",
                        (0, true) => "代打",
                        (1, true) => "代走",
                        (_, true) => "守備",
                    };
                    p.button(x, y, w, h, label, 14.0, [90, 70, 160, 255], true);
                }
                Btn::UseBench(i) => {
                    let name = g
                        .sides
                        .get(g.us)
                        .and_then(|s| s.bench.get(i as usize))
                        .map(|b| b.name.as_str())
                        .unwrap_or("-");
                    p.button(x, y, w, h, name, 14.0, OURS, true);
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
    }

    /// Game day, before the first pitch.
    pub(super) fn draw_match_intro(&self, p: &mut Painter, l: &Layout) {
        let ja = self.lang_ja;
        let hh = p.st.h;
        art::ballpark(p, self.clock);
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::vertical(y0, y0 + h, [10, 14, 34, 120], [10, 14, 34, 200]),
        );
        let f = self.fixture.unwrap_or(Fixture::Practice);
        let (title, title_ja) = Self::fixture_name(f);
        let color = match f {
            Fixture::Practice => [80, 120, 200, 255],
            Fixture::Summer(_) | Fixture::National(_) => [220, 80, 60, 255],
            Fixture::Autumn(_) => [210, 120, 40, 255],
            Fixture::Spring(_) => [220, 110, 150, 255],
        };
        p.ribbon(
            200.0,
            26.0,
            330.0,
            50.0,
            if ja { &title_ja } else { &title },
            18.0,
            color,
        );
        let rival = self.rival_idx.and_then(|i| self.s.rivals.get(i));
        let (them_en, them_ja, them_col) = rival
            .map(|s| {
                (
                    s.name.clone(),
                    s.name_ja.clone(),
                    [s.color[0], s.color[1], s.color[2], 255],
                )
            })
            .unwrap_or_else(|| ("Rival".into(), "相手校".into(), [170, 50, 60, 255]));
        let us_name = if ja {
            self.school_ja()
        } else {
            self.school_en()
        };
        let them_name = if ja {
            them_ja.as_str()
        } else {
            them_en.as_str()
        };
        let slide = (self.clock * 3.0).fract();
        let _ = slide;
        let ey = 160.0;
        for (k, (name, col)) in [(us_name, OURS), (them_name, them_col)]
            .into_iter()
            .enumerate()
        {
            let cx = if k == 0 { 104.0 } else { 296.0 };
            p.glow(cx, ey, 90.0, [col[0], col[1], col[2], 110]);
            let letter: String = name
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_default();
            art::emblem(p, cx, ey, 46.0, col, &letter);
            p.text_outlined(
                name,
                cx,
                ey + 62.0,
                18.0,
                WHITE,
                [10, 12, 26, 255],
                2.0,
                TextAlign::Center,
            );
        }
        p.text_outlined(
            "VS",
            200.0,
            ey - 48.0,
            36.0,
            art::GOLD,
            [40, 20, 10, 255],
            3.0,
            TextAlign::Center,
        );
        // Their ace, between the crests.
        if let Some(arm) = self.pending_rival.as_ref().and_then(|s| s.arms.first()) {
            p.text(
                self.tr("THEIR ACE", "相手エース"),
                200.0,
                ey - 6.0,
                10.0,
                [230, 210, 160, 255],
                TextAlign::Center,
            );
            p.portrait(
                174.0,
                ey + 6.0,
                52.0,
                &art::face_of(&arm.name),
                Expr::Fiery,
                &Outfit::cap(them_col),
                1.0,
            );
            p.text_outlined(
                &arm.name,
                200.0,
                ey + 58.0,
                13.0,
                WHITE,
                [10, 12, 26, 255],
                1.6,
                TextAlign::Center,
            );
            p.text(
                &format!("{:.0} km/h", arm.velo),
                200.0,
                ey + 75.0,
                11.0,
                [220, 226, 240, 255],
                TextAlign::Center,
            );
        }
        // Strength and the record between us.
        let ours = grade(self.s.team.strength().round() as u8);
        let theirs = grade(rival.map(|s| s.strength.round() as u8).unwrap_or(50));
        for (cx, g) in [(104.0, ours), (296.0, theirs)] {
            p.text(
                self.tr("TEAM", "戦力"),
                cx - 14.0,
                ey + 92.0,
                12.0,
                [210, 216, 236, 255],
                TextAlign::Right,
            );
            p.chip(cx + 6.0, ey + 100.0, 24.0, g, grade_color(g));
        }
        if let Some(s) = rival.filter(|s| s.met > 0) {
            let rec = if ja {
                format!("これまで {}勝{}敗", s.wins, s.losses)
            } else {
                format!("Meetings so far: {}-{}", s.wins, s.losses)
            };
            p.text(
                &rec,
                200.0,
                ey + 116.0,
                13.0,
                [230, 230, 240, 255],
                TextAlign::Center,
            );
        }
        // Our order on a card.
        let oy = ey + 140.0;
        let nine = self.s.team.resolved();
        let rows = nine.len().min(9) as f32;
        let row_h = ((hh - oy - 110.0) / rows.max(1.0)).clamp(20.0, 30.0);
        let card_h = 40.0 + rows * row_h;
        p.panel(
            20.0,
            oy,
            360.0,
            card_h,
            16.0,
            art::PAPER,
            art::PAPER_SHADE,
            12.0,
        );
        p.text(
            self.tr("Starting nine", "スタメン"),
            36.0,
            oy + 10.0,
            15.0,
            INK,
            TextAlign::Left,
        );
        let ace = nine.iter().find(|(_, pos)| *pos == Pos::P).map(|(i, _)| *i);
        if let Some(a) = ace.and_then(|i| self.s.team.players.get(i)) {
            let line = if ja {
                format!("先発 {}  {}km/h", a.family, a.velo)
            } else {
                format!("Starter {}  {} km/h", a.family, a.velo)
            };
            p.text(&line, 364.0, oy + 12.0, 12.0, INK_SOFT, TextAlign::Right);
        }
        for (k, (i, pos)) in nine.iter().take(9).enumerate() {
            let pl = &self.s.team.players[*i];
            let yy = oy + 36.0 + k as f32 * row_h;
            if k % 2 == 0 {
                p.fill(
                    &vector::rounded_rect(28.0, yy - 2.0, 344.0, row_h - 2.0, 6.0),
                    Paint::Solid([0, 0, 0, 12]),
                );
            }
            p.text_display(
                &(k + 1).to_string(),
                44.0,
                yy,
                15.0,
                ORANGE,
                TextAlign::Center,
            );
            p.text(
                pos.short(ja),
                70.0,
                yy + 1.0,
                13.0,
                INK_SOFT,
                TextAlign::Center,
            );
            p.portrait(
                84.0,
                yy - 4.0,
                row_h + 2.0,
                &art::player_face(pl),
                art::mood_expr(pl.mood),
                &Outfit::cap(OURS),
                1.0,
            );
            p.text(
                &pl.family,
                84.0 + row_h + 8.0,
                yy + 1.0,
                14.0,
                INK,
                TextAlign::Left,
            );
            let stats = if ja {
                format!(
                    "ミート{} パワー{} 走力{}",
                    grade(pl.contact),
                    grade(pl.power),
                    grade(pl.speed)
                )
            } else {
                format!(
                    "Con {} Pow {} Spd {}",
                    grade(pl.contact),
                    grade(pl.power),
                    grade(pl.speed)
                )
            };
            p.text(&stats, 364.0, yy + 2.0, 11.0, INK_SOFT, TextAlign::Right);
        }
        // Both teams on one radar, when there is room above the buttons.
        let below = oy + card_h + 12.0;
        let room = hh - 86.0 - below;
        if let (Some(them), true) = (self.pending_rival.as_ref(), room > 130.0) {
            let us_side = Side::from_team(&self.s.team, self.school_en(), self.school_ja());
            let r = ((room - 46.0) * 0.5).min(62.0);
            let cy = below + 22.0 + r;
            let labels = if ja {
                ["ミート", "パワー", "走力", "守備", "投手"]
            } else {
                ["Contact", "Power", "Speed", "Field", "Pitch"]
            };
            let (a, b) = (team_axes(&us_side), team_axes(them));
            p.radar(200.0, cy, r, &labels, &[(&b, them_col), (&a, OURS)]);
            p.dot(40.0, cy - r + 6.0, 5.0, OURS);
            p.text(us_name, 50.0, cy - r - 1.0, 12.0, WHITE, TextAlign::Left);
            p.dot(40.0, cy - r + 24.0, 5.0, them_col);
            p.text(them_name, 50.0, cy - r + 17.0, 12.0, WHITE, TextAlign::Left);
        }
        for (b, r) in &l.buttons {
            match *b {
                Btn::PlayBall => {
                    p.button(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        self.tr("PLAY BALL!", "プレイボール！"),
                        18.0,
                        ORANGE,
                        true,
                    );
                }
                Btn::Lineup => {
                    p.button(
                        r[0],
                        r[1],
                        r[2],
                        r[3],
                        self.tr("ORDER", "オーダー変更"),
                        15.0,
                        [80, 96, 150, 255],
                        true,
                    );
                }
                _ => {}
            }
        }
    }

    /// After the game.
    pub(super) fn draw_match_end(&self, p: &mut Painter, l: &Layout) {
        let ja = self.lang_ja;
        let Some(rep) = &self.report else { return };
        art::ballpark(p, self.clock + 40.0);
        let [x0, y0, w, h] = p.st.screen;
        p.fill(
            &[[x0, y0], [x0 + w, y0], [x0 + w, y0 + h], [x0, y0 + h]],
            Paint::Solid(if rep.won {
                [20, 14, 4, 110]
            } else {
                [8, 12, 30, 170]
            }),
        );
        let (word, col) = if rep.won {
            (self.tr("VICTORY!", "勝利！"), art::GOLD)
        } else {
            (self.tr("DEFEAT", "敗戦"), [150, 180, 230, 255])
        };
        if rep.won {
            p.glow(200.0, 70.0, 170.0, [255, 210, 120, 150]);
            for i in 0..26 {
                let a = art::hash(&format!("c{i}"));
                let x = 20.0 + (a % 360) as f32;
                let y =
                    ((a >> 12) % 240) as f32 + (self.clock * 40.0 + i as f32 * 13.0) % 120.0 - 40.0;
                let c = [
                    [255, 90, 90, 255],
                    [255, 210, 60, 255],
                    [90, 170, 255, 255],
                    [120, 220, 140, 255],
                ][i % 4];
                p.fill(&vector::rounded_rect(x, y, 6.0, 9.0, 1.5), Paint::Solid(c));
            }
        }
        p.text(
            if ja { &rep.title.1 } else { &rep.title.0 },
            200.0,
            26.0,
            15.0,
            [230, 232, 244, 255],
            TextAlign::Center,
        );
        p.text_outlined(
            word,
            200.0,
            48.0,
            46.0,
            col,
            [20, 16, 30, 255],
            3.5,
            TextAlign::Center,
        );
        // The final line score.
        let sy = 116.0;
        p.panel_bordered(
            10.0,
            sy,
            380.0,
            SCORE_H,
            12.0,
            [24, 64, 46, 255],
            [16, 46, 34, 255],
            [210, 220, 200, 255],
            2.0,
            8.0,
        );
        let innings = rep.line[0].len().max(rep.line[1].len()).max(9);
        let name_w = 84.0;
        let cell = (380.0 - name_w - 58.0) / innings as f32;
        for k in 0..innings {
            p.text(
                &(k + 1).to_string(),
                10.0 + name_w + (k as f32 + 0.5) * cell,
                sy + 3.0,
                10.0,
                [170, 214, 184, 255],
                TextAlign::Center,
            );
        }
        p.text(
            "R",
            352.0,
            sy + 3.0,
            10.0,
            [170, 214, 184, 255],
            TextAlign::Center,
        );
        p.text(
            "H",
            376.0,
            sy + 3.0,
            10.0,
            [170, 214, 184, 255],
            TextAlign::Center,
        );
        for side in 0..2 {
            let yy = sy + 18.0 + side as f32 * 20.0;
            let c = if side == rep.us {
                OURS
            } else {
                [200, 80, 80, 255]
            };
            p.fill(
                &vector::rounded_rect(16.0, yy + 1.0, 4.0, 15.0, 2.0),
                Paint::Solid(c),
            );
            let name = if ja {
                &rep.names[side].1
            } else {
                &rep.names[side].0
            };
            let short: String = name.chars().take(if ja { 4 } else { 9 }).collect();
            p.text(&short, 24.0, yy + 1.0, 12.5, WHITE, TextAlign::Left);
            for (k, v) in rep.line[side].iter().enumerate() {
                p.text(
                    &v.to_string(),
                    10.0 + name_w + (k as f32 + 0.5) * cell,
                    yy,
                    14.0,
                    WHITE,
                    TextAlign::Center,
                );
            }
            let runs: u32 = rep.line[side].iter().map(|&v| v as u32).sum();
            p.text(
                &runs.to_string(),
                352.0,
                yy - 2.0,
                17.0,
                [255, 220, 110, 255],
                TextAlign::Center,
            );
            p.text(
                &rep.hits[side].to_string(),
                376.0,
                yy,
                13.0,
                [210, 230, 210, 255],
                TextAlign::Center,
            );
        }
        // The player of the game.
        let my = sy + SCORE_H + 16.0;
        p.panel(
            20.0,
            my,
            360.0,
            168.0,
            18.0,
            art::PAPER,
            art::PAPER_SHADE,
            12.0,
        );
        if let Some((name, en, jp)) = &rep.star {
            let face = self.club_face(name);
            chibi::draw(
                p,
                [92.0, my + 158.0],
                142.0,
                &face,
                if rep.won { Expr::Smile } else { Expr::Fiery },
                Facing::Front,
                if rep.won { Pose::Cheer } else { Pose::Stand },
                &Kit::home(OURS),
            );
            p.text(
                if rep.won {
                    self.tr("PLAYER OF THE GAME", "今日のヒーロー")
                } else {
                    self.tr("BEST EFFORT", "敢闘賞")
                },
                170.0,
                my + 22.0,
                13.0,
                ORANGE,
                TextAlign::Left,
            );
            p.text(name, 170.0, my + 42.0, 26.0, INK, TextAlign::Left);
            p.text(
                if ja { jp } else { en },
                170.0,
                my + 80.0,
                15.0,
                INK_SOFT,
                TextAlign::Left,
            );
        } else {
            p.text(
                self.tr("Nobody stood out today.", "今日は全員でつないだ試合。"),
                200.0,
                my + 70.0,
                15.0,
                INK_SOFT,
                TextAlign::Center,
            );
        }
        let exp = if ja {
            format!("試合経験 +{}", rep.exp)
        } else {
            format!("Game experience +{}", rep.exp)
        };
        p.panel(
            170.0,
            my + 112.0,
            196.0,
            34.0,
            17.0,
            [255, 236, 200, 255],
            [255, 220, 170, 255],
            0.0,
        );
        p.text(
            &exp,
            268.0,
            my + 119.0,
            15.0,
            [150, 80, 20, 255],
            TextAlign::Center,
        );
        // What it means.
        let ly = my + 184.0;
        for (k, (en, jp)) in rep.lines.iter().take(3).enumerate() {
            let txt = if ja { jp } else { en };
            p.text_outlined(
                txt,
                200.0,
                ly + k as f32 * 26.0,
                16.0,
                WHITE,
                [10, 12, 26, 255],
                2.0,
                TextAlign::Center,
            );
        }
        // The manager's word, when there is room above the button.
        let by = ly + rep.lines.len().min(3) as f32 * 26.0 + 8.0;
        if by + 96.0 < p.st.h - 92.0 {
            let (face, outfit) = art::manager();
            let (msg, expr) = if rep.won {
                (
                    self.tr(
                        "Nice game! Let's keep this going in practice!",
                        "ナイスゲーム！ この調子で練習もがんばりましょう！",
                    ),
                    Expr::Smile,
                )
            } else {
                (
                    self.tr(
                        "That one hurts... Let's turn it into practice.",
                        "くやしいですね……。次に向けて、練習あるのみです！",
                    ),
                    Expr::Sad,
                )
            };
            p.portrait(8.0, by, 92.0, &face, expr, &outfit, 1.0);
            p.bubble(
                104.0,
                by + 12.0,
                284.0,
                70.0,
                100.0,
                by + 48.0,
                [255, 250, 236, 255],
                [240, 206, 150, 255],
            );
            p.text(
                self.tr("Kotori", "七瀬ことり"),
                118.0,
                by + 17.0,
                11.0,
                [214, 100, 120, 255],
                TextAlign::Left,
            );
            for (k, line) in wrap(msg, 14.0, 256.0).iter().take(2).enumerate() {
                p.text(
                    line,
                    118.0,
                    by + 33.0 + k as f32 * 20.0,
                    14.0,
                    INK,
                    TextAlign::Left,
                );
            }
        }
        for (b, r) in &l.buttons {
            if *b == Btn::Continue {
                p.button(
                    r[0],
                    r[1],
                    r[2],
                    r[3],
                    self.tr("BACK TO PRACTICE", "練習に戻る"),
                    17.0,
                    ORANGE,
                    true,
                );
            }
        }
        let _ = (shade(OURS, 0.0), board::WEEKS);
    }
}

#[cfg(test)]
mod call_tests {
    use super::*;

    fn play(batter: &str, kind: PlayKind, call: Option<Tactic>) -> Play {
        Play {
            en: format!("{batter} test {kind:?} {call:?}"),
            batter: batter.into(),
            kind,
            call,
            ..Play::note("", "")
        }
    }

    #[test]
    fn the_pitches_go_where_the_coach_asked() {
        // Find a right-handed and a left-handed name.
        let righty = (0..100)
            .map(|i| format!("R{i}"))
            .find(|n| !art::bats_left(n))
            .unwrap();
        let lefty = (0..100)
            .map(|i| format!("L{i}"))
            .find(|n| art::bats_left(n))
            .unwrap();
        for (name, inside_sign) in [(&righty, -1.0f32), (&lefty, 1.0)] {
            for (call, check) in [(Tactic::Inside, 1.0f32), (Tactic::Outside, -1.0)] {
                let pl = play(name, PlayKind::GroundOut, Some(call));
                for (i, p) in pl.pitches().iter().enumerate() {
                    let q = location(&pl, i, *p);
                    assert!(
                        q[0] * inside_sign * check > 0.2,
                        "{name} {call:?} {p:?} {q:?}"
                    );
                }
            }
            let low = play(name, PlayKind::FlyOut, Some(Tactic::Low));
            for (i, p) in low.pitches().iter().enumerate() {
                assert!(location(&low, i, *p)[1] > 0.3, "low is low (+y)");
            }
            // The pitch that hits him is on his side, off the plate.
            let hbp = play(name, PlayKind::HitByPitch, Some(Tactic::Inside));
            let n = hbp.pitches().len();
            let q = location(&hbp, n - 1, Pitch::Ball);
            assert!(q[0] * inside_sign > 1.8, "{q:?}");
        }
    }
}

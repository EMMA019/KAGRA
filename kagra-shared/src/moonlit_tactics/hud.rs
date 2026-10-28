//! The HUD: the turn bar, HP bars over the units, the unit panel with the attack forecast,
//! the buttons, and the story / win / loss cards.

use super::rules::Side;
use super::{hud_scale, Btn, Mode, MoonlitTactics};
use crate::flow::FlowPhase;
use crate::hud_kit::{gauge, pill_button, round_panel, wrap_text};
use crate::scene::{DrawList, Quad, TextAlign, TextFace, TextQuad};

const WHITE: [u8; 4] = [240, 244, 255, 255];
const WARM: [u8; 4] = [255, 236, 200, 255];

impl MoonlitTactics {
    /// The buttons that apply now, with their rectangles (x, y, w, h).
    pub(super) fn buttons(&self, w: f32, h: f32) -> Vec<(Btn, [f32; 4])> {
        let mut out = Vec::new();
        if self.mode != Mode::Player || !self.anims.is_empty() {
            return out;
        }
        let s = hud_scale(w, h);
        let (bw, bh) = (104.0 * s, 40.0 * s);
        let y = h - bh - 14.0 * s;
        out.push((Btn::EndTurn, [w - bw - 12.0 * s, 12.0 * s, bw, 34.0 * s]));
        if self.sel.is_some() {
            out.push((Btn::Wait, [w - bw - 12.0 * s, y, bw, bh]));
        }
        if self.moved_from.is_some() {
            out.push((Btn::Undo, [w - 2.0 * bw - 20.0 * s, y, bw, bh]));
        }
        out
    }

    pub(super) fn button_at(&self, x: f32, y: f32, w: f32, h: f32) -> Option<Btn> {
        self.buttons(w, h).into_iter().find_map(|(b, r)| {
            (x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]).then_some(b)
        })
    }

    /// The in-play HUD (the page draws the title and result cards).
    pub fn hud(&self, w: u32, h: u32, _menu: bool) -> DrawList {
        let (w, h) = (w as f32, h as f32);
        self.size.set((w.max(1.0), h.max(1.0)));
        let s = hud_scale(w, h);
        let mut list = DrawList::default();
        if !matches!(self.flow.phase, FlowPhase::Play | FlowPhase::Pause) {
            return list;
        }
        let map = &self.spec.maps[self.map_i];
        // Top bar: the map, the turn, whose phase.
        list.texts.push(TextQuad::new(
            &format!("{}  {}", self.map_i + 1, self.tr(&map.name, &map.name_ja)),
            14.0 * s,
            12.0 * s,
            15.0 * s,
            [170, 200, 255, 230],
        ));
        let phase = match self.mode {
            Mode::Enemy => self.tr("ENEMY PHASE", "敵のターン"),
            _ => self.tr("YOUR PHASE", "味方のターン"),
        };
        list.texts.push(
            TextQuad::new(
                &format!("{} {}", self.tr("TURN", "ターン"), self.turn),
                14.0 * s,
                32.0 * s,
                22.0 * s,
                WARM,
            )
            .with_face(TextFace::Display),
        );
        list.texts.push(TextQuad::new(
            phase,
            14.0 * s,
            60.0 * s,
            14.0 * s,
            if self.mode == Mode::Enemy {
                [255, 140, 130, 255]
            } else {
                [140, 200, 255, 255]
            },
        ));
        // HP bars over the units.
        let view = self.view();
        let t = self.spec.tile;
        for (i, u) in self.board.units.iter().enumerate() {
            if !u.alive() {
                continue;
            }
            let p = self.unit_draw_pos(i);
            let Some((sx, sy)) = view.project([p[0], 1.25 * t, p[2]], w, h) else {
                continue;
            };
            let max = self.spec.classes[u.class].hp as f32;
            let (bw, bh) = (34.0 * s, 5.0 * s);
            gauge(
                &mut list,
                sx - bw * 0.5,
                sy,
                bw,
                bh,
                u.hp as f32 / max,
                if u.side == Side::Hero {
                    [120, 220, 255, 255]
                } else {
                    [255, 110, 100, 255]
                },
                [10, 10, 20, 190],
            );
            if u.leader {
                list.texts.push(
                    TextQuad::new("★", sx, sy - 16.0 * s, 13.0 * s, [255, 210, 120, 255])
                        .aligned(TextAlign::Center),
                );
            }
        }
        // The forecast over each target of the selected hero: damage, and the counter.
        if let Some(u) = self.sel {
            let at = self.board.units[u].at;
            for tgt in self.board.targets_from(u, at) {
                let p = self.unit_draw_pos(tgt);
                let Some((sx, sy)) = view.project([p[0], 1.6 * t, p[2]], w, h) else {
                    continue;
                };
                let label = if self.board.units[tgt].side == self.board.units[u].side {
                    format!("+{}", self.board.class(u).heal)
                } else {
                    let ex = self.board.forecast(u, tgt, at);
                    match ex.countered {
                        Some(c) => format!("-{}  ↺-{c}", ex.dealt),
                        None => format!("-{}", ex.dealt),
                    }
                };
                round_panel(
                    &mut list,
                    sx - 38.0 * s,
                    sy - 22.0 * s,
                    76.0 * s,
                    20.0 * s,
                    10.0 * s,
                    [20, 10, 20, 210],
                    [255, 120, 100, 160],
                    1.0 * s,
                    (0.0, 0.0, 0),
                );
                list.texts.push(
                    TextQuad::new(&label, sx, sy - 19.0 * s, 13.0 * s, WHITE)
                        .aligned(TextAlign::Center),
                );
            }
        }
        // Damage and heal numbers rising.
        for f in &self.floats {
            if f.delay > 0.0 {
                continue;
            }
            let rise = (1.0 - f.life) * 0.9;
            if let Some((sx, sy)) = view.project([f.at[0], 1.4 * t + rise, f.at[2]], w, h) {
                let a = (255.0 * f.life.min(1.0)) as u8;
                list.texts.push(
                    TextQuad::new(
                        &f.text,
                        sx,
                        sy,
                        22.0 * s,
                        [f.color[0], f.color[1], f.color[2], a],
                    )
                    .aligned(TextAlign::Center)
                    .with_face(TextFace::Display),
                );
            }
        }
        // The unit panel, bottom left.
        if let Some(u) = self.sel {
            self.unit_panel(&mut list, u, w, h, s);
        } else if self.mode == Mode::Player && self.anims.is_empty() {
            list.texts.push(TextQuad::new(
                self.tr("Tap one of your units", "味方のユニットをタップ"),
                14.0 * s,
                h - 44.0 * s,
                16.0 * s,
                [255, 240, 220, 220],
            ));
        }
        // Buttons.
        for (b, r) in self.buttons(w, h) {
            let (label, fill) = match b {
                Btn::Wait => (self.tr("WAIT", "待機"), [40, 70, 120, 230]),
                Btn::Undo => (self.tr("UNDO", "戻す"), [60, 60, 80, 220]),
                Btn::EndTurn => (self.tr("END TURN", "ターン終了"), [110, 50, 60, 230]),
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
        // The phase banner.
        if self.banner > 0.0 && matches!(self.mode, Mode::Player | Mode::Enemy) {
            let a = (255.0 * (self.banner / 0.4).min(1.0)) as u8;
            let (txt, col) = if self.mode == Mode::Enemy {
                (self.tr("ENEMY PHASE", "敵のターン"), [255, 130, 120])
            } else {
                (self.tr("YOUR PHASE", "味方のターン"), [140, 200, 255])
            };
            list.quads.push(Quad::new(
                0.0,
                h * 0.42,
                w,
                54.0 * s,
                [5, 5, 15, (a as f32 * 0.7) as u8],
            ));
            list.texts.push(
                TextQuad::new(
                    txt,
                    w * 0.5,
                    h * 0.42 + 13.0 * s,
                    26.0 * s,
                    [col[0], col[1], col[2], a],
                )
                .aligned(TextAlign::Center),
            );
        }
        match self.mode {
            Mode::Intro => self.card_intro(&mut list, w, h, s),
            Mode::Won | Mode::Lost if self.anims.is_empty() => self.card_end(&mut list, w, h, s),
            _ => {}
        }
        if self.flow.phase == FlowPhase::Pause {
            list.texts.push(
                TextQuad::new(
                    self.tr("PAUSED", "一時停止"),
                    w * 0.5,
                    h * 0.4,
                    30.0 * s,
                    [255; 4],
                )
                .aligned(TextAlign::Center),
            );
        }
        list
    }

    fn unit_panel(&self, list: &mut DrawList, u: usize, w: f32, h: f32, s: f32) {
        let unit = &self.board.units[u];
        let k = &self.spec.classes[unit.class];
        let (pw, ph) = ((w * 0.52).min(250.0 * s), 92.0 * s);
        let (x, y) = (12.0 * s, h - ph - 12.0 * s);
        round_panel(
            list,
            x,
            y,
            pw,
            ph,
            14.0 * s,
            [12, 16, 34, 225],
            [120, 170, 255, 110],
            1.2 * s,
            (0.0, 4.0 * s, 80),
        );
        list.texts.push(TextQuad::new(
            self.tr(&k.name, &k.name_ja),
            x + 12.0 * s,
            y + 9.0 * s,
            17.0 * s,
            WARM,
        ));
        list.texts.push(
            TextQuad::new(
                &format!("{}/{}", unit.hp, k.hp),
                x + pw - 12.0 * s,
                y + 9.0 * s,
                16.0 * s,
                WHITE,
            )
            .aligned(TextAlign::Right)
            .with_face(TextFace::Display),
        );
        gauge(
            list,
            x + 12.0 * s,
            y + 33.0 * s,
            pw - 24.0 * s,
            7.0 * s,
            unit.hp as f32 / k.hp as f32,
            [120, 220, 255, 255],
            [10, 10, 20, 200],
        );
        let range = if k.range_max > k.range_min {
            format!("{}-{}", k.range_min, k.range_max)
        } else {
            format!("{}", k.range_max)
        };
        let stats = if k.heal > 0 {
            format!(
                "{} {}  {} {}  {} {}",
                self.tr("HEAL", "回復"),
                k.heal,
                self.tr("DEF", "防"),
                k.def,
                self.tr("MV", "移"),
                k.mv
            )
        } else {
            format!(
                "{} {}  {} {}  {} {}  {} {range}",
                self.tr("ATK", "攻"),
                k.atk,
                self.tr("DEF", "防"),
                k.def,
                self.tr("MV", "移"),
                k.mv,
                self.tr("RNG", "射"),
            )
        };
        list.texts.push(TextQuad::new(
            &stats,
            x + 12.0 * s,
            y + 48.0 * s,
            14.0 * s,
            [200, 210, 235, 255],
        ));
        let hint = if self.moved_from.is_some() {
            if self.board.targets_from(u, unit.at).is_empty() {
                self.tr("Nothing in range: WAIT", "届く相手なし：待機")
            } else if k.heal > 0 {
                self.tr("Tap a green friend to heal", "緑の味方をタップで回復")
            } else {
                self.tr("Tap a red foe to attack", "赤い敵をタップで攻撃")
            }
        } else {
            self.tr("Tap a blue cell to move", "青いマスをタップで移動")
        };
        list.texts.push(TextQuad::new(
            hint,
            x + 12.0 * s,
            y + 68.0 * s,
            13.0 * s,
            [255, 230, 170, 230],
        ));
    }

    fn card(&self, list: &mut DrawList, w: f32, h: f32, s: f32, tall: f32) -> (f32, f32, f32) {
        let cw = (w - 32.0 * s).min(420.0 * s);
        let (x, y) = ((w - cw) * 0.5, h * 0.5 - tall * 0.5);
        list.quads.push(Quad::new(0.0, 0.0, w, h, [0, 0, 10, 120]));
        round_panel(
            list,
            x,
            y,
            cw,
            tall,
            16.0 * s,
            [12, 16, 34, 238],
            [120, 170, 255, 120],
            1.5 * s,
            (0.0, 6.0 * s, 90),
        );
        (x, y, cw)
    }

    fn card_intro(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let map = &self.spec.maps[self.map_i];
        let tall = 220.0 * s;
        let (x, y, cw) = self.card(list, w, h, s, tall);
        list.texts.push(
            TextQuad::new(
                &format!("MAP {}", self.map_i + 1),
                w * 0.5,
                y + 18.0 * s,
                16.0 * s,
                [140, 190, 255, 255],
            )
            .aligned(TextAlign::Center)
            .with_face(TextFace::Display),
        );
        list.texts.push(
            TextQuad::new(
                self.tr(&map.name, &map.name_ja),
                w * 0.5,
                y + 42.0 * s,
                27.0 * s,
                WARM,
            )
            .aligned(TextAlign::Center),
        );
        wrap_text(
            list,
            self.tr(&map.story, &map.story_ja),
            x + 20.0 * s,
            y + 88.0 * s,
            cw - 40.0 * s,
            15.0 * s,
            [215, 222, 240, 255],
            self.lang_ja,
        );
        let a = (160.0 + 90.0 * (self.clock * 3.0).sin()) as u8;
        list.texts.push(
            TextQuad::new(
                self.tr("Tap to begin", "タップで開始"),
                w * 0.5,
                y + tall - 32.0 * s,
                16.0 * s,
                [255, 255, 255, a],
            )
            .aligned(TextAlign::Center),
        );
    }

    fn card_end(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let tall = 210.0 * s;
        let (x, y, cw) = self.card(list, w, h, s, tall);
        let won = self.mode == Mode::Won;
        list.texts.push(
            TextQuad::new(
                if won {
                    self.tr("VICTORY", "勝利")
                } else {
                    self.tr("DEFEAT", "敗北")
                },
                w * 0.5,
                y + 18.0 * s,
                28.0 * s,
                if won {
                    [150, 255, 190, 255]
                } else {
                    [255, 130, 120, 255]
                },
            )
            .aligned(TextAlign::Center),
        );
        if let (true, Some(tl)) = (won, self.tallies.last()) {
            let rows = [
                (self.tr("Turns", "ターン"), format!("{}", tl.turns)),
                (self.tr("Standing", "生存"), format!("{}", tl.survivors)),
                (self.tr("Score", "スコア"), format!("{}", tl.score)),
            ];
            for (i, (k, v)) in rows.iter().enumerate() {
                let ry = y + 64.0 * s + i as f32 * 28.0 * s;
                list.texts.push(TextQuad::new(
                    k,
                    x + 28.0 * s,
                    ry,
                    17.0 * s,
                    [190, 200, 225, 255],
                ));
                list.texts.push(
                    TextQuad::new(v, x + cw - 28.0 * s, ry, 19.0 * s, WARM)
                        .aligned(TextAlign::Right)
                        .with_face(TextFace::Display),
                );
            }
        } else {
            list.texts.push(
                TextQuad::new(
                    self.tr(
                        "The dead hold the yard tonight.",
                        "今夜は亡者が庭を制した。",
                    ),
                    w * 0.5,
                    y + 80.0 * s,
                    16.0 * s,
                    [215, 222, 240, 255],
                )
                .aligned(TextAlign::Center),
            );
        }
        let a = (160.0 + 90.0 * (self.clock * 3.0).sin()) as u8;
        let next = if won && self.map_i + 1 < self.spec.maps.len() {
            self.tr("Tap for the next map", "タップで次のマップ")
        } else {
            self.tr("Tap to finish", "タップでおわり")
        };
        list.texts.push(
            TextQuad::new(
                next,
                w * 0.5,
                y + tall - 30.0 * s,
                15.0 * s,
                [255, 255, 255, a],
            )
            .aligned(TextAlign::Center),
        );
    }
}

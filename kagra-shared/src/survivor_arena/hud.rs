//! The HUD: the run's bars and clock, damage numbers, the stick, the level-up cards and the pause card (the loadout is in `hud_extra`).

use super::*;

impl SurvivorArena {
    pub fn hud(&self, width: u32, height: u32, _menu: bool) -> DrawList {
        let mut list = DrawList {
            clear: [0, 0, 0, 0],
            quads: Vec::new(),
            texts: Vec::new(),
            images: Vec::new(),
        };
        let (w, h) = (width.max(1) as f32, height.max(1) as f32);
        self.size.set((w, h));
        let s = hud_scale(w, h);
        if self.flow.phase == FlowPhase::Title || self.flow.phase == FlowPhase::Result {
            return list;
        }
        let text =
            |list: &mut DrawList, t: &str, x: f32, y: f32, size: f32, c: [u8; 4], a: TextAlign| {
                list.texts
                    .push(TextQuad::new(t, x, y, size * s, c).aligned(a));
            };
        // Numbers (the clock, damage) in the display face.
        let num =
            |list: &mut DrawList, t: &str, x: f32, y: f32, size: f32, c: [u8; 4], a: TextAlign| {
                list.texts.push(
                    TextQuad::new(t, x, y, size * s, c)
                        .aligned(a)
                        .with_face(TextFace::Display),
                );
            };
        // Hurt vignette.
        if self.hurt_flash > 0.0 {
            list.quads.push(Quad::new(
                0.0,
                0.0,
                w,
                h,
                [255, 40, 60, (self.hurt_flash * 70.0) as u8],
            ));
        }
        // XP bar across the top.
        let frac = (self.xp / self.xp_need()).clamp(0.0, 1.0);
        list.quads
            .push(Quad::new(0.0, 0.0, w, 12.0 * s, [8, 8, 24, 190]));
        list.quads.push(Quad::new(
            0.0,
            0.0,
            w * frac,
            12.0 * s,
            [110, 220, 255, 255],
        ));
        // A sheen on the top half and a bright tip where the bar ends.
        list.quads
            .push(Quad::new(0.0, 0.0, w * frac, 4.5 * s, [220, 250, 255, 120]));
        if frac > 0.01 {
            list.quads.push(Quad::new(
                w * frac - 3.0 * s,
                0.0,
                3.0 * s,
                12.0 * s,
                [255, 255, 255, 255],
            ));
        }
        list.quads.push(Quad::new(
            6.0 * s,
            13.0 * s,
            62.0 * s,
            21.0 * s,
            [8, 8, 24, 150],
        ));
        text(
            &mut list,
            &list_lv(self.lang_ja, self.level),
            12.0 * s,
            15.0 * s,
            15.0,
            [255, 240, 170, 255],
            TextAlign::Left,
        );
        // HP bar under it.
        let (bx, by, bw, bh) = (30.0 * s, 40.0 * s, (w * 0.40).min(200.0 * s), 16.0 * s);
        let hf = (self.hp / self.mods.hp_max).clamp(0.0, 1.0);
        list.quads.push(Quad::new(
            bx - 2.0 * s,
            by - 2.0 * s,
            bw + 4.0 * s,
            bh + 4.0 * s,
            [8, 8, 24, 200],
        ));
        let low = hf <= 0.3 && self.hp > 0.0;
        let beat = if low {
            (self.anim_t * 8.0).sin() * 0.5 + 0.5
        } else {
            0.0
        };
        let hc = if hf > 0.5 {
            [120, 235, 150, 255]
        } else if hf > 0.25 {
            [255, 200, 90, 255]
        } else {
            [
                255,
                (90.0 + 120.0 * beat) as u8,
                (100.0 + 60.0 * beat) as u8,
                255,
            ]
        };
        list.quads.push(Quad::new(bx, by, bw * hf, bh, hc));
        list.quads
            .push(Quad::new(bx, by, bw * hf, bh * 0.4, [255, 255, 255, 80]));
        text(
            &mut list,
            "♥",
            bx - 22.0 * s,
            by - 3.0 * s,
            20.0,
            [255, (90.0 + 120.0 * beat) as u8, 110, 255],
            TextAlign::Left,
        );
        if low {
            // A faint red breathing at the screen edge: the bar is nearly gone.
            list.quads.push(Quad::new(
                0.0,
                0.0,
                w,
                h,
                [255, 30, 50, (18.0 + 26.0 * beat) as u8],
            ));
        }
        text(
            &mut list,
            &format!("{}", self.hp.max(0.0).ceil() as i32),
            bx + 6.0 * s,
            by - 1.0 * s,
            12.0,
            [20, 20, 40, 255],
            TextAlign::Left,
        );
        text(
            &mut list,
            &format!("★ {} {}", self.kills, self.tr("KO", "たおした")),
            bx - 22.0 * s,
            by + 22.0 * s,
            14.0,
            [255, 235, 160, 235],
            TextAlign::Left,
        );
        // Clock: time left until dawn.
        let left = (self.spec.run.secs - self.t).max(0.0).ceil() as u32;
        let pw = 96.0 * s;
        list.quads.push(Quad::new(
            w * 0.5 - pw * 0.5,
            14.0 * s,
            pw,
            40.0 * s,
            [8, 8, 24, 150],
        ));
        let prog = (self.t / self.spec.run.secs.max(1.0)).clamp(0.0, 1.0);
        list.quads.push(Quad::new(
            w * 0.5 - pw * 0.5 + 6.0 * s,
            47.0 * s,
            (pw - 12.0 * s) * prog,
            3.0 * s,
            [255, 190 + (60.0 * prog) as u8, 110, 255],
        ));
        // The clock warms toward dawn and flickers in the last ten seconds.
        let urgent = left <= 10 && (self.anim_t * 6.0).sin() > 0.0;
        let clock = if urgent {
            [255, 150, 120, 255]
        } else {
            [
                255,
                240 - (40.0 * prog) as u8,
                200 - (60.0 * prog) as u8,
                255,
            ]
        };
        num(
            &mut list,
            &format!("{}:{:02}", left / 60, left % 60),
            w * 0.5,
            17.0 * s,
            26.0,
            clock,
            TextAlign::Center,
        );
        // Floating damage numbers.
        if self.spec.fx.damage_numbers {
            for d in &self.dmg {
                let t = d.age / DMG_LIFE;
                let Some((sx, sy)) = self.project([d.x, 0.9 + t * 0.9, d.z], w, h) else {
                    continue;
                };
                let a = ((1.0 - t * t) * 255.0) as u8;
                let pop = 1.0 + 0.6 * (1.0 - (d.age * 9.0).min(1.0));
                let label = format!("{}", d.val);
                num(
                    &mut list,
                    &label,
                    sx + 1.5 * s,
                    sy + 1.5 * s,
                    15.0 * pop,
                    [10, 10, 30, a / 2],
                    TextAlign::Center,
                );
                num(
                    &mut list,
                    &label,
                    sx,
                    sy,
                    15.0 * pop,
                    [255, 240, 190, a],
                    TextAlign::Center,
                );
            }
        }
        self.hud_loadout(&mut list, w, h, s);
        // The lock-on arrow over the locked foe.
        if self.aim_mode != 0 && self.flow.phase == FlowPhase::Play {
            if let Some(j) = self.lock_foe() {
                if let Some((sx, sy)) = self.project([self.foes.x[j], 1.35, self.foes.z[j]], w, h) {
                    let bob = (self.anim_t * 7.0).sin() * 3.0 * s;
                    text(
                        &mut list,
                        "▼",
                        sx,
                        sy - 12.0 * s + bob,
                        18.0,
                        [255, 96, 96, 255],
                        TextAlign::Center,
                    );
                }
            }
        }
        // Pause button.
        if self.flow.phase == FlowPhase::Play {
            let r = pause_rect(w, s);
            list.quads
                .push(Quad::new(r.x, r.y, r.w, r.h, [8, 8, 24, 150]));
            for dx in [0.32_f32, 0.58] {
                list.quads.push(Quad::new(
                    r.x + r.w * dx,
                    r.y + r.h * 0.26,
                    r.w * 0.11,
                    r.h * 0.48,
                    [255, 255, 255, 230],
                ));
            }
        }
        if self.stick_mode() && self.flow.phase == FlowPhase::Play && self.cards.is_empty() {
            // The thumb stick: a resting hint where a thumb lands, or the base where it landed,
            // a knob under the thumb and a jelly neck between them.
            let rad = self.stick_radius(w, h);
            let look = stick::StickLook::default();
            match self.finger {
                Some(f) => {
                    let k = stick::knob(self.origin, f, rad);
                    stick::draw(&mut list, self.origin, k, rad, s, true, &look);
                }
                None => {
                    let c = self.stick_center(w, h);
                    stick::draw(&mut list, c, c, rad, s, false, &look);
                }
            }
        } else if let Some((fx, fy)) = self.finger {
            // The thumb marker: a soft round dot (a square read as a stray grey box).
            text(
                &mut list,
                "●",
                fx,
                fy - 12.0 * s,
                22.0,
                [255, 255, 255, 70],
                TextAlign::Center,
            );
        }
        if !self.cards.is_empty() {
            self.hud_cards(&mut list, w, h, s);
        } else if self.flow.phase == FlowPhase::Pause {
            self.hud_pause(&mut list, w, h, s);
        }
        list
    }

    pub(super) fn hud_cards(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        list.quads.push(Quad::new(0.0, 0.0, w, h, [6, 6, 20, 200]));
        list.texts.push(
            TextQuad::new(
                if self.swap_in.is_some() {
                    self.tr("WEAPON FOUND!", "ぶきをみつけた！")
                } else {
                    self.tr("LEVEL UP!", "レベルアップ！")
                },
                w * 0.5,
                h * 0.09,
                32.0 * s,
                [255, 240, 160, 255],
            )
            .aligned(TextAlign::Center),
        );
        list.texts.push(
            TextQuad::new(
                if self.swap_in.is_some() {
                    self.tr(
                        "Slots full: drop one?",
                        "スロットがいっぱい：どれをすてる？",
                    )
                } else {
                    self.tr("Pick one", "ひとつえらんでね")
                },
                w * 0.5,
                h * 0.09 + 40.0 * s,
                14.0 * s,
                [255, 255, 255, 190],
            )
            .aligned(TextAlign::Center),
        );
        for (c, r) in self.cards.iter().zip(card_rects(w, h, s, self.cards.len())) {
            let (tag, name, desc, accent) = self.card_text(*c);
            let a = [accent[0], accent[1], accent[2], 255];
            list.quads.push(Quad::new(
                r.x - 3.0 * s,
                r.y - 3.0 * s,
                r.w + 6.0 * s,
                r.h + 6.0 * s,
                a,
            ));
            list.quads
                .push(Quad::new(r.x, r.y, r.w, r.h, [28, 24, 56, 250]));
            list.quads.push(Quad::new(r.x, r.y, 8.0 * s, r.h, a));
            // An icon badge: the card's glyph on a tinted tile.
            let portrait = h >= w;
            let bs = if portrait {
                (r.h * 0.62).min(76.0 * s)
            } else {
                56.0 * s
            };
            let (bx, by) = if portrait {
                (r.x + 18.0 * s, r.y + (r.h - bs) * 0.5)
            } else {
                (r.x + 18.0 * s, r.y + 14.0 * s)
            };
            list.quads.push(Quad::new(
                bx,
                by,
                bs,
                bs,
                [a[0] / 3, a[1] / 3, a[2] / 3, 255],
            ));
            list.quads
                .push(Quad::new(bx, by, bs, 3.0 * s, [a[0], a[1], a[2], 200]));
            list.texts.push(
                TextQuad::new(
                    self.card_glyph(*c),
                    bx + bs * 0.5,
                    by + bs * 0.5 - bs * 0.30,
                    bs * 0.6,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Center),
            );
            let (tx, ty) = if portrait {
                (bx + bs + 14.0 * s, r.y + 9.0 * s)
            } else {
                (r.x + 22.0 * s, by + bs + 12.0 * s)
            };
            let (cur, max) = self.card_pips(*c);
            if max > 1 {
                let pips: String = (0..max)
                    .map(|k| match k.cmp(&cur) {
                        std::cmp::Ordering::Less => '●',
                        std::cmp::Ordering::Equal => '◆',
                        std::cmp::Ordering::Greater => '○',
                    })
                    .collect();
                list.texts.push(
                    TextQuad::new(&pips, r.x + r.w - 12.0 * s, r.y + 8.0 * s, 12.0 * s, a)
                        .aligned(TextAlign::Right),
                );
            }
            list.texts.push(TextQuad::new(&tag, tx, ty, 12.0 * s, a));
            list.texts.push(TextQuad::new(
                &name,
                tx,
                ty + 17.0 * s,
                22.0 * s,
                [255, 255, 255, 255],
            ));
            list.texts.push(TextQuad::new(
                &desc,
                tx,
                ty + 47.0 * s,
                14.0 * s,
                [225, 225, 245, 230],
            ));
        }
    }

    pub(super) fn hud_pause(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        list.quads.push(Quad::new(0.0, 0.0, w, h, [6, 6, 20, 205]));
        list.texts.push(
            TextQuad::new(
                self.tr("PAUSED", "ポーズ"),
                w * 0.5,
                h * 0.16,
                30.0 * s,
                [255, 255, 255, 255],
            )
            .aligned(TextAlign::Center),
        );
        for (b, r) in pause_buttons(w, h, s) {
            let (label, fill, ink) = match b {
                PauseBtn::Resume => (
                    self.tr("RESUME", "つづける"),
                    [255, 170, 80, 255],
                    [30, 14, 20, 255],
                ),
                PauseBtn::Aim => (
                    match self.aim_mode {
                        0 => self.tr("AIM: AUTO", "ねらい: オート"),
                        1 => self.tr("AIM: TAP", "ねらい: タップ"),
                        _ => self.tr("AIM: HYBRID", "ねらい: ミックス"),
                    },
                    [40, 34, 74, 245],
                    [150, 235, 255, 255],
                ),
                PauseBtn::Retry => (
                    self.tr("RETRY", "リトライ"),
                    [40, 34, 74, 245],
                    [255, 255, 255, 255],
                ),
                PauseBtn::Title => (
                    self.tr("TITLE", "タイトル"),
                    [40, 34, 74, 245],
                    [255, 255, 255, 255],
                ),
            };
            list.quads.push(Quad::new(r.x, r.y, r.w, r.h, fill));
            list.texts.push(
                TextQuad::new(
                    label,
                    r.x + r.w * 0.5,
                    r.y + r.h * 0.5 - 9.0 * s,
                    18.0 * s,
                    ink,
                )
                .aligned(TextAlign::Center),
            );
        }
    }
}

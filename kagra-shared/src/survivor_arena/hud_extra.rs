//! The loadout part of the HUD: weapon slots, passives, power-up timers, the pickup toast and
//! the elite's health bar. All plain quads and glyphs.

use super::*;

impl SurvivorArena {
    pub(super) fn hud_loadout(&self, list: &mut DrawList, w: f32, h: f32, s: f32) {
        let cs = 34.0 * s; // a weapon chip
        let gap = 4.0 * s;
        let (x0, y0) = (8.0 * s, 84.0 * s);
        let max = self.spec.levels.max_weapons as usize;
        let mut slot = 0;
        for (i, wp) in self.spec.weapons.iter().enumerate() {
            let lv = self.weapon_lv[i];
            if lv == 0 {
                continue;
            }
            let x = x0 + slot as f32 * (cs + gap);
            slot += 1;
            let c = wp.color.map(|v| v.min(255) as u8);
            let a = [c[0], c[1], c[2], 255];
            list.quads.push(Quad::new(
                x - 2.0 * s,
                y0 - 2.0 * s,
                cs + 4.0 * s,
                cs + 4.0 * s,
                a,
            ));
            list.quads.push(Quad::new(x, y0, cs, cs, [14, 14, 36, 235]));
            // Cooldown: the bar fills as the weapon charges.
            let st = &self.weapon_stats[i];
            let ready = if wp.kind == WeaponKind::Orbit {
                1.0
            } else {
                1.0 - (self.weapon_cd[i] / (st.cooldown * self.cd_scale()).max(0.1)).clamp(0.0, 1.0)
            };
            list.quads
                .push(Quad::new(x, y0 + cs - 4.0 * s, cs * ready, 4.0 * s, a));
            list.texts.push(
                TextQuad::new(
                    &wp.glyph,
                    x + cs * 0.5,
                    y0 + 1.0 * s,
                    20.0 * s,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Center),
            );
            let maxed = lv as usize > wp.levels.len();
            list.texts.push(
                TextQuad::new(
                    &if maxed {
                        "MAX".to_string()
                    } else {
                        format!("{lv}")
                    },
                    x + 3.0 * s,
                    y0 + cs - 17.0 * s,
                    11.0 * s,
                    if maxed {
                        [255, 225, 120, 255]
                    } else {
                        [230, 235, 255, 230]
                    },
                )
                .aligned(TextAlign::Left),
            );
            // A star on a weapon that has a synergy switched on.
            let has_syn = self
                .spec
                .synergies
                .iter()
                .enumerate()
                .take(32)
                .any(|(k, sy)| self.syn_mask & (1 << k) != 0 && sy.weapon == wp.id);
            if has_syn {
                list.texts.push(
                    TextQuad::new(
                        "★",
                        x + cs - 2.0 * s,
                        y0 - 3.0 * s,
                        13.0 * s,
                        [255, 225, 90, 255],
                    )
                    .aligned(TextAlign::Right),
                );
            }
        }
        // Empty slots stay visible, so a full bar reads as full.
        while slot < max {
            let x = x0 + slot as f32 * (cs + gap);
            list.quads.push(Quad::new(x, y0, cs, cs, [14, 14, 36, 110]));
            list.quads
                .push(Quad::new(x, y0, cs, 2.0 * s, [120, 120, 170, 120]));
            slot += 1;
        }
        // Passives: smaller chips on a second row.
        let ps = 22.0 * s;
        let py = y0 + cs + 8.0 * s;
        let mut n = 0;
        for i in 0..self.spec.passives.len() {
            let lv = self.passive_lv[i];
            if lv == 0 {
                continue;
            }
            let x = x0 + n as f32 * (ps + 3.0 * s);
            n += 1;
            list.quads.push(Quad::new(x, py, ps, ps, [16, 40, 52, 220]));
            list.quads
                .push(Quad::new(x, py, ps, 2.0 * s, [110, 220, 230, 255]));
            list.texts.push(
                TextQuad::new(
                    self.card_glyph(Card::Passive(i as u8)),
                    x + ps * 0.5,
                    py + 1.0 * s,
                    13.0 * s,
                    [255, 255, 255, 255],
                )
                .aligned(TextAlign::Center),
            );
            list.texts.push(
                TextQuad::new(
                    &format!("{lv}"),
                    x + ps - 2.0 * s,
                    py + ps - 11.0 * s,
                    9.0 * s,
                    [200, 235, 245, 230],
                )
                .aligned(TextAlign::Right),
            );
        }

        // Power-up timers under the clock.
        let mut buffs = 0;
        let mut chip = |list: &mut DrawList, glyph: &str, left: f32, total: f32, c: [u8; 3]| {
            let bw = 62.0 * s;
            let x = w * 0.5 - 70.0 * s + buffs as f32 * (bw + 6.0 * s);
            let y = 62.0 * s;
            list.quads
                .push(Quad::new(x, y, bw, 18.0 * s, [8, 8, 24, 190]));
            list.quads.push(Quad::new(
                x,
                y + 15.0 * s,
                bw * (left / total.max(0.1)).clamp(0.0, 1.0),
                3.0 * s,
                [c[0], c[1], c[2], 255],
            ));
            list.texts.push(TextQuad::new(
                &format!("{glyph}{}", left.ceil() as i32),
                x + 4.0 * s,
                y + 1.0 * s,
                13.0 * s,
                [c[0], c[1], c[2], 255],
            ));
            buffs += 1;
        };
        if self.shield_t > 0.0 {
            let total = self.buff_secs("shield");
            chip(list, "◇ ", self.shield_t, total, [120, 220, 255]);
        }
        if self.frenzy_t > 0.0 {
            let total = self.buff_secs("frenzy");
            chip(list, "▲ ", self.frenzy_t, total, [255, 160, 70]);
        }

        // The elite's health bar.
        let mut best: Option<usize> = None;
        for i in 0..self.foes.len() {
            if self.kinds[self.foes.kind[i] as usize].elite {
                best = Some(i);
                break;
            }
        }
        if let (Some(j), true) = (best, self.aux.len() >= self.foes.len()) {
            let f = (self.foes.hp[j] / self.aux[j].hp_max.max(1.0)).clamp(0.0, 1.0);
            let (bw, bh) = ((w * 0.5).min(260.0 * s), 9.0 * s);
            let (x, y) = (w * 0.5 - bw * 0.5, 88.0 * s);
            list.quads.push(Quad::new(
                x - 2.0 * s,
                y - 2.0 * s,
                bw + 4.0 * s,
                bh + 4.0 * s,
                [8, 8, 24, 210],
            ));
            list.quads
                .push(Quad::new(x, y, bw * f, bh, [255, 110, 90, 255]));
            list.quads
                .push(Quad::new(x, y, bw * f, bh * 0.4, [255, 255, 255, 70]));
            list.texts.push(
                TextQuad::new(
                    &format!("☠ {}", self.tr("WARDEN", "ばんにん")),
                    w * 0.5,
                    y + bh + 3.0 * s,
                    12.0 * s,
                    [255, 200, 190, 255],
                )
                .aligned(TextAlign::Center),
            );
        }

        // A marker for the nearest chest that is off screen (or far): an arrow on the edge.
        let mut best: Option<(f32, f32, f32)> = None; // (distance, dx, dz)
        for p in self.pickups.iter().filter(|p| p.item == CHEST) {
            let (dx, dz) = (p.x - self.hx, p.z - self.hz);
            let d = dx.hypot(dz);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, dx, dz));
            }
        }
        if let Some((d, dx, dz)) = best {
            let on = self
                .project([self.hx + dx, 0.5, self.hz + dz], w, h)
                .filter(|&(x, y)| {
                    x > 24.0 * s && x < w - 24.0 * s && y > 90.0 * s && y < h - 24.0 * s
                });
            if on.is_none() {
                // The hero is near the middle of the view, so the bearing on screen is the bearing
                // in the world turned by the camera (it looks down -z): up is -z.
                let a = dx.atan2(-dz); // 0 = up the screen, clockwise
                let arrows = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"];
                let k = ((a / std::f32::consts::FRAC_PI_4).round() as i32).rem_euclid(8) as usize;
                let (cx, cy) = (w * 0.5, h * 0.5);
                let (sx, sy) = (a.sin(), -a.cos());
                let tx = if sx.abs() > 1e-3 {
                    (w * 0.5 - 40.0 * s) / sx.abs()
                } else {
                    f32::MAX
                };
                let ty = if sy.abs() > 1e-3 {
                    (h * 0.5 - 70.0 * s) / sy.abs()
                } else {
                    f32::MAX
                };
                let r = tx.min(ty);
                let (mx, my) = (cx + sx * r, cy + sy * r);
                let pulse = 0.75 + 0.25 * (self.anim_t * 6.0).sin();
                let col = [255, 215, 90, (255.0 * pulse) as u8];
                list.texts.push(
                    TextQuad::new(arrows[k], mx, my - 14.0 * s, 28.0 * s, col)
                        .aligned(TextAlign::Center),
                );
                list.texts.push(
                    TextQuad::new(
                        &format!("宝 {:.0}m", d),
                        mx,
                        my + 12.0 * s,
                        13.0 * s,
                        [255, 235, 160, 235],
                    )
                    .aligned(TextAlign::Center),
                );
            }
        }

        // The toast: what was just picked up.
        if self.toast_t > 0.0 && !self.toast.is_empty() {
            let f = (self.toast_t / TOAST_LIFE).clamp(0.0, 1.0);
            let a = (f * 4.0).min(1.0);
            let rise = (1.0 - f) * 14.0 * s;
            let c = self.toast_col;
            let size = 20.0 * s;
            list.texts.push(
                TextQuad::new(
                    &self.toast,
                    w * 0.5 + 1.5 * s,
                    h * 0.2 - rise + 1.5 * s,
                    size,
                    [10, 10, 30, (a * 130.0) as u8],
                )
                .aligned(TextAlign::Center),
            );
            list.texts.push(
                TextQuad::new(
                    &self.toast,
                    w * 0.5,
                    h * 0.2 - rise,
                    size,
                    [c[0], c[1], c[2], (a * 255.0) as u8],
                )
                .aligned(TextAlign::Center),
            );
        }
    }

    /// The full length of a timed power-up, for its bar.
    fn buff_secs(&self, kind: &str) -> f32 {
        self.spec
            .drops
            .items
            .iter()
            .find(|i| match kind {
                "shield" => i.kind == PickupKind::Shield,
                _ => i.kind == PickupKind::Frenzy,
            })
            .map_or(8.0, |i| i.secs)
    }
}

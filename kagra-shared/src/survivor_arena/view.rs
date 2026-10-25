//! Drawing the new things: orbs, blades, pickups, chests, lightning, floor rings and the
//! shield. Everything refills pooled props and lines in place, so nothing allocates.

use super::*;

use std::f32::consts::{PI, TAU};

/// Fill `l` with a horizontal circle at height `y`.
fn circle(l: &mut crate::strokes::FatLine, x: f32, z: f32, r: f32, y: f32) {
    l.points.clear();
    for i in 0..=RING_SEG {
        let a = i as f32 / RING_SEG as f32 * TAU;
        l.points.push([x + a.sin() * r, y, z + a.cos() * r]);
    }
}

impl SurvivorArena {
    pub(super) fn build_extras(&mut self, slots: &Slots) {
        let t = self.anim_t;
        let props = &mut self.doc.props;
        let lines = &mut self.doc.lines;
        let (hx, hz) = (self.hx, self.hz);

        // Enemy orbs.
        for i in 0..ORB_CAP {
            let p = &mut props[slots.orb0 + i];
            match self.orbs.get(i) {
                None => {
                    if p.enabled {
                        p.enabled = false;
                    }
                }
                Some(o) => {
                    let pulse = 1.0 + 0.2 * (t * 16.0 + i as f32).sin();
                    p.enabled = true;
                    p.position = [o.x, 0.6, o.z];
                    p.scale = [ORB_R * 2.0 * pulse; 3];
                }
            }
        }

        // Orbiting blades.
        let orbit = (0..self.spec.weapons.len())
            .find(|&i| self.spec.weapons[i].kind == WeaponKind::Orbit && self.weapon_lv[i] > 0);
        let (n, area, col) = match orbit {
            Some(wi) => {
                let st = self.weapon_stats[wi];
                let c = self.spec.weapons[wi].color.map(|c| c.min(255));
                (st.count.clamp(1, BLADE_CAP as i32) as usize, st.area, c)
            }
            None => (0, 0.0, [150, 220, 255]),
        };
        for k in 0..BLADE_CAP {
            let p = &mut props[slots.blade0 + k];
            if k >= n {
                if p.enabled {
                    p.enabled = false;
                }
                continue;
            }
            let a = self.orbit_phase + k as f32 * TAU / n as f32;
            let r = area * (1.0 + 0.05 * (t * 3.0).sin());
            p.enabled = true;
            p.position = [hx + a.sin() * r, 0.65, hz + a.cos() * r];
            // The sword lies along its own +z; turn it to run along the circle.
            p.yaw = a + PI * 0.5;
            p.emissive_color = Some(lin(col));
        }
        let l = &mut lines[slots.ln_orbit];
        if n > 0 {
            circle(l, hx, hz, area, 0.06);
            l.color = [col[0] as u8, col[1] as u8, col[2] as u8, 55];
        } else {
            l.points.clear();
        }

        // Pickups.
        let d = &self.spec.drops;
        let (life, blink) = (d.life, d.blink);
        let mut chests = 0;
        for i in 0..d.cap as usize {
            let pk = self.pickups.get(i);
            let (mut show, mut is_chest, mut col) = (false, false, [255_u8; 3]);
            if let Some(pk) = pk {
                is_chest = pk.item == CHEST;
                col = if is_chest {
                    [255, 215, 90]
                } else {
                    d.items[pk.item as usize].color.map(|c| c.min(255) as u8)
                };
                let span = if is_chest { life * 4.0 } else { life };
                let hide = span - pk.age < blink && (t * 9.0) as i32 % 2 == 0;
                show = !hide;
            }
            let (a, rest) = props.split_at_mut(slots.glow0);
            let (g, ch) = rest.split_at_mut(slots.chest0 - slots.glow0);
            let (orb, glow) = (&mut a[slots.pick0 + i], &mut g[i]);
            let (Some(pk), true) = (pk, show) else {
                if orb.enabled {
                    orb.enabled = false;
                }
                if glow.enabled {
                    glow.enabled = false;
                }
                if let Some(l) = lines.get_mut(slots.ln_beam + i) {
                    l.points.clear();
                }
                continue;
            };
            let bob = 0.12 * (t * 3.6 + i as f32 * 1.3).sin();
            let pulse = 1.0 + 0.12 * (t * 6.0 + i as f32).sin();
            let lc = lin([col[0] as u32, col[1] as u32, col[2] as u32]);
            glow.enabled = true;
            glow.position = [pk.x, 0.04, pk.z];
            let g = (if is_chest { 1.5 } else { 0.9 }) * pulse;
            glow.scale = [g, 0.03, g];
            glow.color = Some(col.map(u32::from));
            glow.emissive = Some(0.9);
            glow.emissive_color = Some(lc);
            if is_chest && chests < CHEST_CAP && slots.glb_chest {
                let c = &mut ch[chests];
                chests += 1;
                c.enabled = true;
                c.position = [pk.x, 0.05 + bob * 0.3, pk.z];
                c.yaw = t * 1.1;
                c.emissive = Some(0.35 + 0.2 * (t * 5.0).sin());
                c.emissive_color = Some(lc);
                orb.enabled = false;
            } else {
                orb.enabled = true;
                let s = if is_chest { 0.6 } else { 0.34 } * pulse;
                orb.position = [pk.x, 0.5 + bob, pk.z];
                orb.scale = [s; 3];
                orb.color = Some(col.map(u32::from));
                orb.emissive = Some(1.4);
                orb.emissive_color = Some(lc);
            }
            // A light column for anything good, so it can be spotted across the arena.
            if let Some(l) = lines.get_mut(slots.ln_beam + i) {
                l.points.clear();
                let good = is_chest || d.items[pk.item as usize].rarity >= 1;
                if good {
                    let h = if is_chest { 2.6 } else { 1.8 };
                    l.points.push([pk.x, 0.1, pk.z]);
                    l.points.push([pk.x, h, pk.z]);
                    l.color = [col[0], col[1], col[2], (90.0 + 50.0 * pulse) as u8];
                    l.width = if is_chest { 8.0 } else { 5.0 };
                }
            }
        }
        for j in chests..CHEST_CAP {
            let c = &mut props[slots.chest0 + j];
            if c.enabled {
                c.enabled = false;
            }
        }

        // Lightning: a jagged line through the hero and each foe it struck, fading fast.
        for k in 0..ZAP_CAP {
            let l = &mut lines[slots.ln_zap + k];
            l.points.clear();
            let Some(z) = self.zaps.get(k) else {
                continue;
            };
            let fade = 1.0 - (z.age / ZAP_LIFE).min(1.0);
            let c = self
                .spec
                .weapons
                .get(z.w as usize)
                .map_or([255, 245, 130], |w| w.color.map(|c| c.min(255) as u8));
            l.color = [c[0], c[1], c[2], (255.0 * fade) as u8];
            l.width = 3.0 + 5.0 * fade;
            let n = z.n as usize;
            for m in 0..n {
                let (x0, z0) = if m == 0 {
                    (hx, hz)
                } else {
                    (z.pts[m - 1][0], z.pts[m - 1][1])
                };
                let y0 = if m == 0 { 0.75 } else { 0.6 };
                if m == 0 {
                    l.points.push([x0, y0, z0]);
                }
                let (x1, z1) = (z.pts[m][0], z.pts[m][1]);
                let (dx, dz) = (x1 - x0, z1 - z0);
                let len = dx.hypot(dz).max(1e-3);
                let (px, pz) = (-dz / len, dx / len);
                for sub in 1..4 {
                    let f = sub as f32 / 4.0;
                    let j = ((t * 71.0 + (k * 13 + m * 5 + sub) as f32 * 2.3).sin()) * 0.28 * fade;
                    l.points
                        .push([x0 + dx * f + px * j, 0.7, z0 + dz * f + pz * j]);
                }
                l.points.push([x1, 0.6, z1]);
            }
        }

        // Expanding floor rings.
        for k in 0..RING_CAP {
            let l = &mut lines[slots.ln_fring + k];
            match self.rings.get(k) {
                None => l.points.clear(),
                Some(r) => {
                    let f = (r.age / r.life).min(1.0);
                    let e = 1.0 - (1.0 - f) * (1.0 - f);
                    circle(l, r.x, r.z, r.r * e, 0.1);
                    l.color = [
                        r.color[0],
                        r.color[1],
                        r.color[2],
                        (230.0 * (1.0 - f)) as u8,
                    ];
                    l.width = 2.0 + 6.0 * (1.0 - f);
                }
            }
        }

        // The shield: a ring round the hero, flickering as it runs out.
        let l = &mut lines[slots.ln_shield];
        if self.shield_t > 0.0 && (self.shield_t > 1.5 || (t * 10.0) as i32 % 2 == 0) {
            circle(l, hx, hz, 0.95 + 0.05 * (t * 8.0).sin(), 0.12);
            l.width = 4.0 + 2.0 * (t * 6.0).sin();
        } else {
            l.points.clear();
        }
    }
}

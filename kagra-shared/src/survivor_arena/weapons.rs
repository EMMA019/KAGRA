//! The weapons of the survivor arena: how each kind fires, moves and hurts. A child module of
//! `survivor_arena` (it works on the same run state), kept apart so the main file stays
//! readable. Nothing here allocates in a step: shots, zaps and rings live in pools sized
//! once, area hits go through the `scratch` index list.

use super::*;

/// Shove (m) a pulse gives a foe at the ring's edge.
const PULSE_SHOVE: f32 = 0.9;
/// The share of a hit that armor can never take away.
const ARMOR_FLOOR: f32 = 0.3;

impl SurvivorArena {
    /// Cooldown scale of the passives and a frenzy.
    pub(super) fn cd_scale(&self) -> f32 {
        let f = if self.frenzy_t > 0.0 {
            1.0 - self.frenzy_amt
        } else {
            1.0
        };
        self.mods.cooldown * f
    }

    /// Hurt foe `j` for `dmg` from direction `dir` (a unit vector pointing from the foe toward the
    /// source), shoving it `knock` metres away. `armored` hits lose the foe's armor; blasts,
    /// pulses and bombs pass `false`. Returns the damage dealt. Does not remove the foe.
    pub(super) fn damage_foe(
        &mut self,
        j: usize,
        dmg: f32,
        dir: (f32, f32),
        knock: f32,
        armored: bool,
    ) -> f32 {
        let k = &self.kinds[self.foes.kind[j] as usize];
        let dealt = if armored {
            (dmg - k.armor).max(dmg * ARMOR_FLOOR)
        } else {
            dmg
        };
        let push = knock * (1.0 - k.resist);
        self.foes.hp[j] -= dealt;
        self.foes.flash[j] = 0.1;
        self.foes.x[j] -= dir.0 * push;
        self.foes.z[j] -= dir.1 * push;
        if self.spec.fx.damage_numbers {
            if self.dmg.len() >= DMG_CAP[self.tier] {
                self.dmg.remove(0);
            }
            self.dmg.push(DmgNum {
                x: self.foes.x[j],
                z: self.foes.z[j],
                val: dealt.round().max(1.0) as i32,
                age: 0.0,
            });
        }
        dealt
    }

    /// Every foe whose body touches the circle (`x`,`z`,`r`), into `scratch`.
    fn foes_in(&mut self, x: f32, z: f32, r: f32) {
        self.scratch.clear();
        let (foes, kinds, sc) = (&self.foes, &self.kinds, &mut self.scratch);
        self.grid.query(x, z, r + MAX_FOE_R, |j| {
            if j >= foes.len() {
                return; // the grid is a step old: a foe may have just been removed
            }
            let rs = r + kinds[foes.kind[j] as usize].radius;
            let (dx, dz) = (foes.x[j] - x, foes.z[j] - z);
            if dx * dx + dz * dz < rs * rs && sc.len() < sc.capacity() {
                sc.push(j);
            }
        });
    }

    fn weapon_color(&self, w: u8) -> [u8; 3] {
        self.spec
            .weapons
            .get(w as usize)
            .map_or([255, 190, 90], |s| s.color.map(|c| c.min(255) as u8))
    }

    pub(super) fn add_ring(&mut self, x: f32, z: f32, r: f32, life: f32, color: [u8; 3]) {
        if self.rings.len() < RING_CAP {
            self.rings.push(Ring {
                x,
                z,
                r,
                age: 0.0,
                life,
                color,
            });
        }
    }

    pub(super) fn fire(&mut self, dt: f32) {
        for wi in 0..self.spec.weapons.len() {
            if self.weapon_lv[wi] == 0 {
                continue;
            }
            let kind = self.spec.weapons[wi].kind;
            if kind == WeaponKind::Orbit {
                continue; // the blades are always out: see `step_blades`
            }
            self.weapon_cd[wi] -= dt;
            if self.weapon_cd[wi] > 0.0 {
                continue;
            }
            let st = self.weapon_stats[wi];
            let next = (st.cooldown * self.cd_scale()).max(0.08);
            match kind {
                WeaponKind::Pulse => {
                    if self
                        .grid
                        .nearest(&self.foes.x, &self.foes.z, self.hx, self.hz, st.area)
                        .is_none()
                    {
                        self.weapon_cd[wi] = 0.1;
                        continue;
                    }
                    self.weapon_cd[wi] = next;
                    self.cast_pulse(wi, st);
                }
                WeaponKind::Chain => {
                    let Some((base, dist)) = self.pick_aim(st.range) else {
                        self.weapon_cd[wi] = 0.1;
                        continue;
                    };
                    if !dist.is_finite() {
                        self.weapon_cd[wi] = 0.1; // a ground aim with nothing there: no thread
                        continue;
                    }
                    self.weapon_cd[wi] = next;
                    self.face = base;
                    self.aim_hold = AIM_HOLD;
                    self.cast_chain(wi, st, base, dist);
                }
                _ => {
                    let Some((base, dist)) = self.pick_aim(st.range) else {
                        self.weapon_cd[wi] = 0.1; // nothing to aim at: look again soon
                        continue;
                    };
                    self.weapon_cd[wi] = next;
                    self.cast_shots(wi, st, kind, base, dist);
                }
            }
        }
    }

    /// Bolts, pellets and wisps: shots from the muzzle along `base`.
    fn cast_shots(&mut self, wi: usize, st: WeaponStats, kind: WeaponKind, base: f32, dist: f32) {
        // Turn to the target first: the blaster points where the shot goes, and the
        // shot leaves from the muzzle (nearer if the foe is already in the way).
        self.face = base;
        self.aim_hold = AIM_HOLD;
        let reach = MUZZLE.min(dist * 0.6);
        let n = st.count;
        let (skind, jitter) = match kind {
            WeaponKind::Spread => (1_u8, 0.05_f32),
            WeaponKind::Missile => (2, 0.0),
            _ => (0, 0.0),
        };
        for k in 0..n {
            if self.shots.len() >= self.spec.caps.shots as usize {
                break;
            }
            let mut a = base + (k as f32 - (n - 1) as f32 * 0.5) * st.spread;
            let mut speed = st.speed;
            if jitter > 0.0 {
                a += self.rng.range(-jitter, jitter);
                speed *= self.rng.range(0.92, 1.08);
            }
            self.shots.push(Shot {
                x: self.hx + a.sin() * reach,
                z: self.hz + a.cos() * reach,
                vx: a.sin() * speed,
                vz: a.cos() * speed,
                life: st.range / st.speed,
                dmg: st.damage * self.mods.damage,
                pierce: st.pierce,
                r: st.size,
                hits: [0; 4],
                n: 0,
                kind: skind,
                w: wi as u8,
                area: st.area,
            });
        }
        let cue = match kind {
            WeaponKind::Missile => "wisp",
            WeaponKind::Spread => "petal",
            _ => "shoot",
        };
        self.cue(cue, 0, 0.12);
        let (sn, cs) = base.sin_cos();
        let at = [self.hx + sn * reach, MUZZLE_Y, self.hz + cs * reach];
        self.burst(FX_MUZZLE, 1, at, [0.0, 0.0], 0.0, 0.0, 0.0, 0.09);
        self.burst(FX_SPARK, 2, at, [sn, cs], 5.0, 0.5, 0.5, 0.18);
    }

    /// Lightning: the foe nearest the aim point, then each next-nearest unhit foe within a jump.
    fn cast_chain(&mut self, wi: usize, st: WeaponStats, base: f32, dist: f32) {
        let (sn, cs) = base.sin_cos();
        let (px, pz) = (self.hx + sn * dist, self.hz + cs * dist);
        let first = self
            .grid
            .nearest(&self.foes.x, &self.foes.z, px, pz, 1.6)
            .map(|(j, _)| j);
        let Some(mut cur) = first else {
            return;
        };
        let mut zap = Zap {
            pts: [[0.0; 2]; ZAP_PTS],
            n: 1,
            age: 0.0,
            w: wi as u8,
        };
        zap.pts[0] = [self.hx + sn * 0.7, self.hz + cs * 0.7];
        let mut seen = [u32::MAX; ZAP_PTS - 1];
        let mut dmg = st.damage * self.mods.damage;
        let jumps = (st.count as usize).clamp(1, ZAP_PTS - 1);
        for hop in 0..jumps {
            let uid = self.foes.uid[cur];
            seen[hop] = uid;
            let (fx, fz) = (self.foes.x[cur], self.foes.z[cur]);
            zap.pts[zap.n as usize] = [fx, fz];
            zap.n += 1;
            let (dx, dz) = (fx - self.hx, fz - self.hz);
            let l = dx.hypot(dz).max(1e-3);
            self.damage_foe(cur, dmg, (-dx / l, -dz / l), 0.1, true);
            self.burst(FX_ZAP, 5, [fx, 0.7, fz], [0.0, 0.0], 4.0, 1.0, 2.0, 0.3);
            dmg *= 0.88;
            // The nearest foe not yet struck, within one jump.
            let mut best: Option<(usize, f32)> = None;
            let hops = &seen[..=hop];
            let (foes, kinds) = (&self.foes, &self.kinds);
            let reach = st.area;
            self.grid.query(fx, fz, reach + MAX_FOE_R, |j| {
                if hops.contains(&foes.uid[j]) || foes.hp[j] <= 0.0 {
                    return;
                }
                let (ex, ez) = (foes.x[j] - fx, foes.z[j] - fz);
                let d = ex.hypot(ez) - kinds[foes.kind[j] as usize].radius;
                if d < reach && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((j, d));
                }
            });
            match best {
                Some((j, _)) => cur = j,
                None => break,
            }
        }
        if self.zaps.len() >= ZAP_CAP {
            self.zaps.remove(0);
        }
        self.zaps.push(zap);
        self.cue("zap", 0, 0.1);
        self.core.juice.shake = self.core.juice.shake.max(0.1);
    }

    /// An expanding ring that shoves and hurts everything near the hero.
    fn cast_pulse(&mut self, wi: usize, st: WeaponStats) {
        let (hx, hz) = (self.hx, self.hz);
        self.foes_in(hx, hz, st.area);
        let dmg = st.damage * self.mods.damage;
        for n in 0..self.scratch.len() {
            let j = self.scratch[n];
            let (dx, dz) = (self.foes.x[j] - hx, self.foes.z[j] - hz);
            let l = dx.hypot(dz).max(1e-3);
            // Closer foes are thrown further.
            let shove = PULSE_SHOVE * (1.0 - 0.6 * (l / st.area).min(1.0));
            self.damage_foe(j, dmg, (-dx / l, -dz / l), shove, false);
        }
        let col = self.weapon_color(wi as u8);
        self.add_ring(hx, hz, st.area, 0.42, col);
        self.burst(
            FX_GLINT,
            10,
            [hx, 0.3, hz],
            [0.0, 0.0],
            st.area * 2.2,
            1.0,
            0.8,
            0.5,
        );
        self.cue("pulse", 0, 0.2);
        self.core.juice.shake = self.core.juice.shake.max(0.12);
    }

    /// A burst of `dmg` to every foe within `area` of (`x`,`z`): a wisp, or the bomb pickup.
    pub(super) fn explode(&mut self, x: f32, z: f32, dmg: f32, area: f32, color: [u8; 3]) {
        self.foes_in(x, z, area);
        for n in 0..self.scratch.len() {
            let j = self.scratch[n];
            let (dx, dz) = (self.foes.x[j] - x, self.foes.z[j] - z);
            let l = dx.hypot(dz).max(1e-3);
            self.damage_foe(j, dmg, (-dx / l, -dz / l), 0.35, false);
        }
        self.add_ring(x, z, area, 0.32, color);
        self.burst(
            FX_BLAST,
            8,
            [x, 0.5, z],
            [0.0, 0.0],
            area * 2.0,
            1.0,
            1.5,
            0.5,
        );
        self.burst(FX_SPARK, 6, [x, 0.5, z], [0.0, 0.0], 6.0, 1.0, 3.0, 0.4);
        self.cue("boom", 1, 0.08);
        self.core.juice.shake = self.core.juice.shake.max(0.15);
    }

    /// The blades: always circling while the weapon is held, hurting what they cross.
    pub(super) fn step_blades(&mut self, dt: f32) {
        for a in &mut self.aux {
            a.blade_cd = (a.blade_cd - dt).max(0.0);
        }
        let Some(wi) = self.orbit_weapon() else {
            return;
        };
        let st = self.weapon_stats[wi];
        let spin = if self.frenzy_t > 0.0 { 1.35 } else { 1.0 };
        self.orbit_phase = (self.orbit_phase + st.speed * spin * dt) % std::f32::consts::TAU;
        let n = st.count.clamp(1, BLADE_CAP as i32) as usize;
        let dmg = st.damage * self.mods.damage;
        let gap = (st.cooldown * self.cd_scale()).max(0.1);
        let mut cut = false;
        for k in 0..n {
            let (bx, bz) = self.blade_at(k, n, st.area);
            self.foes_in(bx, bz, st.size);
            for m in 0..self.scratch.len() {
                let j = self.scratch[m];
                if self.aux[j].blade_cd > 0.0 {
                    continue;
                }
                self.aux[j].blade_cd = gap;
                let (dx, dz) = (self.foes.x[j] - self.hx, self.foes.z[j] - self.hz);
                let l = dx.hypot(dz).max(1e-3);
                self.damage_foe(j, dmg, (-dx / l, -dz / l), 0.25, true);
                self.burst(
                    FX_SPARK,
                    3,
                    [bx, 0.6, bz],
                    [dx / l, dz / l],
                    4.5,
                    0.6,
                    1.5,
                    0.25,
                );
                cut = true;
            }
        }
        if cut {
            self.cue("slash", 0, 0.09);
        }
    }

    /// The held orbit weapon, if any.
    pub(super) fn orbit_weapon(&self) -> Option<usize> {
        (0..self.spec.weapons.len())
            .find(|&i| self.spec.weapons[i].kind == WeaponKind::Orbit && self.weapon_lv[i] > 0)
    }

    /// Where blade `k` of `n` is now.
    pub(super) fn blade_at(&self, k: usize, n: usize, radius: f32) -> (f32, f32) {
        let a = self.orbit_phase + k as f32 * std::f32::consts::TAU / n as f32;
        let r = radius * (1.0 + 0.05 * (self.anim_t * 3.0).sin());
        (self.hx + a.sin() * r, self.hz + a.cos() * r)
    }

    pub(super) fn move_shots(&mut self, dt: f32) {
        let lim = self.spec.arena.half + 1.0;
        let mut hit_cue = false;
        for si in (0..self.shots.len()).rev() {
            // Wisps curve toward the nearest foe within sight.
            if self.shots[si].kind == 2 {
                let (sx, sz) = (self.shots[si].x, self.shots[si].z);
                if let Some((j, _)) = self.grid.nearest(&self.foes.x, &self.foes.z, sx, sz, 7.0) {
                    let s = &mut self.shots[si];
                    let want = (self.foes.x[j] - sx).atan2(self.foes.z[j] - sz);
                    let cur = s.vx.atan2(s.vz);
                    let mut d = want - cur;
                    while d > std::f32::consts::PI {
                        d -= std::f32::consts::TAU;
                    }
                    while d < -std::f32::consts::PI {
                        d += std::f32::consts::TAU;
                    }
                    let turn = d.clamp(-5.5 * dt, 5.5 * dt);
                    let sp = s.vx.hypot(s.vz);
                    s.vx = (cur + turn).sin() * sp;
                    s.vz = (cur + turn).cos() * sp;
                }
            }
            let s = &mut self.shots[si];
            s.x += s.vx * dt;
            s.z += s.vz * dt;
            s.life -= dt;
            if s.life <= 0.0 || s.x.abs() > lim || s.z.abs() > lim {
                let (kind, x, z, dmg, area, w) = (s.kind, s.x, s.z, s.dmg, s.area, s.w);
                self.shots.swap_remove(si);
                if kind == 2 {
                    let col = self.weapon_color(w);
                    self.explode(x.clamp(-lim, lim), z.clamp(-lim, lim), dmg, area, col);
                }
                continue;
            }
            let (sx, sz, sr) = (s.x, s.z, s.r);
            let (hits, nh) = (s.hits, s.n);
            let mut found: Option<usize> = None;
            let (foes, kinds) = (&self.foes, &self.kinds);
            self.grid.query(sx, sz, sr + MAX_FOE_R, |j| {
                if found.is_some() {
                    return;
                }
                let rs = sr + kinds[foes.kind[j] as usize].radius;
                let (dx, dz) = (foes.x[j] - sx, foes.z[j] - sz);
                if dx * dx + dz * dz < rs * rs && !hits[..nh as usize].contains(&foes.uid[j]) {
                    found = Some(j);
                }
            });
            if let Some(j) = found {
                let s = self.shots[si];
                if s.kind == 2 {
                    // A wisp bursts on the first foe it meets.
                    self.shots.swap_remove(si);
                    let col = self.weapon_color(s.w);
                    self.explode(s.x, s.z, s.dmg, s.area, col);
                    continue;
                }
                let k = 1.0 / s.vx.hypot(s.vz).max(1e-3);
                let (dirx, dirz) = (-s.vx * k, -s.vz * k);
                let kb = self.spec.fx.knockback;
                self.damage_foe(j, s.dmg, (dirx, dirz), kb, true);
                hit_cue = true;
                self.burst(FX_SPARK, 4, [sx, 0.6, sz], [dirx, dirz], 5.5, 0.6, 1.8, 0.3);
                let s = &mut self.shots[si];
                if (s.n as usize) < s.hits.len() {
                    s.hits[s.n as usize] = self.foes.uid[j];
                    s.n += 1;
                }
                s.pierce -= 1;
                if s.pierce < 0 {
                    self.shots.swap_remove(si);
                }
            }
        }
        if hit_cue {
            self.cue("hit", 1, 0.07);
        }
    }

    /// Enemy orbs: fly, and hurt the hero on touch.
    pub(super) fn step_orbs(&mut self, dt: f32) {
        let lim = self.spec.arena.half + 1.0;
        let reach = self.spec.hero.radius + ORB_R;
        for i in (0..self.orbs.len()).rev() {
            let o = &mut self.orbs[i];
            o.x += o.vx * dt;
            o.z += o.vz * dt;
            o.life -= dt;
            let (x, z, dmg) = (o.x, o.z, o.dmg);
            if o.life <= 0.0 || x.abs() > lim || z.abs() > lim {
                self.orbs.swap_remove(i);
                continue;
            }
            if (x - self.hx).hypot(z - self.hz) < reach {
                self.orbs.swap_remove(i);
                self.hurt(dmg);
                self.burst(FX_BLAST, 3, [x, 0.5, z], [0.0, 0.0], 2.5, 1.0, 1.2, 0.3);
            }
        }
    }

    /// Age the zaps and rings.
    pub(super) fn step_vfx(&mut self, dt: f32) {
        for i in (0..self.zaps.len()).rev() {
            self.zaps[i].age += dt;
            if self.zaps[i].age >= ZAP_LIFE {
                self.zaps.swap_remove(i);
            }
        }
        for i in (0..self.rings.len()).rev() {
            self.rings[i].age += dt;
            if self.rings[i].age >= self.rings[i].life {
                self.rings.swap_remove(i);
            }
        }
    }
}

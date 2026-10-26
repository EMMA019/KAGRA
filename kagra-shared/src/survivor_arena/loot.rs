//! Drops, pickups, chests, power-ups and the weapon bar of the survivor arena. A child module
//! of `survivor_arena`. Pickups live in a pool capped by `drops.cap`; rolling a table and
//! opening a chest use the run's own dice, so a seed always drops the same things.

use super::*;

/// Pickups are taken inside this many metres (plus the hero's radius).
const TAKE_R: f32 = 0.55;
/// Chests lie around this many times longer than other pickups.
const CHEST_LIFE_MUL: f32 = 4.0;
/// A chest is pulled in from this far (m) and opens from this many times the usual reach.
const CHEST_PULL_M: f32 = 7.0;
const CHEST_TAKE_MUL: f32 = 2.4;
/// XP a swap-and-keep or a maxed-out weapon pickup gives (before Growth).
const SPARE_XP: f32 = 4.0;

impl SurvivorArena {
    /// A foe died at (`x`,`z`): maybe leave a pickup (or, for an elite, a chest).
    pub(super) fn drop_loot(&mut self, x: f32, z: f32, k: &Kind) {
        let d = &self.spec.drops;
        if !d.enabled || k.table == NO_TABLE {
            return;
        }
        let t = &d.tables[k.table as usize];
        let chance = (t.chance * k.drop_mul).clamp(0.0, 1.0);
        let pity = d.pity_kills;
        let (chest, rarity_min, table) = (t.chest, t.rarity_min, k.table);
        let cap = d.cap as usize;
        let roll = self.rng.unit(); // always drawn: the dice stay in step
        self.since_drop += 1;
        let hit = roll < chance || (pity > 0 && chance < 1.0 && self.since_drop >= pity);
        if !hit || self.pickups.len() >= cap {
            return;
        }
        self.since_drop = 0;
        let item = if chest {
            CHEST
        } else {
            match self.draw_item(rarity_min) {
                Some(i) => i as u8,
                None => return,
            }
        };
        // Land inside the arena, a little off the corpse so a pack does not stack.
        let half = self.spec.arena.half - 0.7;
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        self.pickups.push(Pickup {
            x: (x + a.sin() * 0.3).clamp(-half, half),
            z: (z + a.cos() * 0.3).clamp(-half, half),
            item,
            table,
            age: 0.0,
            fly: false,
        });
    }

    /// One item by weight among those of at least `rarity_min`.
    fn draw_item(&mut self, rarity_min: u8) -> Option<usize> {
        let mut ws = [0_u32; 16];
        let mut ids = [0_usize; 16];
        let mut m = 0;
        for (i, it) in self.spec.drops.items.iter().enumerate() {
            if it.rarity >= rarity_min && it.weight > 0 && m < 16 {
                ws[m] = it.weight;
                ids[m] = i;
                m += 1;
            }
        }
        if m == 0 {
            return None;
        }
        Some(ids[weighted_pick(&ws[..m], self.rng.next_u32())])
    }

    /// Move, age and collect the pickups.
    pub(super) fn collect_loot(&mut self, dt: f32) {
        let d = &self.spec.drops;
        let (life, pull) = (d.life, d.pull_m * self.mods.magnet);
        let take = self.spec.hero.radius + TAKE_R;
        for pi in (0..self.pickups.len()).rev() {
            let p = &mut self.pickups[pi];
            p.age += dt;
            let span = if p.item == CHEST {
                life * CHEST_LIFE_MUL
            } else {
                life
            };
            if p.age >= span {
                self.pickups.swap_remove(pi);
                continue;
            }
            let (dx, dz) = (self.hx - p.x, self.hz - p.z);
            let dist = dx.hypot(dz);
            let chest = p.item == CHEST;
            // A chest is the prize of a hard fight: it always drifts to the hero from a good
            // way off, whatever the magnet level.
            if dist < pull || self.pull_all_t > 0.0 || (chest && dist < CHEST_PULL_M) {
                p.fly = true;
            }
            if p.fly {
                let sp = (7.0 + 14.0 * (1.0 - (dist / (pull + 5.0)).min(1.0))) * dt;
                let k = sp.min(dist) / dist.max(1e-3);
                p.x += dx * k;
                p.z += dz * k;
            }
            if dist < if chest { take * CHEST_TAKE_MUL } else { take } {
                let p = self.pickups.swap_remove(pi);
                self.take_pickup(p);
            }
        }
    }

    fn take_pickup(&mut self, p: Pickup) {
        let col;
        let label;
        if p.item == CHEST {
            col = [255_u8, 215, 90];
            label = self.open_chest(p.table as usize);
            self.cue("chest", 2, 0.3);
            self.burst(
                FX_GLINT,
                18,
                [p.x, 0.5, p.z],
                [0.0, 0.0],
                4.5,
                1.0,
                3.5,
                0.8,
            );
            self.core.juice.shake = self.core.juice.shake.max(0.2);
        } else {
            let row = &self.spec.drops.items[p.item as usize];
            col = row.color.map(|c| c.min(255) as u8);
            label = self.apply_item(p.item as usize);
            self.cue("pickup", 2, 0.12);
            self.burst(
                FX_GLINT,
                8,
                [p.x, 0.4, p.z],
                [0.0, 0.0],
                3.0,
                1.0,
                3.0,
                0.55,
            );
        }
        if !label.is_empty() {
            self.set_toast(&label, col);
        }
        self.zoom_kick = self.zoom_kick.max(0.25);
    }

    /// Open a chest: `rolls` items from its table, the first the table's guarantee if it has one.
    pub(super) fn open_chest(&mut self, table: usize) -> String {
        let Some(t) = self.spec.drops.tables.get(table) else {
            return String::new();
        };
        let (rolls, rmin) = (t.rolls.max(1), t.rarity_min);
        let guarantee = self
            .spec
            .drops
            .items
            .iter()
            .position(|i| !t.guarantee.is_empty() && i.id == t.guarantee);
        let head = self.tr("CHEST", "たからばこ");
        let mut out = String::from(head);
        for r in 0..rolls {
            let item = if r == 0 && guarantee.is_some() {
                guarantee
            } else {
                self.draw_item(rmin)
            };
            if let Some(i) = item {
                let label = self.apply_item(i);
                if !label.is_empty() {
                    out.push_str("  ");
                    out.push_str(&label);
                }
            }
        }
        out
    }

    /// Do what a pickup does. Returns the line for the toast.
    pub(super) fn apply_item(&mut self, i: usize) -> String {
        let row = self.spec.drops.items[i].clone();
        let name = row.name.get(self.lang_ja).to_string();
        match row.kind {
            PickupKind::Heal => {
                self.hp = (self.hp + self.mods.hp_max * row.amount).min(self.mods.hp_max);
                name
            }
            PickupKind::Magnet => {
                for g in &mut self.gems {
                    g.fly = true;
                }
                self.pull_all_t = 1.4;
                name
            }
            PickupKind::Xp => {
                self.xp += row.amount * self.mods.xp;
                name
            }
            PickupKind::Bomb => {
                let col = row.color.map(|c| c.min(255) as u8);
                self.explode(self.hx, self.hz, row.amount, row.area, col);
                self.core.juice.shake = self.core.juice.shake.max(0.5);
                self.zoom_kick = 1.0;
                name
            }
            PickupKind::Shield => {
                self.shield_t = row.secs;
                name
            }
            PickupKind::Frenzy => {
                self.frenzy_t = row.secs;
                self.frenzy_amt = row.amount.clamp(0.0, 0.8);
                name
            }
            PickupKind::Weapon => self.grant_weapon(),
        }
    }

    /// A weapon pickup: raise one you hold, fill a free slot, or (slots full) offer a swap.
    pub(super) fn grant_weapon(&mut self) -> String {
        let free = self.held_weapons() < self.spec.levels.max_weapons;
        let mut ws = [0_u32; 8];
        let mut ids = [0_usize; 8];
        let mut m = 0;
        for (i, w) in self.spec.weapons.iter().enumerate().take(8) {
            let cur = self.weapon_lv[i] as usize;
            if cur > w.levels.len() {
                continue; // maxed
            }
            // Anything you hold or can still hold is likelier than a weapon with no slot.
            ws[m] = if cur > 0 || free { 3 } else { 1 };
            ids[m] = i;
            m += 1;
        }
        if m == 0 {
            self.xp += SPARE_XP * self.mods.xp;
            return self.tr("MAX! +XP", "さいだい！ +XP").to_string();
        }
        let wi = ids[weighted_pick(&ws[..m], self.rng.next_u32())];
        let name = self.spec.weapons[wi].name.get(self.lang_ja).to_string();
        let cur = self.weapon_lv[wi];
        if cur > 0 {
            self.weapon_lv[wi] += 1;
            self.recompute();
            return format!("{name} LV {}", cur + 1);
        }
        if free {
            self.weapon_lv[wi] = 1;
            self.weapon_cd[wi] = 0.0;
            self.recompute();
            return format!("{} {name}", self.tr("NEW", "あたらしい"));
        }
        if !self.cards.is_empty() {
            self.xp += SPARE_XP * self.mods.xp; // a second offer while one is open
            return String::new();
        }
        self.open_swap(wi);
        format!("{name} ?")
    }

    /// The slots are full: up to three of the lowest-level weapons to drop for `new`, or keep.
    fn open_swap(&mut self, new: usize) {
        self.cards.clear();
        self.swap_in = Some(new as u8);
        let mut taken = [false; 8];
        for _ in 0..3 {
            let mut best: Option<usize> = None;
            for (i, done) in taken.iter().enumerate().take(self.spec.weapons.len()) {
                if self.weapon_lv[i] == 0 || *done {
                    continue;
                }
                if best.is_none_or(|b| self.weapon_lv[i] < self.weapon_lv[b]) {
                    best = Some(i);
                }
            }
            let Some(b) = best else {
                break;
            };
            taken[b] = true;
            self.cards.push(Card::Swap(b as u8));
        }
        self.cards.push(Card::Keep);
        self.cue("levelup", 2, 0.2);
    }
}

//! The rules of Moonlit Tactics, with no picture and no input: the board, the units, moves,
//! attacks with counters, healing, and how the dead decide what to do.
//!
//! Terrain (one character per cell in a map's `rows`):
//! `.` grass, `T` tree (costs 2 to enter, +2 defence), `#` grave (blocks walkers, ghosts fly
//! over), `C` column (blocks everything), `~` mist (costs 2, nothing else).

use serde::{Deserialize, Serialize};

use crate::game_core::grid::{self, Cell};

/// A kind of unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClassSpec {
    pub id: String,
    pub name: String,
    pub name_ja: String,
    /// A GLB under the bundle; a missing one draws as a box.
    pub model: String,
    pub scale: f32,
    pub hp: i32,
    pub atk: i32,
    pub def: i32,
    pub mv: i32,
    /// Attack range in steps (`1..=1` is melee, `2..=3` a bow).
    pub range_min: i32,
    pub range_max: i32,
    /// Hit points healed on a friend in range instead of attacking (0 = cannot heal).
    pub heal: i32,
    /// Flies over graves and pays 1 for trees and mist.
    pub flies: bool,
}

impl Default for ClassSpec {
    fn default() -> Self {
        Self {
            id: "knight".into(),
            name: "Knight".into(),
            name_ja: "騎士".into(),
            model: String::new(),
            scale: 1.6,
            hp: 20,
            atk: 7,
            def: 3,
            mv: 4,
            range_min: 1,
            range_max: 1,
            heal: 0,
            flies: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Hero,
    Foe,
}

/// A unit as a map places it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placed {
    pub class: String,
    pub side: Side,
    pub x: i32,
    pub y: i32,
    /// The map's leader: beating it wins the map at once (foes), losing it loses (heroes).
    #[serde(default)]
    pub leader: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MapSpec {
    pub name: String,
    pub name_ja: String,
    pub story: String,
    pub story_ja: String,
    /// One string per row, top (far) first; all the same length.
    pub rows: Vec<String>,
    pub units: Vec<Placed>,
    /// Clearing in this many turns or fewer earns the turn bonus.
    pub par_turns: u32,
}

impl Default for MapSpec {
    fn default() -> Self {
        Self {
            name: "Map".into(),
            name_ja: String::new(),
            story: String::new(),
            story_ja: String::new(),
            rows: vec![".....".into(); 5],
            units: Vec::new(),
            par_turns: 10,
        }
    }
}

impl MapSpec {
    pub fn size(&self) -> (i32, i32) {
        (
            self.rows.first().map_or(0, |r| r.chars().count()) as i32,
            self.rows.len() as i32,
        )
    }

    pub fn tile(&self, c: Cell) -> char {
        self.rows
            .get(c.1 as usize)
            .and_then(|r| r.chars().nth(c.0 as usize))
            .unwrap_or('C')
    }

    pub fn validate(&self, at: &str, classes: &[ClassSpec]) -> Result<(), String> {
        let (w, h) = self.size();
        if !(1..=16).contains(&w) || !(1..=16).contains(&h) {
            return Err(format!("{at}: the map must be 1..=16 cells each way"));
        }
        for (i, r) in self.rows.iter().enumerate() {
            if r.chars().count() as i32 != w {
                return Err(format!("{at}.rows[{i}] is not {w} cells long"));
            }
            if let Some(bad) = r.chars().find(|c| !".T#C~".contains(*c)) {
                return Err(format!("{at}.rows[{i}]: unknown tile {bad:?}"));
            }
        }
        let mut seen = Vec::new();
        for (i, u) in self.units.iter().enumerate() {
            if !classes.iter().any(|c| c.id == u.class) {
                return Err(format!("{at}.units[{i}]: unknown class {:?}", u.class));
            }
            if u.x < 0 || u.y < 0 || u.x >= w || u.y >= h {
                return Err(format!("{at}.units[{i}] is off the map"));
            }
            if matches!(self.tile((u.x, u.y)), '#' | 'C') {
                return Err(format!("{at}.units[{i}] stands on a wall"));
            }
            if seen.contains(&(u.x, u.y)) {
                return Err(format!("{at}.units[{i}] shares a cell"));
            }
            seen.push((u.x, u.y));
        }
        for side in [Side::Hero, Side::Foe] {
            if !self.units.iter().any(|u| u.side == side) {
                return Err(format!("{at} needs heroes and foes"));
            }
        }
        Ok(())
    }
}

/// A unit on the board.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub class: usize,
    pub side: Side,
    pub at: Cell,
    pub hp: i32,
    pub moved: bool,
    pub acted: bool,
    pub leader: bool,
}

impl Unit {
    pub fn alive(&self) -> bool {
        self.hp > 0
    }
}

/// What one exchange did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Exchange {
    pub dealt: i32,
    pub killed: bool,
    /// The counter, if the defender could hit back.
    pub countered: Option<i32>,
    pub attacker_died: bool,
}

#[derive(Clone, Debug)]
pub struct Board {
    pub map: MapSpec,
    pub classes: Vec<ClassSpec>,
    pub units: Vec<Unit>,
}

impl Board {
    pub fn new(map: &MapSpec, classes: &[ClassSpec]) -> Self {
        let units = map
            .units
            .iter()
            .map(|p| {
                let class = classes.iter().position(|c| c.id == p.class).unwrap_or(0);
                Unit {
                    class,
                    side: p.side,
                    at: (p.x, p.y),
                    hp: classes[class].hp,
                    moved: false,
                    acted: false,
                    leader: p.leader,
                }
            })
            .collect();
        Self {
            map: map.clone(),
            classes: classes.to_vec(),
            units,
        }
    }

    pub fn size(&self) -> (i32, i32) {
        self.map.size()
    }

    pub fn class(&self, u: usize) -> &ClassSpec {
        &self.classes[self.units[u].class]
    }

    /// The living unit on a cell.
    pub fn at(&self, c: Cell) -> Option<usize> {
        self.units.iter().position(|u| u.alive() && u.at == c)
    }

    /// Defence the terrain under a cell adds.
    pub fn cover(&self, c: Cell) -> i32 {
        if self.map.tile(c) == 'T' {
            2
        } else {
            0
        }
    }

    /// The cost for unit `u` to enter `c`, or `None` when it cannot (walls, foes).
    pub fn enter_cost(&self, u: usize, c: Cell) -> Option<i32> {
        let flies = self.class(u).flies;
        let base = match self.map.tile(c) {
            'C' => return None,
            '#' if !flies => return None,
            'T' | '~' if !flies => 2,
            _ => 1,
        };
        match self.at(c) {
            Some(o) if self.units[o].side != self.units[u].side => None,
            _ => Some(base),
        }
    }

    /// Where unit `u` can end its move now, with the cost.
    pub fn reach(&self, u: usize) -> Vec<(Cell, i32)> {
        let (w, h) = self.size();
        let me = &self.units[u];
        grid::reach(
            w,
            h,
            me.at,
            self.class(u).mv,
            |c| self.enter_cost(u, c),
            |c| self.at(c).is_some_and(|o| o != u),
        )
    }

    /// The foes (or, for a healer, the hurt friends) unit `u` could act on from cell `from`.
    pub fn targets_from(&self, u: usize, from: Cell) -> Vec<usize> {
        let k = self.class(u);
        let me = &self.units[u];
        let mut out = Vec::new();
        for (i, o) in self.units.iter().enumerate() {
            if i == u || !o.alive() {
                continue;
            }
            let hostile = o.side != me.side;
            let ok = if k.heal > 0 {
                !hostile && o.hp < self.classes[o.class].hp && grid::in_range(from, o.at, 1, 1)
            } else {
                hostile && grid::in_range(from, o.at, k.range_min, k.range_max)
            };
            if ok {
                out.push(i);
            }
        }
        out
    }

    /// Damage `a` would deal to `d` (the defender on its own cell).
    pub fn damage(&self, a: usize, d: usize) -> i32 {
        let att = self.class(a).atk;
        let def = self.class(d).def + self.cover(self.units[d].at);
        (att - def).max(1)
    }

    /// What `a` attacking `d` from cell `from` would do, without doing it.
    pub fn forecast(&self, a: usize, d: usize, from: Cell) -> Exchange {
        let dealt = self.damage(a, d).min(self.units[d].hp);
        let killed = dealt >= self.units[d].hp;
        let dk = self.class(d);
        let countered = (!killed
            && dk.heal == 0
            && grid::in_range(self.units[d].at, from, dk.range_min, dk.range_max))
        .then(|| {
            let att = dk.atk;
            let def = self.class(a).def + self.cover(from);
            (att - def).max(1).min(self.units[a].hp)
        });
        Exchange {
            dealt,
            killed,
            countered,
            attacker_died: countered.is_some_and(|c| c >= self.units[a].hp),
        }
    }

    /// Unit `a` (already on its cell) attacks `d`; the defender counters if it can.
    pub fn attack(&mut self, a: usize, d: usize) -> Exchange {
        let ex = self.forecast(a, d, self.units[a].at);
        self.units[d].hp -= ex.dealt;
        if let Some(c) = ex.countered {
            self.units[a].hp -= c;
        }
        self.units[a].acted = true;
        self.units[a].moved = true;
        ex
    }

    /// Unit `a` heals friend `d`; returns the hit points restored.
    pub fn heal(&mut self, a: usize, d: usize) -> i32 {
        let max = self.classes[self.units[d].class].hp;
        let amount = self.class(a).heal.min(max - self.units[d].hp).max(0);
        self.units[d].hp += amount;
        self.units[a].acted = true;
        self.units[a].moved = true;
        amount
    }

    pub fn living(&self, side: Side) -> impl Iterator<Item = usize> + '_ {
        self.units
            .iter()
            .enumerate()
            .filter(move |(_, u)| u.alive() && u.side == side)
            .map(|(i, _)| i)
    }

    /// `Some(true)` won (no foes, or the foe leader is down), `Some(false)` lost (no heroes,
    /// or the hero leader is down), `None` still on.
    pub fn outcome(&self) -> Option<bool> {
        let gone = |side: Side| {
            self.living(side).next().is_none()
                || self
                    .units
                    .iter()
                    .any(|u| u.side == side && u.leader && !u.alive())
        };
        if gone(Side::Foe) {
            Some(true)
        } else if gone(Side::Hero) {
            Some(false)
        } else {
            None
        }
    }

    /// A new turn for one side: everyone may move and act again.
    pub fn refresh(&mut self, side: Side) {
        for u in &mut self.units {
            if u.side == side {
                u.moved = false;
                u.acted = false;
            }
        }
    }

    /// What foe `u` will do this turn: the cell to move to and the hero to attack, if any.
    /// It takes the attack that hurts most (a kill first, a healer next, then damage, minus
    /// the counter it would take); with nothing in reach it walks toward the nearest hero.
    pub fn plan(&self, u: usize) -> (Cell, Option<usize>) {
        let here = self.units[u].at;
        let mut best: Option<(i32, Cell, usize)> = None;
        for (cell, cost) in self.reach(u) {
            for t in self.targets_from(u, cell) {
                let ex = self.forecast(u, t, cell);
                let mut score = ex.dealt * 10 - ex.countered.unwrap_or(0) * 6 - cost;
                if ex.killed {
                    score += 200;
                }
                if self.class(t).heal > 0 {
                    score += 15;
                }
                if self.units[t].leader {
                    score += 25;
                }
                if best.is_none_or(|(b, _, _)| score > b) {
                    best = Some((score, cell, t));
                }
            }
        }
        if let Some((_, cell, t)) = best {
            return (cell, Some(t));
        }
        // Walk toward the nearest hero, as far as this turn allows.
        let (w, h) = self.size();
        let reach = self.reach(u);
        let mut goal: Option<(usize, Vec<Cell>)> = None;
        for t in self.living(Side::Hero) {
            let to = self.units[t].at;
            let p = grid::path(w, h, here, to, |c| {
                // Plan through other foes (they will move); only terrain and heroes block.
                let base = self.enter_cost(u, c);
                match self.at(c) {
                    Some(o) if self.units[o].side == self.units[u].side => Some(base.unwrap_or(1)),
                    _ => base,
                }
            });
            if let Some(p) = p {
                if goal.as_ref().is_none_or(|(_, g)| p.len() < g.len()) {
                    goal = Some((t, p));
                }
            }
        }
        let Some((_, path)) = goal else {
            return (here, None);
        };
        let dest = path
            .iter()
            .rev()
            .find(|c| reach.iter().any(|(r, _)| r == *c))
            .copied()
            .unwrap_or(here);
        (dest, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classes() -> Vec<ClassSpec> {
        vec![
            ClassSpec::default(),
            ClassSpec {
                id: "archer".into(),
                hp: 14,
                atk: 6,
                def: 1,
                range_min: 2,
                range_max: 3,
                ..ClassSpec::default()
            },
            ClassSpec {
                id: "cleric".into(),
                hp: 12,
                atk: 2,
                def: 1,
                heal: 8,
                ..ClassSpec::default()
            },
            ClassSpec {
                id: "zombie".into(),
                hp: 14,
                atk: 6,
                def: 2,
                mv: 3,
                ..ClassSpec::default()
            },
            ClassSpec {
                id: "ghost".into(),
                hp: 8,
                atk: 5,
                def: 0,
                mv: 5,
                flies: true,
                ..ClassSpec::default()
            },
        ]
    }

    fn map(rows: &[&str], units: &[(&str, Side, i32, i32)]) -> Board {
        let m = MapSpec {
            rows: rows.iter().map(|r| r.to_string()).collect(),
            units: units
                .iter()
                .map(|(c, s, x, y)| Placed {
                    class: c.to_string(),
                    side: *s,
                    x: *x,
                    y: *y,
                    leader: false,
                })
                .collect(),
            ..MapSpec::default()
        };
        m.validate("map", &classes()).unwrap();
        Board::new(&m, &classes())
    }

    #[test]
    fn moves_follow_terrain_and_ghosts_fly() {
        let b = map(
            &[".....", ".#T..", ".....", ".....", "....."],
            &[("knight", Side::Hero, 1, 2), ("ghost", Side::Foe, 4, 4)],
        );
        let r = b.reach(0);
        let has = |c: Cell| r.iter().any(|(x, _)| *x == c);
        assert!(!has((1, 1)), "a grave blocks walkers");
        assert_eq!(
            r.iter().find(|(c, _)| *c == (2, 1)).unwrap().1,
            3,
            "a tree costs 2"
        );
        let g2 = map(
            &[".#...", "....."],
            &[("ghost", Side::Foe, 0, 0), ("knight", Side::Hero, 4, 1)],
        );
        assert!(
            g2.reach(0).iter().any(|(c, _)| *c == (1, 0)),
            "over the grave"
        );
    }

    #[test]
    fn attacks_counter_in_range_and_trees_shelter() {
        let mut b = map(
            &[".....", ".....", "....."],
            &[
                ("knight", Side::Hero, 0, 0),
                ("zombie", Side::Foe, 1, 0),
                ("archer", Side::Hero, 3, 0),
            ],
        );
        let f = b.forecast(0, 1, (0, 0));
        assert_eq!(f.dealt, 7 - 2);
        assert_eq!(
            f.countered,
            Some(6 - 3),
            "the zombie hits back at melee range"
        );
        let f = b.forecast(2, 1, (3, 0));
        assert_eq!(
            f.countered, None,
            "nothing hits back at an archer two steps away"
        );
        let ex = b.attack(0, 1);
        assert_eq!((b.units[1].hp, b.units[0].hp), (14 - 5, 20 - 3));
        assert!(b.units[0].acted && !ex.killed);
        let t = map(
            &["T.", ".."],
            &[("zombie", Side::Foe, 1, 0), ("knight", Side::Hero, 0, 0)],
        );
        assert_eq!(t.damage(0, 1), 1, "a knight in a tree takes 6 - 3 - 2");
    }

    #[test]
    fn clerics_heal_hurt_friends_only() {
        let mut b = map(
            &["....."],
            &[
                ("cleric", Side::Hero, 0, 0),
                ("knight", Side::Hero, 1, 0),
                ("zombie", Side::Foe, 4, 0),
            ],
        );
        assert!(b.targets_from(0, (0, 0)).is_empty(), "nobody hurt");
        b.units[1].hp = 5;
        assert_eq!(b.targets_from(0, (0, 0)), vec![1]);
        assert_eq!(b.heal(0, 1), 8);
        assert_eq!(b.units[1].hp, 13);
    }

    #[test]
    fn the_dead_go_for_the_kill_or_walk_closer() {
        let mut b = map(
            &[".......", ".......", "......."],
            &[
                ("knight", Side::Hero, 0, 0),
                ("cleric", Side::Hero, 6, 2),
                ("zombie", Side::Foe, 3, 1),
            ],
        );
        b.units[1].hp = 3;
        let (cell, t) = b.plan(2);
        assert_eq!(t, Some(1), "the weak healer");
        assert!(grid::in_range(cell, (6, 2), 1, 1));
        let b = map(
            &["..........", ".........."],
            &[("knight", Side::Hero, 0, 0), ("zombie", Side::Foe, 9, 1)],
        );
        let (cell, t) = b.plan(1);
        assert!(t.is_none());
        assert_eq!(grid::manhattan(cell, (9, 1)), 3, "a full move closer");
        assert!(grid::manhattan(cell, (0, 0)) < grid::manhattan((9, 1), (0, 0)));
    }

    #[test]
    fn leaders_and_wipes_end_the_map() {
        let mut b = map(
            &["...."],
            &[("knight", Side::Hero, 0, 0), ("zombie", Side::Foe, 3, 0)],
        );
        assert_eq!(b.outcome(), None);
        b.units[1].hp = 0;
        assert_eq!(b.outcome(), Some(true));
        b.units[1].hp = 5;
        b.units[0].hp = 0;
        assert_eq!(b.outcome(), Some(false));
        let bad = MapSpec {
            rows: vec!["..#".into()],
            units: vec![Placed {
                class: "knight".into(),
                side: Side::Hero,
                x: 2,
                y: 0,
                leader: false,
            }],
            ..MapSpec::default()
        };
        assert!(bad.validate("m", &classes()).is_err());
    }
}

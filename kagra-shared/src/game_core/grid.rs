//! A square grid for tactics and puzzle games: where a unit can walk this turn, the path to a
//! cell, and which cells are in range. Cells are `(x, y)` with `0 <= x < w`, `0 <= y < h`;
//! steps are the four neighbours (no diagonals). First used by Moonlit Tactics.

use std::collections::VecDeque;

pub type Cell = (i32, i32);

/// The four neighbours, in a fixed order (so ties break the same way every run).
pub const STEPS: [Cell; 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// Grid steps between two cells.
pub fn manhattan(a: Cell, b: Cell) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Is `b` within `min..=max` steps of `a` (an attack or a spell range)?
pub fn in_range(a: Cell, b: Cell, min: i32, max: i32) -> bool {
    let d = manhattan(a, b);
    (min..=max).contains(&d)
}

/// The cells within `min..=max` steps of `from` that are on a `w` x `h` grid.
pub fn ring(from: Cell, min: i32, max: i32, w: i32, h: i32) -> Vec<Cell> {
    let mut out = Vec::new();
    for dy in -max..=max {
        for dx in -max..=max {
            let c = (from.0 + dx, from.1 + dy);
            if c.0 >= 0 && c.1 >= 0 && c.0 < w && c.1 < h && in_range(from, c, min, max) {
                out.push(c);
            }
        }
    }
    out
}

/// Every cell a unit at `from` can end its move on with `budget` move points, and what it
/// costs to get there. `cost(cell)` is the move cost of entering a cell, or `None` when it
/// cannot be entered at all (a wall, a foe). `stop(cell)` says a cell may be passed but not
/// ended on (a friend standing there). Cheapest costs win (a small Dijkstra, the grids are
/// tiny). The start is always included, at cost 0.
pub fn reach(
    w: i32,
    h: i32,
    from: Cell,
    budget: i32,
    cost: impl Fn(Cell) -> Option<i32>,
    stop: impl Fn(Cell) -> bool,
) -> Vec<(Cell, i32)> {
    let idx = |c: Cell| (c.1 * w + c.0) as usize;
    let mut best = vec![i32::MAX; (w * h).max(0) as usize];
    let mut queue = VecDeque::new();
    best[idx(from)] = 0;
    queue.push_back(from);
    while let Some(c) = queue.pop_front() {
        let here = best[idx(c)];
        for s in STEPS {
            let n = (c.0 + s.0, c.1 + s.1);
            if n.0 < 0 || n.1 < 0 || n.0 >= w || n.1 >= h {
                continue;
            }
            let Some(step) = cost(n) else {
                continue;
            };
            let total = here + step.max(1);
            if total <= budget && total < best[idx(n)] {
                best[idx(n)] = total;
                queue.push_back(n);
            }
        }
    }
    let mut out = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let c = (x, y);
            let b = best[idx(c)];
            if b != i32::MAX && (c == from || !stop(c)) {
                out.push((c, b));
            }
        }
    }
    out
}

/// The cheapest path from `from` to `to` (both included), or `None` when `to` cannot be
/// reached at all. No budget: use it to walk toward something far away.
pub fn path(
    w: i32,
    h: i32,
    from: Cell,
    to: Cell,
    cost: impl Fn(Cell) -> Option<i32>,
) -> Option<Vec<Cell>> {
    if from == to {
        return Some(vec![from]);
    }
    let idx = |c: Cell| (c.1 * w + c.0) as usize;
    let n = (w * h).max(0) as usize;
    let mut best = vec![i32::MAX; n];
    let mut prev: Vec<Option<Cell>> = vec![None; n];
    let mut queue = VecDeque::new();
    best[idx(from)] = 0;
    queue.push_back(from);
    while let Some(c) = queue.pop_front() {
        let here = best[idx(c)];
        for s in STEPS {
            let nb = (c.0 + s.0, c.1 + s.1);
            if nb.0 < 0 || nb.1 < 0 || nb.0 >= w || nb.1 >= h {
                continue;
            }
            // The goal may be occupied (walking up to a foe): it is always enterable.
            let step = if nb == to {
                cost(nb).unwrap_or(1)
            } else {
                match cost(nb) {
                    Some(s) => s,
                    None => continue,
                }
            };
            let total = here + step.max(1);
            if total < best[idx(nb)] {
                best[idx(nb)] = total;
                prev[idx(nb)] = Some(c);
                queue.push_back(nb);
            }
        }
    }
    if best[idx(to)] == i32::MAX {
        return None;
    }
    let mut out = vec![to];
    let mut c = to;
    while let Some(p) = prev[idx(c)] {
        out.push(p);
        c = p;
    }
    out.reverse();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 5 x 5 room with a wall down the middle except at the bottom.
    fn room(c: Cell) -> Option<i32> {
        if c.0 == 2 && c.1 < 4 {
            None
        } else {
            Some(1)
        }
    }

    #[test]
    fn reach_respects_the_budget_walls_and_costs() {
        let cells = reach(5, 5, (0, 0), 3, room, |_| false);
        let has = |c: Cell| cells.iter().any(|(x, _)| *x == c);
        assert!(has((0, 0)) && has((1, 2)) && has((0, 3)));
        assert!(!has((3, 0)), "the wall is in the way");
        assert!(!has((2, 2)), "a wall cell");
        assert!(!has((1, 3)), "four steps away");
        // Costly cells (a forest at (1, 0)) eat the budget.
        let forest = |c: Cell| if c == (1, 0) { Some(3) } else { room(c) };
        let cells = reach(5, 5, (0, 0), 3, forest, |_| false);
        assert_eq!(cells.iter().find(|(c, _)| *c == (1, 0)).unwrap().1, 3);
        // A friend's cell can be walked through but not ended on.
        let cells = reach(5, 5, (0, 0), 3, room, |c| c == (0, 1));
        assert!(!cells.iter().any(|(c, _)| *c == (0, 1)));
        assert!(
            cells.iter().any(|(c, _)| *c == (0, 2)),
            "through the friend"
        );
    }

    #[test]
    fn path_goes_round_walls_and_ranges_count_steps() {
        let p = path(5, 5, (0, 0), (4, 0), room).unwrap();
        assert_eq!(p.first(), Some(&(0, 0)));
        assert_eq!(p.last(), Some(&(4, 0)));
        assert!(p.contains(&(2, 4)), "round the bottom of the wall");
        assert_eq!(p.len() as i32 - 1, 12);
        assert!(
            path(5, 5, (0, 0), (2, 1), room).is_some(),
            "a goal on a wall cell is allowed"
        );
        let r = ring((2, 2), 2, 3, 5, 5);
        assert!(r.iter().all(|c| in_range((2, 2), *c, 2, 3)));
        assert!(!r.contains(&(2, 3)), "too close");
        assert!(r.contains(&(0, 1)) && r.contains(&(4, 3)) && !r.contains(&(0, 0)));
    }
}

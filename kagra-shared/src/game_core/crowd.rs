//! Where a crowd of people stands.
//!
//! One leader at the front, then rows that widen up to a cap so a hundred
//! survivors stay on a road. The kit instances a mesh at each slot. The real
//! head count can be larger than the number of bodies drawn: the number is the
//! rule, the slots are the picture. Picture only — a sim stores the count and
//! asks for offsets.

/// Offsets from the leader, in metres: `(side, back)`.
///
/// `side` is lateral (positive to the crowd's right). `back` is how far behind
/// the leader this body stands. `shown` bodies, `spacing` metres apart, at most
/// `max_across` in one row. The first slot is the leader at `(0, 0)`.
pub fn formation(shown: usize, spacing: f32, max_across: usize) -> Vec<(f32, f32)> {
    let spacing = spacing.max(0.2);
    let max_across = max_across.max(1);
    let mut out = Vec::with_capacity(shown);
    let mut row = 0usize;
    while out.len() < shown && row < 64 {
        let n = (row + 1).min(max_across).min(shown - out.len());
        let width = (n as f32 - 1.0) * spacing;
        let back = row as f32 * spacing * 0.92;
        for i in 0..n {
            let side = if n == 1 {
                0.0
            } else {
                -width * 0.5 + i as f32 * spacing
            };
            out.push((side, back));
        }
        row += 1;
    }
    out
}

/// Which pose frame a body is on. `frames` is the flipbook length (at least 1).
/// Neighbours with different `phase` (radians, any) do not step in lockstep.
pub fn flipbook(t: f32, phase: f32, frames: usize) -> usize {
    let frames = frames.max(1);
    let cycle = t * 2.15 + phase / std::f32::consts::TAU;
    (cycle.rem_euclid(1.0) * frames as f32) as usize % frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_leader_is_in_front_and_the_pack_stays_narrow() {
        let slots = formation(40, 0.75, 5);
        assert_eq!(slots.len(), 40);
        assert_eq!(slots[0], (0.0, 0.0));
        let max_side = slots.iter().map(|s| s.0.abs()).fold(0.0_f32, f32::max);
        assert!(max_side < 0.75 * 2.0 + 0.01, "{max_side}");
        let max_back = slots.iter().map(|s| s.1).fold(0.0_f32, f32::max);
        assert!(max_back > 1.0);
        // No two bodies in the same spot.
        for (i, a) in slots.iter().enumerate() {
            for b in slots.iter().skip(i + 1) {
                let d = (a.0 - b.0).hypot(a.1 - b.1);
                assert!(d > 0.3, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn flipbook_covers_every_frame_and_stays_in_range() {
        let mut seen = [false; 4];
        for step in 0..40 {
            let f = flipbook(step as f32 / 20.0, 0.0, 4);
            assert!(f < 4);
            seen[f] = true;
        }
        assert!(seen.iter().all(|s| *s));
        assert_eq!(flipbook(0.0, 0.0, 0), 0);
    }
}

//! Number gates for a crowd runner.
//!
//! A gate is one operation on the crowd's head count: multiply, add (which may
//! be negative) or integer divide. The kit places them in pairs so the player
//! steers the whole crowd through one. Nothing here knows about a road or a
//! picture.

/// One operation on a head count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `+10` or `-5`.
    Add(i32),
    /// `x2`. The factor is at least 2.
    Mul(i32),
    /// `/2`. Integer division, divisor at least 2.
    Div(i32),
}

impl Op {
    /// The count after the crowd runs through this gate. Never negative.
    pub fn apply(self, count: u32) -> u32 {
        let c = count as i32;
        let n = match self {
            Op::Add(v) => c.saturating_add(v),
            Op::Mul(v) => c.saturating_mul(v),
            Op::Div(v) => c / v.max(1),
        };
        n.max(0) as u32
    }

    /// Big readable label: `x2`, `+10`, `-5`, `/2`.
    pub fn label(self) -> String {
        match self {
            Op::Add(v) if v >= 0 => format!("+{v}"),
            Op::Add(v) => format!("{v}"),
            Op::Mul(v) => format!("x{v}"),
            Op::Div(v) => format!("/{v}"),
        }
    }

    /// Green when the operation usually grows a crowd, red when it usually shrinks one.
    /// A pair can still be tense when both are green (`+30` against `x2`) or both red.
    pub fn friendly(self) -> bool {
        match self {
            Op::Add(v) => v >= 0,
            Op::Mul(v) => v >= 2,
            Op::Div(_) => false,
        }
    }

    /// `x2`, `*2`, `×2`, `+10`, `-5`, `/2`, `÷2`. Anything else is `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        let op = if let Some(rest) = strip_mul(s) {
            Op::Mul(rest.parse().ok()?)
        } else if let Some(rest) = strip_div(s) {
            Op::Div(rest.parse().ok()?)
        } else if s.starts_with('+') || s.starts_with('-') {
            Op::Add(s.parse().ok()?)
        } else {
            return None;
        };
        match op {
            Op::Add(v) if (-99..=99).contains(&v) && v != 0 => Some(op),
            Op::Mul(v) if (2..=9).contains(&v) => Some(op),
            Op::Div(v) if (2..=9).contains(&v) => Some(op),
            _ => None,
        }
    }
}

fn strip_mul(s: &str) -> Option<&str> {
    ["x", "X", "*", "×"].iter().find_map(|p| s.strip_prefix(p))
}

fn strip_div(s: &str) -> Option<&str> {
    ["/", "÷"].iter().find_map(|p| s.strip_prefix(p))
}

/// Crowd against a wave. `Some(left)` when the crowd is strictly larger (those
/// people are gone). `None` when the wave wipes the crowd.
pub fn clash(crowd: u32, foes: u32) -> Option<u32> {
    if crowd > foes {
        Some(crowd - foes)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_math() {
        assert_eq!(Op::parse("x2"), Some(Op::Mul(2)));
        assert_eq!(Op::parse("×3"), Some(Op::Mul(3)));
        assert_eq!(Op::parse("+10"), Some(Op::Add(10)));
        assert_eq!(Op::parse("-5"), Some(Op::Add(-5)));
        assert_eq!(Op::parse("/2"), Some(Op::Div(2)));
        assert_eq!(Op::parse("÷2"), Some(Op::Div(2)));
        assert_eq!(Op::parse("x1"), None);
        assert_eq!(Op::parse("0"), None);
        assert_eq!(Op::parse("+10").unwrap().apply(8), 18);
        assert_eq!(Op::parse("-5").unwrap().apply(3), 0);
        assert_eq!(Op::parse("x2").unwrap().apply(8), 16);
        assert_eq!(Op::parse("/2").unwrap().apply(5), 2);
        assert_eq!(Op::parse("x2").unwrap().label(), "x2");
        assert_eq!(Op::parse("-5").unwrap().label(), "-5");
        assert!(Op::parse("+10").unwrap().friendly());
        assert!(!Op::parse("/2").unwrap().friendly());
        assert!(!Op::parse("-5").unwrap().friendly());
    }

    #[test]
    fn clash_needs_a_strict_majority() {
        assert_eq!(clash(13, 12), Some(1));
        assert_eq!(clash(12, 12), None);
        assert_eq!(clash(0, 0), None);
        assert_eq!(clash(4, 9), None);
    }
}

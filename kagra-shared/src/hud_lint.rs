//! Checks on a HUD picture, and a monkey that plays a game to find broken screens.
//!
//! A HUD is a [`DrawList`]; a phone shows its mistakes at once: a word running off the
//! screen, a letter the font does not have (a blank box), text too small to read, two
//! labels on top of each other. [`check`] finds those in one list without a GPU.
//!
//! [`monkey`] loads a game from its world JSON into a [`SharedSession`], taps, drags and
//! presses keys from a seed for a while, and runs [`check`] on the screens it reaches, in
//! each language and at phone size. It walks into screens a hand-written test never opens.
//! A test for a new game is three lines (see `tests` below); what it finds is either fixed
//! or listed in [`Allow`] with the reason.
//!
//! ```ignore
//! let report = hud_lint::monkey(dump, &Monkey::phone("ja"));
//! assert!(report.issues.is_empty(), "{report}");
//! ```

use std::collections::BTreeMap;
use std::fmt;

use crate::font;
use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, TextAlign, TextQuad};
use crate::session::SharedSession;

/// What is wrong with a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// Part of the text is cut by the edge of the screen (or of its clip rectangle).
    OffScreen,
    /// A letter the font has no glyph for: it draws as a blank box.
    MissingGlyph,
    /// Smaller than a phone shows clearly.
    TooSmall,
    /// Drawn over another text in the same layer.
    Overlap,
    /// The game itself did not load (a monkey run's only finding then).
    Broken,
}

/// One finding.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub kind: Kind,
    pub text: String,
    /// The text's box on screen: x, y, w, h.
    pub rect: [f32; 4],
    /// What it ran into: the edge, the other text, the size.
    pub note: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x, y, w, h] = self.rect;
        write!(
            f,
            "{:?} '{}' at {x:.0},{y:.0} {w:.0}x{h:.0}: {}",
            self.kind, self.text, self.note
        )
    }
}

/// The limits [`check`] holds a HUD to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Text smaller than this many pixels is an issue: under 8 nobody reads it, even on a
    /// sharp screen. (A caption of 9 design units on a 360-wide phone is 8.1.)
    pub min_size: f32,
    /// A text may hang this far over an edge before it counts (an outline, a rounding).
    pub slack: f32,
    /// Two texts overlap when they share more than this share of the smaller one's box.
    pub overlap: f32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            min_size: 8.0,
            slack: 2.0,
            overlap: 0.35,
        }
    }
}

/// The box of a single-line text: x, y, w, h.
pub fn text_box(t: &TextQuad) -> [f32; 4] {
    let lines: Vec<&str> = t.text.split('\n').collect();
    let w = lines
        .iter()
        .map(|l| font::measure_text_face(l, t.size, t.face))
        .fold(0.0, f32::max);
    let x = match t.align {
        TextAlign::Left => t.x,
        TextAlign::Center => t.x - w * 0.5,
        TextAlign::Right => t.x - w,
    };
    let line_h = (t.size * 1.2).max(1.0);
    [x, t.y, w, t.size + (lines.len() as f32 - 1.0) * line_h]
}

/// A number or a mark that floats over the play (a damage number, "+1", a timer, a lone
/// icon). Such a thing lands on other text and on the screen's edge by its nature; it is
/// only checked for its size and its glyphs.
pub fn is_marker(text: &str) -> bool {
    let t = text.trim();
    t.chars().count() <= 2
        || t.chars()
            .all(|c| c.is_ascii_digit() || " +-−×x.:%!?/▲▼◇◆●♥★☆".contains(c))
}

/// Where a line's ink is: the box without the empty band above the letters and below the
/// baseline (two lines set close do not count as on top of each other).
fn ink_box(b: [f32; 4], size: f32) -> [f32; 4] {
    [b[0], b[1] + size * 0.18, b[2], (b[3] - size * 0.3).max(1.0)]
}

/// The text with every digit as `#`: "1:51" and "1:50" are one finding.
fn same_shape(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_digit() { '#' } else { c })
        .collect()
}

fn shared_area(a: [f32; 4], b: [f32; 4]) -> f32 {
    let w = (a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]);
    let h = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    if w > 0.0 && h > 0.0 {
        w * h
    } else {
        0.0
    }
}

/// The issues in one HUD at `w` x `h` pixels, with the default [`Limits`].
pub fn check(list: &DrawList, w: u32, h: u32) -> Vec<Issue> {
    check_with(list, w, h, Limits::default())
}

pub fn check_with(list: &DrawList, w: u32, h: u32, lim: Limits) -> Vec<Issue> {
    let (sw, sh) = (w as f32, h as f32);
    let mut out = Vec::new();
    for (layer, clip) in list.flatten_clipped() {
        // Texts as (box, the quad), letters with ink only.
        let texts: Vec<([f32; 4], &TextQuad)> = layer
            .texts
            .iter()
            .filter(|t| t.color[3] > 24 && !t.text.trim().is_empty())
            .map(|t| (text_box(t), t))
            .collect();
        for (rect, t) in &texts {
            let [x, y, tw, th] = *rect;
            let push = |out: &mut Vec<Issue>, kind, note: String| {
                out.push(Issue {
                    kind,
                    text: t.text.clone(),
                    rect: *rect,
                    note,
                })
            };
            if !font::text_covered_on_web(&t.text) {
                let missing: String = t
                    .text
                    .chars()
                    .filter(|c| !c.is_whitespace() && !font::text_covered_on_web(&c.to_string()))
                    .collect();
                push(
                    &mut out,
                    Kind::MissingGlyph,
                    format!("no glyph for '{missing}'"),
                );
            }
            // What is all the way off the screen (a list scrolled away, a card sliding in)
            // is not seen and not judged.
            let seen = x < sw && x + tw > 0.0 && y < sh && y + th > 0.0;
            if !seen {
                continue;
            }
            if t.size < lim.min_size {
                push(&mut out, Kind::TooSmall, format!("{:.1} px", t.size));
            }
            if is_marker(&t.text) {
                continue;
            }
            match clip {
                // A window (a list, a road that slides) cuts what moves through it on
                // purpose. Only a text wider than the window itself can never be read.
                Some(c) => {
                    let room = c[2].min(sw);
                    let in_window =
                        y + th > c[1] && y < c[1] + c[3] && x + tw > c[0] && x < c[0] + c[2];
                    if in_window && tw > room + lim.slack {
                        push(
                            &mut out,
                            Kind::OffScreen,
                            format!("{tw:.0} wide in a window of {room:.0}"),
                        );
                    }
                }
                None => {
                    if x < -lim.slack || x + tw > sw + lim.slack {
                        push(
                            &mut out,
                            Kind::OffScreen,
                            format!("spans {:.0}..{:.0} across a screen of {sw:.0}", x, x + tw),
                        );
                    } else if y < -lim.slack || y + th > sh + lim.slack {
                        push(
                            &mut out,
                            Kind::OffScreen,
                            format!("spans {:.0}..{:.0} down a screen of {sh:.0}", y, y + th),
                        );
                    }
                }
            }
        }
        // Different words on top of each other in one layer. A shadow or an outline is the
        // same word again and does not count; neither does a number floating over a label.
        for (i, (a, ta)) in texts.iter().enumerate() {
            if is_marker(&ta.text) {
                continue;
            }
            for (b, tb) in texts.iter().skip(i + 1) {
                if is_marker(&tb.text)
                    || ta.text == tb.text
                    || ta.text.contains(&tb.text)
                    || tb.text.contains(&ta.text)
                {
                    continue;
                }
                let (ia, ib) = (ink_box(*a, ta.size), ink_box(*b, tb.size));
                let small = (ia[2] * ia[3]).min(ib[2] * ib[3]);
                if small > 1.0 && shared_area(ia, ib) > small * lim.overlap {
                    out.push(Issue {
                        kind: Kind::Overlap,
                        text: ta.text.clone(),
                        rect: *a,
                        note: format!("under '{}'", tb.text),
                    });
                }
            }
        }
    }
    out
}

/// Findings that are known and wanted: (kind, a piece of the text). A finding whose text
/// holds the piece is dropped. Keep the reason next to each entry.
pub type Allow<'a> = &'a [(Kind, &'a str)];

/// How the monkey plays.
#[derive(Clone, Debug, PartialEq)]
pub struct Monkey {
    pub width: u32,
    pub height: u32,
    /// `ja` or `en`.
    pub lang: String,
    pub seed: u64,
    /// Frames to play (60 a second).
    pub frames: u32,
    /// The HUD is checked every this many frames.
    pub check_every: u32,
    pub limits: Limits,
}

impl Monkey {
    /// A phone held upright, 40 seconds of play.
    pub fn phone(lang: &str) -> Self {
        Self {
            width: 390,
            height: 844,
            lang: lang.into(),
            seed: 1,
            frames: 2400,
            check_every: 20,
            limits: Limits::default(),
        }
    }

    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn frames(mut self, frames: u32) -> Self {
        self.frames = frames;
        self
    }

    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

/// What a monkey run found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    /// Each different finding once, with the game's phase when it was first seen.
    pub issues: Vec<(String, Issue)>,
    /// How many HUDs were checked, and the phases the run passed through.
    pub screens: u32,
    pub phases: Vec<String>,
    /// Taps and drags sent.
    pub inputs: u32,
}

impl Report {
    /// Without the findings an [`Allow`] list names.
    pub fn without(mut self, allow: Allow) -> Self {
        self.issues.retain(|(_, i)| {
            !allow
                .iter()
                .any(|(k, piece)| *k == i.kind && i.text.contains(piece))
        });
        self
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} issue(s) in {} screens, {} inputs, phases {:?}",
            self.issues.len(),
            self.screens,
            self.inputs,
            self.phases
        )?;
        for (phase, i) in &self.issues {
            writeln!(f, "  [{phase}] {i}")?;
        }
        Ok(())
    }
}

/// A small generator of its own, so a run is the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // SplitMix64.
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n.max(1) as u64) as u32
    }
}

/// Play the game in `world_json` as the monkey says and report what its screens show.
/// A game that fails to load reports that as its one issue.
pub fn monkey(world_json: &str, m: &Monkey) -> Report {
    let mut report = Report::default();
    let mut s = SharedSession::default();
    s.create_surface(m.width, m.height);
    if let Err(e) = s.load_game_json(world_json) {
        report.issues.push((
            "load".into(),
            Issue {
                kind: Kind::Broken,
                text: String::new(),
                rect: [0.0; 4],
                note: format!("the game did not load: {e}"),
            },
        ));
        return report;
    }
    s.set_lang(&m.lang);
    let mut rng = Rng(m.seed ^ 0x5EED_0000_0000);
    let (w, h) = (m.width as f32, m.height as f32);
    let mut seen: BTreeMap<(Kind, String), ()> = BTreeMap::new();
    let mut held: Option<(f32, f32, u32)> = None;
    let keys = [
        "Enter",
        " ",
        "ArrowLeft",
        "ArrowRight",
        "ArrowUp",
        "1",
        "2",
        "3",
    ];
    let mut look = |s: &SharedSession, report: &mut Report| {
        let phase = s.kit_phase().unwrap_or("world").to_string();
        if !report.phases.contains(&phase) {
            report.phases.push(phase.clone());
        }
        report.screens += 1;
        for issue in check_with(&s.draw_list(), m.width, m.height, m.limits) {
            let key = (issue.kind, same_shape(&issue.text));
            if seen.insert(key, ()).is_none() {
                report.issues.push((phase.clone(), issue));
            }
        }
    };
    look(&s, &mut report);
    for frame in 0..m.frames {
        // A finger held down moves, then lifts.
        if let Some((x, y, left)) = held {
            let (nx, ny) = (
                (x + (rng.unit() - 0.5) * 60.0).clamp(0.0, w - 1.0),
                (y + (rng.unit() - 0.5) * 60.0).clamp(0.0, h - 1.0),
            );
            let phase = if left == 0 {
                PointerPhase::End
            } else {
                PointerPhase::Move
            };
            s.push_pointer(PointerEvent {
                id: 1,
                x: nx,
                y: ny,
                phase,
                pressure: 1.0,
            });
            held = (left > 0).then(|| (nx, ny, left - 1));
        } else if frame % 6 == 0 {
            match rng.below(10) {
                // Mostly taps: anywhere, with a lean to the lower half where buttons are.
                0..=6 => {
                    let x = rng.unit() * w;
                    let y = if rng.below(3) == 0 {
                        rng.unit() * h
                    } else {
                        h * (0.45 + rng.unit() * 0.55)
                    };
                    for phase in [PointerPhase::Begin, PointerPhase::End] {
                        s.push_pointer(PointerEvent {
                            id: 1,
                            x,
                            y: y.min(h - 1.0),
                            phase,
                            pressure: 1.0,
                        });
                    }
                    report.inputs += 1;
                }
                // A drag (a stick, a list, a swipe).
                7 | 8 => {
                    let (x, y) = (rng.unit() * w, rng.unit() * h);
                    s.push_pointer(PointerEvent {
                        id: 1,
                        x,
                        y,
                        phase: PointerPhase::Begin,
                        pressure: 1.0,
                    });
                    held = Some((x, y, 4 + rng.below(20)));
                    report.inputs += 1;
                }
                _ => {
                    let k = keys[rng.below(keys.len() as u32) as usize];
                    s.push_key(k, true);
                    s.push_key(k, false);
                    report.inputs += 1;
                }
            }
        }
        s.request_frame();
        if (frame + 1) % m.check_every.max(1) == 0 {
            look(&s, &mut report);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::TextFace;

    fn list_with(texts: Vec<TextQuad>) -> DrawList {
        DrawList {
            texts,
            ..Default::default()
        }
    }

    #[test]
    fn a_clean_hud_has_no_issues() {
        let list = list_with(vec![
            TextQuad::new("SCORE 120", 12.0, 10.0, 16.0, [255; 4]),
            TextQuad::new("タップで開始", 195.0, 400.0, 20.0, [255; 4]).aligned(TextAlign::Center),
        ]);
        assert_eq!(check(&list, 390, 844), vec![]);
    }

    #[test]
    fn text_past_the_edge_is_found_and_text_all_the_way_off_is_not() {
        let list = list_with(vec![
            TextQuad::new("A LONG LINE THAT RUNS OFF", 300.0, 10.0, 18.0, [255; 4]),
            TextQuad::new(
                "CENTRED TOO WIDE FOR THE SCREEN",
                30.0,
                60.0,
                24.0,
                [255; 4],
            )
            .aligned(TextAlign::Center),
            TextQuad::new(
                "off screen, waiting to slide in",
                500.0,
                10.0,
                18.0,
                [255; 4],
            ),
            TextQuad::new("below", 10.0, 838.0, 16.0, [255; 4]),
            // A damage number at the edge is a marker: not judged.
            TextQuad::new("-12", 380.0, 300.0, 18.0, [255; 4]),
        ]);
        let found = check(&list, 390, 844);
        let off: Vec<_> = found
            .iter()
            .filter(|i| i.kind == Kind::OffScreen)
            .map(|i| i.text.as_str())
            .collect();
        assert_eq!(
            off,
            [
                "A LONG LINE THAT RUNS OFF",
                "CENTRED TOO WIDE FOR THE SCREEN",
                "below"
            ]
        );
    }

    #[test]
    fn a_clip_cuts_rows_but_not_words() {
        let mut list = DrawList::default();
        let window = list.layer();
        window.clip = Some([20.0, 100.0, 200.0, 120.0]);
        // A tile sliding out of the window's side: cut on purpose.
        window
            .texts
            .push(TextQuad::new("sliding", 200.0, 160.0, 16.0, [255; 4]));
        // A row half out of the window's bottom: cut on purpose.
        window
            .texts
            .push(TextQuad::new("row 5", 30.0, 212.0, 16.0, [255; 4]));
        // A row scrolled away: not seen.
        window.texts.push(TextQuad::new(
            "row 9 is far below",
            30.0,
            400.0,
            16.0,
            [255; 4],
        ));
        // A word wider than the window: an issue.
        window.texts.push(TextQuad::new(
            "a name far too long for the list to hold on one line",
            30.0,
            130.0,
            16.0,
            [255; 4],
        ));
        let found = check(&list, 390, 844);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].kind, Kind::OffScreen);
        assert!(found[0].text.starts_with("a name far too long"));
    }

    #[test]
    fn a_missing_glyph_small_text_and_an_overlap_are_found() {
        let list = list_with(vec![
            // A letter no font here has (a private-use one), written as an escape.
            TextQuad::new("\u{E000} OK", 10.0, 10.0, 16.0, [255; 4]),
            TextQuad::new("fine print", 10.0, 60.0, 6.0, [255; 4]),
            TextQuad::new("HITS", 100.0, 200.0, 16.0, [255; 4]),
            TextQuad::new("RUNS", 104.0, 202.0, 16.0, [255; 4]),
            // A shadow: the same word again, not an overlap.
            TextQuad::new("TITLE", 100.0, 300.0, 30.0, [0, 0, 0, 255]),
            TextQuad::new("TITLE", 98.0, 298.0, 30.0, [255; 4]),
            // Invisible text is not judged.
            TextQuad::new("ghost", 100.0, 200.0, 16.0, [255, 255, 255, 0]),
        ]);
        let found = check(&list, 390, 844);
        let kinds: Vec<Kind> = found.iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&Kind::TooSmall), "{found:?}");
        assert!(kinds.contains(&Kind::Overlap), "{found:?}");
        assert_eq!(
            kinds.iter().filter(|k| **k == Kind::Overlap).count(),
            1,
            "{found:?}"
        );
        let glyph = found.iter().find(|i| i.kind == Kind::MissingGlyph).unwrap();
        assert!(glyph.note.contains('\u{E000}'), "{glyph}");
    }

    #[test]
    fn markers_are_numbers_and_lone_icons() {
        for m in ["-12", "+1", "1:51", "▼", "◇ 3", "8", "OK", "×2", "100%"] {
            assert!(is_marker(m), "{m}");
        }
        for word in ["HIT!", "LEVEL UP!", "ホームラン！", "BEST 650", "3回表"] {
            assert!(!is_marker(word), "{word}");
        }
        assert_eq!(same_shape("1:51"), same_shape("1:07"));
        assert_ne!(same_shape("BEST 650"), same_shape("BEST 1115"));
    }

    /// Two lines set close are not on top of each other, and a number floating over a
    /// label is not an overlap.
    #[test]
    fn close_lines_and_floating_numbers_do_not_overlap() {
        let list = list_with(vec![
            TextQuad::new("GAME SET", 100.0, 400.0, 40.0, [255; 4]),
            TextQuad::new("the game is over", 100.0, 436.0, 14.0, [255; 4]),
            TextQuad::new("+30", 110.0, 402.0, 16.0, [255; 4]),
        ]);
        assert_eq!(check(&list, 390, 844), vec![]);
    }

    #[test]
    fn the_display_face_is_measured_with_its_own_letters() {
        let t = TextQuad::new("STAGE 12", 0.0, 0.0, 24.0, [255; 4]).with_face(TextFace::Display);
        let b = text_box(&t);
        assert!((b[2] - font::measure_text_face("STAGE 12", 24.0, TextFace::Display)).abs() < 0.01);
    }

    #[test]
    fn the_same_seed_plays_the_same_run() {
        let dump = crate::crate_tower::CrateTower::new(Default::default())
            .world_value()
            .to_string();
        let m = Monkey::phone("en").frames(400);
        let (a, b) = (monkey(&dump, &m), monkey(&dump, &m));
        assert_eq!(a, b);
        assert!(a.screens >= 20 && a.inputs > 20);
        assert!(a.phases.contains(&"play".to_string()), "{a}");
        let bad = monkey("{\"genre\":\"nope", &m);
        assert_eq!(bad.issues.len(), 1);
    }
}

#[cfg(test)]
mod every_kit {
    use super::*;

    /// Every kit's world fixture (`tests/fixtures/*_world.json` that a kit claims), so a
    /// new game is played by the monkey as soon as its fixture is baked.
    pub(super) fn kit_fixtures() -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(stem) = name.strip_suffix("_world.json") else {
                continue;
            };
            let Ok(dump) = std::fs::read_to_string(e.path()) else {
                continue;
            };
            if crate::kit_host::load(&dump).is_some_and(|k| k.is_ok()) {
                out.push((stem.to_string(), dump));
            }
        }
        out.sort();
        out
    }

    /// The monkey plays every kit for 40 seconds in Japanese and in English on a phone,
    /// and again on a small phone: no panic, the game gets into play, and no screen it
    /// reaches has text off the screen, a blank letter, unreadable print or one label on
    /// another. A finding here is a bug to fix (or, when it is wanted, a line in an
    /// [`Allow`] list with the reason).
    #[test]
    fn every_kit_plays_clean_under_the_monkey() {
        let kits = kit_fixtures();
        assert!(kits.len() >= 11, "{} kits found", kits.len());
        let mut bad = String::new();
        for (name, dump) in &kits {
            for (lang, w, h, seed) in [
                ("ja", 390, 844, 1),
                ("en", 390, 844, 1),
                ("en", 360, 640, 2),
            ] {
                let m = Monkey::phone(lang).size(w, h).seed(seed);
                let r = monkey(dump, &m);
                assert!(
                    r.phases.iter().any(|p| p == "play"),
                    "{name} never played: {r}"
                );
                if !r.issues.is_empty() {
                    bad += &format!("== {name} {lang} {w}x{h}: {r}");
                }
            }
        }
        assert!(bad.is_empty(), "\n{bad}");
    }
}

#[cfg(test)]
mod explore {
    use super::*;

    /// `KAGRA_MONKEY=1 cargo test -p kagra-shared --lib hud_lint::explore -- --nocapture`
    /// prints what the monkey finds in every kit, both languages.
    #[test]
    fn print_what_the_monkey_finds() {
        if std::env::var("KAGRA_MONKEY").is_err() {
            return;
        }
        for (name, dump) in super::every_kit::kit_fixtures() {
            let name = name.as_str();
            // `KAGRA_MONKEY_KITS=diamond_days,engine_lab` plays only those.
            if std::env::var("KAGRA_MONKEY_KITS")
                .is_ok_and(|only| !only.split(',').any(|k| k == name))
            {
                continue;
            }
            let deep = std::env::var("KAGRA_MONKEY").is_ok_and(|v| v == "deep");
            let sizes: &[(u32, u32)] = if deep {
                &[(390, 844), (360, 640), (1280, 720)]
            } else {
                &[(390, 844), (360, 640)]
            };
            let seeds: u64 = if deep { 5 } else { 1 };
            for lang in ["ja", "en"] {
                let mut all: Vec<String> = Vec::new();
                let mut phases: Vec<String> = Vec::new();
                for &(w, h) in sizes {
                    for seed in 1..=seeds {
                        let m = Monkey::phone(lang).size(w, h).seed(seed).frames(if deep {
                            7200
                        } else {
                            2400
                        });
                        let r = monkey(&dump, &m);
                        for p in r.phases {
                            if !phases.contains(&p) {
                                phases.push(p);
                            }
                        }
                        for (phase, i) in r.issues {
                            let line = format!("  [{w}x{h} {phase}] {i}");
                            let key = format!("{:?}{}", i.kind, i.text);
                            if !all.iter().any(|l| {
                                l.contains(&i.text) && l.contains(&format!("{:?}", i.kind))
                            }) {
                                let _ = key;
                                all.push(line);
                            }
                        }
                    }
                }
                eprintln!(
                    "== {name} {lang}: {} issue(s), phases {phases:?}",
                    all.len()
                );
                for l in all {
                    eprintln!("{l}");
                }
            }
        }
    }
}

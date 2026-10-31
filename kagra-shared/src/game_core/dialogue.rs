//! Conversations: who says what, with a face, typed out; choices that branch and set flags;
//! events the kit acts on ("give potion", "open the gate"). The script is JSON, so a game's
//! story lives next to its world.
//!
//! ```json
//! {"start": "gate",
//!  "cast": {"guard": {"name": "Guard", "name_ja": "門番", "skin": [240, 200, 170], "hair": [80, 50, 30]}},
//!  "nodes": {
//!    "gate": {"lines": [{"who": "guard", "text": "Halt!", "text_ja": "止まれ！", "face": "angry"}],
//!             "choices": [{"text": "I have a pass", "goto": "pass", "if": "pass"},
//!                         {"text": "Leave", "goto": ""}]},
//!    "pass": {"lines": [{"who": "guard", "text": "Go on.", "face": "smile"}], "events": ["open_gate"]}}}
//! ```
//!
//! A kit holds a [`Talk`], feeds it taps ([`Talk::tap`], [`Talk::choose`]) and the clock
//! ([`Talk::tick`]), drains [`Talk::events`], and draws it with [`Talk::draw`]. Faces are
//! drawn from shapes ([`face`]), so a speaker needs no picture file: skin, hair, eyes and a
//! mood per line (smile, angry, sad, surprised), with blinking and a mouth that moves while
//! the text types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::hud_kit::{pill_button, round_panel, wrap_text};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};

/// Someone who speaks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Speaker {
    pub name: String,
    pub name_ja: String,
    pub skin: [u8; 3],
    pub hair: [u8; 3],
    pub eyes: [u8; 3],
    /// The name plate and the frame.
    pub color: [u8; 3],
    /// Which side the face sits on ("left" or "right").
    pub side: String,
    /// No face (a sign, a narrator).
    pub faceless: bool,
}

impl Default for Speaker {
    fn default() -> Self {
        Self {
            name: String::new(),
            name_ja: String::new(),
            skin: [240, 205, 175],
            hair: [70, 50, 40],
            eyes: [40, 40, 60],
            color: [120, 170, 255],
            side: "left".into(),
            faceless: false,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Line {
    /// A key of the cast ("" = the narrator: no face, no name).
    pub who: String,
    pub text: String,
    pub text_ja: String,
    /// The mood: "", "smile", "angry", "sad", "surprised".
    pub face: String,
    /// Shown only when this condition holds (see [`Talk::check`]).
    #[serde(rename = "if")]
    pub cond: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Choice {
    pub text: String,
    pub text_ja: String,
    /// The node it leads to ("" ends the talk).
    pub goto: String,
    /// Flags it sets (name -> value).
    pub set: BTreeMap<String, i64>,
    /// Events it sends the kit.
    pub events: Vec<String>,
    /// Offered only when this condition holds.
    #[serde(rename = "if")]
    pub cond: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Node {
    pub lines: Vec<Line>,
    /// Offered after the last line. None: go to `next`.
    pub choices: Vec<Choice>,
    /// The node after this one when there are no choices ("" ends the talk).
    pub next: String,
    /// Flags set on entering.
    pub set: BTreeMap<String, i64>,
    /// Events sent on entering.
    pub events: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Script {
    pub start: String,
    pub cast: BTreeMap<String, Speaker>,
    pub nodes: BTreeMap<String, Node>,
}

impl Script {
    pub fn from_value(v: &serde_json::Value) -> Result<Self, String> {
        let s: Script = serde_json::from_value(v.clone()).map_err(|e| format!("dialogue: {e}"))?;
        s.validate()?;
        Ok(s)
    }

    /// Every link goes somewhere, every speaker is in the cast.
    pub fn validate(&self) -> Result<(), String> {
        let known = |id: &str| id.is_empty() || self.nodes.contains_key(id);
        if !known(&self.start) {
            return Err(format!("dialogue: start {:?} is not a node", self.start));
        }
        for (id, n) in &self.nodes {
            if !known(&n.next) {
                return Err(format!("dialogue: {id}.next {:?} is not a node", n.next));
            }
            for c in &n.choices {
                if !known(&c.goto) {
                    return Err(format!("dialogue: {id} choice goes to {:?}", c.goto));
                }
            }
            for l in &n.lines {
                if !l.who.is_empty() && !self.cast.contains_key(&l.who) {
                    return Err(format!("dialogue: {id} has {:?}, not in the cast", l.who));
                }
            }
        }
        Ok(())
    }
}

/// Where a talk is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beat {
    /// The line is typing out.
    Typing,
    /// The line is all out; a tap goes on.
    Waiting,
    /// Choices are up.
    Choosing,
    /// Over (or never started).
    Done,
}

/// A conversation in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Talk {
    pub script: Script,
    /// Flags the script reads and sets (kept by the kit between talks, saved with the game).
    pub flags: BTreeMap<String, i64>,
    /// Events for the kit, oldest first. The kit drains them.
    pub events: Vec<String>,
    /// Characters a second while typing.
    pub cps: f32,
    pub beat: Beat,
    node: String,
    /// The line shown (index into the node's lines).
    line: usize,
    typed: f32,
    /// The choice the keys point at.
    pub cursor: usize,
    clock: f32,
}

impl Talk {
    pub fn new(script: Script) -> Self {
        Self {
            script,
            flags: BTreeMap::new(),
            events: Vec::new(),
            cps: 40.0,
            beat: Beat::Done,
            node: String::new(),
            line: 0,
            typed: 0.0,
            cursor: 0,
            clock: 0.0,
        }
    }

    pub fn active(&self) -> bool {
        self.beat != Beat::Done
    }

    /// The node playing.
    pub fn node(&self) -> &str {
        &self.node
    }

    /// Start at the script's start.
    pub fn begin(&mut self) {
        let start = self.script.start.clone();
        self.goto(&start);
    }

    /// Jump to a node ("" ends the talk).
    pub fn goto(&mut self, id: &str) {
        let Some(n) = self.script.nodes.get(id) else {
            self.beat = Beat::Done;
            self.node.clear();
            return;
        };
        let (set, events) = (n.set.clone(), n.events.clone());
        self.node = id.into();
        for (k, v) in set {
            self.flags.insert(k, v);
        }
        self.events.extend(events);
        self.line = 0;
        self.cursor = 0;
        self.show_line();
    }

    /// Settle on the first line at or after `self.line` whose condition holds.
    fn show_line(&mut self) {
        let lines = &self.script.nodes[&self.node].lines;
        while self.line < lines.len() && !self.check(&lines[self.line].cond) {
            self.line += 1;
        }
        if self.line < lines.len() {
            self.beat = Beat::Typing;
            self.typed = 0.0;
        } else {
            self.after_lines();
        }
    }

    fn after_lines(&mut self) {
        if !self.choices().is_empty() {
            self.beat = Beat::Choosing;
            self.cursor = 0;
        } else {
            let next = self.script.nodes[&self.node].next.clone();
            self.goto(&next);
        }
    }

    /// Does a condition hold? `""` (always), `"flag"` (non-zero), `"!flag"` (zero or unset),
    /// `"flag>=2"`, `"flag<3"`, `"flag==1"`, `"flag!=0"`.
    pub fn check(&self, cond: &str) -> bool {
        let c = cond.trim();
        if c.is_empty() {
            return true;
        }
        if let Some(name) = c.strip_prefix('!') {
            return self.flag(name.trim()) == 0;
        }
        for op in [">=", "<=", "==", "!=", ">", "<"] {
            if let Some((name, val)) = c.split_once(op) {
                let (a, b) = (
                    self.flag(name.trim()),
                    val.trim().parse::<i64>().unwrap_or(0),
                );
                return match op {
                    ">=" => a >= b,
                    "<=" => a <= b,
                    "==" => a == b,
                    "!=" => a != b,
                    ">" => a > b,
                    _ => a < b,
                };
            }
        }
        self.flag(c) != 0
    }

    pub fn flag(&self, name: &str) -> i64 {
        self.flags.get(name).copied().unwrap_or(0)
    }

    /// The line on screen: who says it and the whole text.
    pub fn line(&self) -> Option<&Line> {
        if !matches!(self.beat, Beat::Typing | Beat::Waiting) {
            return None;
        }
        self.script.nodes.get(&self.node)?.lines.get(self.line)
    }

    /// The choices on offer (index in the node, the choice), conditions applied.
    pub fn choices(&self) -> Vec<(usize, &Choice)> {
        let Some(n) = self.script.nodes.get(&self.node) else {
            return Vec::new();
        };
        n.choices
            .iter()
            .enumerate()
            .filter(|(_, c)| self.check(&c.cond))
            .collect()
    }

    pub fn tick(&mut self, dt: f32) {
        self.clock += dt;
        if self.beat == Beat::Typing {
            self.typed += dt * self.cps;
            let len = self
                .line()
                .map_or(0, |l| l.text.chars().count().max(l.text_ja.chars().count()));
            if self.typed >= len as f32 {
                self.beat = Beat::Waiting;
            }
        }
    }

    /// A tap (or the confirm key): finish typing, or go on. Choices take [`Talk::choose`].
    pub fn tap(&mut self) {
        match self.beat {
            Beat::Typing => self.beat = Beat::Waiting,
            Beat::Waiting => {
                self.line += 1;
                self.show_line();
            }
            Beat::Choosing => {
                let pick = self.cursor;
                self.choose(pick);
            }
            Beat::Done => {}
        }
    }

    /// Move the key cursor over the choices.
    pub fn point(&mut self, dir: i32) {
        let n = self.choices().len() as i32;
        if self.beat == Beat::Choosing && n > 0 {
            self.cursor = (self.cursor as i32 + dir).rem_euclid(n) as usize;
        }
    }

    /// Take the `i`-th choice on offer.
    pub fn choose(&mut self, i: usize) {
        if self.beat != Beat::Choosing {
            return;
        }
        let Some((_, c)) = self.choices().get(i).map(|(k, c)| (*k, (*c).clone())) else {
            return;
        };
        for (k, v) in c.set {
            self.flags.insert(k, v);
        }
        self.events.extend(c.events);
        self.goto(&c.goto);
    }

    /// Take the events out.
    pub fn drain(&mut self) -> Vec<String> {
        std::mem::take(&mut self.events)
    }

    // ---- picture

    /// The rectangles of the choices on a `w` x `h` screen at HUD scale `s` (x, y, w, h),
    /// in the order of [`Talk::choices`].
    pub fn choice_rects(&self, w: f32, h: f32, s: f32) -> Vec<[f32; 4]> {
        if self.beat != Beat::Choosing {
            return Vec::new();
        }
        let n = self.choices().len();
        let bw = (w - 48.0 * s).min(360.0 * s);
        let bh = 46.0 * s;
        let gap = 10.0 * s;
        let bottom = h - panel_h(s) - 26.0 * s;
        (0..n)
            .map(|i| {
                let y = bottom - (n - i) as f32 * (bh + gap);
                [(w - bw) * 0.5, y, bw, bh]
            })
            .collect()
    }

    /// Which choice a tap hit.
    pub fn choice_at(&self, x: f32, y: f32, w: f32, h: f32, s: f32) -> Option<usize> {
        self.choice_rects(w, h, s)
            .iter()
            .position(|r| x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3])
    }

    /// Draw the talk: the box at the bottom, the face, the name, the text so far, the
    /// "more" mark, and the choices.
    pub fn draw(&self, list: &mut DrawList, w: f32, h: f32, s: f32, ja: bool) {
        if !self.active() {
            return;
        }
        // The last line stays under the choices.
        let line = self.line().cloned().or_else(|| {
            let n = self.script.nodes.get(&self.node)?;
            n.lines.iter().rev().find(|l| self.check(&l.cond)).cloned()
        });
        let ph = panel_h(s);
        let (px, py, pw) = (12.0 * s, h - ph - 12.0 * s, w - 24.0 * s);
        round_panel(
            list,
            px,
            py,
            pw,
            ph,
            16.0 * s,
            [10, 14, 30, 236],
            [150, 180, 255, 130],
            1.5 * s,
            (0.0, 4.0 * s, 90),
        );
        if let Some(l) = &line {
            let who = self.script.cast.get(&l.who);
            let right = who.is_some_and(|c| c.side == "right");
            let face_r = 40.0 * s;
            let has_face = who.is_some_and(|c| !c.faceless);
            if let (Some(c), true) = (who, has_face) {
                let cx = if right {
                    px + pw - face_r - 14.0 * s
                } else {
                    px + face_r + 14.0 * s
                };
                let cy = py - face_r * 0.55;
                let talking = self.beat == Beat::Typing;
                face(list, cx, cy, face_r, c, &l.face, self.clock, talking);
            }
            if let Some(c) = who {
                let name = if ja && !c.name_ja.is_empty() {
                    &c.name_ja
                } else {
                    &c.name
                };
                let nx = if !has_face {
                    px + 16.0 * s
                } else if right {
                    px + pw - 2.0 * face_r - 40.0 * s - name_w(name, s)
                } else {
                    px + 2.0 * face_r + 36.0 * s
                };
                let col = [c.color[0], c.color[1], c.color[2], 240];
                round_panel(
                    list,
                    nx,
                    py - 14.0 * s,
                    name_w(name, s),
                    28.0 * s,
                    14.0 * s,
                    col,
                    [255, 255, 255, 80],
                    1.0 * s,
                    (0.0, 2.0 * s, 60),
                );
                list.texts.push(
                    TextQuad::new(
                        name,
                        nx + name_w(name, s) * 0.5,
                        py - 9.0 * s,
                        16.0 * s,
                        [255, 255, 255, 255],
                    )
                    .aligned(TextAlign::Center),
                );
            }
            let text = if ja && !l.text_ja.is_empty() {
                &l.text_ja
            } else {
                &l.text
            };
            let shown: String = if self.beat == Beat::Typing {
                text.chars().take(self.typed as usize).collect()
            } else {
                text.clone()
            };
            wrap_text(
                list,
                &shown,
                px + 18.0 * s,
                py + 24.0 * s,
                pw - 36.0 * s,
                17.0 * s,
                [235, 238, 250, 255],
                ja,
            );
            if self.beat == Beat::Waiting && (self.clock * 2.5).sin() > -0.3 {
                arrow_down(list, px + pw - 22.0 * s, py + ph - 20.0 * s, 12.0 * s);
            }
        }
        for (i, ((_, c), r)) in self
            .choices()
            .iter()
            .zip(self.choice_rects(w, h, s))
            .enumerate()
        {
            let text = if ja && !c.text_ja.is_empty() {
                &c.text_ja
            } else {
                &c.text
            };
            let fill = if i == self.cursor {
                [70, 110, 200, 240]
            } else {
                [30, 40, 80, 230]
            };
            pill_button(
                list,
                r[0],
                r[1],
                r[2],
                r[3],
                text,
                17.0 * s,
                fill,
                [255, 255, 255, 255],
            );
        }
    }
}

fn panel_h(s: f32) -> f32 {
    124.0 * s
}

fn name_w(name: &str, s: f32) -> f32 {
    let wide = name.chars().filter(|c| !c.is_ascii()).count() as f32;
    let narrow = name.chars().count() as f32 - wide;
    (narrow * 9.5 + wide * 16.0) * s + 28.0 * s
}

/// A small solid triangle pointing down (the "more" mark).
pub fn arrow_down(list: &mut DrawList, cx: f32, cy: f32, size: f32) {
    let n = 6;
    let row = size / n as f32;
    for i in 0..n {
        let wide = size * (1.0 - (i as f32 + 0.5) / n as f32);
        list.quads.push(Quad::new(
            cx - wide * 0.5,
            cy - size * 0.5 + i as f32 * row,
            wide,
            row + 0.3,
            [220, 230, 255, 230],
        ));
    }
}

fn rgba(c: [u8; 3], a: u8) -> [u8; 4] {
    [c[0], c[1], c[2], a]
}

fn disc(list: &mut DrawList, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let mut q = Quad::new(cx - r, cy - r, 2.0 * r, 2.0 * r, color);
    q.radius = r;
    list.quads.push(q);
}

fn bar(list: &mut DrawList, cx: f32, cy: f32, w: f32, h: f32, color: [u8; 4]) {
    let mut q = Quad::new(cx - w * 0.5, cy - h * 0.5, w, h, color);
    q.radius = h * 0.5;
    list.quads.push(q);
}

/// A face from shapes in a framed circle of radius `r`: hair, skin, eyes that blink, brows
/// and a mouth set by `mood` ("", "smile", "angry", "sad", "surprised"), the mouth moving
/// while `talking`. `clock` drives the blink and the talk.
#[allow(clippy::too_many_arguments)]
pub fn face(
    list: &mut DrawList,
    cx: f32,
    cy: f32,
    r: f32,
    who: &Speaker,
    mood: &str,
    clock: f32,
    talking: bool,
) {
    // Frame and backdrop.
    disc(list, cx, cy, r + 3.0, rgba(who.color, 230));
    disc(list, cx, cy, r, [24, 30, 56, 255]);
    // Hair behind, then the face, then the fringe.
    disc(list, cx, cy - r * 0.08, r * 0.82, rgba(who.hair, 255));
    disc(list, cx, cy + r * 0.12, r * 0.66, rgba(who.skin, 255));
    let mut fringe = Quad::new(
        cx - r * 0.66,
        cy - r * 0.66,
        r * 1.32,
        r * 0.42,
        rgba(who.hair, 255),
    );
    fringe.radius = r * 0.2;
    list.quads.push(fringe);
    // Cheeks.
    let blush = [255, 140, 140, 70];
    disc(list, cx - r * 0.38, cy + r * 0.3, r * 0.1, blush);
    disc(list, cx + r * 0.38, cy + r * 0.3, r * 0.1, blush);
    // Eyes: blink for a tenth of a second every few seconds.
    let blink = (clock % 3.7) < 0.12;
    let (ex, ey) = (r * 0.25, cy + r * 0.08);
    let eye = rgba(who.eyes, 255);
    let big = if mood == "surprised" { 1.3 } else { 1.0 };
    for side in [-1.0, 1.0] {
        let x = cx + side * ex;
        if blink || mood == "smile" && !talking {
            // Closed (or happy-shut): a short line.
            bar(list, x, ey, r * 0.18, r * 0.05, eye);
        } else {
            bar(list, x, ey, r * 0.1 * big, r * 0.17 * big, eye);
            disc(
                list,
                x + r * 0.02,
                ey - r * 0.04,
                r * 0.025,
                [255, 255, 255, 230],
            );
        }
        // Brows: angry slopes in, sad slopes out (two short steps each).
        let by = ey - r * 0.2;
        let (inner, outer) = match mood {
            "angry" => (r * 0.04, -r * 0.03),
            "sad" => (-r * 0.04, r * 0.03),
            "surprised" => (-r * 0.06, -r * 0.06),
            _ => (0.0, 0.0),
        };
        let hair = rgba(who.hair, 255);
        bar(
            list,
            x - side * r * 0.05,
            by + inner,
            r * 0.1,
            r * 0.04,
            hair,
        );
        bar(
            list,
            x + side * r * 0.05,
            by + outer,
            r * 0.1,
            r * 0.04,
            hair,
        );
    }
    // Mouth.
    let my = cy + r * 0.42;
    let lip = [150, 60, 70, 255];
    let open = talking && (clock * 14.0).sin() > 0.0;
    if open {
        bar(list, cx, my, r * 0.14, r * 0.12, lip);
    } else {
        match mood {
            "smile" => {
                bar(list, cx, my + r * 0.02, r * 0.16, r * 0.05, lip);
                bar(list, cx - r * 0.1, my - r * 0.02, r * 0.07, r * 0.05, lip);
                bar(list, cx + r * 0.1, my - r * 0.02, r * 0.07, r * 0.05, lip);
            }
            "sad" | "angry" => {
                bar(list, cx, my - r * 0.01, r * 0.14, r * 0.05, lip);
                bar(list, cx - r * 0.09, my + r * 0.03, r * 0.06, r * 0.05, lip);
                bar(list, cx + r * 0.09, my + r * 0.03, r * 0.06, r * 0.05, lip);
            }
            "surprised" => disc(list, cx, my, r * 0.07, lip),
            _ => bar(list, cx, my, r * 0.16, r * 0.045, lip),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn script() -> Script {
        Script::from_value(&json!({
            "start": "gate",
            "cast": {
                "guard": {"name": "Guard", "name_ja": "門番", "side": "right"},
                "hero": {"name": "Ren"}
            },
            "nodes": {
                "gate": {
                    "lines": [
                        {"who": "guard", "text": "Halt!", "text_ja": "止まれ！", "face": "angry"},
                        {"who": "guard", "text": "Back again?", "if": "visits>=1"},
                        {"who": "hero", "text": "Let me through."}
                    ],
                    "choices": [
                        {"text": "Show the pass", "goto": "pass", "if": "pass"},
                        {"text": "Bribe him", "goto": "bribe", "if": "!broke", "set": {"broke": 1}, "events": ["pay_10"]},
                        {"text": "Leave", "goto": ""}
                    ],
                    "set": {"visits": 1}
                },
                "pass": {"lines": [{"who": "guard", "text": "Go on.", "face": "smile"}], "events": ["open_gate"]},
                "bribe": {"lines": [{"who": "guard", "text": "...Fine."}], "next": "pass"}
            }
        }))
        .unwrap()
    }

    fn type_out(t: &mut Talk) {
        for _ in 0..200 {
            t.tick(1.0 / 60.0);
        }
    }

    #[test]
    fn scripts_check_their_links_and_cast() {
        let mut bad = script();
        bad.nodes.get_mut("pass").unwrap().next = "nowhere".into();
        assert!(bad.validate().is_err());
        let mut bad = script();
        bad.nodes.get_mut("pass").unwrap().lines[0].who = "ghost".into();
        assert!(bad.validate().is_err());
        assert!(Script::from_value(&json!({"nodez": {}})).is_err());
    }

    #[test]
    fn lines_type_out_skip_on_tap_and_follow_conditions() {
        let mut t = Talk::new(script());
        t.begin();
        assert_eq!(t.beat, Beat::Typing);
        assert_eq!(t.line().unwrap().text, "Halt!");
        assert_eq!(t.flag("visits"), 1, "entering the node set its flag");
        t.tap();
        assert_eq!(t.beat, Beat::Waiting, "a tap shows the whole line");
        t.tap();
        // "Back again?" needs visits >= 1, which entering just set.
        assert_eq!(t.line().unwrap().text, "Back again?");
        t.flags.insert("visits".into(), 0);
        t.tap();
        t.tap();
        assert_eq!(t.line().unwrap().text, "Let me through.");
        type_out(&mut t);
        assert_eq!(t.beat, Beat::Waiting);
        t.tap();
        assert_eq!(t.beat, Beat::Choosing);
        let offered: Vec<&str> = t.choices().iter().map(|(_, c)| c.text.as_str()).collect();
        assert_eq!(offered, ["Bribe him", "Leave"], "no pass yet");
    }

    #[test]
    fn choices_set_flags_send_events_and_branch() {
        let mut t = Talk::new(script());
        t.begin();
        for _ in 0..6 {
            if t.beat == Beat::Choosing {
                break;
            }
            t.tap();
        }
        t.point(1);
        assert_eq!(t.cursor, 1);
        t.point(1);
        assert_eq!(t.cursor, 0, "wraps");
        t.choose(0); // Bribe
        assert_eq!(t.flag("broke"), 1);
        assert_eq!(t.drain(), vec!["pay_10".to_string()]);
        assert_eq!(t.node(), "bribe");
        t.tap();
        t.tap(); // "...Fine." -> next: pass
        assert_eq!(t.node(), "pass");
        assert_eq!(t.drain(), vec!["open_gate".to_string()]);
        t.tap();
        t.tap();
        assert!(!t.active(), "pass has no next: the talk ends");
        // The second visit: the bribe is gone, and with a pass it is offered.
        t.flags.insert("pass".into(), 1);
        t.begin();
        while t.beat != Beat::Choosing {
            t.tap();
        }
        let offered: Vec<&str> = t.choices().iter().map(|(_, c)| c.text.as_str()).collect();
        assert_eq!(offered, ["Show the pass", "Leave"]);
        t.choose(1);
        assert!(!t.active());
    }

    #[test]
    fn the_talk_draws_a_face_a_name_text_and_tappable_choices() {
        let mut t = Talk::new(script());
        t.begin();
        let mut l = DrawList::default();
        t.draw(&mut l, 390.0, 844.0, 1.0, true);
        assert!(
            l.quads.len() > 15,
            "a face is many shapes: {}",
            l.quads.len()
        );
        assert!(l.texts.iter().any(|x| x.text == "門番"));
        while t.beat != Beat::Choosing {
            t.tap();
        }
        let rects = t.choice_rects(390.0, 844.0, 1.0);
        assert_eq!(rects.len(), 2);
        let r = rects[1];
        assert_eq!(
            t.choice_at(r[0] + 5.0, r[1] + 5.0, 390.0, 844.0, 1.0),
            Some(1)
        );
        assert_eq!(t.choice_at(5.0, 5.0, 390.0, 844.0, 1.0), None);
        let mut l = DrawList::default();
        t.draw(&mut l, 390.0, 844.0, 1.0, false);
        assert!(l.texts.iter().any(|x| x.text == "Leave"));
        assert!(
            rects[1][1] + rects[1][3] < 844.0 - panel_h(1.0),
            "above the box"
        );
    }
}

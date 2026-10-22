//! Board puzzle (`genre: "puzzle"`): slide, lights-out grid, and match.
//!
//! `puzzle_pad` (crate push) stays the GenreSession reference and is not this
//! genre. Rules live here. Python only sends taps / swipes and draws the dump.
//! No sokoban editor, net, or Rapier. HUD copy is owned by the Python example
//! (ASCII). This module renders tiles, not text.

use glam::Vec3;

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::world_doc::{
    WorldCamera, WorldDoc, WorldEvent, WorldLight, WorldProp, WorldTimer, WorldWalker,
    WORLD_DUMP_VERSION,
};

pub const GAME_ID: &str = "puzzle";

const CELL: f32 = 1.08;
const EYE: [f32; 3] = [0.0, 8.6, 7.8];
const TARGET: [f32; 3] = [0.0, 0.25, 0.0];
const FOV_DEG: f32 = 40.0;
const PLANE_Y: f32 = 0.40;
const STICK: f32 = 0.5;

const SLIDE_A: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 0, 8];
const SLIDE_A_DEMO: [DemoStep; 1] = [DemoStep::Swipe(-1, 0)];

/// Lights-out. Taps at (0,0) and (2,2) return to all-off (toggles commute).
const GRID_A: [u8; 9] = [1, 1, 0, 1, 0, 1, 0, 1, 1];
const GRID_A_DEMO: [DemoStep; 2] = [DemoStep::Tap(0, 0), DemoStep::Tap(2, 2)];

const MATCH_A: [u8; 9] = [2, 1, 1, 1, 2, 2, 1, 1, 2];
const MATCH_A_DEMO: [DemoStep; 1] = [DemoStep::Swap(0, 0, 0, 1)];

const SLIDE_B: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 0, 9, 11, 12, 13, 10, 14, 15];
const SLIDE_B_DEMO: [DemoStep; 4] = [
    DemoStep::Swipe(-1, 0),
    DemoStep::Swipe(0, -1),
    DemoStep::Swipe(-1, 0),
    DemoStep::Swipe(-1, 0),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Slide,
    Grid,
    Match,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Slide => "slide",
            Kind::Grid => "grid",
            Kind::Match => "match",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DemoStep {
    /// Tile step into the blank (`+x` right, `+y` down the board).
    Swipe(i8, i8),
    Tap(u8, u8),
    Swap(u8, u8, u8, u8),
}

struct Level {
    kind: Kind,
    cols: usize,
    rows: usize,
    cells: &'static [u8],
    quota: u32,
    demo: &'static [DemoStep],
}

const LEVELS: [Level; 4] = [
    Level {
        kind: Kind::Slide,
        cols: 3,
        rows: 3,
        cells: &SLIDE_A,
        quota: 0,
        demo: &SLIDE_A_DEMO,
    },
    Level {
        kind: Kind::Grid,
        cols: 3,
        rows: 3,
        cells: &GRID_A,
        quota: 0,
        demo: &GRID_A_DEMO,
    },
    Level {
        kind: Kind::Match,
        cols: 3,
        rows: 3,
        cells: &MATCH_A,
        quota: 6,
        demo: &MATCH_A_DEMO,
    },
    Level {
        kind: Kind::Slide,
        cols: 4,
        rows: 4,
        cells: &SLIDE_B,
        quota: 0,
        demo: &SLIDE_B_DEMO,
    },
];

/// Slide / grid / match session. Picture is a `WorldDoc` (`genre = puzzle`).
#[derive(Clone, Debug)]
pub struct PuzzleSession {
    doc: WorldDoc,
    input: WalkInput,
    phase: GamePhase,
    level: usize,
    kind: Kind,
    cols: usize,
    rows: usize,
    cells: Vec<u8>,
    quota: u32,
    moves: u32,
    cleared: u32,
    level_clear: bool,
    cursor: (i32, i32),
    facing: (i32, i32),
    selected: Option<(i32, i32)>,
    demo_i: usize,
    event_seq: u32,
}

impl PuzzleSession {
    /// Load a dump. `genre` must be [`GAME_ID`]. Board layout comes from the
    /// built-in levels (the dump is the picture contract, not a level editor).
    pub fn open(json: &str) -> Result<Self, String> {
        let doc = WorldDoc::from_json(json)?;
        if doc.genre.as_deref() != Some(GAME_ID) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                GAME_ID, doc.genre
            ));
        }
        let mut session = Self {
            doc,
            input: WalkInput::default(),
            phase: GamePhase::Title,
            level: 0,
            kind: Kind::Slide,
            cols: 1,
            rows: 1,
            cells: vec![0],
            quota: 0,
            moves: 0,
            cleared: 0,
            level_clear: false,
            cursor: (0, 0),
            facing: (1, 0),
            selected: None,
            demo_i: 0,
            event_seq: 0,
        };
        session.load_level(0);
        Ok(session)
    }

    pub fn apply_input(&mut self, input: WalkInput) {
        let input = input.clamped();
        let prev = self.input;
        self.input = input;
        if self.blocked() {
            return;
        }
        let ax = input.lx.abs();
        let az = input.lz.abs();
        let prev_a = prev.lx.abs().max(prev.lz.abs());
        if ax.max(az) > STICK && prev_a <= STICK {
            let (dx, dy) = if ax >= az {
                (input.lx.signum() as i32, 0)
            } else if input.lz > 0.0 {
                (0, -1)
            } else {
                (0, 1)
            };
            self.facing = (dx, dy);
            match self.kind {
                Kind::Slide => {
                    self.swipe(dx, dy);
                }
                Kind::Grid | Kind::Match => self.move_cursor(dx, dy),
            }
        }
        if input.attack && !prev.attack && !self.blocked() {
            match self.kind {
                Kind::Slide | Kind::Grid => {
                    self.tap(self.cursor.0, self.cursor.1);
                }
                Kind::Match => {
                    self.swap_facing();
                }
            }
        }
    }

    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.tick_timers(dt);
    }

    pub fn world_json(&self) -> Result<String, String> {
        self.doc.to_json()
    }

    pub fn drain_events(&mut self, name: &str) -> Vec<WorldEvent> {
        let mut out = Vec::new();
        self.doc.events.retain(|e| {
            if e.name == name {
                out.push(e.clone());
                false
            } else {
                true
            }
        });
        out
    }

    pub fn confirm_action(&mut self) {
        match self.phase {
            GamePhase::Title => {
                self.phase = GamePhase::Playing;
                self.load_level(0);
            }
            GamePhase::Playing if self.level_clear => self.advance(),
            GamePhase::Complete => {
                self.phase = GamePhase::Title;
                self.load_level(0);
            }
            GamePhase::Playing => {}
        }
    }

    pub fn record_event(&mut self, name: &str, data: Option<serde_json::Value>) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(ev) = self.doc.events.iter_mut().find(|e| e.name == name) {
            ev.count += 1;
            if data.is_some() {
                ev.data = data;
            }
            return;
        }
        self.event_seq += 1;
        self.doc.events.push(WorldEvent {
            id: format!("event:{}", self.event_seq),
            kind: "event".into(),
            name: name.into(),
            count: 1,
            data,
        });
    }

    pub fn begin_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        let id = format!("timer:{name}");
        if let Some(t) = self.doc.timers.iter_mut().find(|t| t.id == id) {
            t.name = name.into();
            t.seconds = seconds;
            t.remaining = seconds;
            t.on_done = on_done.map(str::to_string);
            t.active = true;
            return id;
        }
        self.doc.timers.push(WorldTimer {
            id: id.clone(),
            kind: "timer".into(),
            name: name.into(),
            seconds,
            remaining: seconds,
            on_done: on_done.map(str::to_string),
            active: true,
        });
        id
    }

    /// J / attack near a prop that carries `interact`. Board tiles do not,
    /// so a normal puzzle tick does not emit from here.
    pub fn use_nearby(&mut self) {
        if !self.input.attack {
            return;
        }
        let Some(player) = self.doc.player.as_ref() else {
            return;
        };
        let (px, pz) = (player.position[0], player.position[2]);
        let mut best: Option<(usize, f32)> = None;
        for (i, prop) in self.doc.props.iter().enumerate() {
            if !prop.enabled || prop.interact.is_none() {
                continue;
            }
            let reach = prop
                .interact
                .as_ref()
                .map(|it| it.reach)
                .filter(|r| *r > 0.0)
                .unwrap_or(2.5);
            let dx = px - prop.position[0];
            let dz = pz - prop.position[2];
            let d = (dx * dx + dz * dz).sqrt();
            if d <= reach && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        let Some((i, _)) = best else {
            return;
        };
        let Some(on_use) = self.doc.props[i]
            .interact
            .as_ref()
            .and_then(|it| it.on_use.clone())
        else {
            return;
        };
        let (id, name) = (self.doc.props[i].id.clone(), self.doc.props[i].name.clone());
        self.record_event(
            &on_use,
            Some(serde_json::json!({ "prop": id, "name": name })),
        );
    }

    pub fn current_phase(&self) -> GamePhase {
        self.phase
    }

    pub fn level(&self) -> usize {
        self.level
    }

    pub fn level_count(&self) -> usize {
        LEVELS.len()
    }

    pub fn kind_name(&self) -> &'static str {
        self.kind.as_str()
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn moves(&self) -> u32 {
        self.moves
    }

    pub fn cleared(&self) -> u32 {
        self.cleared
    }

    pub fn quota(&self) -> u32 {
        self.quota
    }

    pub fn solved(&self) -> bool {
        self.level_clear
    }

    pub fn won(&self) -> bool {
        self.phase == GamePhase::Complete
    }

    pub fn cursor(&self) -> (i32, i32) {
        self.cursor
    }

    pub fn cells(&self) -> Vec<u32> {
        self.cells.iter().map(|c| u32::from(*c)).collect()
    }

    /// Keyboard / stick swipe. Slide: move the tile beside the blank.
    /// Match and grid ignore this (use [`Self::tap`] / [`Self::swipe_at`]).
    pub fn swipe(&mut self, dx: i32, dy: i32) -> bool {
        if self.blocked() || self.kind != Kind::Slide {
            return false;
        }
        let (dx, dy) = axis(dx, dy);
        if dx == 0 && dy == 0 {
            return false;
        }
        let Some((bc, br)) = self.blank_pos() else {
            return false;
        };
        self.slide_one(bc - dx, br - dy, dx, dy)
    }

    /// Pointer swipe. `(dx, dy)` is the direction the tile under the finger moves.
    pub fn swipe_at(&mut self, col: i32, row: i32, dx: i32, dy: i32) -> bool {
        if self.blocked() {
            return false;
        }
        let (dx, dy) = axis(dx, dy);
        if dx == 0 && dy == 0 {
            return false;
        }
        match self.kind {
            Kind::Slide => self.slide_one(col, row, dx, dy),
            Kind::Match => self.swap_cells(col, row, col + dx, row + dy),
            Kind::Grid => self.tap(col, row),
        }
    }

    /// Tap a cell. Slide: shift that line into the blank. Grid: toggle the
    /// light and its orthogonal neighbors. Match: select, then swap.
    pub fn tap(&mut self, col: i32, row: i32) -> bool {
        if self.blocked() || self.idx(col, row).is_none() {
            return false;
        }
        self.cursor = (col, row);
        match self.kind {
            Kind::Slide => {
                if !self.slide_line(col, row) {
                    return false;
                }
                self.after_move();
                true
            }
            Kind::Grid => {
                self.toggle(col, row);
                self.after_move();
                true
            }
            Kind::Match => self.tap_match(col, row),
        }
    }

    pub fn retry(&mut self) {
        match self.phase {
            GamePhase::Title => {}
            GamePhase::Playing => self.load_level(self.level),
            GamePhase::Complete => {
                self.phase = GamePhase::Playing;
                self.load_level(LEVELS.len() - 1);
            }
        }
    }

    /// Play the next authored solution step. Headless verify and the H hint.
    pub fn play_demo_move(&mut self) -> bool {
        if self.blocked() {
            return false;
        }
        let step = {
            let demo = LEVELS[self.level].demo;
            if self.demo_i >= demo.len() {
                return false;
            }
            demo[self.demo_i]
        };
        let ok = match step {
            DemoStep::Swipe(dx, dy) => self.swipe(i32::from(dx), i32::from(dy)),
            DemoStep::Tap(c, r) => self.tap(i32::from(c), i32::from(r)),
            DemoStep::Swap(c0, r0, c1, r1) => {
                self.swap_cells(i32::from(c0), i32::from(r0), i32::from(c1), i32::from(r1))
            }
        };
        if ok {
            self.demo_i += 1;
        }
        ok
    }

    /// Screen pixel (origin top-left) to a board cell, using the room camera.
    pub fn cell_at(&self, px: f32, py: f32, width: f32, height: f32) -> Option<(i32, i32)> {
        if width < 1.0 || height < 1.0 || !px.is_finite() || !py.is_finite() {
            return None;
        }
        let eye = Vec3::from_array(EYE);
        let target = Vec3::from_array(TARGET);
        let forward = (target - eye).normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();
        let aspect = width / height;
        let tan = (FOV_DEG.to_radians() * 0.5).tan();
        let ndc_x = (px / width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (py / height) * 2.0;
        let dir = (right * (ndc_x * aspect * tan) + up * (ndc_y * tan) + forward).normalize();
        if dir.y.abs() < 1e-5 {
            return None;
        }
        let t = (PLANE_Y - eye.y) / dir.y;
        if t < 0.0 {
            return None;
        }
        let hit = eye + dir * t;
        self.cell_at_world(hit.x, hit.z)
    }

    fn blocked(&self) -> bool {
        self.phase != GamePhase::Playing || self.level_clear
    }

    fn load_level(&mut self, index: usize) {
        let spec = &LEVELS[index];
        self.level = index;
        self.kind = spec.kind;
        self.cols = spec.cols;
        self.rows = spec.rows;
        self.cells = spec.cells.to_vec();
        self.quota = spec.quota;
        self.moves = 0;
        self.cleared = 0;
        self.level_clear = false;
        self.demo_i = 0;
        self.selected = None;
        self.facing = (1, 0);
        self.cursor = if spec.kind == Kind::Slide {
            self.blank_pos().unwrap_or((0, 0))
        } else {
            (0, 0)
        };
        self.rebuild();
    }

    fn advance(&mut self) {
        if self.level + 1 >= LEVELS.len() {
            self.phase = GamePhase::Complete;
            self.level_clear = true;
            self.record_event("win", Some(serde_json::json!({ "levels": LEVELS.len() })));
            self.rebuild();
            return;
        }
        self.load_level(self.level + 1);
        self.phase = GamePhase::Playing;
        self.rebuild();
    }

    fn after_move(&mut self) {
        self.moves += 1;
        if self.kind == Kind::Match {
            self.cleared += self.resolve_matches();
        }
        self.check_solved();
        if self.kind == Kind::Slide {
            if let Some(blank) = self.blank_pos() {
                self.cursor = blank;
            }
        }
        self.rebuild();
    }

    fn check_solved(&mut self) {
        let ok = match self.kind {
            Kind::Slide => self.slide_solved(),
            Kind::Grid => self.cells.iter().all(|c| *c == 0),
            Kind::Match => self.quota > 0 && self.cleared >= self.quota,
        };
        if ok && !self.level_clear {
            self.level_clear = true;
            self.record_event(
                "clear",
                Some(serde_json::json!({
                    "level": self.level,
                    "kind": self.kind.as_str(),
                    "moves": self.moves,
                })),
            );
        }
    }

    fn slide_solved(&self) -> bool {
        let n = self.cols * self.rows;
        if self.cells.len() != n || n == 0 {
            return false;
        }
        self.cells[..n - 1]
            .iter()
            .enumerate()
            .all(|(i, c)| *c == (i as u8) + 1)
            && self.cells[n - 1] == 0
    }

    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let c = (self.cursor.0 + dx).clamp(0, self.cols as i32 - 1);
        let r = (self.cursor.1 + dy).clamp(0, self.rows as i32 - 1);
        if (c, r) == self.cursor {
            return;
        }
        self.cursor = (c, r);
        self.rebuild();
    }

    fn swap_facing(&mut self) -> bool {
        let (dx, dy) = self.facing;
        if dx == 0 && dy == 0 {
            return false;
        }
        let (c, r) = self.cursor;
        self.swap_cells(c, r, c + dx, r + dy)
    }

    fn swap_cells(&mut self, c0: i32, r0: i32, c1: i32, r1: i32) -> bool {
        if self.blocked() || self.kind != Kind::Match {
            return false;
        }
        if (c0 - c1).abs() + (r0 - r1).abs() != 1 {
            return false;
        }
        let Some(i0) = self.idx(c0, r0) else {
            return false;
        };
        let Some(i1) = self.idx(c1, r1) else {
            return false;
        };
        if self.cells[i0] == 0 || self.cells[i1] == 0 {
            return false;
        }
        self.cells.swap(i0, i1);
        if !self.has_match() {
            self.cells.swap(i0, i1);
            return false;
        }
        self.cursor = (c1, r1);
        self.selected = None;
        self.after_move();
        true
    }

    fn tap_match(&mut self, col: i32, row: i32) -> bool {
        let Some(i) = self.idx(col, row) else {
            return false;
        };
        if self.cells[i] == 0 {
            self.selected = None;
            self.rebuild();
            return false;
        }
        match self.selected {
            None => {
                self.selected = Some((col, row));
                self.rebuild();
                true
            }
            Some((sc, sr)) if (sc, sr) == (col, row) => {
                self.selected = None;
                self.rebuild();
                true
            }
            Some((sc, sr)) if (col - sc).abs() + (row - sr).abs() == 1 => {
                self.selected = None;
                self.swap_cells(sc, sr, col, row)
            }
            Some(_) => {
                self.selected = Some((col, row));
                self.rebuild();
                true
            }
        }
    }

    fn slide_one(&mut self, col: i32, row: i32, dx: i32, dy: i32) -> bool {
        let Some(i) = self.idx(col, row) else {
            return false;
        };
        if self.cells[i] == 0 {
            return false;
        }
        let Some((bc, br)) = self.blank_pos() else {
            return false;
        };
        if col != bc && row != br {
            return false;
        }
        let toward = ((bc - col).signum(), (br - row).signum());
        if (dx, dy) != toward {
            return false;
        }
        let Some(bi) = self.idx(bc, br) else {
            return false;
        };
        let Some(ni) = self.idx(bc - dx, br - dy) else {
            return false;
        };
        self.cells.swap(bi, ni);
        self.after_move();
        true
    }

    fn slide_line(&mut self, col: i32, row: i32) -> bool {
        let Some((mut bc, mut br)) = self.blank_pos() else {
            return false;
        };
        if (col != bc && row != br) || (col == bc && row == br) {
            return false;
        }
        let mut moved = false;
        for _ in 0..(self.cols + self.rows) {
            if bc == col && br == row {
                break;
            }
            let sc = (col - bc).signum();
            let sr = (row - br).signum();
            let Some(bi) = self.idx(bc, br) else {
                break;
            };
            let Some(ni) = self.idx(bc + sc, br + sr) else {
                break;
            };
            self.cells.swap(bi, ni);
            bc += sc;
            br += sr;
            moved = true;
        }
        moved
    }

    fn toggle(&mut self, col: i32, row: i32) {
        for (dc, dr) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            self.flip(col + dc, row + dr);
        }
    }

    fn flip(&mut self, col: i32, row: i32) {
        if let Some(i) = self.idx(col, row) {
            self.cells[i] = u8::from(self.cells[i] == 0);
        }
    }

    fn has_match(&self) -> bool {
        self.match_mask().iter().any(|on| *on)
    }

    fn resolve_matches(&mut self) -> u32 {
        let mut total = 0u32;
        for _ in 0..8 {
            let mask = self.match_mask();
            let n = mask.iter().filter(|on| **on).count() as u32;
            if n == 0 {
                break;
            }
            total += n;
            for (i, on) in mask.iter().enumerate() {
                if *on {
                    self.cells[i] = 0;
                }
            }
            self.gravity();
        }
        total
    }

    fn match_mask(&self) -> Vec<bool> {
        let mut mask = vec![false; self.cells.len()];
        for r in 0..self.rows {
            let mut c = 0;
            while c < self.cols {
                let v = self.cells[r * self.cols + c];
                let mut end = c + 1;
                if v != 0 {
                    while end < self.cols && self.cells[r * self.cols + end] == v {
                        end += 1;
                    }
                }
                if v != 0 && end - c >= 3 {
                    for k in c..end {
                        mask[r * self.cols + k] = true;
                    }
                }
                c = end;
            }
        }
        for c in 0..self.cols {
            let mut r = 0;
            while r < self.rows {
                let v = self.cells[r * self.cols + c];
                let mut end = r + 1;
                if v != 0 {
                    while end < self.rows && self.cells[end * self.cols + c] == v {
                        end += 1;
                    }
                }
                if v != 0 && end - r >= 3 {
                    for k in r..end {
                        mask[k * self.cols + c] = true;
                    }
                }
                r = end;
            }
        }
        mask
    }

    fn gravity(&mut self) {
        for c in 0..self.cols {
            let mut stack = Vec::new();
            for r in (0..self.rows).rev() {
                let v = self.cells[r * self.cols + c];
                if v != 0 {
                    stack.push(v);
                }
            }
            for r in (0..self.rows).rev() {
                let from_bottom = self.rows - 1 - r;
                self.cells[r * self.cols + c] = stack.get(from_bottom).copied().unwrap_or(0);
            }
        }
    }

    fn blank_pos(&self) -> Option<(i32, i32)> {
        let i = self.cells.iter().position(|c| *c == 0)?;
        Some(((i % self.cols) as i32, (i / self.cols) as i32))
    }

    fn idx(&self, col: i32, row: i32) -> Option<usize> {
        if col < 0 || row < 0 {
            return None;
        }
        let (c, r) = (col as usize, row as usize);
        if c >= self.cols || r >= self.rows {
            return None;
        }
        Some(r * self.cols + c)
    }

    fn cell_at_world(&self, x: f32, z: f32) -> Option<(i32, i32)> {
        let (ox, oz) = board_origin(self.cols, self.rows);
        let c = ((x - ox) / CELL + 0.5).floor() as i32;
        let r = ((z - oz) / CELL + 0.5).floor() as i32;
        self.idx(c, r).map(|_| (c, r))
    }

    fn tick_timers(&mut self, dt: f32) {
        let mut done = Vec::new();
        for t in &mut self.doc.timers {
            if !t.active {
                continue;
            }
            t.remaining = (t.remaining - dt).max(0.0);
            if t.remaining <= 1e-4 {
                t.active = false;
                if let Some(ev) = t.on_done.clone() {
                    done.push(ev);
                }
            }
        }
        for ev in done {
            self.record_event(&ev, None);
        }
    }

    fn progress_coins(&self) -> u32 {
        if self.phase == GamePhase::Complete {
            return LEVELS.len() as u32;
        }
        let mut n = self.level as u32;
        if self.level_clear {
            n += 1;
        }
        n
    }

    fn beat_name(&self) -> &'static str {
        match self.phase {
            GamePhase::Title => "player",
            GamePhase::Complete => "solved",
            GamePhase::Playing if self.level_clear => "cleared",
            GamePhase::Playing => "playing",
        }
    }

    fn rebuild(&mut self) {
        let events = self.doc.events.clone();
        let timers = self.doc.timers.clone();
        self.doc = self.picture();
        self.doc.events = events;
        self.doc.timers = timers;
    }

    fn picture(&self) -> WorldDoc {
        let (ox, oz) = board_origin(self.cols, self.rows);
        let mut props = Vec::new();
        props.push(prop(
            "prop:floor",
            "floor",
            [0.0, -0.28, 0.2],
            [10.0, 0.12, 10.0],
            [36, 40, 44],
            0.0,
            0.95,
        ));
        let board_w = self.cols as f32 * CELL + 0.85;
        let board_d = self.rows as f32 * CELL + 0.85;
        let mut board = prop(
            "prop:board",
            "board",
            [0.0, -0.02, 0.0],
            [board_w, 0.16, board_d],
            [112, 74, 48],
            0.04,
            0.82,
        );
        board.gltf = None;
        props.push(board);

        match self.kind {
            Kind::Slide => self.push_slide_tiles(&mut props, ox, oz),
            Kind::Grid => self.push_grid_cells(&mut props, ox, oz),
            Kind::Match => self.push_gems(&mut props, ox, oz),
        }

        if self.phase == GamePhase::Playing {
            let (cc, cr) = self.cursor;
            let (x, z) = cell_xz(ox, oz, cc, cr);
            let mut cursor = prop(
                "prop:cursor",
                "cursor",
                [x, 0.03, z],
                [1.02, 0.05, 1.02],
                [255, 210, 90],
                0.1,
                0.45,
            );
            cursor.gltf = None;
            props.push(cursor);
        }

        let flag_on = self.level_clear || self.phase == GamePhase::Complete;
        let mut flag = prop(
            "prop:flag",
            "flag",
            [3.15, 0.62, -2.55],
            [0.16, 0.92, 0.1],
            if flag_on {
                [70, 180, 110]
            } else {
                [70, 76, 72]
            },
            0.0,
            0.8,
        );
        flag.enabled = flag_on;
        flag.gltf = None;
        props.push(flag);

        let front_z = oz + (self.rows as f32 - 1.0) * CELL;
        let walker = walker(
            self.beat_name(),
            [1.85, 0.95, front_z + 1.35],
            std::f32::consts::PI,
        );
        WorldDoc {
            version: WORLD_DUMP_VERSION,
            genre: Some(GAME_ID.into()),
            half: 8.0,
            floor_y: 0.0,
            gravity: 9.8,
            coins: self.progress_coins(),
            player: Some(walker.clone()),
            props,
            walkers: vec![walker],
            lights: room_lights(),
            cameras: vec![room_camera()],
            ibl: Some(0.28),
            sky: Some([22, 26, 32, 255]),
            ground: Some(false),
            ..WorldDoc::default()
        }
    }

    fn push_slide_tiles(&self, props: &mut Vec<WorldProp>, ox: f32, oz: f32) {
        for r in 0..self.rows {
            for c in 0..self.cols {
                let v = self.cells[r * self.cols + c];
                let (x, z) = cell_xz(ox, oz, c as i32, r as i32);
                if v == 0 {
                    let mut hole = prop(
                        "prop:blank",
                        "blank",
                        [x, 0.05, z],
                        [0.78, 0.06, 0.78],
                        [24, 26, 30],
                        0.0,
                        0.9,
                    );
                    hole.gltf = None;
                    props.push(hole);
                    continue;
                }
                let mut tile = prop(
                    &format!("prop:tile-{v}"),
                    "tile",
                    [x, 0.46, z],
                    [0.78, 0.78, 0.78],
                    slide_color(v),
                    0.12,
                    0.55,
                );
                tile.model = "box".into();
                tile.gltf = Some("crate.glb".into());
                props.push(tile);
            }
        }
    }

    fn push_grid_cells(&self, props: &mut Vec<WorldProp>, ox: f32, oz: f32) {
        for r in 0..self.rows {
            for c in 0..self.cols {
                let on = self.cells[r * self.cols + c] != 0;
                let (x, z) = cell_xz(ox, oz, c as i32, r as i32);
                let (h, y, color) = if on {
                    (0.32, 0.22, [255, 186, 64])
                } else {
                    (0.12, 0.12, [42, 46, 52])
                };
                let mut cell = prop(
                    &format!("prop:cell-{c}-{r}"),
                    "cell",
                    [x, y, z],
                    [0.86, h, 0.86],
                    color,
                    if on { 0.05 } else { 0.0 },
                    if on { 0.35 } else { 0.9 },
                );
                cell.gltf = None;
                props.push(cell);
            }
        }
    }

    fn push_gems(&self, props: &mut Vec<WorldProp>, ox: f32, oz: f32) {
        for r in 0..self.rows {
            for c in 0..self.cols {
                let v = self.cells[r * self.cols + c];
                if v == 0 {
                    continue;
                }
                let (x, z) = cell_xz(ox, oz, c as i32, r as i32);
                let mut gem = prop(
                    &format!("prop:gem-{c}-{r}"),
                    "gem",
                    [x, 0.42, z],
                    [0.62, 0.62, 0.62],
                    gem_color(v),
                    0.08,
                    0.32,
                );
                gem.model = "sphere".into();
                gem.gltf = None;
                props.push(gem);
            }
        }
    }
}

fn axis(dx: i32, dy: i32) -> (i32, i32) {
    let dx = dx.signum();
    let dy = dy.signum();
    if dx != 0 && dy != 0 {
        (0, 0)
    } else {
        (dx, dy)
    }
}

fn board_origin(cols: usize, rows: usize) -> (f32, f32) {
    let ox = -((cols as f32) - 1.0) * CELL * 0.5;
    let oz = -((rows as f32) - 1.0) * CELL * 0.5;
    (ox, oz)
}

fn cell_xz(ox: f32, oz: f32, col: i32, row: i32) -> (f32, f32) {
    (ox + col as f32 * CELL, oz + row as f32 * CELL)
}

fn slide_color(id: u8) -> [u32; 3] {
    const PALETTE: [[u32; 3]; 16] = [
        [40, 44, 48],
        [214, 72, 64],
        [232, 140, 48],
        [236, 204, 72],
        [72, 176, 86],
        [40, 168, 158],
        [56, 132, 214],
        [96, 104, 204],
        [176, 92, 196],
        [214, 92, 140],
        [176, 120, 72],
        [96, 176, 204],
        [204, 164, 112],
        [132, 196, 96],
        [112, 128, 196],
        [196, 104, 72],
    ];
    PALETTE[(id as usize) % PALETTE.len()]
}

fn gem_color(id: u8) -> [u32; 3] {
    match id {
        1 => [214, 72, 68],
        2 => [64, 176, 96],
        3 => [64, 132, 214],
        4 => [232, 186, 64],
        5 => [168, 96, 204],
        _ => [180, 180, 186],
    }
}

fn prop(
    id: &str,
    name: &str,
    position: [f32; 3],
    scale: [f32; 3],
    color: [u32; 3],
    metallic: f32,
    roughness: f32,
) -> WorldProp {
    WorldProp {
        id: id.into(),
        kind: "prop".into(),
        name: name.into(),
        position,
        model: "box".into(),
        scale,
        enabled: true,
        color: Some(color),
        metallic,
        roughness,
        is_static: true,
        friction: 0.85,
        ..WorldProp::default()
    }
}

fn walker(name: &str, position: [f32; 3], yaw: f32) -> WorldWalker {
    WorldWalker {
        id: "walker:player".into(),
        kind: "walker".into(),
        name: name.into(),
        position,
        yaw,
        face: yaw,
        on_ground: true,
        anim: "idle".into(),
        expression: "blink".into(),
        ..WorldWalker::default()
    }
}

fn room_camera() -> WorldCamera {
    WorldCamera {
        id: "camera:main".into(),
        kind: "camera".into(),
        name: "board".into(),
        position: EYE,
        target: TARGET,
        fov: FOV_DEG,
    }
}

fn room_lights() -> Vec<WorldLight> {
    vec![
        light(0, "key", [-0.6, 4.6, 1.4], 1.35, 16.0, [1.0, 0.92, 0.78]),
        light(1, "fill", [2.6, 3.2, 3.2], 0.55, 14.0, [0.55, 0.72, 1.0]),
        light(2, "rim", [-2.4, 3.1, -2.2], 0.48, 12.0, [0.7, 0.95, 0.88]),
        light(3, "bounce", [0.2, 1.5, 2.4], 0.36, 10.0, [1.0, 0.78, 0.55]),
    ]
}

fn light(
    slot: u32,
    name: &str,
    position: [f32; 3],
    intensity: f32,
    radius: f32,
    color: [f32; 3],
) -> WorldLight {
    WorldLight {
        id: format!("light:{slot}"),
        kind_type: "light".into(),
        name: name.into(),
        position,
        kind: "point".into(),
        slot,
        intensity,
        radius,
        color: Some(color),
        ..WorldLight::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: &str = r#"{"version":1,"genre":"puzzle"}"#;

    fn started() -> PuzzleSession {
        let mut s = PuzzleSession::open(MIN).unwrap();
        s.confirm_action();
        s
    }

    #[test]
    fn slide_tutorial_solves_in_one_swipe() {
        let mut s = started();
        assert_eq!(s.kind_name(), "slide");
        assert!(!s.swipe(0, -1), "no tile above the blank");
        assert_eq!(s.moves(), 0);
        assert!(s.swipe(-1, 0));
        assert!(s.solved());
        assert_eq!(s.moves(), 1);
        assert!(s.slide_solved());
        assert_eq!(s.cells(), vec![1, 2, 3, 4, 5, 6, 7, 8, 0]);
    }

    #[test]
    fn retry_restores_the_level() {
        let mut s = started();
        assert!(s.swipe(0, 1));
        assert!(s.moves() >= 1);
        s.retry();
        assert_eq!(s.moves(), 0);
        assert!(!s.solved());
        assert_eq!(
            s.cells(),
            SLIDE_A.iter().map(|c| u32::from(*c)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn grid_taps_clear_the_lights() {
        let mut s = started();
        assert!(s.play_demo_move());
        s.confirm_action();
        assert_eq!(s.kind_name(), "grid");
        assert!(s.play_demo_move());
        assert!(!s.solved());
        assert!(s.play_demo_move());
        assert!(s.solved());
        assert!(s.cells().iter().all(|c| *c == 0));
    }

    fn at_level(index: usize) -> PuzzleSession {
        let mut s = started();
        while s.level() < index {
            if s.solved() {
                s.confirm_action();
            } else {
                assert!(s.play_demo_move());
            }
        }
        s
    }

    #[test]
    fn match_swap_clears_quota_and_rejects_a_dud() {
        let mut s = at_level(2);
        assert_eq!(s.kind_name(), "match");
        assert!(!s.swap_cells(2, 0, 2, 1), "swap that makes no line reverts");
        assert_eq!(s.moves(), 0);
        assert!(s.swap_cells(0, 0, 0, 1));
        assert!(s.solved());
        assert!(s.cleared() >= 6);
    }

    #[test]
    fn demo_clears_every_level() {
        let mut s = started();
        for _ in 0..16 {
            if s.won() {
                break;
            }
            if s.solved() {
                s.confirm_action();
            } else {
                assert!(
                    s.play_demo_move(),
                    "level {} kind {}",
                    s.level(),
                    s.kind_name()
                );
            }
        }
        assert!(s.won());
        assert_eq!(s.current_phase(), GamePhase::Complete);
        assert_eq!(s.progress_coins(), 4);
        let dump = s.world_json().unwrap();
        assert!(dump.contains("solved"));
        assert!(dump.contains("puzzle"));
        s.retry();
        assert_eq!(s.current_phase(), GamePhase::Playing);
        assert_eq!(s.kind_name(), "slide");
        assert_eq!(s.cols(), 4);
        assert!(!s.won());
    }

    #[test]
    fn stick_left_solves_the_tutorial() {
        let mut s = started();
        s.apply_input(WalkInput {
            lx: -1.0,
            lz: 0.0,
            jump: false,
            attack: false,
            dodge: false,
        });
        assert!(s.solved());
        s.apply_input(WalkInput {
            lx: -1.0,
            lz: 0.0,
            jump: false,
            attack: false,
            dodge: false,
        });
        assert_eq!(s.moves(), 1, "held stick must not repeat");
    }

    #[test]
    fn screen_center_hits_the_middle_cell() {
        let s = PuzzleSession::open(MIN).unwrap();
        assert_eq!(s.cell_at(160.0, 90.0, 320.0, 180.0), Some((1, 1)));
        assert_eq!(s.cell_at(-20.0, -20.0, 320.0, 180.0), None);
    }

    #[test]
    fn timer_fires_and_wrong_genre_is_rejected() {
        let mut s = PuzzleSession::open(MIN).unwrap();
        let id = s.begin_timer("hint", 0.05, Some("hint_ready"));
        assert_eq!(id, "timer:hint");
        s.step(0.1);
        let ev = s.drain_events("hint_ready");
        assert_eq!(ev.len(), 1);
        let err = PuzzleSession::open(include_str!("../tests/fixtures/puzzle_pad_world.json"))
            .unwrap_err();
        assert!(err.contains("puzzle"));
        let doc = WorldDoc::from_json(MIN).unwrap();
        assert!(!crate::puzzle::is_puzzle(&doc));
        let play = crate::world_play::WorldPlay::from_json(MIN).unwrap();
        assert!(!play.is_puzzle());
    }

    #[test]
    fn title_picture_matches_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/puzzle_world.json");
        let session = PuzzleSession::open(MIN).unwrap();
        let dumped = session.world_json().unwrap();
        let file = std::fs::read_to_string(&path).unwrap_or_default();
        let file_v: serde_json::Value = serde_json::from_str(&file).expect("puzzle_world.json");
        let dump_v: serde_json::Value = serde_json::from_str(&dumped).unwrap();
        assert_eq!(file_v, dump_v);
        assert_eq!(session.current_phase(), GamePhase::Title);
        assert_eq!(file_v["genre"], "puzzle");
        let tiles = file_v["props"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["name"] == "tile")
            .count();
        assert_eq!(tiles, 8);
        let scene = session.doc.compile_scene(16.0 / 9.0);
        assert!(scene.instance_count() >= 8, "n={}", scene.instance_count());
    }
}

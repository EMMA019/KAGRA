//! 4-lane rhythm chart (`genre: "rhythm"`).
//!
//! `rhythm_beat` (one marker on `WorldPlay`) is a different genre and stays
//! untouched. Timing windows, perfect / good / miss, combo, and score live
//! here. Python sends lane presses (keys D F J K or a screen tap) and draws
//! the dump. The chart is a fixed 120 BPM pattern; a bundled song is playback
//! only. No audio decoder, net, or Rapier. HUD copy is ASCII in the example.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::world_doc::{
    WorldCamera, WorldDoc, WorldEvent, WorldLight, WorldProp, WorldTimer, WorldWalker,
    WORLD_DUMP_VERSION,
};

pub const GAME_ID: &str = "rhythm";
pub const BPM: f32 = 120.0;
const LANES: u8 = 4;
const LANE_GAP: f32 = 1.15;
const JUDGE_Z: f32 = 1.45;
const SPAWN_Z: f32 = -6.35;
const APPROACH: f32 = 2.40;
const PERFECT: f32 = 0.050;
const GOOD: f32 = 0.100;
const TAIL: f32 = 0.55;
const NOTE_Y: f32 = 0.48;
const FLASH: f32 = 0.16;

const LANE_COLOR: [[u32; 3]; 4] = [
    [214, 72, 78],
    [64, 136, 214],
    [236, 196, 72],
    [72, 186, 104],
];

/// (lane, milliseconds). 120 BPM: a beat is 500 ms.
const CHART_MS: &[(u8, u32)] = &[
    (0, 800),
    (1, 1200),
    (2, 1600),
    (3, 2000),
    (0, 2400),
    (2, 2400),
    (1, 2800),
    (3, 2800),
    (0, 3200),
    (1, 3600),
    (2, 4000),
    (3, 4400),
    (3, 4800),
    (2, 5200),
    (1, 5600),
    (0, 6000),
    (0, 6400),
    (1, 6650),
    (2, 6900),
    (3, 7150),
    (1, 7600),
    (2, 8000),
    (0, 8600),
    (3, 8600),
    (0, 9400),
    (1, 9400),
    (2, 9400),
    (3, 9400),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Judge {
    Perfect,
    Good,
    Miss,
}

impl Judge {
    fn as_str(self) -> &'static str {
        match self {
            Judge::Perfect => "perfect",
            Judge::Good => "good",
            Judge::Miss => "miss",
        }
    }

    fn points(self) -> u32 {
        match self {
            Judge::Perfect => 300,
            Judge::Good => 100,
            Judge::Miss => 0,
        }
    }
}

#[derive(Clone, Debug)]
struct Note {
    lane: u8,
    time: f32,
    judged: Option<Judge>,
}

/// One short 4-lane chart. Picture is a `WorldDoc` (`genre = rhythm`).
#[derive(Clone, Debug)]
pub struct RhythmSession {
    doc: WorldDoc,
    /// Last host stick. Lane hits use `press_lane`, not this stick.
    #[allow(dead_code)]
    input: WalkInput,
    phase: GamePhase,
    notes: Vec<Note>,
    time: f32,
    score: u32,
    combo: u32,
    max_combo: u32,
    perfect: u32,
    good: u32,
    miss: u32,
    last: Option<Judge>,
    last_lane: Option<u8>,
    flash: f32,
    auto: bool,
    event_seq: u32,
}

impl RhythmSession {
    /// Load a dump. `genre` must be [`GAME_ID`]. The chart is built in, not read
    /// from the dump (the dump is the picture).
    pub fn open(json: &str) -> Result<Self, String> {
        let doc = WorldDoc::from_json(json)?;
        if doc.genre.as_deref() != Some(GAME_ID) {
            return Err(format!(
                "WorldDoc.genre must be {GAME_ID:?}, got {:?}",
                doc.genre
            ));
        }
        let mut session = Self {
            doc,
            input: WalkInput::default(),
            phase: GamePhase::Title,
            notes: Vec::new(),
            time: 0.0,
            score: 0,
            combo: 0,
            max_combo: 0,
            perfect: 0,
            good: 0,
            miss: 0,
            last: None,
            last_lane: None,
            flash: 0.0,
            auto: false,
            event_seq: 0,
        };
        session.reset_chart();
        session.phase = GamePhase::Title;
        session.rebuild();
        Ok(session)
    }

    pub fn apply_input(&mut self, input: WalkInput) {
        self.input = input.clamped();
    }

    pub fn step(&mut self, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.tick_timers(dt);
        if self.phase == GamePhase::Playing {
            self.time += dt;
            self.flash = (self.flash - dt).max(0.0);
            if self.auto {
                self.auto_hit();
            }
            self.expire();
            self.finish_if_done();
        }
        self.rebuild();
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
                self.reset_chart();
                self.phase = GamePhase::Playing;
            }
            GamePhase::Complete => {
                self.reset_chart();
                self.phase = GamePhase::Title;
            }
            GamePhase::Playing => {}
        }
        self.rebuild();
    }

    /// Restart the chart in play. Keeps auto-play.
    pub fn retry(&mut self) {
        let auto = self.auto;
        self.reset_chart();
        self.auto = auto;
        self.phase = GamePhase::Playing;
        self.rebuild();
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

    /// Lane hits are [`Self::press_lane`], not prop interact.
    pub fn use_nearby(&mut self) {}

    pub fn current_phase(&self) -> GamePhase {
        self.phase
    }

    pub fn set_auto(&mut self, on: bool) {
        self.auto = on;
    }

    pub fn score(&self) -> u32 {
        self.score
    }

    pub fn combo(&self) -> u32 {
        self.combo
    }

    pub fn max_combo(&self) -> u32 {
        self.max_combo
    }

    pub fn perfect_count(&self) -> u32 {
        self.perfect
    }

    pub fn good_count(&self) -> u32 {
        self.good
    }

    pub fn miss_count(&self) -> u32 {
        self.miss
    }

    pub fn note_count(&self) -> u32 {
        self.notes.len() as u32
    }

    pub fn lane_count(&self) -> u32 {
        u32::from(LANES)
    }

    pub fn song_time(&self) -> f32 {
        self.time
    }

    pub fn song_length(&self) -> f32 {
        self.notes.iter().map(|n| n.time).fold(0.0, f32::max) + TAIL
    }

    pub fn bpm(&self) -> f32 {
        BPM
    }

    pub fn last_judge(&self) -> &'static str {
        self.last.map(Judge::as_str).unwrap_or("")
    }

    /// S / A / B / C from perfect=300, good=100, miss=0 over the whole chart.
    pub fn rank(&self) -> &'static str {
        if self.notes.is_empty() {
            return "C";
        }
        let notes = self.notes.len() as f32;
        let got = self.perfect as f32 * 300.0 + self.good as f32 * 100.0;
        let acc = got / (notes * 300.0);
        if acc >= 0.95 {
            "S"
        } else if acc >= 0.85 {
            "A"
        } else if acc >= 0.70 {
            "B"
        } else {
            "C"
        }
    }

    /// Edge-triggered lane hit. Returns `perfect`, `good`, or `""` (no note in window).
    pub fn press_lane(&mut self, lane: u32) -> &'static str {
        if self.phase != GamePhase::Playing || lane >= u32::from(LANES) {
            return "";
        }
        let Some(index) = self.closest_open(lane as u8, GOOD) else {
            return "";
        };
        let dt = (self.time - self.notes[index].time).abs();
        let kind = if dt <= PERFECT {
            Judge::Perfect
        } else {
            Judge::Good
        };
        self.apply_judge(index, kind);
        self.rebuild();
        kind.as_str()
    }

    /// Screen tap. Title and result confirm. Play maps `x` across 4 lanes.
    pub fn pointer(&mut self, x: f32, _y: f32, width: f32, _height: f32) -> i32 {
        match self.phase {
            GamePhase::Title | GamePhase::Complete => {
                self.confirm_action();
                -1
            }
            GamePhase::Playing => {
                let Some(lane) = lane_from_x(x, width) else {
                    return -1;
                };
                self.press_lane(u32::from(lane));
                i32::from(lane)
            }
        }
    }

    fn reset_chart(&mut self) {
        self.notes = chart_notes();
        self.time = 0.0;
        self.score = 0;
        self.combo = 0;
        self.max_combo = 0;
        self.perfect = 0;
        self.good = 0;
        self.miss = 0;
        self.last = None;
        self.last_lane = None;
        self.flash = 0.0;
        self.input = WalkInput::default();
        self.doc.events.clear();
        self.event_seq = 0;
    }

    fn auto_hit(&mut self) {
        for lane in 0..LANES {
            if let Some(index) = self.closest_open(lane, PERFECT) {
                self.apply_judge(index, Judge::Perfect);
            }
        }
    }

    fn expire(&mut self) {
        let late: Vec<usize> = self
            .notes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                if n.judged.is_none() && self.time > n.time + GOOD {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        for index in late {
            self.apply_judge(index, Judge::Miss);
        }
    }

    fn finish_if_done(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }
        let length = self.song_length();
        let all_judged = self.notes.iter().all(|n| n.judged.is_some());
        if self.time < length || !all_judged {
            return;
        }
        self.phase = GamePhase::Complete;
        self.record_event(
            "clear",
            Some(serde_json::json!({
                "score": self.score,
                "rank": self.rank(),
                "combo": self.max_combo,
            })),
        );
    }

    fn closest_open(&self, lane: u8, window: f32) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, note) in self.notes.iter().enumerate() {
            if note.lane != lane || note.judged.is_some() {
                continue;
            }
            let dt = (self.time - note.time).abs();
            if dt <= window && best.is_none_or(|(_, prev)| dt < prev) {
                best = Some((i, dt));
            }
        }
        best.map(|(i, _)| i)
    }

    fn apply_judge(&mut self, index: usize, kind: Judge) {
        let Some(note) = self.notes.get_mut(index) else {
            return;
        };
        if note.judged.is_some() {
            return;
        }
        let lane = note.lane;
        note.judged = Some(kind);
        match kind {
            Judge::Perfect => self.perfect += 1,
            Judge::Good => self.good += 1,
            Judge::Miss => self.miss += 1,
        }
        if kind == Judge::Miss {
            self.combo = 0;
        } else {
            self.combo = self.combo.saturating_add(1);
            self.score = self.score.saturating_add(kind.points());
        }
        self.max_combo = self.max_combo.max(self.combo);
        self.last = Some(kind);
        self.last_lane = Some(lane);
        self.flash = FLASH;
        self.record_event(
            "judge",
            Some(serde_json::json!({
                "lane": lane,
                "judge": kind.as_str(),
                "combo": self.combo,
                "score": self.score,
            })),
        );
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

    fn rebuild(&mut self) {
        let events = std::mem::take(&mut self.doc.events);
        let timers = std::mem::take(&mut self.doc.timers);
        self.doc = self.picture();
        self.doc.events = events;
        self.doc.timers = timers;
    }

    fn picture(&self) -> WorldDoc {
        let mut props = Vec::new();
        props.push(prop(
            "prop:floor",
            "floor",
            [0.0, -0.12, -1.2],
            [16.0, 0.12, 18.0],
            [32, 28, 44],
            0.02,
            0.92,
        ));
        let span = JUDGE_Z - SPAWN_Z;
        let mid_z = (JUDGE_Z + SPAWN_Z) * 0.5;
        for lane in 0..LANES {
            let x = lane_x(lane);
            let strip = darken(LANE_COLOR[lane as usize]);
            props.push(prop(
                &format!("prop:lane-{lane}"),
                "lane",
                [x, 0.02, mid_z],
                [0.92, 0.05, span + 0.35],
                strip,
                0.04,
                0.88,
            ));
        }
        props.push(prop(
            "prop:judge",
            "judge",
            [0.0, 0.08, JUDGE_Z],
            [LANE_GAP * f32::from(LANES) + 0.35, 0.08, 0.16],
            [236, 232, 244],
            0.35,
            0.28,
        ));
        for lane in 0..LANES {
            let x = lane_x(lane);
            let color = self.receptor_color(lane);
            let scale = if self.flash > 0.0 && self.last_lane == Some(lane) {
                0.78
            } else {
                0.62
            };
            props.push(with_gltf(
                prop(
                    &format!("prop:receptor-{lane}"),
                    "receptor",
                    [x, 0.28, JUDGE_Z],
                    [scale, scale, scale],
                    color,
                    0.08,
                    0.45,
                ),
                "chest",
            ));
        }
        for (i, note) in self.notes.iter().enumerate() {
            let Some(pos) = self.note_position(note) else {
                continue;
            };
            props.push(with_gltf(
                prop(
                    &format!("prop:note-{i}"),
                    "note",
                    pos,
                    [0.52, 0.52, 0.52],
                    LANE_COLOR[note.lane as usize],
                    0.12,
                    0.48,
                ),
                "crate.glb",
            ));
        }
        let clear = self.phase == GamePhase::Complete;
        let mut flag = prop(
            "prop:flag",
            "flag",
            [3.35, 0.85, 2.4],
            [0.16, 1.15, 0.1],
            if clear { [70, 180, 110] } else { [70, 76, 72] },
            0.0,
            0.8,
        );
        flag.enabled = clear;
        props.push(flag);
        props.push(with_gltf(
            prop(
                "prop:inn",
                "inn",
                [0.0, 0.0, -9.2],
                [0.42, 0.42, 0.42],
                [186, 164, 140],
                0.02,
                0.8,
            ),
            "inn",
        ));
        props.push(with_gltf(
            prop(
                "prop:tree",
                "tree",
                [-4.6, 0.0, -2.4],
                [0.72, 0.72, 0.72],
                [64, 130, 72],
                0.0,
                0.85,
            ),
            "stylized_tree",
        ));
        props.push(with_gltf(
            prop(
                "prop:pine",
                "pine",
                [4.7, 0.0, -3.1],
                [0.62, 0.62, 0.62],
                [48, 96, 72],
                0.0,
                0.85,
            ),
            "stylized_pine",
        ));
        props.push(with_gltf(
            prop(
                "prop:rock",
                "rock",
                [-4.2, 0.15, 2.8],
                [0.48, 0.36, 0.48],
                [140, 136, 128],
                0.02,
                0.9,
            ),
            "rock",
        ));
        props.push(with_gltf(
            prop(
                "prop:crate-side",
                "crate",
                [3.6, 0.38, 3.3],
                [0.7, 0.7, 0.7],
                [168, 122, 78],
                0.08,
                0.62,
            ),
            "crate.glb",
        ));

        let walker = walker(self.walker_name(), [0.15, 0.0, 4.55], std::f32::consts::PI);
        WorldDoc {
            version: WORLD_DUMP_VERSION,
            genre: Some(GAME_ID.into()),
            half: 16.0,
            floor_y: 0.0,
            gravity: 9.8,
            coins: self.score,
            player: Some(walker.clone()),
            props,
            walkers: vec![walker],
            lights: stage_lights(),
            cameras: vec![stage_camera()],
            ibl: Some(0.42),
            exposure: Some(1.12),
            sky: Some([14, 12, 24, 255]),
            ground: Some(false),
            ..WorldDoc::default()
        }
    }

    fn walker_name(&self) -> &'static str {
        match self.phase {
            GamePhase::Title => "player",
            GamePhase::Complete => "clear",
            GamePhase::Playing => self.last.map(Judge::as_str).unwrap_or("play"),
        }
    }

    fn receptor_color(&self, lane: u8) -> [u32; 3] {
        if self.flash > 0.0 && self.last_lane == Some(lane) {
            match self.last {
                Some(Judge::Perfect) => [255, 244, 214],
                Some(Judge::Good) => [186, 230, 150],
                Some(Judge::Miss) => [220, 72, 68],
                None => LANE_COLOR[lane as usize],
            }
        } else {
            LANE_COLOR[lane as usize]
        }
    }

    fn note_position(&self, note: &Note) -> Option<[f32; 3]> {
        if note.judged.is_some() {
            return None;
        }
        let start = note.time - APPROACH;
        if self.time + 1.0e-4 < start || self.time > note.time + GOOD {
            return None;
        }
        let frac = ((self.time - start) / APPROACH).clamp(0.0, 1.15);
        let z = SPAWN_Z + (JUDGE_Z - SPAWN_Z) * frac;
        Some([lane_x(note.lane), NOTE_Y, z])
    }
}

fn chart_notes() -> Vec<Note> {
    CHART_MS
        .iter()
        .map(|&(lane, ms)| Note {
            lane,
            time: ms as f32 / 1000.0,
            judged: None,
        })
        .collect()
}

fn lane_x(lane: u8) -> f32 {
    (f32::from(lane) - (f32::from(LANES) - 1.0) * 0.5) * LANE_GAP
}

fn lane_from_x(x: f32, width: f32) -> Option<u8> {
    if width <= 1.0 || !x.is_finite() {
        return None;
    }
    let u = (x / width).clamp(0.0, 0.999_999);
    let lane = (u * f32::from(LANES)).floor();
    if (0.0..f32::from(LANES)).contains(&lane) {
        Some(lane as u8)
    } else {
        None
    }
}

fn darken(c: [u32; 3]) -> [u32; 3] {
    [c[0] / 4, c[1] / 4, c[2] / 4]
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

fn with_gltf(mut item: WorldProp, gltf: &str) -> WorldProp {
    if gltf != "crate.glb" {
        item.model = gltf.into();
    }
    item.gltf = Some(gltf.into());
    item
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
        model: "knight".into(),
        gltf: Some("knight".into()),
        anim: "idle".into(),
        expression: "blink".into(),
        ..WorldWalker::default()
    }
}

fn stage_camera() -> WorldCamera {
    WorldCamera {
        id: "camera:main".into(),
        kind: "camera".into(),
        name: "stage".into(),
        position: [0.0, 3.55, 7.15],
        target: [0.0, 0.55, -0.4],
        fov: 42.0,
    }
}

fn stage_lights() -> Vec<WorldLight> {
    vec![
        light(0, "key", [-1.2, 5.2, 3.4], 1.45, 18.0, [1.0, 0.9, 0.78]),
        light(1, "fill", [2.8, 3.4, 2.2], 0.55, 14.0, [0.45, 0.62, 1.0]),
        light(2, "rim", [-2.6, 3.2, -3.4], 0.62, 16.0, [0.85, 0.45, 0.95]),
        light(3, "lane", [0.2, 1.8, 1.2], 0.7, 8.0, [1.0, 0.82, 0.55]),
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
    use crate::game::GamePhase;
    use crate::world_doc::WorldDoc;

    const MIN: &str = r#"{"version":1,"genre":"rhythm"}"#;

    fn open() -> RhythmSession {
        RhythmSession::open(MIN).unwrap()
    }

    fn playing() -> RhythmSession {
        let mut s = open();
        s.confirm_action();
        s
    }

    #[test]
    fn title_holds_and_rejects_other_genres() {
        let mut s = open();
        assert_eq!(s.current_phase(), GamePhase::Title);
        assert_eq!(s.bpm(), 120.0);
        assert_eq!(s.lane_count(), 4);
        assert!(s.note_count() >= 16);
        s.press_lane(0);
        s.step(1.0);
        assert_eq!(s.song_time(), 0.0);
        assert_eq!(s.perfect_count(), 0);
        assert_eq!(s.current_phase(), GamePhase::Title);
        let err = RhythmSession::open(include_str!("../tests/fixtures/rhythm_beat_world.json"))
            .unwrap_err();
        assert!(err.contains("rhythm_beat"));
        assert!(!crate::rhythm::is_rhythm(&s.doc));
        let doc = WorldDoc::from_json(&s.world_json().unwrap()).unwrap();
        assert!(doc.genre_is(GAME_ID));
        assert!(!crate::rhythm::is_rhythm(&doc));
        assert_eq!(doc.props.iter().filter(|p| p.name == "lane").count(), 4);
        assert_eq!(
            doc.props.iter().filter(|p| p.name == "note").count(),
            6,
            "title highway shows the notes already inside the approach window"
        );
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "note" && p.gltf.as_deref() == Some("crate.glb")));
        assert!(doc
            .props
            .iter()
            .any(|p| p.name == "receptor" && p.gltf.as_deref() == Some("chest")));
        assert_eq!(doc.player.as_ref().unwrap().gltf.as_deref(), Some("knight"));
        assert!(doc.player.as_ref().unwrap().on_ground);
        let flag = doc.props.iter().find(|p| p.name == "flag").unwrap();
        assert!(!flag.enabled);
        assert_eq!(doc.lights.len(), 4);
        assert_eq!(doc.coins, 0);
    }

    #[test]
    fn perfect_good_and_miss_windows() {
        let mut s = playing();
        s.step(0.8);
        assert_eq!(s.press_lane(0), "perfect");
        assert_eq!(s.perfect_count(), 1);
        assert_eq!(s.combo(), 1);
        assert_eq!(s.score(), 300);
        assert_eq!(s.last_judge(), "perfect");

        let mut s = playing();
        s.step(0.8);
        s.step(0.08);
        assert_eq!(s.press_lane(0), "good");
        assert_eq!(s.good_count(), 1);
        assert_eq!(s.score(), 100);
        assert_eq!(s.combo(), 1);

        let mut s = playing();
        s.step(0.2);
        assert_eq!(s.press_lane(0), "");
        assert_eq!(s.miss_count(), 0);
        s.step(0.6);
        assert_eq!(s.press_lane(0), "perfect");

        let mut s = playing();
        s.step(0.8);
        assert_eq!(s.press_lane(0), "perfect");
        s.step(0.55);
        assert_eq!(s.miss_count(), 1, "the 1.2s note must expire");
        assert_eq!(s.combo(), 0);
        assert_eq!(s.max_combo(), 1);
        assert_eq!(s.last_judge(), "miss");
    }

    #[test]
    fn chord_and_screen_columns() {
        let mut s = playing();
        s.step(2.4);
        assert_eq!(s.press_lane(0), "perfect");
        assert_eq!(s.press_lane(2), "perfect");
        assert_eq!(s.combo(), 2);
        assert_eq!(s.pointer(10.0, 20.0, 800.0, 450.0), 0);
        assert_eq!(s.pointer(600.0, 20.0, 800.0, 450.0), 3);
        let mut title = open();
        assert_eq!(title.pointer(20.0, 20.0, 400.0, 200.0), -1);
        assert_eq!(title.current_phase(), GamePhase::Playing);
    }

    #[test]
    fn auto_play_is_a_full_clear() {
        let mut s = playing();
        s.set_auto(true);
        for _ in 0..900 {
            if s.current_phase() == GamePhase::Complete {
                break;
            }
            s.step(1.0 / 60.0);
        }
        assert_eq!(s.current_phase(), GamePhase::Complete);
        assert_eq!(s.miss_count(), 0);
        assert_eq!(s.good_count(), 0);
        assert_eq!(s.perfect_count(), s.note_count());
        assert_eq!(s.max_combo(), s.note_count());
        assert_eq!(s.score(), 300 * s.note_count());
        assert_eq!(s.rank(), "S");
        let doc = WorldDoc::from_json(&s.world_json().unwrap()).unwrap();
        assert_eq!(doc.player.as_ref().unwrap().name, "clear");
        assert!(doc.props.iter().any(|p| p.name == "flag" && p.enabled));
        assert_eq!(doc.coins, s.score());
        assert_eq!(s.drain_events("clear").len(), 1);
        s.confirm_action();
        assert_eq!(s.current_phase(), GamePhase::Title);
        assert_eq!(s.score(), 0);
        assert_eq!(s.perfect_count(), 0);
    }

    #[test]
    fn timer_emits_on_done() {
        let mut s = open();
        let id = s.begin_timer("beat", 0.05, Some("tick"));
        assert_eq!(id, "timer:beat");
        s.step(0.1);
        assert_eq!(s.drain_events("tick").len(), 1);
    }
}

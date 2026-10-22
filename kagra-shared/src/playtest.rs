//! In-game playtest handoff. Debug / preview only.
//!
//! A hidden gesture (three-finger long-press, or F12) pauses the sim and
//! opens a small panel. The panel collects a free-text note (Japanese is
//! stored as UTF-8; hosts that can show an IME field push the composed
//! string in), `GenreSession::dump` checkpoints, and the input log since
//! the oldest checkpoint still inside ~10 s.
//!
//! Sending does not embed a token. The host opens
//! `https://github.com/EMMA019/KAGRA/issues/new?template=playtest.yml`
//! with the note, build, and either the state or a short hint. When the
//! URL would be too long, the state is a zlib+base64 block the player
//! pastes into the issue. Screenshots stay out of the URL; the host saves
//! one and tells the player to paste it.
//!
//! `scripts/repro_issue.py` decodes the same block, rebuilds the session
//! from the dump, and replays `walk` / `confirm` events.

use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use serde_json::{json, Value};

use crate::input::{PointerEvent, PointerPhase};
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};
use crate::KAGRA_SHARED_VERSION;

pub const ISSUE_NEW: &str = "https://github.com/EMMA019/KAGRA/issues/new";
pub const BLOCK_HEADER: &str = "KAGRA-PLAYTEST-1";
pub const URL_LIMIT: usize = 6000;
pub const HOLD_SEC: f32 = 0.6;
pub const SNAP_EVERY: f32 = 1.0;
pub const WINDOW_SEC: f32 = 10.5;
pub const NOTE_MAX: usize = 400;

/// True for a local debug binary unless `KAGRA_PLAYTEST` says otherwise.
/// Packaged release builds stay off until the host opts in (debug APK,
/// browser preview, Windows playtest zip).
pub fn host_default_enabled() -> bool {
    match std::env::var("KAGRA_PLAYTEST") {
        Ok(v) => matches!(
            v.trim(),
            "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
        ),
        Err(_) => cfg!(debug_assertions),
    }
}

pub fn profile_name() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

fn target_os() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "wasm"
    } else {
        std::env::consts::OS
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PanelLayout {
    pub scale: f32,
    pub panel: Rect,
    pub note: Rect,
    pub send: Rect,
    pub copy: Rect,
    pub close: Rect,
}

/// Keep this in lockstep with `kagra.playtest.layout`.
pub fn layout(width: u32, height: u32) -> PanelLayout {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.55, 1.6);
    let pw = (440.0 * scale).min(w - 24.0).max(120.0);
    let ph = (300.0 * scale).min(h - 24.0).max(100.0);
    let x = ((w - pw) * 0.5).max(0.0);
    let y = ((h - ph) * 0.5).max(0.0);
    let pad = 14.0 * scale;
    let title_h = 28.0 * scale;
    let note_h = 72.0 * scale;
    let btn_h = 40.0 * scale;
    let gap = 8.0 * scale;
    let inner_w = (pw - pad * 2.0).max(40.0);
    let btn_w = (inner_w - gap * 2.0) / 3.0;
    let note = Rect {
        x: x + pad,
        y: y + pad + title_h,
        w: inner_w,
        h: note_h,
    };
    let by = y + ph - pad - btn_h;
    let send = Rect {
        x: x + pad,
        y: by,
        w: btn_w,
        h: btn_h,
    };
    let copy = Rect {
        x: send.x + btn_w + gap,
        y: by,
        w: btn_w,
        h: btn_h,
    };
    let close = Rect {
        x: copy.x + btn_w + gap,
        y: by,
        w: btn_w,
        h: btn_h,
    };
    PanelLayout {
        scale,
        panel: Rect { x, y, w: pw, h: ph },
        note,
        send,
        copy,
        close,
    }
}

#[derive(Clone, Debug)]
pub struct IssuePacket {
    pub url: String,
    pub block: String,
    pub fits: bool,
    pub copy: String,
    pub instruction: String,
    pub title: String,
    pub action: String,
}

#[derive(Debug)]
struct Finger {
    id: u32,
    x0: f32,
    y0: f32,
    x: f32,
    y: f32,
}

#[derive(Debug)]
struct Snap {
    t: f32,
    dump: String,
}

#[derive(Debug)]
enum LogEvent {
    Walk {
        seq: u32,
        t: f32,
        dt: f32,
        lx: f32,
        lz: f32,
        jump: bool,
        attack: bool,
        dodge: bool,
    },
    Pointer {
        seq: u32,
        t: f32,
        id: u32,
        x: f32,
        y: f32,
        phase: &'static str,
        pressure: f32,
    },
    Key {
        seq: u32,
        t: f32,
        name: String,
        down: bool,
    },
    Confirm {
        seq: u32,
        t: f32,
    },
}

impl LogEvent {
    fn t(&self) -> f32 {
        match self {
            Self::Walk { t, .. }
            | Self::Pointer { t, .. }
            | Self::Key { t, .. }
            | Self::Confirm { t, .. } => *t,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Walk {
                seq,
                t,
                dt,
                lx,
                lz,
                jump,
                attack,
                dodge,
            } => json!({
                "seq": seq, "t": t, "dt": dt, "kind": "walk",
                "lx": lx, "lz": lz, "jump": jump, "attack": attack, "dodge": dodge,
            }),
            Self::Pointer {
                seq,
                t,
                id,
                x,
                y,
                phase,
                pressure,
            } => json!({
                "seq": seq, "t": t, "kind": "pointer",
                "id": id, "x": x, "y": y, "phase": phase, "pressure": pressure,
            }),
            Self::Key { seq, t, name, down } => json!({
                "seq": seq, "t": t, "kind": "key", "name": name, "down": down,
            }),
            Self::Confirm { seq, t } => json!({
                "seq": seq, "t": t, "kind": "confirm",
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct WalkAxes {
    lx: f32,
    lz: f32,
    jump: bool,
    attack: bool,
    dodge: bool,
}

/// Gesture, input ring, note, and the GitHub handoff.
#[derive(Debug)]
pub struct Playtest {
    enabled: bool,
    open: bool,
    external_chrome: bool,
    holding: bool,
    hold_t: f32,
    clock: f32,
    capture_t: f32,
    last_f12: f32,
    view_w: u32,
    view_h: u32,
    note: String,
    preedit: String,
    status: String,
    device_label: String,
    lang: String,
    shot: bool,
    shot_path: String,
    packet_seq: u32,
    packet_action: String,
    packet: Option<IssuePacket>,
    fingers: Vec<Finger>,
    snaps: Vec<Snap>,
    events: Vec<LogEvent>,
    seq: u32,
    pending: WalkAxes,
}

impl Playtest {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            open: false,
            external_chrome: false,
            holding: false,
            hold_t: 0.0,
            clock: 0.0,
            capture_t: 0.0,
            last_f12: -1.0,
            view_w: 1280,
            view_h: 720,
            note: String::new(),
            preedit: String::new(),
            status: String::new(),
            device_label: String::new(),
            lang: String::new(),
            shot: false,
            shot_path: String::new(),
            packet_seq: 0,
            packet_action: String::new(),
            packet: None,
            fingers: Vec::new(),
            snaps: Vec::new(),
            events: Vec::new(),
            seq: 0,
            pending: WalkAxes::default(),
        }
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
        if !on {
            self.close();
            self.events.clear();
            self.snaps.clear();
        }
    }

    pub fn set_external_chrome(&mut self, on: bool) {
        self.external_chrome = on;
    }

    pub fn set_device(&mut self, label: impl Into<String>, lang: impl Into<String>) {
        self.device_label = label.into();
        self.lang = lang.into();
    }

    pub fn set_viewport(&mut self, w: u32, h: u32) {
        self.view_w = w.max(1);
        self.view_h = h.max(1);
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Game ticks and fresh input should wait. True while the long-press
    /// is in progress and while the panel is up.
    pub fn blocks_sim(&self) -> bool {
        self.enabled && (self.open || self.holding)
    }

    pub fn advance(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        let dt = dt.max(0.0);
        self.clock += dt;
        if self.holding && !self.open {
            self.hold_t += dt;
            if self.hold_t >= HOLD_SEC {
                self.try_open();
            }
        }
    }

    pub fn on_pointer(&mut self, ev: PointerEvent) -> bool {
        if !self.enabled {
            return false;
        }
        if self.open {
            if !self.external_chrome && matches!(ev.phase, PointerPhase::Begin) {
                self.hit(ev.x, ev.y);
            }
            return true;
        }
        match ev.phase {
            PointerPhase::Begin => {
                self.fingers.retain(|f| f.id != ev.id);
                self.fingers.push(Finger {
                    id: ev.id,
                    x0: ev.x,
                    y0: ev.y,
                    x: ev.x,
                    y: ev.y,
                });
            }
            PointerPhase::Move => {
                if let Some(f) = self.fingers.iter_mut().find(|f| f.id == ev.id) {
                    f.x = ev.x;
                    f.y = ev.y;
                }
            }
            PointerPhase::End | PointerPhase::Cancel => {
                self.fingers.retain(|f| f.id != ev.id);
            }
        }
        let slop = (self.view_w.min(self.view_h) as f32 * 0.04).max(36.0);
        let stable = self.fingers.len() >= 3
            && self.fingers.iter().all(|f| {
                let dx = f.x - f.x0;
                let dy = f.y - f.y0;
                dx * dx + dy * dy <= slop * slop
            });
        if stable {
            if !self.holding {
                self.holding = true;
                self.hold_t = 0.0;
            }
            return true;
        }
        self.holding = false;
        self.hold_t = 0.0;
        false
    }

    pub fn on_key(&mut self, name: &str, down: bool) -> bool {
        if !self.enabled || name.is_empty() {
            return false;
        }
        if name.eq_ignore_ascii_case("F12") {
            if !down {
                return true;
            }
            if self.last_f12 >= 0.0 && (self.clock - self.last_f12) < 0.25 {
                return true;
            }
            self.last_f12 = self.clock;
            if self.open {
                self.close();
            } else {
                self.try_open();
            }
            return true;
        }
        if self.open {
            if !down {
                return true;
            }
            if name.eq_ignore_ascii_case("escape") {
                self.close();
            } else if name.eq_ignore_ascii_case("backspace") {
                self.backspace();
            } else if name.eq_ignore_ascii_case("enter") || name.eq_ignore_ascii_case("return") {
                self.push_text("\n");
            }
            return true;
        }
        self.push_log(LogEvent::Key {
            seq: 0,
            t: self.clock,
            name: name.to_string(),
            down,
        });
        false
    }

    pub fn push_text(&mut self, text: &str) {
        if !self.open {
            return;
        }
        for ch in text.chars() {
            if ch == '\u{7f}' {
                self.backspace();
                continue;
            }
            if ch.is_control() && ch != '\n' {
                continue;
            }
            self.note.push(ch);
        }
        self.trim_note();
    }

    pub fn set_note(&mut self, note: &str) {
        self.note = note.to_string();
        self.trim_note();
    }

    pub fn set_preedit(&mut self, text: impl Into<String>) {
        self.preedit = text.into();
    }

    pub fn note_walk(&mut self, lx: f32, lz: f32, jump: bool, attack: bool, dodge: bool) {
        if !self.enabled || self.blocks_sim() {
            return;
        }
        self.pending = WalkAxes {
            lx,
            lz,
            jump,
            attack,
            dodge,
        };
    }

    pub fn commit_walk(&mut self, dt: f32) {
        if !self.enabled || self.blocks_sim() || dt <= 0.0 {
            return;
        }
        let w = self.pending;
        self.push_log(LogEvent::Walk {
            seq: 0,
            t: self.clock,
            dt,
            lx: w.lx,
            lz: w.lz,
            jump: w.jump,
            attack: w.attack,
            dodge: w.dodge,
        });
    }

    pub fn record_pointer(&mut self, ev: PointerEvent) {
        if !self.enabled || self.open {
            return;
        }
        self.push_log(LogEvent::Pointer {
            seq: 0,
            t: self.clock,
            id: ev.id,
            x: ev.x,
            y: ev.y,
            phase: ev.phase.as_str(),
            pressure: ev.pressure,
        });
    }

    pub fn record_confirm(&mut self) {
        if !self.enabled || self.open {
            return;
        }
        self.push_log(LogEvent::Confirm {
            seq: 0,
            t: self.clock,
        });
    }

    pub fn wants_snapshot(&self) -> bool {
        if !self.enabled || self.open {
            return false;
        }
        match self.snaps.last() {
            None => true,
            Some(s) => self.clock - s.t >= SNAP_EVERY - 1e-3,
        }
    }

    pub fn snapshot_dump(&mut self, dump: &str) {
        if !self.wants_snapshot() || dump.trim().is_empty() {
            return;
        }
        self.push_snap(compact_json(dump));
    }

    /// Record `dump` even if a snapshot was just taken. A same-timestamp
    /// snap is replaced, so enabling the overlay before `load_game_json`
    /// still checkpoints the world at t=0 instead of the empty stats.
    pub fn snapshot_loaded(&mut self, dump: &str) {
        if !self.enabled || dump.trim().is_empty() {
            return;
        }
        let compact = compact_json(dump);
        if let Some(last) = self.snaps.last_mut() {
            if (self.clock - last.t).abs() <= 1e-3 {
                last.dump = compact;
                return;
            }
        }
        self.push_snap(compact);
    }

    fn push_snap(&mut self, dump: String) {
        self.snaps.push(Snap {
            t: self.clock,
            dump,
        });
        let cutoff = self.clock - WINDOW_SEC;
        self.snaps.retain(|s| s.t >= cutoff - 1e-3);
        if self.snaps.len() > 12 {
            let drop = self.snaps.len() - 12;
            self.snaps.drain(0..drop);
        }
        if let Some(oldest) = self.snaps.first().map(|s| s.t) {
            self.events.retain(|e| e.t() >= oldest - 1e-3);
        }
        if self.events.len() > 4000 {
            let drop = self.events.len() - 4000;
            self.events.drain(0..drop);
        }
    }

    pub fn packet(&self) -> Option<&IssuePacket> {
        self.packet.as_ref()
    }

    pub fn try_open(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.open {
            return true;
        }
        self.open = true;
        self.shot = true;
        self.capture_t = self.clock;
        self.holding = false;
        self.hold_t = 0.0;
        self.fingers.clear();
        self.preedit.clear();
        self.status = "一時停止。メモを書いて「送る」。".into();
        true
    }

    pub fn close(&mut self) {
        self.open = false;
        self.holding = false;
        self.hold_t = 0.0;
        self.fingers.clear();
        self.preedit.clear();
        self.shot = false;
        self.status.clear();
    }

    pub fn action(&mut self, name: &str) {
        match name {
            "close" | "cancel" => self.close(),
            "backspace" => {
                if self.open {
                    self.backspace();
                }
            }
            "send" | "copy" if self.open => {
                self.packet_action = name.to_string();
                self.build_packet();
            }
            _ => {}
        }
    }

    pub fn set_shot_path(&mut self, path: &str) {
        self.shot_path = path.to_string();
        if self.packet.is_some() {
            self.refresh_instruction();
        }
    }

    pub fn take_shot(&mut self) -> bool {
        let shot = self.shot;
        self.shot = false;
        shot
    }

    pub fn packet_seq(&self) -> u32 {
        self.packet_seq
    }

    pub fn packet_json(&self) -> String {
        let Some(packet) = &self.packet else {
            return "{}".into();
        };
        json!({
            "seq": self.packet_seq,
            "action": packet.action,
            "url": packet.url,
            "block": packet.block,
            "fits": packet.fits,
            "copy": packet.copy,
            "instruction": packet.instruction,
            "title": packet.title,
        })
        .to_string()
    }

    pub fn state_json(&mut self) -> String {
        let lay = layout(self.view_w, self.view_h);
        let shot = self.shot;
        self.shot = false;
        json!({
            "enabled": self.enabled,
            "open": self.open,
            "blocks": self.blocks_sim(),
            "note": self.note,
            "status": self.display_status(),
            "shot": shot,
            "packet": self.packet_seq > 0 && self.open,
            "packet_seq": self.packet_seq,
            "packet_action": self.packet_action,
            "external_chrome": self.external_chrome,
            "shot_path": self.shot_path,
            "note_rect": [lay.note.x, lay.note.y, lay.note.w, lay.note.h],
        })
        .to_string()
    }

    pub fn report_value(&self) -> Value {
        let (dump_t, dump_text) = self.baseline();
        let dump: Value = serde_json::from_str(&dump_text).unwrap_or(Value::Null);
        let genre = dump
            .get("genre")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let cap = if self.open {
            self.capture_t
        } else {
            self.clock
        };
        let inputs: Vec<Value> = self
            .events
            .iter()
            .filter(|e| e.t() > dump_t + 1e-4 && e.t() <= cap + 1e-3)
            .map(LogEvent::to_json)
            .collect();
        json!({
            "v": 1,
            "note": self.note,
            "captured_t": cap,
            "dump_t": dump_t,
            "build": {
                "kagra_shared": KAGRA_SHARED_VERSION,
                "git": env!("KAGRA_GIT_REV"),
                "profile": profile_name(),
                "os": target_os(),
                "arch": std::env::consts::ARCH,
            },
            "device": {
                "label": self.device_label,
                "lang": self.lang,
                "width": self.view_w,
                "height": self.view_h,
            },
            "genre": genre,
            "dump": dump,
            "inputs": inputs,
        })
    }

    pub fn append_hud(&self, list: &mut DrawList, width: u32, height: u32) {
        if !self.open || self.external_chrome {
            return;
        }
        let lay = layout(width, height);
        let s = lay.scale;
        list.quads.push(Quad::new(
            0.0,
            0.0,
            width as f32,
            height as f32,
            [8, 10, 16, 150],
        ));
        list.quads.push(Quad::new(
            lay.panel.x,
            lay.panel.y,
            lay.panel.w,
            lay.panel.h,
            [28, 32, 44, 238],
        ));
        list.quads.push(Quad::new(
            lay.note.x,
            lay.note.y,
            lay.note.w,
            lay.note.h,
            [14, 16, 24, 255],
        ));
        list.quads.push(Quad::new(
            lay.send.x,
            lay.send.y,
            lay.send.w,
            lay.send.h,
            [36, 122, 90, 245],
        ));
        list.quads.push(Quad::new(
            lay.copy.x,
            lay.copy.y,
            lay.copy.w,
            lay.copy.h,
            [48, 78, 132, 245],
        ));
        list.quads.push(Quad::new(
            lay.close.x,
            lay.close.y,
            lay.close.w,
            lay.close.h,
            [112, 58, 58, 245],
        ));
        let ink = [236, 232, 220, 255];
        list.texts.push(TextQuad::new(
            "直してここ",
            lay.panel.x + 14.0 * s,
            lay.panel.y + 8.0 * s,
            20.0 * s,
            [232, 176, 84, 255],
        ));
        let shown = self.display_note();
        let (note_text, note_color) = if shown.is_empty() {
            ("メモ（日本語）".to_string(), [150, 146, 136, 255])
        } else {
            (tail_chars(&shown, 72), ink)
        };
        list.texts.push(TextQuad::new(
            &note_text,
            lay.note.x + 8.0 * s,
            lay.note.y + 8.0 * s,
            16.0 * s,
            note_color,
        ));
        let status = wrap_chars(&self.display_status(), 26);
        list.texts.push(TextQuad::new(
            &status,
            lay.note.x,
            lay.note.y + lay.note.h + 8.0 * s,
            13.0 * s,
            [210, 206, 196, 255],
        ));
        let btn = 15.0 * s;
        let label_y = |r: Rect| r.y + (r.h - btn) * 0.35;
        list.texts.push(
            TextQuad::new(
                "送る",
                lay.send.x + lay.send.w * 0.5,
                label_y(lay.send),
                btn,
                ink,
            )
            .aligned(TextAlign::Center),
        );
        list.texts.push(
            TextQuad::new(
                "コピー",
                lay.copy.x + lay.copy.w * 0.5,
                label_y(lay.copy),
                btn,
                ink,
            )
            .aligned(TextAlign::Center),
        );
        list.texts.push(
            TextQuad::new(
                "閉じる",
                lay.close.x + lay.close.w * 0.5,
                label_y(lay.close),
                btn,
                ink,
            )
            .aligned(TextAlign::Center),
        );
    }

    fn hit(&mut self, x: f32, y: f32) {
        let lay = layout(self.view_w, self.view_h);
        if lay.send.contains(x, y) {
            self.action("send");
        } else if lay.copy.contains(x, y) {
            self.action("copy");
        } else if lay.close.contains(x, y) {
            self.action("close");
        }
    }

    fn backspace(&mut self) {
        self.note.pop();
    }

    fn trim_note(&mut self) {
        if self.note.chars().count() > NOTE_MAX {
            self.note = self.note.chars().take(NOTE_MAX).collect();
        }
    }

    fn push_log(&mut self, mut ev: LogEvent) {
        self.seq = self.seq.saturating_add(1);
        match &mut ev {
            LogEvent::Walk { seq, .. }
            | LogEvent::Pointer { seq, .. }
            | LogEvent::Key { seq, .. }
            | LogEvent::Confirm { seq, .. } => *seq = self.seq,
        }
        self.events.push(ev);
        if self.events.len() > 4000 {
            let drop_n = self.events.len() - 4000;
            self.events.drain(0..drop_n);
        }
    }

    fn baseline(&self) -> (f32, String) {
        match self.snaps.first() {
            Some(s) => (s.t, s.dump.clone()),
            None => (0.0, "{}".into()),
        }
    }

    fn display_note(&self) -> String {
        if self.preedit.is_empty() {
            self.note.clone()
        } else {
            format!("{}{}", self.note, self.preedit)
        }
    }

    fn display_status(&self) -> String {
        if self.status.is_empty() {
            "一時停止。メモを書いて「送る」。".into()
        } else {
            self.status.clone()
        }
    }

    fn build_packet(&mut self) {
        let report = self.report_value();
        let mut packet = issue_from_report(&report);
        packet.action = self.packet_action.clone();
        packet.instruction = instruction_text(packet.fits, &self.shot_path);
        self.status = packet.instruction.clone();
        self.packet_seq = self.packet_seq.saturating_add(1);
        self.packet = Some(packet);
    }

    fn refresh_instruction(&mut self) {
        let Some(packet) = self.packet.as_mut() else {
            return;
        };
        packet.instruction = instruction_text(packet.fits, &self.shot_path);
        self.status = packet.instruction.clone();
    }
}

fn instruction_text(fits: bool, shot_path: &str) -> String {
    let shot = if shot_path.is_empty() {
        "スクリーンショットを issue に貼り付けてください。".to_string()
    } else {
        format!("スクリーンショットを保存しました ({shot_path})。issue に貼り付けてください。")
    };
    if fits {
        format!("issue の下書きを開きました。{shot}")
    } else {
        format!("状態が大きいので KAGRA-PLAYTEST-1 ブロックを再現データ欄に貼ってください。{shot}")
    }
}

pub fn issue_from_report(report: &Value) -> IssuePacket {
    let note = report.get("note").and_then(|v| v.as_str()).unwrap_or("");
    let title = title_from_note(note);
    let build = build_summary(report);
    let block = encode_block(report);
    let hint = "状態が URL に入りきりません。画面の KAGRA-PLAYTEST-1 ブロックを再現データ欄に貼り、スクリーンショットも貼ってください。";
    let full = issue_url(&title, note, &build, &block);
    let (url, fits) = if full.len() <= URL_LIMIT {
        (full, true)
    } else {
        (issue_url(&title, note, &build, hint), false)
    };
    let instruction = instruction_text(fits, "");
    IssuePacket {
        url,
        copy: block.clone(),
        block,
        fits,
        instruction,
        title,
        action: String::new(),
    }
}

fn title_from_note(note: &str) -> String {
    let line = note.lines().next().unwrap_or("").trim();
    let short: String = line.chars().take(60).collect();
    if short.is_empty() {
        "[playtest] 直してここ".into()
    } else {
        format!("[playtest] {short}")
    }
}

fn build_summary(report: &Value) -> String {
    let b = &report["build"];
    let d = &report["device"];
    format!(
        "kagra-shared {} git {} {} {}/{}\n{} {}x{} lang {}",
        b["kagra_shared"].as_str().unwrap_or(""),
        b["git"].as_str().unwrap_or(""),
        b["profile"].as_str().unwrap_or(""),
        b["os"].as_str().unwrap_or(""),
        b["arch"].as_str().unwrap_or(""),
        d["label"].as_str().unwrap_or(""),
        d["width"].as_u64().unwrap_or(0),
        d["height"].as_u64().unwrap_or(0),
        d["lang"].as_str().unwrap_or(""),
    )
}

fn issue_url(title: &str, note: &str, build: &str, repro: &str) -> String {
    format!(
        "{ISSUE_NEW}?template=playtest.yml&labels=playtest&title={}&note={}&build={}&repro={}",
        percent_encode(title),
        percent_encode(note),
        percent_encode(build),
        percent_encode(repro),
    )
}

pub fn encode_block(report: &Value) -> String {
    let raw = serde_json::to_vec(report).unwrap_or_default();
    let compressed = zlib_compress(&raw);
    format!("{BLOCK_HEADER}\n{}\n", wrap76(&b64_encode(&compressed)))
}

pub fn decode_block(text: &str) -> Result<Value, String> {
    if let Some(b64) = extract_b64(text) {
        let bytes = b64_decode(&b64)?;
        let raw = zlib_decompress(&bytes)?;
        return serde_json::from_slice(&raw).map_err(|e| e.to_string());
    }
    let trimmed = text.trim();
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            if let Ok(v) = serde_json::from_str::<Value>(&trimmed[start..=end]) {
                if v.get("v").is_some() || v.get("dump").is_some() {
                    return Ok(v);
                }
            }
        }
    }
    Err("no KAGRA-PLAYTEST block".into())
}

fn extract_b64(text: &str) -> Option<String> {
    let mut found = false;
    let mut buf = String::new();
    for line in text.lines() {
        let line = line.trim().trim_start_matches('`').trim_end_matches('`');
        if !found {
            if line.starts_with(BLOCK_HEADER) {
                found = true;
            }
            continue;
        }
        if line.is_empty() || line.starts_with("```") {
            break;
        }
        let ok = line
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');
        if ok && !line.is_empty() {
            buf.push_str(line);
        } else {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

fn compact_json(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(v) => serde_json::to_string(&v).unwrap_or_else(|_| text.to_string()),
        Err(_) => text.to_string(),
    }
}

fn zlib_compress(data: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(6));
    enc.write_all(data).ok();
    enc.finish().unwrap_or_default()
}

fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut dec = ZlibDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn wrap76(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && i % 76 == 0 {
            out.push('\n');
        }
        out.push(ch);
    }
    out
}

fn tail_chars(s: &str, n: usize) -> String {
    let count = s.chars().count();
    if count <= n {
        return s.to_string();
    }
    s.chars().skip(count - n).collect()
}

fn wrap_chars(s: &str, cols: usize) -> String {
    if cols == 0 {
        return s.to_string();
    }
    let mut out = String::new();
    let mut col = 0usize;
    for ch in s.chars() {
        if ch == '\n' {
            out.push('\n');
            col = 0;
            continue;
        }
        if col == cols {
            out.push('\n');
            col = 0;
        }
        out.push(ch);
        col += 1;
    }
    out
}

fn b64_encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8) | data[i + 2] as u32;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        i += 3;
    }
    let rem = data.len() - i;
    if rem == 1 {
        let n = (data[i] as u32) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let n = ((data[i] as u32) << 16) | ((data[i + 1] as u32) << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn b64_val(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn b64_decode(text: &str) -> Result<Vec<u8>, String> {
    let bytes: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !bytes.len().is_multiple_of(4) {
        return Err("bad base64 length".into());
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let pad = chunk.iter().filter(|b| **b == b'=').count();
        let mut n = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            if *b == b'=' {
                continue;
            }
            let v = b64_val(*b).ok_or_else(|| "bad base64".to_string())? as u32;
            n |= v << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_url(url: &str) -> bool {
    let mut cmd = {
        #[cfg(target_os = "windows")]
        {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "start", "", url]);
            c
        }
        #[cfg(target_os = "macos")]
        {
            let mut c = std::process::Command::new("open");
            c.arg(url);
            c
        }
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        {
            let mut c = std::process::Command::new("xdg-open");
            c.arg(url);
            c
        }
    };
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn copy_text(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        return copy_via(&["cmd", "/C", "clip"], text);
    }
    #[cfg(target_os = "macos")]
    {
        return copy_via(&["pbcopy"], text);
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        copy_via(&["wl-copy"], text) || copy_via(&["xclip", "-selection", "clipboard"], text)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_via(argv: &[&str], text: &str) -> bool {
    let Some((bin, args)) = argv.split_first() else {
        return false;
    };
    let Ok(mut child) = std::process::Command::new(bin)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let Some(mut stdin) = child.stdin.take() else {
        return false;
    };
    use std::io::Write as _;
    if stdin.write_all(text.as_bytes()).is_err() {
        return false;
    }
    drop(stdin);
    child.wait().map(|s| s.success()).unwrap_or(false)
}

pub fn downloads_dir() -> std::path::PathBuf {
    if let Ok(user) = std::env::var("USERPROFILE") {
        let p = std::path::PathBuf::from(user).join("Downloads");
        if p.is_dir() {
            return p;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = std::path::PathBuf::from(&home).join("Downloads");
        if p.is_dir() {
            return p;
        }
        let lower = std::path::PathBuf::from(&home).join("downloads");
        if lower.is_dir() {
            return lower;
        }
        let _ = std::fs::create_dir_all(&p);
        return p;
    }
    std::path::PathBuf::from("scratch")
}

pub fn write_rgba_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::PointerPhase;

    fn finger(id: u32, x: f32, y: f32, phase: PointerPhase) -> PointerEvent {
        PointerEvent {
            id,
            x,
            y,
            phase,
            pressure: 1.0,
        }
    }

    #[test]
    fn layout_1280_matches_python() {
        let lay = layout(1280, 720);
        assert!((lay.scale - 1.0).abs() < 1e-6);
        assert!((lay.panel.w - 440.0).abs() < 1e-3);
        assert!((lay.panel.h - 300.0).abs() < 1e-3);
        assert!((lay.panel.x - 420.0).abs() < 1e-3);
        assert!((lay.panel.y - 210.0).abs() < 1e-3);
        assert!((lay.note.y - 252.0).abs() < 1e-3);
        assert!((lay.note.h - 72.0).abs() < 1e-3);
    }

    #[test]
    fn three_finger_long_press_opens() {
        let mut p = Playtest::new(true);
        p.set_viewport(800, 600);
        for id in 0..2u32 {
            assert!(!p.on_pointer(finger(
                id,
                80.0 + id as f32 * 40.0,
                120.0,
                PointerPhase::Begin,
            )));
        }
        assert!(p.on_pointer(finger(2, 160.0, 120.0, PointerPhase::Begin)));
        assert!(p.blocks_sim());
        assert!(!p.is_open());
        p.advance(0.3);
        assert!(!p.is_open());
        p.advance(0.35);
        assert!(p.is_open());
    }

    #[test]
    fn finger_drift_cancels_the_hold() {
        let mut p = Playtest::new(true);
        p.set_viewport(800, 600);
        for id in 0..3u32 {
            p.on_pointer(finger(
                id,
                100.0,
                100.0 + id as f32 * 30.0,
                PointerPhase::Begin,
            ));
        }
        assert!(p.blocks_sim());
        assert!(!p.on_pointer(finger(0, 400.0, 100.0, PointerPhase::Move)));
        p.advance(1.0);
        assert!(!p.is_open());
    }

    #[test]
    fn f12_toggles_and_disabled_ignores_it() {
        let mut p = Playtest::new(true);
        assert!(p.on_key("F12", true));
        assert!(p.is_open());
        p.advance(0.3);
        assert!(p.on_key("F12", true));
        assert!(!p.is_open());
        let mut off = Playtest::new(false);
        assert!(!off.on_key("F12", true));
        assert!(!off.is_open());
    }

    #[test]
    fn japanese_note_roundtrip_and_backspace() {
        let mut p = Playtest::new(true);
        assert!(p.try_open());
        p.push_text("直してここ");
        p.push_text("!");
        p.backspace();
        assert_eq!(p.report_value()["note"], "直してここ");
        let block = encode_block(&p.report_value());
        let back = decode_block(&block).unwrap();
        assert_eq!(back["note"], "直してここ");
    }

    #[test]
    fn decodes_python_zlib_block() {
        let text = include_str!("../tests/fixtures/playtest_py_block.txt");
        let v = decode_block(text).unwrap();
        assert_eq!(v["note"], "直してここ");
        assert_eq!(v["v"], 1);
    }

    #[test]
    fn small_issue_url_carries_the_template_and_japanese_note() {
        let report = json!({"v": 1, "note": "直してここ", "dump": {"version": 1, "genre": "box_hop"}, "inputs": []});
        let packet = issue_from_report(&report);
        assert!(packet.fits, "len {}", packet.url.len());
        assert!(packet.url.starts_with(ISSUE_NEW));
        assert!(packet.url.contains("template=playtest.yml"));
        assert!(packet.url.contains("labels=playtest"));
        assert!(packet
            .url
            .contains("%E7%9B%B4%E3%81%97%E3%81%A6%E3%81%93%E3%81%93"));
        assert!(packet.block.starts_with(BLOCK_HEADER));
    }

    #[test]
    fn large_state_falls_back_to_a_paste_block() {
        let mut big = String::new();
        for i in 0..12_000 {
            big.push_str(&format!("k{i:08x}-"));
        }
        let report = json!({
            "v": 1,
            "note": "直してここ",
            "dump": {"version": 1, "blob": big},
            "inputs": [],
        });
        let packet = issue_from_report(&report);
        assert!(!packet.fits);
        assert!(packet.url.len() <= URL_LIMIT);
        assert!(!packet.url.contains(&big));
        assert!(packet.block.contains(BLOCK_HEADER));
        let back = decode_block(&packet.block).unwrap();
        assert_eq!(back["dump"]["blob"], big);
    }

    #[test]
    fn input_window_keeps_the_oldest_checkpoint_inside_ten_seconds() {
        let mut p = Playtest::new(true);
        p.snapshot_dump(r#"{"version":1,"genre":"box_hop"}"#);
        for _ in 0..12 {
            p.advance(1.0);
            p.note_walk(1.0, 0.0, false, false, false);
            p.commit_walk(1.0);
            p.snapshot_dump(r#"{"version":1,"genre":"box_hop"}"#);
        }
        assert!(p.try_open());
        let report = p.report_value();
        let dump_t = report["dump_t"].as_f64().unwrap();
        assert!(dump_t >= 1.0, "dump_t {dump_t}");
        assert!(report["dump"]["version"].as_u64() == Some(1));
        for ev in report["inputs"].as_array().unwrap() {
            assert!(ev["t"].as_f64().unwrap() + 1e-3 >= dump_t);
        }
        assert!(!report["inputs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn decodes_its_own_japanese_block() {
        let text = include_str!("../tests/fixtures/playtest_rs_block.txt");
        let v = decode_block(text).unwrap();
        assert_eq!(v["note"], "直してここ");
        assert_eq!(v["v"], 1);
    }

    #[test]
    fn hud_draws_the_japanese_title() {
        let mut p = Playtest::new(true);
        p.try_open();
        p.set_note("直してここ");
        let mut list = DrawList::default();
        p.append_hud(&mut list, 1280, 720);
        assert!(list.quads.len() >= 4);
        assert!(list.texts.iter().any(|t| t.text.contains("直してここ")));
        assert!(list.texts.iter().any(|t| t.text.contains("送る")));
    }
}

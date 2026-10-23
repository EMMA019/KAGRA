//! Title → play → pause → result, plus volume settings and a JSON save.
//!
//! Genres opt in (Box Hop, the packaged default). The file lives in a
//! platform path: Windows `%APPDATA%/kagra`, Android internal storage (the
//! shell passes `filesDir`), and `$XDG_DATA_HOME/kagra` elsewhere.
//! HUD strings are ASCII plus a few kana that PixelMplus covers. Missing
//! glyphs are not drawn.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::font;
use crate::scene::{DrawList, Quad, TextAlign, TextQuad};

pub const SAVE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowPhase {
    Title,
    Play,
    Pause,
    Result,
}

impl FlowPhase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Play => "play",
            Self::Pause => "pause",
            Self::Result => "result",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowAction {
    Start,
    Resume,
    Retry,
    ToTitle,
    Pause,
    VolumeUp,
    VolumeDown,
    ToggleMute,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VolumeSettings {
    pub master: f32,
    pub bgm: f32,
    pub sfx: f32,
    pub muted: bool,
}

impl Default for VolumeSettings {
    fn default() -> Self {
        Self {
            master: 0.8,
            bgm: 0.7,
            sfx: 1.0,
            muted: false,
        }
    }
}

impl VolumeSettings {
    pub fn clamped(self) -> Self {
        Self {
            master: self.master.clamp(0.0, 1.0),
            bgm: self.bgm.clamp(0.0, 1.0),
            sfx: self.sfx.clamp(0.0, 1.0),
            muted: self.muted,
        }
    }

    pub fn apply_audio(&self) {
        crate::mixer::set_mute(self.muted);
        let _ = crate::mixer::set_group("master", self.master);
        let _ = crate::mixer::set_group("bgm", self.bgm);
        let _ = crate::mixer::set_group("sfx", self.sfx);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ProgressFile {
    version: u32,
    game_id: String,
    best_score: u32,
    progress: u32,
    settings: VolumeSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameFlow {
    pub phase: FlowPhase,
    pub score: u32,
    pub best_score: u32,
    /// Best landed / checkpoint count seen so far.
    pub best_progress: u32,
    pub settings: VolumeSettings,
    pub game_id: String,
    /// Result line: GOAL / FELL. Empty on other screens.
    pub subtitle: String,
    /// Title banner. Box Hop keeps "BOX HOP".
    pub title: String,
    /// Second title line. Box Hop keeps "はじめる".
    pub tagline: String,
}

impl GameFlow {
    pub fn new(game_id: impl Into<String>) -> Self {
        Self {
            phase: FlowPhase::Title,
            score: 0,
            best_score: 0,
            best_progress: 0,
            settings: VolumeSettings::default(),
            game_id: game_id.into(),
            subtitle: String::new(),
            title: "BOX HOP".into(),
            tagline: "はじめる".into(),
        }
    }

    pub fn enter_play(&mut self) {
        self.phase = FlowPhase::Play;
        self.score = 0;
        self.subtitle.clear();
        self.settings.apply_audio();
        crate::mixer::play_bgm("bgm", 0.4);
    }

    pub fn pause(&mut self) {
        if self.phase == FlowPhase::Play {
            self.phase = FlowPhase::Pause;
            crate::mixer::stop_bgm(0.25);
        }
    }

    pub fn resume(&mut self) {
        if self.phase == FlowPhase::Pause {
            self.phase = FlowPhase::Play;
            self.settings.apply_audio();
            crate::mixer::play_bgm("bgm", 0.25);
        }
    }

    pub fn show_title(&mut self) {
        self.phase = FlowPhase::Title;
        crate::mixer::stop_bgm(0.2);
    }

    pub fn finish(&mut self, score: u32, progress: u32, subtitle: &str) {
        if self.phase == FlowPhase::Result {
            return;
        }
        self.score = score;
        if score > self.best_score {
            self.best_score = score;
        }
        if progress > self.best_progress {
            self.best_progress = progress;
        }
        self.subtitle = subtitle.to_string();
        self.phase = FlowPhase::Result;
        crate::mixer::stop_bgm(0.3);
        let _ = self.write_save();
    }

    pub fn bump_master(&mut self, delta: f32) {
        self.settings.master = (self.settings.master + delta).clamp(0.0, 1.0);
        self.settings.apply_audio();
        let _ = self.write_save();
    }

    pub fn toggle_mute(&mut self) {
        self.settings.muted = !self.settings.muted;
        self.settings.apply_audio();
        let _ = self.write_save();
    }

    pub fn on_key(&self, key: &str) -> Option<FlowAction> {
        let key = key.to_ascii_lowercase();
        match self.phase {
            FlowPhase::Title => match key.as_str() {
                "enter" | "return" | "space" | "confirm" => Some(FlowAction::Start),
                "m" => Some(FlowAction::ToggleMute),
                _ => None,
            },
            FlowPhase::Play => match key.as_str() {
                "escape" | "esc" | "p" | "pause" => Some(FlowAction::Pause),
                "m" => Some(FlowAction::ToggleMute),
                _ => None,
            },
            FlowPhase::Pause => match key.as_str() {
                "enter" | "return" | "space" => Some(FlowAction::Resume),
                "escape" | "esc" | "t" => Some(FlowAction::ToTitle),
                "r" => Some(FlowAction::Retry),
                "m" => Some(FlowAction::ToggleMute),
                "=" | "+" | "equal" | "plus" => Some(FlowAction::VolumeUp),
                "-" | "_" | "minus" => Some(FlowAction::VolumeDown),
                _ => None,
            },
            FlowPhase::Result => match key.as_str() {
                "enter" | "return" | "space" | "r" => Some(FlowAction::Retry),
                "escape" | "esc" | "t" => Some(FlowAction::ToTitle),
                _ => None,
            },
        }
    }

    pub fn hit(&self, x: f32, y: f32, width: u32, height: u32) -> Option<FlowAction> {
        buttons(self.phase, width, height)
            .into_iter()
            .find(|b| b.quad.contains(x, y))
            .map(|b| b.action)
    }

    /// HUD-pixel rect `[x, y, w, h]` of one button, top-left origin.
    ///
    /// Hosts use this (via `hud_json`'s `pause` field) instead of guessing a
    /// corner of the frame. `None` when this phase does not draw `action`.
    pub fn button_rect(&self, action: FlowAction, width: u32, height: u32) -> Option<[f32; 4]> {
        buttons(self.phase, width, height)
            .into_iter()
            .find(|button| button.action == action)
            .map(|button| [button.quad.x, button.quad.y, button.quad.w, button.quad.h])
    }

    pub fn paint(&self, list: &mut DrawList, width: u32, height: u32) {
        let w = width.max(1) as f32;
        let h = height.max(1) as f32;
        let white = [248, 248, 244, 255];
        let gold = [255, 220, 120, 255];
        match self.phase {
            FlowPhase::Title => {
                list.quads.push(Quad::new(0.0, 0.0, w, h, [8, 12, 22, 150]));
                list.quads.push(Quad::new(
                    w * 0.18,
                    h * 0.22,
                    w * 0.64,
                    h * 0.28,
                    [16, 28, 48, 230],
                ));
                let banner = if self.title.is_empty() {
                    "BOX HOP"
                } else {
                    self.title.as_str()
                };
                let tag = if self.tagline.is_empty() {
                    "はじめる"
                } else {
                    self.tagline.as_str()
                };
                text(
                    list,
                    banner,
                    w * 0.5,
                    h * 0.30,
                    28.0,
                    white,
                    TextAlign::Center,
                );
                text(list, tag, w * 0.5, h * 0.40, 18.0, gold, TextAlign::Center);
                text(
                    list,
                    &format!("BEST {}", self.best_score),
                    w * 0.5,
                    h * 0.78,
                    14.0,
                    gold,
                    TextAlign::Center,
                );
                text(
                    list,
                    "SPACE / TAP",
                    w * 0.5,
                    h * 0.86,
                    12.0,
                    white,
                    TextAlign::Center,
                );
            }
            FlowPhase::Play => {}
            FlowPhase::Pause => {
                list.quads.push(Quad::new(0.0, 0.0, w, h, [6, 8, 14, 170]));
                text(
                    list,
                    "PAUSE",
                    w * 0.5,
                    h * 0.16,
                    28.0,
                    white,
                    TextAlign::Center,
                );
                text(
                    list,
                    "ポーズ",
                    w * 0.5,
                    h * 0.24,
                    16.0,
                    gold,
                    TextAlign::Center,
                );
            }
            FlowPhase::Result => {
                let line = if self.subtitle.is_empty() {
                    "RESULT"
                } else {
                    self.subtitle.as_str()
                };
                text(
                    list,
                    line,
                    w * 0.5,
                    h * 0.26,
                    26.0,
                    white,
                    TextAlign::Center,
                );
                text(
                    list,
                    &format!("SCORE {}", self.score),
                    w * 0.5,
                    h * 0.36,
                    16.0,
                    gold,
                    TextAlign::Center,
                );
                text(
                    list,
                    &format!("BEST {}", self.best_score),
                    w * 0.5,
                    h * 0.42,
                    14.0,
                    white,
                    TextAlign::Center,
                );
            }
        }
        for button in buttons(self.phase, width, height) {
            list.quads.push(button.quad);
            let label = if button.action == FlowAction::ToggleMute && self.settings.muted {
                "SOUND"
            } else if button.action == FlowAction::VolumeUp {
                "VOL+"
            } else if button.action == FlowAction::VolumeDown {
                "VOL-"
            } else {
                button.label
            };
            let sum = button.quad.color[0] as u16
                + button.quad.color[1] as u16
                + button.quad.color[2] as u16;
            let ink = if sum < 280 { white } else { [12, 16, 22, 255] };
            text(
                list,
                label,
                button.quad.x + button.quad.w * 0.5,
                button.quad.y + button.quad.h * 0.22,
                14.0,
                ink,
                TextAlign::Center,
            );
        }
    }

    pub fn load_save(&mut self) {
        if !persist_enabled() {
            return;
        }
        let Ok(text) = std::fs::read_to_string(save_path(&self.game_id)) else {
            return;
        };
        let Ok(file) = serde_json::from_str::<ProgressFile>(&text) else {
            return;
        };
        if file.version == 0 || file.version > SAVE_VERSION || file.game_id != self.game_id {
            return;
        }
        self.best_score = file.best_score;
        self.best_progress = file.progress;
        self.settings = file.settings.clamped();
        self.settings.apply_audio();
    }

    pub fn write_save(&self) -> Result<(), String> {
        if !persist_enabled() {
            return Ok(());
        }
        let file = ProgressFile {
            version: SAVE_VERSION,
            game_id: self.game_id.clone(),
            best_score: self.best_score,
            progress: self.best_progress,
            settings: self.settings.clone().clamped(),
        };
        let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
        let path = save_path(&self.game_id);
        atomic_write(&path, &text)
    }
}

struct MenuButton {
    action: FlowAction,
    quad: Quad,
    label: &'static str,
}

fn buttons(phase: FlowPhase, width: u32, height: u32) -> Vec<MenuButton> {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let scale = (w.min(h) / 720.0).clamp(0.5, 2.0);
    match phase {
        FlowPhase::Play => {
            let bw = 84.0 * scale;
            let bh = 32.0 * scale;
            vec![MenuButton {
                action: FlowAction::Pause,
                quad: Quad::new(
                    w - bw - 12.0 * scale,
                    12.0 * scale,
                    bw,
                    bh,
                    [20, 28, 40, 220],
                ),
                label: "PAUSE",
            }]
        }
        FlowPhase::Title => vec![MenuButton {
            action: FlowAction::Start,
            quad: Quad::new(
                w * 0.32,
                h * 0.58,
                w * 0.36,
                52.0 * scale,
                [80, 170, 230, 255],
            ),
            label: "START",
        }],
        FlowPhase::Pause => column(
            w,
            h,
            scale,
            &[
                (FlowAction::Resume, "RESUME", [70, 160, 110, 240]),
                (FlowAction::Retry, "RETRY", [70, 120, 180, 240]),
                (FlowAction::ToTitle, "TITLE", [90, 96, 120, 240]),
                (FlowAction::VolumeDown, "VOL-", [150, 110, 70, 240]),
                (FlowAction::ToggleMute, "MUTE", [150, 90, 70, 240]),
            ],
        ),
        FlowPhase::Result => column(
            w,
            h,
            scale,
            &[
                (FlowAction::Retry, "RETRY", [70, 160, 110, 240]),
                (FlowAction::ToTitle, "TITLE", [90, 96, 120, 240]),
            ],
        ),
    }
}

fn column(
    w: f32,
    h: f32,
    scale: f32,
    rows: &[(FlowAction, &'static str, [u8; 4])],
) -> Vec<MenuButton> {
    let bw = 220.0 * scale;
    let bh = 40.0 * scale;
    let gap = 10.0 * scale;
    let total = rows.len() as f32 * bh + (rows.len().saturating_sub(1) as f32) * gap;
    let mut y = (h - total) * 0.55;
    let x = (w - bw) * 0.5;
    rows.iter()
        .map(|(action, label, color)| {
            let button = MenuButton {
                action: *action,
                quad: Quad::new(x, y, bw, bh, *color),
                label,
            };
            y += bh + gap;
            button
        })
        .collect()
}

fn text(
    list: &mut DrawList,
    value: &str,
    x: f32,
    y: f32,
    size: f32,
    color: [u8; 4],
    align: TextAlign,
) {
    list.texts
        .push(TextQuad::new(value, x, y, size, color).aligned(align));
}

/// Every HUD glyph we ship must exist. A hole would draw a blank, not a box.
pub fn hud_text_covered() -> bool {
    const LINES: &[&str] = &[
        "BOX HOP",
        "SPACE / TAP  START",
        "BEST 0",
        "SCORE 0",
        "PAUSE",
        "RESUME",
        "RETRY",
        "TITLE",
        "MUTE",
        "SOUND",
        "VOL+",
        "VOL-",
        "START",
        "GOAL",
        "FELL",
        "RESULT",
        "はじめる",
        "ポーズ",
    ];
    LINES.iter().all(|line| font::text_covered(line))
}

pub fn draw_list_json(list: &DrawList) -> String {
    let quads: Vec<_> = list
        .quads
        .iter()
        .map(|q| {
            serde_json::json!({
                "x": q.x, "y": q.y, "w": q.w, "h": q.h, "color": q.color,
            })
        })
        .collect();
    let texts: Vec<_> = list
        .texts
        .iter()
        .map(|t| {
            let align = match t.align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Right => "right",
            };
            serde_json::json!({
                "text": t.text, "x": t.x, "y": t.y, "size": t.size,
                "color": t.color, "align": align,
            })
        })
        .collect();
    serde_json::json!({ "quads": quads, "texts": texts }).to_string()
}

thread_local! {
    static SAVE_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Shell override. Android passes `context.filesDir`. `None` clears it.
pub fn set_save_root(path: Option<PathBuf>) {
    SAVE_ROOT.with(|slot| *slot.borrow_mut() = path);
}

fn persist_enabled() -> bool {
    let rooted = SAVE_ROOT.with(|slot| slot.borrow().is_some());
    if rooted
        || std::env::var("KAGRA_SAVE_DIR")
            .ok()
            .is_some_and(|s| !s.is_empty())
    {
        return true;
    }
    // Library tests must not write into the developer profile.
    #[cfg(test)]
    {
        false
    }
    #[cfg(not(test))]
    {
        true
    }
}

pub fn save_dir() -> PathBuf {
    let root = SAVE_ROOT.with(|slot| slot.borrow().clone());
    resolve_save_dir(std::env::consts::OS, |key| std::env::var(key).ok(), root)
}

pub fn save_path(game_id: &str) -> PathBuf {
    let slug = if game_id.is_empty() { "game" } else { game_id };
    save_dir().join(format!("{slug}.json"))
}

/// `os_name` is `std::env::consts::OS` (`windows`, `android`, `linux`, `macos`).
pub fn resolve_save_dir(
    os_name: &str,
    getenv: impl Fn(&str) -> Option<String>,
    root: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = root {
        return path;
    }
    if let Some(path) = getenv("KAGRA_SAVE_DIR") {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    match os_name {
        "windows" => match getenv("APPDATA") {
            Some(app) if !app.is_empty() => PathBuf::from(app).join("kagra"),
            _ => PathBuf::from("kagra-save"),
        },
        // The Activity sets the root to internal storage. This fallback is
        // only used before that call.
        "android" => PathBuf::from("kagra-save"),
        _ => {
            if let Some(xdg) = getenv("XDG_DATA_HOME") {
                if !xdg.is_empty() {
                    return PathBuf::from(xdg).join("kagra");
                }
            }
            match getenv("HOME") {
                Some(home) if !home.is_empty() => PathBuf::from(home).join(".local/share/kagra"),
                _ => PathBuf::from("kagra-save"),
            }
        }
    }
}

fn atomic_write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn title_play_pause_result_retry_title() {
        let mut flow = GameFlow::new("box_hop");
        assert_eq!(flow.phase, FlowPhase::Title);
        assert_eq!(flow.on_key("space"), Some(FlowAction::Start));
        flow.enter_play();
        assert_eq!(flow.phase, FlowPhase::Play);
        assert_eq!(flow.on_key("escape"), Some(FlowAction::Pause));
        flow.pause();
        assert_eq!(flow.phase, FlowPhase::Pause);
        assert_eq!(flow.on_key("enter"), Some(FlowAction::Resume));
        flow.resume();
        flow.finish(1200, 4, "GOAL");
        assert_eq!(flow.phase, FlowPhase::Result);
        assert_eq!(flow.best_score, 1200);
        assert_eq!(flow.best_progress, 4);
        flow.finish(10, 0, "FELL");
        assert_eq!(
            flow.best_score, 1200,
            "a later worse run does not erase the best"
        );
        assert_eq!(flow.on_key("r"), Some(FlowAction::Retry));
        flow.show_title();
        assert_eq!(flow.phase, FlowPhase::Title);
        assert_eq!(flow.best_score, 1200);
    }

    #[test]
    fn pause_button_is_hittable_and_text_has_glyphs() {
        let flow = GameFlow::new("box_hop");
        assert!(hud_text_covered(), "HUD must not use a missing glyph");
        let buttons = buttons(FlowPhase::Play, 640, 360);
        let pause = &buttons[0];
        let x = pause.quad.x + pause.quad.w * 0.5;
        let y = pause.quad.y + pause.quad.h * 0.5;
        assert_eq!(flow.hit(x, y, 640, 360), None, "title has no pause button");
        let mut playing = flow.clone();
        playing.phase = FlowPhase::Play;
        assert_eq!(playing.hit(x, y, 640, 360), Some(FlowAction::Pause));
        let mut list = DrawList::default();
        playing.paint(&mut list, 640, 360);
        assert!(list.texts.iter().any(|t| t.text == "PAUSE"));
        playing.phase = FlowPhase::Title;
        list.texts.clear();
        playing.paint(&mut list, 640, 360);
        assert!(list.texts.iter().any(|t| t.text == "BOX HOP"));
        assert!(list.texts.iter().any(|t| t.text == "はじめる"));
        playing.phase = FlowPhase::Pause;
        list.texts.clear();
        playing.paint(&mut list, 640, 360);
        assert!(list.texts.iter().any(|t| t.text == "ポーズ"));
    }

    #[test]
    fn windows_android_and_linux_paths() {
        let win = resolve_save_dir(
            "windows",
            env_of(&[("APPDATA", r"C:\Users\a\AppData\Roaming")]),
            None,
        );
        assert_eq!(
            win,
            PathBuf::from(r"C:\Users\a\AppData\Roaming").join("kagra")
        );
        let forced = resolve_save_dir(
            "windows",
            env_of(&[]),
            Some(PathBuf::from("/data/user/0/dev.kagra/files/kagra")),
        );
        assert_eq!(forced, PathBuf::from("/data/user/0/dev.kagra/files/kagra"));
        let android = resolve_save_dir("android", env_of(&[]), None);
        assert_eq!(android, PathBuf::from("kagra-save"));
        let linux = resolve_save_dir("linux", env_of(&[("HOME", "/home/player")]), None);
        assert_eq!(linux, PathBuf::from("/home/player/.local/share/kagra"));
    }

    #[test]
    fn save_roundtrip_uses_the_thread_root() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "kagra-flow-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        set_save_root(Some(dir.clone()));
        let mut flow = GameFlow::new("box_hop");
        flow.best_score = 400;
        flow.best_progress = 3;
        flow.settings.master = 0.4;
        flow.write_save().unwrap();
        let mut loaded = GameFlow::new("box_hop");
        loaded.load_save();
        assert_eq!(loaded.best_score, 400);
        assert_eq!(loaded.best_progress, 3);
        assert!((loaded.settings.master - 0.4).abs() < 1.0e-4);
        let raw = std::fs::read_to_string(dir.join("box_hop.json")).unwrap();
        assert!(raw.contains("\"version\": 1"));
        set_save_root(None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

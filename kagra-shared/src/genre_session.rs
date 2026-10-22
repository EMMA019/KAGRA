//! Common genre session contract — Python (`py/`), Android (`ffi`), Wasm (`wasm`).
//!
//! Each genre exposes the same tick surface: load dump JSON, push walk input,
//! step simulation, read dump + adhesive events. Genre-specific rules stay in
//! `kagra-shared/src/<genre>.rs`; this module is the thin adapter over
//! `WorldPlay` (today) or a slimmer loop later.

use crate::collectathon::WalkInput;
use crate::game::GamePhase;
use crate::novel;
use crate::puzzle;
use crate::world_doc::WorldEvent;
use crate::world_play::WorldPlay;

/// Session errors (JSON parse, wrong `WorldDoc.genre`, serialize).
pub type GenreSessionError = String;

/// Shared per-genre loop for hosts that do not embed `WorldPlay` directly.
///
/// Implementations wrap `WorldPlay` until a genre can run without the full
/// multi-genre state machine. New genres add `impl GenreSession for FooSession`
/// plus `py/foo.rs` (and later `ffi` / JNI shims that call the same methods).
pub trait GenreSession: Sized {
    /// Stable id matching `WorldDoc.genre` (e.g. `puzzle_pad`).
    fn genre_id(&self) -> &'static str;

    fn from_json(json: &str) -> Result<Self, GenreSessionError>;

    fn set_input(&mut self, input: WalkInput);

    fn tick(&mut self, dt: f32);

    fn dump(&self) -> Result<String, GenreSessionError>;

    fn take_events(&mut self, name: &str) -> Vec<WorldEvent>;

    fn confirm(&mut self);

    fn emit_event(
        &mut self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), GenreSessionError>;

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String;

    fn step_interact(&mut self);

    fn phase(&self) -> GamePhase;
}

/// Reference implementation: push-box puzzle (`puzzle_pad`).
#[derive(Clone, Debug)]
pub struct PuzzlePadSession {
    inner: WorldPlay,
}

impl GenreSession for PuzzlePadSession {
    fn genre_id(&self) -> &'static str {
        puzzle::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        let inner = WorldPlay::from_json(json)?;
        if !puzzle::is_puzzle(&inner.doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                puzzle::GAME_ID,
                inner.doc.genre
            ));
        }
        Ok(Self { inner })
    }

    fn set_input(&mut self, input: WalkInput) {
        self.inner.input = input.clamped();
    }

    fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    fn dump(&self) -> Result<String, GenreSessionError> {
        self.inner.doc.to_json()
    }

    fn take_events(&mut self, name: &str) -> Vec<WorldEvent> {
        self.inner.take_events(name)
    }

    fn confirm(&mut self) {
        self.inner.confirm();
    }

    fn emit_event(
        &mut self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), GenreSessionError> {
        self.inner.emit_event(name, data);
        Ok(())
    }

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.inner.start_timer(name, seconds, on_done)
    }

    fn step_interact(&mut self) {
        self.inner.step_interact();
    }

    fn phase(&self) -> GamePhase {
        self.inner.game.phase
    }
}

impl PuzzlePadSession {
    pub fn pushing(&self) -> bool {
        self.inner.puzzle.pushing
    }

    pub fn solved(&self) -> bool {
        self.inner.puzzle.solved
    }

    pub fn done(&self) -> bool {
        self.inner.puzzle.done
    }

    pub fn latch_open(&self) -> bool {
        self.inner.puzzle.latch_open
    }
}

/// Visual novel (`novel_pages`). Script, tap-to-advance, and HUD text stay in Rust.
#[derive(Clone, Debug)]
pub struct NovelSession {
    inner: WorldPlay,
}

impl GenreSession for NovelSession {
    fn genre_id(&self) -> &'static str {
        novel::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        let inner = WorldPlay::from_json(json)?;
        if !novel::is_novel(&inner.doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                novel::GAME_ID,
                inner.doc.genre
            ));
        }
        Ok(Self { inner })
    }

    fn set_input(&mut self, input: WalkInput) {
        self.inner.input = input.clamped();
    }

    fn tick(&mut self, dt: f32) {
        self.inner.tick(dt);
    }

    fn dump(&self) -> Result<String, GenreSessionError> {
        self.inner.doc.to_json()
    }

    fn take_events(&mut self, name: &str) -> Vec<WorldEvent> {
        self.inner.take_events(name)
    }

    fn confirm(&mut self) {
        self.inner.confirm();
    }

    fn emit_event(
        &mut self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), GenreSessionError> {
        self.inner.emit_event(name, data);
        Ok(())
    }

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.inner.start_timer(name, seconds, on_done)
    }

    fn step_interact(&mut self) {
        self.inner.step_interact();
    }

    fn phase(&self) -> GamePhase {
        self.inner.game.phase
    }
}

impl NovelSession {
    pub fn page(&self) -> u32 {
        self.inner.novel.page
    }

    pub fn choice(&self) -> u32 {
        self.inner.novel.choice
    }

    pub fn done(&self) -> bool {
        self.inner.novel.done
    }

    pub fn flag(&self) -> Option<String> {
        self.inner.novel.flag.clone()
    }

    pub fn speaker(&self) -> &'static str {
        novel::speaker()
    }

    pub fn body(&self) -> &'static str {
        novel::body(&self.inner.novel)
    }

    pub fn choice_label(&self, index: u32) -> &'static str {
        novel::choice_label(index)
    }

    /// Tap in screen pixels. Title and result confirm; play advances or picks.
    pub fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) {
        match self.phase() {
            GamePhase::Title | GamePhase::Complete => self.confirm(),
            GamePhase::Playing => novel::pointer(&mut self.inner.novel, x, y, width, height),
        }
    }

    pub fn hud_json(&self, width: u32, height: u32) -> String {
        novel::hud_json(&self.inner.novel, self.phase(), width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GamePhase;

    const ROOM: &str = include_str!("../tests/fixtures/puzzle_pad_world.json");

    #[test]
    fn puzzle_pad_session_contract() {
        let mut s = PuzzlePadSession::from_json(ROOM).expect("fixture");
        assert_eq!(s.genre_id(), "puzzle_pad");
        assert_eq!(s.phase(), GamePhase::Title);
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        s.set_input(WalkInput {
            lx: 0.0,
            lz: 1.0,
            jump: false,
            attack: false,
            dodge: false,
        });
        for _ in 0..200 {
            s.tick(1.0 / 60.0);
            if s.done() {
                break;
            }
        }
        assert!(s.solved() && s.done());
        assert_eq!(s.phase(), GamePhase::Complete);
        let dump = s.dump().expect("dump");
        assert!(dump.contains("solved"));
        assert!(s.take_events("nonexistent").is_empty());
    }

    #[test]
    fn wrong_genre_rejected() {
        let crest = include_str!("../tests/fixtures/crest_isle_world.json");
        let err = PuzzlePadSession::from_json(crest).unwrap_err();
        assert!(err.contains("puzzle_pad"));
    }

    const NOVEL: &str = include_str!("../tests/fixtures/novel_pages_world.json");

    #[test]
    fn novel_session_contract() {
        let mut s = NovelSession::from_json(NOVEL).expect("fixture");
        assert_eq!(s.genre_id(), "novel_pages");
        assert_eq!(s.phase(), GamePhase::Title);
        assert_eq!(s.speaker(), "ユキ");
        s.pointer(10.0, 10.0, 960, 540);
        assert_eq!(s.phase(), GamePhase::Playing);
        assert!(s.hud_json(960, 540).contains("雨"));
        s.pointer(20.0, 20.0, 960, 540);
        s.tick(1.0 / 60.0);
        s.pointer(20.0, 20.0, 960, 540);
        s.tick(1.0 / 60.0);
        assert_eq!(s.page(), novel::CHOICE_PAGE);
        assert!(s.body().contains('?'));
        // Left choice button (same rects as the HUD).
        s.pointer(960.0 * 0.31, 540.0 * 0.70, 960, 540);
        s.tick(1.0 / 60.0);
        assert!(s.done());
        assert_eq!(s.flag().as_deref(), Some(novel::FLAG_STAY));
        assert_eq!(s.phase(), GamePhase::Complete);
        let dump = s.dump().expect("dump");
        assert!(dump.contains("stay"));
        assert!(s.take_events("nonexistent").is_empty());
    }

    #[test]
    fn novel_session_rejects_other_genres() {
        let err = NovelSession::from_json(ROOM).unwrap_err();
        assert!(err.contains("novel_pages"));
    }
}

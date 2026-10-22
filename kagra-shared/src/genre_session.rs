//! Common genre session contract — Python (`py/`), Android (`ffi`), Wasm (`wasm`).
//!
//! Each genre exposes the same tick surface: load dump JSON, push walk input,
//! step simulation, read dump + adhesive events. Genre-specific rules stay in
//! `kagra-shared/src/<genre>.rs`; this module is the thin adapter over
//! `WorldPlay` (today) or a slimmer loop later.

use crate::action;
use crate::collectathon::WalkInput;
use crate::fight;
use crate::game::GamePhase;
use crate::novel;
use crate::platformer;
use crate::puzzle;
use crate::race;
use crate::td;
use crate::world_doc::WorldEvent;
use crate::world_play::WorldPlay;

pub use crate::fishcook::FishCookSession;
pub use crate::puzzle_board::PuzzleSession;
pub use crate::rhythm_chart::RhythmSession;

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

/// Slide / lights-out grid / match. Not [`PuzzlePadSession`] (crate push).
impl GenreSession for PuzzleSession {
    fn genre_id(&self) -> &'static str {
        crate::puzzle_board::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        PuzzleSession::open(json)
    }

    fn set_input(&mut self, input: WalkInput) {
        self.apply_input(input);
    }

    fn tick(&mut self, dt: f32) {
        self.step(dt);
    }

    fn dump(&self) -> Result<String, GenreSessionError> {
        self.world_json()
    }

    fn take_events(&mut self, name: &str) -> Vec<WorldEvent> {
        self.drain_events(name)
    }

    fn confirm(&mut self) {
        self.confirm_action();
    }

    fn emit_event(
        &mut self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), GenreSessionError> {
        self.record_event(name, data);
        Ok(())
    }

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.begin_timer(name, seconds, on_done)
    }

    fn step_interact(&mut self) {
        self.use_nearby();
    }

    fn phase(&self) -> GamePhase {
        self.current_phase()
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

/// Side-scrolling platformer (`box_hop`). Implements [`GenreSession`].
///
/// Jump, land, fall, checkpoint, and goal stay in `platformer.rs` / `WorldPlay`.
/// This wrapper is the host surface (Python, and later Android FFI / Wasm).
#[derive(Clone, Debug)]
pub struct PlatformerSession {
    inner: WorldPlay,
}

impl GenreSession for PlatformerSession {
    fn genre_id(&self) -> &'static str {
        platformer::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        let inner = WorldPlay::from_json(json)?;
        if !platformer::is_platformer(&inner.doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                platformer::GAME_ID,
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

impl PlatformerSession {
    pub fn dead(&self) -> bool {
        self.inner.platform.dead
    }

    pub fn won(&self) -> bool {
        self.inner.platform.won
    }

    pub fn landed(&self) -> u32 {
        self.inner.platform.landed
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

/// Tower defense (`td_lane`). Path, waves, gold, lives, and placement stay in Rust.
#[derive(Clone, Debug)]
pub struct TdSession {
    inner: WorldPlay,
}

impl GenreSession for TdSession {
    fn genre_id(&self) -> &'static str {
        td::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        let inner = WorldPlay::from_json(json)?;
        if !td::is_td(&inner.doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                td::GAME_ID,
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

impl TdSession {
    pub fn gold(&self) -> u32 {
        self.inner.td.coins
    }

    pub fn lives(&self) -> u32 {
        self.inner.td.lives
    }

    pub fn wave(&self) -> u32 {
        self.inner.td.wave_number()
    }

    pub fn waves(&self) -> u32 {
        self.inner.td.wave_count()
    }

    pub fn selected(&self) -> u8 {
        self.inner.td.selected
    }

    pub fn kills(&self) -> u32 {
        self.inner.td.kills
    }

    pub fn leaks(&self) -> u32 {
        self.inner.td.leaks
    }

    pub fn won(&self) -> bool {
        self.inner.td.won
    }

    pub fn done(&self) -> bool {
        self.inner.td.done
    }

    /// 0 arrow, 1 cannon, 2 mage.
    pub fn select_tower(&mut self, kind: u8) {
        self.inner.td.select(kind);
    }

    /// Screen tap or mouse click. Title / result confirms (start or retry).
    /// Play selects a tower button or places the selected type on a slot.
    pub fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) {
        match self.phase() {
            GamePhase::Title | GamePhase::Complete => self.confirm(),
            GamePhase::Playing => {
                td::pointer_play(&mut self.inner.doc, &mut self.inner.td, x, y, width, height);
            }
        }
    }

    pub fn world_to_screen(
        &self,
        x: f32,
        y: f32,
        z: f32,
        width: u32,
        height: u32,
    ) -> Option<(f32, f32)> {
        td::world_to_screen(&self.inner.doc, [x, y, z], width, height)
    }

    pub fn hud_json(&self, width: u32, height: u32) -> String {
        td::hud_json(&self.inner.td, self.phase(), width, height)
    }
}

/// Racing (`race_drive`). Implements [`GenreSession`].
///
/// Steer, throttle, laps, the lap clock, and the best-lap ghost stay in
/// `race.rs` (the `vehicle` / `road` / `driving` stack when the dump has
/// waypoints). This wrapper is the host surface.
#[derive(Clone, Debug)]
pub struct RaceSession {
    inner: WorldPlay,
}

impl GenreSession for RaceSession {
    fn genre_id(&self) -> &'static str {
        race::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        let inner = WorldPlay::from_json(json)?;
        if !race::is_race(&inner.doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                race::GAME_ID,
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

impl RaceSession {
    pub fn laps(&self) -> u32 {
        self.inner.race.laps
    }

    pub fn laps_need(&self) -> u32 {
        self.inner.race.laps_need
    }

    pub fn lap_time(&self) -> f32 {
        self.inner.race.lap_time
    }

    pub fn best_lap(&self) -> f32 {
        self.inner.race.best_lap
    }

    pub fn race_time(&self) -> f32 {
        self.inner.race.race_time
    }

    pub fn speed(&self) -> f32 {
        self.inner.race.speed
    }

    pub fn won(&self) -> bool {
        self.inner.race.won
    }

    pub fn done(&self) -> bool {
        self.inner.race.done
    }

    pub fn ghost(&self) -> bool {
        self.inner.race.has_ghost()
    }
}

/// 4-lane chart (`rhythm`). Timing, judgement, combo, and score stay in
/// `rhythm_chart`. `rhythm_beat` (one marker, `WorldPlay`) is a different genre.
impl GenreSession for RhythmSession {
    fn genre_id(&self) -> &'static str {
        crate::rhythm_chart::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        RhythmSession::open(json)
    }

    fn set_input(&mut self, input: WalkInput) {
        self.apply_input(input);
    }

    fn tick(&mut self, dt: f32) {
        self.step(dt);
    }

    fn dump(&self) -> Result<String, GenreSessionError> {
        self.world_json()
    }

    fn take_events(&mut self, name: &str) -> Vec<WorldEvent> {
        self.drain_events(name)
    }

    fn confirm(&mut self) {
        self.confirm_action();
    }

    fn emit_event(
        &mut self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), GenreSessionError> {
        self.record_event(name, data);
        Ok(())
    }

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.begin_timer(name, seconds, on_done)
    }

    fn step_interact(&mut self) {
        self.use_nearby();
    }

    fn phase(&self) -> GamePhase {
        self.current_phase()
    }
}

/// Arena combat (`action_arena`). Hitboxes, HP, combos, minion AI, and the
/// boss wave stay in `action.rs`.
#[derive(Clone, Debug)]
pub struct ActionSession {
    inner: WorldPlay,
}

impl GenreSession for ActionSession {
    fn genre_id(&self) -> &'static str {
        action::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        let inner = WorldPlay::from_json(json)?;
        if !action::is_action(&inner.doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                action::GAME_ID,
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

impl ActionSession {
    pub fn hp(&self) -> u32 {
        self.inner.action.hp
    }

    pub fn hp_max(&self) -> u32 {
        action::PLAYER_HP
    }

    pub fn hits(&self) -> u32 {
        self.inner.action.hits
    }

    pub fn kills(&self) -> u32 {
        self.inner.action.kills
    }

    pub fn combo(&self) -> u32 {
        self.inner.action.combo
    }

    pub fn dead(&self) -> bool {
        self.inner.action.dead
    }

    pub fn won(&self) -> bool {
        self.inner.action.won
    }

    pub fn boss_up(&self) -> bool {
        self.inner.action.boss_up
    }

    pub fn boss_hp(&self) -> u32 {
        self.inner.action.boss_hp
    }

    pub fn boss_hp_max(&self) -> u32 {
        if self.inner.action.has_boss() {
            action::BOSS_HP
        } else {
            0
        }
    }

    pub fn foes_left(&self) -> u32 {
        let pid = self.inner.doc.player.as_ref().map(|w| w.id.as_str());
        let props = self
            .inner
            .doc
            .props
            .iter()
            .filter(|p| p.name == "foe" && p.enabled)
            .count();
        let walks = self
            .inner
            .doc
            .walkers
            .iter()
            .filter(|w| w.name == "foe" && Some(w.id.as_str()) != pid)
            .count();
        (props + walks) as u32
    }
}

/// 1v1 round (`fight_hitstun`). Hitstun, guard, combo, and opponent AI stay
/// in `fight.rs`.
#[derive(Clone, Debug)]
pub struct FightSession {
    inner: WorldPlay,
}

impl GenreSession for FightSession {
    fn genre_id(&self) -> &'static str {
        fight::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        let inner = WorldPlay::from_json(json)?;
        if !fight::is_fight(&inner.doc) {
            return Err(format!(
                "WorldDoc.genre must be {:?}, got {:?}",
                fight::GAME_ID,
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

impl FightSession {
    pub fn hp(&self) -> u32 {
        self.inner.fight.hp
    }

    pub fn hp_max(&self) -> u32 {
        fight::PLAYER_HP
    }

    pub fn opp_hp(&self) -> u32 {
        self.inner.fight.opp_hp
    }

    pub fn opp_hp_max(&self) -> u32 {
        fight::OPP_HP
    }

    pub fn hits(&self) -> u32 {
        self.inner.fight.hits
    }

    pub fn combo(&self) -> u32 {
        self.inner.fight.combo
    }

    pub fn won(&self) -> bool {
        self.inner.fight.won
    }

    pub fn ko(&self) -> bool {
        self.inner.fight.ko
    }

    pub fn done(&self) -> bool {
        self.inner.fight.done
    }

    pub fn guarding(&self) -> bool {
        self.inner.fight.guarding
    }
}

/// Cast, reel, cook, and sell (`fishcook`). Rules stay in `fishcook` /
/// `fish` / `cook` / `shop`. Not the one-shot `fish_cast` or `cook_stove` loops.
impl GenreSession for FishCookSession {
    fn genre_id(&self) -> &'static str {
        crate::fishcook::GAME_ID
    }

    fn from_json(json: &str) -> Result<Self, GenreSessionError> {
        FishCookSession::open(json)
    }

    fn set_input(&mut self, input: WalkInput) {
        self.apply_input(input);
    }

    fn tick(&mut self, dt: f32) {
        self.step(dt);
    }

    fn dump(&self) -> Result<String, GenreSessionError> {
        self.world_json()
    }

    fn take_events(&mut self, name: &str) -> Vec<WorldEvent> {
        self.drain_events(name)
    }

    fn confirm(&mut self) {
        self.confirm_action();
    }

    fn emit_event(
        &mut self,
        name: &str,
        data: Option<serde_json::Value>,
    ) -> Result<(), GenreSessionError> {
        self.record_event(name, data);
        Ok(())
    }

    fn start_timer(&mut self, name: &str, seconds: f32, on_done: Option<&str>) -> String {
        self.begin_timer(name, seconds, on_done)
    }

    fn step_interact(&mut self) {
        self.use_nearby();
    }

    fn phase(&self) -> GamePhase {
        self.current_phase()
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

    #[test]
    fn puzzle_session_clears_slide_grid_and_match() {
        let mut s = PuzzleSession::from_json(r#"{"version":1,"genre":"puzzle"}"#).expect("open");
        assert_eq!(s.genre_id(), "puzzle");
        assert_eq!(s.phase(), GamePhase::Title);
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        for _ in 0..16 {
            if s.phase() == GamePhase::Complete {
                break;
            }
            if s.solved() {
                s.confirm();
            } else {
                assert!(s.play_demo_move(), "level {}", s.level());
            }
        }
        assert!(s.won());
        assert_eq!(s.phase(), GamePhase::Complete);
        let dump = s.dump().expect("dump");
        assert!(dump.contains("solved"));
        assert!(s.take_events("win").len() == 1);
        let err = PuzzlePadSession::from_json(r#"{"version":1,"genre":"puzzle"}"#).unwrap_err();
        assert!(err.contains("puzzle_pad"));
    }

    const HOP: &str = include_str!("../tests/fixtures/platformer_world.json");

    fn hop_value(s: &PlatformerSession) -> serde_json::Value {
        serde_json::from_str(&s.dump().expect("dump")).expect("json")
    }

    fn hop_pos(s: &PlatformerSession) -> [f32; 3] {
        let v = hop_value(s);
        let p = &v["player"]["position"];
        [
            p[0].as_f64().unwrap() as f32,
            p[1].as_f64().unwrap() as f32,
            p[2].as_f64().unwrap() as f32,
        ]
    }

    fn hop_on_ground(s: &PlatformerSession) -> bool {
        hop_value(s)["player"]["on_ground"]
            .as_bool()
            .unwrap_or(false)
    }

    /// Right edges of every platform except the goal (rightmost) slab.
    fn hop_jump_edges(s: &PlatformerSession) -> Vec<f32> {
        let v = hop_value(s);
        let mut edges = Vec::new();
        for p in v["props"].as_array().expect("props") {
            if p["name"].as_str() == Some("platform") {
                let x = p["position"][0].as_f64().unwrap() as f32;
                let sx = p["scale"][0].as_f64().unwrap() as f32;
                edges.push(x + 0.5 * sx.abs());
            }
        }
        edges.sort_by(|a, b| a.partial_cmp(b).unwrap());
        edges.pop();
        edges
    }

    fn hop_drive(s: &mut PlatformerSession, edges: &[f32]) {
        if s.dead() {
            s.confirm();
        }
        let pos = hop_pos(s);
        let jump = hop_on_ground(s)
            && edges
                .iter()
                .any(|edge| pos[0] > edge - 0.85 && pos[0] < edge - 0.12);
        s.set_input(WalkInput {
            lx: 1.0,
            lz: 0.0,
            jump,
            attack: false,
            dodge: false,
        });
        s.tick(1.0 / 60.0);
    }

    #[test]
    fn platformer_session_side_scroll_to_goal() {
        let mut s = PlatformerSession::from_json(HOP).expect("fixture");
        assert_eq!(s.genre_id(), "box_hop");
        assert_eq!(s.phase(), GamePhase::Title);
        let dump = s.dump().expect("dump");
        assert!(dump.contains("knight"));
        assert!(dump.contains("crate.glb"));
        assert!(dump.contains("chest"));
        assert!(dump.contains("well"));
        let edges = hop_jump_edges(&s);
        assert_eq!(
            edges.len(),
            3,
            "three gaps before the goal slab, got {edges:?}"
        );
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        for _ in 0..30 {
            hop_drive(&mut s, &edges);
        }
        let pos = hop_pos(&s);
        assert!(
            pos[0] > -9.2,
            "D / +lx must run toward +X (screen right), pos={pos:?}"
        );
        assert!(pos[2].abs() < 0.25, "lane stays on Z=0, pos={pos:?}");
        for _ in 0..900 {
            if s.won() {
                break;
            }
            hop_drive(&mut s, &edges);
        }
        let pos = hop_pos(&s);
        assert!(
            s.won() && !s.dead(),
            "scripted jumps should reach the chest, won={} dead={} landed={} pos={pos:?}",
            s.won(),
            s.dead(),
            s.landed(),
        );
        assert_eq!(s.phase(), GamePhase::Complete);
        assert!(s.landed() >= 1);
        assert!(s.take_events("nonexistent").is_empty());
    }

    #[test]
    fn platformer_rejects_other_genre() {
        let crest = include_str!("../tests/fixtures/crest_isle_world.json");
        let err = PlatformerSession::from_json(crest).unwrap_err();
        assert!(err.contains("box_hop"));
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

    const TD: &str = include_str!("../tests/fixtures/td_world.json");

    #[test]
    fn td_session_places_from_a_tap() {
        let mut s = TdSession::from_json(TD).expect("fixture");
        assert_eq!(s.genre_id(), "td_lane");
        assert_eq!(s.phase(), GamePhase::Title);
        assert_eq!(s.waves(), 5);
        assert_eq!(s.lives(), td::WAVE_LIVES);
        assert_eq!(s.gold(), td::WAVE_GOLD);
        s.pointer(20.0, 20.0, 960, 540);
        assert_eq!(s.phase(), GamePhase::Playing);
        let hud = s.hud_json(960, 540);
        assert!(hud.contains("wave 1/5"));
        assert!(hud.is_ascii());
        let dump = s.dump().expect("dump");
        let world: serde_json::Value = serde_json::from_str(&dump).unwrap();
        let slot = world["props"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "prop:slot-a")
            .unwrap();
        let x = slot["position"][0].as_f64().unwrap() as f32;
        let z = slot["position"][2].as_f64().unwrap() as f32;
        let (sx, sy) = s
            .world_to_screen(x, 0.0, z, 960, 540)
            .expect("slot on screen");
        s.pointer(sx, sy, 960, 540);
        assert!(s.gold() < td::WAVE_GOLD);
        assert!(s.dump().unwrap().contains("gazebo"));
        s.select_tower(2);
        assert_eq!(s.selected(), 2);
        assert!(s.take_events("nonexistent").is_empty());
    }

    #[test]
    fn td_session_rejects_other_genres() {
        let err = TdSession::from_json(ROOM).unwrap_err();
        assert!(err.contains("td_lane"));
    }

    const RACE: &str = include_str!("../tests/fixtures/race_world.json");

    fn race_value(s: &RaceSession) -> serde_json::Value {
        serde_json::from_str(&s.dump().expect("dump")).expect("json")
    }

    fn race_pose(s: &RaceSession) -> (f32, f32, f32) {
        let v = race_value(s);
        let p = &v["player"]["position"];
        (
            p[0].as_f64().unwrap() as f32,
            p[2].as_f64().unwrap() as f32,
            v["player"]["yaw"].as_f64().unwrap() as f32,
        )
    }

    fn race_waypoints(s: &RaceSession) -> Vec<(f32, f32)> {
        let v = race_value(s);
        let mut rows: Vec<(String, f32, f32)> = Vec::new();
        for p in v["props"].as_array().expect("props") {
            if p["name"].as_str() == Some("waypoint") {
                rows.push((
                    p["id"].as_str().unwrap_or("").to_string(),
                    p["position"][0].as_f64().unwrap() as f32,
                    p["position"][2].as_f64().unwrap() as f32,
                ));
            }
        }
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows.into_iter().map(|(_, x, z)| (x, z)).collect()
    }

    fn race_steer(x: f32, z: f32, yaw: f32, wps: &[(f32, f32)]) -> (f32, f32) {
        let mut nearest = 0usize;
        let mut best = f32::MAX;
        for (i, w) in wps.iter().enumerate() {
            let d = (w.0 - x).powi(2) + (w.1 - z).powi(2);
            if d < best {
                best = d;
                nearest = i;
            }
        }
        let ahead = if best < 2.2 * 2.2 { 2 } else { 1 };
        let (tx, tz) = wps[(nearest + ahead) % wps.len()];
        let desired = (tx - x).atan2(tz - z);
        let err = crate::vehicle::wrap_angle(desired - yaw);
        let steer = (-err * 2.4).clamp(-1.0, 1.0);
        let throttle = if steer.abs() > 0.55 { 0.62 } else { 1.0 };
        (steer, throttle)
    }

    #[test]
    fn race_session_three_laps_timer_and_ghost() {
        let mut s = RaceSession::from_json(RACE).expect("fixture");
        assert_eq!(s.genre_id(), "race_drive");
        assert_eq!(s.phase(), GamePhase::Title);
        assert_eq!(s.laps_need(), 3);
        assert!(!s.ghost());
        let dump = s.dump().expect("dump");
        assert!(dump.contains("car_sedan"));
        assert!(dump.contains("road_straight"));
        assert!(dump.contains("car_taxi"));
        assert!(dump.contains("stylized_tree"));
        let wps = race_waypoints(&s);
        assert!(wps.len() >= 8, "waypoints {}", wps.len());
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        let mut saw_ghost_ahead = false;
        for _ in 0..4200 {
            if s.won() {
                break;
            }
            let (x, z, yaw) = race_pose(&s);
            let (steer, throttle) = if s.laps() >= 1 && s.lap_time() < 0.45 {
                (0.0, 0.15)
            } else {
                race_steer(x, z, yaw, &wps)
            };
            if s.laps() == 1 && s.ghost() && s.lap_time() > 0.2 && s.lap_time() < 0.45 {
                let ghost_x = race_value(&s)["props"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|p| p["name"].as_str() == Some("ghost"))
                    .and_then(|p| p["position"][0].as_f64())
                    .unwrap_or(0.0) as f32;
                if ghost_x > x + 0.8 {
                    saw_ghost_ahead = true;
                }
            }
            s.set_input(WalkInput {
                lx: steer,
                lz: throttle,
                jump: false,
                attack: false,
                dodge: false,
            });
            s.tick(1.0 / 60.0);
        }
        let (x, z, _) = race_pose(&s);
        assert!(
            s.won() && s.done() && s.laps() >= 3,
            "3 laps, won={} laps={} best={:.2} t={:.2} ghost={} pos=({x:.1},{z:.1})",
            s.won(),
            s.laps(),
            s.best_lap(),
            s.race_time(),
            s.ghost(),
        );
        assert!(s.best_lap() > 0.5, "best lap {}", s.best_lap());
        assert!(s.ghost(), "best lap must leave a ghost");
        assert!(
            saw_ghost_ahead,
            "slow lap 2 should see the faster ghost ahead"
        );
        assert_eq!(s.phase(), GamePhase::Complete);
        let clock = race_value(&s);
        let lap = clock["timers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"].as_str() == Some("lap"))
            .expect("lap timer");
        assert!(lap["seconds"].as_f64().unwrap() > 0.0);
        assert!(s.take_events("nonexistent").is_empty());
    }

    #[test]
    fn race_rejects_other_genre() {
        let crest = include_str!("../tests/fixtures/crest_isle_world.json");
        let err = RaceSession::from_json(crest).unwrap_err();
        assert!(err.contains("race_drive"));
    }

    #[test]
    fn rhythm_session_auto_clears_chart() {
        let mut s = RhythmSession::from_json(r#"{"version":1,"genre":"rhythm"}"#).expect("open");
        assert_eq!(s.genre_id(), "rhythm");
        assert_eq!(s.phase(), GamePhase::Title);
        assert!(s.lane_count() == 4);
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        s.set_auto(true);
        for _ in 0..900 {
            if s.phase() == GamePhase::Complete {
                break;
            }
            s.tick(1.0 / 60.0);
        }
        assert_eq!(s.phase(), GamePhase::Complete);
        assert_eq!(s.miss_count(), 0);
        assert_eq!(s.perfect_count(), s.note_count());
        assert!(s.score() >= 300);
        let dump = s.dump().expect("dump");
        assert!(dump.contains("clear"));
        assert!(dump.contains("crate.glb"));
        assert!(s.take_events("clear").len() == 1);
        let err =
            RhythmSession::from_json(include_str!("../tests/fixtures/rhythm_beat_world.json"))
                .unwrap_err();
        assert!(err.contains("rhythm_beat"));
    }

    const ARENA: &str = include_str!("../tests/fixtures/action_world.json");
    const RING: &str = include_str!("../tests/fixtures/fight_hitstun_world.json");

    fn arena_value(s: &ActionSession) -> serde_json::Value {
        serde_json::from_str(&s.dump().expect("dump")).expect("json")
    }

    fn arena_input(s: &ActionSession) -> WalkInput {
        let v = arena_value(s);
        let p = &v["player"]["position"];
        let px = p[0].as_f64().unwrap() as f32;
        let pz = p[2].as_f64().unwrap() as f32;
        let facing = v["player"]["yaw"].as_f64().unwrap_or(0.0) as f32;
        let cam = &v["cameras"][0];
        let dx =
            cam["position"][0].as_f64().unwrap() as f32 - cam["target"][0].as_f64().unwrap() as f32;
        let dz =
            cam["position"][2].as_f64().unwrap() as f32 - cam["target"][2].as_f64().unwrap() as f32;
        let yaw = (-dx).atan2(-dz);
        let (siny, cosy) = yaw.sin_cos();
        let pid = v["player"]["id"].as_str().unwrap_or("");
        let mut best: Option<(f32, f32, f32)> = None;
        for w in v["walkers"].as_array().unwrap() {
            let id = w["id"].as_str().unwrap_or("");
            if id == pid {
                continue;
            }
            let name = w["name"].as_str().unwrap_or("");
            if name != "foe" && name != "boss" {
                continue;
            }
            let x = w["position"][0].as_f64().unwrap() as f32;
            let z = w["position"][2].as_f64().unwrap() as f32;
            let d = (x - px).hypot(z - pz);
            if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                best = Some((d, x, z));
            }
        }
        for prop in v["props"].as_array().unwrap() {
            if prop["enabled"].as_bool() == Some(false) {
                continue;
            }
            let name = prop["name"].as_str().unwrap_or("");
            if name != "foe" && name != "boss" {
                continue;
            }
            let x = prop["position"][0].as_f64().unwrap() as f32;
            let z = prop["position"][2].as_f64().unwrap() as f32;
            let d = (x - px).hypot(z - pz);
            if best.map(|(bd, _, _)| d < bd).unwrap_or(true) {
                best = Some((d, x, z));
            }
        }
        let Some((dist, tx, tz)) = best else {
            return WalkInput::default();
        };
        let len = (tx - px).hypot(tz - pz).max(1e-4);
        let nx = (tx - px) / len;
        let nz = (tz - pz) / len;
        let lx = nx * -cosy + nz * siny;
        let lz = nx * siny + nz * cosy;
        let desired = nx.atan2(nz);
        let mut err = desired - facing;
        while err > std::f32::consts::PI {
            err -= std::f32::consts::TAU;
        }
        while err < -std::f32::consts::PI {
            err += std::f32::consts::TAU;
        }
        // Wish must stay above the walk deadzone or the body never turns into the swing.
        let approach = if dist < 0.95 {
            -1.0
        } else if err.abs() > 0.45 || dist > 1.35 {
            1.0
        } else {
            0.35
        };
        WalkInput {
            lx: (lx * approach).clamp(-1.0, 1.0),
            lz: (lz * approach).clamp(-1.0, 1.0),
            jump: false,
            attack: dist < 1.7 && err.abs() < 0.65,
            dodge: dist < 0.98,
        }
    }

    #[test]
    fn action_session_clears_minions_then_boss() {
        let mut s = ActionSession::from_json(ARENA).expect("fixture");
        assert_eq!(s.genre_id(), "action_arena");
        assert_eq!(s.phase(), GamePhase::Title);
        let dump = s.dump().expect("dump");
        assert!(dump.contains("knight"));
        assert!(dump.contains("rogue"));
        assert!(dump.contains("mage"));
        assert!(dump.contains("barbarian"));
        assert_eq!(s.foes_left(), 3);
        assert!(!s.boss_up());
        assert_eq!(s.boss_hp_max(), action::BOSS_HP);
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        let mut saw_boss = false;
        let mut saw_combo = false;
        let mut deaths = 0u32;
        for _ in 0..3600 {
            if s.won() {
                break;
            }
            if s.dead() || s.phase() != GamePhase::Playing {
                deaths += 1;
                s.confirm();
            }
            if s.boss_up() {
                saw_boss = true;
            }
            if s.combo() >= 2 {
                saw_combo = true;
            }
            let input = arena_input(&s);
            s.set_input(input);
            s.tick(1.0 / 60.0);
        }
        assert!(
            s.won() && !s.dead(),
            "clear minions then boss, won={} dead={} hp={} kills={} foes={} boss_up={} boss_hp={} deaths={deaths}",
            s.won(),
            s.dead(),
            s.hp(),
            s.kills(),
            s.foes_left(),
            s.boss_up(),
            s.boss_hp(),
        );
        assert!(saw_boss, "boss must enter after the minions fall");
        assert!(saw_combo, "two hits inside the combo window");
        assert!(s.kills() >= 4, "3 minions + boss, kills {}", s.kills());
        assert!(deaths < 8, "too many retries: {deaths}");
        assert_eq!(s.phase(), GamePhase::Complete);
        assert!(s.hits() >= 4);
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        assert_eq!(s.hp(), action::PLAYER_HP);
        assert_eq!(s.foes_left(), 3);
        assert!(!s.boss_up());
        assert!(s.take_events("nonexistent").is_empty());
    }

    #[test]
    fn action_rejects_other_genre() {
        let crest = include_str!("../tests/fixtures/crest_isle_world.json");
        let err = ActionSession::from_json(crest).unwrap_err();
        assert!(err.contains("action_arena"));
        let err = ActionSession::from_json(RING).unwrap_err();
        assert!(err.contains("action_arena"));
    }

    #[test]
    fn fight_session_kos_and_retries() {
        let mut s = FightSession::from_json(RING).expect("fixture");
        assert_eq!(s.genre_id(), "fight_hitstun");
        assert_eq!(s.phase(), GamePhase::Title);
        assert_eq!(s.hp(), fight::PLAYER_HP);
        assert_eq!(s.opp_hp(), fight::OPP_HP);
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        for _ in 0..900 {
            if s.done() {
                break;
            }
            let v = serde_json::from_str::<serde_json::Value>(&s.dump().unwrap()).unwrap();
            let px = v["player"]["position"][0].as_f64().unwrap() as f32;
            let ox = v["props"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| {
                    p["name"].as_str() == Some("opponent") && p["enabled"].as_bool() != Some(false)
                })
                .and_then(|p| p["position"][0].as_f64())
                .unwrap_or(f64::from(px) + 2.0) as f32;
            let dist = (ox - px).abs();
            s.set_input(WalkInput {
                lx: if ox > px { 1.0 } else { -1.0 },
                lz: 0.0,
                jump: false,
                attack: dist < 1.7,
                dodge: false,
            });
            s.tick(1.0 / 60.0);
        }
        assert!(
            s.won() && s.done() && !s.ko(),
            "won={} done={} ko={} hp={} opp={} hits={} combo={}",
            s.won(),
            s.done(),
            s.ko(),
            s.hp(),
            s.opp_hp(),
            s.hits(),
            s.combo(),
        );
        assert!(s.hits() >= fight::OPP_HP);
        assert_eq!(s.phase(), GamePhase::Complete);
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        assert!(!s.done());
        assert_eq!(s.opp_hp(), fight::OPP_HP);
        assert!(s.take_events("nonexistent").is_empty());
    }

    #[test]
    fn fight_rejects_other_genre() {
        let err = FightSession::from_json(ARENA).unwrap_err();
        assert!(err.contains("fight_hitstun"));
    }

    const FISHCOOK: &str = include_str!("../tests/fixtures/fishcook_world.json");

    #[test]
    fn fishcook_session_sells_until_the_day_goal() {
        let mut s = FishCookSession::from_json(FISHCOOK).expect("fixture");
        assert_eq!(s.genre_id(), "fishcook");
        assert_eq!(s.phase(), GamePhase::Title);
        let dump = s.dump().expect("dump");
        assert!(dump.contains("knight"));
        assert!(dump.contains("gazebo"));
        assert!(dump.contains("mill"));
        s.confirm();
        assert_eq!(s.phase(), GamePhase::Playing);
        crate::fishcook::drive_day(&mut s, &mut |session, input| {
            session.set_input(input);
            session.tick(1.0 / 60.0);
        });
        assert!(s.won());
        assert!(s.done());
        assert_eq!(s.phase(), GamePhase::Complete);
        assert_eq!(s.coins(), s.goal());
        assert!(s.hud_json(640, 360).contains("DAY END"));
        assert!(s.hud_json(640, 360).contains("goal met"));
        let sold = s.take_events("sold");
        assert_eq!(sold.len(), 1);
        assert!(sold[0].count >= 2);
        assert!(s.take_events("nonexistent").is_empty());
        let err = FishCookSession::from_json(ROOM).unwrap_err();
        assert!(err.contains("fishcook"));
    }
}

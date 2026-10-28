//! The session's view of a game kit.
//!
//! `SharedSession` used to keep one `Option<..>` slot per genre and a
//! branch for each in about ten places (touch, pointer, key, tick, dump, HUD,
//! debug, title, confirm). It now holds one `Box<dyn KitHost>`. A new kit
//! implements this trait, adds one line to [`KITS`], and the session, the
//! wasm shell and the playtest hand-off pick it up with no further plumbing.
//!
//! Kits that tick through [`game_core::tick_frame`](crate::game_core::tick_frame)
//! implement `tick` by calling it. Only the host calls into this trait; a kit
//! keeps its own typed API for tests and tools (reach it with
//! [`SharedSession::cat`](crate::session::SharedSession::cat) and friends, which
//! downcast).

use std::any::Any;
use std::fmt::Debug;

use crate::input::PointerEvent;
use crate::scene::DrawList;
use crate::world_doc::WorldDoc;

/// What the session needs from a kit.
pub trait KitHost: Debug + Any {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Short name for errors ("neon cat").
    fn name(&self) -> &'static str;
    /// `title`, `play`, `pause` or `result`.
    fn phase_name(&self) -> &'static str;
    fn playing(&self) -> bool;
    /// The drawable world for the renderer.
    fn document(&self) -> &WorldDoc;
    /// The JSON the kit was loaded from, with its current state (playtest hand-off).
    fn dump_json(&self) -> String;
    /// The per-frame state the page reads (`sim`).
    fn state_json(&self) -> String;
    /// Start (title, result) or resume (pause).
    fn confirm(&mut self);
    fn show_title(&mut self);
    /// A tap or click. True when the kit's own UI used it.
    fn pointer(&mut self, x: f32, y: f32, width: u32, height: u32) -> bool;
    fn on_key(&mut self, name: &str, down: bool) -> bool;
    /// One fixed step.
    fn tick(&mut self, dt: f32);
    /// The HUD. `menu`: draw the in-canvas flow column (false when the page has its own).
    fn hud(&self, width: u32, height: u32, menu: bool) -> DrawList;
    /// Stick or key steering, already filtered to the frames that take it.
    fn walk(&mut self, lx: f32, lz: f32, jump: bool);
    /// What the kit keeps on the device between visits (unlocked stages, records), as JSON,
    /// or `None` for nothing. With the manifest's `page.save`, the page stores it per game
    /// and hands it back in the world as `save` on the next load.
    fn save_json(&self) -> Option<String> {
        None
    }
    /// The second and third buttons held (keys J / K / Shift, a gamepad's B or X), for kits
    /// with more than a jump. Kits with none ignore them.
    fn buttons(&mut self, _attack: bool, _dodge: bool) {}
    /// Every pointer event, for swipe and flick gestures (`t`: seconds on the frame clock).
    fn touch(&mut self, _ev: PointerEvent, _width: u32, _height: u32, _t: f32) {}
    /// Hosts drop their own touch steering and action buttons when true.
    fn uses_swipe(&self) -> bool {
        false
    }
    fn set_debug(&mut self, _on: bool) {}
    /// The page language (`ja` or `en`) for in-canvas texts. Kits with none ignore it.
    fn set_lang(&mut self, _lang: &str) {}
    /// Pause the run (a hidden tab, the Escape key). Kits with a pause card implement it.
    fn pause_run(&mut self) {}
    /// Back into the run from the pause card.
    fn resume_run(&mut self) {}
    /// Does this kit take part in graphics tiers (`quality.rs`)? Off by default so a kit with
    /// its own picture settings (Neon Cat's `?fx=`) is left alone until it opts in.
    fn supports_quality_tiers(&self) -> bool {
        false
    }
    /// The two tier knobs only the kit can apply: sky props (`atmo`, 1 = the full set) and
    /// spark particles kept (`particles`, 0..=1). Called when the tier changes.
    fn quality_hints(&mut self, _atmo: f32, _particles: f32) {}
}

type Detect = fn(&str) -> bool;
type Load = fn(&str) -> Result<Box<dyn KitHost>, String>;

/// Every kit the session can load from a genre dump, in the order tried.
pub const KITS: &[(Detect, Load)] = &[
    (crate::neon_cat::is_cat_dump, |json| {
        Ok(Box::new(crate::neon_cat::NeonCat::from_json(json)?))
    }),
    (crate::neon_run::is_neon_dump, |json| {
        Ok(Box::new(crate::neon_run::NeonRun::from_json(json)?))
    }),
    (crate::hop_beat::is_hop_dump, |json| {
        Ok(Box::new(crate::hop_beat::HopBeat::from_json(json)?))
    }),
    (crate::survivor_arena::is_survivor_dump, |json| {
        Ok(Box::new(crate::survivor_arena::SurvivorArena::from_json(
            json,
        )?))
    }),
    (crate::starter::is_starter_dump, |json| {
        Ok(Box::new(crate::starter::Starter::from_json(json)?))
    }),
    (crate::lantern_watch::is_lantern_watch_dump, |json| {
        Ok(Box::new(crate::lantern_watch::LanternWatch::from_json(
            json,
        )?))
    }),
    (crate::crate_tower::is_crate_tower_dump, |json| {
        Ok(Box::new(crate::crate_tower::CrateTower::from_json(json)?))
    }),
    (crate::station_escape::is_station_escape_dump, |json| {
        Ok(Box::new(crate::station_escape::StationEscape::from_json(
            json,
        )?))
    }),
    (crate::moonlit_tactics::is_moonlit_tactics_dump, |json| {
        Ok(Box::new(crate::moonlit_tactics::MoonlitTactics::from_json(
            json,
        )?))
    }),
    // new_kit.py adds kits above this line.
];

/// The kit that claims `json`, loaded. `None` when no kit does (a WorldPlay genre).
pub fn load(json: &str) -> Option<Result<Box<dyn KitHost>, String>> {
    KITS.iter()
        .find(|(detect, _)| detect(json))
        .map(|(_, load)| load(json))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_dump_finds_its_kit_and_a_world_finds_none() {
        let cat = include_str!("../tests/fixtures/neon_cat_world.json");
        let kit = load(cat).unwrap().unwrap();
        assert_eq!(kit.name(), "neon cat");
        assert_eq!(kit.phase_name(), "title");
        assert!(kit.uses_swipe() && !kit.playing());
        assert!(load(r#"{"genre":"platformer"}"#).is_none());
        assert!(load("not json").is_none());
    }

    #[test]
    fn the_cat_is_reachable_by_downcast() {
        let cat = include_str!("../tests/fixtures/neon_cat_world.json");
        let kit = load(cat).unwrap().unwrap();
        assert!(kit
            .as_any()
            .downcast_ref::<crate::neon_cat::NeonCat>()
            .is_some());
        assert!(kit
            .as_any()
            .downcast_ref::<crate::neon_run::NeonRun>()
            .is_none());
    }
}

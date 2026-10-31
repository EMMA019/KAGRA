//! A character's moves: which clip plays now, and how it blends to the next.
//!
//! A kit says what the body is doing ([`Animator::locomote`]: speed, on the ground, rising
//! or falling) and what it just did ([`Animator::act`]: "attack", "hurt", "die", "cheer").
//! The animator picks the move, crossfades skinned glTF clips through
//! [`AnimMixer`](crate::anim_mixer::AnimMixer), plays one-shot moves once and goes back to
//! walking, holds the last frame of a death, and tells the kit when an attack's blow lands
//! ([`Animator::tick`] returns the cue). Moves and clip names are JSON, so one kit can drive
//! any rigged model (KayKit adventurers name theirs `Idle`, `Walking_A`, `1H_Melee_Attack_Chop`).
//!
//! A model with no clips (a Kenney mini character, a crate) gets the same moves as a
//! procedural [`Pose`]: a bob when walking, a lean into a swing, a squash on landing, a
//! flinch, a fall. [`Animator::apply_walker`] writes either into the world.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::anim_mixer::AnimMixer;

/// One move: the clip, how it plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Move {
    /// The glTF clip name ("" = none: the procedural pose only).
    pub clip: String,
    /// Playback speed (1 = authored).
    pub speed: f32,
    /// Seconds a one-shot move lasts at speed 1 (its clip's length).
    pub secs: f32,
    /// Seconds to crossfade into it.
    pub fade: f32,
    /// For an attack: seconds in when the blow lands (the kit deals damage then).
    pub hit_at: Option<f32>,
    /// Stay on the last frame when done (a death).
    pub hold: bool,
}

impl Default for Move {
    fn default() -> Self {
        Self {
            clip: String::new(),
            speed: 1.0,
            secs: 0.8,
            fade: 0.15,
            hit_at: None,
            hold: false,
        }
    }
}

/// A character's moves by name. The locomotion names are `idle`, `walk`, `run`, `jump`,
/// `fall`; anything else is a one-shot the kit names in [`Animator::act`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MoveSet {
    pub moves: BTreeMap<String, Move>,
    /// Speed (units/s) from which `walk` plays, and `run`.
    pub walk_from: f32,
    pub run_from: f32,
    /// Walk / run clips play faster as the body moves faster: authored speed at this pace.
    pub stride: f32,
}

impl Default for MoveSet {
    fn default() -> Self {
        Self {
            moves: BTreeMap::new(),
            walk_from: 0.15,
            run_from: 3.2,
            stride: 2.0,
        }
    }
}

impl MoveSet {
    /// The KayKit Adventurers rig (Knight, Barbarian, Mage, Rogue): 76 clips, these are the
    /// ones most games need (lengths read from the files).
    pub fn kaykit() -> Self {
        let m = |clip: &str, secs: f32| Move {
            clip: clip.into(),
            secs,
            ..Move::default()
        };
        let mut moves = BTreeMap::new();
        moves.insert("idle".into(), m("Idle", 0.0));
        moves.insert("walk".into(), m("Walking_A", 0.0));
        moves.insert("run".into(), m("Running_A", 0.0));
        moves.insert("jump".into(), m("Jump_Idle", 0.0));
        moves.insert("fall".into(), m("Jump_Idle", 0.0));
        moves.insert(
            "attack".into(),
            Move {
                hit_at: Some(0.45),
                fade: 0.08,
                ..m("1H_Melee_Attack_Chop", 1.07)
            },
        );
        moves.insert(
            "attack2".into(),
            Move {
                hit_at: Some(0.4),
                fade: 0.08,
                ..m("1H_Melee_Attack_Slice_Horizontal", 1.07)
            },
        );
        moves.insert(
            "cast".into(),
            Move {
                hit_at: Some(0.6),
                ..m("Spellcast_Shoot", 0.93)
            },
        );
        moves.insert(
            "shoot".into(),
            Move {
                hit_at: Some(0.35),
                ..m("1H_Ranged_Shoot", 1.07)
            },
        );
        moves.insert("block".into(), m("Block", 1.07));
        moves.insert(
            "hurt".into(),
            Move {
                fade: 0.05,
                ..m("Hit_A", 0.67)
            },
        );
        moves.insert(
            "die".into(),
            Move {
                hold: true,
                ..m("Death_A", 0.8)
            },
        );
        moves.insert("cheer".into(), m("Cheer", 1.67));
        moves.insert("use".into(), m("Use_Item", 1.6));
        moves.insert("pickup".into(), m("PickUp", 1.3));
        moves.insert("dodge".into(), m("Dodge_Forward", 0.4));
        Self {
            moves,
            ..Self::default()
        }
    }

    /// Moves with no clips: the procedural pose only (static models).
    pub fn procedural() -> Self {
        let m = |secs: f32, hit_at: Option<f32>, hold: bool| Move {
            secs,
            hit_at,
            hold,
            ..Move::default()
        };
        let mut moves = BTreeMap::new();
        for name in ["idle", "walk", "run", "jump", "fall"] {
            moves.insert(name.into(), m(0.0, None, false));
        }
        moves.insert("attack".into(), m(0.45, Some(0.2), false));
        moves.insert("cast".into(), m(0.6, Some(0.4), false));
        moves.insert("hurt".into(), m(0.35, None, false));
        moves.insert("die".into(), m(0.7, None, true));
        moves.insert("cheer".into(), m(1.0, None, false));
        Self {
            moves,
            ..Self::default()
        }
    }
}

/// What a body is doing, for the procedural pose: offsets on top of where the kit put it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    /// Up (world units).
    pub lift: f32,
    /// Forward along the facing (a lunge), world units.
    pub lunge: f32,
    /// Lean forward (radians; a negative prop pitch leans toward the facing).
    pub lean: f32,
    /// Sideways roll (radians): a fall to the side.
    pub roll: f32,
    /// Scale: (wide, tall) multipliers (squash and stretch).
    pub squash: (f32, f32),
}

/// One character's animation state.
#[derive(Clone, Debug, PartialEq)]
pub struct Animator {
    pub set: MoveSet,
    pub mixer: AnimMixer,
    /// The locomotion move ("idle", "walk", ...).
    base: String,
    /// The one-shot playing, its seconds in, and whether its hit cue has gone.
    action: Option<(String, f32, bool)>,
    /// Holding a finished death (or another held move).
    held: Option<String>,
    speed: f32,
    grounded: bool,
    vy: f32,
    /// Seconds since landing (for the squash).
    landed: f32,
    /// The walk cycle (radians), for the bob.
    phase: f32,
}

impl Animator {
    pub fn new(set: MoveSet) -> Self {
        let mut a = Self {
            set,
            mixer: AnimMixer::new(),
            base: "idle".into(),
            action: None,
            held: None,
            speed: 0.0,
            grounded: true,
            vy: 0.0,
            landed: 1.0,
            phase: 0.0,
        };
        a.start_clip("idle", true);
        a
    }

    /// The move now (a one-shot if one plays, else the locomotion).
    pub fn current(&self) -> &str {
        if let Some(h) = &self.held {
            return h;
        }
        match &self.action {
            Some((name, ..)) => name,
            None => &self.base,
        }
    }

    /// Is a one-shot (an attack, a flinch) still playing?
    pub fn busy(&self) -> bool {
        self.action.is_some()
    }

    /// Dead and holding the last frame.
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// How the body moves this frame: horizontal speed, on the ground or not, vertical speed.
    pub fn locomote(&mut self, speed: f32, grounded: bool, vy: f32) {
        if grounded && !self.grounded && self.vy < -1.0 {
            self.landed = 0.0;
        }
        self.speed = speed.max(0.0);
        self.grounded = grounded;
        self.vy = vy;
        let base = if !grounded {
            if vy > 0.0 {
                "jump"
            } else {
                "fall"
            }
        } else if self.speed >= self.set.run_from && self.set.moves.contains_key("run") {
            "run"
        } else if self.speed >= self.set.walk_from {
            "walk"
        } else {
            "idle"
        };
        if base != self.base {
            self.base = base.into();
            if self.action.is_none() && self.held.is_none() {
                self.start_clip(base, false);
            }
        }
    }

    /// Play a one-shot move now (restarts it if it is already playing). A move the set
    /// does not have is ignored. Returns whether it started.
    pub fn act(&mut self, name: &str) -> bool {
        if self.held.is_some() || !self.set.moves.contains_key(name) {
            return false;
        }
        self.action = Some((name.into(), 0.0, false));
        self.start_clip(name, true);
        true
    }

    /// Back to life (after a death hold): idle again.
    pub fn revive(&mut self) {
        self.held = None;
        self.action = None;
        self.base = "idle".into();
        self.start_clip("idle", true);
    }

    fn start_clip(&mut self, name: &str, restart: bool) {
        let Some(m) = self.set.moves.get(name) else {
            return;
        };
        if m.clip.is_empty() {
            return;
        }
        if restart
            && self
                .mixer
                .current
                .as_ref()
                .is_some_and(|c| c.name == m.clip)
        {
            // The same clip again from its start (two attacks in a row).
            self.mixer.play(&m.clip);
            if let Some(c) = self.mixer.current.as_mut() {
                c.time = 0.0;
            }
        } else {
            self.mixer.cross_fade_to(&m.clip, m.fade);
        }
        self.mixer.time_scale = m.speed;
    }

    /// Advance by `dt`. Returns the name of a one-shot whose blow lands this frame (its
    /// `hit_at`), so the kit deals the damage in time with the swing.
    pub fn tick(&mut self, dt: f32) -> Option<String> {
        self.landed += dt;
        let mut cue = None;
        if let Some((name, t, cued)) = self.action.as_mut() {
            let m = self
                .set
                .moves
                .get(name.as_str())
                .cloned()
                .unwrap_or_default();
            *t += dt * m.speed;
            if let Some(h) = m.hit_at {
                if !*cued && *t >= h {
                    *cued = true;
                    cue = Some(name.clone());
                }
            }
            if *t >= m.secs {
                let name = name.clone();
                self.action = None;
                if m.hold {
                    self.held = Some(name);
                } else {
                    let base = self.base.clone();
                    self.start_clip(&base, false);
                }
            }
        }
        // Walk and run clips keep pace with the body.
        let looping = self.action.is_none() && self.held.is_none();
        if looping && matches!(self.base.as_str(), "walk" | "run") {
            let m = self.set.moves.get(&self.base).cloned().unwrap_or_default();
            self.mixer.time_scale =
                m.speed * (self.speed / self.set.stride.max(0.1)).clamp(0.5, 2.5);
        }
        if self.held.is_none() {
            self.mixer.update(dt);
        }
        if self.grounded && self.speed >= self.set.walk_from {
            self.phase += dt * self.speed * 4.0;
        } else {
            self.phase *= 0.9;
        }
        cue
    }

    /// The procedural pose for models without clips (and a touch of squash for those with).
    pub fn pose(&self) -> Pose {
        let mut p = Pose {
            squash: (1.0, 1.0),
            ..Pose::default()
        };
        // Walking bob and a lean into the pace.
        if self.grounded && self.speed >= self.set.walk_from {
            p.lift = 0.06 * self.phase.sin().abs();
            p.lean = -(self.speed * 0.04).min(0.25);
        }
        if !self.grounded {
            let k = (self.vy * 0.04).clamp(-0.15, 0.15);
            p.squash = (1.0 - k, 1.0 + k);
        }
        if self.landed < 0.18 {
            let k = 0.18 * (1.0 - self.landed / 0.18);
            p.squash = (1.0 + k, 1.0 - k);
        }
        let (name, t, secs) = match (&self.held, &self.action) {
            (Some(h), _) => {
                let secs = self.set.moves.get(h).map_or(1.0, |m| m.secs);
                (h.as_str(), secs, secs)
            }
            (None, Some((n, t, _))) => {
                let secs = self.set.moves.get(n).map_or(1.0, |m| m.secs).max(1e-3);
                (n.as_str(), *t, secs)
            }
            _ => return p,
        };
        let u = (t / secs).clamp(0.0, 1.0);
        let swing = (u * std::f32::consts::PI).sin();
        match name {
            "die" => {
                p.roll = std::f32::consts::FRAC_PI_2 * super::tween::Ease::OutBounce.apply(u);
                p.lift = -0.05 * u;
            }
            "hurt" => {
                p.lean = 0.35 * swing;
                p.lunge = -0.15 * swing;
                p.squash = (1.0 + 0.1 * swing, 1.0 - 0.1 * swing);
            }
            "cheer" => {
                p.lift = 0.35 * (u * std::f32::consts::TAU * 2.0).sin().abs();
            }
            "cast" => {
                p.lift = 0.1 * swing;
                p.squash = (1.0 - 0.06 * swing, 1.0 + 0.1 * swing);
            }
            _ => {
                // Attacks and anything else: wind up back, lunge forward.
                let wind = if u < 0.35 {
                    -(u / 0.35)
                } else {
                    (u - 0.35) / 0.65
                };
                let k = if u < 0.35 {
                    wind
                } else {
                    (1.0 - wind) * 2.0 - 1.0
                };
                p.lean = -0.45 * k.max(-0.6);
                p.lunge = 0.35 * swing;
            }
        }
        p
    }

    /// Write the move into a walker (a rigged model): the clip and its crossfade, plus the
    /// pose's squash. A held move stays on its last frame.
    pub fn apply_walker(&self, w: &mut crate::world_doc::WorldWalker) {
        self.mixer.apply(w);
        // The renderer blends from the rest pose by `anim_blend`: full clip when one plays.
        w.anim_blend = if self.mixer.current.is_some() {
            1.0
        } else {
            0.0
        };
        if let Some(h) = &self.held {
            if let Some(m) = self.set.moves.get(h) {
                w.clip = (m.secs - 0.02).max(1e-3);
            }
        }
        let p = self.pose();
        w.anim = self.current().to_string();
        let s = w.scale.unwrap_or([1.0; 3]);
        if p.squash != (1.0, 1.0) {
            w.scale = Some([s[0] * p.squash.0, s[1] * p.squash.1, s[2] * p.squash.0]);
        }
    }

    /// Write the procedural pose into a prop standing at `base` facing `yaw`, at `scale`.
    pub fn apply_prop(
        &self,
        prop: &mut crate::world_doc::WorldProp,
        base: [f32; 3],
        yaw: f32,
        scale: f32,
    ) {
        let p = self.pose();
        let (fx, fz) = (yaw.sin(), yaw.cos());
        prop.position = [
            base[0] + fx * p.lunge,
            base[1] + p.lift.max(-0.2),
            base[2] + fz * p.lunge,
        ];
        prop.yaw = yaw;
        prop.pitch = p.lean;
        prop.roll = p.roll;
        prop.scale = [scale * p.squash.0, scale * p.squash.1, scale * p.squash.0];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn run(a: &mut Animator, secs: f32) -> Vec<String> {
        (0..(secs / DT) as usize)
            .filter_map(|_| a.tick(DT))
            .collect()
    }

    #[test]
    fn locomotion_picks_idle_walk_run_jump_and_fall() {
        let mut a = Animator::new(MoveSet::kaykit());
        assert_eq!(a.current(), "idle");
        a.locomote(1.0, true, 0.0);
        assert_eq!(a.current(), "walk");
        run(&mut a, 0.3);
        assert_eq!(a.mixer.current.as_ref().unwrap().name, "Walking_A");
        assert!(a.mixer.previous.is_none(), "the fade is done");
        a.locomote(5.0, true, 0.0);
        assert_eq!(a.current(), "run");
        assert_eq!(
            a.mixer.previous.as_ref().unwrap().name,
            "Walking_A",
            "crossfades"
        );
        a.locomote(5.0, false, 4.0);
        assert_eq!(a.current(), "jump");
        a.locomote(5.0, false, -4.0);
        assert_eq!(a.current(), "fall");
        a.locomote(0.0, true, 0.0);
        assert_eq!(a.current(), "idle");
        assert!(a.pose().squash.1 < 1.0, "a landing squashes");
    }

    #[test]
    fn an_attack_cues_its_blow_once_then_walks_on() {
        let mut a = Animator::new(MoveSet::kaykit());
        a.locomote(1.0, true, 0.0);
        assert!(a.act("attack"));
        assert_eq!(a.current(), "attack");
        assert_eq!(
            a.mixer.current.as_ref().unwrap().name,
            "1H_Melee_Attack_Chop"
        );
        let cues = run(&mut a, 0.4);
        assert!(cues.is_empty(), "the blow lands at 0.45 s");
        let cues = run(&mut a, 0.2);
        assert_eq!(cues, vec!["attack".to_string()]);
        run(&mut a, 0.6);
        assert!(!a.busy());
        assert_eq!(a.current(), "walk", "back to walking");
        assert!(!a.act("moonwalk"), "an unknown move is ignored");
    }

    #[test]
    fn death_holds_the_last_frame_until_revived() {
        let mut a = Animator::new(MoveSet::kaykit());
        a.act("die");
        run(&mut a, 2.0);
        assert!(a.holding());
        assert_eq!(a.current(), "die");
        assert!(!a.act("attack"), "the dead do not swing");
        let mut w = crate::world_doc::WorldWalker::default();
        a.apply_walker(&mut w);
        assert_eq!(w.clip_name.as_deref(), Some("Death_A"));
        assert!(
            (w.clip - 0.78).abs() < 1e-3,
            "on the last frame: {}",
            w.clip
        );
        a.revive();
        assert_eq!(a.current(), "idle");
    }

    #[test]
    fn models_without_clips_get_a_procedural_pose() {
        let mut a = Animator::new(MoveSet::procedural());
        a.locomote(2.0, true, 0.0);
        run(&mut a, 0.2);
        let p = a.pose();
        assert!(p.lean < 0.0, "leans into the walk");
        a.act("attack");
        run(&mut a, 0.25);
        assert!(a.pose().lunge > 0.1, "lunges at the target");
        a.act("die");
        run(&mut a, 1.0);
        assert!(
            (a.pose().roll - std::f32::consts::FRAC_PI_2).abs() < 0.05,
            "lies down"
        );
        let mut prop = crate::world_doc::WorldProp::default();
        a.apply_prop(&mut prop, [1.0, 0.0, 2.0], 0.0, 1.5);
        assert!((prop.roll - std::f32::consts::FRAC_PI_2).abs() < 0.05);
        assert_eq!(prop.scale[1], 1.5);
        assert!(
            a.mixer.current.is_none(),
            "no clip names for a static model"
        );
    }

    #[test]
    fn move_sets_read_from_json() {
        let set: MoveSet = serde_json::from_str(
            r#"{"moves": {"idle": {"clip": "Idle"}, "attack": {"clip": "Swing", "secs": 0.5, "hit_at": 0.2}}, "run_from": 4.0}"#,
        )
        .unwrap();
        assert_eq!(set.moves["attack"].hit_at, Some(0.2));
        assert_eq!(set.walk_from, 0.15);
        assert!(serde_json::from_str::<MoveSet>(r#"{"mooves": {}}"#).is_err());
    }
}

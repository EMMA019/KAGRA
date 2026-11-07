//! The 3D picture (rebuilt each frame: about 50 props): a ballpark in the middle, the ring of practice squares around it, the
//! coach walking the ring, the club on the field.

use super::board::Square;
use super::team::Pos;
use super::{DiamondDays, Mode, COACH_GLB, PITCHER_GLB, PLAYER_GLB};
use crate::game_core::declare_models;
use crate::game_core::view::View;
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::world_doc::{WorldCamera, WorldDoc};

/// The ring's radius (m) and where square `i` of `n` sits (x, z).
pub const RING: f32 = 8.6;

pub fn square_pos(i: usize, n: usize) -> (f32, f32) {
    // Start at the bottom (near the camera) and go round counter-clockwise on screen.
    let a = std::f32::consts::FRAC_PI_2 + i as f32 / n as f32 * std::f32::consts::TAU;
    (RING * a.cos(), RING * a.sin())
}

/// Where each fielder stands (x, z); home plate at (0, 3.6), centre field toward -z.
fn field_spot(p: Pos) -> (f32, f32) {
    match p {
        Pos::P => (0.0, 0.6),
        Pos::C => (0.0, 4.3),
        Pos::B1 => (2.4, 1.0),
        Pos::B2 => (1.2, -0.9),
        Pos::B3 => (-2.4, 1.0),
        Pos::SS => (-1.2, -0.9),
        Pos::LF => (-3.6, -3.4),
        Pos::CF => (0.0, -4.6),
        Pos::RF => (3.6, -3.4),
    }
}

impl DiamondDays {
    pub(super) fn view(&self) -> View {
        let (w, h) = self.size.get();
        let m = RING + 1.4;
        View::frame(
            [0.0, 0.0, 0.6],
            [0.0, 1.5, 1.0],
            42.0,
            &View::ground_box(-m, m, -m, m, 0.0),
            w / h.max(1.0),
        )
    }

    pub(super) fn update_doc(&mut self) {
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(super::GAME_ID.into()),
            half: 24.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([120, 170, 230, 255]),
            hemi_sky: Some([0.55, 0.62, 0.75]),
            hemi_ground: Some([0.3, 0.35, 0.22]),
            env_map: Some("preset:studio".into()),
            ibl: Some(0.3),
            exposure: Some(1.0),
            bloom: Some(0.25),
            bloom_threshold: Some(0.95),
            vignette: Some(0.3),
            fxaa: Some(true),
            tonemap: Some(true),
            ..WorldDoc::default()
        };
        let flat = |id: &str, pos: [f32; 3], size: [f32; 3], col: [u32; 3]| {
            prop(id, "box", id, pos, size, col, 0.0, 0.9, 0.0)
        };
        // The ground, the outfield grass, the infield dirt (a diamond: a box turned 45°).
        doc.props.push(flat(
            "ground",
            [0.0, -0.3, 0.0],
            [60.0, 0.4, 60.0],
            [96, 150, 86],
        ));
        // The outfield: a fan of grass (a square turned 45° behind the infield).
        let mut grass = flat("grass", [0.0, -0.06, -0.6], [7.6, 0.06, 7.6], [80, 168, 80]);
        grass.yaw = std::f32::consts::FRAC_PI_4;
        doc.props.push(grass);
        let mut dirt = flat("dirt", [0.0, -0.02, 1.6], [3.9, 0.06, 3.9], [190, 140, 90]);
        dirt.yaw = std::f32::consts::FRAC_PI_4;
        doc.props.push(dirt);
        let mut infield = flat("infield", [0.0, 0.0, 1.6], [2.9, 0.06, 2.9], [92, 172, 88]);
        infield.yaw = std::f32::consts::FRAC_PI_4;
        doc.props.push(infield);
        // Bases and the mound.
        for (i, (x, z)) in [(0.0, 3.6), (2.0, 1.6), (0.0, -0.4), (-2.0, 1.6)]
            .iter()
            .enumerate()
        {
            let mut b = flat(
                &format!("base-{i}"),
                [*x, 0.05, *z],
                [0.32, 0.08, 0.32],
                [250, 250, 250],
            );
            b.yaw = std::f32::consts::FRAC_PI_4;
            doc.props.push(b);
        }
        doc.props.push(flat(
            "mound",
            [0.0, 0.02, 1.6],
            [0.8, 0.1, 0.8],
            [200, 150, 100],
        ));
        // The ring of squares; the one the coach stands on glows.
        let n = self.spec.ring.len();
        for (i, sq) in self.spec.ring.iter().enumerate() {
            let (x, z) = square_pos(i, n);
            let here = i == self.s.pos;
            let mut p = prop(
                &format!("sq-{i}"),
                "box",
                "square",
                [x, if here { 0.12 } else { 0.05 }, z],
                [1.9, 0.2, 1.9],
                sq.color(),
                if here { 0.35 } else { 0.0 },
                0.6,
                0.0,
            );
            p.yaw = -(i as f32 / n as f32) * std::f32::consts::TAU;
            doc.props.push(p);
            if *sq == Square::Event {
                doc.props.push(prop(
                    &format!("sq-mark-{i}"),
                    "sphere",
                    "mark",
                    [x, 0.3, z],
                    [0.35; 3],
                    [255, 210, 80],
                    0.4,
                    0.4,
                    0.0,
                ));
            }
        }
        // The coach on the ring, hopping between squares.
        let (bx, bz) = square_pos(self.s.pos, n);
        let k = if matches!(self.mode, Mode::Walk { .. }) {
            self.hop
        } else {
            1.0
        };
        let (cx, cz) = if matches!(self.mode, Mode::Walk { .. }) {
            let (nx, nz) = square_pos((self.s.pos + 1) % n, n);
            (bx + (nx - bx) * k, bz + (nz - bz) * k)
        } else {
            (bx, bz)
        };
        let jump = (k * std::f32::consts::PI).sin() * 0.5;
        if glb_ready(COACH_GLB) {
            let mut coach = glb_prop(
                "coach",
                COACH_GLB,
                "coach",
                [0.0; 3],
                [1.0; 3],
                [255, 255, 255],
                0.0,
                None,
            );
            let face = (-cx).atan2(-cz);
            self.coach
                .apply_prop(&mut coach, [cx, 0.15 + jump, cz], face, 1.9);
            doc.props.push(coach);
        }
        // The club on the field: the line-up at their positions.
        if let Some(ace) = self.s.team.ace() {
            let nine = self.s.team.lineup(ace);
            for (k, &i) in nine.iter().enumerate() {
                let p = &self.s.team.players[i];
                let pos = if i == ace { Pos::P } else { p.pos };
                let (x, z) = field_spot(pos);
                // Two at one spot (a gap filled): step aside.
                let x = x + if k >= 8 && pos != Pos::P { 0.6 } else { 0.0 };
                let model = if pos == Pos::P {
                    PITCHER_GLB
                } else {
                    PLAYER_GLB
                };
                if glb_ready(model) {
                    let bob = ((self.clock * 2.0 + k as f32).sin() * 0.03).max(0.0);
                    let mut pr = glb_prop(
                        &format!("player-{k}"),
                        model,
                        "player",
                        [x, bob, z],
                        [1.7; 3],
                        [255, 255, 255],
                        0.0,
                        None,
                    );
                    pr.yaw = (0.0 - x).atan2(3.6 - z);
                    doc.props.push(pr);
                }
            }
        }
        declare_models(&mut doc, &[COACH_GLB, PLAYER_GLB, PITCHER_GLB]);
        let view = self.view();
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "park".into(),
            position: view.eye,
            target: view.target,
            fov: view.fov,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "sun",
            [4.0, 14.0, 8.0],
            [0.0; 3],
            [1.0, 0.97, 0.9],
            1.4,
            60.0,
            false,
        ));
        self.doc = doc;
    }

    pub(super) fn build_doc(&mut self) {
        self.update_doc();
    }
}

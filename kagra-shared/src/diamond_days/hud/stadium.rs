//! The game in 3D, under the broadcast HUD: a high-school ballpark (a dark dirt infield,
//! striped grass, chalk, a green fence, stands full of fans, light towers) and both nines as
//! [`chibi3d`] figures. Pitch by pitch the camera sits high behind the catcher, with the
//! batter on his side of the plate and the pitcher in the middle. A ball in play cuts to a
//! high view behind home that follows the ball while the runners and the fielders go.
//!
//! The picture is framed for the band between the scoreboard and the lower panel
//! ([`View::in_band`]). Everything here is primitives and two baked pictures (the field and
//! a block of stands), so the world rebuilds every frame without touching the GPU's meshes.

use super::broadcast::{broadcast_rect, in_play, location, Phase};
use crate::diamond_days::art;
use crate::diamond_days::board;
use crate::diamond_days::sim::{Game, Pitch, PlayKind};
use crate::diamond_days::{DiamondDays, Mode, GAME_ID};
use crate::game_core::chibi::{Kit, Pose};
use crate::game_core::chibi3d::{self, Detail, Figure};
use crate::game_core::portrait::{Expr, Face};
use crate::game_core::view::View;
use crate::hud_art::{Painter, Stage};
use crate::image2d::{self, PicId};
use crate::scene::DrawList;
use crate::vector::{self, mix, Paint, Rgba, P};
use crate::world_doc::{WorldCamera, WorldDoc, WorldProp};
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::sync::OnceLock;

/// Players from cleats to cap (m): deformed, and a little big so they read from the stands.
const H: f32 = 1.9;
const MOUND_Z: f32 = -18.44;
const MOUND_TOP: f32 = 0.2;
/// The field picture covers x -80..80 and z -135..25 (m).
const FIELD_X0: f32 = -80.0;
const FIELD_Z1: f32 = 25.0;
const FIELD_M: f32 = 160.0;
const FIELD_PX: u32 = 1024;
const GRASS: Rgba = [74, 138, 62, 255];
const GRASS_LIGHT: Rgba = [86, 152, 70, 255];
const DIRT: Rgba = [112, 84, 64, 255];
const DIRT_LIGHT: Rgba = [140, 106, 78, 255];
const TRACK: Rgba = [150, 116, 84, 255];
const CHALK: [u32; 3] = [246, 246, 240];
const FENCE: [u32; 3] = [34, 88, 64];
const WALL_H: f32 = 3.0;
/// Batter's box centre, off the plate.
const BOX_X: f32 = 0.95;

/// A point `dist` m from home at `ang` degrees (0 is straight to centre, + toward first).
fn at(dist: f32, ang: f32) -> [f32; 3] {
    let a = ang.to_radians();
    [a.sin() * dist, 0.0, -a.cos() * dist]
}

/// Home (0) and first, second, third.
fn base(b: usize) -> [f32; 3] {
    match b % 4 {
        0 => [0.0, 0.0, 0.0],
        1 => at(27.43, 45.0),
        2 => at(38.79, 0.0),
        _ => at(27.43, -45.0),
    }
}

/// How far the fence is at `ang` degrees: 92 m down the lines, 115 m to centre.
fn fence(ang: f32) -> f32 {
    92.0 + 23.0 * (ang.to_radians() * 2.0).cos().max(0.0)
}

fn lerp3(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
    ]
}

/// The yaw that faces from `from` toward `to` (chibi figures face +Z at yaw 0).
fn face_toward(from: [f32; 3], to: [f32; 3]) -> f32 {
    (to[0] - from[0]).atan2(to[2] - from[2])
}

/// Looks only: about one player in four throws and bats left.
fn lefty(name: &str) -> bool {
    art::bats_left(name)
}

fn rgb(c: Rgba) -> [u32; 3] {
    [c[0] as u32, c[1] as u32, c[2] as u32]
}

#[allow(clippy::too_many_arguments)]
fn prop(
    id: &str,
    model: &str,
    pos: [f32; 3],
    yaw: f32,
    pitch: f32,
    scale: [f32; 3],
    color: [u32; 3],
    toon: bool,
) -> WorldProp {
    WorldProp {
        id: id.into(),
        kind: "prop".into(),
        name: "ballpark".into(),
        model: model.into(),
        position: pos,
        yaw,
        pitch,
        scale,
        enabled: true,
        color: Some(color),
        roughness: 0.9,
        is_static: true,
        collider: Some("none".into()),
        toon,
        ..WorldProp::default()
    }
}

/// The ground as one picture: grass mown in stripes, the dark dirt of the infield and the
/// foul ground beside it, the mound, the warning track. Baked once.
fn field_picture() -> Option<PicId> {
    static PIC: OnceLock<Option<PicId>> = OnceLock::new();
    *PIC.get_or_init(|| {
        let n = FIELD_PX as f32;
        let s = n / FIELD_M;
        let px = |x: f32, z: f32| -> P { [(x - FIELD_X0) * s, (FIELD_Z1 - z) * s] };
        let mut list = DrawList::default();
        let m = &mut list.mesh;
        let rect = |x0: f32, y0: f32, x1: f32, y1: f32| [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
        vector::fill(m, &rect(0.0, 0.0, n, n), &Paint::Solid(GRASS));
        // Mown in stripes, 6 m wide, running out to centre.
        for i in 0..(FIELD_M / 6.0) as usize {
            if i % 2 == 0 {
                let x0 = i as f32 * 6.0 * s;
                vector::fill(
                    m,
                    &rect(x0, 0.0, x0 + 6.0 * s, n),
                    &Paint::Solid(GRASS_LIGHT),
                );
            }
        }
        // The warning track along the fence.
        let mut track = Vec::new();
        let mut inner = Vec::new();
        for i in 0..=60 {
            let a = -50.0 + i as f32 * (100.0 / 60.0);
            let o = at(fence(a) + 8.0, a);
            let k = at(fence(a) - 4.5, a);
            track.push(px(o[0], o[2]));
            inner.push(px(k[0], k[2]));
        }
        inner.reverse();
        track.extend(inner);
        vector::fill(m, &track, &Paint::Solid(TRACK));
        // Dirt: the foul ground beside the infield and behind home...
        for side in [-1.0f32, 1.0] {
            let line = at(64.0, 45.0 * side);
            let poly = [
                px(0.0, 0.0),
                px(line[0], line[2]),
                px(side * 80.0, line[2]),
                px(side * 80.0, 25.0),
                px(0.0, 25.0),
            ];
            vector::fill(m, &poly, &Paint::Solid(DIRT));
        }
        // ...and the whole infield, out past the bases.
        let mound = px(0.0, MOUND_Z);
        vector::fill(
            m,
            &vector::circle(mound[0], mound[1], 29.0 * s),
            &Paint::Solid(DIRT),
        );
        // Where the infield meets the grass, a darker rim.
        let mut rim = vector::arc(mound[0], mound[1], 29.0 * s, 29.0 * s, 0.0, PI * 2.0, 160);
        rim.push(rim[0]);
        vector::stroke(m, &rim, 2.0, [64, 110, 52, 255]);
        // Lighter dirt where feet scuff it: the mound and the base paths.
        vector::fill(
            m,
            &vector::circle(mound[0], mound[1], 2.9 * s),
            &Paint::Solid(DIRT_LIGHT),
        );
        for b in 0..4 {
            let (p, q) = (base(b), base(b + 1));
            vector::stroke(
                m,
                &[px(p[0], p[2]), px(q[0], q[2])],
                1.2 * s,
                mix(DIRT, DIRT_LIGHT, 0.5),
            );
        }
        let home = px(0.0, 0.0);
        vector::fill(
            m,
            &vector::circle(home[0], home[1], 4.0 * s),
            &Paint::Solid(mix(DIRT, DIRT_LIGHT, 0.4)),
        );
        image2d::bake(&list, FIELD_PX, FIELD_PX).ok()
    })
}

/// A block of the stands: concrete steps and seats with the fans of the crowd sheet on
/// them. Tiled along every stand.
fn stand_picture() -> Option<PicId> {
    static PIC: OnceLock<Option<PicId>> = OnceLock::new();
    *PIC.get_or_init(|| {
        let sheet = art::crowd_sheet()?;
        let (w, h) = (192u32, 96u32);
        let (fw, fh) = (w as f32, h as f32);
        let mut list = DrawList::default();
        let mut p = Painter::new(&mut list, Stage::fit(fw, fh, fw, fh));
        for r in 0..4 {
            let y = r as f32 * fh / 4.0;
            let row = |y0: f32, y1: f32| [[0.0, y0], [fw, y0], [fw, y1], [0.0, y1]];
            p.fill(&row(y, y + fh / 4.0), Paint::Solid([176, 178, 186, 255]));
            p.fill(
                &row(y + fh * 0.07, y + fh * 0.2),
                Paint::Solid([62, 88, 140, 255]),
            );
        }
        p.image_part(sheet, [0.0, 0.0, fw, fh], art::crowd_uv(0), [255; 4]);
        image2d::bake(&list, w, h).ok()
    })
}

/// The ballpark around the field: ground, mound, plate, bases, chalk, the fence and its
/// yellow cap, the foul poles, the stands, the scoreboard and the light towers.
fn ballpark(out: &mut Vec<WorldProp>, night: bool, cheer: f32, clock: f32) {
    if let Some(pic) = field_picture() {
        let mut ground = prop(
            "ground",
            "plane",
            [FIELD_X0 + FIELD_M * 0.5, 0.0, FIELD_Z1 - FIELD_M * 0.5],
            0.0,
            0.0,
            [FIELD_M, 1.0, FIELD_M],
            [255, 255, 255],
            false,
        );
        ground.texture = Some(image2d::texture_spec(pic));
        ground.roughness = 1.0;
        out.push(ground);
    }
    // Grass beyond the picture, under the stands.
    out.push(prop(
        "outer",
        "box",
        [0.0, -0.06, -60.0],
        0.0,
        0.0,
        [420.0, 0.1, 420.0],
        rgb(GRASS),
        false,
    ));
    out.push(prop(
        "mound",
        "sphere",
        [0.0, MOUND_TOP - 0.25, MOUND_Z],
        0.0,
        0.0,
        [5.6, 0.5, 5.6],
        rgb(DIRT_LIGHT),
        false,
    ));
    out.push(prop(
        "rubber",
        "box",
        [0.0, MOUND_TOP + 0.01, MOUND_Z],
        0.0,
        0.0,
        [0.61, 0.04, 0.15],
        CHALK,
        false,
    ));
    // Home plate: a pentagon from a square and a diamond.
    out.push(prop(
        "plate",
        "box",
        [0.0, 0.012, -0.11],
        0.0,
        0.0,
        [0.43, 0.02, 0.215],
        CHALK,
        false,
    ));
    out.push(prop(
        "plate2",
        "box",
        [0.0, 0.012, 0.0],
        FRAC_PI_4,
        0.0,
        [0.304, 0.02, 0.304],
        CHALK,
        false,
    ));
    for b in 1..4 {
        let p = base(b);
        out.push(prop(
            &format!("base{b}"),
            "box",
            [p[0], 0.05, p[2]],
            FRAC_PI_4,
            0.0,
            [0.42, 0.1, 0.42],
            CHALK,
            true,
        ));
    }
    // Chalk: the foul lines out to the fence, the batter's boxes.
    for side in [-1.0f32, 1.0] {
        let end = at(fence(45.0), 45.0 * side);
        out.push(prop(
            &format!("line{side}"),
            "box",
            [end[0] * 0.5, 0.012, end[2] * 0.5],
            side * 3.0 * FRAC_PI_4,
            0.0,
            [0.12, 0.02, fence(45.0)],
            CHALK,
            false,
        ));
        let cx = side * BOX_X;
        for (dx, dz, sx, sz) in [
            (-0.6, 0.0, 0.08, 1.8),
            (0.6, 0.0, 0.08, 1.8),
            (0.0, -0.9, 1.2, 0.08),
            (0.0, 0.9, 1.2, 0.08),
        ] {
            out.push(prop(
                &format!("box{side}{dx}{dz}"),
                "box",
                [cx + dx, 0.011, dz],
                0.0,
                0.0,
                [sx, 0.02, sz],
                CHALK,
                false,
            ));
        }
    }
    // The fence and the stands, in segments from pole to pole, then down both lines and
    // round behind home.
    let mut wall: Vec<[f32; 3]> = (0..=20)
        .map(|i| {
            let a = -45.0 + i as f32 * 4.5;
            at(fence(a), a)
        })
        .collect();
    let pole = |s: f32| at(fence(45.0), 45.0 * s);
    let along = |s: f32, d: f32, off: f32| {
        // Down the line from the pole toward home, `off` m out into foul ground.
        let p = at(d, 45.0 * s);
        let n = [s * FRAC_PI_4.cos(), 0.0, FRAC_PI_4.sin()];
        [p[0] + n[0] * off, 0.0, p[2] + n[2] * off]
    };
    let right: Vec<[f32; 3]> = [
        along(1.0, fence(45.0), 6.0),
        along(1.0, 60.0, 18.0),
        along(1.0, 30.0, 18.0),
        along(1.0, 6.0, 18.0),
    ]
    .to_vec();
    // Round behind home from the end of the first-base side to the third-base side.
    let start = right[right.len() - 1];
    let r = (start[0] * start[0] + start[2] * start[2]).sqrt();
    let a0 = start[0].atan2(start[2]);
    let behind: Vec<[f32; 3]> = (1..6)
        .map(|i| {
            let a = a0 - 2.0 * a0 * i as f32 / 6.0;
            [a.sin() * r, 0.0, a.cos() * r]
        })
        .collect();
    let left: Vec<[f32; 3]> = right.iter().rev().map(|p| [-p[0], 0.0, p[2]]).collect();
    wall.extend(right);
    wall.extend(behind);
    wall.extend(left);
    wall.push(wall[0]);
    let stand_tex = stand_picture().map(image2d::texture_spec);
    for (i, w) in wall.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        let d = [b[0] - a[0], b[2] - a[2]];
        let len = (d[0] * d[0] + d[1] * d[1]).sqrt();
        if len < 0.5 {
            continue;
        }
        let mid = [(a[0] + b[0]) * 0.5, 0.0, (a[2] + b[2]) * 0.5];
        let yaw = d[0].atan2(d[1]);
        // Outward: away from the middle of the field.
        let centre = [0.0, 0.0, -40.0];
        let mut out_dir = [-d[1] / len, d[0] / len];
        if out_dir[0] * (mid[0] - centre[0]) + out_dir[1] * (mid[2] - centre[2]) < 0.0 {
            out_dir = [-out_dir[0], -out_dir[1]];
        }
        out.push(prop(
            &format!("wall{i}"),
            "box",
            [mid[0], WALL_H * 0.5, mid[2]],
            yaw,
            0.0,
            [0.5, WALL_H, len + 0.3],
            FENCE,
            false,
        ));
        out.push(prop(
            &format!("cap{i}"),
            "box",
            [mid[0], WALL_H + 0.06, mid[2]],
            yaw,
            0.0,
            [0.56, 0.14, len + 0.3],
            [236, 200, 60],
            false,
        ));
        // The stands rise outward from the top of the wall.
        let (run, rise) = (16.0f32, 10.0f32);
        let slope = (run * run + rise * rise).sqrt();
        let pitch = (rise / run).atan();
        // On a hit the stands jump, block by block.
        let bounce = cheer * 0.5 * (clock * 11.0 + i as f32 * 1.3).sin().max(0.0);
        let c = [
            mid[0] + out_dir[0] * (run * 0.5 + 0.6),
            WALL_H + 0.4 + rise * 0.5 + bounce,
            mid[2] + out_dir[1] * (run * 0.5 + 0.6),
        ];
        let mut stand = prop(
            &format!("stand{i}"),
            "plane",
            c,
            out_dir[0].atan2(out_dir[1]),
            pitch,
            // Wider than the wall: neighbouring blocks fan apart as they rise.
            [len * 1.25 + 2.0, 1.0, slope],
            [255, 255, 255],
            false,
        );
        stand.texture = stand_tex.clone();
        stand.uv_scale = Some([3.0, 3.0]);
        out.push(stand);
        // The back of the stands.
        out.push(prop(
            &format!("back{i}"),
            "box",
            [
                mid[0] + out_dir[0] * (run + 1.2),
                (WALL_H + rise) * 0.5 + 1.0,
                mid[2] + out_dir[1] * (run + 1.2),
            ],
            yaw,
            0.0,
            [1.0, WALL_H + rise + 2.0, len + 1.0],
            [150, 154, 166],
            false,
        ));
    }
    // The foul poles.
    for s in [-1.0f32, 1.0] {
        let p = pole(s);
        out.push(prop(
            &format!("pole{s}"),
            "cylinder",
            [p[0], 0.0, p[2]],
            0.0,
            0.0,
            [0.5, 22.0, 0.5],
            [250, 214, 60],
            false,
        ));
    }
    // The scoreboard over centre field.
    let sb = at(fence(0.0) + 19.0, 0.0);
    out.push(prop(
        "board",
        "box",
        [sb[0], 21.0, sb[2]],
        0.0,
        0.0,
        [34.0, 14.0, 2.4],
        [26, 48, 40],
        false,
    ));
    out.push(prop(
        "board-face",
        "box",
        [sb[0], 21.6, sb[2] + 1.25],
        0.0,
        0.0,
        [30.0, 9.0, 0.1],
        [14, 22, 20],
        false,
    ));
    for row in 0..3 {
        let mut lamp = prop(
            &format!("board-row{row}"),
            "box",
            [sb[0], 19.0 + row as f32 * 2.6, sb[2] + 1.32],
            0.0,
            0.0,
            [26.0, 0.5, 0.06],
            [250, 220, 120],
            false,
        );
        lamp.emissive = Some(if night { 2.2 } else { 0.8 });
        out.push(lamp);
    }
    // The sky plates stand round the park, past the light towers.
    if let Some(pic) = crate::diamond_days::sprites::sky_picture(night) {
        let tex = image2d::texture_spec(pic);
        for i in 0..10 {
            let a = (i as f32 / 10.0) * PI * 2.0;
            let (r, cz) = (230.0f32, -40.0);
            let p = [a.sin() * r, 70.0, cz + a.cos() * r];
            // A thin box, not a card: cards draw over the scene without a depth test.
            let mut plate = prop(
                &format!("sky{i}"),
                "box",
                p,
                face_toward(p, [0.0, 0.0, cz]),
                0.0,
                [2.0 * r * (PI / 10.0).tan() + 4.0, 170.0, 0.5],
                [255, 255, 255],
                false,
            );
            plate.texture = Some(tex.clone());
            // Self-lit, but under 0.3: above that a prop is drawn as a glow, pulled in
            // front of the stands.
            plate.emissive = Some(0.28);
            out.push(plate);
        }
    }
    // Light towers, lamps lit at night.
    for (i, (d, a)) in [(120.0, -52.0), (120.0, 52.0), (60.0, -78.0), (60.0, 78.0)]
        .into_iter()
        .enumerate()
    {
        let p = at(d, a);
        out.push(prop(
            &format!("tower{i}"),
            "cylinder",
            [p[0], 0.0, p[2]],
            0.0,
            0.0,
            [0.9, 34.0, 0.9],
            [150, 154, 166],
            false,
        ));
        let mut lamps = prop(
            &format!("lamps{i}"),
            "box",
            [p[0], 35.0, p[2]],
            face_toward(p, [0.0, 0.0, -40.0]),
            0.3,
            [8.0, 3.6, 0.6],
            if night {
                [255, 250, 230]
            } else {
                [196, 200, 210]
            },
            false,
        );
        if night {
            lamps.emissive = Some(4.0);
        }
        out.push(lamps);
    }
}

/// How far and how high a ball in play goes (m).
fn flight(kind: PlayKind) -> (f32, f32) {
    match kind {
        PlayKind::Hit(4) => (128.0, 34.0),
        PlayKind::Hit(3) => (96.0, 16.0),
        PlayKind::Hit(2) => (86.0, 14.0),
        PlayKind::Hit(_) => (52.0, 5.0),
        PlayKind::FlyOut | PlayKind::SacFly => (76.0, 28.0),
        PlayKind::GroundOut | PlayKind::DoublePlay => (34.0, 0.0),
        PlayKind::Bunt => (9.0, 0.0),
        _ => (0.0, 0.0),
    }
}

/// A point on the base paths: `p` bases from home (0..=4).
fn on_paths(p: f32) -> [f32; 3] {
    let p = p.clamp(0.0, 4.0);
    let seg = (p.floor() as usize).min(3);
    lerp3(base(seg), base(seg + 1), p - seg as f32)
}

/// Where the nine stand, by fielding position (pitcher first).
fn fielder_spot(i: usize) -> [f32; 3] {
    let (d, a) = art::FIELDERS[i];
    let p = at(d, a);
    if i == 0 {
        [p[0], MOUND_TOP, p[2]]
    } else {
        p
    }
}

/// How a finished game ends on screen (its last line).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::diamond_days) enum Finale {
    /// The home side won it in its last turn at bat: `hero` drove the run in, and his
    /// bench pours out to him.
    WalkOff { hero: String },
    /// Both nines line up across the plate and bow; in a tournament the winners then
    /// sing their school song facing the field, and the losers run to their stands.
    LineUp,
    /// We won a tournament outright (not by a walk-off): everyone runs to the mound and
    /// makes a ring, fingers to the sky.
    Crown,
}

/// A bow over `k` 0..1: up, held a moment, back up (0 upright, 1 the deepest point).
fn bow_at(k: f32, from: f32) -> f32 {
    let k = k - from;
    if k <= 0.0 {
        0.0
    } else if k < 0.14 {
        k / 0.14
    } else if k < 0.24 {
        1.0
    } else {
        (1.0 - (k - 0.24) / 0.14).max(0.0)
    }
}

impl DiamondDays {
    /// The last line of a finished game is on screen: how the game ends.
    pub(in crate::diamond_days) fn finale(&self, g: &Game, shown: usize) -> Option<Finale> {
        if !g.over || shown == 0 || shown != g.log.len() || g.log[shown - 1].is_pa() {
            return None;
        }
        let at = g.log.iter().rposition(|q| q.is_pa())?;
        let last = &g.log[at];
        // A walk-off ends on the play itself: one shout after it, the home side ahead.
        let walk_off = last.side == 1 && last.runs > 0 && g.runs(1) > g.runs(0) && at + 2 == shown;
        let title =
            self.cup_game().is_some_and(|f| f.is_final()) && g.runs(g.us) > g.runs(1 - g.us);
        Some(if walk_off {
            Finale::WalkOff {
                hero: last.batter.clone(),
            }
        } else if title {
            Finale::Crown
        } else {
            Finale::LineUp
        })
    }

    /// We bat with a runner in scoring position: the band plays the chance theme.
    pub(in crate::diamond_days) fn our_chance(
        g: &Game,
        pl: &crate::diamond_days::sim::Play,
    ) -> bool {
        pl.is_pa() && pl.side as usize == g.us && pl.bases0 & 0b110 != 0
    }

    /// The tournament game on screen, if this game is one.
    pub(in crate::diamond_days) fn cup_game(&self) -> Option<crate::diamond_days::board::Fixture> {
        self.game_this_week().filter(|f| f.is_cup())
    }

    /// A side's uniform (caps, not helmets).
    fn side_kit(&self, g: &Game, side: usize) -> Kit {
        let c = self.side_color(g, side);
        if side == 1 {
            Kit::home(c)
        } else {
            Kit::road(c)
        }
    }

    /// The end of the game, `k` 0..1 through its last line.
    fn finale_scene(&self, g: &Game, f: &Finale, k: f32, out: &mut Vec<WorldProp>) -> View {
        let clock = self.clock;
        let hop = |i: usize| (clock * 7.0 + i as f32 * 1.9).sin().abs() * 0.3;
        match f {
            Finale::WalkOff { hero } => {
                // The losers: heads down where they stood.
                let fkit = self.side_kit(g, 0);
                let ckit = Kit { gear: true, ..fkit };
                let pitcher = g.pitcher_now().name.clone();
                let ids: Vec<String> = (0..9).map(|i| format!("f{i}")).collect();
                for (i, id) in ids.iter().enumerate() {
                    let p = fielder_spot(i);
                    let face = self.fielder_face(g, 0, i, &pitcher);
                    let mut fig = Figure::new(id, p, face_toward(p, base(0)), H, &face, &fkit)
                        .pose(Pose::Bow(if i == 0 { 0.75 } else { 0.4 }))
                        .expr(Expr::Sad)
                        .detail(Detail::Far);
                    if i == 1 {
                        fig.kit = &ckit;
                        fig = fig.mitt(true);
                    }
                    chibi3d::build(&fig, out);
                }
                // The hero, jumping, between home and first.
                let spot = lerp3(base(0), base(1), 0.6);
                let wkit = self.side_kit(g, 1);
                let helmet = Kit {
                    helmet: true,
                    ..wkit
                };
                let hface = self.face_for(g, 1, hero);
                let eye_dir = {
                    let d = [-spot[0], -22.0 - spot[2]];
                    let n = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
                    let a = (k - 0.5) * 0.4;
                    let (sa, ca) = a.sin_cos();
                    [(d[0] * ca - d[1] * sa) / n, (d[0] * sa + d[1] * ca) / n]
                };
                let cam = [
                    spot[0] + eye_dir[0] * 15.0,
                    0.0,
                    spot[2] + eye_dir[1] * 15.0,
                ];
                chibi3d::build(
                    &Figure::new(
                        "hero",
                        [spot[0], hop(0) * 1.6, spot[2]],
                        face_toward(spot, cam),
                        H,
                        &hface,
                        &helmet,
                    )
                    .pose(Pose::Cheer)
                    .expr(Expr::Smile)
                    .lefty(lefty(hero))
                    .glove(false),
                    out,
                );
                // The bench pours out of the first-base dugout and rings him.
                let mut mates: Vec<(String, Face)> = g.sides[1]
                    .bats
                    .iter()
                    .filter(|b| b.name != *hero)
                    .map(|b| (b.name.clone(), self.face_for(g, 1, &b.name)))
                    .collect();
                for i in 0..5 {
                    let name = format!("{}-bench-{i}", g.sides[1].name);
                    let face = art::face_of(&name);
                    mates.push((name, face));
                }
                let dugout = [25.0, 0.0, -3.0];
                let toward_cam = (cam[2] - spot[2]).atan2(cam[0] - spot[0]);
                let n = mates.len().max(1);
                let ids: Vec<String> = (0..mates.len()).map(|j| format!("mate{j}")).collect();
                for (j, (name, face)) in mates.iter().enumerate() {
                    // Round him, leaving the side toward the camera open so he shows.
                    let open = 1.1;
                    let a = toward_cam
                        + open
                        + j as f32 / (n.max(2) - 1) as f32 * (std::f32::consts::TAU - 2.0 * open);
                    let ring = 1.7 + (j % 2) as f32 * 0.8;
                    let goal = [spot[0] + a.cos() * ring, 0.0, spot[2] + a.sin() * ring];
                    let start = [
                        dugout[0] + (j % 4) as f32 * 0.9,
                        0.0,
                        dugout[2] - (j / 4) as f32 * 1.1,
                    ];
                    let run = ((k - j as f32 * 0.012) / 0.3).clamp(0.0, 1.0);
                    let e = 1.0 - (1.0 - run).powi(2);
                    let mut p = lerp3(start, goal, e);
                    let (pose, yaw) = if run < 1.0 {
                        (
                            Pose::Run(clock * 1.8 + j as f32 * 0.3),
                            face_toward(start, goal),
                        )
                    } else {
                        p[1] = hop(j + 1);
                        (Pose::Cheer, face_toward(goal, spot))
                    };
                    chibi3d::build(
                        &Figure::new(&ids[j], p, yaw, H, face, &wkit)
                            .pose(pose)
                            .expr(Expr::Happy)
                            .lefty(lefty(name))
                            .glove(false)
                            .detail(Detail::Far),
                        out,
                    );
                }
                View {
                    eye: [cam[0], 8.5, cam[2]],
                    target: [spot[0], 0.9, spot[2]],
                    fov: 30.0,
                }
            }
            Finale::LineUp => {
                // Two rows in front of the plate, facing each other (the visitors on the
                // third-base side), the umpire at the plate. They bow together. In a
                // tournament the winners then turn to the field for their school song and
                // the losers run to their stands to bow; after a practice game the winners
                // just jump.
                let cup = self.cup_game().is_some();
                let won = |side: usize| g.runs(side) > g.runs(1 - side);
                let ids: Vec<Vec<String>> = (0..2)
                    .map(|s| (0..9).map(|j| format!("row{s}-{j}")).collect())
                    .collect();
                for (side, ids) in ids.iter().enumerate() {
                    let kit = self.side_kit(g, side);
                    let x = if side == 0 { -1.5 } else { 1.5 };
                    let yaw = if side == 0 { FRAC_PI_2 } else { -FRAC_PI_2 };
                    for (j, b) in g.sides[side].bats.iter().take(9).enumerate() {
                        let face = self.face_for(g, side, &b.name);
                        let after = ((k - 0.6) / 0.4).clamp(0.0, 1.0);
                        let home = [x, 0.0, -1.6 - j as f32 * 0.95];
                        let mut p = home;
                        let mut facing = yaw;
                        let (pose, expr) = if k <= 0.6 {
                            (Pose::Bow(bow_at(k, 0.14)), Expr::Serious)
                        } else if won(side) && cup {
                            // The school song: facing the field, standing tall.
                            facing = PI;
                            (Pose::Stand, Expr::Smile)
                        } else if won(side) {
                            p[1] = hop(j + side * 9);
                            (Pose::Cheer, Expr::Happy)
                        } else if cup {
                            // Off to their own stands, then a deep bow.
                            let stands = [x.signum() * 24.0, 0.0, 2.0 - j as f32 * 1.1];
                            let run = (after / 0.6).min(1.0);
                            p = lerp3(home, stands, 1.0 - (1.0 - run).powi(2));
                            if run < 1.0 {
                                facing = face_toward(home, stands);
                                (Pose::Run(clock * 1.8 + j as f32 * 0.3), Expr::Sad)
                            } else {
                                facing = face_toward(stands, [x.signum() * 40.0, 0.0, 8.0]);
                                (Pose::Bow(bow_at((after - 0.6) * 1.2, 0.0)), Expr::Sad)
                            }
                        } else {
                            (Pose::Bow(0.4), Expr::Sad)
                        };
                        chibi3d::build(
                            &Figure::new(&ids[j], p, facing, H, &face, &kit)
                                .pose(pose)
                                .expr(expr)
                                .lefty(lefty(&b.name))
                                .glove(false),
                            out,
                        );
                    }
                }
                let ump = Kit {
                    cap: [26, 28, 38, 255],
                    shirt: [40, 46, 64, 255],
                    trim: [30, 34, 48, 255],
                    pants: [92, 96, 108, 255],
                    socks: [26, 28, 38, 255],
                    number: None,
                    helmet: false,
                    gear: false,
                };
                let uface = Face::from_seed(4242);
                chibi3d::build(
                    &Figure::new("ump", [0.0, 0.0, 0.2], PI, H, &uface, &ump)
                        .pose(Pose::Bow(bow_at(k, 0.14) * 0.6))
                        .glove(false),
                    out,
                );
                // From behind the mound, the stands behind the plate beyond them, easing in.
                let push = k.min(0.62) / 0.62;
                let lineup = View {
                    eye: [0.0, 8.0 - push * 1.2, -25.0 + push * 3.0],
                    target: [0.0, 0.8, -4.6],
                    fov: 30.0,
                };
                // After a tournament loss the camera follows the losers to their stands.
                let loser = (0..2).find(|&side| !won(side) && won(1 - side));
                match loser.filter(|_| cup) {
                    Some(side) if k > 0.6 => {
                        let x = if side == 0 { -1.0 } else { 1.0 };
                        let stands = View {
                            eye: [x * 6.0, 7.0, -16.0],
                            target: [x * 22.0, 1.0, -2.0],
                            fov: 40.0,
                        };
                        let w = ((k - 0.6) / 0.2).clamp(0.0, 1.0);
                        lineup.lerp(&stands, w * w * (3.0 - 2.0 * w))
                    }
                    _ => lineup,
                }
            }
            Finale::Crown => {
                // Everyone sprints from where they stood to the mound and makes a ring
                // round the pitcher, jumping, fingers to the sky; the other nine stand by
                // their bench with their heads down.
                let us = g.us;
                let kit = self.side_kit(g, us);
                let mound = [0.0, MOUND_TOP, MOUND_Z];
                let pitcher = g
                    .log
                    .iter()
                    .rev()
                    .find(|q| q.is_pa() && q.side as usize != us)
                    .map(|q| q.pitcher.clone())
                    .unwrap_or_default();
                let run = (k / 0.3).min(1.0);
                let e = 1.0 - (1.0 - run).powi(2);
                let ids: Vec<String> = (0..9).map(|i| format!("ring{i}")).collect();
                for (i, id) in ids.iter().enumerate() {
                    let face = self.fielder_face(g, us, i, &pitcher);
                    let a = i as f32 / 8.0 * std::f32::consts::TAU + 0.3;
                    let goal = if i == 0 {
                        mound
                    } else {
                        [a.cos() * 1.6, MOUND_TOP, MOUND_Z + a.sin() * 1.6]
                    };
                    let mut p = lerp3(fielder_spot(i), goal, e);
                    let (pose, yaw) = if run < 1.0 && i > 0 {
                        (
                            Pose::Run(clock * 1.8 + i as f32 * 0.3),
                            face_toward(fielder_spot(i), goal),
                        )
                    } else {
                        p[1] += hop(i) * 1.2;
                        (
                            Pose::Cheer,
                            face_toward(goal, mound) + if i == 0 { PI } else { 0.0 },
                        )
                    };
                    chibi3d::build(
                        &Figure::new(id, p, yaw, H, &face, &kit)
                            .pose(pose)
                            .expr(Expr::Happy)
                            .lefty(lefty(&pitcher) && i == 0)
                            .glove(false)
                            .detail(Detail::Far),
                        out,
                    );
                }
                let them = 1 - us;
                let tkit = self.side_kit(g, them);
                let bench_x: f32 = if them == 0 { -14.0 } else { 14.0 };
                let ids: Vec<String> = (0..9).map(|j| format!("lost{j}")).collect();
                for (j, b) in g.sides[them].bats.iter().take(9).enumerate() {
                    let face = self.face_for(g, them, &b.name);
                    let p = [
                        bench_x + (j % 3) as f32 * bench_x.signum() * 1.0,
                        0.0,
                        -2.0 - (j / 3) as f32 * 1.2,
                    ];
                    chibi3d::build(
                        &Figure::new(&ids[j], p, face_toward(p, mound), H, &face, &tkit)
                            .pose(Pose::Bow(0.5))
                            .expr(Expr::Sad)
                            .glove(false)
                            .detail(Detail::Far),
                        out,
                    );
                }
                // From behind home, low, the ring against the outfield and the stands.
                let push = k.min(1.0);
                View {
                    eye: [0.0, 5.5 + push * 1.0, 4.0 - push * 4.0],
                    target: [0.0, 1.2, MOUND_Z],
                    fov: 30.0,
                }
            }
        }
    }
}

impl DiamondDays {
    /// Who plays fielding position `i` (pitcher first): the play's pitcher on the mound,
    /// then the rest of the nine in order, each once (the pitcher is not in the field
    /// twice). `None` past the end of a short side.
    pub(in crate::diamond_days) fn fielder_name(
        g: &Game,
        side: usize,
        i: usize,
        pitcher: &str,
    ) -> Option<String> {
        if i == 0 {
            return Some(pitcher.to_string());
        }
        g.sides[side]
            .bats
            .iter()
            .filter(|b| b.name != pitcher)
            .nth(i - 1)
            .map(|b| b.name.clone())
    }

    /// The face of the fielder at position `i` ([`DiamondDays::fielder_name`]). The pitch
    /// view and the field view use the same faces, so nobody changes at the cut.
    fn fielder_face(&self, g: &Game, side: usize, i: usize, pitcher: &str) -> Face {
        Self::fielder_name(g, side, i, pitcher)
            .map(|n| self.face_for(g, side, &n))
            .unwrap_or_else(|| Face::from_seed(i as u64))
    }

    /// The band of the screen the 3D picture fills (fractions of the height, from the
    /// top): between the scoreboard and the lower panel.
    pub(in crate::diamond_days) fn match_band(&self) -> (f32, f32) {
        let (w, h) = self.size.get();
        let hh = self.stage(w, h).h.max(1.0);
        let r = broadcast_rect(hh);
        (r[1] / hh, (r[1] + r[3]) / hh)
    }

    /// The 3D world under the match HUD, or an empty world outside a game.
    pub(in crate::diamond_days) fn match_world(&self) -> WorldDoc {
        let (Mode::Match { shown, t, asking }, Some(g)) = (&self.mode, self.game.as_ref()) else {
            return WorldDoc::default();
        };
        let night = matches!(board::month(self.s.week), 1 | 2 | 10..=12);
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(GAME_ID.into()),
            half: 200.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some(if night {
                [12, 18, 40, 255]
            } else {
                [132, 190, 240, 255]
            }),
            hemi_sky: Some(if night {
                [0.32, 0.36, 0.5]
            } else {
                [0.62, 0.72, 0.9]
            }),
            hemi_ground: Some([0.26, 0.3, 0.2]),
            exposure: Some(if night { 1.05 } else { 1.0 }),
            fog_start: Some(380.0),
            fog_end: Some(900.0),
            fxaa: Some(true),
            bloom: night.then_some(0.55),
            bloom_threshold: Some(0.9),
            toon_outline: Some(0.022),
            ..WorldDoc::default()
        };
        // The crowd goes up on a hit or a run, most for a home run, and settles at the end.
        let cheer = match self.phase(g, *shown, *t, *asking) {
            (Phase::Result { k }, Some(i)) => g
                .log
                .get(i)
                .filter(|pl| pl.is_pa() && (pl.runs > 0 || matches!(pl.kind, PlayKind::Hit(_))))
                .map(|pl| {
                    let big = if matches!(pl.kind, PlayKind::Hit(4)) {
                        1.0
                    } else {
                        0.6
                    };
                    big * (1.0 - ((k - 0.75) * 4.0).max(0.0))
                })
                .unwrap_or(0.0),
            // Our chance: the band strikes up the chance theme and the stands sway.
            (Phase::Pitch { .. }, Some(i)) => g
                .log
                .get(i)
                .filter(|pl| Self::our_chance(g, pl))
                .map(|_| 0.3)
                .unwrap_or(0.0),
            _ => 0.0,
        };
        ballpark(&mut doc.props, night, cheer, self.clock);
        let view = self.match_scene(g, *shown, *t, *asking, &mut doc.props);
        let (y0, y1) = self.match_band();
        let v = view.in_band(y0, y1);
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "broadcast".into(),
            position: v.eye,
            target: v.target,
            fov: v.fov,
            roll: 0.0,
        });
        doc
    }

    /// The players and the ball for the moment on screen, and the camera for it.
    fn match_scene(
        &self,
        g: &Game,
        shown: usize,
        t: f32,
        asking: Option<crate::diamond_days::sim::Spot>,
        out: &mut Vec<WorldProp>,
    ) -> View {
        if let Some(f) = self.finale(g, shown) {
            let hold = self.spec.line_secs * self.line_pace(shown);
            let k = (t / hold.max(0.05)).clamp(0.0, 1.0);
            return self.finale_scene(g, &f, k, out);
        }
        let (phase, line) = self.phase(g, shown, t, asking);
        let play = line.and_then(|i| g.log.get(i));
        let pa = play.filter(|pl| pl.is_pa());
        let (bat_side, batter, pitcher, bases, on_base) = match (pa, phase) {
            (Some(pl), Phase::Pitch { .. } | Phase::Result { .. }) => (
                pl.side as usize,
                pl.batter.clone(),
                pl.pitcher.clone(),
                pl.bases0,
                pl.runners0.clone(),
            ),
            _ => (
                g.batting(),
                g.batter_now().name.clone(),
                g.pitcher_now().name.clone(),
                g.bases
                    .iter()
                    .enumerate()
                    .fold(0u8, |m, (i, x)| m | ((x.is_some() as u8) << i)),
                g.runner_names(),
            ),
        };
        let runners = (bases, &on_base);
        let fld_side = 1 - bat_side;
        let kit_for = |side: usize| {
            let c = self.side_color(g, side);
            if side == 1 {
                Kit::home(c)
            } else {
                Kit::road(c)
            }
        };
        let fkit = kit_for(fld_side);
        let bkit = Kit {
            helmet: true,
            ..kit_for(bat_side)
        };
        let ckit = Kit { gear: true, ..fkit };
        let ump = Kit {
            cap: [26, 28, 38, 255],
            shirt: [40, 46, 64, 255],
            trim: [30, 34, 48, 255],
            pants: [92, 96, 108, 255],
            socks: [26, 28, 38, 255],
            number: None,
            helmet: false,
            gear: false,
        };
        let clock = self.clock;
        let field_view =
            matches!((pa, phase), (Some(pl), Phase::Result { .. }) if in_play(pl.kind));
        let pitches = pa.map(|pl| pl.pitches()).unwrap_or_default();

        // The pitch: the pitcher's motion, the swing, the ball.
        let mut windup = 0.0;
        let mut swing: Option<f32> = None;
        let mut ball: Option<[f32; 3]> = None;
        // Where the catcher sets his mitt, and whether the pitch is in it.
        let mut mitt: Option<[f32; 3]> = None;
        let mut in_mitt = false;
        let mut batter_expr = Expr::Serious;
        let mut pitcher_expr = Expr::Fiery;
        let mut walk_k: Option<f32> = None;
        let bl = lefty(&batter);
        let pl_lefty = lefty(&pitcher);
        let pitcher_at = [0.0, MOUND_TOP, MOUND_Z];
        let pface = self.face_for(g, fld_side, &pitcher);
        let release = chibi3d::throwing_hand(
            &Figure::new("r", pitcher_at, 0.0, H, &pface, &fkit)
                .pose(Pose::Pitch(0.7))
                .lefty(pl_lefty),
        );
        match (pa, phase) {
            (Some(pl), Phase::Pitch { i, k }) => {
                let pitch = pitches[i];
                windup = if k < 0.5 {
                    k / 0.5 * 0.68
                } else {
                    0.68 + ((k - 0.5) / 0.3).min(1.0) * 0.32
                };
                if matches!(pitch, Pitch::Swinging | Pitch::Foul | Pitch::InPlay) {
                    swing = Some(((k - 0.56) / 0.3).clamp(0.0, 1.0));
                }
                let q = location(pl, i, pitch);
                let plate = [q[0] * 0.32, 0.82 - q[1] * 0.3, 0.1];
                let travel = (k - 0.5) / 0.24;
                if !matches!(pitch, Pitch::InPlay) && travel > 0.2 {
                    mitt = Some([plate[0], plate[1], 0.95]);
                }
                ball = if (0.0..=1.0).contains(&travel) {
                    let mut b = lerp3(release, plate, travel);
                    b[1] += (travel * PI).sin() * 0.35;
                    Some(b)
                } else if travel > 1.0 && matches!(pitch, Pitch::Foul) {
                    let a = (travel - 1.0) * 2.4;
                    Some([
                        plate[0] + a * 2.0,
                        plate[1] + a * 9.0 - a * a * 3.0,
                        plate[2] + a * 12.0,
                    ])
                } else if travel > 1.0 && !matches!(pitch, Pitch::InPlay) {
                    in_mitt = true;
                    Some([plate[0], plate[1], 1.0])
                } else {
                    None
                };
            }
            (Some(pl), Phase::Result { k }) => {
                windup = 1.0;
                match pl.kind {
                    PlayKind::Strikeout => {
                        if pitches.last() == Some(&Pitch::Swinging) {
                            swing = Some(1.0);
                        }
                        batter_expr = Expr::Sad;
                        pitcher_expr = Expr::Happy;
                    }
                    PlayKind::Walk => walk_k = Some(k),
                    PlayKind::HitByPitch => {
                        walk_k = Some(k);
                        batter_expr = Expr::Sad;
                    }
                    PlayKind::Hit(_) => {
                        batter_expr = Expr::Happy;
                        pitcher_expr = Expr::Surprised;
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        if field_view {
            return self.field_play(
                g,
                pa.expect("a play"),
                phase,
                (bat_side, &batter, runners),
                (&fkit, &bkit, &ckit),
                clock,
                out,
            );
        }

        // Behind the plate: pitcher, batter, catcher, umpire, the rest of the nine.
        chibi3d::build(
            &Figure::new("pitcher", pitcher_at, 0.0, H, &pface, &fkit)
                .pose(Pose::Pitch(windup))
                .expr(pitcher_expr)
                .lefty(pl_lefty),
            out,
        );
        let bface = self.face_for(g, bat_side, &batter);
        let side = if bl { 1.0 } else { -1.0 };
        let (bat_at, bat_pose) = match walk_k {
            // Ball four: he drops the bat and trots to first.
            Some(k) => (
                lerp3([side * BOX_X, 0.0, 0.0], base(1), (k * 0.9).min(1.0) * 0.35),
                Pose::Run(clock * 1.4),
            ),
            None => (
                [side * BOX_X, 0.0, 0.0],
                Pose::Bat(swing.unwrap_or(((clock * 1.3).sin() * 0.5 + 0.5) * 0.04)),
            ),
        };
        let bat_yaw = if walk_k.is_some() {
            face_toward(bat_at, base(1))
        } else if bl {
            -FRAC_PI_2
        } else {
            FRAC_PI_2
        };
        chibi3d::build(
            &Figure::new("batter", bat_at, bat_yaw, H, &bface, &bkit)
                .pose(bat_pose)
                .expr(batter_expr)
                .lefty(bl),
            out,
        );
        let cface = self.fielder_face(g, fld_side, 1, &pitcher);
        // He shifts toward the pitch and reaches the mitt to it.
        let shift = mitt.map(|m| (m[0] * 0.6).clamp(-0.25, 0.25)).unwrap_or(0.0);
        let catcher = Figure::new("catcher", [shift, 0.0, 1.35], PI, H, &cface, &ckit)
            .pose(Pose::Squat)
            .mitt(true)
            .reach(mitt);
        chibi3d::build(&catcher, out);
        if in_mitt {
            let m = chibi3d::glove_hand(&catcher);
            ball = Some([m[0], m[1], m[2] - 0.1]);
        }
        let uface = Face::from_seed(4242);
        chibi3d::build(
            // Over the catcher's shoulder on the batter's side.
            &Figure::new("ump", [side * 0.5, 0.0, 2.6], PI, H, &uface, &ump)
                .pose(Pose::Ready)
                .detail(Detail::Far),
            out,
        );
        let ids: Vec<String> = (2..9).map(|i| format!("f{i}")).collect();
        let faces: Vec<Face> = (2..9)
            .map(|i| self.fielder_face(g, fld_side, i, &pitcher))
            .collect();
        for (j, i) in (2..9).enumerate() {
            let p = fielder_spot(i);
            chibi3d::build(
                &Figure::new(
                    &ids[j],
                    p,
                    face_toward(p, [0.0, 0.0, 0.0]),
                    H,
                    &faces[j],
                    &fkit,
                )
                .pose(Pose::Ready)
                .detail(Detail::Far),
                out,
            );
        }
        self.runners(g, bat_side, runners, 0.0, 0, &bkit, clock, out);
        if let Some(b) = ball {
            push_ball(out, b, 0.24);
        }
        View {
            eye: [0.0, 7.0, 15.0],
            target: [0.0, 1.7, -14.0],
            fov: 27.0,
        }
    }

    /// Runners on base (`bases` bits, and who is on each bag), `ahead` bases on along the
    /// paths at `k`.
    #[allow(clippy::too_many_arguments)]
    fn runners(
        &self,
        g: &Game,
        bat_side: usize,
        (bases, names): (u8, &[String; 3]),
        k: f32,
        ahead: u8,
        kit: &Kit,
        clock: f32,
        out: &mut Vec<WorldProp>,
    ) {
        for (b, on_bag) in names.iter().enumerate() {
            if bases & (1 << b) == 0 {
                continue;
            }
            let from = (b + 1) as f32;
            let to = (from + ahead as f32).min(4.0);
            let p = from + (to - from) * k;
            let at = on_paths(p);
            // A lead off the bag while he waits.
            let lead = if ahead == 0 {
                lerp3(base(b + 1), base(b + 2), 0.12)
            } else {
                at
            };
            let moving = ahead > 0 && k < 1.0 && p < 4.0;
            let next = base((p.floor() as usize + 1).min(4));
            // An old save keeps the bags but not the names: any face will do there.
            let name = if on_bag.is_empty() {
                format!("runner{b}")
            } else {
                on_bag.clone()
            };
            let face = self.face_for(g, bat_side, &name);
            let id = format!("run{b}");
            chibi3d::build(
                &Figure::new(
                    &id,
                    lead,
                    if moving {
                        face_toward(lead, next)
                    } else {
                        face_toward(lead, [0.0, 0.0, MOUND_Z])
                    },
                    H,
                    &face,
                    kit,
                )
                .pose(if moving {
                    Pose::Run(clock * 1.6 + b as f32 * 0.3)
                } else {
                    Pose::Ready
                })
                .detail(Detail::Far),
                out,
            );
        }
    }

    /// The ball in play from the high view behind home: the flight, the fielder going for
    /// it, the batter running, the runners going round.
    #[allow(clippy::too_many_arguments)]
    fn field_play(
        &self,
        g: &Game,
        pl: &crate::diamond_days::sim::Play,
        phase: Phase,
        (bat_side, batter, runners): (usize, &str, (u8, &[String; 3])),
        (fkit, bkit, ckit): (&Kit, &Kit, &Kit),
        clock: f32,
        out: &mut Vec<WorldProp>,
    ) -> View {
        let k = if let Phase::Result { k } = phase {
            k
        } else {
            1.0
        };
        let fld_side = 1 - bat_side;
        let (dist, apex) = flight(pl.kind);
        let kk = (k * 1.4).min(1.0);
        let e = 1.0 - (1.0 - kk).powi(2);
        let land = at(dist, pl.dir);
        let ground = at(dist * e, pl.dir);
        let y = if apex > 0.5 {
            1.0 * (1.0 - kk) + 4.0 * apex * kk * (1.0 - kk)
        } else {
            // A grounder: low hops.
            ((kk * PI * 5.0).sin().abs() * 0.7 * (1.0 - kk)).max(0.08)
        };
        let out_play = !matches!(pl.kind, PlayKind::Hit(_));
        // The fielder nearest where it comes down goes for it.
        let chaser = (1..9)
            .filter(|i| *i != 1)
            .min_by(|a, b| {
                let d = |i: usize| {
                    let p = fielder_spot(i);
                    (p[0] - land[0]).powi(2) + (p[2] - land[2]).powi(2)
                };
                d(*a).total_cmp(&d(*b))
            })
            .unwrap_or(7);
        let ids: Vec<String> = (0..9).map(|i| format!("f{i}")).collect();
        let mut caught_at = None;
        // On a ground ball the first baseman takes the bag and the ball is thrown there.
        let grounder = matches!(
            pl.kind,
            PlayKind::GroundOut | PlayKind::DoublePlay | PlayKind::Bunt
        );
        let throw_to = (grounder && chaser != 2).then(|| {
            let b = base(1);
            [b[0] - 0.5, 0.0, b[2] + 0.5]
        });
        let mut first_glove = None;
        // Where the out is made, known before anyone is placed: the first baseman (an
        // earlier slot than most chasers) stretches toward it.
        let catch_point = (out_play && (k * 1.5) >= 1.0 && !matches!(pl.kind, PlayKind::Hit(4)))
            .then_some([land[0], 0.0, land[2]]);
        for (i, id) in ids.iter().enumerate() {
            let home_spot = fielder_spot(i);
            let face = self.fielder_face(g, fld_side, i, &pl.pitcher);
            let covering = (i == 2).then_some(throw_to).flatten();
            let (p, pose, yaw) = if let Some(bag) = covering {
                let go = (k * 2.4).min(1.0);
                let p = lerp3(home_spot, bag, go);
                let pose = if go < 1.0 {
                    Pose::Run(clock * 1.7)
                } else {
                    Pose::Ready
                };
                (p, pose, face_toward(p, [land[0], 0.0, land[2]]))
            } else if i == chaser && !matches!(pl.kind, PlayKind::Hit(4)) {
                // Out: he gets there. A hit: it gets past him.
                let reach = if out_play { 1.0 } else { 0.72 };
                let goal = lerp3(home_spot, [land[0], 0.0, land[2]], reach);
                let run = (k * 1.5).min(1.0);
                let p = lerp3(home_spot, goal, run);
                if out_play && run >= 1.0 {
                    caught_at = Some(p);
                }
                let pose = if run < 1.0 {
                    Pose::Run(clock * 1.7)
                } else {
                    Pose::Ready
                };
                (p, pose, face_toward(home_spot, goal))
            } else {
                (home_spot, Pose::Ready, face_toward(home_spot, ground))
            };
            let mut f = Figure::new(id, p, yaw, H, &face, fkit)
                .pose(pose)
                .detail(Detail::Far);
            if i == 1 {
                f.kit = ckit;
                f = f.mitt(true);
            }
            if covering.is_some() {
                f = f.reach(catch_point.map(|c| [c[0], 1.0, c[2]]));
                first_glove = Some(chibi3d::glove_hand(&f));
            }
            chibi3d::build(&f, out);
        }
        // The batter runs it out: round the bases on a hit, to first on anything else.
        let bases_got = match pl.kind {
            PlayKind::Hit(n) => n.min(4) as f32,
            _ => 1.0,
        };
        // A home run is a trot; on an out the throw just beats him to first.
        let pace = match pl.kind {
            PlayKind::Hit(4) => 0.9,
            PlayKind::Hit(_) => 1.3,
            _ => 1.05,
        };
        let prog = (k * pace * bases_got.max(1.0)).min(bases_got);
        let bp = on_paths(prog);
        let next = base((prog.floor() as usize + 1).min(4));
        let bface = self.face_for(g, bat_side, batter);
        chibi3d::build(
            &Figure::new("batter", bp, face_toward(bp, next), H, &bface, bkit)
                .pose(if prog < bases_got {
                    Pose::Run(clock * 1.7)
                } else if matches!(pl.kind, PlayKind::Hit(_)) {
                    Pose::Cheer
                } else {
                    Pose::Stand
                })
                .expr(if out_play { Expr::Sad } else { Expr::Happy })
                .lefty(lefty(batter))
                .detail(Detail::Far),
            out,
        );
        let ahead = match pl.kind {
            PlayKind::Hit(n) => n,
            PlayKind::SacFly if runners.0 & 4 != 0 => 1,
            _ => 0,
        };
        self.runners(
            g,
            bat_side,
            runners,
            (k * 1.3).min(1.0),
            ahead,
            bkit,
            clock,
            out,
        );
        // The ball (in the glove once caught), and its shadow on the grass.
        let throw = ((k - 0.74) / 0.2).clamp(0.0, 1.0);
        let ball = match (caught_at, first_glove) {
            // Fielded and thrown across to first.
            (Some(c), Some(to)) if kk >= 1.0 && throw > 0.0 => {
                let mut b = lerp3([c[0], 1.2, c[2]], to, throw);
                b[1] += (throw * PI).sin() * 1.6;
                b
            }
            (Some(c), _) if kk >= 1.0 => [c[0], 1.0, c[2]],
            _ => [ground[0], y, ground[2]],
        };
        push_ball(out, ball, 0.42);
        out.push(prop(
            "ball-shadow",
            "sphere",
            [ball[0], 0.02, ball[2]],
            0.0,
            0.0,
            [1.1, 0.02, 1.1],
            [30, 44, 28],
            false,
        ));
        // The camera follows the ball from high behind home.
        let look = lerp3([0.0, 0.0, -34.0], [ball[0], 0.0, ball[2]], 0.75);
        let far = (look[2].abs() / 120.0).clamp(0.0, 1.0);
        View {
            eye: [look[0] * 0.25, 24.0 + far * 10.0, 30.0],
            target: [look[0], 0.0, look[2]],
            fov: 44.0 + far * 6.0,
        }
    }
}

/// The ball, drawn bigger than life (`size` m across) and a little self-lit so it reads
/// against the stands and the sky (under the 0.3 glow threshold, so it still sorts).
fn push_ball(out: &mut Vec<WorldProp>, at: [f32; 3], size: f32) {
    let mut ball = prop(
        "ball",
        "sphere",
        at,
        0.0,
        0.0,
        [size; 3],
        [250, 250, 248],
        true,
    );
    ball.emissive = Some(0.25);
    out.push(ball);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_where_the_bases_are() {
        // 90 feet between bases, second straight out from home.
        let d = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
        for b in 0..4 {
            assert!((d(base(b), base(b + 1)) - 27.43).abs() < 0.05, "{b}");
        }
        assert!(
            base(1)[0] > 0.0,
            "first base is on the right from behind home"
        );
        assert!((fence(0.0) - 115.0).abs() < 0.1 && (fence(45.0) - 92.0).abs() < 0.1);
        assert!((on_paths(1.5)[0] - (base(1)[0] + base(2)[0]) * 0.5).abs() < 1e-4);
    }

    #[test]
    fn the_stands_share_one_picture() {
        let mut props = Vec::new();
        ballpark(&mut props, false, 0.0, 0.0);
        assert!(props.iter().any(|p| p.texture.is_some()));
        let doc = WorldDoc {
            version: 1,
            props,
            ..Default::default()
        };
        // Every stand shares one mesh, every sky plate another: the field, one block of
        // stands, the sky.
        let textured = doc
            .compile_meshes()
            .into_iter()
            .filter(|(_, m)| m.albedo.is_some())
            .count();
        assert_eq!(textured, 3);
    }
}

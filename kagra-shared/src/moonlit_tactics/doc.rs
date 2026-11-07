//! The 3D picture: a persistent prop list per map (tiles, terrain, a highlight per cell, the
//! units), edited in place each frame.

use super::rules::{Board, Side};
use super::{Anim, MoonlitTactics};
use crate::game_core::declare_models;
use crate::game_core::grid::Cell;
use crate::game_core::view::View;
use crate::hop_beat::{glb_prop, glb_ready, light, prop};
use crate::world_doc::{WorldCamera, WorldDoc};

const GRAVE_GLB: &str = "assets/models/kenney/graveyard-kit/gravestone-round.glb";
const CROSS_GLB: &str = "assets/models/kenney/graveyard-kit/gravestone-cross.glb";
const TREE_GLB: &str = "assets/models/tree.glb";
const COLUMN_GLB: &str = "assets/models/kenney/mini-arena/column.glb";
const LANTERN_GLB: &str = "assets/models/kenney/graveyard-kit/lantern-candle.glb";

/// Indices of the props a frame edits.
#[derive(Clone, Debug, Default)]
pub(super) struct Slots {
    /// The highlight over each cell (row by row).
    marks: usize,
    units: usize,
    /// The ring under the selected unit.
    cursor: usize,
}

/// The middle of a cell on the ground (world metres; +x right, +z toward the camera).
pub(super) fn cell_pos(b: &Board, c: Cell, tile: f32) -> [f32; 3] {
    let (w, h) = b.size();
    [
        (c.0 as f32 - (w - 1) as f32 * 0.5) * tile,
        0.0,
        (c.1 as f32 - (h - 1) as f32 * 0.5) * tile,
    ]
}

/// The cell under a ground point, if it is on the board.
pub(super) fn cell_at(b: &Board, x: f32, z: f32, tile: f32) -> Option<Cell> {
    let (w, h) = b.size();
    let cx = (x / tile + (w - 1) as f32 * 0.5).round() as i32;
    let cy = (z / tile + (h - 1) as f32 * 0.5).round() as i32;
    (cx >= 0 && cy >= 0 && cx < w && cy < h).then_some((cx, cy))
}

/// The camera: the whole board on screen, either way up, with room for the HUD above
/// (a bar) and below (a panel and buttons).
pub(super) fn board_view(b: &Board, tile: f32, aspect: f32) -> View {
    let (w, h) = b.size();
    let hx = w as f32 * tile * 0.5 + 0.2;
    let hz = h as f32 * tile * 0.5;
    let (top, bottom) = if aspect < 1.0 {
        (hz + 1.3 * tile, hz + 2.8 * tile)
    } else {
        (hz + 0.5 * tile, hz + 0.9 * tile)
    };
    let keep = View::ground_box(-hx, hx, -top, bottom, 0.0);
    View::frame(
        [0.0, 0.0, 0.25 * tile],
        [0.0, 1.9, 1.0],
        38.0,
        &keep,
        aspect,
    )
}

impl MoonlitTactics {
    /// Build the whole map once: ground, terrain, marks, units, camera and lights.
    pub(super) fn build_doc(&mut self) {
        let t = self.spec.tile;
        let (w, h) = self.board.size();
        let mut doc = WorldDoc {
            version: 1,
            genre: Some(super::GAME_ID.into()),
            half: (w.max(h) as f32) * t + 6.0,
            ground: Some(false),
            contact_blobs: Some(false),
            sky: Some([6, 8, 20, 255]),
            hemi_sky: Some([0.2, 0.24, 0.45]),
            hemi_ground: Some([0.1, 0.09, 0.15]),
            env_map: Some("preset:studio".into()),
            ibl: Some(0.25),
            exposure: Some(0.9),
            bloom: Some(0.45),
            bloom_threshold: Some(0.9),
            vignette: Some(0.5),
            fxaa: Some(true),
            tonemap: Some(true),
            ..WorldDoc::default()
        };
        let mut models: Vec<&str> = vec![GRAVE_GLB, CROSS_GLB, TREE_GLB, COLUMN_GLB, LANTERN_GLB];
        for c in &self.spec.classes {
            models.push(c.model.as_str());
        }
        declare_models(&mut doc, &models);
        let model = |id: &str, path: &str, name: &str, pos: [f32; 3], s: f32, col: [u32; 3]| {
            if glb_ready(path) {
                glb_prop(id, path, name, pos, [s; 3], [255, 255, 255], 0.0, None)
            } else {
                prop(
                    id,
                    "box",
                    name,
                    [pos[0], pos[1] + s * 0.25, pos[2]],
                    [s * 0.5; 3],
                    col,
                    0.0,
                    0.7,
                    0.0,
                )
            }
        };
        // The night ground around the board.
        doc.props.push(prop(
            "earth",
            "box",
            "earth",
            [0.0, -0.3, 0.0],
            [60.0, 0.2, 60.0],
            [22, 28, 30],
            0.0,
            1.0,
            0.0,
        ));
        // Tiles: a checker of dark turf; mist cells pale.
        for y in 0..h {
            for x in 0..w {
                let c = (x, y);
                let p = cell_pos(&self.board, c, t);
                let tile = self.board.map.tile(c);
                let col = match tile {
                    '~' => [70, 78, 98],
                    _ if (x + y) % 2 == 0 => [44, 58, 50],
                    _ => [38, 50, 44],
                };
                doc.props.push(prop(
                    &format!("tile-{x}-{y}"),
                    "box",
                    "tile",
                    [p[0], -0.1, p[2]],
                    [t * 0.97, 0.2, t * 0.97],
                    col,
                    0.0,
                    0.95,
                    0.0,
                ));
                let k = (x * 7 + y * 3) as f32;
                match tile {
                    '#' => {
                        let path = if (x + y) % 3 == 0 {
                            CROSS_GLB
                        } else {
                            GRAVE_GLB
                        };
                        let mut g = model(
                            &format!("grave-{x}-{y}"),
                            path,
                            "grave",
                            p,
                            1.7,
                            [110, 115, 130],
                        );
                        g.yaw = (k * 0.37).sin() * 0.3;
                        g.color = Some([150, 155, 175]);
                        doc.props.push(g);
                    }
                    'T' => {
                        let mut g = model(
                            &format!("tree-{x}-{y}"),
                            TREE_GLB,
                            "tree",
                            p,
                            1.7,
                            [40, 90, 60],
                        );
                        g.yaw = k;
                        g.color = Some([170, 210, 190]);
                        doc.props.push(g);
                    }
                    'C' => {
                        let mut g = model(
                            &format!("column-{x}-{y}"),
                            COLUMN_GLB,
                            "column",
                            p,
                            1.6,
                            [120, 120, 140],
                        );
                        g.color = Some([170, 170, 190]);
                        doc.props.push(g);
                    }
                    _ => {}
                }
            }
        }
        // Lanterns at the corners, for warm light in the cold.
        for (i, (sx, sz)) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)]
            .iter()
            .enumerate()
        {
            let pos = [
                sx * (w as f32 * 0.5 + 0.4) * t,
                0.0,
                sz * (h as f32 * 0.5 + 0.4) * t,
            ];
            let mut l = model(
                &format!("lantern-{i}"),
                LANTERN_GLB,
                "lantern",
                pos,
                1.8,
                [255, 200, 120],
            );
            l.emissive = Some(0.9);
            l.emissive_color = Some([1.0, 0.7, 0.3]);
            doc.props.push(l);
        }
        let mut slots = Slots {
            marks: doc.props.len(),
            ..Slots::default()
        };
        for y in 0..h {
            for x in 0..w {
                let p = cell_pos(&self.board, (x, y), t);
                let mut m = prop(
                    &format!("mark-{x}-{y}"),
                    "box",
                    "mark",
                    [p[0], 0.02, p[2]],
                    [t * 0.86, 0.04, t * 0.86],
                    [90, 160, 255],
                    1.0,
                    0.5,
                    0.0,
                );
                m.enabled = false;
                doc.props.push(m);
            }
        }
        slots.cursor = doc.props.len();
        let mut cur = prop(
            "cursor",
            "box",
            "cursor",
            [0.0, 0.05, 0.0],
            [t * 0.95, 0.05, t * 0.95],
            [255, 230, 140],
            1.6,
            0.5,
            0.0,
        );
        cur.enabled = false;
        doc.props.push(cur);
        slots.units = doc.props.len();
        for (i, u) in self.board.units.iter().enumerate() {
            let k = &self.spec.classes[u.class];
            let mut p = model(
                &format!("unit-{i}"),
                &k.model,
                "unit",
                cell_pos(&self.board, u.at, t),
                k.scale,
                if u.side == Side::Hero {
                    [255, 210, 150]
                } else {
                    [170, 180, 230]
                },
            );
            if u.side == Side::Foe {
                p.emissive = Some(0.15);
                p.emissive_color = Some([0.5, 0.55, 1.0]);
            }
            doc.props.push(p);
        }
        doc.cameras.push(WorldCamera {
            id: "camera:main".into(),
            kind: "camera".into(),
            name: "board".into(),
            position: [0.0, 10.0, 8.0],
            target: [0.0; 3],
            fov: 38.0,
            roll: 0.0,
        });
        doc.lights.push(light(
            0,
            "moon",
            [-4.0, 14.0, 6.0],
            [0.0; 3],
            [0.7, 0.78, 1.0],
            1.15,
            60.0,
            false,
        ));
        doc.lights.push(light(
            1,
            "sel",
            [0.0, 2.5, 0.0],
            [0.0; 3],
            [1.0, 0.8, 0.5],
            0.0,
            5.0,
            false,
        ));
        self.slots = slots;
        self.doc = doc;
        self.doc_map = Some(self.map_i);
    }

    /// Where unit `u` is drawn now: on its cell, or along its walk, or lunging in a strike.
    pub(super) fn unit_draw_pos(&self, u: usize) -> [f32; 3] {
        let t = self.spec.tile;
        let b = &self.board;
        let mut pos = cell_pos(b, b.units[u].at, t);
        if let Some(a) = self.anims.front() {
            match a {
                Anim::Walk { u: wu, path, t: at } if *wu == u && path.len() > 1 => {
                    let f = (at / self.spec.step_secs.max(1e-3)).min((path.len() - 1) as f32);
                    let i = (f.floor() as usize).min(path.len() - 2);
                    let k = f - i as f32;
                    let (p0, p1) = (cell_pos(b, path[i], t), cell_pos(b, path[i + 1], t));
                    pos = [
                        p0[0] + (p1[0] - p0[0]) * k,
                        0.08 * (k * std::f32::consts::PI).sin(),
                        p0[2] + (p1[2] - p0[2]) * k,
                    ];
                }
                Anim::Strike { a: au, d, t: at } if *au == u => {
                    let to = cell_pos(b, b.units[*d].at, t);
                    let k = (at / self.spec.strike_secs.max(1e-3) * std::f32::consts::PI).sin();
                    let lunge = 0.3 * k.max(0.0);
                    pos[0] += (to[0] - pos[0]) * lunge;
                    pos[2] += (to[2] - pos[2]) * lunge;
                }
                _ => {}
            }
        }
        // Units still to walk later in the queue are drawn where they start.
        for a in self.anims.iter().skip(1) {
            if let Anim::Walk { u: wu, path, .. } = a {
                if *wu == u {
                    if let Some(first) = path.first() {
                        pos = cell_pos(b, *first, t);
                    }
                    break;
                }
            }
        }
        pos
    }

    /// A frame: marks, units, the cursor, the camera.
    pub(super) fn update_doc(&mut self) {
        if self.doc_map != Some(self.map_i) {
            self.build_doc();
        }
        let (w, h) = self.board.size();
        let t = self.spec.tile;
        // Marks: blue where the selected hero can go, red its targets, green heal targets.
        let mut marks = vec![None; (w * h) as usize];
        if let Some(u) = self.sel {
            for (c, _) in &self.reach {
                marks[(c.1 * w + c.0) as usize] = Some([90, 150, 255]);
            }
            let at = self.board.units[u].at;
            let heal = self.board.class(u).heal > 0;
            for tgt in self.board.targets_from(u, at) {
                let c = self.board.units[tgt].at;
                marks[(c.1 * w + c.0) as usize] =
                    Some(if heal { [110, 255, 150] } else { [255, 90, 80] });
            }
        }
        let pulse = 0.8 + 0.3 * (self.clock * 4.0).sin();
        for (i, m) in marks.iter().enumerate() {
            let p = &mut self.doc.props[self.slots.marks + i];
            p.enabled = m.is_some();
            if let Some(col) = m {
                p.color = Some([col[0] as u32, col[1] as u32, col[2] as u32]);
                p.emissive = Some(pulse);
                p.emissive_color = Some([
                    col[0] as f32 / 255.0,
                    col[1] as f32 / 255.0,
                    col[2] as f32 / 255.0,
                ]);
            }
        }
        // Units.
        for u in 0..self.board.units.len() {
            let pos = self.unit_draw_pos(u);
            let alive = self.board.units[u].alive();
            let struck = matches!(self.anims.front(), Some(Anim::Strike { d, t: at, .. }) if *d == u && *at > self.spec.strike_secs * 0.4);
            // The fallen fade out while their strike plays.
            let dying =
                !alive && matches!(self.anims.front(), Some(Anim::Strike { d, .. }) if *d == u);
            let unit = &self.board.units[u];
            let k = &self.spec.classes[unit.class];
            let spent = unit.side == Side::Hero && unit.acted && self.mode == super::Mode::Player;
            let p = &mut self.doc.props[self.slots.units + u];
            p.enabled = alive || dying;
            p.position = pos;
            p.yaw = self.facing.get(u).copied().unwrap_or(0.0);
            let bob = if unit.side == Side::Foe {
                0.04 * (self.clock * 2.0 + u as f32).sin()
            } else {
                0.0
            };
            p.position[1] += bob;
            let shrink = if dying { 0.6 } else { 1.0 };
            p.scale = [k.scale * shrink; 3];
            p.color = Some(if spent {
                [120, 120, 130]
            } else {
                [255, 255, 255]
            });
            p.emissive = Some(if struck {
                2.5
            } else if unit.side == Side::Foe {
                0.15
            } else {
                0.0
            });
            p.emissive_color = Some(if struck {
                [1.0, 1.0, 1.0]
            } else {
                [0.5, 0.55, 1.0]
            });
        }
        // The cursor under the selected hero, and a warm light on it.
        let cur = self.sel.map(|u| self.unit_draw_pos(u));
        let c = &mut self.doc.props[self.slots.cursor];
        c.enabled = cur.is_some();
        if let Some(p) = cur {
            c.position = [p[0], 0.05, p[2]];
            c.emissive = Some(1.2 + 0.5 * (self.clock * 5.0).sin());
        }
        self.doc.lights[1].intensity = if cur.is_some() { 1.4 } else { 0.0 };
        if let Some(p) = cur {
            self.doc.lights[1].position = [p[0], 2.2, p[2] + 0.6];
        }
        let (sw, sh) = self.size.get();
        let view = board_view(&self.board, t, sw / sh.max(1.0));
        let sh_ = self.core.juice.shake;
        let cam = &mut self.doc.cameras[0];
        cam.position = [
            view.eye[0] + sh_ * (self.clock * 53.0).sin() * 0.3,
            view.eye[1],
            view.eye[2],
        ];
        cam.target = view.target;
        cam.fov = view.fov;
    }
}

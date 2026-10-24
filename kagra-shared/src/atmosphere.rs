//! Neon Cat's night atmosphere, drawn into the frame's `WorldDoc`.
//!
//! Three things, each switchable in `game.atmosphere` (see
//! [`crate::neon_fx_spec`]):
//!
//! * **Rain.** Thin streaks that fall through the tunnel and a small ripple
//!   where one lands. Nothing is stored: a streak's place is a pure function
//!   of its index, the cat's z and the page's clock, so a replay or a
//!   screenshot draws the same rain and the sim never sees it.
//! * **Neon signs.** Outline drawings (cat, fish, arrow, shop bars, glyphs) on
//!   the side walls, above head height, each in front of a dim glowing plate.
//!   They are placed by a hash of the cell index, so the same course always
//!   has the same signs. A share of them flicker.
//! * **Concrete walls.** A ring of textured wall boxes (one per
//!   `wall.segment` metres) just inside the plain black walls. The ring has a
//!   fixed number of slots and each slot keeps its prop id as it moves
//!   ahead, so the GPU mesh key does not change while the cat runs.
//!
//! The low tier (`quality: low`) draws no texture and no plates and thins the
//! rain and the signs. A missing texture file leaves the plain wall.

use crate::neon_fx_spec::{AtmosphereSpec, RainSpec, SignsSpec, Tier, WallSpec};
use crate::strokes::FatLine;
use crate::world_doc::{WorldDoc, WorldProp};

/// Inner wall height and half width the tunnel uses (`neon_cat::push_tunnel`).
const WALL_H: f32 = 6.9;
const WALL_X: f32 = 2.78;
const RAIN_HALF_W: f32 = 2.55;

/// Where and when the frame is drawn.
#[derive(Clone, Copy, Debug)]
pub struct View<'a> {
    pub spec: &'a AtmosphereSpec,
    pub tier: Tier,
    /// The wall texture this tier uses, already checked to exist. `None`
    /// keeps the plain wall.
    pub wall_texture: Option<&'a str>,
    /// Seconds. Only the rain and the flicker read it.
    pub time: f32,
    /// The cat's z.
    pub z: f32,
    /// The course seed: a different seed puts the signs elsewhere.
    pub seed: u32,
}

/// How far ahead of the cat signs are placed. The tunnel is never shorter.
const SIGN_REACH: f32 = 44.0;
/// Signs hang a little off the wall: a stroke a few cm in front of a far
/// plate loses to the depth buffer, so the strokes stand a clear hand's width
/// off the plate.
const SIGN_X: f32 = 2.42;
/// The backing plate sits on the concrete, a neon tube's stand-off behind
/// the strokes.
const PLATE_X: f32 = 2.7;

/// Rain and signs. Does nothing when the atmosphere is switched off.
pub fn push_effects(doc: &mut WorldDoc, view: &View) {
    let spec = view.spec;
    if !spec.enabled {
        return;
    }
    if spec.signs.enabled {
        push_signs(doc, &spec.signs, view);
    }
    if spec.rain.enabled {
        push_rain(doc, &spec.rain, view);
    }
}

/// The number of wall slots per side for `wall`. Fixed by the spec.
pub fn wall_slots(wall: &WallSpec) -> i32 {
    (((wall.reach + 6.0) / wall.segment).ceil() as i32) + 2
}

// ------------------------------------------------------------------- hashing

pub(crate) fn hash(a: i32, salt: u32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

pub(crate) fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

fn stroke(doc: &mut WorldDoc, id: String, points: Vec<[f32; 3]>, color: [u8; 4], width: f32) {
    doc.lines.push(FatLine {
        id,
        points,
        color,
        width,
    });
}

fn scale_alpha(a: u8, k: f32) -> u8 {
    (a as f32 * k).clamp(0.0, 255.0) as u8
}

// --------------------------------------------------------------------- walls

/// The side walls of the tunnel between `z0` and `z1`: the plain black box
/// on each side (as the tunnel always drew it) and, when the tier has a
/// texture, the ring of concrete panels just inside it. `None` leaves the
/// tunnel to draw its own plain walls.
pub fn wall_props(view: &View, z0: f32, z1: f32) -> Option<Vec<WorldProp>> {
    let spec = view.spec;
    let tex = view.wall_texture?;
    if !spec.enabled || !spec.wall.enabled || tex.is_empty() {
        return None;
    }
    let wall = &spec.wall;
    let mut out = Vec::new();
    let (zmid, depth) = ((z0 + z1) * 0.5, z1 - z0);
    for (name, sx) in [("wall-l", -1.0_f32), ("wall-r", 1.0)] {
        out.push(WorldProp {
            id: format!("prop:{name}"),
            kind: "prop".into(),
            name: "grid".into(),
            position: [sx * WALL_X, WALL_H * 0.5, zmid],
            model: "box".into(),
            scale: [0.05, WALL_H, depth],
            enabled: true,
            color: Some([4, 3, 10]),
            emissive: Some(0.0),
            roughness: 0.96,
            metallic: 0.0,
            collider: Some("none".into()),
            is_static: true,
            ..WorldProp::default()
        });
    }
    let slots = wall_slots(wall);
    let first = ((view.z - 6.0) / wall.segment).floor() as i32;
    let last = ((view.z + wall.reach) / wall.segment).floor() as i32;
    let repeat = (wall.segment / 8.0).round().max(1.0);
    for seg in first..=last.min(first + slots - 1) {
        let slot = seg.rem_euclid(slots);
        let zc = (seg as f32 + 0.5) * wall.segment;
        for (side, sx) in [("l", -1.0_f32), ("r", 1.0)] {
            out.push(WorldProp {
                id: format!("prop:wtex-{side}-{slot}"),
                kind: "prop".into(),
                name: "wall-concrete".into(),
                position: [sx * (WALL_X - 0.035), WALL_H * 0.5, zc],
                model: "box".into(),
                scale: [0.02, WALL_H, wall.segment * 0.999],
                enabled: true,
                color: Some([
                    wall.tint[0] as u32,
                    wall.tint[1] as u32,
                    wall.tint[2] as u32,
                ]),
                // Self-lit a little so the pattern reads in the dark; the
                // neon strokes and the tight lights do the rest.
                emissive: Some(0.34),
                emissive_color: Some([0.30, 0.26, 0.55]),
                emissive_map: Some(tex.to_string()),
                roughness: 0.96,
                metallic: 0.0,
                collider: Some("none".into()),
                is_static: true,
                texture: Some(tex.to_string()),
                uv_scale: Some([repeat, 1.0]),
                ..WorldProp::default()
            });
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------- rain

fn push_rain(doc: &mut WorldDoc, rain: &RainSpec, view: &View) {
    let n = rain.count(view.tier);
    if n == 0 {
        return;
    }
    let span = rain.reach + rain.behind;
    let origin = view.z - rain.behind;
    let fall_secs = WALL_H / rain.fall_speed;
    let ripple_secs = if rain.splash { 0.3 } else { 0.0 };
    let cycle = fall_secs + ripple_secs;
    for k in 0..n as i32 {
        let x = -RAIN_HALF_W + 2.0 * RAIN_HALF_W * unit(hash(k, 2));
        let z = origin + (unit(hash(k, 3)) * span - origin).rem_euclid(span);
        let phase = (view.time / cycle + unit(hash(k, 4))).fract() * cycle;
        if phase < fall_secs {
            let y = WALL_H - phase * rain.fall_speed;
            let y0 = y.max(0.03);
            let y1 = (y + rain.length).min(WALL_H);
            if y1 <= y0 {
                continue;
            }
            stroke(
                doc,
                format!("rn-{k}"),
                vec![[x, y0, z], [x - rain.slant * (y1 - y0), y1, z]],
                rain.color,
                rain.width,
            );
        } else if k % 2 == 0 {
            let age = (phase - fall_secs) / ripple_secs;
            let rx = 0.05 + 0.32 * age;
            let pts: Vec<[f32; 3]> = (0..=8)
                .map(|i| {
                    let a = i as f32 / 8.0 * std::f32::consts::TAU;
                    [x + rx * a.cos(), 0.05, z + rx * 0.5 * a.sin()]
                })
                .collect();
            let mut col = rain.color;
            col[3] = scale_alpha(col[3], (1.0 - age) * 0.8);
            stroke(doc, format!("rp-{k}"), pts, col, rain.width);
        }
    }
}

// --------------------------------------------------------------------- signs

type Pts = Vec<[f32; 2]>;

/// Glyphs in a box of u -1..1 (along the wall), v -0.6..0.6 (up). Each is a
/// set of polylines. Drawn by hand, nothing traced from anything.
fn glyph(pattern: u32) -> Vec<Pts> {
    match pattern % 8 {
        // Cat head: ears, cheeks, chin, eyes, whiskers.
        0 => vec![
            vec![
                [-0.34, -0.28],
                [-0.42, 0.0],
                [-0.38, 0.4],
                [-0.18, 0.2],
                [0.18, 0.2],
                [0.38, 0.4],
                [0.42, 0.0],
                [0.34, -0.28],
                [0.0, -0.4],
                [-0.34, -0.28],
            ],
            vec![[-0.22, 0.0], [-0.1, 0.0]],
            vec![[0.1, 0.0], [0.22, 0.0]],
            vec![[-0.42, -0.1], [-0.8, -0.04]],
            vec![[-0.4, -0.2], [-0.76, -0.24]],
            vec![[0.42, -0.1], [0.8, -0.04]],
            vec![[0.4, -0.2], [0.76, -0.24]],
        ],
        // Fish.
        1 => vec![
            vec![
                [-0.7, 0.0],
                [-0.35, 0.28],
                [0.1, 0.28],
                [0.45, 0.0],
                [0.1, -0.28],
                [-0.35, -0.28],
                [-0.7, 0.0],
            ],
            vec![[0.45, 0.0], [0.8, 0.3], [0.8, -0.3], [0.45, 0.0]],
            vec![[-0.45, 0.06], [-0.38, 0.06]],
        ],
        // Arrow (points along +u).
        2 => vec![vec![
            [-0.8, 0.1],
            [0.15, 0.1],
            [0.15, 0.34],
            [0.8, 0.0],
            [0.15, -0.34],
            [0.15, -0.1],
            [-0.8, -0.1],
            [-0.8, 0.1],
        ]],
        // Shop bars.
        3 => vec![
            vec![[-0.6, -0.34], [-0.6, 0.1]],
            vec![[-0.2, -0.34], [-0.2, 0.34]],
            vec![[0.2, -0.34], [0.2, 0.22]],
            vec![[0.6, -0.34], [0.6, 0.34]],
            vec![[-0.8, -0.4], [0.8, -0.4]],
        ],
        // A blocky glyph: a bar over a post over a base.
        4 => vec![
            vec![[-0.5, 0.3], [0.5, 0.3]],
            vec![[0.0, 0.3], [0.0, -0.3]],
            vec![[-0.7, -0.3], [0.7, -0.3]],
        ],
        // A box with a dot.
        5 => vec![
            vec![
                [-0.4, 0.34],
                [0.4, 0.34],
                [0.4, -0.34],
                [-0.4, -0.34],
                [-0.4, 0.34],
            ],
            vec![[-0.06, 0.0], [0.06, 0.0]],
        ],
        // A ring with a cross.
        6 => vec![
            (0..=12)
                .map(|i| {
                    let a = i as f32 / 12.0 * std::f32::consts::TAU;
                    [0.42 * a.cos() * 1.3, 0.36 * a.sin()]
                })
                .collect(),
            vec![[-0.3, 0.0], [0.3, 0.0]],
            vec![[0.0, -0.26], [0.0, 0.26]],
        ],
        // A fish bone.
        _ => vec![
            vec![[-0.8, 0.0], [0.5, 0.0]],
            vec![[0.5, 0.0], [0.8, 0.26], [0.8, -0.26], [0.5, 0.0]],
            vec![[-0.5, 0.0], [-0.3, 0.26]],
            vec![[-0.5, 0.0], [-0.3, -0.26]],
            vec![[-0.1, 0.0], [0.1, 0.26]],
            vec![[-0.1, 0.0], [0.1, -0.26]],
        ],
    }
}

/// Is the sign dimmed at `t`? Flickering signs drop out for a moment now
/// and then, with a short stutter before.
fn flicker_gain(t: f32, phase: f32) -> f32 {
    let c = (t * 0.7 + phase * 7.0).rem_euclid(3.0);
    if c > 2.84 {
        0.15
    } else if (2.5..2.56).contains(&c) {
        0.45
    } else {
        1.0
    }
}

fn push_signs(doc: &mut WorldDoc, s: &SignsSpec, view: &View) {
    let density = s.density(view.tier);
    if density <= 0.0 || s.palette.is_empty() || s.rows.is_empty() {
        return;
    }
    let i0 = ((view.z - 6.0) / s.cell).floor() as i32;
    let i1 = ((view.z + SIGN_REACH) / s.cell).ceil() as i32;
    for i in i0..i1 {
        let seed = view.seed.wrapping_mul(0x9E37_79B1);
        let hash = |a: i32, salt: u32| hash(a, salt ^ seed);
        if unit(hash(i, 10)) >= density {
            continue;
        }
        let side = if hash(i, 11) & 1 == 0 { 1.0_f32 } else { -1.0 };
        let pattern = hash(i, 12);
        let rgb = s.palette[hash(i, 13) as usize % s.palette.len()];
        let y = s.rows[hash(i, 14) as usize % s.rows.len()];
        let shaky = unit(hash(i, 15)) < s.flicker;
        let gain = if shaky {
            flicker_gain(view.time, unit(hash(i, 16)))
        } else {
            1.0
        };
        let zc = (i as f32 + 0.5) * s.cell;
        let half = s.size * 0.5;
        // Reading direction as seen from the tunnel: +u runs forward on the
        // +x wall and backward on the -x wall. An arrow always points ahead.
        let mut dir = side;
        if pattern % 8 == 2 {
            dir = 1.0;
        }
        let to_world = |u: f32, v: f32| [side * SIGN_X, y + v * half, zc + u * half * dir * side];
        let col = |a: f32| [rgb[0], rgb[1], rgb[2], scale_alpha(255, a * gain)];
        let frame: Pts = vec![
            [-1.0, -0.6],
            [1.0, -0.6],
            [1.0, 0.6],
            [-1.0, 0.6],
            [-1.0, -0.6],
        ];
        stroke(
            doc,
            format!("sg-{i}-f"),
            frame.iter().map(|p| to_world(p[0], p[1])).collect(),
            col(0.85),
            s.line_width,
        );
        for (n, g) in glyph(pattern).into_iter().enumerate() {
            stroke(
                doc,
                format!("sg-{i}-{n}"),
                g.iter().map(|p| to_world(p[0] * 0.9, p[1] * 0.9)).collect(),
                col(1.0),
                s.line_width,
            );
        }
        if view.tier != Tier::Low && s.glow > 0.0 {
            let k = |c: u8| (c as f32 * 0.16) as u32;
            doc.props.push(WorldProp {
                id: format!("prop:sgp-{i}"),
                kind: "prop".into(),
                name: "sign-plate".into(),
                position: [side * PLATE_X, y, zc],
                model: "box".into(),
                scale: [0.02, s.size * 0.6, s.size],
                enabled: true,
                color: Some([k(rgb[0]), k(rgb[1]), k(rgb[2])]),
                emissive: Some(s.glow * 0.2 * gain),
                emissive_color: Some([
                    rgb[0] as f32 / 255.0,
                    rgb[1] as f32 / 255.0,
                    rgb[2] as f32 / 255.0,
                ]),
                roughness: 0.96,
                metallic: 0.0,
                collider: Some("none".into()),
                is_static: true,
                ..WorldProp::default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEX: &str = "assets/textures/neon_cat/concrete_wall.png";

    fn view(spec: &AtmosphereSpec, z: f32, t: f32, tier: Tier) -> View<'_> {
        View {
            spec,
            tier,
            wall_texture: (tier != Tier::Low).then_some(TEX),
            time: t,
            z,
            seed: 7,
        }
    }

    fn effects(spec: &AtmosphereSpec, z: f32, t: f32, tier: Tier) -> WorldDoc {
        let mut doc = WorldDoc::default();
        push_effects(&mut doc, &view(spec, z, t, tier));
        doc
    }

    #[test]
    fn off_draws_nothing_and_tiers_thin_the_rain() {
        let off = AtmosphereSpec::off();
        let doc = effects(&off, 10.0, 0.4, Tier::High);
        assert!(doc.lines.is_empty() && doc.props.is_empty());
        assert!(wall_props(&view(&off, 10.0, 0.0, Tier::High), 4.0, 60.0).is_none());
        let spec = AtmosphereSpec::default();
        let count = |tier| {
            let doc = effects(&spec, 10.0, 0.4, tier);
            doc.lines.iter().filter(|l| l.id.starts_with("rn-")).count()
        };
        assert!(count(Tier::Low) < count(Tier::Mobile));
        assert!(count(Tier::Mobile) < count(Tier::High));
        assert!(count(Tier::High) > 40);
    }

    #[test]
    fn each_part_switches_off_alone() {
        let mut spec = AtmosphereSpec::default();
        spec.rain.enabled = false;
        let doc = effects(&spec, 50.0, 0.4, Tier::High);
        assert!(doc
            .lines
            .iter()
            .all(|l| !l.id.starts_with("rn-") && !l.id.starts_with("rp-")));
        assert!(doc.lines.iter().any(|l| l.id.starts_with("sg-")));
        spec.signs.enabled = false;
        let doc = effects(&spec, 50.0, 0.4, Tier::High);
        assert!(doc.lines.is_empty() && doc.props.is_empty());
        spec.wall.enabled = false;
        assert!(wall_props(&view(&spec, 10.0, 0.0, Tier::High), 4.0, 60.0).is_none());
    }

    #[test]
    fn rain_is_a_pure_function_of_z_and_the_clock() {
        let spec = AtmosphereSpec::default();
        let a = effects(&spec, 33.3, 1.7, Tier::High);
        let b = effects(&spec, 33.3, 1.7, Tier::High);
        assert_eq!(a.lines, b.lines);
        let later = effects(&spec, 33.3, 1.9, Tier::High);
        assert_ne!(a.lines, later.lines, "rain falls");
        for l in a.lines.iter().filter(|l| l.id.starts_with("rn-")) {
            assert!(l
                .points
                .iter()
                .all(|p| p[1] >= 0.0 && p[1] <= WALL_H + 1e-3));
            assert!(l.points.iter().all(|p| p[0].abs() <= RAIN_HALF_W + 0.2));
        }
    }

    #[test]
    fn rain_stays_inside_its_reach_window() {
        let spec = AtmosphereSpec::default();
        for z in [0.0_f32, 17.7, 300.0, 12345.6] {
            let doc = effects(&spec, z, 0.9, Tier::High);
            for l in doc.lines.iter().filter(|l| l.id.starts_with("rn-")) {
                let pz = l.points[0][2];
                assert!(
                    pz >= z - spec.rain.behind - 1e-2 && pz <= z + spec.rain.reach + 1e-2,
                    "z {z}: streak at {pz}"
                );
            }
        }
    }

    #[test]
    fn signs_are_placed_by_cell_and_sit_on_the_walls() {
        let spec = AtmosphereSpec::default();
        let signs = |d: &WorldDoc| {
            d.lines
                .iter()
                .filter(|l| l.id.starts_with("sg-") && l.id.ends_with("-f"))
                .map(|l| l.id.clone())
                .collect::<Vec<_>>()
        };
        let a = effects(&spec, 100.0, 0.0, Tier::High);
        let b = effects(&spec, 100.0, 5.0, Tier::High);
        assert_eq!(signs(&a), signs(&b), "the clock never moves a sign");
        assert!(signs(&a).len() >= 3, "a stretch of tunnel has signs");
        for l in a.lines.iter().filter(|l| l.id.starts_with("sg-")) {
            for p in &l.points {
                assert!((p[0].abs() - SIGN_X).abs() < 1e-4, "on the sign plane");
                assert!(
                    p[1] > 2.0 && p[1] < 6.5,
                    "above head height, under the roof: {}",
                    p[1]
                );
            }
        }
        // The seed moves them; the low tier keeps the strokes and drops the plates.
        let mut other = view(&spec, 100.0, 0.0, Tier::High);
        other.seed = 99;
        let mut doc = WorldDoc::default();
        push_effects(&mut doc, &other);
        assert_ne!(signs(&a), signs(&doc));
        let low = effects(&spec, 100.0, 0.0, Tier::Low);
        assert!(low.props.iter().all(|p| p.name != "sign-plate"));
        assert!(low.lines.iter().any(|l| l.id.starts_with("sg-")));
        assert!(a.props.iter().any(|p| p.name == "sign-plate"));
    }

    #[test]
    fn every_glyph_fits_the_frame() {
        for pattern in 0..8 {
            let g = glyph(pattern);
            assert!(!g.is_empty());
            for poly in g {
                assert!(poly.len() >= 2);
                for p in poly {
                    assert!(
                        p[0].abs() <= 0.9 + 1e-3 && p[1].abs() <= 0.45 + 1e-3,
                        "{pattern}: {p:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn flicker_dims_a_sign_only_briefly() {
        let dim = (0..3000)
            .filter(|i| flicker_gain(*i as f32 * 0.01, 0.3) < 0.5)
            .count();
        assert!(dim > 0 && dim < 300, "dim for {dim} of 3000 steps");
    }

    #[test]
    fn wall_ring_reuses_the_same_ids_while_running() {
        let spec = AtmosphereSpec::default();
        let slots = wall_slots(&spec.wall) as usize;
        let panels = |z: f32| {
            let v = view(&spec, z, 0.0, Tier::High);
            let props = wall_props(&v, z - 6.0, z + 50.0).expect("textured walls");
            let mut ids: Vec<String> = props
                .iter()
                .filter(|p| p.name == "wall-concrete")
                .map(|p| p.id.clone())
                .collect();
            ids.sort();
            // The two plain boxes are always there, so the far end stays black.
            assert_eq!(props.iter().filter(|p| p.name == "grid").count(), 2);
            ids
        };
        for z in [0.0_f32, 3.0, 7.9, 8.1, 55.5, 1000.0] {
            let got = panels(z);
            assert!(
                got.len() <= slots * 2 && got.len() >= (slots - 3) * 2,
                "{z}: {}",
                got.len()
            );
            let mut dedup = got.clone();
            dedup.dedup();
            assert_eq!(dedup.len(), got.len(), "no id used twice at z {z}");
        }
        // Every id that ever appears is one of the ring's fixed names, so the
        // GPU mesh key never changes while the cat runs.
        let all: std::collections::BTreeSet<String> =
            (0..400).flat_map(|i| panels(i as f32 * 1.3)).collect();
        assert!(
            all.len() <= slots * 2,
            "{} ids for {} slots",
            all.len(),
            slots
        );
        // No texture (the low tier, or a missing file) leaves the plain wall.
        let low = AtmosphereSpec::default();
        assert!(wall_props(&view(&low, 5.0, 0.0, Tier::Low), -1.0, 50.0).is_none());
    }

    #[test]
    fn panels_sit_inside_the_plain_wall() {
        let spec = AtmosphereSpec::default();
        let props = wall_props(&view(&spec, 20.0, 0.0, Tier::High), 14.0, 70.0).unwrap();
        for p in props.iter().filter(|p| p.name == "wall-concrete") {
            // Inner face of the panel is closer to the centre than the plain wall's.
            assert!(p.position[0].abs() + p.scale[0] * 0.5 <= WALL_X - 0.025 + 1e-4 + 0.03);
            assert!(p.position[0].abs() - p.scale[0] * 0.5 < WALL_X - 0.025);
            assert!(p.texture.is_some() && p.emissive_map == p.texture);
        }
    }
}

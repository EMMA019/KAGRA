//! Camera-facing points and screen-space fat lines on a world dump.
//!
//! three.js `Points` sizes a point with `gl_PointSize`. WebGPU's point-list
//! is one pixel and WebGL2 has no storage buffers, so each point is an
//! instanced camera-facing quad. Fat lines follow the three.js `Line2` idea:
//! one instanced quad per segment, expanded in clip space to a pixel width.
//! The WGSL is original (`render/strokes.wgsl`). No three.js source is copied.
//! No base instance. `Emitter` and `Trail` stay CPU props.

use serde::{Deserialize, Serialize};

/// Hard cap so a dump cannot blow the phone triangle budget.
pub const MAX_POINTS: usize = 2048;
pub const MAX_SEGMENTS: usize = 2048;

fn white_byte() -> [u8; 4] {
    [255, 255, 255, 255]
}

fn default_point_size() -> f32 {
    0.06
}

fn default_line_width() -> f32 {
    3.0
}

/// How a point cloud composites over the frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointBlend {
    /// Additive (`One`/`One`): glows, sparks, flames. Feeds the bloom emit
    /// channel. Draw order between additive sprites does not matter.
    #[default]
    Additive,
    /// Premultiplied alpha (`One` / `OneMinusSrcAlpha`): occluding sprites
    /// such as smoke. Kept out of the bloom emit channel (smoke never glows).
    /// Sprites composite in cloud order and are NOT depth-sorted within or
    /// between clouds, so two overlapping puffs blend in doc order rather
    /// than depth order — the accepted artifact of this path.
    Alpha,
}

fn is_additive(blend: &PointBlend) -> bool {
    *blend == PointBlend::Additive
}

/// One GPU point cloud. `size` is the sprite radius in meters.
///
/// A cloud may bind a texture atlas ([`PointCloud::atlas`]); each point then
/// samples its own frame rect with its own tint, radius and rotation. Without
/// an atlas the points stay the historical shader-drawn soft discs, and every
/// per-point attribute falls back to the cloud's `color` / `size`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointCloud {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positions: Vec<[f32; 3]>,
    /// sRGB author color. Alpha is the sprite opacity.
    #[serde(default = "white_byte")]
    pub color: [u8; 4],
    /// Billboard radius in world meters. 0 uses [`default_point_size`].
    #[serde(default = "default_point_size")]
    pub size: f32,
    /// Texture atlas spec: a PNG path or `image2d:<id>`, resolved exactly
    /// like a prop `texture`. Declaring the atlas on a cloud — even an empty
    /// one with no positions — is what uploads it, so a kit declares its
    /// atlases in the doc from the first frame and never at the first spawn
    /// (the first-occurrence rule). An atlas that fails to resolve falls back
    /// to the soft disc instead of a white square.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atlas: Option<String>,
    /// Per-point UV rect in atlas space, `(u0, v0, u1, v1)` with `v0` at the
    /// TOP row of the sheet. Parallel to `positions`; a missing entry samples
    /// the whole atlas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<[f32; 4]>,
    /// Per-point sRGB tint, parallel to `positions`; a missing entry uses the
    /// cloud `color`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tints: Vec<[u8; 4]>,
    /// Per-point radius in meters, parallel to `positions`; a missing or
    /// non-positive entry uses the cloud `size`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sizes: Vec<f32>,
    /// Per-point rotation in radians (counter-clockwise on screen), parallel
    /// to `positions`; a missing entry is unrotated.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rotations: Vec<f32>,
    /// Compositing for this cloud. Default [`PointBlend::Additive`].
    #[serde(default, skip_serializing_if = "is_additive")]
    pub blend: PointBlend,
}

impl Default for PointCloud {
    fn default() -> Self {
        Self {
            id: String::new(),
            positions: Vec::new(),
            color: white_byte(),
            size: default_point_size(),
            atlas: None,
            frames: Vec::new(),
            tints: Vec::new(),
            sizes: Vec::new(),
            rotations: Vec::new(),
            blend: PointBlend::Additive,
        }
    }
}

/// One screen-space polyline. `width` is pixels, not meters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FatLine {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f32; 3]>,
    /// sRGB author color. Alpha is the stroke opacity.
    #[serde(default = "white_byte")]
    pub color: [u8; 4],
    /// Width in framebuffer pixels. 0 uses 3.
    #[serde(default = "default_line_width")]
    pub width: f32,
}

impl Default for FatLine {
    fn default() -> Self {
        Self {
            id: String::new(),
            points: Vec::new(),
            color: white_byte(),
            width: default_line_width(),
        }
    }
}

/// One point after the cap and the finite-position filter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointSprite {
    pub position: [f32; 3],
    pub size: f32,
    pub color: [u8; 4],
    /// Atlas UV rect `(u0, v0, u1, v1)`, `v0` at the top of the sheet. The
    /// whole sheet when the point names no frame.
    pub uv: [f32; 4],
    /// On-screen rotation in radians, counter-clockwise.
    pub rotation: f32,
    /// Sample the cloud's atlas. False draws the soft disc.
    pub textured: bool,
}

/// A run of sprites that share one atlas and blend mode — one instanced draw.
/// Batches preserve cloud order; only CONSECUTIVE clouds with the same atlas
/// and blend merge, so an alpha cloud authored between two additive ones
/// still composites in doc order.
#[derive(Clone, Debug, PartialEq)]
pub struct PointBatch {
    pub atlas: Option<String>,
    pub blend: PointBlend,
    pub sprites: Vec<PointSprite>,
}

/// One segment. Zero-length pairs are dropped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub width: f32,
    pub color: [u8; 4],
}

fn finite3(p: [f32; 3]) -> bool {
    p.iter().all(|c| c.is_finite())
}

/// Expand one cloud into `out`, honouring the shared `total` cap. Returns
/// true when the cap stopped the expansion early.
fn expand_cloud(cloud: &PointCloud, out: &mut Vec<PointSprite>, total: &mut usize) -> bool {
    let base_size = if cloud.size > 0.0 {
        cloud.size
    } else {
        default_point_size()
    };
    let base_size = base_size.clamp(0.002, 4.0);
    let textured = cloud.atlas.is_some();
    for (i, p) in cloud.positions.iter().enumerate() {
        if *total >= MAX_POINTS {
            return true;
        }
        if !finite3(*p) {
            continue;
        }
        let size = cloud
            .sizes
            .get(i)
            .copied()
            .filter(|s| s.is_finite() && *s > 0.0)
            .map_or(base_size, |s| s.clamp(0.002, 4.0));
        let uv = cloud
            .frames
            .get(i)
            .copied()
            .filter(|r| r.iter().all(|c| c.is_finite()))
            .unwrap_or([0.0, 0.0, 1.0, 1.0]);
        let rotation = cloud
            .rotations
            .get(i)
            .copied()
            .filter(|r| r.is_finite())
            .unwrap_or(0.0);
        out.push(PointSprite {
            position: *p,
            size,
            color: cloud.tints.get(i).copied().unwrap_or(cloud.color),
            uv,
            rotation,
            textured,
        });
        *total += 1;
    }
    false
}

/// Flatten clouds. Stops at [`MAX_POINTS`].
pub fn collect_points(clouds: &[PointCloud]) -> Vec<PointSprite> {
    let mut out = Vec::new();
    let mut total = 0;
    for cloud in clouds {
        if expand_cloud(cloud, &mut out, &mut total) {
            break;
        }
    }
    out
}

/// Flatten clouds into per-atlas, per-blend batches for the renderer. The
/// [`MAX_POINTS`] cap is shared across all batches.
pub fn collect_point_batches(clouds: &[PointCloud]) -> Vec<PointBatch> {
    let mut batches: Vec<PointBatch> = Vec::new();
    let mut total = 0;
    for cloud in clouds {
        let merge = batches
            .last()
            .is_some_and(|b| b.atlas == cloud.atlas && b.blend == cloud.blend);
        if !merge {
            batches.push(PointBatch {
                atlas: cloud.atlas.clone(),
                blend: cloud.blend,
                sprites: Vec::new(),
            });
        }
        let batch = batches.last_mut().expect("batch just pushed");
        if expand_cloud(cloud, &mut batch.sprites, &mut total) {
            break;
        }
    }
    batches.retain(|b| !b.sprites.is_empty() || b.atlas.is_some());
    batches
}

/// Walk each polyline. Stops at [`MAX_SEGMENTS`].
pub fn collect_segments(lines: &[FatLine]) -> Vec<Segment> {
    let mut out = Vec::new();
    for line in lines {
        let width = if line.width > 0.0 {
            line.width
        } else {
            default_line_width()
        };
        let width = width.clamp(0.5, 32.0);
        for pair in line.points.windows(2) {
            if out.len() >= MAX_SEGMENTS {
                return out;
            }
            let a = pair[0];
            let b = pair[1];
            if !finite3(a) || !finite3(b) {
                continue;
            }
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            let dz = b[2] - a[2];
            if dx * dx + dy * dy + dz * dz < 1e-8 {
                continue;
            }
            out.push(Segment {
                start: a,
                end: b,
                width,
                color: line.color,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_drop_duplicates_and_honor_the_cap() {
        let mut points = vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        points.extend((0..MAX_SEGMENTS + 4).map(|i| [i as f32, 0.0, 0.0]));
        let line = FatLine {
            id: "bar".into(),
            points,
            color: [1, 2, 3, 4],
            width: 0.0,
        };
        let segs = collect_segments(&[line]);
        assert_eq!(segs.len(), MAX_SEGMENTS);
        assert!((segs[0].width - 3.0).abs() < 1e-4);
        assert_eq!(segs[0].start, [0.0, 0.0, 0.0]);
        assert_eq!(segs[0].end, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn points_skip_non_finite_and_roundtrip_json() {
        let cloud = PointCloud {
            id: "dots".into(),
            positions: vec![[0.0, 1.0, 2.0], [f32::NAN, 0.0, 0.0]],
            color: [9, 8, 7, 6],
            size: 0.2,
            ..PointCloud::default()
        };
        let sprites = collect_points(&[cloud]);
        assert_eq!(sprites.len(), 1);
        assert!((sprites[0].size - 0.2).abs() < 1e-5);
        let finite = PointCloud {
            id: "dots".into(),
            positions: vec![[0.0, 1.0, 2.0]],
            color: [9, 8, 7, 6],
            size: 0.2,
            ..PointCloud::default()
        };
        let json = serde_json::to_string(&finite).unwrap();
        let back: PointCloud = serde_json::from_str(&json).unwrap();
        assert_eq!(back, finite);
    }

    #[test]
    fn per_point_attributes_fall_back_to_the_cloud() {
        let cloud = PointCloud {
            id: "fx".into(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
            color: [10, 20, 30, 40],
            size: 0.3,
            atlas: Some("image2d:7".into()),
            frames: vec![[0.0, 0.0, 0.5, 0.5]],
            tints: vec![[1, 2, 3, 4]],
            sizes: vec![0.9],
            rotations: vec![1.5],
            ..PointCloud::default()
        };
        let sprites = collect_points(&[cloud]);
        assert_eq!(sprites.len(), 3);
        // Point 0 takes every per-point attribute.
        assert!(sprites[0].textured);
        assert_eq!(sprites[0].uv, [0.0, 0.0, 0.5, 0.5]);
        assert_eq!(sprites[0].color, [1, 2, 3, 4]);
        assert!((sprites[0].size - 0.9).abs() < 1e-6);
        assert!((sprites[0].rotation - 1.5).abs() < 1e-6);
        // Points 1 and 2 fall back to the cloud's color/size, the whole
        // atlas frame and no rotation.
        for s in &sprites[1..] {
            assert_eq!(s.color, [10, 20, 30, 40]);
            assert!((s.size - 0.3).abs() < 1e-6);
            assert_eq!(s.uv, [0.0, 0.0, 1.0, 1.0]);
            assert_eq!(s.rotation, 0.0);
        }
    }

    #[test]
    fn batches_group_consecutive_clouds_and_keep_declarations() {
        let disc = |id: &str, n: usize| PointCloud {
            id: id.into(),
            positions: vec![[0.0, 0.0, 0.0]; n],
            ..PointCloud::default()
        };
        let mut smoke = disc("smoke", 2);
        smoke.atlas = Some("atlas.png".into());
        smoke.blend = PointBlend::Alpha;
        let mut declare = disc("declare", 0);
        declare.atlas = Some("atlas.png".into());
        declare.blend = PointBlend::Alpha;
        let batches =
            collect_point_batches(&[disc("a", 1), disc("b", 2), smoke, disc("c", 1), declare]);
        // a+b merge (same key), smoke is its own alpha batch, c starts a new
        // additive batch (not merged across the smoke), and the empty atlas
        // declaration survives as an empty batch so the renderer uploads it.
        assert_eq!(batches.len(), 4);
        assert_eq!(batches[0].sprites.len(), 3);
        assert_eq!(batches[0].blend, PointBlend::Additive);
        assert_eq!(batches[1].sprites.len(), 2);
        assert_eq!(batches[1].blend, PointBlend::Alpha);
        assert_eq!(batches[2].sprites.len(), 1);
        assert!(batches[3].sprites.is_empty());
        assert_eq!(batches[3].atlas.as_deref(), Some("atlas.png"));
    }

    #[test]
    fn cloud_json_stays_backward_compatible() {
        // A pre-atlas dump (only the original fields) still parses.
        let old = r#"{"id":"dots","positions":[[0.0,1.0,2.0]],"color":[9,8,7,6],"size":0.2}"#;
        let cloud: PointCloud = serde_json::from_str(old).unwrap();
        assert_eq!(cloud.atlas, None);
        assert_eq!(cloud.blend, PointBlend::Additive);
        // A default cloud serializes exactly like before (no new keys), so
        // existing docs and fixtures do not churn.
        let plain = PointCloud {
            id: "dots".into(),
            positions: vec![[0.0, 1.0, 2.0]],
            color: [9, 8, 7, 6],
            size: 0.2,
            ..PointCloud::default()
        };
        assert_eq!(serde_json::to_string(&plain).unwrap(), old);
        // The new fields round-trip when set.
        let mut rich = plain.clone();
        rich.atlas = Some("image2d:3".into());
        rich.frames = vec![[0.0, 0.0, 0.5, 1.0]];
        rich.blend = PointBlend::Alpha;
        let back: PointCloud =
            serde_json::from_str(&serde_json::to_string(&rich).unwrap()).unwrap();
        assert_eq!(back, rich);
    }

    #[test]
    fn dump_roundtrip_reaches_the_draw_list() {
        let mut doc = crate::world_doc::WorldDoc {
            version: 1,
            ..crate::world_doc::WorldDoc::default()
        };
        let quiet = doc.to_json().unwrap();
        assert!(!quiet.contains("\"points\""));
        assert!(!quiet.contains("\"lines\""));
        doc.points.push(PointCloud {
            id: "dots".into(),
            positions: vec![[0.0, 1.0, 0.0]],
            ..PointCloud::default()
        });
        doc.lines.push(FatLine {
            id: "bar".into(),
            points: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            width: 5.0,
            ..FatLine::default()
        });
        let back = crate::world_doc::WorldDoc::from_json(&doc.to_json().unwrap()).unwrap();
        assert_eq!(back.points[0].id, "dots");
        assert_eq!(back.lines[0].points.len(), 2);
        let scene = back.compile_scene(1.0);
        assert_eq!(scene.points.len(), 1);
        assert_eq!(scene.lines[0].id, "bar");
    }
}

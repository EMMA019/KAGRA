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

/// One GPU point cloud. `size` is the sprite radius in meters.
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
}

impl Default for PointCloud {
    fn default() -> Self {
        Self {
            id: String::new(),
            positions: Vec::new(),
            color: white_byte(),
            size: default_point_size(),
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

/// Flatten clouds. Stops at [`MAX_POINTS`].
pub fn collect_points(clouds: &[PointCloud]) -> Vec<PointSprite> {
    let mut out = Vec::new();
    for cloud in clouds {
        let size = if cloud.size > 0.0 {
            cloud.size
        } else {
            default_point_size()
        };
        let size = size.clamp(0.002, 4.0);
        for p in &cloud.positions {
            if out.len() >= MAX_POINTS {
                return out;
            }
            if !finite3(*p) {
                continue;
            }
            out.push(PointSprite {
                position: *p,
                size,
                color: cloud.color,
            });
        }
    }
    out
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
        };
        let sprites = collect_points(&[cloud]);
        assert_eq!(sprites.len(), 1);
        assert!((sprites[0].size - 0.2).abs() < 1e-5);
        let finite = PointCloud {
            id: "dots".into(),
            positions: vec![[0.0, 1.0, 2.0]],
            color: [9, 8, 7, 6],
            size: 0.2,
        };
        let json = serde_json::to_string(&finite).unwrap();
        let back: PointCloud = serde_json::from_str(&json).unwrap();
        assert_eq!(back, finite);
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

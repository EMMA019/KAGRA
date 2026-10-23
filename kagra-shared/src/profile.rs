//! One-frame cost of a world dump. No GPU, no editor.
//!
//! Headless dumps splice this next to `coordinates`. Omitted from `to_json`
//! so an old dump stays byte-stable. Phone budgets are the warning lines.

use std::path::{Path, PathBuf};

use crate::scene3d::AlbedoRgba;
use crate::world_doc::WorldDoc;

/// 60 Hz frame. A phone dump that crosses these prints a warning.
pub const BUDGET_CPU_MS: f32 = 16.7;
pub const BUDGET_GPU_MS: f32 = 16.7;
pub const BUDGET_DRAWS: u32 = 200;
pub const BUDGET_TRIANGLES: u32 = 100_000;
/// Shadow map plus authored images. Not the HDR framebuffer.
pub const BUDGET_TEXTURE_BYTES: u64 = 48 * 1024 * 1024;

const SHADOW_MAP_BYTES: u64 = 2048 * 2048 * 4;
const ENV_STRIP_BYTES: u64 = 64 * 64 * 4;

/// Counts for one compiled frame. `cpu_ms` / `gpu_ms` stay empty until a
/// renderer measured that frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameProfile {
    pub cpu_ms: Option<f32>,
    pub gpu_ms: Option<f32>,
    pub draws: u32,
    pub triangles: u32,
    pub texture_bytes: u64,
}

impl FrameProfile {
    /// Draws, triangles, and texture bytes from `compile_scene` / `compile_meshes`.
    /// CPU and GPU times stay empty. The shadow map and the env strip are
    /// counted because the renderer always allocates them.
    pub fn from_world(doc: &WorldDoc) -> Self {
        let meshes = doc.compile_meshes();
        let scene = doc.compile_scene(16.0 / 9.0);
        let mut draws = 0u32;
        let mut triangles = 0u32;
        let mut texture_bytes = SHADOW_MAP_BYTES + ENV_STRIP_BYTES;
        for batch in &scene.batches {
            if batch.instances.is_empty() {
                continue;
            }
            let Some((_, mesh)) = meshes.iter().find(|(id, _)| *id == batch.mesh) else {
                continue;
            };
            draws += 1;
            let tris = (mesh.indices.len() as u32 / 3).saturating_mul(batch.instances.len() as u32);
            triangles = triangles.saturating_add(tris);
            texture_bytes = texture_bytes.saturating_add(image_bytes(&mesh.albedo));
            texture_bytes = texture_bytes.saturating_add(image_bytes(&mesh.matcap));
            texture_bytes = texture_bytes.saturating_add(image_bytes(&mesh.normal));
            texture_bytes = texture_bytes.saturating_add(image_bytes(&mesh.orm));
            texture_bytes = texture_bytes.saturating_add(image_bytes(&mesh.emissive_map));
        }
        Self {
            cpu_ms: None,
            gpu_ms: None,
            draws,
            triangles,
            texture_bytes,
        }
    }

    pub fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(ms) = self.cpu_ms {
            if ms > BUDGET_CPU_MS {
                out.push(format!("cpu {ms:.1} ms over budget {BUDGET_CPU_MS} ms"));
            }
        }
        if let Some(ms) = self.gpu_ms {
            if ms > BUDGET_GPU_MS {
                out.push(format!("gpu {ms:.1} ms over budget {BUDGET_GPU_MS} ms"));
            }
        }
        if self.draws > BUDGET_DRAWS {
            out.push(format!("draws {} over budget {BUDGET_DRAWS}", self.draws));
        }
        if self.triangles > BUDGET_TRIANGLES {
            out.push(format!(
                "triangles {} over budget {BUDGET_TRIANGLES}",
                self.triangles
            ));
        }
        if self.texture_bytes > BUDGET_TEXTURE_BYTES {
            out.push(format!(
                "texture {} over budget {BUDGET_TEXTURE_BYTES}",
                self.texture_bytes
            ));
        }
        out
    }

    pub fn to_json(&self) -> String {
        let warnings = self.warnings();
        serde_json::json!({
            "cpu_ms": self.cpu_ms,
            "gpu_ms": self.gpu_ms,
            "draws": self.draws,
            "triangles": self.triangles,
            "texture_bytes": self.texture_bytes,
            "warnings": warnings,
        })
        .to_string()
    }
}

/// Insert `"profile"` before the closing brace of a world dump.
/// `to_json` itself does not call this.
pub fn splice_profile(dump: &str, profile: &FrameProfile) -> String {
    let trimmed = dump.trim_end();
    let Some(body) = trimmed.strip_suffix('}') else {
        return dump.to_string();
    };
    format!("{body},\n  \"profile\": {}\n}}", profile.to_json())
}

/// Dump file plus authored images and glTF the desktop watcher should follow.
/// `preset:` names are not files.
pub fn watch_paths(dump: &Path, doc: &WorldDoc) -> Vec<PathBuf> {
    let mut out = Vec::new();
    out.push(dump.to_path_buf());
    let mut push = |spec: &Option<String>| {
        if let Some(path) = file_spec(spec) {
            let p = PathBuf::from(path);
            if !out.iter().any(|e| e == &p) {
                out.push(p);
            }
        }
    };
    push(&doc.env_map);
    if let Some(player) = &doc.player {
        push(&player.gltf);
    }
    for prop in &doc.props {
        push(&prop.texture);
        push(&prop.roughness_map);
        push(&prop.metalness_map);
        push(&prop.emissive_map);
        push(&prop.gltf);
    }
    out
}

fn file_spec(spec: &Option<String>) -> Option<&str> {
    let s = spec.as_deref()?.trim();
    if s.is_empty() || s.starts_with("preset:") {
        None
    } else {
        Some(s)
    }
}

fn image_bytes(img: &Option<AlbedoRgba>) -> u64 {
    img.as_ref().map(|a| a.rgba.len() as u64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_box_stays_inside_the_phone_budget() {
        let doc = WorldDoc::from_json(
            r#"{"version":1,"ground":false,"props":[{"id":"b","type":"prop","model":"box","position":[0.0,1.0,0.0]}]}"#,
        )
        .unwrap();
        let profile = FrameProfile::from_world(&doc);
        assert!(profile.draws >= 1, "draws {}", profile.draws);
        assert!(profile.triangles >= 12, "tris {}", profile.triangles);
        assert!(profile.triangles < BUDGET_TRIANGLES);
        assert!(profile.texture_bytes > SHADOW_MAP_BYTES);
        assert!(profile.warnings().is_empty(), "{:?}", profile.warnings());
        assert!(profile.cpu_ms.is_none());
        let dump = doc.to_json().unwrap();
        assert!(!dump.contains("\"profile\""));
        let spliced = splice_profile(&dump, &profile);
        assert!(spliced.contains("\"profile\""));
        assert!(spliced.contains("\"warnings\":[]"));
        let parsed: serde_json::Value = serde_json::from_str(&spliced).unwrap();
        assert!(parsed.get("coordinates").is_some());
        assert_eq!(parsed["profile"]["draws"], profile.draws);
    }

    #[test]
    fn over_budget_names_the_counter() {
        let profile = FrameProfile {
            cpu_ms: Some(40.0),
            gpu_ms: Some(30.0),
            draws: 400,
            triangles: 250_000,
            texture_bytes: BUDGET_TEXTURE_BYTES + 1,
        };
        let warnings = profile.warnings();
        assert!(warnings.iter().any(|w| w.starts_with("cpu ")));
        assert!(warnings.iter().any(|w| w.starts_with("gpu ")));
        assert!(warnings.iter().any(|w| w.starts_with("draws ")));
        assert!(warnings.iter().any(|w| w.starts_with("triangles ")));
        assert!(warnings.iter().any(|w| w.starts_with("texture ")));
    }

    #[test]
    fn watch_list_skips_presets_and_keeps_files() {
        let doc = WorldDoc::from_json(
            r#"{"version":1,"env_map":"preset:studio","props":[{"id":"a","type":"prop","model":"box","texture":"art/floor.png","roughness_map":"preset:split","emissive_map":"art/glow.png"},{"id":"b","type":"prop","model":"box","texture":"art/floor.png"}]}"#,
        )
        .unwrap();
        let paths = watch_paths(Path::new("worlds/room.json"), &doc);
        let text: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        assert!(text.iter().any(|p| p.ends_with("room.json")));
        assert!(text.iter().any(|p| p.ends_with("art/floor.png")));
        assert!(text.iter().any(|p| p.ends_with("art/glow.png")));
        assert_eq!(text.iter().filter(|p| p.ends_with("floor.png")).count(), 1);
        assert!(!text.iter().any(|p| p.contains("preset")));
    }
}

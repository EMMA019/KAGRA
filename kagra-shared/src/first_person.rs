//! VRM firstPerson mesh annotations. Parsed and kept; applying them (hide the
//! head / mouth in a first-person view) needs a first-person camera, which is
//! a later slice. Port of kagra-core vrm_first_person.rs.

use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MeshAnnotation {
    #[default]
    Auto,
    Both,
    FirstPersonOnly,
    ThirdPersonOnly,
}

impl MeshAnnotation {
    fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "both" => Self::Both,
            "firstpersononly" | "first_person_only" => Self::FirstPersonOnly,
            "thirdpersononly" | "third_person_only" => Self::ThirdPersonOnly,
            _ => Self::Auto,
        }
    }
}

/// Parsed VRM 0.x / VRM 1.0 firstPerson annotations.
#[derive(Clone, Debug, Default)]
pub struct FirstPerson {
    pub by_mesh: HashMap<usize, MeshAnnotation>,
    pub by_node: HashMap<usize, MeshAnnotation>,
}

impl FirstPerson {
    pub fn is_empty(&self) -> bool {
        self.by_mesh.is_empty() && self.by_node.is_empty()
    }
}

/// mesh index → annotation, node index → annotation.
pub fn parse_mesh_annotations(extensions: Option<&Value>) -> FirstPerson {
    let mut fp = FirstPerson::default();
    let Some(ext) = extensions else {
        return fp;
    };

    // VRM 1.0
    if let Some(arr) = ext
        .pointer("/VRMC_vrm/firstPerson/meshAnnotations")
        .and_then(|v| v.as_array())
    {
        for a in arr {
            let typ = a.get("type").and_then(|t| t.as_str()).unwrap_or("auto");
            let flag = MeshAnnotation::from_str(typ);
            if let Some(n) = a.get("node").and_then(|x| x.as_u64()) {
                fp.by_node.insert(n as usize, flag);
            }
            if let Some(m) = a.get("mesh").and_then(|x| x.as_u64()) {
                fp.by_mesh.insert(m as usize, flag);
            }
        }
    }

    // VRM 0.x
    if let Some(arr) = ext
        .pointer("/VRM/firstPerson/meshAnnotations")
        .and_then(|v| v.as_array())
    {
        for a in arr {
            let typ = a
                .get("firstPersonFlag")
                .or_else(|| a.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("Auto");
            let flag = MeshAnnotation::from_str(typ);
            if let Some(m) = a.get("mesh").and_then(|x| x.as_u64()) {
                fp.by_mesh.entry(m as usize).or_insert(flag);
            }
            if let Some(n) = a.get("node").and_then(|x| x.as_u64()) {
                fp.by_node.entry(n as usize).or_insert(flag);
            }
        }
    }
    fp
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_v0_and_v1_annotations() {
        let ext = json!({
            "VRM": {
                "firstPerson": {
                    "meshAnnotations": [
                        {"mesh": 0, "firstPersonFlag": "FirstPersonOnly"},
                        {"mesh": 1, "firstPersonFlag": "ThirdPersonOnly"}
                    ]
                }
            },
            "VRMC_vrm": {
                "firstPerson": {
                    "meshAnnotations": [
                        {"node": 2, "type": "FirstPersonOnly"}
                    ]
                }
            }
        });
        let fp = parse_mesh_annotations(Some(&ext));
        assert_eq!(fp.by_mesh.get(&0), Some(&MeshAnnotation::FirstPersonOnly));
        assert_eq!(fp.by_mesh.get(&1), Some(&MeshAnnotation::ThirdPersonOnly));
        assert_eq!(fp.by_node.get(&2), Some(&MeshAnnotation::FirstPersonOnly));
        assert!(!fp.is_empty());
    }

    #[test]
    fn empty_when_no_annotations() {
        let fp = parse_mesh_annotations(None);
        assert!(fp.is_empty());
    }
}

//! アセット別名（Python `kagra.contracts` と揃える）。

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AssetKind {
    Vrm = 0,
    Fbx = 1,
    Bvh = 2,
    Texture = 3,
    Font = 4,
    Audio = 5,
    Any = 255,
}

impl AssetKind {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Vrm,
            1 => Self::Fbx,
            2 => Self::Bvh,
            3 => Self::Texture,
            4 => Self::Font,
            5 => Self::Audio,
            _ => Self::Any,
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Vrm => &[".vrm"],
            Self::Fbx => &[".fbx"],
            Self::Bvh => &[".bvh"],
            Self::Texture => &[".png", ".jpg", ".jpeg", ".webp"],
            Self::Font => &[".ttf", ".ttc", ".otf"],
            Self::Audio => &[".wav", ".ogg", ".mp3"],
            Self::Any => &[],
        }
    }
}

/// 論理名 → 相対パス候補（存在確認はホスト側）。
pub fn resolve_alias(name: &str) -> Vec<&'static str> {
    match name.trim().to_ascii_lowercase().as_str() {
        "emma" => vec![
            "assets/Emma.vrm",
            "assets/model/Emma.vrm",
            "assets/model/player.vrm",
        ],
        "player" => vec![
            "assets/model/player.vrm",
            "assets/Emma.vrm",
            "assets/player.vrm",
        ],
        "walk" => vec![
            "tests/fixtures/synthetic_walk.bvh",
            "assets/walk.fbx",
            "assets/anim/walk.fbx",
        ],
        "walk_fbx" => vec!["assets/walk.fbx", "assets/anim/walk.fbx"],
        "dance" => vec![
            "tests/fixtures/synthetic_dance.bvh",
            "assets/dance.bvh",
            "assets/anim/dance.bvh",
        ],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emma_alias() {
        let a = resolve_alias("Emma");
        assert!(a.iter().any(|p| p.ends_with("Emma.vrm")));
    }
}

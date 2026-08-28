//! アセット別名（Python `kagra.contracts` と揃える）。

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AssetKind {
    Vrm = 0,
    Fbx = 1,
    Bvh = 2,
    Texture = 3,
    Font = 4,
    Audio = 5,
    Vrma = 6,
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
            6 => Self::Vrma,
            _ => Self::Any,
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Vrm => &[".vrm"],
            Self::Fbx => &[".fbx"],
            Self::Bvh => &[".bvh"],
            Self::Vrma => &[".vrma"],
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
            "assets/dance.vrma",
            "assets/anim/dance.vrma",
        ],
        _ => Vec::new(),
    }
}

fn looks_like_repo_root(p: &Path) -> bool {
    p.join("kagra-shared").join("Cargo.toml").is_file()
}

fn ancestors_of(p: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for a in p.ancestors() {
        out.push(a.to_path_buf());
        if out.len() >= 8 {
            break;
        }
    }
    out
}

/// Workspace root that holds `kagra-shared/Cargo.toml`.
///
/// Looks at `$KAGRA_ROOT`, `CARGO_MANIFEST_DIR/..`, cwd ancestors, then the
/// running `window.exe` (so `target/debug/examples` still finds repo-root
/// `assets/Emma.vrm`).
pub fn repo_root() -> Option<PathBuf> {
    if let Ok(raw) = std::env::var("KAGRA_ROOT") {
        let p = PathBuf::from(raw.trim());
        if looks_like_repo_root(&p) {
            return Some(p);
        }
    }
    let mut cands: Vec<PathBuf> = Vec::new();
    cands.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    if let Ok(cwd) = std::env::current_dir() {
        cands.extend(ancestors_of(&cwd));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.extend(ancestors_of(dir));
        }
    }
    cands.into_iter().find(|c| looks_like_repo_root(c))
}

/// Open a dump-relative asset (`assets/Emma.vrm`, alias `emma`) from the repo
/// root even when cwd is `target/debug/examples`.
pub fn resolve_asset(spec: &str) -> Option<PathBuf> {
    let spec = spec.trim();
    if spec.is_empty() {
        return None;
    }
    let mut names: Vec<String> = vec![spec.to_string()];
    let stem = Path::new(spec)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(spec);
    for key in [stem, spec] {
        for a in resolve_alias(key) {
            if !names.iter().any(|n| n == a) {
                names.push(a.to_string());
            }
        }
    }
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Some(root) = repo_root() {
        bases.push(root);
    }
    bases.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."));
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd);
    }
    for name in &names {
        let direct = PathBuf::from(name);
        if direct.is_file() {
            return Some(direct);
        }
        for base in &bases {
            let p = base.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emma_alias() {
        let a = resolve_alias("Emma");
        assert!(a.iter().any(|p| p.ends_with("Emma.vrm")));
    }

    #[test]
    fn emma_vrm_resolves_from_repo_root_not_cwd() {
        let root = repo_root().expect("repo root from CARGO_MANIFEST_DIR");
        assert!(
            root.join("kagra-shared").join("Cargo.toml").is_file(),
            "repo root {}",
            root.display()
        );
        // repo_root is searched before cwd, so target/debug/examples still works.
        let found = resolve_asset("assets/Emma.vrm").or_else(|| resolve_asset("emma"));
        match found {
            Some(p) => {
                assert!(
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.eq_ignore_ascii_case("Emma.vrm")),
                    "resolved {}",
                    p.display()
                );
                let canon = p.canonicalize().unwrap_or(p.clone());
                let root_canon = root.canonicalize().unwrap_or(root.clone());
                assert!(
                    canon.starts_with(&root_canon),
                    "must resolve under repo root, got {}",
                    p.display()
                );
            }
            None => {
                // CI: gitignored file missing. Alias still names the repo-relative path.
                assert!(resolve_alias("emma").contains(&"assets/Emma.vrm"));
            }
        }
    }
}

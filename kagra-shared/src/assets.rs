//! アセット別名（Python `kagra.contracts` と揃える）。

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

/// Filesystem root of a packaged game (Android `filesDir`, Windows bundle).
///
/// `resolve_asset` checks this before `$KAGRA_ROOT` and the compile-time
/// workspace path. The host sets it from `SharedSession::set_asset_root`.
static PACKAGED_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Point asset lookup at a packaged tree. `None` clears it.
pub fn set_packaged_root(root: Option<PathBuf>) {
    if let Ok(mut guard) = PACKAGED_ROOT.lock() {
        *guard = root;
    }
}

fn packaged_root() -> Option<PathBuf> {
    PACKAGED_ROOT.lock().ok().and_then(|guard| guard.clone())
}

/// Bytes staged by the browser (or a test) under a repo-relative path.
///
/// Wasm has no game directory. The page fetches `world.json` and the staged
/// assets, then [`mount_file`] so [`resolve_asset`] / [`read_bytes`] see them.
static MOUNTED: LazyLock<Mutex<HashMap<String, Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// `assets/a/../b.png` and `assets\\b.png` become `assets/b.png`.
pub fn normalize_mount_key(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Remember `bytes` at a repo-relative path. Empty or escaped keys are ignored.
pub fn mount_file(path: &str, bytes: Vec<u8>) {
    let key = normalize_mount_key(path);
    if key.is_empty() {
        return;
    }
    if let Ok(mut guard) = MOUNTED.lock() {
        guard.insert(key, bytes);
    }
}

pub fn unmount_file(path: &str) {
    let key = normalize_mount_key(path);
    if let Ok(mut guard) = MOUNTED.lock() {
        guard.remove(&key);
    }
}

pub fn clear_mount() {
    if let Ok(mut guard) = MOUNTED.lock() {
        guard.clear();
    }
}

fn lookup_mounted(path: &Path) -> Option<Vec<u8>> {
    let key = normalize_mount_key(&path.to_string_lossy());
    if key.is_empty() {
        return None;
    }
    MOUNTED.lock().ok()?.get(&key).cloned()
}

pub fn mounted_is_file(path: impl AsRef<Path>) -> bool {
    let key = normalize_mount_key(&path.as_ref().to_string_lossy());
    if key.is_empty() {
        return false;
    }
    MOUNTED
        .lock()
        .ok()
        .is_some_and(|guard| guard.contains_key(&key))
}

/// Mounted bytes, or a real file on disk.
pub fn is_file(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    mounted_is_file(path) || path.is_file()
}

pub fn read_bytes(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let path = path.as_ref();
    if let Some(bytes) = lookup_mounted(path) {
        return Ok(bytes);
    }
    std::fs::read(path)
}

pub fn read_to_string(path: impl AsRef<Path>) -> io::Result<String> {
    let bytes = read_bytes(path)?;
    String::from_utf8(bytes).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}

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
        "knight" => vec!["assets/library/kaykit/adventurers/characters/Knight.glb"],
        "rogue" => vec!["assets/library/kaykit/adventurers/characters/Rogue.glb"],
        "mage" => vec!["assets/library/kaykit/adventurers/characters/Mage.glb"],
        "barbarian" => vec!["assets/library/kaykit/adventurers/characters/Barbarian.glb"],
        "house" | "fantasy_house" => vec!["assets/models/placeholder/house.glb"],
        "inn" | "fantasy_inn" => vec!["assets/models/placeholder/inn.glb"],
        "tower" | "bell_tower" => vec!["assets/models/placeholder/bell_tower.glb"],
        "mill" => vec!["assets/models/placeholder/mill.glb"],
        "gazebo" => vec!["assets/models/placeholder/gazebo.glb"],
        "well" => vec!["assets/models/placeholder/well.glb"],
        "stylized_tree" | "tree" => {
            vec![
                "assets/models/placeholder/tree.glb",
                "assets/models/tree.glb",
            ]
        }
        "stylized_pine" | "pine" => vec!["assets/models/placeholder/pine.glb"],
        "rock" | "rock_medium" => vec!["assets/models/placeholder/rock.glb"],
        "chest" => vec!["assets/models/placeholder/chest.glb"],
        "biplane" | "airplane" | "plane_glb" => {
            vec!["assets/models/biplane.glb"]
        }
        "chess_set" | "chess_rook" | "chess_pawn" | "chess_bishop" | "chess_queen"
        | "chess_king" | "chess_knight" | "chess_board" => {
            vec!["assets/library/polyhaven/props/chess_set/chess_set_1k.gltf"]
        }
        _ => Vec::new(),
    }
}

fn looks_like_repo_root(p: &Path) -> bool {
    p.join("kagra-shared").join("Cargo.toml").is_file()
}

/// A cargo workspace, or a staged game tree (`assets/` or `kagra_game.json`).
fn usable_game_root(p: &Path) -> bool {
    looks_like_repo_root(p) || p.join("assets").is_dir() || p.join("kagra_game.json").is_file()
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
    if let Some(p) = packaged_root() {
        if usable_game_root(&p) {
            return Some(p);
        }
    }
    if let Ok(raw) = std::env::var("KAGRA_ROOT") {
        let p = PathBuf::from(raw.trim());
        if usable_game_root(&p) {
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
        let key = normalize_mount_key(name);
        if !key.is_empty() && mounted_is_file(Path::new(&key)) {
            return Some(PathBuf::from(key));
        }
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
    fn mounted_bytes_resolve_without_a_disk_file() {
        let rel = "assets/kagra_mount_probe.txt";
        mount_file("assets/../assets/kagra_mount_probe.txt", b"probe".to_vec());
        let path = resolve_asset(rel).expect("mounted alias path");
        assert_eq!(read_bytes(&path).expect("bytes"), b"probe");
        assert!(is_file(Path::new(rel)));
        unmount_file(rel);
        assert!(!mounted_is_file(Path::new(rel)));
    }

    #[test]
    fn emma_alias() {
        let a = resolve_alias("Emma");
        assert!(a.iter().any(|p| p.ends_with("Emma.vrm")));
    }

    #[test]
    fn packaged_root_resolves_before_the_workspace() {
        let tmp = std::env::temp_dir().join(format!("kagra-packaged-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("assets")).unwrap();
        std::fs::write(tmp.join("assets").join("marker.txt"), b"ok").unwrap();
        std::fs::write(tmp.join("kagra_game.json"), b"{}").unwrap();
        set_packaged_root(Some(tmp.clone()));
        let got = resolve_asset("assets/marker.txt").expect("marker via packaged root");
        let got = got.canonicalize().unwrap();
        let expect = tmp
            .join("assets")
            .join("marker.txt")
            .canonicalize()
            .unwrap();
        set_packaged_root(None);
        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(got, expect);
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

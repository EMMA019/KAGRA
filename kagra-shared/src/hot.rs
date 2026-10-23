//! Desktop reload of a world dump and its material files.
//!
//! `notify` is a desktop dependency. Wasm and Android keep [`watch_paths`]
//! and do not link the watcher. There is no editor window.

pub use crate::profile::watch_paths;

/// What changed. A dump reload replaces the document. A material reload
/// only re-reads images into the meshes that are already playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadKind {
    Dump,
    Material,
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
mod desktop {
    use super::ReloadKind;
    use crate::world_doc::WorldDoc;
    use notify::Watcher;
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{self, Receiver};

    pub struct DumpWatch {
        _watcher: notify::RecommendedWatcher,
        rx: Receiver<Result<notify::Event, notify::Error>>,
        dump: PathBuf,
    }

    impl DumpWatch {
        pub fn open(dump: &Path, doc: &WorldDoc) -> Result<Self, String> {
            let (tx, rx) = mpsc::channel();
            let mut watcher = notify::recommended_watcher(move |res| {
                let _ = tx.send(res);
            })
            .map_err(|e| e.to_string())?;
            for path in super::watch_paths(dump, doc) {
                if path.is_file() {
                    let _ = watcher.watch(&path, notify::RecursiveMode::NonRecursive);
                }
            }
            Ok(Self {
                _watcher: watcher,
                rx,
                dump: dump.to_path_buf(),
            })
        }

        /// Drain the watcher. A dump change wins over a material change in
        /// the same batch.
        pub fn poll(&self) -> Option<ReloadKind> {
            let mut kind = None;
            while let Ok(event) = self.rx.try_recv() {
                let Ok(event) = event else {
                    continue;
                };
                if event.paths.iter().any(|p| p == &self.dump) {
                    kind = Some(ReloadKind::Dump);
                } else if kind.is_none() {
                    kind = Some(ReloadKind::Material);
                }
            }
            kind
        }
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
pub use desktop::DumpWatch;

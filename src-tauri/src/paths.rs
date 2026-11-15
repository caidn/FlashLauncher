use std::path::{Path, PathBuf};

/// Where everything lives under the app data folder
/// (%APPDATA%\com.flashlauncher.app on Windows).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub db: PathBuf,
    /// Library copies of SWFs, named by SHA-256.
    pub swfs: PathBuf,
    /// art/<game id>/...
    pub art: PathBuf,
    /// Images pulled during inspection, before the user confirms an import.
    pub pending: PathBuf,
    /// flashpoint/<id>/<host>/<path>: extra files of downloaded games.
    pub content: PathBuf,
    /// Partial downloads.
    pub downloads: PathBuf,
}

impl AppPaths {
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        let p = Self {
            db: root.join("library.db"),
            swfs: root.join("swfs"),
            art: root.join("art"),
            pending: root.join("cache").join("pending"),
            content: root.join("flashpoint"),
            downloads: root.join("cache").join("downloads"),
            root,
        };
        for dir in [&p.root, &p.swfs, &p.art, &p.pending, &p.content, &p.downloads] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(p)
    }

    pub fn abs(&self, rel: &str) -> String {
        self.root.join(rel).to_string_lossy().into_owned()
    }

    /// Path relative to the data dir, with forward slashes, for storing in the DB.
    pub fn rel(&self, abs: &Path) -> String {
        abs.strip_prefix(&self.root)
            .unwrap_or(abs)
            .to_string_lossy()
            .replace('\\', "/")
    }

    /// Leftovers from inspections that were cancelled.
    pub fn clear_pending(&self) {
        for d in [&self.pending, &self.downloads] {
            let _ = std::fs::remove_dir_all(d);
            let _ = std::fs::create_dir_all(d);
        }
    }
}

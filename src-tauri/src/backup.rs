//! Library backups: one zip with the database (games, saves, play time,
//! collections, settings), the SWF copies, art, and downloaded games' files.
//!
//! Restoring replaces the whole library. The current one is moved aside to
//! before-restore/ first (only the latest is kept), so a restore can be
//! undone by hand if it was a mistake.

use crate::db;
use crate::error::{AppError, AppResult};
use crate::AppState;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

const MANIFEST: &str = "flash-launcher-backup.json";
const APP: &str = "Flash Launcher";
/// Folders under the data dir that make up the library.
const DIRS: &[&str] = &["swfs", "art", "flashpoint"];
const DB_FILE: &str = "library.db";

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    app: String,
    version: String,
    schema: i64,
    created: String,
    games: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupProgress {
    /// pack | unpack
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
}

fn files_under(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            files_under(root, &p, out);
        } else if let Ok(rel) = p.strip_prefix(root) {
            out.push(rel.to_path_buf());
        }
    }
}

fn zip_name(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Writes the backup to `dest` and returns its size in bytes.
pub fn export(st: &AppState, dest: &Path, mut progress: impl FnMut(BackupProgress)) -> AppResult<u64> {
    let root = &st.paths.root;
    let tmp_db = st.paths.downloads.join("backup-library.db");
    let _ = std::fs::remove_file(&tmp_db);
    // A consistent copy of the live database, WAL included.
    let (schema, created, games) = {
        let conn = st.db();
        conn.execute("VACUUM INTO ?1", [tmp_db.to_string_lossy()])?;
        let schema: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let games: i64 = conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?;
        (schema, db::now(&conn)?, games)
    };

    let mut files = Vec::new();
    for d in DIRS {
        files_under(root, &root.join(d), &mut files);
    }
    let total = files.len() + 1;
    let part = dest.with_extension("zip.part");
    let result = (|| -> AppResult<()> {
        let mut zip = zip::ZipWriter::new(File::create(&part)?);
        let deflate = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .large_file(true);
        // SWFs and PNGs are compressed already; storing them is much faster.
        let stored = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(true);
        let manifest = Manifest {
            app: APP.into(),
            version: env!("CARGO_PKG_VERSION").into(),
            schema,
            created,
            games,
        };
        zip.start_file(MANIFEST, deflate)?;
        zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
        zip.start_file(DB_FILE, deflate)?;
        std::io::copy(&mut File::open(&tmp_db)?, &mut zip)?;
        progress(BackupProgress {
            phase: "pack",
            done: 1,
            total,
        });
        let mut buf = vec![0u8; 256 * 1024];
        for (i, rel) in files.iter().enumerate() {
            let ext = rel.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
            let opts = match ext.as_deref() {
                Some("swf" | "png" | "jpg" | "jpeg" | "gif" | "mp3" | "zip") => stored,
                _ => deflate,
            };
            zip.start_file(zip_name(rel), opts)?;
            let mut f = File::open(root.join(rel))?;
            loop {
                let n = f.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                zip.write_all(&buf[..n])?;
            }
            if i % 20 == 0 || i + 1 == files.len() {
                progress(BackupProgress {
                    phase: "pack",
                    done: i + 2,
                    total,
                });
            }
        }
        zip.finish()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&tmp_db);
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, dest).or_else(|_| {
        std::fs::copy(&part, dest)?;
        std::fs::remove_file(&part)
    })?;
    Ok(std::fs::metadata(dest)?.len())
}

impl From<zip::result::ZipError> for AppError {
    fn from(e: zip::result::ZipError) -> Self {
        AppError(format!("zip: {e}"))
    }
}

/// Only these may come out of a backup: the database and the library folders.
fn allowed(rel: &Path) -> bool {
    let mut parts = rel.components();
    match parts.next() {
        Some(Component::Normal(first)) => {
            let first = first.to_string_lossy();
            (first == DB_FILE && parts.next().is_none())
                || (DIRS.contains(&first.as_ref()) && parts.all(|c| matches!(c, Component::Normal(_))))
        }
        _ => false,
    }
}

/// Replaces the library with the one in `src`.
pub fn restore(st: &AppState, src: &Path, mut progress: impl FnMut(BackupProgress)) -> AppResult<()> {
    let mut archive = zip::ZipArchive::new(File::open(src)?)
        .map_err(|_| AppError("that file isn't a Flash Launcher backup".into()))?;
    let manifest: Manifest = {
        let mut f = archive
            .by_name(MANIFEST)
            .map_err(|_| AppError("that zip isn't a Flash Launcher backup".into()))?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        serde_json::from_str(&s).map_err(|_| AppError("the backup's manifest is damaged".into()))?
    };
    if manifest.app != APP {
        return Err(AppError("that zip isn't a Flash Launcher backup".into()));
    }
    if manifest.schema > db::schema_version() {
        return Err(AppError(format!(
            "that backup is from a newer version of Flash Launcher ({}). Update the launcher first.",
            manifest.version
        )));
    }

    // Unpack next to the library, then check the database opens.
    let root = st.paths.root.clone();
    let staging = root.join("restore-staging");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let unpacked = (|| -> AppResult<()> {
        let total = archive.len();
        for i in 0..total {
            let mut entry = archive.by_index(i)?;
            let Some(rel) = entry.enclosed_name() else {
                continue;
            };
            if entry.is_dir() || !allowed(&rel) {
                continue;
            }
            let out = staging.join(&rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::io::copy(&mut entry, &mut File::create(&out)?)?;
            if i % 20 == 0 || i + 1 == total {
                progress(BackupProgress {
                    phase: "unpack",
                    done: i + 1,
                    total,
                });
            }
        }
        let db_path = staging.join(DB_FILE);
        if !db_path.is_file() {
            return Err(AppError("the backup has no library database in it".into()));
        }
        let conn = Connection::open(&db_path)?;
        db::init(&conn).map_err(|e| AppError(format!("the backup's database is damaged: {e}")))?;
        conn.pragma_update(None, "journal_mode", "DELETE")?;
        Ok(())
    })();
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(e);
    }

    // Swap, holding the DB lock so nothing writes in between. The old
    // connection has to be closed before its file can move (Windows).
    let mut guard = st.db();
    *guard = Connection::open_in_memory()?;
    let aside = root.join("before-restore");
    let _ = std::fs::remove_dir_all(&aside);
    std::fs::create_dir_all(&aside)?;
    let names: Vec<String> = [
        DB_FILE.to_string(),
        format!("{DB_FILE}-wal"),
        format!("{DB_FILE}-shm"),
    ]
    .into_iter()
    .chain(DIRS.iter().map(|d| d.to_string()))
    .collect();
    for name in &names {
        let p = root.join(name);
        if p.exists() {
            std::fs::rename(&p, aside.join(name))?;
        }
    }
    for name in std::iter::once(DB_FILE).chain(DIRS.iter().copied()) {
        let from = staging.join(name);
        if from.exists() {
            std::fs::rename(&from, root.join(name))?;
        }
    }
    for d in DIRS {
        std::fs::create_dir_all(root.join(d))?;
    }
    let _ = std::fs::remove_dir_all(&staging);
    *guard = db::open(&st.paths.db)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;
    use crate::scrape::Http;
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;

    fn state(dir: &Path) -> AppState {
        let paths = AppPaths::new(dir.to_path_buf()).unwrap();
        let conn = db::open(&paths.db).unwrap();
        AppState {
            db: Mutex::new(conn),
            http: Http::new(dir.join("cache/remote")).unwrap(),
            paths,
            scrape_cancel: AtomicBool::new(false),
            download_cancel: AtomicBool::new(false),
            presence: crate::discord::Presence::off(),
        }
    }

    #[test]
    fn round_trip() {
        let base = std::env::temp_dir().join(format!("fl-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let a = state(&base.join("a"));
        a.db()
            .execute("INSERT INTO settings (key, value) VALUES ('theme', 'light')", [])
            .unwrap();
        std::fs::write(a.paths.swfs.join("abc.swf"), b"FWS-ish").unwrap();
        std::fs::create_dir_all(a.paths.content.join("x/site.com")).unwrap();
        std::fs::write(a.paths.content.join("x/site.com/level.xml"), b"<l/>").unwrap();
        let zip = base.join("backup.zip");
        let size = export(&a, &zip, |_| {}).unwrap();
        assert!(size > 0);

        let b = state(&base.join("b"));
        std::fs::write(b.paths.swfs.join("old.swf"), b"old").unwrap();
        restore(&b, &zip, |_| {}).unwrap();
        let theme: String = b
            .db()
            .query_row("SELECT value FROM settings WHERE key = 'theme'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(theme, "light");
        assert!(b.paths.swfs.join("abc.swf").is_file());
        assert!(!b.paths.swfs.join("old.swf").exists());
        assert!(
            b.paths.root.join("before-restore/swfs/old.swf").is_file(),
            "old library kept aside"
        );
        assert_eq!(
            std::fs::read(b.paths.content.join("x/site.com/level.xml")).unwrap(),
            b"<l/>"
        );

        // Not a backup: refused, library untouched.
        std::fs::write(base.join("junk.zip"), b"nope").unwrap();
        assert!(restore(&b, &base.join("junk.zip"), |_| {}).is_err());
        assert!(b.paths.swfs.join("abc.swf").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn only_library_files_come_out() {
        assert!(allowed(Path::new("library.db")));
        assert!(allowed(Path::new("swfs/a.swf")));
        assert!(allowed(Path::new("flashpoint/id/host/a.swf")));
        assert!(!allowed(Path::new("library.db/x")));
        assert!(!allowed(Path::new("cache/x")));
        assert!(!allowed(Path::new("evil.exe")));
    }
}

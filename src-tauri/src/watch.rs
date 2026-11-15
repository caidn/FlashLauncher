//! Watched folders: SWFs dropped into them show up in the library on their
//! own. Scans run when the app starts and when its window comes back into
//! focus (at most once a minute), not with a file watcher, so nothing runs
//! in the background while you play.
//!
//! Every file a watched folder offers is remembered by hash, so a game you
//! delete from the library stays deleted even though the file is still in
//! the folder. Files are remembered by path, size and modified time too, so
//! a rescan only reads what's new or changed.

use crate::db;
use crate::error::AppResult;
use crate::import::{self, NewGame};
use crate::library::{self, Game};
use crate::AppState;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::time::UNIX_EPOCH;

pub const SETTING: &str = "watchedFolders";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchFailure {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WatchReport {
    pub added: Vec<Game>,
    pub failed: Vec<WatchFailure>,
    pub scanned: usize,
}

pub fn folders(st: &AppState) -> AppResult<Vec<String>> {
    let raw: Option<String> = st
        .db()
        .query_row("SELECT value FROM settings WHERE key = ?1", [SETTING], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(raw
        .and_then(|r| serde_json::from_str::<Vec<String>>(&r).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|f| std::path::Path::new(f).is_dir())
        .collect())
}

pub fn scan(st: &AppState) -> AppResult<WatchReport> {
    let folders = folders(st)?;
    let mut report = WatchReport::default();
    if folders.is_empty() {
        return Ok(report);
    }
    for path in import::expand_paths(&folders) {
        report.scanned += 1;
        let key = path.to_string_lossy().into_owned();
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        let size = meta.len() as i64;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let known: Option<(i64, i64)> = st
            .db()
            .query_row(
                "SELECT size, mtime FROM watched_files WHERE path = ?1",
                [&key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if known == Some((size, mtime)) {
            continue;
        }

        // Hashing happens without the lock; only the bookkeeping takes it.
        let bytes = match import::read_swf_file(&path) {
            Ok(b) => b,
            Err(e) => {
                report.failed.push(WatchFailure {
                    path: key,
                    error: e.0,
                });
                continue;
            }
        };
        let hash = import::sha256_of(&bytes);
        let conn = st.db();
        conn.execute(
            "INSERT INTO watched_files (path, size, mtime, sha256) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET size = excluded.size, mtime = excluded.mtime, sha256 = excluded.sha256",
            params![key, size, mtime, hash],
        )?;
        let seen: bool = conn
            .query_row("SELECT 1 FROM watched_seen WHERE sha256 = ?1", [&hash], |_| {
                Ok(true)
            })
            .optional()?
            .unwrap_or(false);
        let now = db::now(&conn)?;
        conn.execute(
            "INSERT OR IGNORE INTO watched_seen (sha256, seen_at) VALUES (?1, ?2)",
            params![hash, now],
        )?;
        if seen || library::find_by_hash(&conn, &hash)?.is_some() {
            continue;
        }
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match import::import_bytes(
            &conn,
            &st.paths,
            NewGame {
                bytes: &bytes,
                hash: &hash,
                file_name: &file_name,
                original_path: Some(&key),
                title: "",
                source: "uploaded",
            },
        ) {
            Ok(g) => report.added.push(g),
            Err(e) => report.failed.push(WatchFailure {
                path: key,
                error: e.0,
            }),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;
    use crate::scrape::Http;
    use crate::swf::tests::{body, fws};
    use rusqlite::Connection;
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;

    #[test]
    fn picks_up_new_files_once_and_respects_deletes() {
        let dir = std::env::temp_dir().join(format!("fl-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let watched = dir.join("games");
        std::fs::create_dir_all(&watched).unwrap();
        let paths = AppPaths::new(dir.join("data")).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let st = AppState {
            db: Mutex::new(conn),
            http: Http::new(dir.join("cache")).unwrap(),
            paths,
            scrape_cancel: AtomicBool::new(false),
            download_cancel: AtomicBool::new(false),
            presence: crate::discord::Presence::off(),
        };
        let folders = serde_json::to_string(&[watched.to_string_lossy()]).unwrap();
        st.db()
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)",
                params![SETTING, folders],
            )
            .unwrap();

        let swf = |bg: u8| fws(8, &body(550, 400, (24, 0), 10, &[(9, vec![bg, 0, 0])]));
        std::fs::write(watched.join("sky_ferry.swf"), swf(1)).unwrap();
        let r = scan(&st).unwrap();
        assert_eq!(r.added.len(), 1);
        assert_eq!(r.added[0].title, "Sky Ferry");

        // Nothing new: nothing added. A new file: just that one.
        assert!(scan(&st).unwrap().added.is_empty());
        std::fs::write(watched.join("moon_miner.swf"), swf(2)).unwrap();
        assert_eq!(scan(&st).unwrap().added.len(), 1);

        // Deleting a game doesn't make it come back, even if the file changes name.
        let id = r.added[0].id;
        library::delete_game(&st.db(), &st.paths, id).unwrap();
        std::fs::rename(watched.join("sky_ferry.swf"), watched.join("renamed.swf")).unwrap();
        assert!(scan(&st).unwrap().added.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

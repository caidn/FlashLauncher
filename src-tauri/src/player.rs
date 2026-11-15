//! Everything the player needs from the backend: the SWF bytes, saves,
//! play-time sessions, screenshots/art choice, and the external projector.

use crate::db;
use crate::error::{AppError, AppResult};
use crate::paths::AppPaths;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// Saves are tiny in practice; this only guards against a runaway game.
const MAX_SAVE_BYTES: usize = 32 * 1024 * 1024;
const MAX_SCREENSHOT_BYTES: usize = 40 * 1024 * 1024;

pub fn read_swf(conn: &Connection, paths: &AppPaths, id: i64) -> AppResult<Vec<u8>> {
    let file: String = conn
        .query_row("SELECT swf_file FROM games WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError(format!("game {id} not found")))?;
    std::fs::read(paths.root.join(&file)).map_err(|e| {
        AppError(format!(
            "couldn't read the library copy of this game ({e}). Try removing and re-adding it."
        ))
    })
}

// ---------------------------------------------------------------- saves

pub fn get_saves(conn: &Connection, id: i64) -> AppResult<Option<String>> {
    Ok(conn
        .query_row("SELECT data FROM saves WHERE game_id = ?1", [id], |r| r.get(0))
        .optional()?)
}

pub fn put_saves(conn: &Connection, id: i64, data: &str) -> AppResult<()> {
    if data.len() > MAX_SAVE_BYTES {
        return Err("save data is unreasonably large, not storing it".into());
    }
    // Must be a JSON object of key -> string, which is what the player sends.
    let parsed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(data)?;
    if !parsed.values().all(|v| v.is_string()) {
        return Err("save data must map keys to strings".into());
    }
    let now = db::now(conn)?;
    if parsed.is_empty() {
        conn.execute("DELETE FROM saves WHERE game_id = ?1", [id])?;
    } else {
        conn.execute(
            "INSERT INTO saves (game_id, data, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(game_id) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at",
            params![id, data, now],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------- sessions

/// Starts a play session and stamps the game as last played.
pub fn start_session(conn: &Connection, id: i64) -> AppResult<i64> {
    let tx = conn.unchecked_transaction()?;
    let now = db::now(&tx)?;
    tx.execute(
        "INSERT INTO play_sessions (game_id, started_at, seconds) VALUES (?1, ?2, 0)",
        params![id, now],
    )?;
    let session = tx.last_insert_rowid();
    tx.execute(
        "UPDATE games SET last_played = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    tx.commit()?;
    Ok(session)
}

/// Records the session's running total. Only the increase since the last
/// update is added to the game, so sending the same total twice is harmless.
pub fn update_session(conn: &Connection, session: i64, seconds: i64) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    let row: Option<(i64, i64)> = tx
        .query_row(
            "SELECT game_id, seconds FROM play_sessions WHERE id = ?1",
            [session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((game_id, prev)) = row else {
        return Err(AppError(format!("play session {session} not found")));
    };
    let delta = seconds - prev;
    if delta > 0 {
        tx.execute(
            "UPDATE play_sessions SET seconds = ?2 WHERE id = ?1",
            params![session, seconds],
        )?;
        tx.execute(
            "UPDATE games SET play_seconds = play_seconds + ?2 WHERE id = ?1",
            params![game_id, delta],
        )?;
    }
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------- art

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtItem {
    pub id: i64,
    pub slot: String,
    pub source: String,
    /// Absolute path once downloaded; empty for remote candidates.
    pub path: String,
    pub remote_url: Option<String>,
    pub thumb_url: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub selected: bool,
}

/// Every image we know about for a game, in every slot: downloaded or
/// local ones first within a slot, then remote candidates.
pub fn list_art(conn: &Connection, paths: &AppPaths, game_id: i64) -> AppResult<Vec<ArtItem>> {
    let mut stmt = conn.prepare(
        "SELECT id, slot, source, file, width, height, selected, remote_url, thumb_url FROM art
         WHERE game_id = ?1
         ORDER BY slot, selected DESC, file = '' , id DESC",
    )?;
    let items = stmt
        .query_map([game_id], |r| {
            let file: String = r.get(3)?;
            Ok(ArtItem {
                id: r.get(0)?,
                slot: r.get(1)?,
                source: r.get(2)?,
                path: if file.is_empty() {
                    String::new()
                } else {
                    paths.abs(&file)
                },
                width: r.get(4)?,
                height: r.get(5)?,
                selected: r.get(6)?,
                remote_url: r.get(7)?,
                thumb_url: r.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

pub fn add_screenshot(conn: &Connection, paths: &AppPaths, game_id: i64, png: &[u8]) -> AppResult<ArtItem> {
    if png.len() > MAX_SCREENSHOT_BYTES {
        return Err("screenshot is too large".into());
    }
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
        [game_id],
        |r| r.get(0),
    )?;
    if !exists {
        return Err(AppError(format!("game {game_id} not found")));
    }
    // Decode to make sure it's a real image and to learn its size.
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|e| AppError(format!("screenshot isn't a valid PNG: {e}")))?;
    let dir = paths.art.join(game_id.to_string());
    std::fs::create_dir_all(&dir)?;
    let stamp: String = conn.query_row("SELECT strftime('%Y%m%d-%H%M%f', 'now')", [], |r| r.get(0))?;
    let file = dir.join(format!("screenshot-{}.png", stamp.replace('.', "")));
    std::fs::write(&file, png)?;
    conn.execute(
        "INSERT INTO art (game_id, slot, source, file, width, height, selected)
         VALUES (?1, 'screenshot', 'player', ?2, ?3, ?4, 0)",
        params![game_id, paths.rel(&file), img.width(), img.height()],
    )?;
    let id = conn.last_insert_rowid();
    Ok(ArtItem {
        id,
        slot: "screenshot".into(),
        source: "player".into(),
        path: file.to_string_lossy().into_owned(),
        remote_url: None,
        thumb_url: None,
        width: Some(img.width() as i64),
        height: Some(img.height() as i64),
        selected: false,
    })
}

/// Screenshots you took and images you uploaded can be deleted. Images
/// from the SWF or a scraper can't (they'd come back on the next scan);
/// pick something else for the slot instead.
pub fn delete_art(conn: &Connection, paths: &AppPaths, game_id: i64, art_id: i64) -> AppResult<()> {
    let file: Option<String> = conn
        .query_row(
            "SELECT file FROM art WHERE id = ?2 AND game_id = ?1 AND source IN ('player', 'user')",
            params![game_id, art_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(file) = file else {
        return Err(AppError(format!("image {art_id} can't be deleted")));
    };
    conn.execute("DELETE FROM art WHERE id = ?1", [art_id])?;
    let _ = std::fs::remove_file(paths.root.join(file));
    Ok(())
}

// ---------------------------------------------------------------- projector

pub const PROJECTOR_SETTING: &str = "projectorPath";

/// Returns the projector exe and the game's SWF path, or a message saying
/// what's missing.
pub fn projector_command(conn: &Connection, paths: &AppPaths, game_id: i64) -> AppResult<(String, String)> {
    let exe: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [PROJECTOR_SETTING],
            |r| r.get(0),
        )
        .optional()?;
    let exe = exe.filter(|e| !e.trim().is_empty()).ok_or_else(|| {
        AppError("no Flash Player projector is set up. Add one in settings, under player.".into())
    })?;
    if !std::path::Path::new(&exe).is_file() {
        return Err(AppError(format!(
            "the projector at {exe} doesn't exist anymore. Check settings."
        )));
    }
    let (swf, content, launch): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT swf_file, content_id, launch_url FROM games WHERE id = ?1",
            [game_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| AppError(format!("game {game_id} not found")))?;
    // Downloaded games run from inside their own files, so relative loads
    // (levels, sounds next to the SWF) find what they need.
    if let (Some(c), Some(url)) = (content, launch) {
        if let Some(rel) = crate::download::url_to_rel(&url).filter(|_| crate::download::valid_id(&c)) {
            let p = paths.content.join(&c).join(rel);
            if p.is_file() {
                return Ok((exe, p.to_string_lossy().into_owned()));
            }
        }
    }
    Ok((exe, paths.abs(&swf)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Connection, AppPaths, std::path::PathBuf, i64) {
        let dir =
            std::env::temp_dir().join(format!("fl-player-test-{}-{}", std::process::id(), rand_suffix()));
        let paths = AppPaths::new(dir.clone()).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        conn.execute(
            "INSERT INTO games (title, sort_title, swf_file, original_name, sha256, file_size, swf_version,
                compression, width, height, frame_rate, frame_count, as_version, color, added_at)
             VALUES ('T', 't', 'swfs/x.swf', 'x.swf', 'abc', 3, 8, 'none', 550, 400, 24, 1, 2, '#000000', 'now')",
            [],
        )
        .unwrap();
        std::fs::write(dir.join("swfs/x.swf"), b"FWS").unwrap();
        (conn, paths, dir, 1)
    }

    fn rand_suffix() -> u32 {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        N.fetch_add(1, Ordering::SeqCst)
    }

    #[test]
    fn sessions_add_only_new_time() {
        let (conn, _p, dir, id) = setup();
        let s = start_session(&conn, id).unwrap();
        update_session(&conn, s, 30).unwrap();
        update_session(&conn, s, 30).unwrap(); // repeat is a no-op
        update_session(&conn, s, 75).unwrap();
        update_session(&conn, s, 10).unwrap(); // going backwards is ignored
        let (secs, last): (i64, Option<String>) = conn
            .query_row(
                "SELECT play_seconds, last_played FROM games WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(secs, 75);
        assert!(last.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn saves_round_trip_and_survive_delete_and_reimport() {
        let (conn, paths, dir, id) = setup();
        assert_eq!(get_saves(&conn, id).unwrap(), None);
        put_saves(&conn, id, r#"{"localhost/game/slot1":"abc"}"#).unwrap();
        assert!(get_saves(&conn, id).unwrap().unwrap().contains("slot1"));
        assert!(put_saves(&conn, id, r#"{"k": 5}"#).is_err());
        assert!(put_saves(&conn, id, "not json").is_err());

        crate::library::delete_game(&conn, &paths, id).unwrap();
        let backup: String = conn
            .query_row("SELECT data FROM save_backups WHERE sha256 = 'abc'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(backup.contains("slot1"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn read_swf_and_art_selection() {
        let (conn, paths, dir, id) = setup();
        assert_eq!(read_swf(&conn, &paths, id).unwrap(), b"FWS");
        assert!(read_swf(&conn, &paths, 99).is_err());

        let img = image::RgbaImage::from_pixel(40, 30, image::Rgba([10, 200, 30, 255]));
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let shot = add_screenshot(&conn, &paths, id, &png).unwrap();
        assert_eq!((shot.width, shot.height), (Some(40), Some(30)));
        assert!(add_screenshot(&conn, &paths, id, b"nope").is_err());

        conn.execute("UPDATE art SET selected = 1 WHERE id = ?1", [shot.id])
            .unwrap();
        let game = crate::library::get_game(&conn, &paths, id).unwrap();
        assert_eq!(game.fallback_image.as_deref(), Some(shot.path.as_str()));
        assert_eq!(game.art.screenshot.as_deref(), Some(shot.path.as_str()));

        delete_art(&conn, &paths, id, shot.id).unwrap();
        assert!(list_art(&conn, &paths, id).unwrap().is_empty());
        assert!(!std::path::Path::new(&shot.path).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn projector_needs_a_real_exe() {
        let (conn, paths, dir, id) = setup();
        assert!(projector_command(&conn, &paths, id)
            .unwrap_err()
            .0
            .contains("no Flash Player projector"));
        crate::library::set_setting(&conn, PROJECTOR_SETTING, "/definitely/not/here.exe").unwrap();
        assert!(projector_command(&conn, &paths, id)
            .unwrap_err()
            .0
            .contains("doesn't exist"));
        let fake = dir.join("projector.exe");
        std::fs::write(&fake, b"").unwrap();
        crate::library::set_setting(&conn, PROJECTOR_SETTING, &fake.to_string_lossy()).unwrap();
        let (exe, swf) = projector_command(&conn, &paths, id).unwrap();
        assert!(exe.ends_with("projector.exe") && swf.ends_with("x.swf"));
        let _ = std::fs::remove_dir_all(dir);
    }
}

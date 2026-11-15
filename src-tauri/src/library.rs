//! Library queries: reading and changing games and collections.

use crate::error::AppResult;
use crate::paths::AppPaths;
use crate::title;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub year: Option<i64>,
    pub description: Option<String>,
    pub source: String,
    /// Absolute path of the library's copy of the SWF.
    pub swf_path: String,
    pub original_name: String,
    pub sha256: String,
    pub file_size: i64,
    pub swf_version: i64,
    pub compression: String,
    pub width: f64,
    pub height: f64,
    pub frame_rate: f64,
    pub frame_count: i64,
    pub as_version: i64,
    pub uses_network: bool,
    pub background: Option<String>,
    pub compat: String,
    pub color: String,
    pub favorite: bool,
    pub play_seconds: i64,
    pub last_played: Option<String>,
    pub added_at: String,
    pub runtime: String,
    pub allow_network: bool,
    pub tags: Vec<String>,
    /// Absolute path of the image chosen for generated art (pulled from the
    /// SWF or a screenshot), if any.
    pub fallback_image: Option<String>,
    pub scale_mode: String,
    pub quality: String,
    pub frame_rate_override: Option<f64>,
    pub letterbox: Option<String>,
    pub series: Option<String>,
    pub release_date: Option<String>,
    pub scraped_at: Option<String>,
    pub title_locked: bool,
    /// Selected image per slot (absolute paths).
    pub art: ArtSet,
    /// Sources this game is matched to ("flashpoint", "steamgriddb", ...).
    pub links: Vec<String>,
    /// Flashpoint id when the game was downloaded with its own extra files.
    pub content_id: Option<String>,
    /// Where the main SWF originally lived (its loads are resolved from there).
    pub launch_url: Option<String>,
    /// JSON object of controller button -> key code, or None for the default.
    pub gamepad_map: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ArtSet {
    pub cover: Option<String>,
    pub wide: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    pub icon: Option<String>,
    pub screenshot: Option<String>,
    pub embedded: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub game_ids: Vec<i64>,
}

/// Fields the UI may change. Anything left out stays as it is.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePatch {
    pub title: Option<String>,
    pub favorite: Option<bool>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub year: Option<i64>,
    pub description: Option<String>,
    pub compat: Option<String>,
    pub scale_mode: Option<String>,
    pub quality: Option<String>,
    /// 0 or less clears the override.
    pub frame_rate_override: Option<f64>,
    /// Empty string clears it.
    pub letterbox: Option<String>,
    pub runtime: Option<String>,
    pub allow_network: Option<bool>,
    /// JSON object; empty string goes back to the default layout.
    pub gamepad_map: Option<String>,
}

const GAME_COLUMNS: &str = "g.id, g.title, g.developer, g.publisher, g.year, g.description, g.source, g.swf_file,
    g.original_name, g.sha256, g.file_size, g.swf_version, g.compression, g.width, g.height, g.frame_rate,
    g.frame_count, g.as_version, g.uses_network, g.background, g.compat, g.color, g.favorite, g.play_seconds,
    g.last_played, g.added_at, g.runtime, g.allow_network,
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot IN ('embedded', 'screenshot') AND a.selected = 1
        AND a.file <> '' ORDER BY a.slot = 'embedded' DESC LIMIT 1),
    (SELECT group_concat(tag, char(31)) FROM game_tags t WHERE t.game_id = g.id),
    g.scale_mode, g.quality, g.frame_rate_override, g.letterbox,
    g.series, g.release_date, g.scraped_at, g.title_locked,
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot = 'cover' AND a.selected = 1 AND a.file <> '' LIMIT 1),
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot = 'wide' AND a.selected = 1 AND a.file <> '' LIMIT 1),
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot = 'hero' AND a.selected = 1 AND a.file <> '' LIMIT 1),
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot = 'logo' AND a.selected = 1 AND a.file <> '' LIMIT 1),
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot = 'icon' AND a.selected = 1 AND a.file <> '' LIMIT 1),
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot = 'screenshot' AND a.selected = 1 AND a.file <> '' LIMIT 1),
    (SELECT file FROM art a WHERE a.game_id = g.id AND a.slot = 'embedded' AND a.selected = 1 AND a.file <> '' LIMIT 1),
    (SELECT group_concat(source) FROM game_links l WHERE l.game_id = g.id),
    g.content_id, g.launch_url, g.gamepad_map";

fn row_to_game(r: &Row, paths: &AppPaths) -> rusqlite::Result<Game> {
    let swf_file: String = r.get(7)?;
    let fallback: Option<String> = r.get(28)?;
    let tags: Option<String> = r.get(29)?;
    Ok(Game {
        id: r.get(0)?,
        title: r.get(1)?,
        developer: r.get(2)?,
        publisher: r.get(3)?,
        year: r.get(4)?,
        description: r.get(5)?,
        source: r.get(6)?,
        swf_path: paths.abs(&swf_file),
        original_name: r.get(8)?,
        sha256: r.get(9)?,
        file_size: r.get(10)?,
        swf_version: r.get(11)?,
        compression: r.get(12)?,
        width: r.get(13)?,
        height: r.get(14)?,
        frame_rate: r.get(15)?,
        frame_count: r.get(16)?,
        as_version: r.get(17)?,
        uses_network: r.get(18)?,
        background: r.get(19)?,
        compat: r.get(20)?,
        color: r.get(21)?,
        favorite: r.get(22)?,
        play_seconds: r.get(23)?,
        last_played: r.get(24)?,
        added_at: r.get(25)?,
        runtime: r.get(26)?,
        allow_network: r.get(27)?,
        tags: tags
            .map(|t| t.split('\u{1f}').map(String::from).collect())
            .unwrap_or_default(),
        fallback_image: fallback.map(|f| paths.abs(&f)),
        scale_mode: r.get(30)?,
        quality: r.get(31)?,
        frame_rate_override: r.get(32)?,
        letterbox: r.get(33)?,
        series: r.get(34)?,
        release_date: r.get(35)?,
        scraped_at: r.get(36)?,
        title_locked: r.get(37)?,
        art: {
            let slot = |i: usize| -> rusqlite::Result<Option<String>> {
                Ok(r.get::<_, Option<String>>(i)?.map(|f| paths.abs(&f)))
            };
            ArtSet {
                cover: slot(38)?,
                wide: slot(39)?,
                hero: slot(40)?,
                logo: slot(41)?,
                icon: slot(42)?,
                screenshot: slot(43)?,
                embedded: slot(44)?,
            }
        },
        links: r
            .get::<_, Option<String>>(45)?
            .map(|s| s.split(',').map(String::from).collect())
            .unwrap_or_default(),
        content_id: r.get(46)?,
        launch_url: r.get(47)?,
        gamepad_map: r.get(48)?,
    })
}

pub fn list_games(conn: &Connection, paths: &AppPaths) -> AppResult<Vec<Game>> {
    let sql = format!("SELECT {GAME_COLUMNS} FROM games g ORDER BY g.sort_title");
    let mut stmt = conn.prepare(&sql)?;
    let games = stmt
        .query_map([], |r| row_to_game(r, paths))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(games)
}

pub fn get_game(conn: &Connection, paths: &AppPaths, id: i64) -> AppResult<Game> {
    let sql = format!("SELECT {GAME_COLUMNS} FROM games g WHERE g.id = ?1");
    conn.query_row(&sql, [id], |r| row_to_game(r, paths))
        .optional()?
        .ok_or_else(|| format!("game {id} not found").into())
}

pub fn find_by_hash(conn: &Connection, sha256: &str) -> AppResult<Option<(i64, String)>> {
    Ok(conn
        .query_row("SELECT id, title FROM games WHERE sha256 = ?1", [sha256], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?)
}

pub fn update_game(conn: &Connection, paths: &AppPaths, id: i64, patch: GamePatch) -> AppResult<Game> {
    let tx = conn.unchecked_transaction()?;
    if let Some(t) = patch.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        // A title the user typed is theirs: scrapers won't overwrite it.
        tx.execute(
            "UPDATE games SET title = ?2, sort_title = ?3, title_locked = 1 WHERE id = ?1",
            params![id, t, title::normalize(t)],
        )?;
    }
    if let Some(f) = patch.favorite {
        tx.execute("UPDATE games SET favorite = ?2 WHERE id = ?1", params![id, f])?;
    }
    if let Some(v) = patch.developer {
        tx.execute(
            "UPDATE games SET developer = NULLIF(TRIM(?2), '') WHERE id = ?1",
            params![id, v],
        )?;
    }
    if let Some(v) = patch.publisher {
        tx.execute(
            "UPDATE games SET publisher = NULLIF(TRIM(?2), '') WHERE id = ?1",
            params![id, v],
        )?;
    }
    if let Some(v) = patch.year {
        tx.execute("UPDATE games SET year = ?2 WHERE id = ?1", params![id, v])?;
    }
    if let Some(v) = patch.description {
        tx.execute(
            "UPDATE games SET description = NULLIF(TRIM(?2), '') WHERE id = ?1",
            params![id, v],
        )?;
    }
    if let Some(v) = patch.compat {
        one_of(&v, &["works", "partial", "projector", "unknown"], "compatibility")?;
        tx.execute("UPDATE games SET compat = ?2 WHERE id = ?1", params![id, v])?;
    }
    if let Some(v) = patch.scale_mode {
        one_of(&v, &["fit", "stretch", "original"], "scale mode")?;
        tx.execute("UPDATE games SET scale_mode = ?2 WHERE id = ?1", params![id, v])?;
    }
    if let Some(v) = patch.quality {
        one_of(&v, &["low", "medium", "high", "best"], "quality")?;
        tx.execute("UPDATE games SET quality = ?2 WHERE id = ?1", params![id, v])?;
    }
    if let Some(v) = patch.frame_rate_override {
        let v = (v > 0.0).then_some(v.min(240.0));
        tx.execute(
            "UPDATE games SET frame_rate_override = ?2 WHERE id = ?1",
            params![id, v],
        )?;
    }
    if let Some(v) = patch.letterbox {
        let v = v.trim();
        let valid = v.is_empty()
            || (v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit()));
        if !valid {
            return Err(format!("letterbox must be a #rrggbb color, got {v}").into());
        }
        tx.execute(
            "UPDATE games SET letterbox = NULLIF(?2, '') WHERE id = ?1",
            params![id, v],
        )?;
    }
    if let Some(v) = patch.runtime {
        one_of(&v, &["ruffle", "projector"], "runtime")?;
        tx.execute("UPDATE games SET runtime = ?2 WHERE id = ?1", params![id, v])?;
    }
    if let Some(v) = patch.gamepad_map {
        let v = v.trim();
        if !v.is_empty() {
            let parsed: serde_json::Value =
                serde_json::from_str(v).map_err(|e| format!("controller layout isn't valid JSON: {e}"))?;
            let ok = parsed
                .as_object()
                .is_some_and(|o| o.values().all(|k| k.as_u64().is_some_and(|k| k < 256)));
            if !ok {
                return Err("controller layout must map buttons to key codes".into());
            }
        }
        tx.execute(
            "UPDATE games SET gamepad_map = NULLIF(?2, '') WHERE id = ?1",
            params![id, v],
        )?;
    }
    if let Some(v) = patch.allow_network {
        tx.execute(
            "UPDATE games SET allow_network = ?2 WHERE id = ?1",
            params![id, v],
        )?;
    }
    tx.commit()?;
    get_game(conn, paths, id)
}

fn one_of(v: &str, allowed: &[&str], what: &str) -> AppResult<()> {
    if allowed.contains(&v) {
        Ok(())
    } else {
        Err(format!("unknown {what}: {v}").into())
    }
}

/// Removes the game row and its files (the SWF copy and its art folder).
/// Its saves are kept aside by file hash, so re-importing the same SWF
/// brings them back.
pub fn delete_game(conn: &Connection, paths: &AppPaths, id: i64) -> AppResult<()> {
    let row: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT swf_file, sha256, content_id FROM games WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((swf_file, sha, content_id)) = row else {
        return Ok(());
    };
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO save_backups (sha256, data, deleted_at)
         SELECT ?2, data, strftime('%Y-%m-%dT%H:%M:%fZ', 'now') FROM saves WHERE game_id = ?1
         ON CONFLICT(sha256) DO UPDATE SET data = excluded.data, deleted_at = excluded.deleted_at",
        params![id, sha],
    )?;
    tx.execute("DELETE FROM games WHERE id = ?1", [id])?;
    tx.commit()?;
    let _ = std::fs::remove_file(paths.root.join(&swf_file));
    let _ = std::fs::remove_dir_all(paths.art.join(id.to_string()));
    if let Some(c) = content_id.filter(|c| crate::download::valid_id(c)) {
        let _ = std::fs::remove_dir_all(paths.content.join(c));
    }
    Ok(())
}

pub fn list_collections(conn: &Connection) -> AppResult<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name,
            (SELECT group_concat(game_id) FROM
                (SELECT game_id FROM collection_games WHERE collection_id = c.id ORDER BY position))
         FROM collections c ORDER BY c.name COLLATE NOCASE",
    )?;
    let rows = stmt
        .query_map([], |r| {
            let ids: Option<String> = r.get(2)?;
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                game_ids: ids
                    .map(|s| s.split(',').filter_map(|x| x.parse().ok()).collect())
                    .unwrap_or_default(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_settings(conn: &Connection) -> AppResult<std::collections::HashMap<String, String>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let map = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(map)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

// ---------------------------------------------------------------- collections

fn clean_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("a collection needs a name".into());
    }
    if n.chars().count() > 80 {
        return Err("that name is too long (80 characters max)".into());
    }
    Ok(n.to_string())
}

fn get_collection(conn: &Connection, id: i64) -> AppResult<Collection> {
    list_collections(conn)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("collection {id} not found").into())
}

pub fn create_collection(conn: &Connection, name: &str, game_ids: &[i64]) -> AppResult<Collection> {
    let name = clean_name(name)?;
    let tx = conn.unchecked_transaction()?;
    let now = crate::db::now(&tx)?;
    tx.execute(
        "INSERT INTO collections (name, created_at) VALUES (?1, ?2)",
        params![name, now],
    )?;
    let id = tx.last_insert_rowid();
    for (pos, g) in game_ids.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO collection_games (collection_id, game_id, position) VALUES (?1, ?2, ?3)",
            params![id, g, pos as i64],
        )?;
    }
    tx.commit()?;
    get_collection(conn, id)
}

pub fn rename_collection(conn: &Connection, id: i64, name: &str) -> AppResult<Collection> {
    let name = clean_name(name)?;
    if conn.execute(
        "UPDATE collections SET name = ?2 WHERE id = ?1",
        params![id, name],
    )? == 0
    {
        return Err(format!("collection {id} not found").into());
    }
    get_collection(conn, id)
}

pub fn delete_collection(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

/// Adds games to the end of a collection (games already in it stay put).
pub fn add_to_collection(conn: &Connection, id: i64, game_ids: &[i64]) -> AppResult<Collection> {
    let tx = conn.unchecked_transaction()?;
    let next: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM collection_games WHERE collection_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    for (i, g) in game_ids.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO collection_games (collection_id, game_id, position) VALUES (?1, ?2, ?3)",
            params![id, g, next + i as i64],
        )?;
    }
    tx.commit()?;
    get_collection(conn, id)
}

pub fn remove_from_collection(conn: &Connection, id: i64, game_ids: &[i64]) -> AppResult<Collection> {
    for g in game_ids {
        conn.execute(
            "DELETE FROM collection_games WHERE collection_id = ?1 AND game_id = ?2",
            params![id, g],
        )?;
    }
    get_collection(conn, id)
}

/// Replaces the order of a collection's games (ids not in it are ignored).
pub fn reorder_collection(conn: &Connection, id: i64, game_ids: &[i64]) -> AppResult<Collection> {
    let tx = conn.unchecked_transaction()?;
    for (pos, g) in game_ids.iter().enumerate() {
        tx.execute(
            "UPDATE collection_games SET position = ?3 WHERE collection_id = ?1 AND game_id = ?2",
            params![id, g, pos as i64],
        )?;
    }
    tx.commit()?;
    get_collection(conn, id)
}

#[cfg(test)]
mod collection_tests {
    use super::*;

    fn conn_with_games(n: i64) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::init(&conn).unwrap();
        for i in 1..=n {
            conn.execute(
                "INSERT INTO games (title, sort_title, swf_file, original_name, sha256, file_size, swf_version,
                    compression, width, height, frame_rate, frame_count, as_version, color, added_at)
                 VALUES (?1, ?1, 'x', 'x', ?2, 1, 8, 'none', 1, 1, 1, 1, 2, '#000', 'now')",
                params![format!("g{i}"), format!("h{i}")],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn collection_lifecycle() {
        let conn = conn_with_games(4);
        let c = create_collection(&conn, "  couch co-op ", &[2, 1]).unwrap();
        assert_eq!(c.name, "couch co-op");
        assert_eq!(c.game_ids, vec![2, 1]);
        assert!(create_collection(&conn, "   ", &[]).is_err());

        let c = add_to_collection(&conn, c.id, &[3, 1]).unwrap();
        assert_eq!(
            c.game_ids,
            vec![2, 1, 3],
            "duplicates are ignored, new ones go last"
        );
        let c = reorder_collection(&conn, c.id, &[3, 2, 1]).unwrap();
        assert_eq!(c.game_ids, vec![3, 2, 1]);
        let c = remove_from_collection(&conn, c.id, &[2]).unwrap();
        assert_eq!(c.game_ids, vec![3, 1]);
        let c = rename_collection(&conn, c.id, "lab").unwrap();
        assert_eq!(c.name, "lab");

        // deleting a game drops it from collections
        conn.execute("DELETE FROM games WHERE id = 3", []).unwrap();
        assert_eq!(get_collection(&conn, c.id).unwrap().game_ids, vec![1]);

        delete_collection(&conn, c.id).unwrap();
        assert!(list_collections(&conn).unwrap().is_empty());
    }
}

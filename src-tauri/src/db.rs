//! SQLite setup and schema migrations. Each migration runs once, tracked by
//! PRAGMA user_version, so later steps just append to MIGRATIONS.

use rusqlite::Connection;
use std::path::Path;

const MIGRATIONS: &[&str] = &[
    // 1: library basics
    r#"
    CREATE TABLE games (
        id             INTEGER PRIMARY KEY,
        title          TEXT    NOT NULL,
        sort_title     TEXT    NOT NULL,
        developer      TEXT,
        publisher      TEXT,
        year           INTEGER,
        description    TEXT,
        source         TEXT    NOT NULL DEFAULT 'uploaded',  -- uploaded | archive
        swf_file       TEXT    NOT NULL,                     -- relative to the data dir
        original_name  TEXT    NOT NULL,
        original_path  TEXT,
        sha256         TEXT    NOT NULL UNIQUE,
        file_size      INTEGER NOT NULL,
        swf_version    INTEGER NOT NULL,
        compression    TEXT    NOT NULL,
        width          REAL    NOT NULL,
        height         REAL    NOT NULL,
        frame_rate     REAL    NOT NULL,
        frame_count    INTEGER NOT NULL,
        as_version     INTEGER NOT NULL,
        uses_network   INTEGER NOT NULL DEFAULT 0,
        background     TEXT,
        compat         TEXT    NOT NULL DEFAULT 'unknown',   -- works | partial | projector | unknown
        color          TEXT    NOT NULL,
        favorite       INTEGER NOT NULL DEFAULT 0,
        play_seconds   INTEGER NOT NULL DEFAULT 0,
        last_played    TEXT,
        added_at       TEXT    NOT NULL,
        runtime        TEXT    NOT NULL DEFAULT 'ruffle',    -- ruffle | projector
        allow_network  INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX games_sort_title ON games(sort_title);

    CREATE TABLE game_tags (
        game_id INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
        tag     TEXT    NOT NULL,
        PRIMARY KEY (game_id, tag)
    );

    -- Every image we know about for a game. slot is 'embedded' for images
    -- pulled from the SWF; scraped art adds cover/hero/logo/icon/screenshot.
    CREATE TABLE art (
        id       INTEGER PRIMARY KEY,
        game_id  INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
        slot     TEXT    NOT NULL,
        source   TEXT    NOT NULL,
        file     TEXT    NOT NULL,   -- relative to the data dir
        width    INTEGER,
        height   INTEGER,
        selected INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX art_game ON art(game_id, slot);

    CREATE TABLE collections (
        id         INTEGER PRIMARY KEY,
        name       TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE TABLE collection_games (
        collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
        game_id       INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
        position      INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (collection_id, game_id)
    );

    CREATE TABLE play_sessions (
        id         INTEGER PRIMARY KEY,
        game_id    INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
        started_at TEXT    NOT NULL,
        seconds    INTEGER NOT NULL DEFAULT 0
    );

    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#,
    // 2: player settings per game, saves, art for screenshots
    r#"
    ALTER TABLE games ADD COLUMN scale_mode TEXT NOT NULL DEFAULT 'fit';     -- fit | stretch | original
    ALTER TABLE games ADD COLUMN quality TEXT NOT NULL DEFAULT 'high';       -- low | medium | high | best
    ALTER TABLE games ADD COLUMN frame_rate_override REAL;                   -- NULL = the SWF's own
    ALTER TABLE games ADD COLUMN letterbox TEXT;                             -- NULL = black

    -- Flash SharedObjects (game saves), as the JSON blob the player keeps.
    CREATE TABLE saves (
        game_id    INTEGER PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
        data       TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    -- Saves of deleted games, keyed by file hash so re-importing the same
    -- SWF brings them back.
    CREATE TABLE save_backups (
        sha256     TEXT PRIMARY KEY,
        data       TEXT NOT NULL,
        deleted_at TEXT NOT NULL
    );
    "#,
    // 3: scraped metadata and art
    r#"
    ALTER TABLE games ADD COLUMN series TEXT;
    ALTER TABLE games ADD COLUMN release_date TEXT;
    -- set when the user renames a game, so scrapers never overwrite it
    ALTER TABLE games ADD COLUMN title_locked INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE games ADD COLUMN scraped_at TEXT;

    -- art rows can now be remote candidates: file stays '' until chosen
    ALTER TABLE art ADD COLUMN remote_url TEXT;
    ALTER TABLE art ADD COLUMN thumb_url TEXT;
    CREATE UNIQUE INDEX art_remote ON art(game_id, slot, remote_url) WHERE remote_url IS NOT NULL;

    -- which entry in each source a game is matched to
    CREATE TABLE game_links (
        game_id   INTEGER NOT NULL REFERENCES games(id) ON DELETE CASCADE,
        source    TEXT    NOT NULL,
        source_id TEXT    NOT NULL,
        title     TEXT,
        score     REAL,
        payload   TEXT,
        linked_at TEXT    NOT NULL,
        PRIMARY KEY (game_id, source)
    );
    "#,
    // 4: downloaded games with their own files, controller maps, watched folders
    r#"
    -- Flashpoint id when the game came with extra files (flashpoint/<id>/)
    ALTER TABLE games ADD COLUMN content_id TEXT;
    -- the address the main SWF originally lived at, so its own loads resolve
    ALTER TABLE games ADD COLUMN launch_url TEXT;
    -- JSON {ruffle button name: key code}; NULL = the default layout
    ALTER TABLE games ADD COLUMN gamepad_map TEXT;

    -- every SWF a watched folder has offered, so deleting a game sticks
    CREATE TABLE watched_seen (
        sha256  TEXT PRIMARY KEY,
        seen_at TEXT NOT NULL
    );
    -- files already hashed, so rescans only read new or changed ones
    CREATE TABLE watched_files (
        path   TEXT PRIMARY KEY,
        size   INTEGER NOT NULL,
        mtime  INTEGER NOT NULL,
        sha256 TEXT NOT NULL
    );
    "#,
];

/// Schema version this build writes (backups from newer builds are refused).
pub fn schema_version() -> i64 {
    MIGRATIONS.len() as i64
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    init(&conn)?;
    Ok(conn)
}

pub fn init(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current.max(0) as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

/// ISO-8601 UTC timestamp, the format every *_at column uses.
pub fn now(conn: &Connection) -> rusqlite::Result<String> {
    conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |r| r.get(0))
}

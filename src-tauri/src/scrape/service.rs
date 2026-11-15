//! Ties scrapers to the library: searching every source for a game,
//! linking the chosen matches, pulling metadata and art candidates,
//! picking sensible defaults, and downloading/resizing images.
//!
//! The DB lock is only held for short reads and writes, never across a
//! network call, so the UI stays responsive while scraping.

use super::{all, by_id, confident, rank, Details, Link, Match, Query, RemoteArt, Settings, SLOTS};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::library;
use crate::AppState;
use image::{GenericImageView, ImageFormat};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::io::Cursor;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchGroup {
    pub source: String,
    pub label: String,
    /// Why the source can't run (missing key), if so.
    pub unavailable: Option<String>,
    pub error: Option<String>,
    pub matches: Vec<Match>,
    /// The id this game is currently linked to in this source.
    pub linked_id: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeReport {
    pub game_id: i64,
    pub title: String,
    pub linked: Vec<String>,
    /// Some source had results but none was a clear match.
    pub needs_review: bool,
    pub errors: Vec<String>,
}

/// Preferred source order when picking a default image for each slot.
fn slot_preference(slot: &str) -> &'static [&'static str] {
    match slot {
        "cover" => &["user", "steamgriddb", "igdb", "archive"],
        "wide" => &["user", "steamgriddb", "flashpoint"],
        "hero" => &["user", "steamgriddb", "igdb"],
        "logo" => &["user", "steamgriddb"],
        "icon" => &["user", "steamgriddb"],
        "screenshot" => &["user", "player", "flashpoint", "igdb"],
        _ => &[],
    }
}

/// Largest size kept for each slot; bigger downloads get scaled down.
fn slot_max(slot: &str) -> (u32, u32) {
    match slot {
        "cover" => (600, 900),
        "wide" => (920, 460),
        "hero" => (1920, 1080),
        "logo" => (1280, 640),
        "icon" => (256, 256),
        _ => (1280, 1024),
    }
}

pub fn settings(st: &AppState) -> AppResult<Settings> {
    library::get_settings(&st.db())
}

fn query_for(st: &AppState, game_id: i64, title: Option<String>) -> AppResult<Query> {
    let (t, file): (String, String) = st.db().query_row(
        "SELECT title, original_name FROM games WHERE id = ?1",
        [game_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(Query {
        title: title.filter(|t| !t.trim().is_empty()).unwrap_or(t),
        file_name: file,
    })
}

fn linked_ids(conn: &Connection, game_id: i64) -> AppResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT source, source_id FROM game_links WHERE game_id = ?1")?;
    let rows = stmt
        .query_map([game_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Searches every source. Sources that fail or lack a key report why
/// instead of failing the whole search.
pub fn search(st: &AppState, game_id: i64, title: Option<String>) -> AppResult<Vec<SearchGroup>> {
    let q = query_for(st, game_id, title)?;
    let settings = settings(st)?;
    let linked = linked_ids(&st.db(), game_id)?;
    let mut out = Vec::new();
    for s in all() {
        let mut g = SearchGroup {
            source: s.id().into(),
            label: s.label().into(),
            unavailable: s.unavailable(&settings),
            error: None,
            matches: vec![],
            linked_id: linked
                .iter()
                .find(|(src, _)| src == s.id())
                .map(|(_, id)| id.clone()),
        };
        if g.unavailable.is_none() {
            match s.search(&st.http, &settings, &q) {
                Ok(m) => g.matches = rank(&q, m).into_iter().take(8).collect(),
                Err(e) => g.error = Some(e.0),
            }
        }
        out.push(g);
    }
    Ok(out)
}

/// Links the chosen matches (one per source) and refreshes metadata + art.
pub fn apply(st: &AppState, game_id: i64, picks: Vec<Match>) -> AppResult<ScrapeReport> {
    {
        let conn = st.db();
        let tx = conn.unchecked_transaction()?;
        let now = db::now(&tx)?;
        for m in &picks {
            if by_id(&m.source).is_none() {
                return Err(AppError(format!("unknown source {}", m.source)));
            }
            let payload = m.payload.as_ref().map(|p| p.to_string());
            tx.execute(
                "INSERT INTO game_links (game_id, source, source_id, title, score, payload, linked_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(game_id, source) DO UPDATE SET source_id = excluded.source_id, title = excluded.title,
                    score = excluded.score, payload = excluded.payload, linked_at = excluded.linked_at",
                params![game_id, m.source, m.id, m.title, m.score, payload, now],
            )?;
            // Art from a previous, different match in this source is stale.
            tx.execute(
                "DELETE FROM art WHERE game_id = ?1 AND source = ?2 AND remote_url IS NOT NULL AND selected = 0",
                params![game_id, m.source],
            )?;
        }
        tx.commit()?;
    }
    refresh(st, game_id)
}

/// Removes a source link (the user said "none of these").
pub fn unlink(st: &AppState, game_id: i64, source: &str) -> AppResult<()> {
    let conn = st.db();
    conn.execute(
        "DELETE FROM game_links WHERE game_id = ?1 AND source = ?2",
        params![game_id, source],
    )?;
    conn.execute(
        "DELETE FROM art WHERE game_id = ?1 AND source = ?2 AND remote_url IS NOT NULL AND selected = 0",
        params![game_id, source],
    )?;
    Ok(())
}

/// Re-reads details and art candidates from every linked source.
pub fn refresh(st: &AppState, game_id: i64) -> AppResult<ScrapeReport> {
    let settings = settings(st)?;
    let links: Vec<(String, String, Option<String>)> = {
        let conn = st.db();
        let mut stmt =
            conn.prepare("SELECT source, source_id, payload FROM game_links WHERE game_id = ?1")?;
        let rows = stmt
            .query_map([game_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    let mut report = ScrapeReport {
        game_id,
        ..Default::default()
    };

    // Metadata: best-priority source that has any.
    let mut scrapers: Vec<_> = links
        .iter()
        .filter_map(|(src, id, payload)| {
            let s = by_id(src)?;
            let link = Link {
                id: id.clone(),
                payload: payload.as_deref().and_then(|p| serde_json::from_str(p).ok()),
            };
            Some((s, link))
        })
        .collect();
    scrapers.sort_by_key(|(s, _)| s.metadata_priority());

    let mut details: Option<Details> = None;
    let mut art: Vec<(String, RemoteArt)> = Vec::new();
    for (s, link) in &scrapers {
        report.linked.push(s.id().to_string());
        if details.is_none() {
            match s.details(&st.http, &settings, link) {
                Ok(d) => details = d,
                Err(e) => report.errors.push(format!("{}: {}", s.label(), e.0)),
            }
        }
        match s.art(&st.http, &settings, link) {
            Ok(a) => art.extend(a.into_iter().map(|x| (s.id().to_string(), x))),
            Err(e) => report.errors.push(format!("{}: {}", s.label(), e.0)),
        }
    }

    {
        let conn = st.db();
        let tx = conn.unchecked_transaction()?;
        if let Some(d) = &details {
            apply_details(&tx, game_id, d)?;
        }
        for (source, a) in &art {
            tx.execute(
                "INSERT OR IGNORE INTO art (game_id, slot, source, file, width, height, selected, remote_url, thumb_url)
                 VALUES (?1, ?2, ?3, '', ?4, ?5, 0, ?6, ?7)",
                params![game_id, a.slot, source, a.width, a.height, a.url, a.thumb],
            )?;
        }
        tx.execute(
            "UPDATE games SET scraped_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [game_id],
        )?;
        tx.commit()?;
    }

    // Default picks for empty slots. Downloads happen outside the lock.
    for slot in SLOTS {
        if let Err(e) = autoselect_slot(st, game_id, slot) {
            report.errors.push(format!("{slot}: {}", e.0));
        }
    }
    report.title = st
        .db()
        .query_row("SELECT title FROM games WHERE id = ?1", [game_id], |r| r.get(0))?;
    Ok(report)
}

fn apply_details(conn: &Connection, game_id: i64, d: &Details) -> AppResult<()> {
    let locked: bool = conn.query_row("SELECT title_locked FROM games WHERE id = ?1", [game_id], |r| {
        r.get(0)
    })?;
    if let (Some(t), false) = (&d.title, locked) {
        conn.execute(
            "UPDATE games SET title = ?2, sort_title = ?3 WHERE id = ?1",
            params![game_id, t, crate::title::normalize(t)],
        )?;
    }
    conn.execute(
        "UPDATE games SET
            developer = COALESCE(?2, developer), publisher = COALESCE(?3, publisher),
            year = COALESCE(?4, year), description = COALESCE(?5, description),
            series = COALESCE(?6, series), release_date = COALESCE(?7, release_date)
         WHERE id = ?1",
        params![
            game_id,
            d.developer,
            d.publisher,
            d.year,
            d.description,
            d.series,
            d.release_date
        ],
    )?;
    if !d.tags.is_empty() {
        conn.execute("DELETE FROM game_tags WHERE game_id = ?1", [game_id])?;
        for t in &d.tags {
            conn.execute(
                "INSERT OR IGNORE INTO game_tags (game_id, tag) VALUES (?1, ?2)",
                params![game_id, t.to_lowercase()],
            )?;
        }
    }
    Ok(())
}

/// If nothing is selected in this slot, picks the best candidate and
/// downloads it.
fn autoselect_slot(st: &AppState, game_id: i64, slot: &str) -> AppResult<()> {
    let pick: Option<i64> = {
        let conn = st.db();
        let has: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM art WHERE game_id = ?1 AND slot = ?2 AND selected = 1)",
            params![game_id, slot],
            |r| r.get(0),
        )?;
        if has {
            return Ok(());
        }
        let mut found = None;
        for src in slot_preference(slot) {
            found = conn
                .query_row(
                    "SELECT id FROM art WHERE game_id = ?1 AND slot = ?2 AND source = ?3 ORDER BY id LIMIT 1",
                    params![game_id, slot, src],
                    |r| r.get(0),
                )
                .optional()?;
            if found.is_some() {
                break;
            }
        }
        found
    };
    if let Some(art_id) = pick {
        select(st, game_id, Some(art_id), Some(slot))?;
    }
    Ok(())
}

/// Selects one image for its slot (downloading it first if it's still
/// remote), or clears the slot when `art_id` is None.
pub fn select(st: &AppState, game_id: i64, art_id: Option<i64>, slot: Option<&str>) -> AppResult<()> {
    let Some(art_id) = art_id else {
        let slot = slot.ok_or_else(|| AppError("say which slot to clear".into()))?;
        st.db().execute(
            "UPDATE art SET selected = 0 WHERE game_id = ?1 AND slot = ?2",
            params![game_id, slot],
        )?;
        return Ok(());
    };
    let (slot, source, file, remote): (String, String, String, Option<String>) = st
        .db()
        .query_row(
            "SELECT slot, source, file, remote_url FROM art WHERE id = ?1 AND game_id = ?2",
            params![art_id, game_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| AppError(format!("art {art_id} doesn't belong to game {game_id}")))?;

    if file.is_empty() {
        let url = remote.ok_or_else(|| AppError("this image has no file and no address".into()))?;
        let src: &'static str = by_id(&source).map(|s| s.id()).unwrap_or("download");
        let bytes = st.http.image(src, &url)?;
        let (rel, w, h) = store_image(st, game_id, &slot, &format!("{source}-{art_id}"), &bytes)?;
        st.db().execute(
            "UPDATE art SET file = ?2, width = ?3, height = ?4 WHERE id = ?1",
            params![art_id, rel, w, h],
        )?;
    }
    let conn = st.db();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE art SET selected = 0 WHERE game_id = ?1 AND slot = ?2",
        params![game_id, slot],
    )?;
    tx.execute("UPDATE art SET selected = 1 WHERE id = ?1", [art_id])?;
    tx.commit()?;
    Ok(())
}

/// Adds the user's own image file to a slot and selects it.
pub fn upload(st: &AppState, game_id: i64, slot: &str, path: &str) -> AppResult<()> {
    if !SLOTS.contains(&slot) {
        return Err(AppError(format!("unknown art slot {slot}")));
    }
    let meta = std::fs::metadata(path)?;
    if meta.len() > 40 * 1024 * 1024 {
        return Err("that image is over 40 MB".into());
    }
    let bytes = std::fs::read(path)?;
    let stamp: String = st
        .db()
        .query_row("SELECT strftime('%Y%m%d%H%M%f', 'now')", [], |r| r.get(0))?;
    let (rel, w, h) = store_image(
        st,
        game_id,
        slot,
        &format!("user-{}", stamp.replace('.', "")),
        &bytes,
    )?;
    let conn = st.db();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE art SET selected = 0 WHERE game_id = ?1 AND slot = ?2",
        params![game_id, slot],
    )?;
    tx.execute(
        "INSERT INTO art (game_id, slot, source, file, width, height, selected) VALUES (?1, ?2, 'user', ?3, ?4, ?5, 1)",
        params![game_id, slot, rel, w, h],
    )?;
    tx.commit()?;
    Ok(())
}

/// Validates, scales down if needed, and writes an image into the game's
/// art folder. Returns (relative path, width, height).
fn store_image(
    st: &AppState,
    game_id: i64,
    slot: &str,
    name: &str,
    bytes: &[u8],
) -> AppResult<(String, u32, u32)> {
    let img =
        image::load_from_memory(bytes).map_err(|e| AppError(format!("not an image I can read: {e}")))?;
    let (w, h) = img.dimensions();
    let (mw, mh) = slot_max(slot);
    // Many PNGs carry an alpha channel that's fully opaque; those can be JPEG.
    let has_alpha = img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < 255);
    let dir = st.paths.art.join(game_id.to_string());
    std::fs::create_dir_all(&dir)?;
    let ext = if has_alpha { "png" } else { "jpg" };
    let file = dir.join(format!("{slot}-{name}.{ext}"));

    let fits = w <= mw && h <= mh;
    let orig_fmt = image::guess_format(bytes).ok();
    let same_fmt = matches!(
        (orig_fmt, has_alpha),
        (Some(ImageFormat::Png), true) | (Some(ImageFormat::Jpeg), false)
    );
    let (out_w, out_h) = if fits && same_fmt {
        std::fs::write(&file, bytes)?;
        (w, h)
    } else {
        let scaled = if fits {
            img
        } else {
            img.resize(mw, mh, image::imageops::FilterType::Lanczos3)
        };
        let mut buf = Vec::new();
        if has_alpha {
            scaled.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        } else {
            let rgb = image::DynamicImage::ImageRgb8(scaled.to_rgb8());
            let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 88);
            enc.encode_image(&rgb)
        }
        .map_err(|e| AppError(format!("couldn't save image: {e}")))?;
        std::fs::write(&file, &buf)?;
        scaled.dimensions()
    };
    Ok((st.paths.rel(&file), out_w, out_h))
}

/// Search every source and link whatever is a clear match. Used after
/// import and by the bulk "find missing art" action.
pub fn auto(st: &AppState, game_id: i64) -> AppResult<ScrapeReport> {
    auto_except(st, game_id, &[])
}

/// `auto`, leaving out sources the game is already linked to by other means.
pub fn auto_except(st: &AppState, game_id: i64, skip: &[&str]) -> AppResult<ScrapeReport> {
    let q = query_for(st, game_id, None)?;
    let settings = settings(st)?;
    let mut picks = Vec::new();
    let mut needs_review = false;
    let mut errors = Vec::new();
    for s in all() {
        if s.unavailable(&settings).is_some() || skip.contains(&s.id()) {
            continue;
        }
        match s.search(&st.http, &settings, &q) {
            Ok(m) => {
                let ranked = rank(&q, m);
                match confident(&ranked) {
                    Some(best) => picks.push(best.clone()),
                    // Flashpoint is the authority for Flash games; if it has
                    // candidates but none is certain, the user should pick.
                    None if !ranked.is_empty() && s.id() == "flashpoint" => needs_review = true,
                    None => {}
                }
            }
            Err(e) => errors.push(format!("{}: {}", s.label(), e.0)),
        }
    }
    let mut report = if picks.is_empty() {
        st.db().execute(
            "UPDATE games SET scraped_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
            [game_id],
        )?;
        ScrapeReport {
            game_id,
            title: q.title.clone(),
            ..Default::default()
        }
    } else {
        apply(st, game_id, picks)?
    };
    report.needs_review = needs_review || report.linked.is_empty();
    report.errors.extend(errors);
    Ok(report)
}

/// Games worth scraping in bulk: never scraped, or no real art selected.
pub fn missing(conn: &Connection) -> AppResult<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT g.id FROM games g WHERE g.scraped_at IS NULL OR NOT EXISTS (
            SELECT 1 FROM art a WHERE a.game_id = g.id AND a.selected = 1 AND a.file <> ''
              AND a.slot IN ('cover', 'wide', 'hero', 'logo'))
         ORDER BY g.sort_title",
    )?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;
    use crate::scrape::Http;
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;

    fn state(tag: &str) -> (AppState, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("fl-scrape-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = AppPaths::new(dir.clone()).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        conn.execute(
            "INSERT INTO games (title, sort_title, swf_file, original_name, sha256, file_size, swf_version,
                compression, width, height, frame_rate, frame_count, as_version, color, added_at)
             VALUES ('Papas Freezeria', 'papas freezeria', 'swfs/x.swf', 'papasfreezeria_v2.swf', 'abc', 3, 15,
                'zlib', 640, 480, 30, 1, 3, '#000000', 'now')",
            [],
        )
        .unwrap();
        let http = Http::new(dir.join("cache")).unwrap();
        (
            AppState {
                db: Mutex::new(conn),
                paths,
                http,
                scrape_cancel: AtomicBool::new(false),
                download_cancel: AtomicBool::new(false),
                presence: crate::discord::Presence::off(),
            },
            dir,
        )
    }

    fn png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(w, h, |x, y| {
            image::Rgba([x as u8, y as u8, 90, if alpha { 128 } else { 255 }])
        });
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn stores_and_scales_images_per_slot() {
        let (st, dir) = state("store");
        // too big for an icon, has alpha -> scaled PNG
        let (rel, w, h) = store_image(&st, 1, "icon", "t", &png(1024, 512, true)).unwrap();
        assert!(rel.ends_with(".png"));
        assert_eq!((w, h), (256, 128));
        // opaque PNG becomes JPEG, small enough to keep its size
        let (rel, w, h) = store_image(&st, 1, "cover", "t2", &png(300, 450, false)).unwrap();
        assert!(rel.ends_with(".jpg"));
        assert_eq!((w, h), (300, 450));
        assert!(store_image(&st, 1, "cover", "bad", b"nope").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn upload_selects_and_metadata_respects_title_lock() {
        let (st, dir) = state("upload");
        let f = dir.join("mine.png");
        std::fs::write(&f, png(600, 900, false)).unwrap();
        upload(&st, 1, "cover", &f.to_string_lossy()).unwrap();
        let g = library::get_game(&st.db(), &st.paths, 1).unwrap();
        assert!(g.art.cover.is_some());
        assert!(upload(&st, 1, "banner", &f.to_string_lossy()).is_err());

        let d = Details {
            title: Some("Papa's Freezeria".into()),
            developer: Some("Flipline Studios".into()),
            tags: vec!["Cooking".into()],
            ..Default::default()
        };
        apply_details(&st.db(), 1, &d).unwrap();
        let g = library::get_game(&st.db(), &st.paths, 1).unwrap();
        assert_eq!(g.title, "Papa's Freezeria");
        assert_eq!(g.developer.as_deref(), Some("Flipline Studios"));
        assert_eq!(g.tags, vec!["cooking"]);

        // after a manual rename the scraper leaves the title alone
        library::update_game(
            &st.db(),
            &st.paths,
            1,
            library::GamePatch {
                title: Some("Freezy".into()),
                ..Default::default()
            },
        )
        .unwrap();
        apply_details(&st.db(), 1, &d).unwrap();
        assert_eq!(library::get_game(&st.db(), &st.paths, 1).unwrap().title, "Freezy");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Hits the real Flashpoint and Internet Archive APIs.
    /// `cargo test live_auto_scrape -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_auto_scrape() {
        let (st, dir) = state("live");
        let groups = search(&st, 1, None).unwrap();
        for g in &groups {
            println!("{}: {:?} {:?}", g.label, g.unavailable, g.error);
            for m in g.matches.iter().take(3) {
                println!(
                    "   {:.3} {} ({:?}) file_match={}",
                    m.score, m.title, m.year, m.file_match
                );
            }
        }
        let report = auto(&st, 1).unwrap();
        println!("{report:?}");
        let g = library::get_game(&st.db(), &st.paths, 1).unwrap();
        println!("{} / {:?} / {:?} / {:?}", g.title, g.developer, g.year, g.art);
        assert_eq!(g.title, "Papa's Freezeria");
        assert!(g.art.wide.is_some() && g.art.screenshot.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}

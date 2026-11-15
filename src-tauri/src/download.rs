//! Browsing and downloading games from Flashpoint Archive.
//!
//! Search goes through the same public JSON API the scraper uses. Game
//! files come from Flashpoint's own download servers, the ones its
//! launcher (Flashpoint Infinity) uses:
//!   - most games are a zip at gib-roms/Games/<id>-<dateAdded ms>.zip with
//!     every file the game needs under content/<host>/<path>, exactly as
//!     they were on the original sites;
//!   - older "legacy" entries have no zip; their files sit in one big
//!     htdocs tree and get fetched one by one, as the game asks for them.
//!
//! Files land in flashpoint/<id>/<host>/<path>. The main SWF also goes
//! through the normal import (hash, header, embedded art), so a
//! downloaded game is a regular library game that happens to carry its
//! own files. While it plays, the fpcontent:// protocol (see `content`)
//! serves those files back to Ruffle in place of the original sites.

use crate::error::{AppError, AppResult};
use crate::import::{self, NewGame};
use crate::library::{self, Game};
use crate::scrape::{self, service};
use crate::AppState;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const API: &str = "https://flashpointarchive.org/api/search";
const IMAGES: &str = "https://infinity.unstable.life/images/";
const ZIP_MIRRORS: &[&str] = &[
    "https://download.flashpointarchive.org/gib-roms/Games/",
    "https://download.unstable.life/gib-roms/Games/",
];
pub const LEGACY_MIRRORS: &[&str] = &[
    "https://infinity.flashpointarchive.org/Flashpoint/Legacy/htdocs/",
    "https://infinity.unstable.life/Flashpoint/Legacy/htdocs/",
];
/// Marks a content folder whose missing files may still be fetched.
pub const LEGACY_MARKER: &str = ".legacy";

const MAX_ZIP_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 50_000;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(4 * 60 * 60);

// ---------------------------------------------------------------- browse

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseGame {
    pub id: String,
    pub title: String,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub series: Option<String>,
    pub release_date: Option<String>,
    pub year: Option<i64>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub play_mode: Option<String>,
    pub language: Option<String>,
    pub logo: Option<String>,
    pub logo_thumb: Option<String>,
    pub screenshot: Option<String>,
    /// Zip size in bytes; None for legacy entries (fetched file by file).
    pub size: Option<i64>,
    /// "swf" when it launches straight into a SWF, "page" when it starts
    /// from a web page (those may not run here).
    pub launch_kind: &'static str,
    /// The library game it's already linked to.
    pub library_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePage {
    pub total: i64,
    pub pages: i64,
    pub page: i64,
    pub games: Vec<BrowseGame>,
}

fn s(v: &Value, k: &str) -> Option<String> {
    scrape::non_empty(v.get(k).and_then(Value::as_str))
}

/// Flashpoint ids are UUIDs. Anything else never touches the disk.
pub fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Sort keys the site understands.
fn sort_key(sort: &str) -> (&'static str, &'static str) {
    match sort {
        "new" => ("dateAdded", "desc"),
        "newest" => ("releaseDate", "desc"),
        "oldest" => ("releaseDate", "asc"),
        "updated" => ("dateModified", "desc"),
        _ => ("title", "asc"),
    }
}

/// Fewer than this many hits after filtering, and the next page is
/// fetched too (games and animations are mixed together upstream).
const MIN_PER_PAGE: usize = 24;
const MAX_PAGES_PER_CALL: u32 = 6;

/// `kind` is "games", "animations" or "all". Flashpoint's API can't filter by
/// library, so filtering happens here, reading ahead a few pages when needed;
/// the returned `page` is the last one read.
pub fn browse(st: &AppState, query: &str, page: u32, sort: &str, kind: &str) -> AppResult<BrowsePage> {
    // The site's smart search: platform:Flash keeps it to what Ruffle runs.
    let q = format!("platform:Flash {}", query.trim());
    let (sort, dir) = sort_key(sort);
    let want = match kind {
        "games" => Some("arcade"),
        "animations" => Some("theatre"),
        _ => None,
    };
    let first = page.max(1);
    let mut out = BrowsePage {
        total: 0,
        pages: 0,
        page: 1,
        games: Vec::new(),
    };
    for page in first..first + MAX_PAGES_PER_CALL {
        let p = page.to_string();
        let json = st.http.json("flashpoint", |c| {
            c.get(API).query(&[
                ("query", q.as_str()),
                ("page", p.as_str()),
                ("sort", sort),
                ("dir", dir),
            ])
        })?;
        out.total = json.get("total").and_then(Value::as_i64).unwrap_or(0);
        out.pages = json.get("pages").and_then(Value::as_i64).unwrap_or(0);
        out.page = page as i64;
        let games = json
            .get("games")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        out.games.extend(
            games
                .iter()
                .filter(|g| want.is_none_or(|w| s(g, "library").as_deref() == Some(w)))
                .filter_map(to_browse),
        );
        if out.games.len() >= MIN_PER_PAGE || out.page >= out.pages {
            break;
        }
    }
    let ids: Vec<String> = out.games.iter().map(|g| g.id.clone()).collect();
    let linked = linked_games(&st.db(), &ids)?;
    for g in &mut out.games {
        g.library_id = linked.iter().find(|(id, _)| *id == g.id).map(|(_, gid)| *gid);
    }
    Ok(out)
}

fn to_browse(g: &Value) -> Option<BrowseGame> {
    let id = s(g, "id").filter(|i| valid_id(i))?;
    let release = s(g, "releaseDate");
    let data = latest_game_data(g);
    let launch = launch_command(g).unwrap_or_default();
    Some(BrowseGame {
        title: s(g, "title")?,
        developer: s(g, "developer"),
        publisher: s(g, "publisher"),
        series: s(g, "series"),
        year: release.as_deref().and_then(scrape::year_of),
        release_date: release,
        description: s(g, "originalDescription"),
        tags: g
            .get("tags")
            .and_then(Value::as_array)
            .map(|t| t.iter().filter_map(|x| scrape::non_empty(x.as_str())).collect())
            .unwrap_or_default(),
        play_mode: s(g, "playMode"),
        language: s(g, "language"),
        logo: s(g, "logoPath").map(|p| format!("{IMAGES}{p}")),
        logo_thumb: s(g, "logoPath").map(|p| format!("{IMAGES}{p}?type=jpg")),
        screenshot: s(g, "screenshotPath").map(|p| format!("{IMAGES}{p}")),
        size: data.and_then(|d| d.get("size").and_then(Value::as_i64)),
        launch_kind: if is_swf(&launch) { "swf" } else { "page" },
        library_id: None,
        id,
    })
}

fn linked_games(conn: &Connection, ids: &[String]) -> AppResult<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT source_id, game_id FROM game_links WHERE source = 'flashpoint' AND source_id = ?1",
    )?;
    let mut out = Vec::new();
    for id in ids {
        if let Some(gid) = stmt.query_row([id], |r| r.get::<_, i64>(1)).optional()? {
            out.push((id.clone(), gid));
        }
    }
    Ok(out)
}

/// Newest entry in gameData (an entry can be re-curated over the years).
fn latest_game_data(g: &Value) -> Option<&Value> {
    g.get("gameData")?
        .as_array()?
        .iter()
        .max_by(|a, b| s(a, "dateAdded").cmp(&s(b, "dateAdded")))
}

fn launch_command(g: &Value) -> Option<String> {
    latest_game_data(g)
        .and_then(|d| s(d, "launchCommand"))
        .or_else(|| s(g, "legacyLaunchCommand"))
}

fn strip_query(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

fn is_swf(url: &str) -> bool {
    strip_query(url).to_ascii_lowercase().ends_with(".swf")
}

/// "2023-12-21T21:42:00.421826Z" -> 1703194920421 (what the zip name uses).
pub fn date_to_millis(date: &str) -> Option<i64> {
    let (d, t) = date.trim_end_matches('Z').split_once('T')?;
    let mut dp = d.split('-').map(|x| x.parse::<i64>().ok());
    let (y, m, day) = (dp.next()??, dp.next()??, dp.next()??);
    let (hms, frac) = t.split_once('.').unwrap_or((t, "0"));
    let mut tp = hms.split(':').map(|x| x.parse::<i64>().ok());
    let (hh, mm, ss) = (tp.next()??, tp.next()??, tp.next()??);
    let ms: i64 = format!("{:0<3}", frac.chars().take(3).collect::<String>())
        .parse()
        .ok()?;
    // Days from civil (Howard Hinnant's algorithm).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hh) * 60 + mm) * 60_000 + ss * 1000 + ms)
}

// ---------------------------------------------------------------- download

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub id: String,
    /// download | unpack | import | art | done
    pub phase: &'static str,
    pub received: u64,
    pub total: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloaded {
    pub game: Game,
    /// The game was already in the library (same SWF, or this entry).
    pub already: bool,
}

fn fetch_entry(st: &AppState, id: &str) -> AppResult<Value> {
    let q = format!("id:{id}");
    let json = st
        .http
        .json("flashpoint", |c| c.get(API).query(&[("query", q.as_str())]))?;
    json.get("games")
        .and_then(Value::as_array)
        .and_then(|g| g.iter().find(|g| s(g, "id").as_deref() == Some(id)))
        .cloned()
        .ok_or_else(|| AppError("Flashpoint doesn't have that game anymore".into()))
}

pub fn download(
    st: &AppState,
    id: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(DownloadProgress),
) -> AppResult<Downloaded> {
    if !valid_id(id) {
        return Err(AppError("that isn't a Flashpoint game id".into()));
    }
    // Bound first: a guard in the if-let would stay locked for the whole block.
    let linked = linked_games(&st.db(), &[id.to_string()])?.pop();
    if let Some((_, gid)) = linked {
        let game = library::get_game(&st.db(), &st.paths, gid)?;
        return Ok(Downloaded { game, already: true });
    }
    let mut report = |phase, received, total| {
        progress(DownloadProgress {
            id: id.to_string(),
            phase,
            received,
            total,
        })
    };
    report("download", 0, 0);

    let entry = fetch_entry(st, id)?;
    let title = s(&entry, "title").unwrap_or_default();
    let launch = launch_command(&entry)
        .ok_or_else(|| AppError("Flashpoint has no launch file for this game".into()))?;

    let staging = st.paths.downloads.join(format!("{id}.dir"));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    let result = (|| -> AppResult<(PathBuf, String)> {
        if let Some(data) = latest_game_data(&entry) {
            let ms = s(data, "dateAdded")
                .as_deref()
                .and_then(date_to_millis)
                .ok_or_else(|| AppError("Flashpoint's entry has no date for its game files".into()))?;
            let zip = st.paths.downloads.join(format!("{id}.zip"));
            let urls: Vec<String> = ZIP_MIRRORS.iter().map(|m| format!("{m}{id}-{ms}.zip")).collect();
            let expected = Expected {
                size: data.get("size").and_then(Value::as_u64),
                sha256: s(data, "sha256"),
            };
            fetch_to_file(st, &urls, &zip, &expected, cancel, &mut |r, t| {
                report("download", r, t)
            })?;
            report("unpack", 0, 0);
            let unpacked = unpack(&zip, &staging, cancel);
            let _ = std::fs::remove_file(&zip);
            unpacked?;
        } else {
            std::fs::write(staging.join(LEGACY_MARKER), b"")?;
            fetch_legacy(st, &staging, &launch, cancel, &mut |r, t| {
                report("download", r, t)
            })?;
        }
        find_main(&staging, &launch)
    })();
    let (main_rel, main_url) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };

    // Swap the staged folder in only once everything worked.
    let dest = st.paths.content.join(id);
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&staging, &dest).or_else(|_| copy_dir(&staging, &dest).map(|_| ()))?;
    let _ = std::fs::remove_dir_all(&staging);

    report("import", 0, 0);
    let bytes = std::fs::read(dest.join(&main_rel))?;
    let hash = import::sha256_of(&bytes);
    let file_name = strip_query(&main_url)
        .rsplit('/')
        .next()
        .unwrap_or("game.swf")
        .to_string();
    let (game_id, already) = {
        let conn = st.db();
        let (game_id, already) = match library::find_by_hash(&conn, &hash)? {
            Some((gid, _)) => (gid, true),
            None => {
                let g = import::import_bytes(
                    &conn,
                    &st.paths,
                    NewGame {
                        bytes: &bytes,
                        hash: &hash,
                        file_name: &file_name,
                        original_path: Some(&main_url),
                        title: &title,
                        source: "archive",
                    },
                )?;
                (g.id, false)
            }
        };
        // Same SWF as another downloaded game: the newer files win.
        let old: Option<String> = conn
            .query_row("SELECT content_id FROM games WHERE id = ?1", [game_id], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        conn.execute(
            "UPDATE games SET content_id = ?2, launch_url = ?3 WHERE id = ?1",
            params![game_id, id, main_url],
        )?;
        if let Some(old) = old.filter(|o| o != id && valid_id(o)) {
            let _ = std::fs::remove_dir_all(st.paths.content.join(old));
        }
        (game_id, already)
    };

    // Link it to its Flashpoint entry (that's where the title, description
    // and art come from), then let the other sources fill in the rest.
    report("art", 0, 0);
    if let Some(m) = scrape::flashpoint_match(&entry, &file_name) {
        if let Err(e) = service::apply(st, game_id, vec![m]) {
            eprintln!("flashpoint link for {game_id} failed: {e}");
        }
    }
    if let Err(e) = service::auto_except(st, game_id, &["flashpoint"]) {
        eprintln!("extra art for {game_id} failed: {e}");
    }
    report("done", 1, 1);
    let game = library::get_game(&st.db(), &st.paths, game_id)?;
    Ok(Downloaded { game, already })
}

struct Expected {
    size: Option<u64>,
    sha256: Option<String>,
}

/// Streams the first mirror that answers into `dest`, checking size and
/// hash on the way.
fn fetch_to_file(
    st: &AppState,
    urls: &[String],
    dest: &Path,
    expected: &Expected,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> AppResult<()> {
    let mut last_err = AppError("no download server answered".into());
    for url in urls {
        match fetch_one(st, url, dest, expected, cancel, progress) {
            Ok(()) => return Ok(()),
            Err(e) if cancel.load(Ordering::SeqCst) => return Err(e),
            Err(e) => last_err = e,
        }
    }
    let _ = std::fs::remove_file(dest);
    Err(last_err)
}

fn fetch_one(
    st: &AppState,
    url: &str,
    dest: &Path,
    expected: &Expected,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> AppResult<()> {
    let mut resp = st
        .http
        .send("flashpoint-files", |c| c.get(url).timeout(DOWNLOAD_TIMEOUT))?;
    let total = resp.content_length().or(expected.size).unwrap_or(0);
    if total > MAX_ZIP_BYTES {
        return Err(AppError("that download is too big".into()));
    }
    let mut file = File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut received: u64 = 0;
    let mut last = Instant::now();
    progress(0, total);
    loop {
        if cancel.load(Ordering::SeqCst) {
            drop(file);
            let _ = std::fs::remove_file(dest);
            return Err(AppError("download cancelled".into()));
        }
        let n = resp
            .read(&mut buf)
            .map_err(|e| AppError(format!("download interrupted: {e}")))?;
        if n == 0 {
            break;
        }
        received += n as u64;
        if received > MAX_ZIP_BYTES {
            return Err(AppError("that download is too big".into()));
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])?;
        if last.elapsed() > Duration::from_millis(120) {
            progress(received, total);
            last = Instant::now();
        }
    }
    file.flush()?;
    progress(received, total.max(received));
    if let Some(want) = &expected.sha256 {
        let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !got.eq_ignore_ascii_case(want) {
            return Err(AppError(
                "the download didn't match Flashpoint's checksum, so it was thrown away".into(),
            ));
        }
    }
    Ok(())
}

/// Unpacks content/<host>/<path> entries into `dest` (other entries, like
/// the zip's content.json, aren't needed).
fn unpack(zip: &Path, dest: &Path, cancel: &AtomicBool) -> AppResult<()> {
    let mut archive = zip::ZipArchive::new(File::open(zip)?)
        .map_err(|e| AppError(format!("the download isn't a valid zip: {e}")))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(AppError("the download has too many files".into()));
    }
    let mut unpacked: u64 = 0;
    for i in 0..archive.len() {
        if cancel.load(Ordering::SeqCst) {
            return Err(AppError("download cancelled".into()));
        }
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError(format!("couldn't read the zip: {e}")))?;
        // enclosed_name refuses absolute paths and ../ tricks.
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Ok(rel) = name.strip_prefix("content") else {
            continue;
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        unpacked += entry.size();
        if unpacked > MAX_UNPACKED_BYTES {
            return Err(AppError(
                "the game unpacks to more than 4 GB, which is too much".into(),
            ));
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = File::create(&out)?;
        std::io::copy(&mut entry, &mut f)
            .map_err(|e| AppError(format!("couldn't unpack {}: {e}", rel.display())))?;
    }
    Ok(())
}

/// URL -> path under a content folder: http://host/a/b.swf?x -> host/a/b.swf.
/// Percent-escapes are decoded; anything that could leave the folder is refused.
pub fn url_to_rel(url: &str) -> Option<PathBuf> {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let rest = strip_query(rest);
    let mut out = PathBuf::new();
    let mut parts = rest.split('/').peekable();
    let host = parts.next()?.split(':').next()?.to_ascii_lowercase();
    if host.is_empty() || host == ".." || host == "." {
        return None;
    }
    out.push(host);
    let mut last_empty = true;
    for p in parts {
        last_empty = p.is_empty();
        if p.is_empty() {
            continue;
        }
        let seg = percent_decode(p);
        if seg == "." || seg == ".." || seg.contains(['/', '\\', '\0']) || seg.contains(':') {
            return None;
        }
        out.push(seg);
    }
    if last_empty {
        out.push("index.html");
    }
    // Belt and braces: only plain components.
    out.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then_some(out)
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolves `href` the way a browser would, against `base`.
fn join_url(base: &str, href: &str) -> String {
    let href = href.trim();
    if href.contains("://") {
        return href.to_string();
    }
    let base = strip_query(base);
    let (scheme, rest) = base.split_once("://").unwrap_or(("http", base));
    let host = rest.split('/').next().unwrap_or(rest);
    if let Some(abs) = href.strip_prefix("//") {
        return format!("{scheme}://{abs}");
    }
    if href.starts_with('/') {
        return format!("{scheme}://{host}{href}");
    }
    let dir = match base.rfind('/') {
        Some(i) if i > scheme.len() + 2 => &base[..=i],
        _ => return format!("{scheme}://{host}/{href}"),
    };
    let mut segs: Vec<&str> = dir[scheme.len() + 3..].trim_end_matches('/').split('/').collect();
    for part in href.split('/') {
        match part {
            "." | "" => {}
            ".." if segs.len() > 1 => {
                segs.pop();
            }
            ".." => {}
            p => segs.push(p),
        }
    }
    format!("{scheme}://{}", segs.join("/"))
}

/// .swf references in a web page (embed src, object data, swfobject calls...).
fn swf_refs(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find(".swf") {
        let end = from + i + 4;
        let start = lower[..from + i]
            .rfind(['"', '\'', '=', '(', ' ', '\n', '\t', '>', ','])
            .map(|p| p + 1)
            .unwrap_or(0);
        let r = html[start..end].trim();
        if !r.is_empty() && !out.iter().any(|x| x == r) {
            out.push(r.to_string());
        }
        from = end;
    }
    out
}

/// Picks the SWF to import: the launch file itself, the first SWF its page
/// embeds, or failing both the biggest SWF in the folder.
fn find_main(dir: &Path, launch: &str) -> AppResult<(PathBuf, String)> {
    let launch_rel = url_to_rel(launch);
    if let Some(rel) = &launch_rel {
        let p = dir.join(rel);
        if is_swf(launch) && p.is_file() {
            return Ok((rel.clone(), launch.to_string()));
        }
        if p.is_file() {
            if let Ok(html) = std::fs::read(&p) {
                for r in swf_refs(&String::from_utf8_lossy(&html)) {
                    let url = join_url(launch, &r);
                    if let Some(rel) = url_to_rel(&url).filter(|rel| dir.join(rel).is_file()) {
                        return Ok((rel, url));
                    }
                }
            }
        }
    }
    let mut best: Option<(u64, PathBuf)> = None;
    walk_files(dir, dir, &mut |rel, size| {
        if rel.extension().is_some_and(|e| e.eq_ignore_ascii_case("swf"))
            && best.as_ref().is_none_or(|b| size > b.0)
        {
            best = Some((size, rel.to_path_buf()));
        }
    });
    match best {
        Some((_, rel)) => {
            let url = format!("http://{}", rel.to_string_lossy().replace('\\', "/"));
            Ok((rel, url))
        }
        None => Err(AppError(
            "this game doesn't include a SWF file, so it isn't something the launcher can play".into(),
        )),
    }
}

fn walk_files(root: &Path, dir: &Path, f: &mut dyn FnMut(&Path, u64)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_files(root, &p, f);
        } else if let (Ok(rel), Ok(meta)) = (p.strip_prefix(root), e.metadata()) {
            f(rel, meta.len());
        }
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<u64> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from)?.flatten() {
        let src = e.path();
        let dst = to.join(e.file_name());
        if src.is_dir() {
            n += copy_dir(&src, &dst)?;
        } else {
            n += std::fs::copy(&src, &dst)?;
        }
    }
    Ok(n)
}

/// Legacy entries: fetch the launch file, and if that's a page, the SWF it
/// embeds. Everything else comes on demand while playing.
fn fetch_legacy(
    st: &AppState,
    dir: &Path,
    launch: &str,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> AppResult<()> {
    let rel = url_to_rel(launch).ok_or_else(|| AppError("Flashpoint's launch address looks wrong".into()))?;
    fetch_legacy_file(st, dir, &rel, cancel, progress)?;
    if !is_swf(launch) {
        let html = std::fs::read(dir.join(&rel))?;
        for r in swf_refs(&String::from_utf8_lossy(&html)) {
            let url = join_url(launch, &r);
            if let Some(rel) = url_to_rel(&url) {
                if fetch_legacy_file(st, dir, &rel, cancel, progress).is_ok() {
                    break;
                }
            }
        }
    }
    Ok(())
}

pub fn legacy_urls(rel: &Path) -> Vec<String> {
    let path: Vec<String> = rel
        .components()
        .map(|c| encode_segment(&c.as_os_str().to_string_lossy()))
        .collect();
    LEGACY_MIRRORS
        .iter()
        .map(|m| format!("{m}{}", path.join("/")))
        .collect()
}

fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=@".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn fetch_legacy_file(
    st: &AppState,
    dir: &Path,
    rel: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> AppResult<()> {
    let dest = dir.join(rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let part = dest.with_extension(format!(
        "{}part",
        dest.extension()
            .map(|e| format!("{}.", e.to_string_lossy()))
            .unwrap_or_default()
    ));
    let expected = Expected {
        size: None,
        sha256: None,
    };
    fetch_to_file(st, &legacy_urls(rel), &part, &expected, cancel, progress)?;
    std::fs::rename(&part, &dest)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn zip_names_use_millisecond_timestamps() {
        assert_eq!(date_to_millis("2023-12-21T21:42:00.421826Z"), Some(1703194920421));
        assert_eq!(date_to_millis("2023-10-24T04:34:54.642806Z"), Some(1698122094642));
        assert_eq!(date_to_millis("2018-06-04T06:25:36.316Z"), Some(1528093536316));
        assert_eq!(date_to_millis("2020-01-01T00:00:00Z"), Some(1577836800000));
        assert_eq!(date_to_millis("garbage"), None);
    }

    #[test]
    fn urls_map_into_the_content_folder() {
        assert_eq!(
            url_to_rel("http://i.flipline.com/gamefiles/papasfreezeria/papasfreezeria_v2.swf"),
            Some(PathBuf::from(
                "i.flipline.com/gamefiles/papasfreezeria/papasfreezeria_v2.swf"
            ))
        );
        assert_eq!(
            url_to_rel("http://chat.kongregate.com/gamez/0014/Golem.swf?kongregate_api_path=http://x/y.swf"),
            Some(PathBuf::from("chat.kongregate.com/gamez/0014/Golem.swf"))
        );
        assert_eq!(
            url_to_rel("http://Example.com:8080/a%20b/c.swf"),
            Some(PathBuf::from("example.com/a b/c.swf"))
        );
        assert_eq!(
            url_to_rel("http://example.com/"),
            Some(PathBuf::from("example.com/index.html"))
        );
        assert_eq!(url_to_rel("http://example.com/a/../../etc/passwd"), None);
        assert_eq!(url_to_rel("http://example.com/a/%2e%2e/x"), None);
        assert_eq!(url_to_rel("http://example.com/a%2Fb"), None);
        assert_eq!(url_to_rel("http://../x"), None);
    }

    #[test]
    fn page_references_resolve_like_a_browser() {
        let html = r#"<embed src="game.swf" width=550><script>swfobject.embedSWF('/files/loader.swf', 'x')</script>
            <param name=movie value=../up.SWF>"#;
        assert_eq!(swf_refs(html), vec!["game.swf", "/files/loader.swf", "../up.SWF"]);
        let base = "http://site.com/games/a/index.html?x=1";
        assert_eq!(join_url(base, "game.swf"), "http://site.com/games/a/game.swf");
        assert_eq!(
            join_url(base, "/files/loader.swf"),
            "http://site.com/files/loader.swf"
        );
        assert_eq!(join_url(base, "../up.SWF"), "http://site.com/games/up.SWF");
        assert_eq!(join_url(base, "//cdn.com/z.swf"), "http://cdn.com/z.swf");
        assert_eq!(join_url(base, "https://o.com/q.swf"), "https://o.com/q.swf");
    }

    #[test]
    fn picks_the_main_swf() {
        let dir = std::env::temp_dir().join(format!("fl-dl-main-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("site.com/games")).unwrap();
        std::fs::create_dir_all(dir.join("ads.com")).unwrap();
        std::fs::write(dir.join("site.com/games/play.html"), "<embed src='real.swf'>").unwrap();
        std::fs::write(dir.join("site.com/games/real.swf"), b"FWS").unwrap();
        std::fs::write(dir.join("ads.com/big.swf"), vec![0u8; 100]).unwrap();

        let (rel, url) = find_main(&dir, "http://site.com/games/real.swf?a=1").unwrap();
        assert_eq!(rel, PathBuf::from("site.com/games/real.swf"));
        assert_eq!(url, "http://site.com/games/real.swf?a=1");
        let (rel, url) = find_main(&dir, "http://site.com/games/play.html").unwrap();
        assert_eq!(rel, PathBuf::from("site.com/games/real.swf"));
        assert_eq!(url, "http://site.com/games/real.swf");
        let (rel, _) = find_main(&dir, "http://gone.com/x.swf").unwrap();
        assert_eq!(
            rel,
            PathBuf::from("ads.com/big.swf"),
            "falls back to the biggest SWF"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_urls_are_encoded() {
        let u = legacy_urls(Path::new("files.crazygames.com/a b/fancy.swf"));
        assert_eq!(
            u[0],
            "https://infinity.flashpointarchive.org/Flashpoint/Legacy/htdocs/files.crazygames.com/a%20b/fancy.swf"
        );
    }

    #[test]
    fn ids_are_checked() {
        assert!(valid_id("92ba2d91-e041-2bd4-49ea-21758df711ff"));
        assert!(!valid_id("../../../etc"));
        assert!(!valid_id("92ba2d91-e041-2bd4-49ea-21758df711f/"));
    }

    /// Real download of a small game. Run with:
    /// cargo test live_download -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_download() {
        let dir = std::env::temp_dir().join(format!("fl-dl-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = crate::paths::AppPaths::new(dir.clone()).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        let st = AppState {
            db: std::sync::Mutex::new(conn),
            http: scrape::Http::new(dir.join("cache/remote")).unwrap(),
            paths,
            scrape_cancel: AtomicBool::new(false),
            download_cancel: AtomicBool::new(false),
            presence: crate::discord::Presence::off(),
        };
        let page = browse(&st, "qwop", 1, "title", "games").unwrap();
        let qwop = page
            .games
            .iter()
            .find(|g| g.title == "QWOP")
            .expect("QWOP in results");
        println!("{} {:?} {:?}", qwop.id, qwop.size, qwop.launch_kind);
        let cancel = AtomicBool::new(false);
        let d = download(&st, &qwop.id, &cancel, |p| {
            if p.phase != "download" {
                println!("{p:?}");
            }
        })
        .unwrap();
        println!(
            "{} {:?} {:?} links={:?}",
            d.game.title, d.game.content_id, d.game.launch_url, d.game.links
        );
        assert_eq!(d.game.title, "QWOP");
        assert_eq!(d.game.source, "archive");
        assert!(d.game.links.contains(&"flashpoint".to_string()));
        let rel = url_to_rel(d.game.launch_url.as_deref().unwrap()).unwrap();
        assert!(st.paths.content.join(&qwop.id).join(rel).is_file());

        // A legacy entry (no zip): the SWF comes straight from htdocs.
        let page = browse(&st, "fancy snowboarding", 1, "title", "all").unwrap();
        let fs = page
            .games
            .iter()
            .find(|g| g.title == "Fancy Snowboarding")
            .expect("legacy game");
        assert_eq!(fs.size, None);
        let d = download(&st, &fs.id, &cancel, |_| {}).unwrap();
        println!("{} {:?} {}", d.game.title, d.game.launch_url, d.game.file_size);
        assert!(st.paths.content.join(&fs.id).join(LEGACY_MARKER).exists());
        assert!(d.game.file_size > 1_000_000);

        // Downloading the same entry again finds it in the library.
        assert!(download(&st, &qwop.id, &cancel, |_| {}).unwrap().already);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

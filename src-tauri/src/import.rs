//! Import is two steps so the user can check things in between:
//!   1. inspect: read each file, hash it, parse the header, guess a title,
//!      flag duplicates, pull embedded images into a pending folder.
//!   2. import: copy the SWF into the library, insert the row, move its
//!      images over.

use crate::db;
use crate::error::{AppError, AppResult};
use crate::library::{self, Game};
use crate::paths::AppPaths;
use crate::swf::{extract_images, interest, Swf, SwfInfo};
use crate::title;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Most images we keep per game from inside the SWF.
const MAX_EMBEDDED: usize = 6;
/// Folder scans stop here so a mis-dropped drive doesn't run forever.
const MAX_FILES: usize = 2000;
const MAX_DEPTH: usize = 10;
const MAX_SWF_BYTES: u64 = 600 * 1024 * 1024;

/// Metro accent colors used for generated art until real art is scraped.
const TILE_COLORS: &[&str] = &[
    "#a4c400", "#60a917", "#008a00", "#00aba9", "#1ba1e2", "#0050ef", "#6a00ff", "#aa00ff", "#f472d0",
    "#d80073", "#a20025", "#e51400", "#fa6800", "#f0a30a", "#825a2c", "#6d8764", "#647687", "#76608a",
];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub path: String,
    pub file_name: String,
    pub file_size: u64,
    pub sha256: Option<String>,
    pub title: String,
    pub info: Option<SwfInfo>,
    pub error: Option<String>,
    /// Set when this exact file is already in the library.
    pub duplicate_of: Option<DuplicateRef>,
    /// Set when the same file appears earlier in this batch.
    pub duplicate_in_batch: bool,
    pub preview: Option<String>,
    pub image_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateRef {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItem {
    pub path: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailure {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub imported: Vec<Game>,
    pub failed: Vec<ImportFailure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
    pub current: String,
}

/// Turns a mix of files and folders into a flat list of .swf paths.
pub fn expand_paths(inputs: &[String]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for p in inputs {
        let path = PathBuf::from(p);
        if path.is_dir() {
            walk(&path, 0, &mut out, &mut seen);
        } else if seen.insert(path.clone()) {
            out.push(path);
        }
        if out.len() >= MAX_FILES {
            break;
        }
    }
    out.truncate(MAX_FILES);
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, depth + 1, out, seen);
        } else if is_swf_name(&p) && seen.insert(p.clone()) {
            out.push(p);
        }
        if out.len() >= MAX_FILES {
            return;
        }
    }
}

fn is_swf_name(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case("swf"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn read_swf_file(path: &Path) -> AppResult<Vec<u8>> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_SWF_BYTES {
        return Err(AppError(format!(
            "file is {} MB, which is too big for a SWF",
            meta.len() / 1024 / 1024
        )));
    }
    Ok(std::fs::read(path)?)
}

pub fn inspect(
    conn: &Connection,
    paths: &AppPaths,
    inputs: &[String],
    mut progress: impl FnMut(Progress),
) -> AppResult<Vec<ImportCandidate>> {
    let files = expand_paths(inputs);
    let total = files.len();
    let mut seen_hashes = HashSet::new();
    let mut out = Vec::with_capacity(total);

    for (i, path) in files.into_iter().enumerate() {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        progress(Progress {
            phase: "inspect",
            done: i,
            total,
            current: file_name.clone(),
        });

        let mut c = ImportCandidate {
            path: path.to_string_lossy().into_owned(),
            file_name: file_name.clone(),
            file_size: 0,
            sha256: None,
            title: title::title_from_filename(&file_name),
            info: None,
            error: None,
            duplicate_of: None,
            duplicate_in_batch: false,
            preview: None,
            image_count: 0,
        };

        if !is_swf_name(&path) {
            c.error = Some("not a .swf file".into());
            out.push(c);
            continue;
        }

        let bytes = match read_swf_file(&path) {
            Ok(b) => b,
            Err(e) => {
                c.error = Some(e.0);
                out.push(c);
                continue;
            }
        };
        c.file_size = bytes.len() as u64;
        let hash = sha256_hex(&bytes);
        c.duplicate_in_batch = !seen_hashes.insert(hash.clone());
        c.duplicate_of = library::find_by_hash(conn, &hash)?.map(|(id, title)| DuplicateRef { id, title });
        c.sha256 = Some(hash.clone());

        match Swf::parse(&bytes) {
            Ok(swf) => {
                c.title = title::guess_title(&file_name, swf.info.meta_title.as_deref());
                if c.duplicate_of.is_none() && !c.duplicate_in_batch {
                    let (count, preview) = stash_images(paths, &hash, &swf)?;
                    c.image_count = count;
                    c.preview = preview;
                }
                c.info = Some(swf.info);
            }
            Err(e) => c.error = Some(e.to_string()),
        }
        out.push(c);
    }
    progress(Progress {
        phase: "inspect",
        done: total,
        total,
        current: String::new(),
    });
    Ok(out)
}

/// Writes the biggest embedded images to pending/<hash>/ and returns how
/// many there were plus the best one for a preview.
fn stash_images(paths: &AppPaths, hash: &str, swf: &Swf) -> AppResult<(usize, Option<String>)> {
    let dir = paths.pending.join(hash);
    let _ = std::fs::remove_dir_all(&dir);
    let images = extract_images(swf, MAX_EMBEDDED);
    if images.is_empty() {
        return Ok((0, None));
    }
    std::fs::create_dir_all(&dir)?;
    for (i, img) in images.iter().enumerate() {
        std::fs::write(dir.join(format!("{i}.png")), &img.png)?;
    }
    let best = best_index(images.iter().map(|i| (i.width, i.height, i.interest)));
    let preview = best.map(|b| dir.join(format!("{b}.png")).to_string_lossy().into_owned());
    Ok((images.len(), preview))
}

/// Below this, a plain generated tile looks better than the image.
const MIN_INTEREST: f64 = 0.12;

/// Picks the image to show on generated art. Size counts (square-rooted so
/// a huge dull backdrop can't win on area alone), sensible aspect ratios
/// beat long strips like scrolling backgrounds and sprite sheets, and dull
/// images (mostly black loading screens) lose to colorful ones or to
/// nothing at all.
fn best_index(images: impl Iterator<Item = (u32, u32, f64)>) -> Option<usize> {
    images
        .enumerate()
        .filter(|(_, (_, _, interest))| *interest >= MIN_INTEREST)
        .map(|(i, (w, h, interest))| {
            let aspect = w as f64 / h.max(1) as f64;
            let fit = if (0.6..=2.5).contains(&aspect) { 1.0 } else { 0.35 };
            (i, (w as f64 * h as f64).sqrt() * fit * interest)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

pub fn import(
    conn: &Connection,
    paths: &AppPaths,
    items: &[ImportItem],
    mut progress: impl FnMut(Progress),
) -> AppResult<ImportSummary> {
    let total = items.len();
    let mut summary = ImportSummary {
        imported: Vec::new(),
        failed: Vec::new(),
    };

    for (i, item) in items.iter().enumerate() {
        progress(Progress {
            phase: "import",
            done: i,
            total,
            current: item.title.clone(),
        });
        match import_one(conn, paths, item) {
            Ok(game) => summary.imported.push(game),
            Err(e) => summary.failed.push(ImportFailure {
                path: item.path.clone(),
                error: e.0,
            }),
        }
    }
    progress(Progress {
        phase: "import",
        done: total,
        total,
        current: String::new(),
    });
    Ok(summary)
}

fn import_one(conn: &Connection, paths: &AppPaths, item: &ImportItem) -> AppResult<Game> {
    let src = PathBuf::from(&item.path);
    let bytes = read_swf_file(&src)?;
    let hash = sha256_hex(&bytes);
    if let Some((_, existing)) = library::find_by_hash(conn, &hash)? {
        return Err(AppError(format!("already in your library as \"{existing}\"")));
    }
    let file_name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    import_bytes(
        conn,
        paths,
        NewGame {
            bytes: &bytes,
            hash: &hash,
            file_name: &file_name,
            original_path: Some(&item.path),
            title: &item.title,
            source: "uploaded",
        },
    )
}

/// A SWF on its way into the library, from a file or a download.
pub struct NewGame<'a> {
    pub bytes: &'a [u8],
    pub hash: &'a str,
    pub file_name: &'a str,
    pub original_path: Option<&'a str>,
    /// Empty means guess from the file.
    pub title: &'a str,
    /// uploaded | archive
    pub source: &'a str,
}

pub fn sha256_of(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}

/// Copies the SWF in, inserts its row with the default player settings,
/// brings back saves from an earlier copy, and stores embedded art.
pub fn import_bytes(conn: &Connection, paths: &AppPaths, g: NewGame) -> AppResult<Game> {
    let NewGame {
        bytes,
        hash,
        file_name,
        original_path,
        title,
        source,
    } = g;
    let hash = hash.to_string();
    let swf = Swf::parse(bytes)?;
    let info = &swf.info;

    let title = Some(title.trim())
        .filter(|t| !t.is_empty())
        .map(String::from)
        .unwrap_or_else(|| title::guess_title(file_name, info.meta_title.as_deref()));

    // Copy first, insert second: a failed copy leaves no orphan row.
    let dest = paths.swfs.join(format!("{hash}.swf"));
    std::fs::write(&dest, bytes)?;

    let color = TILE_COLORS[u8::from_str_radix(&hash[..2], 16).unwrap_or(0) as usize % TILE_COLORS.len()];
    let compression = serde_json::to_value(info.compression)?
        .as_str()
        .unwrap_or("none")
        .to_string();
    let tx = conn.unchecked_transaction()?;
    let now = db::now(&tx)?;
    tx.execute(
        "INSERT INTO games (title, sort_title, source, swf_file, original_name, original_path, sha256, file_size,
            swf_version, compression, width, height, frame_rate, frame_count, as_version, uses_network,
            background, color, added_at)
         VALUES (?1, ?2, ?19, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            title,
            title::normalize(&title),
            paths.rel(&dest),
            file_name,
            original_path,
            hash,
            bytes.len() as i64,
            info.version,
            compression,
            info.width,
            info.height,
            info.frame_rate,
            info.frame_count,
            info.as_version,
            info.uses_network,
            info.background,
            color,
            now,
            source,
        ],
    )?;
    let id = tx.last_insert_rowid();
    apply_player_defaults(&tx, Some(id))?;

    // Saves from an earlier copy of this exact file come back.
    tx.execute(
        "INSERT INTO saves (game_id, data, updated_at)
         SELECT ?1, data, deleted_at FROM save_backups WHERE sha256 = ?2",
        params![id, hash],
    )?;
    tx.execute("DELETE FROM save_backups WHERE sha256 = ?1", [&hash])?;

    if let Err(e) = store_art(&tx, paths, id, &hash, &swf) {
        // Art is a nice-to-have; never fail an import over it.
        eprintln!("art for game {id} skipped: {e}");
    }
    tx.commit()?;
    library::get_game(conn, paths, id)
}

/// Copies the default player settings from Settings onto one game, or
/// onto every game when `id` is None. Bad values in settings are ignored.
pub fn apply_player_defaults(conn: &Connection, id: Option<i64>) -> AppResult<()> {
    let get = |k: &str| -> AppResult<Option<String>> {
        Ok(conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [k], |r| r.get(0))
            .optional()?)
    };
    let scale = get("defaultScaleMode")?.filter(|v| ["fit", "stretch", "original"].contains(&v.as_str()));
    let quality = get("defaultQuality")?.filter(|v| ["low", "medium", "high", "best"].contains(&v.as_str()));
    let network = get("defaultAllowNetwork")?.map(|v| v == "true");
    let filter = if id.is_some() {
        "WHERE id = ?4"
    } else {
        "WHERE ?4 IS NULL"
    };
    conn.execute(
        &format!(
            "UPDATE games SET scale_mode = COALESCE(?1, scale_mode), quality = COALESCE(?2, quality),
                allow_network = COALESCE(?3, allow_network) {filter}"
        ),
        params![scale, quality, network, id],
    )?;
    Ok(())
}

fn store_art(conn: &Connection, paths: &AppPaths, id: i64, hash: &str, swf: &Swf) -> AppResult<()> {
    let pending = paths.pending.join(hash);
    let art_dir = paths.art.join(id.to_string());
    std::fs::create_dir_all(&art_dir)?;

    // Reuse what inspection already pulled out; re-extract if it's gone.
    let mut files: Vec<PathBuf> = Vec::new();
    if pending.is_dir() {
        let mut found: Vec<PathBuf> = std::fs::read_dir(&pending)?.flatten().map(|e| e.path()).collect();
        found.sort_by_key(|p| {
            p.file_stem()
                .and_then(|s| s.to_str()?.parse::<u32>().ok())
                .unwrap_or(u32::MAX)
        });
        for (i, f) in found.iter().enumerate() {
            let dest = art_dir.join(format!("embedded-{i}.png"));
            if std::fs::rename(f, &dest).is_err() {
                std::fs::copy(f, &dest)?;
            }
            files.push(dest);
        }
        let _ = std::fs::remove_dir_all(&pending);
    } else {
        for (i, img) in extract_images(swf, MAX_EMBEDDED).iter().enumerate() {
            let dest = art_dir.join(format!("embedded-{i}.png"));
            std::fs::write(&dest, &img.png)?;
            files.push(dest);
        }
    }

    let dims: Vec<(u32, u32, f64)> = files
        .iter()
        .map(|f| match image::open(f) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                (rgba.width(), rgba.height(), interest(&rgba))
            }
            Err(_) => (0, 0, 0.0),
        })
        .collect();
    let best = best_index(dims.iter().copied());
    for (i, (f, (w, h, _))) in files.iter().zip(&dims).enumerate() {
        conn.execute(
            "INSERT INTO art (game_id, slot, source, file, width, height, selected)
             VALUES (?1, 'embedded', 'swf', ?2, ?3, ?4, ?5)",
            params![id, paths.rel(f), w, h, Some(i) == best],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::swf::tests::{body, fws};

    fn setup() -> (Connection, AppPaths, tempdir::Dir) {
        let dir = tempdir::Dir::new();
        let paths = AppPaths::new(dir.0.join("data")).unwrap();
        let conn = Connection::open_in_memory().unwrap();
        db::init(&conn).unwrap();
        (conn, paths, dir)
    }

    /// Tiny self-cleaning temp dir so tests don't need another crate.
    mod tempdir {
        pub struct Dir(pub std::path::PathBuf);
        impl Dir {
            pub fn new() -> Self {
                use std::sync::atomic::{AtomicU32, Ordering};
                static N: AtomicU32 = AtomicU32::new(0);
                let p = std::env::temp_dir().join(format!(
                    "flashlauncher-test-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::SeqCst)
                ));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    fn write_swf(dir: &Path, name: &str, bg: u8) -> String {
        let b = body(550, 400, (24, 0), 10, &[(9, vec![bg, 0, 0])]);
        let p = dir.join(name);
        std::fs::write(&p, fws(8, &b)).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn inspect_then_import_round_trip() {
        let (conn, paths, dir) = setup();
        let src = dir.0.join("incoming");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        let a = write_swf(&src, "paper_plane_derby_v2.swf", 1);
        write_swf(&src.join("nested"), "gravity-goo.swf", 2);
        write_swf(&src.join("nested"), "copy_of_derby.swf", 1); // same bytes as a
        std::fs::write(src.join("readme.txt"), "hi").unwrap();

        let cands = inspect(&conn, &paths, &[src.to_string_lossy().into_owned()], |_| {}).unwrap();
        assert_eq!(cands.len(), 3, "folder scan picks up only .swf files");
        let derby = cands.iter().find(|c| c.path == a).unwrap();
        assert_eq!(derby.title, "Paper Plane Derby");
        assert_eq!(derby.info.as_ref().unwrap().width, 550.0);
        assert_eq!(cands.iter().filter(|c| c.duplicate_in_batch).count(), 1);

        let items: Vec<ImportItem> = cands
            .iter()
            .filter(|c| !c.duplicate_in_batch)
            .map(|c| ImportItem {
                path: c.path.clone(),
                title: c.title.clone(),
            })
            .collect();
        let summary = import(&conn, &paths, &items, |_| {}).unwrap();
        assert_eq!(summary.imported.len(), 2);
        assert!(summary.failed.is_empty());
        let g = &summary.imported[0];
        assert!(Path::new(&g.swf_path).exists());
        assert_eq!(g.as_version, 2);
        assert_eq!(g.compression, "none");

        // Re-inspecting now flags the library duplicate; importing again fails.
        let again = inspect(&conn, &paths, std::slice::from_ref(&a), |_| {}).unwrap();
        assert!(again[0].duplicate_of.is_some());
        let res = import(
            &conn,
            &paths,
            &[ImportItem {
                path: a,
                title: "x".into(),
            }],
            |_| {},
        )
        .unwrap();
        assert_eq!(res.failed.len(), 1);

        // Rename, favorite, delete.
        let id = g.id;
        let patched = library::update_game(
            &conn,
            &paths,
            id,
            library::GamePatch {
                title: Some("The Derby".into()),
                favorite: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(patched.title, "The Derby");
        assert!(patched.favorite);
        library::delete_game(&conn, &paths, id).unwrap();
        assert!(!Path::new(&patched.swf_path).exists());
        assert_eq!(library::list_games(&conn, &paths).unwrap().len(), 1);
    }

    #[test]
    fn bad_files_report_errors_instead_of_failing() {
        let (conn, paths, dir) = setup();
        let p = dir.0.join("broken.swf");
        std::fs::write(&p, b"definitely not flash").unwrap();
        let c = inspect(&conn, &paths, &[p.to_string_lossy().into_owned()], |_| {}).unwrap();
        assert_eq!(c.len(), 1);
        assert!(c[0].error.as_deref().unwrap().contains("not a SWF"));
    }

    #[test]
    fn best_image_prefers_sane_aspect() {
        assert_eq!(
            best_index([(2000, 100, 0.8), (400, 300, 0.8)].into_iter()),
            Some(1)
        );
        assert_eq!(best_index(std::iter::empty()), None);
    }

    #[test]
    fn best_image_skips_dull_ones() {
        // A big dark loading screen loses to a smaller colorful image...
        assert_eq!(
            best_index([(800, 600, 0.05), (200, 150, 0.7)].into_iter()),
            Some(1)
        );
        // ...and with nothing worth showing, there's no pick at all.
        assert_eq!(best_index([(800, 600, 0.05)].into_iter()), None);
    }
}

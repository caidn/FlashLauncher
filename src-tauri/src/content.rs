//! The fpcontent:// protocol: serves a downloaded game's own files to
//! Ruffle. The player rewrites every http(s) address the game asks for to
//! fpcontent://localhost/<flashpoint id>/<host>/<path> (on Windows,
//! http://fpcontent.localhost/...), so a game that loads
//! http://site.com/levels.xml gets flashpoint/<id>/site.com/levels.xml.
//!
//! Legacy Flashpoint entries come without a zip; their missing files are
//! fetched from Flashpoint's htdocs the first time the game asks, then kept.

use crate::download::{self, LEGACY_MARKER};
use crate::AppState;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use tauri::http::{header, Response, StatusCode};

pub const SCHEME: &str = "fpcontent";

pub fn respond(st: &AppState, path: &str) -> Response<Vec<u8>> {
    match resolve(st, path) {
        Some(file) => match std::fs::read(&file) {
            Ok(bytes) => build(StatusCode::OK, mime(&file), bytes),
            Err(_) => not_found(),
        },
        None => not_found(),
    }
}

/// "/<id>/<host>/<path>" -> a file on disk, fetching legacy files on demand.
fn resolve(st: &AppState, path: &str) -> Option<PathBuf> {
    let path = path.trim_start_matches('/');
    let (id, rest) = path.split_once('/')?;
    if !download::valid_id(id) {
        return None;
    }
    let root = st.paths.content.join(id);
    if !root.is_dir() {
        return None;
    }
    // Paths arrive still percent-encoded; url_to_rel decodes and checks them.
    let rel = download::url_to_rel(&format!("http://{rest}"))?;
    let file = root.join(&rel);
    if file.is_file() {
        return Some(file);
    }
    if root.join(LEGACY_MARKER).exists() {
        let cancel = AtomicBool::new(false);
        if download::fetch_legacy_file(st, &root, &rel, &cancel, &mut |_, _| {}).is_ok() {
            return Some(file);
        }
    }
    None
}

fn not_found() -> Response<Vec<u8>> {
    build(StatusCode::NOT_FOUND, "text/plain", Vec::new())
}

fn build(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, mime)
        // Ruffle fetches these from the app's own origin.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::CACHE_CONTROL, "no-store")
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn mime(p: &std::path::Path) -> &'static str {
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "swf" | "spl" => "application/x-shockwave-flash",
        "xml" => "text/xml",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "js" => "text/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "mp3" => "audio/mpeg",
        "flv" => "video/x-flv",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

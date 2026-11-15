//! Tauri commands. Each one hops onto a blocking thread so file I/O,
//! hashing and parsing never stall the UI.

use crate::download;
use crate::error::{AppError, AppResult};
use crate::import::{self, ImportCandidate, ImportItem, ImportSummary, Progress};
use crate::library::{self, Collection, Game, GamePatch};
use crate::player;
use crate::scrape::{
    self,
    service::{ScrapeReport, SearchGroup},
    Match,
};
use crate::AppState;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub const IMPORT_PROGRESS_EVENT: &str = "import://progress";

async fn blocking<T, F>(state: &State<'_, Arc<AppState>>, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> AppResult<T> + Send + 'static,
{
    let st = Arc::clone(state);
    tauri::async_runtime::spawn_blocking(move || f(&st))
        .await
        .map_err(|e| AppError(format!("background task failed: {e}")))?
}

#[tauri::command]
pub async fn library_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<Game>> {
    blocking(&state, |s| library::list_games(&s.db(), &s.paths)).await
}

#[tauri::command]
pub async fn library_inspect(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
) -> AppResult<Vec<ImportCandidate>> {
    blocking(&state, move |s| {
        import::inspect(&s.db(), &s.paths, &paths, |p: Progress| {
            let _ = app.emit(IMPORT_PROGRESS_EVENT, p);
        })
    })
    .await
}

#[tauri::command]
pub async fn library_import(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    items: Vec<ImportItem>,
) -> AppResult<ImportSummary> {
    blocking(&state, move |s| {
        import::import(&s.db(), &s.paths, &items, |p: Progress| {
            let _ = app.emit(IMPORT_PROGRESS_EVENT, p);
        })
    })
    .await
}

#[tauri::command]
pub async fn game_update(state: State<'_, Arc<AppState>>, id: i64, patch: GamePatch) -> AppResult<Game> {
    blocking(&state, move |s| {
        library::update_game(&s.db(), &s.paths, id, patch)
    })
    .await
}

#[tauri::command]
pub async fn game_delete(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<()> {
    blocking(&state, move |s| library::delete_game(&s.db(), &s.paths, id)).await
}

#[tauri::command]
pub async fn collections_list(state: State<'_, Arc<AppState>>) -> AppResult<Vec<Collection>> {
    blocking(&state, |s| library::list_collections(&s.db())).await
}

#[tauri::command]
pub async fn settings_get(state: State<'_, Arc<AppState>>) -> AppResult<HashMap<String, String>> {
    blocking(&state, |s| library::get_settings(&s.db())).await
}

#[tauri::command]
pub async fn settings_set(state: State<'_, Arc<AppState>>, key: String, value: String) -> AppResult<()> {
    blocking(&state, move |s| library::set_setting(&s.db(), &key, &value)).await
}

#[tauri::command]
pub fn app_data_dir(state: State<'_, Arc<AppState>>) -> String {
    state.paths.root.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------- player

/// The SWF as raw bytes. Returned as a binary IPC response so a 30 MB game
/// doesn't get turned into a 100 MB JSON array on the way.
#[tauri::command]
pub async fn game_read_swf(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<tauri::ipc::Response> {
    let bytes = blocking(&state, move |s| player::read_swf(&s.db(), &s.paths, id)).await?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn saves_get(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<Option<String>> {
    blocking(&state, move |s| player::get_saves(&s.db(), id)).await
}

#[tauri::command]
pub async fn saves_put(state: State<'_, Arc<AppState>>, id: i64, data: String) -> AppResult<()> {
    blocking(&state, move |s| player::put_saves(&s.db(), id, &data)).await
}

#[tauri::command]
pub async fn session_start(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<i64> {
    blocking(&state, move |s| {
        let session = player::start_session(&s.db(), id)?;
        crate::discord::playing(s, id);
        Ok(session)
    })
    .await
}

/// The game stopped: take it off your Discord profile.
#[tauri::command]
pub async fn discord_clear(state: State<'_, Arc<AppState>>) -> AppResult<()> {
    state.presence.clear();
    Ok(())
}

#[tauri::command]
pub async fn session_update(state: State<'_, Arc<AppState>>, session: i64, seconds: i64) -> AppResult<()> {
    blocking(&state, move |s| player::update_session(&s.db(), session, seconds)).await
}

#[tauri::command]
pub async fn game_get(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<Game> {
    blocking(&state, move |s| library::get_game(&s.db(), &s.paths, id)).await
}

#[tauri::command]
pub async fn art_list(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<Vec<player::ArtItem>> {
    blocking(&state, move |s| player::list_art(&s.db(), &s.paths, id)).await
}

/// Selects an image for its slot (downloading it if needed), or clears
/// `slot` when `art` is null.
#[tauri::command]
pub async fn art_select(
    state: State<'_, Arc<AppState>>,
    id: i64,
    art: Option<i64>,
    slot: Option<String>,
) -> AppResult<()> {
    blocking(&state, move |s| {
        scrape::service::select(s, id, art, slot.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn art_delete(state: State<'_, Arc<AppState>>, id: i64, art: i64) -> AppResult<()> {
    blocking(&state, move |s| player::delete_art(&s.db(), &s.paths, id, art)).await
}

#[tauri::command]
pub async fn art_upload(
    state: State<'_, Arc<AppState>>,
    id: i64,
    slot: String,
    path: String,
) -> AppResult<()> {
    blocking(&state, move |s| scrape::service::upload(s, id, &slot, &path)).await
}

/// Takes the PNG as the raw request body; the game id rides in a header.
#[tauri::command]
pub async fn art_add_screenshot(
    state: State<'_, Arc<AppState>>,
    request: tauri::ipc::Request<'_>,
) -> AppResult<player::ArtItem> {
    let tauri::ipc::InvokeBody::Raw(png) = request.body() else {
        return Err("expected the screenshot as raw PNG bytes".into());
    };
    let png = png.clone();
    let id: i64 = request
        .headers()
        .get("x-game-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| AppError("missing x-game-id header".into()))?;
    blocking(&state, move |s| {
        player::add_screenshot(&s.db(), &s.paths, id, &png)
    })
    .await
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectorExit {
    pub game_id: i64,
    pub seconds: i64,
    pub error: Option<String>,
}

pub const PROJECTOR_EXIT_EVENT: &str = "projector://exit";

/// Starts the user's standalone Flash Player with the game. Returns once
/// it's running; a `projector://exit` event follows when it closes, and the
/// play time is recorded then.
#[tauri::command]
pub async fn projector_launch(app: AppHandle, state: State<'_, Arc<AppState>>, id: i64) -> AppResult<()> {
    let (exe, swf, session) = blocking(&state, move |s| {
        let (exe, swf) = player::projector_command(&s.db(), &s.paths, id)?;
        let session = player::start_session(&s.db(), id)?;
        crate::discord::playing(s, id);
        Ok((exe, swf, session))
    })
    .await?;

    let mut child = std::process::Command::new(&exe)
        .arg(&swf)
        .spawn()
        .map_err(|e| AppError(format!("couldn't start the projector: {e}")))?;

    let st = Arc::clone(&state);
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let status = child.wait();
        let seconds = started.elapsed().as_secs() as i64;
        let mut error = status.err().map(|e| e.to_string());
        if let Err(e) = player::update_session(&st.db(), session, seconds) {
            error.get_or_insert(e.0);
        }
        st.presence.clear();
        let _ = app.emit(
            PROJECTOR_EXIT_EVENT,
            ProjectorExit {
                game_id: id,
                seconds,
                error,
            },
        );
    });
    Ok(())
}

// ---------------------------------------------------------------- scraping

pub const SCRAPE_PROGRESS_EVENT: &str = "scrape://progress";

#[tauri::command]
pub async fn scrape_search(
    state: State<'_, Arc<AppState>>,
    id: i64,
    title: Option<String>,
) -> AppResult<Vec<SearchGroup>> {
    blocking(&state, move |s| scrape::service::search(s, id, title)).await
}

#[tauri::command]
pub async fn scrape_apply(
    state: State<'_, Arc<AppState>>,
    id: i64,
    picks: Vec<Match>,
) -> AppResult<ScrapeReport> {
    blocking(&state, move |s| scrape::service::apply(s, id, picks)).await
}

#[tauri::command]
pub async fn scrape_unlink(state: State<'_, Arc<AppState>>, id: i64, source: String) -> AppResult<()> {
    blocking(&state, move |s| scrape::service::unlink(s, id, &source)).await
}

#[tauri::command]
pub async fn scrape_refresh(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<ScrapeReport> {
    blocking(&state, move |s| scrape::service::refresh(s, id)).await
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrapeProgress {
    pub done: usize,
    pub total: usize,
    pub current: String,
    pub finished: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkReport {
    pub reports: Vec<ScrapeReport>,
    pub cancelled: bool,
}

/// Auto-scrapes the given games, or every game missing art when `ids` is
/// null. One game at a time, with progress events and cancellation.
#[tauri::command]
pub async fn scrape_bulk(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    ids: Option<Vec<i64>>,
) -> AppResult<BulkReport> {
    blocking(&state, move |s| {
        s.scrape_cancel.store(false, Ordering::SeqCst);
        let ids = match ids {
            Some(ids) => ids,
            None => scrape::service::missing(&s.db())?,
        };
        let total = ids.len();
        let mut reports = Vec::new();
        let mut cancelled = false;
        for (i, id) in ids.into_iter().enumerate() {
            if s.scrape_cancel.load(Ordering::SeqCst) {
                cancelled = true;
                break;
            }
            let title: String = s
                .db()
                .query_row("SELECT title FROM games WHERE id = ?1", [id], |r| r.get(0))
                .unwrap_or_default();
            let _ = app.emit(
                SCRAPE_PROGRESS_EVENT,
                ScrapeProgress {
                    done: i,
                    total,
                    current: title.clone(),
                    finished: false,
                },
            );
            match scrape::service::auto(s, id) {
                Ok(r) => reports.push(r),
                Err(e) => reports.push(ScrapeReport {
                    game_id: id,
                    title,
                    errors: vec![e.0],
                    ..Default::default()
                }),
            }
        }
        let _ = app.emit(
            SCRAPE_PROGRESS_EVENT,
            ScrapeProgress {
                done: total,
                total,
                current: String::new(),
                finished: true,
            },
        );
        Ok(BulkReport { reports, cancelled })
    })
    .await
}

#[tauri::command]
pub fn scrape_cancel(state: State<'_, Arc<AppState>>) {
    state.scrape_cancel.store(true, Ordering::SeqCst);
}

// ---------------------------------------------------------------- collections

#[tauri::command]
pub async fn collection_create(
    state: State<'_, Arc<AppState>>,
    name: String,
    games: Vec<i64>,
) -> AppResult<Collection> {
    blocking(&state, move |s| {
        library::create_collection(&s.db(), &name, &games)
    })
    .await
}

#[tauri::command]
pub async fn collection_rename(
    state: State<'_, Arc<AppState>>,
    id: i64,
    name: String,
) -> AppResult<Collection> {
    blocking(&state, move |s| library::rename_collection(&s.db(), id, &name)).await
}

#[tauri::command]
pub async fn collection_delete(state: State<'_, Arc<AppState>>, id: i64) -> AppResult<()> {
    blocking(&state, move |s| library::delete_collection(&s.db(), id)).await
}

#[tauri::command]
pub async fn collection_add(
    state: State<'_, Arc<AppState>>,
    id: i64,
    games: Vec<i64>,
) -> AppResult<Collection> {
    blocking(&state, move |s| library::add_to_collection(&s.db(), id, &games)).await
}

#[tauri::command]
pub async fn collection_remove(
    state: State<'_, Arc<AppState>>,
    id: i64,
    games: Vec<i64>,
) -> AppResult<Collection> {
    blocking(&state, move |s| {
        library::remove_from_collection(&s.db(), id, &games)
    })
    .await
}

#[tauri::command]
pub async fn collection_reorder(
    state: State<'_, Arc<AppState>>,
    id: i64,
    games: Vec<i64>,
) -> AppResult<Collection> {
    blocking(&state, move |s| library::reorder_collection(&s.db(), id, &games)).await
}

// ---------------------------------------------------------------- flashpoint

pub const DOWNLOAD_PROGRESS_EVENT: &str = "flashpoint://progress";

#[tauri::command]
pub async fn flashpoint_browse(
    state: State<'_, Arc<AppState>>,
    query: String,
    page: u32,
    sort: String,
    kind: String,
) -> AppResult<download::BrowsePage> {
    blocking(&state, move |s| download::browse(s, &query, page, &sort, &kind)).await
}

#[tauri::command]
pub async fn flashpoint_download(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> AppResult<download::Downloaded> {
    blocking(&state, move |s| {
        s.download_cancel.store(false, Ordering::SeqCst);
        download::download(s, &id, &s.download_cancel, |p| {
            let _ = app.emit(DOWNLOAD_PROGRESS_EVENT, p);
        })
    })
    .await
}

#[tauri::command]
pub async fn flashpoint_cancel(state: State<'_, Arc<AppState>>) -> AppResult<()> {
    state.download_cancel.store(true, Ordering::SeqCst);
    Ok(())
}

// ---------------------------------------------------------------- settings extras

pub const BACKUP_PROGRESS_EVENT: &str = "backup://progress";

#[tauri::command]
pub async fn watch_scan(state: State<'_, Arc<AppState>>) -> AppResult<crate::watch::WatchReport> {
    blocking(&state, crate::watch::scan).await
}

#[tauri::command]
pub async fn games_apply_defaults(state: State<'_, Arc<AppState>>) -> AppResult<()> {
    blocking(&state, |s| import::apply_player_defaults(&s.db(), None)).await
}

#[tauri::command]
pub async fn library_export(app: AppHandle, state: State<'_, Arc<AppState>>, path: String) -> AppResult<u64> {
    blocking(&state, move |s| {
        crate::backup::export(s, std::path::Path::new(&path), |p| {
            let _ = app.emit(BACKUP_PROGRESS_EVENT, p);
        })
    })
    .await
}

#[tauri::command]
pub async fn library_restore(app: AppHandle, state: State<'_, Arc<AppState>>, path: String) -> AppResult<()> {
    blocking(&state, move |s| {
        crate::backup::restore(s, std::path::Path::new(&path), |p| {
            let _ = app.emit(BACKUP_PROGRESS_EVENT, p);
        })
    })
    .await
}

/// Opens a folder in the file manager, or an https link in the browser.
#[tauri::command]
pub async fn open_external(target: String) -> AppResult<()> {
    let is_url = target.starts_with("https://") && !target.contains(char::is_whitespace);
    if !is_url && !std::path::Path::new(&target).is_dir() {
        return Err(AppError("that folder doesn't exist".into()));
    }
    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(&target)
        .spawn()
        .map_err(|e| AppError(format!("couldn't open it: {e}")))?;
    Ok(())
}

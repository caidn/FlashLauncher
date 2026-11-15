mod backup;
mod commands;
mod content;
mod db;
mod discord;
mod download;
mod error;
mod import;
mod library;
mod paths;
mod player;
mod scrape;
mod swf;
mod title;
mod watch;

use paths::AppPaths;
use rusqlite::Connection;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::Manager;

pub struct AppState {
    db: Mutex<Connection>,
    pub paths: AppPaths,
    pub http: scrape::Http,
    /// Set to stop a running bulk scrape after the current game.
    pub scrape_cancel: AtomicBool,
    /// Set to stop the running Flashpoint download.
    pub download_cancel: AtomicBool,
    /// Discord Rich Presence ("Playing ...").
    pub presence: discord::Presence,
}

impl AppState {
    /// One connection, one writer. Fine for a single-user desktop library.
    pub fn db(&self) -> MutexGuard<'_, Connection> {
        // A panic while holding the lock leaves the connection usable.
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .register_asynchronous_uri_scheme_protocol(content::SCHEME, |ctx, request, responder| {
            let st = Arc::clone(ctx.app_handle().state::<Arc<AppState>>().inner());
            let path = request.uri().path().to_string();
            // Disk reads and legacy fetches stay off the webview's thread.
            tauri::async_runtime::spawn_blocking(move || responder.respond(content::respond(&st, &path)));
        })
        .setup(|app| {
            let paths = AppPaths::new(app.path().app_data_dir()?)?;
            paths.clear_pending();
            let conn = db::open(&paths.db)?;
            let http = scrape::Http::new(paths.root.join("cache").join("remote"))?;
            app.manage(Arc::new(AppState {
                db: Mutex::new(conn),
                paths,
                http,
                scrape_cancel: AtomicBool::new(false),
                download_cancel: AtomicBool::new(false),
                presence: discord::Presence::start(),
            }));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::library_list,
            commands::library_inspect,
            commands::library_import,
            commands::game_update,
            commands::game_delete,
            commands::collections_list,
            commands::settings_get,
            commands::settings_set,
            commands::app_data_dir,
            commands::game_read_swf,
            commands::game_get,
            commands::saves_get,
            commands::saves_put,
            commands::session_start,
            commands::session_update,
            commands::art_list,
            commands::art_select,
            commands::art_add_screenshot,
            commands::art_delete,
            commands::art_upload,
            commands::scrape_search,
            commands::scrape_apply,
            commands::scrape_unlink,
            commands::scrape_refresh,
            commands::scrape_bulk,
            commands::scrape_cancel,
            commands::collection_create,
            commands::collection_rename,
            commands::collection_delete,
            commands::collection_add,
            commands::collection_remove,
            commands::collection_reorder,
            commands::projector_launch,
            commands::flashpoint_browse,
            commands::flashpoint_download,
            commands::flashpoint_cancel,
            commands::watch_scan,
            commands::games_apply_defaults,
            commands::library_export,
            commands::library_restore,
            commands::open_external,
            commands::discord_clear,
            commands::updates_configured,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Flash Launcher");
}

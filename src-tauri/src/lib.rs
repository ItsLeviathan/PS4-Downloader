pub mod checksum;
mod commands;
pub mod downloader;
pub mod error;
pub mod events;
pub mod games;
pub mod settings;
pub mod state;
pub mod storage;

use downloader::manager::Manager;
use events::TauriSink;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager as _, RunEvent};

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let sink = Arc::new(TauriSink::new(app.handle().clone()));
            // Load inside the async runtime so background tasks attach to it.
            let manager = tauri::async_runtime::block_on(async move { Manager::load(data_dir, sink) });
            app.manage(manager);
            app.manage(games::GameDb::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::get_storage_info,
            commands::get_storage_usage,
            commands::create_storage_folder,
            commands::list_downloads,
            commands::add_download,
            commands::pause_download,
            commands::resume_download,
            commands::retry_download,
            commands::cancel_download,
            commands::remove_download,
            commands::pause_all,
            commands::resume_all,
            commands::reveal_download,
            commands::open_storage_folder,
            commands::search_games,
            commands::get_game,
            commands::open_external,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build the PS4 Downloader application");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            // Give transfers a moment to flush data and save resume manifests.
            let manager = handle.state::<Arc<Manager>>().inner().clone();
            tauri::async_runtime::block_on(manager.shutdown(Duration::from_secs(3)));
        }
    });
}

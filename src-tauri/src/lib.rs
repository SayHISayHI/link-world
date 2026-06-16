use tauri::Manager;

pub mod commands;
pub mod domain;
pub mod errors;
pub mod events;
pub mod jobs;
pub mod repositories;
pub mod runtime;
pub mod search;
pub mod security;
pub mod services;
pub mod state;
pub mod storage;
pub mod telemetry;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = tauri::async_runtime::block_on(state::AppState::initialize(app.handle()))
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::ping,
            commands::capture::submit_capture,
            commands::library::get_recent_objects,
            commands::library::get_object_detail
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Link World");
}

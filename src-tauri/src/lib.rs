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
pub mod telemetry;

pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![commands::system::ping])
        .run(tauri::generate_context!())
        .expect("failed to run Link World");
}


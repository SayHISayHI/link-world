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
            let capture_service = services::capture::CaptureService::from_state(&state)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            let ai_service = services::ai::AIEnrichmentService::from_state(&state)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;

            services::browser_capture::spawn_loopback_capture_server(
                app.handle().clone(),
                capture_service,
                ai_service,
            );
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::ping,
            commands::ai::get_model_provider_config,
            commands::ai::test_model_provider_config,
            commands::ai::update_model_provider_config,
            commands::ai::trigger_ai_enrichment,
            commands::capture::submit_capture,
            commands::evaluation::trigger_evaluation,
            commands::evaluation::get_evaluation_run,
            commands::library::get_recent_objects,
            commands::library::get_object_detail,
            commands::library::delete_object,
            commands::operations::get_background_job,
            commands::operations::get_object_jobs,
            commands::operations::retry_background_job,
            commands::search::search_hybrid,
            commands::search::rebuild_search_index,
            commands::search::reindex_object
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Link World");
}

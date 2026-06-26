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
            let app_handle = app.handle().clone();
            let data_dir = state::AppState::app_data_dir(&app_handle)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;

            match tauri::async_runtime::block_on(state::AppState::initialize_from_data_dir(
                data_dir.clone(),
            )) {
                Ok(app_state) => {
                    let capture_service = services::capture::CaptureService::from_state(&app_state)
                        .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
                    let ai_service = services::ai::AIEnrichmentService::from_state(&app_state)
                        .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;

                    services::browser_capture::spawn_loopback_capture_server(
                        app_handle,
                        capture_service,
                        ai_service,
                    );
                    app.manage(state::StartupState::ready(data_dir));
                    app.manage(app_state);
                }
                Err(error) => {
                    app.manage(state::StartupState::recovery(data_dir, error));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::get_startup_status,
            commands::system::restart_app,
            commands::system::ping,
            commands::system::get_local_metrics_snapshot,
            commands::backup::create_backup,
            commands::backup::list_backups,
            commands::backup::prepare_restore,
            commands::backup::get_restore_status,
            commands::backup::restart_to_apply_restore,
            commands::backup::verify_backup,
            commands::ai::get_model_provider_config,
            commands::ai::list_model_provider_configs,
            commands::ai::save_model_provider_config,
            commands::ai::delete_model_provider_config,
            commands::ai::set_default_model_provider,
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
            commands::portable_export::export_library,
            commands::search::search_hybrid,
            commands::search::check_search_index,
            commands::search::rebuild_search_index,
            commands::search::get_search_index_rebuild_status,
            commands::search::cancel_search_index_rebuild,
            commands::search::reindex_object
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Link World");
}

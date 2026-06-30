use crate::domain::backup::{BackupSummary, BackupVerification, RestorePreparation, RestoreStatus};
use crate::errors::{map_ipc_result, AppError, AppResult, IpcResponse};
use crate::services::backup::{BackupCatalog, BackupService, BACKUPS_DIR_NAME};
use crate::services::restore::{has_pending_restore_in_dir, read_last_status, RestoreService};
use crate::state::{AppState, StartupState};
use crate::storage::database::Database;
use crate::storage::object_store::ObjectStore;
use crate::telemetry::StructuredLogger;
use tauri::Manager;

#[tauri::command]
pub async fn create_backup(
    app_handle: tauri::AppHandle,
) -> Result<IpcResponse<BackupSummary>, String> {
    let result = async {
        let service = ready_backup_service(&app_handle)?;
        service.create_backup().await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn list_backups(
    app_handle: tauri::AppHandle,
    startup: tauri::State<'_, StartupState>,
) -> Result<IpcResponse<Vec<BackupSummary>>, String> {
    let result = async {
        let catalog = backup_catalog(&app_handle, startup.inner())?;
        catalog.list_backups().await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn verify_backup(
    app_handle: tauri::AppHandle,
    startup: tauri::State<'_, StartupState>,
    backup_id: String,
) -> Result<IpcResponse<BackupVerification>, String> {
    let result = async {
        let catalog = backup_catalog(&app_handle, startup.inner())?;
        catalog.verify_backup(&backup_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}
#[tauri::command]
pub async fn prepare_restore(
    app_handle: tauri::AppHandle,
    startup: tauri::State<'_, StartupState>,
    backup_id: String,
) -> Result<IpcResponse<RestorePreparation>, String> {
    let result = async {
        let service = restore_service(&app_handle, startup.inner()).await?;
        service.prepare_restore(&backup_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn get_restore_status(
    startup: tauri::State<'_, StartupState>,
) -> Result<IpcResponse<Option<RestoreStatus>>, String> {
    let result = read_last_status(startup.data_dir());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn restart_to_apply_restore(
    app_handle: tauri::AppHandle,
    startup: tauri::State<'_, StartupState>,
) -> Result<IpcResponse<bool>, String> {
    let result = has_pending_restore_in_dir(startup.data_dir()).and_then(|has_pending| {
        if has_pending {
            Ok(true)
        } else {
            Err(AppError::RestoreInvalid(
                "no prepared restore is pending".to_string(),
            ))
        }
    });

    if result.is_ok() {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            tauri::process::restart(&app_handle.env());
        });
    }

    Ok(map_ipc_result(result))
}

fn ready_backup_service(app_handle: &tauri::AppHandle) -> AppResult<BackupService> {
    let state = app_handle.try_state::<AppState>().ok_or_else(|| {
        AppError::RestoreInvalid(
            "creating a new backup is unavailable while startup recovery is active".to_string(),
        )
    })?;
    BackupService::from_state(state.inner())
}

fn backup_catalog(
    app_handle: &tauri::AppHandle,
    startup: &StartupState,
) -> AppResult<BackupCatalog> {
    if let Some(state) = app_handle.try_state::<AppState>() {
        return BackupService::from_state(state.inner()).map(|service| service.catalog());
    }

    Ok(BackupCatalog::new(
        startup.data_dir().join(BACKUPS_DIR_NAME),
    ))
}

async fn restore_service(
    app_handle: &tauri::AppHandle,
    startup: &StartupState,
) -> AppResult<RestoreService> {
    if let Some(state) = app_handle.try_state::<AppState>() {
        return RestoreService::from_state(state.inner());
    }

    let data_dir = startup.data_dir().to_path_buf();
    let object_store = ObjectStore::initialize(data_dir.clone())?;
    let database = Database::connect_without_migrations(data_dir.clone()).await?;
    let backup_service = BackupService::new(
        database.pool().clone(),
        object_store.root().to_path_buf(),
        data_dir.join(BACKUPS_DIR_NAME),
        startup.backend_version().to_string(),
    );

    let structured_logger = StructuredLogger::new(&data_dir);
    Ok(RestoreService::new(backup_service, data_dir).with_structured_logger(structured_logger))
}

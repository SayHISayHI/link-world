use crate::domain::backup::{BackupSummary, BackupVerification, RestorePreparation, RestoreStatus};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::backup::BackupService;
use crate::services::restore::RestoreService;
use crate::state::AppState;
use tauri::Manager;

#[tauri::command]
pub async fn create_backup(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<BackupSummary>, String> {
    let result = async {
        let service = BackupService::from_state(state.inner())?;
        service.create_backup().await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn list_backups(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<Vec<BackupSummary>>, String> {
    let result = async {
        let service = BackupService::from_state(state.inner())?;
        service.list_backups().await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn verify_backup(
    state: tauri::State<'_, AppState>,
    backup_id: String,
) -> Result<IpcResponse<BackupVerification>, String> {
    let result = async {
        let service = BackupService::from_state(state.inner())?;
        service.verify_backup(&backup_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}
#[tauri::command]
pub async fn prepare_restore(
    state: tauri::State<'_, AppState>,
    backup_id: String,
) -> Result<IpcResponse<RestorePreparation>, String> {
    let result = async {
        let service = RestoreService::from_state(state.inner())?;
        service.prepare_restore(&backup_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn get_restore_status(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<Option<RestoreStatus>>, String> {
    let result =
        RestoreService::from_state(state.inner()).and_then(|service| service.get_last_status());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn restart_to_apply_restore(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<bool>, String> {
    let result = RestoreService::from_state(state.inner()).and_then(|service| {
        if service.has_pending_restore()? {
            Ok(true)
        } else {
            Err(crate::errors::AppError::RestoreInvalid(
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

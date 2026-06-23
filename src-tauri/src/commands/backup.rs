use crate::domain::backup::{BackupSummary, BackupVerification};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::backup::BackupService;
use crate::state::AppState;

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

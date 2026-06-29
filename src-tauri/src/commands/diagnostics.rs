use crate::domain::diagnostics::SupportBundleSummary;
use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::services::support_bundle::SupportBundleService;
use crate::state::{AppState, StartupState};

#[tauri::command]
pub async fn export_support_bundle(
    confirmed: bool,
    state: tauri::State<'_, AppState>,
    startup: tauri::State<'_, StartupState>,
) -> Result<IpcResponse<SupportBundleSummary>, String> {
    let result = async {
        if startup.is_recovery() {
            return Err(AppError::PolicyDenied(
                "support bundle export is unavailable while startup recovery is active".to_string(),
            ));
        }

        let service = SupportBundleService::new(
            state.database()?.clone(),
            state.object_store()?.clone(),
            startup.data_dir(),
            state.backend_version().to_string(),
        );
        service.export_support_bundle(confirmed).await
    }
    .await;

    Ok(map_ipc_result(result))
}

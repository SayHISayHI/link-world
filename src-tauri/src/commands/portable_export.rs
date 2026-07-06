use crate::domain::portable_export::PortableExportSummary;
use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::services::portable_export::PortableExportService;
use crate::state::{AppState, StartupState};

#[tauri::command]
pub async fn export_library(
    state: tauri::State<'_, AppState>,
    startup: tauri::State<'_, StartupState>,
) -> Result<IpcResponse<PortableExportSummary>, String> {
    let result = async {
        if startup.is_recovery() {
            return Err(AppError::PolicyDenied(
                "portable export is unavailable while startup recovery is active".to_string(),
            ));
        }

        let service = PortableExportService::from_state(state.inner(), startup.data_dir())?;

        service.export_library().await
    }
    .await;

    Ok(map_ipc_result(result))
}

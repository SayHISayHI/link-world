use crate::domain::diagnostics::LocalMetricsSnapshot;
use crate::domain::startup::StartupStatus;
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::system::SystemService;
use crate::state::{AppState, StartupState};
use serde::Serialize;
use tauri::Manager;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    message: String,
    backend_version: String,
}

#[tauri::command]
pub fn get_startup_status(startup: tauri::State<'_, StartupState>) -> IpcResponse<StartupStatus> {
    map_ipc_result(Ok(startup.status().clone()))
}

#[tauri::command]
pub fn restart_app(app_handle: tauri::AppHandle) -> IpcResponse<bool> {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        tauri::process::restart(&app_handle.env());
    });
    map_ipc_result(Ok(true))
}

#[tauri::command]
pub async fn get_local_metrics_snapshot(
    state: tauri::State<'_, AppState>,
    startup: tauri::State<'_, StartupState>,
) -> Result<IpcResponse<LocalMetricsSnapshot>, String> {
    let result = async {
        let service = SystemService::new(state.inner());
        service.local_metrics_snapshot(startup.data_dir()).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub fn ping(state: tauri::State<'_, AppState>) -> IpcResponse<PingResponse> {
    map_ipc_result({
        let service = SystemService::new(state.inner());
        let backend_version = service.backend_version();

        Ok(PingResponse {
            message: "Node Tide backend is available.".to_string(),
            backend_version,
        })
    })
}

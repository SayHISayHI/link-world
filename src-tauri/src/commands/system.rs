use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::system::SystemService;
use crate::state::AppState;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    message: String,
    backend_version: String,
}

#[tauri::command]
pub fn ping(state: tauri::State<'_, AppState>) -> IpcResponse<PingResponse> {
    map_ipc_result({
        let service = SystemService::new(state.inner());
        let backend_version = service.backend_version();

        Ok(PingResponse {
            message: "Link World backend is available.".to_string(),
            backend_version,
        })
    })
}

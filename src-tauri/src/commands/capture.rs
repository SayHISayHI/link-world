use crate::domain::capture::{RawCaptureItem, SubmitCaptureResponse};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::capture::CaptureService;
use crate::state::AppState;

#[tauri::command]
pub async fn submit_capture(
    state: tauri::State<'_, AppState>,
    item: RawCaptureItem,
) -> Result<IpcResponse<SubmitCaptureResponse>, String> {
    let result = async {
        let service = CaptureService::from_state(state.inner())?;
        service.submit(item).await
    }
    .await;

    Ok(map_ipc_result(result))
}

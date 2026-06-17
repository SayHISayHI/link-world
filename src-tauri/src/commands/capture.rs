use crate::domain::capture::{RawCaptureItem, SubmitCaptureResponse};
use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::services::capture::{spawn_fetch_job_runner, CaptureService};
use crate::state::AppState;

#[tauri::command]
pub async fn submit_capture(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    item: RawCaptureItem,
) -> Result<IpcResponse<SubmitCaptureResponse>, String> {
    let result = async {
        let service = CaptureService::from_state(state.inner())?;
        let response = service.submit(item).await?;

        if response.parsed_document_id.is_none() {
            spawn_fetch_job_runner(app_handle, service, response.job_id.clone());
        }

        Ok::<SubmitCaptureResponse, AppError>(response)
    }
    .await;

    Ok(map_ipc_result(result))
}

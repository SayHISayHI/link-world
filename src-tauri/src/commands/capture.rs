use crate::domain::capture::{RawCaptureItem, SubmitCaptureResponse};
use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::services::capture::CaptureService;
use crate::state::AppState;
use serde_json::json;
use tauri::Emitter;

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

fn spawn_fetch_job_runner(app_handle: tauri::AppHandle, service: CaptureService, job_id: String) {
    tauri::async_runtime::spawn(async move {
        let result = service.run_fetch_job(&job_id).await;

        let payload = match result {
            Ok(Some(result)) => json!({
                "jobId": job_id,
                "status": result.status,
                "objectId": result.object_id,
                "lifecycleStatus": result.lifecycle_status,
                "parsedDocumentId": result.parsed_document_id,
                "failureReason": result.failure_reason,
            }),
            Ok(None) => json!({
                "jobId": job_id,
                "status": "skipped",
            }),
            Err(error) => json!({
                "jobId": job_id,
                "status": "failed",
                "failureReason": error.to_string(),
            }),
        };

        let _ = app_handle.emit("capture://job-completed", payload);
        let _ = app_handle.emit("library://objects-updated", ());
    });
}

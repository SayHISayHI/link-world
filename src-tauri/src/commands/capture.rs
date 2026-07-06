use crate::domain::capture::{RawCaptureItem, SubmitCaptureResponse};
use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::services::ai::{spawn_ai_enrichment_runner, AIEnrichmentService};
use crate::services::capture::{spawn_fetch_job_runner, CaptureService};
use crate::state::AppState;
use tauri::Emitter;

#[tauri::command]
pub async fn submit_capture(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    item: RawCaptureItem,
) -> Result<IpcResponse<SubmitCaptureResponse>, String> {
    let result = async {
        let service = CaptureService::from_state(state.inner())?;
        let ai_service = AIEnrichmentService::from_state(state.inner())?;
        let response = service.submit(item).await?;

        if response.deduplicated {
            let _ = app_handle.emit("library://objects-updated", ());
            return Ok::<SubmitCaptureResponse, AppError>(response);
        }

        if let Some(job_id) = response
            .job_id
            .clone()
            .filter(|_| response.parsed_document_id.is_none())
        {
            spawn_fetch_job_runner(app_handle.clone(), service, ai_service, job_id);
        } else if response.parsed_document_id.is_some() {
            spawn_ai_enrichment_runner(app_handle.clone(), ai_service, response.object_id.clone());
        }

        let _ = app_handle.emit("library://objects-updated", ());
        Ok::<SubmitCaptureResponse, AppError>(response)
    }
    .await;

    Ok(map_ipc_result(result))
}

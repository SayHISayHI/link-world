use crate::domain::jobs::BackgroundJob;
use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::services::ai::AIEnrichmentService;
use crate::services::capture::{spawn_fetch_job_runner, CaptureService};
use crate::services::operations::OperationsService;
use crate::state::AppState;
use tauri::Emitter;

#[tauri::command]
pub async fn get_background_job(
    state: tauri::State<'_, AppState>,
    job_id: String,
) -> Result<IpcResponse<BackgroundJob>, String> {
    let result = async {
        let service = OperationsService::from_state(state.inner())?;
        service.get_background_job(&job_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn get_object_jobs(
    state: tauri::State<'_, AppState>,
    object_id: String,
    limit: Option<i64>,
) -> Result<IpcResponse<Vec<BackgroundJob>>, String> {
    let result = async {
        let service = OperationsService::from_state(state.inner())?;
        service.list_object_jobs(&object_id, limit).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn retry_background_job(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    job_id: String,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        let operations = OperationsService::from_state(state.inner())?;
        let retried = operations.reserve_retry(&job_id).await?;

        if retried.job_type == "capture.fetch_url" {
            let service = CaptureService::from_state(state.inner())?;
            let ai_service = AIEnrichmentService::from_state(state.inner())?;
            spawn_fetch_job_runner(app_handle.clone(), service, ai_service, retried.id);
        } else {
            return Err(AppError::PolicyDenied(format!(
                "retry runner is not registered for job type: {}",
                retried.job_type
            )));
        }

        Ok::<bool, AppError>(true)
    }
    .await;

    if result.is_ok() {
        let _ = app_handle.emit("library://objects-updated", ());
    }

    Ok(map_ipc_result(result))
}

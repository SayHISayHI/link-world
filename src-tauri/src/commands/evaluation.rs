use crate::domain::evaluation::{EvaluatorCapability, TriggerEvaluationResponse};
use crate::domain::knowledge::EvaluationRun;
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::evaluation::EvaluationService;
use crate::state::AppState;
use serde_json::json;
use tauri::Emitter;

#[tauri::command]
pub async fn trigger_evaluation(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
    evaluator_type: String,
    request_id: Option<String>,
) -> Result<IpcResponse<TriggerEvaluationResponse>, String> {
    let result = async {
        let service = EvaluationService::from_state(state.inner())?;
        service
            .trigger_evaluation(&object_id, &evaluator_type, request_id.as_deref())
            .await
    }
    .await;

    if let Ok(response) = &result {
        if response.status == "passed" {
            let _ = app_handle.emit(
                "evaluation://completed",
                json!({
                    "objectId": object_id,
                    "runId": response.run_id,
                }),
            );
            let _ = app_handle.emit("library://objects-updated", ());
        }
    }

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn list_evaluator_capabilities(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<Vec<EvaluatorCapability>>, String> {
    let result = EvaluationService::from_state(state.inner())
        .map(|service| service.list_evaluator_capabilities());
    Ok(map_ipc_result(result))
}
#[tauri::command]
pub async fn get_evaluation_run(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> Result<IpcResponse<EvaluationRun>, String> {
    let result = async {
        let service = EvaluationService::from_state(state.inner())?;
        service.get_evaluation_run(&run_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

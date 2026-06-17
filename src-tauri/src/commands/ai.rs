use crate::domain::ai::{AIEnrichmentRunResult, ModelProviderConfig};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::ai::AIEnrichmentService;
use crate::state::AppState;
use tauri::Emitter;

#[tauri::command]
pub async fn update_model_provider_config(
    state: tauri::State<'_, AppState>,
    config: ModelProviderConfig,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        let service =
            AIEnrichmentService::new(state.database()?.pool().clone(), state.secrets().clone());

        service.update_model_provider_config(config).await?;
        Ok(true)
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn trigger_ai_enrichment(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
) -> Result<IpcResponse<AIEnrichmentRunResult>, String> {
    let result = async {
        let service =
            AIEnrichmentService::new(state.database()?.pool().clone(), state.secrets().clone());

        service.run_enrichment_for_object(&object_id).await
    }
    .await;

    if result
        .as_ref()
        .map(|run| run.status == "succeeded")
        .unwrap_or(false)
    {
        let _ = app_handle.emit("library://objects-updated", ());
    }

    Ok(map_ipc_result(result))
}

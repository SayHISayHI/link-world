use crate::domain::ai::{
    AIEnrichmentRunResult, ModelProviderConfig, ModelProviderConfigView, ModelProviderTestResult,
};
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
        let service = AIEnrichmentService::from_state(state.inner())?;

        service.update_model_provider_config(config).await?;
        Ok(true)
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn get_model_provider_config(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<Option<ModelProviderConfigView>>, String> {
    let result = async {
        let service = AIEnrichmentService::from_state(state.inner())?;
        service.get_model_provider_config().await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn test_model_provider_config(
    state: tauri::State<'_, AppState>,
    config: ModelProviderConfig,
) -> Result<IpcResponse<ModelProviderTestResult>, String> {
    let result = async {
        let service = AIEnrichmentService::from_state(state.inner())?;
        service.test_model_provider_config(config).await
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
        let service = AIEnrichmentService::from_state(state.inner())?;

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

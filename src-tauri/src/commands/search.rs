use crate::domain::search::{RebuildSearchIndexResponse, ReindexObjectResponse, SearchResult};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::search::SearchService;
use crate::state::AppState;
use serde_json::json;
use tauri::Emitter;

#[tauri::command]
pub async fn search_hybrid(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<i64>,
    filter_type: Option<String>,
) -> Result<IpcResponse<Vec<SearchResult>>, String> {
    let result = async {
        let service = SearchService::from_state(state.inner())?;
        service.search_hybrid(&query, limit, filter_type).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn rebuild_search_index(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<RebuildSearchIndexResponse>, String> {
    let result = async {
        let service = SearchService::from_state(state.inner())?;
        service.rebuild_search_index().await
    }
    .await;

    if let Ok(response) = &result {
        let _ = app_handle.emit(
            "search://index-rebuilt",
            json!({
                "jobId": response.job_id,
                "indexedObjects": response.indexed_objects,
            }),
        );
        let _ = app_handle.emit("library://objects-updated", ());
    }

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn reindex_object(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
) -> Result<IpcResponse<ReindexObjectResponse>, String> {
    let result = async {
        let service = SearchService::from_state(state.inner())?;
        service.reindex_object(&object_id).await
    }
    .await;

    if let Ok(response) = &result {
        let _ = app_handle.emit(
            "search://object-reindexed",
            json!({
                "jobId": response.job_id,
                "objectId": response.object_id,
                "indexed": response.indexed,
            }),
        );
    }

    Ok(map_ipc_result(result))
}

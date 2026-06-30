use crate::domain::search::{
    RebuildSearchIndexResponse, ReindexObjectResponse, SearchIndexHealthResponse, SearchResult,
    SEARCH_REBUILD_FAILURE_REASON,
};
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
        let response = service.rebuild_search_index().await?;
        spawn_search_index_rebuild_runner(app_handle.clone(), service, response.job_id.clone());
        Ok(response)
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn get_search_index_rebuild_status(
    state: tauri::State<'_, AppState>,
    job_id: String,
) -> Result<IpcResponse<RebuildSearchIndexResponse>, String> {
    let result = async {
        let service = SearchService::from_state(state.inner())?;
        service.get_rebuild_search_index_status(&job_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn cancel_search_index_rebuild(
    state: tauri::State<'_, AppState>,
    job_id: String,
) -> Result<IpcResponse<RebuildSearchIndexResponse>, String> {
    let result = async {
        let service = SearchService::from_state(state.inner())?;
        service.cancel_rebuild_search_index(&job_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn check_search_index(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<SearchIndexHealthResponse>, String> {
    let result = async {
        let service = SearchService::from_state(state.inner())?;
        service.check_search_index().await
    }
    .await;

    Ok(map_ipc_result(result))
}

fn spawn_search_index_rebuild_runner(
    app_handle: tauri::AppHandle,
    service: SearchService,
    job_id: String,
) {
    tauri::async_runtime::spawn(async move {
        let result = service.run_rebuild_search_index(&job_id).await;
        match result {
            Ok(response) => {
                let _ = app_handle.emit("search://index-rebuild-status", &response);
                if response.status == "succeeded" {
                    let _ = app_handle.emit(
                        "search://index-rebuilt",
                        json!({
                            "jobId": response.job_id,
                            "status": response.status,
                            "stage": response.stage,
                            "indexedObjects": response.indexed_objects,
                            "expectedObjects": response.expected_objects,
                        }),
                    );
                    let _ = app_handle.emit("library://objects-updated", ());
                }
            }
            Err(_) => {
                let _ = app_handle.emit(
                    "search://index-rebuild-status",
                    json!({
                        "jobId": job_id,
                        "status": "failed",
                        "stage": "failed",
                        "failureReason": SEARCH_REBUILD_FAILURE_REASON,
                        "cancellable": false,
                    }),
                );
            }
        }
    });
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

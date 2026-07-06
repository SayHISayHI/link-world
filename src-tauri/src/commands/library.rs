use crate::domain::knowledge::{
    DeleteObjectMode, DeleteObjectResponse, KnowledgeObject, KnowledgeObjectDetail,
};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::library::LibraryService;
use crate::state::AppState;
use tauri::Emitter;

#[tauri::command]
pub async fn get_recent_objects(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    filter_type: Option<String>,
) -> Result<IpcResponse<Vec<KnowledgeObject>>, String> {
    let result = async {
        let service = LibraryService::from_state(state.inner())?;
        service.list_recent(limit, offset, filter_type).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn get_object_detail(
    state: tauri::State<'_, AppState>,
    object_id: String,
) -> Result<IpcResponse<KnowledgeObjectDetail>, String> {
    let result = async {
        let service = LibraryService::from_state(state.inner())?;
        service.get_detail(&object_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn delete_object(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
    mode: DeleteObjectMode,
) -> Result<IpcResponse<DeleteObjectResponse>, String> {
    let result = async {
        let service = LibraryService::from_state(state.inner())?;
        service.delete_object(&object_id, mode).await
    }
    .await;

    if result.is_ok() {
        let _ = app_handle.emit("library://objects-updated", ());
    }

    Ok(map_ipc_result(result))
}

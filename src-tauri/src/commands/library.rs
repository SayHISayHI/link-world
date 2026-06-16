use crate::domain::knowledge::{KnowledgeObject, KnowledgeObjectDetail};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::repositories::knowledge_objects::KnowledgeObjectRepository;
use crate::state::AppState;

#[tauri::command]
pub async fn get_recent_objects(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
    filter_type: Option<String>,
) -> Result<IpcResponse<Vec<KnowledgeObject>>, String> {
    let result = async {
        let pool = state.database()?.pool().clone();
        let repository = KnowledgeObjectRepository::new(pool);

        repository.list_recent(limit, offset, filter_type).await
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
        let pool = state.database()?.pool().clone();
        let repository = KnowledgeObjectRepository::new(pool);

        repository.get_detail(&object_id).await
    }
    .await;

    Ok(map_ipc_result(result))
}

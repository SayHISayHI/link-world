use crate::domain::search::SearchResult;
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::search::SearchService;
use crate::state::AppState;

#[tauri::command]
pub async fn search_hybrid(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<IpcResponse<Vec<SearchResult>>, String> {
    let result = async {
        let service = SearchService::from_state(state.inner())?;
        service.search_hybrid(&query, limit).await
    }
    .await;

    Ok(map_ipc_result(result))
}

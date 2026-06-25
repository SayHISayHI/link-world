use crate::domain::portable_export::PortableExportSummary;
use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::repositories::knowledge_objects::KnowledgeObjectRepository;
use crate::services::portable_export::PortableExportService;
use crate::state::{AppState, StartupState};

#[tauri::command]
pub async fn export_library(
    state: tauri::State<'_, AppState>,
    startup: tauri::State<'_, StartupState>,
) -> Result<IpcResponse<PortableExportSummary>, String> {
    let result = async {
        if startup.is_recovery() {
            return Err(AppError::PolicyDenied(
                "portable export is unavailable while startup recovery is active".to_string(),
            ));
        }

        let pool = state.database()?.pool().clone();
        let repository = KnowledgeObjectRepository::new(pool);
        let service = PortableExportService::new(
            repository,
            startup.data_dir(),
            state.backend_version().to_string(),
        );

        service.export_library().await
    }
    .await;

    Ok(map_ipc_result(result))
}

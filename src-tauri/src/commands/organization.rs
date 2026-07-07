use crate::domain::knowledge::KnowledgeObject;
use crate::domain::organization::{
    Collection, CreateCollectionInput, CreateSmartViewInput, LibraryNavigation, LibraryPage,
    LibraryQuery, ObjectOrganization, Tag, UpdateCollectionInput,
};
use crate::errors::{map_ipc_result, IpcResponse};
use crate::services::organization::OrganizationService;
use crate::state::AppState;
use tauri::Emitter;

#[tauri::command]
pub async fn get_library_navigation(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<LibraryNavigation>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .get_navigation()
            .await
    }
    .await;
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn list_library_objects(
    state: tauri::State<'_, AppState>,
    query: LibraryQuery,
) -> Result<IpcResponse<LibraryPage<KnowledgeObject>>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .list_objects(query)
            .await
    }
    .await;
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn create_collection(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    input: CreateCollectionInput,
) -> Result<IpcResponse<Collection>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .create_collection(input)
            .await
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn create_smart_view(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    input: CreateSmartViewInput,
) -> Result<IpcResponse<Collection>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .create_smart_view(input)
            .await
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn update_collection(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    input: UpdateCollectionInput,
) -> Result<IpcResponse<Collection>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .update_collection(input)
            .await
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn archive_collection(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    collection_id: String,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .archive_collection(&collection_id)
            .await?;
        Ok(true)
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn add_object_to_collection(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
    collection_id: String,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .add_object_to_collection(&object_id, &collection_id)
            .await?;
        Ok(true)
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn remove_object_from_collection(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
    collection_id: String,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .remove_object_from_collection(&object_id, &collection_id)
            .await?;
        Ok(true)
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn mark_object_triaged(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
    filed: bool,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .mark_object_triaged(&object_id, filed)
            .await?;
        Ok(true)
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn add_user_tag(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
    name: String,
) -> Result<IpcResponse<Tag>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .add_user_tag(&object_id, &name)
            .await
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn remove_object_tag(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    object_id: String,
    tag_id: String,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .remove_object_tag(&object_id, &tag_id)
            .await?;
        Ok(true)
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn accept_tag_suggestion(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    suggestion_id: String,
) -> Result<IpcResponse<Tag>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .accept_tag_suggestion(&suggestion_id)
            .await
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn reject_tag_suggestion(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    suggestion_id: String,
) -> Result<IpcResponse<bool>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .reject_tag_suggestion(&suggestion_id)
            .await?;
        Ok(true)
    }
    .await;
    emit_organization_update(&app_handle, result.is_ok());
    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn get_object_organization(
    state: tauri::State<'_, AppState>,
    object_id: String,
) -> Result<IpcResponse<ObjectOrganization>, String> {
    let result = async {
        OrganizationService::from_state(state.inner())?
            .get_object_organization(&object_id)
            .await
    }
    .await;
    Ok(map_ipc_result(result))
}

fn emit_organization_update(app_handle: &tauri::AppHandle, succeeded: bool) {
    if succeeded {
        let _ = app_handle.emit("organization://updated", ());
        let _ = app_handle.emit("library://objects-updated", ());
    }
}

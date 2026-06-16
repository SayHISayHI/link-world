use crate::errors::{AppError, AppResult};
use crate::storage::database::Database;
use crate::storage::object_store::ObjectStore;
use tauri::Manager;

#[derive(Debug, Default)]
pub struct AppState {
    backend_version: String,
    database: Option<Database>,
    object_store: Option<ObjectStore>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            database: None,
            object_store: None,
        }
    }

    pub async fn initialize(app_handle: &tauri::AppHandle) -> AppResult<Self> {
        let data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Filesystem(error.to_string()))?;
        let database = Database::initialize(data_dir.clone()).await?;
        let object_store = ObjectStore::initialize(data_dir)?;

        Ok(Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            database: Some(database),
            object_store: Some(object_store),
        })
    }

    pub fn backend_version(&self) -> &str {
        &self.backend_version
    }

    pub fn database(&self) -> AppResult<&Database> {
        self.database
            .as_ref()
            .ok_or_else(|| AppError::Database("database is not initialized".to_string()))
    }

    pub fn object_store(&self) -> AppResult<&ObjectStore> {
        self.object_store
            .as_ref()
            .ok_or_else(|| AppError::Filesystem("object store is not initialized".to_string()))
    }
}

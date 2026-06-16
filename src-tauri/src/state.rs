use crate::errors::{AppError, AppResult};
use crate::storage::database::Database;
use tauri::Manager;

#[derive(Debug, Default)]
pub struct AppState {
    backend_version: String,
    database: Option<Database>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            database: None,
        }
    }

    pub async fn initialize(app_handle: &tauri::AppHandle) -> AppResult<Self> {
        let data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Filesystem(error.to_string()))?;
        let database = Database::initialize(data_dir).await?;

        Ok(Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            database: Some(database),
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
}

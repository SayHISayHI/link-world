use crate::errors::{AppError, AppResult};
use crate::runtime::models::ModelProviderRegistry;
use crate::storage::database::Database;
use crate::storage::object_store::ObjectStore;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tauri::Manager;

#[derive(Debug)]
pub struct AppState {
    backend_version: String,
    database: Option<Database>,
    object_store: Option<ObjectStore>,
    secrets: SecretStore,
    model_registry: ModelProviderRegistry,
}

#[derive(Debug, Clone, Default)]
pub struct SecretStore {
    inner: Arc<RwLock<HashMap<String, String>>>,
}

impl SecretStore {
    pub fn set(&self, secret_ref: &str, value: String) -> AppResult<()> {
        let mut secrets = self.inner.write().map_err(|_| AppError::SecretStorage)?;
        secrets.insert(secret_ref.to_string(), value);
        Ok(())
    }

    pub fn resolve(&self, secret_ref: &str) -> AppResult<Option<String>> {
        if let Some(env_key) = secret_ref.strip_prefix("env:") {
            return Ok(std::env::var(env_key)
                .ok()
                .filter(|value| !value.is_empty()));
        }

        let secrets = self.inner.read().map_err(|_| AppError::SecretStorage)?;
        Ok(secrets.get(secret_ref).cloned())
    }
}

impl AppState {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            database: None,
            object_store: None,
            secrets: SecretStore::default(),
            model_registry: ModelProviderRegistry::new()?,
        })
    }

    pub async fn initialize(app_handle: &tauri::AppHandle) -> AppResult<Self> {
        let data_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Filesystem(error.to_string()))?;
        let database = Database::initialize(data_dir.clone()).await?;
        let object_store = ObjectStore::initialize(data_dir)?;
        let model_registry = ModelProviderRegistry::new()?;

        Ok(Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            database: Some(database),
            object_store: Some(object_store),
            secrets: SecretStore::default(),
            model_registry,
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

    pub fn secrets(&self) -> &SecretStore {
        &self.secrets
    }

    pub fn model_registry(&self) -> &ModelProviderRegistry {
        &self.model_registry
    }
}

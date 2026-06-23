use crate::errors::{AppError, AppResult};
use crate::runtime::models::ModelProviderRegistry;
use crate::storage::database::Database;
use crate::storage::object_store::ObjectStore;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};
use tauri::Manager;

#[cfg(target_os = "windows")]
const KEYRING_SERVICE: &str = "com.linkworld.app.model-provider";
#[cfg(target_os = "windows")]
const KEYRING_SECRET_PREFIX: &str = "keyring:model-provider:";

#[derive(Debug)]
pub struct AppState {
    backend_version: String,
    database: Option<Database>,
    object_store: Option<ObjectStore>,
    secrets: SecretStore,
    model_registry: ModelProviderRegistry,
}

trait SecretBackend: Send + Sync {
    fn set(&self, secret_ref: &str, value: &str) -> AppResult<()>;
    fn resolve(&self, secret_ref: &str) -> AppResult<Option<String>>;
    fn delete(&self, secret_ref: &str) -> AppResult<()>;
    fn backend_name(&self) -> &'static str;
}

#[derive(Clone)]
pub struct SecretStore {
    backend: Arc<dyn SecretBackend>,
}

#[derive(Debug, Default)]
struct MemorySecretBackend {
    inner: RwLock<HashMap<String, String>>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct WindowsCredentialBackend;

impl SecretStore {
    pub fn system() -> AppResult<Self> {
        #[cfg(target_os = "windows")]
        {
            Ok(Self {
                backend: Arc::new(WindowsCredentialBackend),
            })
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(AppError::SecretStorage)
        }
    }

    pub fn set(&self, secret_ref: &str, value: String) -> AppResult<()> {
        if secret_ref.starts_with("env:") {
            return Err(AppError::SecretStorage);
        }
        self.backend.set(secret_ref, &value)
    }

    pub fn resolve(&self, secret_ref: &str) -> AppResult<Option<String>> {
        if secret_ref.starts_with("memory:") {
            return Ok(None);
        }

        if let Some(env_key) = secret_ref.strip_prefix("env:") {
            return Ok(std::env::var(env_key)
                .ok()
                .filter(|value| !value.is_empty()));
        }

        self.backend.resolve(secret_ref)
    }

    pub fn delete(&self, secret_ref: &str) -> AppResult<()> {
        if secret_ref.starts_with("env:") || secret_ref.starts_with("memory:") {
            return Ok(());
        }
        self.backend.delete(secret_ref)
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.backend_name()
    }
}

impl Default for SecretStore {
    fn default() -> Self {
        Self {
            backend: Arc::new(MemorySecretBackend::default()),
        }
    }
}

impl fmt::Debug for SecretStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretStore")
            .field("backend", &self.backend_name())
            .finish()
    }
}

impl SecretBackend for MemorySecretBackend {
    fn set(&self, secret_ref: &str, value: &str) -> AppResult<()> {
        let mut secrets = self.inner.write().map_err(|_| AppError::SecretStorage)?;
        secrets.insert(secret_ref.to_string(), value.to_string());
        Ok(())
    }

    fn resolve(&self, secret_ref: &str) -> AppResult<Option<String>> {
        let secrets = self.inner.read().map_err(|_| AppError::SecretStorage)?;
        Ok(secrets.get(secret_ref).cloned())
    }

    fn delete(&self, secret_ref: &str) -> AppResult<()> {
        let mut secrets = self.inner.write().map_err(|_| AppError::SecretStorage)?;
        secrets.remove(secret_ref);
        Ok(())
    }

    fn backend_name(&self) -> &'static str {
        "memory"
    }
}

#[cfg(target_os = "windows")]
impl WindowsCredentialBackend {
    fn entry(secret_ref: &str) -> AppResult<keyring::Entry> {
        let account = secret_ref
            .strip_prefix(KEYRING_SECRET_PREFIX)
            .filter(|value| !value.is_empty())
            .ok_or(AppError::SecretStorage)?;
        keyring::Entry::new(KEYRING_SERVICE, account).map_err(|_| AppError::SecretStorage)
    }
}

#[cfg(target_os = "windows")]
impl SecretBackend for WindowsCredentialBackend {
    fn set(&self, secret_ref: &str, value: &str) -> AppResult<()> {
        Self::entry(secret_ref)?
            .set_password(value)
            .map_err(|_| AppError::SecretStorage)
    }

    fn resolve(&self, secret_ref: &str) -> AppResult<Option<String>> {
        match Self::entry(secret_ref)?.get_password() {
            Ok(value) if !value.is_empty() => Ok(Some(value)),
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(AppError::SecretStorage),
        }
    }

    fn delete(&self, secret_ref: &str) -> AppResult<()> {
        match Self::entry(secret_ref)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(AppError::SecretStorage),
        }
    }

    fn backend_name(&self) -> &'static str {
        "windows_credential_manager"
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
        let secrets = SecretStore::system()?;

        Ok(Self {
            backend_version: env!("CARGO_PKG_VERSION").to_string(),
            database: Some(database),
            object_store: Some(object_store),
            secrets,
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

#[cfg(test)]
mod tests {
    use super::SecretStore;

    #[test]
    fn memory_secret_store_supports_set_resolve_and_delete() {
        let store = SecretStore::default();
        let secret_ref = "keyring:model-provider:test";

        store
            .set(secret_ref, "secret-value".to_string())
            .expect("secret should be stored");
        assert_eq!(
            store.resolve(secret_ref).expect("secret should resolve"),
            Some("secret-value".to_string())
        );

        store.delete(secret_ref).expect("secret should be deleted");
        assert_eq!(
            store
                .resolve("memory:model_provider:legacy:api_key")
                .expect("legacy memory refs should be treated as missing"),
            None
        );
        assert_eq!(
            store
                .resolve(secret_ref)
                .expect("missing secret should resolve"),
            None
        );
    }
}

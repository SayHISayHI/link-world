use crate::domain::startup::{StartupIssue, StartupMode, StartupRecoveryKind, StartupStatus};
use crate::errors::{AppError, AppResult};
use crate::repositories::jobs::JobsRepository;
use crate::runtime::models::ModelProviderRegistry;
use crate::services::migration::MigrationService;
use crate::services::restore::begin_pending_restore;
use crate::storage::database::Database;
use crate::storage::object_store::ObjectStore;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use tauri::Manager;
use uuid::Uuid;

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

#[derive(Debug)]
pub struct StartupState {
    data_dir: PathBuf,
    status: StartupStatus,
}

impl StartupState {
    pub fn ready(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            status: StartupStatus {
                mode: StartupMode::Ready,
                backend_version: env!("CARGO_PKG_VERSION").to_string(),
                issue: None,
            },
        }
    }

    pub fn recovery(data_dir: PathBuf, error: AppError) -> Self {
        let raw_message = error.to_string();
        let message = sanitize_startup_message(&data_dir, &raw_message);
        let migration = if matches!(error, AppError::DbMigration(_)) {
            MigrationService::recovery_info(&data_dir).ok().flatten()
        } else {
            None
        };
        let verified_backup_id = migration
            .as_ref()
            .and_then(|info| info.backup_id.clone())
            .or_else(|| extract_backup_id(&raw_message));

        Self {
            data_dir,
            status: StartupStatus {
                mode: StartupMode::Recovery,
                backend_version: env!("CARGO_PKG_VERSION").to_string(),
                issue: Some(StartupIssue {
                    code: startup_error_code(&error).to_string(),
                    title: startup_issue_title(&error).to_string(),
                    message,
                    recovery_kind: startup_recovery_kind(&error),
                    verified_backup_id,
                    migration,
                }),
            },
        }
    }

    pub fn status(&self) -> &StartupStatus {
        &self.status
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn backend_version(&self) -> &str {
        &self.status.backend_version
    }

    pub fn is_recovery(&self) -> bool {
        self.status.mode == StartupMode::Recovery
    }
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

fn startup_error_code(error: &AppError) -> &'static str {
    match error {
        AppError::BackupInvalid(_) => "ERR_BACKUP_INVALID",
        AppError::RestoreInvalid(_) => "ERR_RESTORE_INVALID",
        AppError::DbConstraint => "ERR_DB_CONSTRAINT",
        AppError::DbMigration(_) => "ERR_DB_MIGRATION",
        AppError::JobNotFound => "ERR_JOB_NOT_FOUND",
        AppError::NetworkTimeout => "ERR_NETWORK_TIMEOUT",
        AppError::ParseFailed(_) => "ERR_PARSE_FAILED",
        AppError::ModelAuth => "ERR_MODEL_AUTH",
        AppError::ModelRateLimit => "ERR_MODEL_RATE_LIMIT",
        AppError::ModelNotFound => "ERR_MODEL_NOT_FOUND",
        AppError::ModelOutputSchema(_) => "ERR_MODEL_OUTPUT_SCHEMA",
        AppError::PolicyDenied(_) => "ERR_POLICY_DENIED",
        AppError::PluginPermission(_) => "ERR_PLUGIN_PERMISSION",
        AppError::ObjectNotFound => "ERR_OBJECT_NOT_FOUND",
        AppError::SecretStorage => "ERR_SECRET_STORAGE",
        AppError::Database(_) | AppError::Filesystem(_) | AppError::Unknown(_) => "ERR_UNKNOWN",
    }
}

fn startup_issue_title(error: &AppError) -> &'static str {
    match error {
        AppError::DbMigration(_) => "Database migration needs recovery",
        AppError::RestoreInvalid(_) => "Restore did not complete safely",
        AppError::Database(_) => "Database could not be opened",
        AppError::Filesystem(_) => "Storage could not be opened",
        AppError::SecretStorage => "Credential storage is unavailable",
        _ => "Startup failed",
    }
}

fn startup_recovery_kind(error: &AppError) -> StartupRecoveryKind {
    match error {
        AppError::DbMigration(_) => StartupRecoveryKind::DatabaseMigration,
        AppError::RestoreInvalid(_) => StartupRecoveryKind::Restore,
        AppError::Database(_) => StartupRecoveryKind::Database,
        AppError::Filesystem(_) => StartupRecoveryKind::Storage,
        _ => StartupRecoveryKind::Unknown,
    }
}

fn sanitize_startup_message(data_dir: &Path, message: &str) -> String {
    let data_dir_text = data_dir.to_string_lossy();
    let redacted = message.replace(data_dir_text.as_ref(), "<app-data>");
    truncate_chars(&redacted, 512)
}

fn extract_backup_id(message: &str) -> Option<String> {
    message
        .split(|character: char| {
            !(character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        })
        .find(|candidate| Uuid::parse_str(candidate).is_ok())
        .map(ToOwned::to_owned)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    truncated
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

    pub fn app_data_dir(app_handle: &tauri::AppHandle) -> AppResult<PathBuf> {
        app_handle
            .path()
            .app_data_dir()
            .map_err(|error| AppError::Filesystem(error.to_string()))
    }

    pub async fn initialize(app_handle: &tauri::AppHandle) -> AppResult<Self> {
        Self::initialize_from_data_dir(Self::app_data_dir(app_handle)?).await
    }

    pub async fn initialize_from_data_dir(data_dir: PathBuf) -> AppResult<Self> {
        let restore_transaction = begin_pending_restore(&data_dir).await?;
        let validate_restored_data = restore_transaction.is_some();
        let storage = Self::initialize_storage(data_dir.clone(), validate_restored_data).await;

        let (database, object_store) = match (storage, restore_transaction) {
            (Ok(storage), Some(transaction)) => {
                transaction.complete()?;
                storage
            }
            (Err(error), Some(transaction)) => {
                let reason = error.to_string();
                transaction.rollback(&reason)?;
                Self::initialize_storage(data_dir.clone(), false).await?
            }
            (Ok(storage), None) => storage,
            (Err(error), None) => return Err(error),
        };
        JobsRepository::new(database.pool().clone())
            .recover_interrupted_jobs_on_startup()
            .await?;
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

    async fn initialize_storage(
        data_dir: std::path::PathBuf,
        validate_integrity: bool,
    ) -> AppResult<(Database, ObjectStore)> {
        let object_store = ObjectStore::initialize(data_dir.clone())?;
        let database = Database::connect_without_migrations(data_dir.clone()).await?;
        if let Err(error) = MigrationService::migrate_with_protection(
            &database,
            &object_store,
            &data_dir,
            env!("CARGO_PKG_VERSION"),
        )
        .await
        {
            database.pool().close().await;
            return Err(error);
        }
        if validate_integrity {
            if let Err(error) = database.validate_integrity().await {
                database.pool().close().await;
                return Err(error);
            }
        }

        Ok((database, object_store))
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
    use super::{SecretStore, StartupState};
    use crate::domain::startup::{StartupMode, StartupRecoveryKind};
    use crate::errors::AppError;
    use std::path::PathBuf;
    use uuid::Uuid;

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

    #[test]
    fn startup_recovery_status_redacts_data_dir_and_extracts_backup_id() {
        let backup_id = Uuid::new_v4().to_string();
        let data_dir = PathBuf::from("C:/Users/example/AppData/Link World");
        let state = StartupState::recovery(
            data_dir.clone(),
            AppError::DbMigration(format!(
                "failed at {}; verified restore point {} is available",
                data_dir.display(),
                backup_id
            )),
        );

        let status = state.status();
        assert_eq!(status.mode, StartupMode::Recovery);
        let issue = status.issue.as_ref().expect("recovery issue should exist");
        assert_eq!(
            issue.verified_backup_id.as_deref(),
            Some(backup_id.as_str())
        );
        assert_eq!(issue.recovery_kind, StartupRecoveryKind::DatabaseMigration);
        assert!(!issue.message.contains("C:/Users/example"));
        assert!(issue.message.contains("<app-data>"));
    }
}

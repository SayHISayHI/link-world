use serde::Serialize;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("backup invalid: {0}")]
    BackupInvalid(String),
    #[error("restore invalid: {0}")]
    RestoreInvalid(String),
    #[error("another Link World process is using this data directory")]
    RuntimeBusy,
    #[error("object not found")]
    ObjectNotFound,
    #[error("database constraint failed")]
    DbConstraint,
    #[error("database migration failed: {0}")]
    DbMigration(String),
    #[error("database operation failed: {0}")]
    Database(String),
    #[error("filesystem error: {0}")]
    Filesystem(String),
    #[error("job not found")]
    JobNotFound,
    #[error("network timeout")]
    NetworkTimeout,
    #[error("parse failed: {0}")]
    ParseFailed(String),
    #[error("model authentication failed")]
    ModelAuth,
    #[error("model rate limit")]
    ModelRateLimit,
    #[error("model or provider endpoint not found")]
    ModelNotFound,
    #[error("model output schema invalid: {0}")]
    ModelOutputSchema(String),
    #[error("policy denied: {0}")]
    PolicyDenied(String),
    #[error("plugin permission denied: {0}")]
    PluginPermission(String),
    #[error("secret storage error")]
    SecretStorage,
    #[error("unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcErrorCode {
    ErrBackupInvalid,
    ErrRestoreInvalid,
    ErrRuntimeBusy,
    ErrDbConstraint,
    ErrDbMigration,
    ErrNetworkTimeout,
    ErrParseFailed,
    ErrModelAuth,
    ErrModelRateLimit,
    ErrModelNotFound,
    ErrModelOutputSchema,
    ErrPolicyDenied,
    ErrPluginPermission,
    ErrJobNotFound,
    ErrObjectNotFound,
    ErrSecretStorage,
    ErrUnknown,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    code: IpcErrorCode,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum IpcResponse<T>
where
    T: Serialize,
{
    #[serde(rename = "success")]
    Success { data: T },
    #[serde(rename = "error")]
    Error { error: IpcError },
}

pub fn map_ipc_result<T>(result: AppResult<T>) -> IpcResponse<T>
where
    T: Serialize,
{
    match result {
        Ok(data) => IpcResponse::Success { data },
        Err(error) => IpcResponse::Error {
            error: IpcError::from(error),
        },
    }
}

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        let code = match error {
            AppError::BackupInvalid(_) => IpcErrorCode::ErrBackupInvalid,
            AppError::RestoreInvalid(_) => IpcErrorCode::ErrRestoreInvalid,
            AppError::RuntimeBusy => IpcErrorCode::ErrRuntimeBusy,
            AppError::ObjectNotFound => IpcErrorCode::ErrObjectNotFound,
            AppError::DbConstraint => IpcErrorCode::ErrDbConstraint,
            AppError::DbMigration(_) => IpcErrorCode::ErrDbMigration,
            AppError::Database(_) => IpcErrorCode::ErrUnknown,
            AppError::Filesystem(_) => IpcErrorCode::ErrUnknown,
            AppError::JobNotFound => IpcErrorCode::ErrJobNotFound,
            AppError::NetworkTimeout => IpcErrorCode::ErrNetworkTimeout,
            AppError::ParseFailed(_) => IpcErrorCode::ErrParseFailed,
            AppError::ModelAuth => IpcErrorCode::ErrModelAuth,
            AppError::ModelRateLimit => IpcErrorCode::ErrModelRateLimit,
            AppError::ModelNotFound => IpcErrorCode::ErrModelNotFound,
            AppError::ModelOutputSchema(_) => IpcErrorCode::ErrModelOutputSchema,
            AppError::PolicyDenied(_) => IpcErrorCode::ErrPolicyDenied,
            AppError::PluginPermission(_) => IpcErrorCode::ErrPluginPermission,
            AppError::SecretStorage => IpcErrorCode::ErrSecretStorage,
            AppError::Unknown(_) => IpcErrorCode::ErrUnknown,
        };

        Self {
            code,
            message: error.to_string(),
            details: None,
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
                AppError::DbConstraint
            }
            other => AppError::Database(other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        AppError::Filesystem(error.to_string())
    }
}

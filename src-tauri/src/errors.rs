use serde::Serialize;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("object not found")]
    ObjectNotFound,
    #[error("database constraint failed")]
    DbConstraint,
    #[error("network timeout")]
    NetworkTimeout,
    #[error("parse failed: {0}")]
    ParseFailed(String),
    #[error("model authentication failed")]
    ModelAuth,
    #[error("model rate limit")]
    ModelRateLimit,
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
    ErrDbConstraint,
    ErrDbMigration,
    ErrNetworkTimeout,
    ErrParseFailed,
    ErrModelAuth,
    ErrModelRateLimit,
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
            AppError::ObjectNotFound => IpcErrorCode::ErrObjectNotFound,
            AppError::DbConstraint => IpcErrorCode::ErrDbConstraint,
            AppError::NetworkTimeout => IpcErrorCode::ErrNetworkTimeout,
            AppError::ParseFailed(_) => IpcErrorCode::ErrParseFailed,
            AppError::ModelAuth => IpcErrorCode::ErrModelAuth,
            AppError::ModelRateLimit => IpcErrorCode::ErrModelRateLimit,
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


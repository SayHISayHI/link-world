//! CLI result payload, stable error mapping and process exit codes.

use super::*;

#[derive(Debug)]
pub(super) struct CliSuccess {
    pub(super) command: &'static str,
    pub(super) data: Value,
}

#[derive(Debug)]
pub(super) struct CliError {
    pub(super) code: String,
    pub(super) message: String,
    pub(super) retryable: bool,
    pub(super) exit_code: i32,
    pub(super) correlation_id: Option<String>,
    pub(super) operation: Option<Value>,
}

impl CliError {
    pub(super) fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "ERR_INVALID_ARGUMENT".to_string(),
            message: message.into(),
            retryable: false,
            exit_code: 2,
            correlation_id: None,
            operation: None,
        }
    }

    pub(super) fn from_app(error: AppError) -> Self {
        let (code, message, retryable, exit_code) = match error {
            AppError::ObjectNotFound => ("ERR_OBJECT_NOT_FOUND", "Object was not found.", false, 3),
            AppError::JobNotFound => ("ERR_JOB_NOT_FOUND", "Job was not found.", false, 3),
            AppError::RuntimeBusy => (
                "ERR_RUNTIME_BUSY",
                "Another Node Tide process is using the local data directory.",
                true,
                5,
            ),
            AppError::PolicyDenied(_) => (
                "ERR_POLICY_DENIED",
                "The operation was denied by local policy.",
                false,
                4,
            ),
            AppError::PluginPermission(_) => (
                "ERR_PLUGIN_PERMISSION",
                "The operation was denied by plugin policy.",
                false,
                4,
            ),
            AppError::NetworkTimeout => (
                "ERR_NETWORK_TIMEOUT",
                "The network operation timed out.",
                true,
                6,
            ),
            AppError::ModelAuth => ("ERR_MODEL_AUTH", "Model authentication failed.", false, 6),
            AppError::ModelRateLimit => (
                "ERR_MODEL_RATE_LIMIT",
                "The model provider rate limit was reached.",
                true,
                6,
            ),
            AppError::ModelNotFound => (
                "ERR_MODEL_NOT_FOUND",
                "The model or provider endpoint was not found.",
                false,
                6,
            ),
            AppError::ModelOutputSchema(_) => (
                "ERR_MODEL_OUTPUT_SCHEMA",
                "The model returned an invalid structured result.",
                true,
                6,
            ),
            AppError::SecretStorage => (
                "ERR_SECRET_STORAGE",
                "The system credential store is unavailable.",
                false,
                7,
            ),
            AppError::BackupInvalid(_) => (
                "ERR_BACKUP_INVALID",
                "The backup is invalid or incomplete.",
                false,
                7,
            ),
            AppError::RestoreInvalid(_) => (
                "ERR_RESTORE_INVALID",
                "Restore state is invalid or incomplete.",
                false,
                7,
            ),
            AppError::DbMigration(_) => (
                "ERR_DB_MIGRATION",
                "The local database requires migration recovery.",
                false,
                7,
            ),
            AppError::DbConstraint => (
                "ERR_DB_CONSTRAINT",
                "The operation conflicts with existing local data.",
                false,
                7,
            ),
            AppError::Database(_) => (
                "ERR_DATABASE",
                "The local database operation failed.",
                true,
                7,
            ),
            AppError::Filesystem(_) => (
                "ERR_FILESYSTEM",
                "The local filesystem operation failed.",
                true,
                7,
            ),
            AppError::ParseFailed(_) => (
                "ERR_PARSE_FAILED",
                "The submitted content could not be parsed.",
                false,
                2,
            ),
            AppError::Unknown(_) => (
                "ERR_UNKNOWN",
                "An internal Node Tide operation failed.",
                false,
                10,
            ),
        };
        Self {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
            exit_code,
            correlation_id: None,
            operation: None,
        }
    }

    pub(super) fn terminal(
        code: String,
        message: &'static str,
        retryable: bool,
        exit_code: i32,
        correlation_id: Option<String>,
        operation: Value,
    ) -> Self {
        Self {
            code,
            message: message.to_string(),
            retryable,
            exit_code,
            correlation_id,
            operation: Some(operation),
        }
    }
}

pub(super) fn stable_cli_error_code(reason: Option<&str>, fallback: &str) -> String {
    let Some(prefix) = reason
        .and_then(|reason| reason.split_once(':').map(|(prefix, _)| prefix))
        .filter(|prefix| {
            !prefix.is_empty()
                && prefix.len() <= 96
                && prefix.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '.' | '_' | '-')
                })
        })
    else {
        return fallback.to_string();
    };
    format!(
        "ERR_{}",
        prefix
            .chars()
            .map(|character| match character {
                '.' | '-' => '_',
                other => other.to_ascii_uppercase(),
            })
            .collect::<String>()
    )
}

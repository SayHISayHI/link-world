use crate::errors::{AppError, AppResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const LOGS_DIR_NAME: &str = "logs";
// Legacy storage ABI: support bundles continue the existing bounded log stream.
const LOG_FILE_NAME: &str = "link-world.jsonl";
const ROTATED_LOG_FILE_NAME: &str = "link-world.jsonl.1";
const MAX_LOG_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_LOG_ENTRY_BYTES: usize = 4 * 1024;
const MAX_MESSAGE_CHARS: usize = 256;
const DEFAULT_READ_LIMIT: usize = 100;

#[derive(Debug, Clone)]
pub struct StructuredLogger {
    log_path: Arc<PathBuf>,
    write_lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone)]
pub struct StructuredLogEvent {
    level: &'static str,
    module: &'static str,
    event: &'static str,
    correlation_id: Option<String>,
    object_id: Option<String>,
    job_id: Option<String>,
    error_code: Option<String>,
    message: &'static str,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredLogEntry {
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    pub message: String,
}

impl StructuredLogEvent {
    pub fn info(module: &'static str, event: &'static str, message: &'static str) -> Self {
        Self::new("info", module, event, message)
    }

    pub fn error(module: &'static str, event: &'static str, message: &'static str) -> Self {
        Self::new("error", module, event, message)
    }

    fn new(
        level: &'static str,
        module: &'static str,
        event: &'static str,
        message: &'static str,
    ) -> Self {
        Self {
            level,
            module,
            event,
            correlation_id: None,
            object_id: None,
            job_id: None,
            error_code: None,
            message,
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    pub fn with_object_id(mut self, object_id: impl Into<String>) -> Self {
        self.object_id = Some(object_id.into());
        self
    }

    pub fn with_job_id(mut self, job_id: impl Into<String>) -> Self {
        self.job_id = Some(job_id.into());
        self
    }

    pub fn with_error_code(mut self, error_code: impl Into<String>) -> Self {
        self.error_code = Some(error_code.into());
        self
    }

    fn into_entry(self) -> AppResult<StructuredLogEntry> {
        let entry = StructuredLogEntry {
            timestamp: Utc::now().to_rfc3339(),
            level: self.level.to_string(),
            module: self.module.to_string(),
            event: self.event.to_string(),
            correlation_id: self.correlation_id,
            object_id: self.object_id,
            job_id: self.job_id,
            error_code: self.error_code,
            message: self.message.to_string(),
        };
        validate_entry(&entry)?;
        Ok(entry)
    }
}

impl StructuredLogger {
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            log_path: Arc::new(data_dir.as_ref().join(LOGS_DIR_NAME).join(LOG_FILE_NAME)),
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn path(&self) -> &Path {
        self.log_path.as_path()
    }

    pub async fn record(&self, event: StructuredLogEvent) -> AppResult<()> {
        let entry = event.into_entry()?;
        let mut line = serde_json::to_vec(&entry)
            .map_err(|_| AppError::Unknown("structured log serialization failed".to_string()))?;
        line.push(b'\n');
        if line.len() > MAX_LOG_ENTRY_BYTES {
            return Err(AppError::PolicyDenied(
                "structured log entry exceeds safety limit".to_string(),
            ));
        }

        let path = self.log_path.as_ref().clone();
        let write_lock = self.write_lock.clone();
        tokio::task::spawn_blocking(move || write_log_line(&path, &write_lock, &line))
            .await
            .map_err(|_| AppError::Filesystem("structured log writer stopped".to_string()))?
    }

    pub async fn read_recent(
        data_dir: impl AsRef<Path>,
        limit: Option<usize>,
    ) -> AppResult<Vec<StructuredLogEntry>> {
        let path = data_dir.as_ref().join(LOGS_DIR_NAME).join(LOG_FILE_NAME);
        let limit = limit.unwrap_or(DEFAULT_READ_LIMIT).clamp(1, 500);

        tokio::task::spawn_blocking(move || read_recent_entries(&path, limit))
            .await
            .map_err(|_| AppError::Filesystem("structured log reader stopped".to_string()))?
    }
}

fn write_log_line(path: &Path, write_lock: &Mutex<()>, line: &[u8]) -> AppResult<()> {
    let _guard = write_lock
        .lock()
        .map_err(|_| AppError::Filesystem("structured log lock is unavailable".to_string()))?;
    let parent = path.parent().ok_or_else(|| {
        AppError::Filesystem("structured log directory is unavailable".to_string())
    })?;
    fs::create_dir_all(parent)?;
    rotate_if_needed(path, line.len() as u64)?;

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line)?;
    Ok(())
}

fn rotate_if_needed(path: &Path, incoming_bytes: u64) -> AppResult<()> {
    let current_bytes = fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    if current_bytes.saturating_add(incoming_bytes) <= MAX_LOG_FILE_BYTES {
        return Ok(());
    }

    let parent = path.parent().ok_or_else(|| {
        AppError::Filesystem("structured log directory is unavailable".to_string())
    })?;
    let rotated_path = parent.join(ROTATED_LOG_FILE_NAME);
    if rotated_path.exists() {
        fs::remove_file(&rotated_path)?;
    }
    if path.exists() {
        fs::rename(path, rotated_path)?;
    }
    Ok(())
}

fn read_recent_entries(path: &Path, limit: usize) -> AppResult<Vec<StructuredLogEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)?;
    let mut entries = content
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<StructuredLogEntry>(line).ok())
        .filter(|entry| validate_entry(entry).is_ok())
        .take(limit)
        .collect::<Vec<_>>();
    entries.reverse();
    Ok(entries)
}

fn validate_entry(entry: &StructuredLogEntry) -> AppResult<()> {
    if !matches!(entry.level.as_str(), "info" | "warn" | "error") {
        return Err(unsafe_log_entry());
    }
    if chrono::DateTime::parse_from_rfc3339(&entry.timestamp).is_err()
        || !is_safe_label(&entry.module, 64)
        || !is_safe_label(&entry.event, 96)
        || entry
            .correlation_id
            .as_deref()
            .is_some_and(|value| !is_safe_identifier(value))
        || entry
            .object_id
            .as_deref()
            .is_some_and(|value| !is_safe_identifier(value))
        || entry
            .job_id
            .as_deref()
            .is_some_and(|value| !is_safe_identifier(value))
        || entry
            .error_code
            .as_deref()
            .is_some_and(|value| !is_safe_label(value, 96))
        || entry.message.chars().count() > MAX_MESSAGE_CHARS
        || contains_sensitive_text(&entry.message)
    {
        return Err(unsafe_log_entry());
    }

    Ok(())
}

fn is_safe_label(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-')
        })
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn contains_sensitive_text(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "http://",
        "https://",
        "keyring:",
        "authorization:",
        "api_key=",
        "apikey=",
        "token=",
        "cookie=",
        "session=",
        "\n",
        "\r",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn unsafe_log_entry() -> AppError {
    AppError::PolicyDenied("structured log entry violates redaction policy".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        rotate_if_needed, StructuredLogEvent, StructuredLogger, MAX_LOG_FILE_BYTES,
        ROTATED_LOG_FILE_NAME,
    };
    use crate::errors::AppError;
    use std::fs;
    use uuid::Uuid;

    #[tokio::test]
    async fn writes_and_reads_bounded_structured_log_entries() {
        let data_dir =
            std::env::temp_dir().join(format!("node-tide-structured-log-{}", Uuid::new_v4()));
        let logger = StructuredLogger::new(&data_dir);
        let correlation_id = Uuid::new_v4().to_string();

        logger
            .record(
                StructuredLogEvent::info(
                    "capture",
                    "capture.fetch.succeeded",
                    "Capture fetch completed successfully.",
                )
                .with_correlation_id(&correlation_id)
                .with_object_id("object-1")
                .with_job_id("job-1"),
            )
            .await
            .expect("structured log should write");

        let entries = StructuredLogger::read_recent(&data_dir, Some(10))
            .await
            .expect("structured log should read");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].correlation_id.as_deref(),
            Some(correlation_id.as_str())
        );
        assert_eq!(entries[0].event, "capture.fetch.succeeded");
        assert_eq!(entries[0].message, "Capture fetch completed successfully.");

        let serialized = fs::read_to_string(logger.path()).expect("log file should be readable");
        assert_eq!(serialized.lines().count(), 1);
        assert!(!serialized.contains("http"));
        assert!(!serialized.contains("token="));

        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn rotates_the_current_log_before_the_bound_is_exceeded() {
        let data_dir =
            std::env::temp_dir().join(format!("node-tide-log-rotation-{}", Uuid::new_v4()));
        let logger = StructuredLogger::new(&data_dir);
        let log_dir = logger.path().parent().expect("log path should have parent");
        fs::create_dir_all(log_dir).expect("log directory should initialize");
        fs::write(logger.path(), "current-log").expect("current log should write");
        fs::write(log_dir.join(ROTATED_LOG_FILE_NAME), "older-log")
            .expect("old rotated log should write");

        rotate_if_needed(logger.path(), MAX_LOG_FILE_BYTES)
            .expect("log should rotate before exceeding the bound");

        assert!(!logger.path().exists());
        assert_eq!(
            fs::read_to_string(log_dir.join(ROTATED_LOG_FILE_NAME))
                .expect("rotated log should be readable"),
            "current-log"
        );

        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn rejects_sensitive_or_unstructured_log_messages() {
        let data_dir = std::env::temp_dir().join(format!(
            "node-tide-structured-log-denied-{}",
            Uuid::new_v4()
        ));
        let logger = StructuredLogger::new(&data_dir);

        let error = logger
            .record(StructuredLogEvent::error(
                "capture",
                "capture.fetch.failed",
                "provider returned https://example.com/?token=secret",
            ))
            .await
            .expect_err("sensitive log message should be rejected");
        assert!(matches!(error, AppError::PolicyDenied(_)));
        assert!(!logger.path().exists());

        let _ = fs::remove_dir_all(data_dir);
    }
}

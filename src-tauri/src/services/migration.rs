use crate::domain::startup::StartupMigrationRecovery;
use crate::errors::{AppError, AppResult};
use crate::services::backup::BackupService;
use crate::storage::database::{Database, MigrationPlan};
use crate::storage::object_store::ObjectStore;
use crate::telemetry::{StructuredLogEvent, StructuredLogger};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

const MIGRATION_DIR_NAME: &str = "migration";
const PREPARED_MARKER_NAME: &str = "guard.prepared.json";
const RUNNING_MARKER_NAME: &str = "guard.running.json";
const LAST_RESULT_NAME: &str = "last-result.json";
const CONTROL_SCHEMA_VERSION: i64 = 1;
const MAX_CONTROL_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuardPhase {
    Prepared,
    Running,
}

impl GuardPhase {
    fn file_name(self) -> &'static str {
        match self {
            Self::Prepared => PREPARED_MARKER_NAME,
            Self::Running => RUNNING_MARKER_NAME,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationGuard {
    schema_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    correlation_id: Option<String>,
    backup_id: String,
    from_version: Option<i64>,
    target_version: i64,
    app_version: String,
    created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationResult {
    schema_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    correlation_id: Option<String>,
    backup_id: String,
    from_version: Option<i64>,
    target_version: i64,
    app_version: String,
    status: String,
    completed_at: String,
}

pub struct MigrationService;

impl MigrationService {
    pub async fn migrate_with_protection(
        database: &Database,
        object_store: &ObjectStore,
        data_dir: &Path,
        app_version: &str,
    ) -> AppResult<()> {
        Self::migrate_internal(database, object_store, data_dir, app_version, None).await
    }

    pub async fn migrate_with_logger(
        database: &Database,
        object_store: &ObjectStore,
        data_dir: &Path,
        app_version: &str,
        structured_logger: &StructuredLogger,
    ) -> AppResult<()> {
        Self::migrate_internal(
            database,
            object_store,
            data_dir,
            app_version,
            Some(structured_logger),
        )
        .await
    }

    async fn migrate_internal(
        database: &Database,
        object_store: &ObjectStore,
        data_dir: &Path,
        app_version: &str,
        structured_logger: Option<&StructuredLogger>,
    ) -> AppResult<()> {
        let migration_root = data_dir.join(MIGRATION_DIR_NAME);
        let generated_correlation_id = Uuid::new_v4().to_string();
        let pending_guard = match read_guard(&migration_root) {
            Ok(guard) => guard,
            Err(error) => {
                record_migration_log(
                    structured_logger,
                    StructuredLogEvent::info(
                        "migration",
                        "migration.started",
                        "Database migration started.",
                    )
                    .with_correlation_id(&generated_correlation_id),
                )
                .await;
                record_migration_log(
                    structured_logger,
                    StructuredLogEvent::error(
                        "migration",
                        "migration.failed",
                        "Database migration failed.",
                    )
                    .with_correlation_id(&generated_correlation_id)
                    .with_error_code("migration.guard_invalid"),
                )
                .await;
                return Err(error);
            }
        };
        let correlation_id = pending_guard
            .as_ref()
            .map(|(_, guard)| guard_correlation_id(guard, &generated_correlation_id))
            .unwrap_or(generated_correlation_id);
        let plan = match database.migration_plan().await {
            Ok(plan) => plan,
            Err(error) => {
                record_migration_log(
                    structured_logger,
                    StructuredLogEvent::info(
                        "migration",
                        "migration.started",
                        "Database migration started.",
                    )
                    .with_correlation_id(&correlation_id),
                )
                .await;
                record_migration_log(
                    structured_logger,
                    StructuredLogEvent::error(
                        "migration",
                        "migration.failed",
                        "Database migration failed.",
                    )
                    .with_correlation_id(&correlation_id)
                    .with_error_code("migration.plan_failed"),
                )
                .await;
                return Err(error);
            }
        };

        if !plan.requires_migration() && pending_guard.is_none() {
            return Ok(());
        }
        record_migration_log(
            structured_logger,
            StructuredLogEvent::info(
                "migration",
                "migration.started",
                "Database migration started.",
            )
            .with_correlation_id(&correlation_id),
        )
        .await;

        let result: AppResult<()> = async {
            if !plan.requires_migration() {
                if let Some((_phase, guard)) = pending_guard {
                    validate_guard(&guard)?;
                    database.validate_integrity().await.map_err(|error| {
                        AppError::DbMigration(format!(
                            "migration completed but integrity validation failed; verified restore point {} is available; {error}",
                            guard.backup_id
                        ))
                    })?;
                    complete_guard(&migration_root, guard);
                }
                return Ok(());
            }

            if !plan.requires_backup() {
                database.run_migrations().await?;
                database.validate_integrity().await?;
                return Ok(());
            }

            let backup_service = BackupService::new(
                database.pool().clone(),
                object_store.root().to_path_buf(),
                data_dir.join("backups"),
                app_version.to_string(),
            );

            let (phase, guard) = match pending_guard {
                Some((GuardPhase::Running, guard)) => {
                    validate_guard(&guard)?;
                    return Err(AppError::DbMigration(format!(
                        "a previous migration attempt did not complete; automatic retry is blocked; verified restore point {} is available",
                        guard.backup_id
                    )));
                }
                Some((GuardPhase::Prepared, guard)) => {
                    validate_guard_for_plan(&guard, &plan)?;
                    verify_guard_backup(&backup_service, &guard).await?;
                    (GuardPhase::Prepared, guard)
                }
                None => {
                    let guard = prepare_guard(
                        &backup_service,
                        &migration_root,
                        &plan,
                        app_version,
                        &correlation_id,
                    )
                    .await?;
                    (GuardPhase::Prepared, guard)
                }
            };

            record_migration_log(
                structured_logger,
                StructuredLogEvent::info(
                    "migration",
                    "migration.prepared",
                    "Database migration restore point is prepared.",
                )
                .with_correlation_id(&correlation_id),
            )
            .await;

            transition_guard(&migration_root, phase, GuardPhase::Running)?;
            record_migration_log(
                structured_logger,
                StructuredLogEvent::info(
                    "migration",
                    "migration.running",
                    "Database migration is running.",
                )
                .with_correlation_id(&correlation_id),
            )
            .await;

            let migration_result: AppResult<()> = async {
                database.run_migrations().await?;
                database.validate_integrity().await?;
                Ok(())
            }
            .await;

            if let Err(error) = migration_result {
                return Err(AppError::DbMigration(format!(
                    "migration failed; automatic retry is blocked; verified restore point {} is available; {error}",
                    guard.backup_id
                )));
            }

            complete_guard(&migration_root, guard);
            Ok(())
        }
        .await;

        match &result {
            Ok(()) => {
                record_migration_log(
                    structured_logger,
                    StructuredLogEvent::info(
                        "migration",
                        "migration.succeeded",
                        "Database migration succeeded.",
                    )
                    .with_correlation_id(&correlation_id),
                )
                .await;
            }
            Err(error) => {
                record_migration_log(
                    structured_logger,
                    StructuredLogEvent::error(
                        "migration",
                        "migration.failed",
                        "Database migration failed.",
                    )
                    .with_correlation_id(&correlation_id)
                    .with_error_code(migration_error_code(error)),
                )
                .await;
            }
        }

        result
    }

    pub fn recovery_info(data_dir: &Path) -> AppResult<Option<StartupMigrationRecovery>> {
        let migration_root = data_dir.join(MIGRATION_DIR_NAME);
        let Some((phase, guard)) = read_guard(&migration_root)? else {
            return Ok(None);
        };
        validate_guard(&guard)?;

        Ok(Some(StartupMigrationRecovery {
            phase: match phase {
                GuardPhase::Prepared => "prepared".to_string(),
                GuardPhase::Running => "running".to_string(),
            },
            backup_id: Some(guard.backup_id),
            from_version: guard.from_version,
            target_version: guard.target_version,
            app_version: guard.app_version,
            created_at: guard.created_at,
        }))
    }
}

async fn record_migration_log(
    structured_logger: Option<&StructuredLogger>,
    event: StructuredLogEvent,
) {
    if let Some(logger) = structured_logger {
        let _ = logger.record(event).await;
    }
}

fn migration_error_code(error: &AppError) -> &'static str {
    match error {
        AppError::DbMigration(message) if message.contains("automatic retry is blocked") => {
            "migration.retry_blocked"
        }
        _ => "migration.failed",
    }
}

fn guard_correlation_id(guard: &MigrationGuard, fallback: &str) -> String {
    guard
        .correlation_id
        .as_deref()
        .filter(|correlation_id| Uuid::parse_str(correlation_id).is_ok())
        .or_else(|| {
            Uuid::parse_str(&guard.backup_id)
                .ok()
                .map(|_| guard.backup_id.as_str())
        })
        .unwrap_or(fallback)
        .to_string()
}

async fn prepare_guard(
    backup_service: &BackupService,
    migration_root: &Path,
    plan: &MigrationPlan,
    app_version: &str,
    correlation_id: &str,
) -> AppResult<MigrationGuard> {
    fs::create_dir_all(migration_root)?;
    let summary = backup_service.create_backup().await?;
    let verification = backup_service.verify_backup(&summary.backup_id).await?;
    if !verification.valid {
        return Err(AppError::DbMigration(format!(
            "pre-migration restore point verification failed: {}",
            verification.issues.join("; ")
        )));
    }

    let target_version = plan.target_version().ok_or_else(|| {
        AppError::DbMigration("no target migration version is available".to_string())
    })?;
    let guard = MigrationGuard {
        schema_version: CONTROL_SCHEMA_VERSION,
        correlation_id: Some(correlation_id.to_string()),
        backup_id: summary.backup_id,
        from_version: plan.current_version(),
        target_version,
        app_version: app_version.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    write_new_json(&migration_root.join(PREPARED_MARKER_NAME), &guard)?;
    Ok(guard)
}

async fn verify_guard_backup(
    backup_service: &BackupService,
    guard: &MigrationGuard,
) -> AppResult<()> {
    let verification = backup_service.verify_backup(&guard.backup_id).await?;
    if verification.valid {
        Ok(())
    } else {
        Err(AppError::DbMigration(format!(
            "pre-migration restore point is no longer valid: {}",
            verification.issues.join("; ")
        )))
    }
}

fn validate_guard_for_plan(guard: &MigrationGuard, plan: &MigrationPlan) -> AppResult<()> {
    validate_guard(guard)?;
    if guard.from_version != plan.current_version()
        || Some(guard.target_version) != plan.target_version()
    {
        return Err(AppError::DbMigration(
            "prepared migration guard does not match the current database plan".to_string(),
        ));
    }
    Ok(())
}

fn validate_guard(guard: &MigrationGuard) -> AppResult<()> {
    if guard.schema_version != CONTROL_SCHEMA_VERSION {
        return Err(AppError::DbMigration(format!(
            "unsupported migration guard schema version: {}",
            guard.schema_version
        )));
    }
    if guard.backup_id.is_empty()
        || guard.backup_id.len() > 128
        || !guard
            .backup_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::DbMigration(
            "migration guard contains an invalid backup identifier".to_string(),
        ));
    }
    if guard
        .correlation_id
        .as_deref()
        .is_some_and(|correlation_id| Uuid::parse_str(correlation_id).is_err())
    {
        return Err(AppError::DbMigration(
            "migration guard contains an invalid correlation identifier".to_string(),
        ));
    }
    if guard.target_version <= 0 {
        return Err(AppError::DbMigration(
            "migration guard contains an invalid target version".to_string(),
        ));
    }
    Ok(())
}

fn read_guard(migration_root: &Path) -> AppResult<Option<(GuardPhase, MigrationGuard)>> {
    let mut found = Vec::new();
    for phase in [GuardPhase::Prepared, GuardPhase::Running] {
        let path = migration_root.join(phase.file_name());
        if path.exists() {
            found.push((phase, path));
        }
    }
    if found.len() > 1 {
        return Err(AppError::DbMigration(
            "multiple migration guard phase files exist".to_string(),
        ));
    }
    let Some((phase, path)) = found.pop() else {
        return Ok(None);
    };
    Ok(Some((phase, read_bounded_json(&path)?)))
}

fn transition_guard(migration_root: &Path, from: GuardPhase, to: GuardPhase) -> AppResult<()> {
    fs::rename(
        migration_root.join(from.file_name()),
        migration_root.join(to.file_name()),
    )?;
    Ok(())
}

fn complete_guard(migration_root: &Path, guard: MigrationGuard) {
    let _ = remove_file_if_exists(&migration_root.join(PREPARED_MARKER_NAME));
    let _ = remove_file_if_exists(&migration_root.join(RUNNING_MARKER_NAME));
    let _ = write_last_result(
        migration_root,
        MigrationResult {
            schema_version: CONTROL_SCHEMA_VERSION,
            correlation_id: guard.correlation_id,
            backup_id: guard.backup_id,
            from_version: guard.from_version,
            target_version: guard.target_version,
            app_version: guard.app_version,
            status: "succeeded".to_string(),
            completed_at: Utc::now().to_rfc3339(),
        },
    );
}

fn write_new_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| AppError::DbMigration(error.to_string()))?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_bounded_json<T: for<'de> Deserialize<'de>>(path: &Path) -> AppResult<T> {
    if fs::metadata(path)?.len() > MAX_CONTROL_BYTES {
        return Err(AppError::DbMigration(
            "migration guard exceeds size limit".to_string(),
        ));
    }
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|error| AppError::DbMigration(error.to_string()))
}

fn write_last_result(migration_root: &Path, result: MigrationResult) -> AppResult<()> {
    fs::create_dir_all(migration_root)?;
    let path = migration_root.join(LAST_RESULT_NAME);
    let temporary = migration_root.join(format!(".last-result-{}.tmp", Uuid::new_v4()));
    write_new_json(&temporary, &result)?;
    remove_file_if_exists(&path)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        prepare_guard, read_bounded_json, read_guard, transition_guard, GuardPhase,
        MigrationResult, MigrationService, PREPARED_MARKER_NAME, RUNNING_MARKER_NAME,
    };
    use crate::services::backup::BackupService;
    use crate::storage::database::{Database, DATABASE_FILE_NAME};
    use crate::storage::object_store::ObjectStore;
    use crate::telemetry::StructuredLogger;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::borrow::Cow;
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    static ALL_MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

    async fn v1_fixture() -> (PathBuf, Database, ObjectStore) {
        let data_dir =
            std::env::temp_dir().join(format!("link-world-guard-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&data_dir).expect("fixture directory should create");
        let path = data_dir.join(DATABASE_FILE_NAME);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .journal_mode(SqliteJournalMode::Wal),
            )
            .await
            .expect("fixture database should connect");
        let partial = Migrator {
            migrations: Cow::Owned(
                ALL_MIGRATIONS
                    .iter()
                    .filter(|migration| migration.version <= 1)
                    .cloned()
                    .collect(),
            ),
            ..Migrator::DEFAULT
        };
        partial.run(&pool).await.expect("v1 migration should apply");
        sqlx::query(
            "INSERT INTO knowledge_objects              (id, user_id, object_type, privacy_level, lifecycle_status)              VALUES ('guard-object', 'local-user', 'article', 'sensitive', 'captured')",
        )
        .execute(&pool)
        .await
        .expect("fixture object should seed");
        pool.close().await;

        let database = Database::connect_without_migrations(data_dir.clone())
            .await
            .expect("unmigrated fixture should reopen");
        let object_store =
            ObjectStore::initialize(data_dir.clone()).expect("object store should initialize");
        object_store
            .write_capture_snapshot("guard-object", "snapshot-1", b"pre-migration".to_vec())
            .await
            .expect("fixture object should write");
        (data_dir, database, object_store)
    }

    fn backup_service(
        database: &Database,
        object_store: &ObjectStore,
        data_dir: &Path,
    ) -> BackupService {
        BackupService::new(
            database.pool().clone(),
            object_store.root().to_path_buf(),
            data_dir.join("backups"),
            "0.1.0-test".to_string(),
        )
    }

    #[tokio::test]
    async fn existing_database_creates_verified_restore_point_before_migration() {
        let (data_dir, database, object_store) = v1_fixture().await;
        let structured_logger = StructuredLogger::new(&data_dir);
        MigrationService::migrate_with_logger(
            &database,
            &object_store,
            &data_dir,
            "0.1.0-test",
            &structured_logger,
        )
        .await
        .expect("protected migration should succeed");

        let plan = database
            .migration_plan()
            .await
            .expect("latest migration plan should inspect");
        assert!(!plan.requires_migration());
        let backups = backup_service(&database, &object_store, &data_dir)
            .list_backups()
            .await
            .expect("pre-migration backup should list");
        assert_eq!(backups.len(), 1);
        let verification = backup_service(&database, &object_store, &data_dir)
            .verify_backup(&backups[0].backup_id)
            .await
            .expect("pre-migration backup should verify");
        assert!(verification.valid, "{:?}", verification.issues);

        let backup_database = data_dir
            .join("backups")
            .join(&backups[0].backup_id)
            .join("database.sqlite3");
        let backup_pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(backup_database)
                    .create_if_missing(false),
            )
            .await
            .expect("backup database should open");
        let display_hints_columns: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('ai_analysis')              WHERE name = 'display_hints_json'",
        )
        .fetch_one(&backup_pool)
        .await
        .expect("backup schema should query");
        assert_eq!(display_hints_columns, 0);
        backup_pool.close().await;

        assert!(!data_dir
            .join("migration")
            .join(RUNNING_MARKER_NAME)
            .exists());
        let result_path = data_dir.join("migration").join("last-result.json");
        assert!(result_path.is_file());
        let migration_result: MigrationResult =
            read_bounded_json(&result_path).expect("migration result should read");
        let entries = StructuredLogger::read_recent(&data_dir, None)
            .await
            .expect("migration logs should read");
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec![
                "migration.started",
                "migration.prepared",
                "migration.running",
                "migration.succeeded",
            ]
        );
        let correlation_id = entries[0]
            .correlation_id
            .as_deref()
            .expect("migration correlation should exist");
        Uuid::parse_str(correlation_id).expect("migration correlation should be a UUID");
        assert!(entries.iter().all(|entry| {
            entry.correlation_id.as_deref() == Some(correlation_id) && entry.error_code.is_none()
        }));
        assert_eq!(
            migration_result.correlation_id.as_deref(),
            Some(correlation_id)
        );
        let raw_log =
            fs::read_to_string(structured_logger.path()).expect("migration log should be readable");
        assert!(!raw_log.contains(&backups[0].backup_id));
        assert!(!raw_log.contains(data_dir.to_string_lossy().as_ref()));
        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn fresh_database_migrates_without_creating_backup() {
        let data_dir =
            std::env::temp_dir().join(format!("link-world-fresh-test-{}", Uuid::new_v4()));
        let object_store =
            ObjectStore::initialize(data_dir.clone()).expect("object store should initialize");
        let database = Database::connect_without_migrations(data_dir.clone())
            .await
            .expect("fresh database should connect");

        MigrationService::migrate_with_protection(
            &database,
            &object_store,
            &data_dir,
            "0.1.0-test",
        )
        .await
        .expect("fresh migration should succeed");

        let backup_count = fs::read_dir(data_dir.join("backups"))
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0);
        assert_eq!(backup_count, 0);
        assert!(!database
            .migration_plan()
            .await
            .expect("fresh plan should inspect")
            .requires_migration());

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn interrupted_running_migration_blocks_automatic_retry() {
        let (data_dir, database, object_store) = v1_fixture().await;
        let service = backup_service(&database, &object_store, &data_dir);
        let plan = database
            .migration_plan()
            .await
            .expect("v1 plan should inspect");
        let migration_root = data_dir.join("migration");
        let correlation_id = Uuid::new_v4().to_string();
        let guard = prepare_guard(
            &service,
            &migration_root,
            &plan,
            "0.1.0-test",
            &correlation_id,
        )
        .await
        .expect("guard should prepare");
        transition_guard(&migration_root, GuardPhase::Prepared, GuardPhase::Running)
            .expect("guard should enter running phase");

        let structured_logger = StructuredLogger::new(&data_dir);
        let error = MigrationService::migrate_with_logger(
            &database,
            &object_store,
            &data_dir,
            "0.1.0-test",
            &structured_logger,
        )
        .await
        .expect_err("running guard with pending migrations must block retry");
        assert!(error.to_string().contains("automatic retry is blocked"));
        assert!(read_guard(&migration_root)
            .expect("guard should read")
            .is_some());

        let display_hints_columns: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('ai_analysis')              WHERE name = 'display_hints_json'",
        )
        .fetch_one(database.pool())
        .await
        .expect("v1 schema should query");
        assert_eq!(display_hints_columns, 0);

        let entries = StructuredLogger::read_recent(&data_dir, None)
            .await
            .expect("migration failure logs should read");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].event, "migration.started");
        assert_eq!(entries[1].event, "migration.failed");
        assert_eq!(
            entries[1].error_code.as_deref(),
            Some("migration.retry_blocked")
        );
        assert!(entries
            .iter()
            .all(|entry| entry.correlation_id.as_deref() == Some(correlation_id.as_str())));
        let raw_log = fs::read_to_string(structured_logger.path())
            .expect("migration failure log should be readable");
        assert!(!raw_log.contains(&guard.backup_id));
        assert!(!raw_log.contains(data_dir.to_string_lossy().as_ref()));

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn malformed_guard_emits_only_stable_failure_evidence() {
        let data_dir =
            std::env::temp_dir().join(format!("link-world-guard-invalid-test-{}", Uuid::new_v4()));
        let object_store =
            ObjectStore::initialize(data_dir.clone()).expect("object store should initialize");
        let database = Database::connect_without_migrations(data_dir.clone())
            .await
            .expect("fresh database should connect");
        let migration_root = data_dir.join("migration");
        fs::create_dir_all(&migration_root).expect("migration directory should create");
        let canary = "migration-guard-secret-canary";
        fs::write(
            migration_root.join(PREPARED_MARKER_NAME),
            format!("{{not-json-{canary}"),
        )
        .expect("malformed guard should write");
        let structured_logger = StructuredLogger::new(&data_dir);

        MigrationService::migrate_with_logger(
            &database,
            &object_store,
            &data_dir,
            "0.1.0-test",
            &structured_logger,
        )
        .await
        .expect_err("malformed guard must fail closed");

        let entries = StructuredLogger::read_recent(&data_dir, None)
            .await
            .expect("migration failure logs should read");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].event, "migration.started");
        assert_eq!(entries[1].event, "migration.failed");
        assert_eq!(
            entries[1].error_code.as_deref(),
            Some("migration.guard_invalid")
        );
        let correlation_id = entries[0]
            .correlation_id
            .as_deref()
            .expect("migration correlation should exist");
        Uuid::parse_str(correlation_id).expect("migration correlation should be a UUID");
        assert_eq!(entries[1].correlation_id.as_deref(), Some(correlation_id));
        let raw_log = fs::read_to_string(structured_logger.path())
            .expect("migration failure log should be readable");
        assert!(!raw_log.contains(canary));
        assert!(!raw_log.contains(data_dir.to_string_lossy().as_ref()));

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }
    #[tokio::test]
    async fn completed_migration_with_running_guard_finishes_on_next_startup() {
        let (data_dir, database, object_store) = v1_fixture().await;
        let service = backup_service(&database, &object_store, &data_dir);
        let plan = database
            .migration_plan()
            .await
            .expect("v1 plan should inspect");
        let migration_root = data_dir.join("migration");
        let guard = prepare_guard(
            &service,
            &migration_root,
            &plan,
            "0.1.0-test",
            &Uuid::new_v4().to_string(),
        )
        .await
        .expect("guard should prepare");
        transition_guard(&migration_root, GuardPhase::Prepared, GuardPhase::Running)
            .expect("guard should enter running phase");
        database
            .run_migrations()
            .await
            .expect("simulated migration should complete");

        MigrationService::migrate_with_protection(
            &database,
            &object_store,
            &data_dir,
            "0.1.0-test",
        )
        .await
        .expect("completed migration should converge");
        assert!(read_guard(&migration_root)
            .expect("guard should read")
            .is_none());
        assert!(migration_root.join("last-result.json").is_file());
        assert!(data_dir.join("backups").join(guard.backup_id).is_dir());

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }
}

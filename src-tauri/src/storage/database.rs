use crate::errors::{AppError, AppResult};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// Legacy storage ABI: keep this filename until an explicit data migration is shipped.
pub const DATABASE_FILE_NAME: &str = "link-world.sqlite3";
const MAX_SQLITE_CONNECTIONS: u32 = 5;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlan {
    pub applied_versions: Vec<i64>,
    pub pending_versions: Vec<i64>,
    pub has_user_schema: bool,
}

impl MigrationPlan {
    pub fn requires_migration(&self) -> bool {
        !self.pending_versions.is_empty()
    }

    pub fn requires_backup(&self) -> bool {
        self.has_user_schema && self.requires_migration()
    }

    pub fn current_version(&self) -> Option<i64> {
        self.applied_versions.last().copied()
    }

    pub fn target_version(&self) -> Option<i64> {
        MIGRATOR
            .iter()
            .filter(|migration| !migration.migration_type.is_down_migration())
            .map(|migration| migration.version)
            .max()
    }
}
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
    path: PathBuf,
}

impl Database {
    #[cfg(test)]
    pub async fn initialize(data_dir: PathBuf) -> AppResult<Self> {
        let database = Self::connect_without_migrations(data_dir).await?;
        if let Err(error) = database.run_migrations().await {
            database.pool.close().await;
            return Err(error);
        }
        Ok(database)
    }

    pub(crate) async fn connect_without_migrations(data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;
        let path = data_dir.join(DATABASE_FILE_NAME);
        let pool = SqlitePoolOptions::new()
            .max_connections(MAX_SQLITE_CONNECTIONS)
            .connect_with(sqlite_options(&path))
            .await?;
        Ok(Self { pool, path })
    }

    pub(crate) async fn migration_plan(&self) -> AppResult<MigrationPlan> {
        inspect_migration_plan(&self.pool).await
    }

    pub(crate) async fn run_migrations(&self) -> AppResult<()> {
        run_migrations(&self.pool).await
    }

    pub async fn initialize_in_memory() -> AppResult<Self> {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        run_migrations(&pool).await?;

        Ok(Self {
            pool,
            path: PathBuf::from(":memory:"),
        })
    }

    pub async fn validate_restore_candidate(path: &Path) -> AppResult<()> {
        let options = sqlite_options(path).create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;

        let result: AppResult<()> = async {
            run_migrations(&pool).await?;
            validate_pool_integrity(&pool).await?;
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
                .execute(&pool)
                .await?;
            Ok(())
        }
        .await;
        pool.close().await;
        result
    }

    pub async fn validate_integrity(&self) -> AppResult<()> {
        validate_pool_integrity(&self.pool).await
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn sqlite_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
}

async fn inspect_migration_plan(pool: &SqlitePool) -> AppResult<MigrationPlan> {
    let has_migrations_table: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;
    let user_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name != '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await?;

    let known_migrations = MIGRATOR
        .iter()
        .filter(|migration| !migration.migration_type.is_down_migration())
        .collect::<Vec<_>>();
    let mut applied_versions = Vec::new();

    if has_migrations_table != 0 {
        let applied: Vec<(i64, bool, Vec<u8>)> = sqlx::query_as(
            "SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(pool)
        .await?;

        for (version, success, checksum) in applied {
            if !success {
                return Err(AppError::DbMigration(format!(
                    "migration {version} is marked as incomplete"
                )));
            }
            let migration = known_migrations
                .iter()
                .find(|migration| migration.version == version)
                .ok_or_else(|| {
                    AppError::DbMigration(format!(
                        "database contains unknown migration version {version}"
                    ))
                })?;
            if migration.checksum.as_ref() != checksum.as_slice() {
                return Err(AppError::DbMigration(format!(
                    "migration {version} checksum does not match this application"
                )));
            }
            applied_versions.push(version);
        }
    }

    let applied = applied_versions.iter().copied().collect::<BTreeSet<_>>();
    let pending_versions = known_migrations
        .iter()
        .filter(|migration| !applied.contains(&migration.version))
        .map(|migration| migration.version)
        .collect();

    Ok(MigrationPlan {
        applied_versions,
        pending_versions,
        has_user_schema: user_table_count != 0,
    })
}

async fn run_migrations(pool: &SqlitePool) -> AppResult<()> {
    MIGRATOR
        .run(pool)
        .await
        .map_err(|error| AppError::DbMigration(error.to_string()))
}

async fn validate_pool_integrity(pool: &SqlitePool) -> AppResult<()> {
    let quick_check: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(pool)
        .await?;
    if quick_check != "ok" {
        return Err(AppError::RestoreInvalid(format!(
            "database quick_check returned: {quick_check}"
        )));
    }

    let foreign_key_violations: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_foreign_key_check")
            .fetch_one(pool)
            .await?;
    if foreign_key_violations != 0 {
        return Err(AppError::RestoreInvalid(format!(
            "database contains {foreign_key_violations} foreign key violations"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod migration_tests;

#[cfg(test)]
mod tests {
    use super::{Database, MIGRATOR};

    #[test]
    fn embeds_released_v1_migration_checksum() {
        let migration = MIGRATOR
            .iter()
            .find(|migration| migration.version == 1)
            .expect("migration 1 should be embedded");
        let checksum = migration
            .checksum
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();

        assert_eq!(
            checksum,
            "600E0AD7A91121BF11028D0CF6CA50D3A8055212F993484A67096D02676D114C395C5D226B53965049634576D9C9C5BE"
        );
    }

    #[tokio::test]
    async fn initializes_in_memory_database_with_core_tables() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");

        let table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'knowledge_objects'",
        )
        .fetch_one(database.pool())
        .await
        .expect("schema query should succeed");

        assert_eq!(table_count, 1);

        let display_hints_column_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('ai_analysis') WHERE name = 'display_hints_json'",
        )
        .fetch_one(database.pool())
        .await
        .expect("AI analysis migration should be queryable");
        assert_eq!(display_hints_column_count, 1);

        let api_family_column_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('model_provider_configs') WHERE name = 'api_family'",
        )
        .fetch_one(database.pool())
        .await
        .expect("model provider protocol migration should be queryable");
        assert_eq!(api_family_column_count, 1);
    }
}

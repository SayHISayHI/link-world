use crate::errors::{AppError, AppResult};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

pub const DATABASE_FILE_NAME: &str = "link-world.sqlite3";
const MAX_SQLITE_CONNECTIONS: u32 = 5;

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
    path: PathBuf,
}

impl Database {
    pub async fn initialize(data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&data_dir)?;

        let path = data_dir.join(DATABASE_FILE_NAME);
        let options = sqlite_options(&path);
        let pool = SqlitePoolOptions::new()
            .max_connections(MAX_SQLITE_CONNECTIONS)
            .connect_with(options)
            .await?;

        if let Err(error) = run_migrations(&pool).await {
            pool.close().await;
            return Err(error);
        }

        Ok(Self { pool, path })
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

async fn run_migrations(pool: &SqlitePool) -> AppResult<()> {
    sqlx::migrate!("./migrations")
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
mod tests {
    use super::Database;

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

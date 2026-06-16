use crate::errors::{AppError, AppResult};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

const DATABASE_FILE_NAME: &str = "link-world.sqlite3";
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

        run_migrations(&pool).await?;

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
    }
}

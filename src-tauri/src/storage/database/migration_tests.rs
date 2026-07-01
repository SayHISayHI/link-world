use super::{sqlite_options, Database, DATABASE_FILE_NAME};
use crate::errors::AppError;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

static ALL_MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

fn migrator_through(version: i64) -> Migrator {
    Migrator {
        migrations: Cow::Owned(
            ALL_MIGRATIONS
                .iter()
                .filter(|migration| migration.version <= version)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    }
}

async fn historical_database(version: i64) -> (PathBuf, SqlitePool) {
    let data_dir =
        std::env::temp_dir().join(format!("link-world-migration-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&data_dir).expect("migration fixture directory should create");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlite_options(&data_dir.join(DATABASE_FILE_NAME)))
        .await
        .expect("historical database should connect");
    migrator_through(version)
        .run(&pool)
        .await
        .expect("historical migrations should apply");
    (data_dir, pool)
}

async fn seed_v1_release_fixture(pool: &SqlitePool) {
    let mut transaction = pool
        .begin()
        .await
        .expect("fixture transaction should start");
    let mut objects = QueryBuilder::<Sqlite>::new(
        "INSERT INTO knowledge_objects          (id, user_id, object_type, title, privacy_level, lifecycle_status, failure_reason) ",
    );
    objects.push_values(0..1000, |mut row, index| {
        let privacy_level = match index {
            0 => "sensitive",
            1 => "secret",
            _ => "personal",
        };
        let lifecycle_status = if index == 2 { "failed" } else { "parsed" };
        let failure_reason = (index == 2).then(|| "legacy fetch failed".to_string());
        row.push_bind(format!("object-{index:04}"))
            .push_bind("local-user")
            .push_bind("article")
            .push_bind(format!("Legacy object {index}"))
            .push_bind(privacy_level)
            .push_bind(lifecycle_status)
            .push_bind(failure_reason);
    });
    objects
        .build()
        .execute(&mut *transaction)
        .await
        .expect("objects should seed");

    sqlx::query(
        "INSERT INTO source_snapshots          (id, object_id, snapshot_type, storage_uri, content_hash, parser_id, parser_version)          VALUES ('snapshot-1', 'object-0000', 'html',          'local://objects/object-0000/snapshot-1.html', 'snapshot-hash', 'legacy-parser', '1')",
    )
    .execute(&mut *transaction)
    .await
    .expect("snapshot should seed");
    sqlx::query(
        "INSERT INTO parsed_documents          (id, object_id, source_snapshot_id, title, text_content, markdown_content, language,          word_count, content_hash, parser_id, parser_version)          VALUES ('document-1', 'object-0000', 'snapshot-1', 'Legacy document',          'legacy migration fixture content', '# Legacy document', 'en', 4,          'document-hash', 'legacy-parser', '1')",
    )
    .execute(&mut *transaction)
    .await
    .expect("parsed document should seed");
    sqlx::query(
        "INSERT INTO ai_analysis          (id, object_id, parsed_document_id, analysis_type, schema_version, summary, tags_json)          VALUES ('analysis-1', 'object-0000', 'document-1', 'general', 1,          'legacy summary', '[\"legacy\"]')",
    )
    .execute(&mut *transaction)
    .await
    .expect("analysis should seed");
    sqlx::query(
        "INSERT INTO ai_traces          (id, analysis_id, object_id, provider, model, capability, input_parsed_document_id,          input_hash, output_hash, prompt_tokens, completion_tokens)          VALUES ('trace-1', 'analysis-1', 'object-0000', 'legacy-provider', 'legacy-model',          'chat', 'document-1', 'input-hash', 'output-hash', 10, 5)",
    )
    .execute(&mut *transaction)
    .await
    .expect("trace should seed");
    sqlx::query(
        "INSERT INTO evaluation_runs          (id, object_id, evaluator_type, evaluator_version, status, evidence_json,          limitations_json, next_actions_json, verdict)          VALUES ('evaluation-1', 'object-0000', 'generic', '1', 'passed', '[]', '[]', '[]',          'useful')",
    )
    .execute(&mut *transaction)
    .await
    .expect("evaluation should seed");
    sqlx::query(
        "INSERT INTO evaluation_artifacts          (id, evaluation_run_id, artifact_type, storage_uri, content_hash)          VALUES ('artifact-1', 'evaluation-1', 'report',          'local://objects/object-0000/evaluations/evaluation-1/artifact-1.json',          'artifact-hash')",
    )
    .execute(&mut *transaction)
    .await
    .expect("artifact should seed");
    sqlx::query(
        "INSERT INTO knowledge_fts          (object_id, parsed_document_id, title, author, content, ai_summary)          VALUES ('object-0000', 'document-1', 'Legacy document', 'Legacy author',          'legacy migration fixture content', 'legacy summary')",
    )
    .execute(&mut *transaction)
    .await
    .expect("FTS row should seed");
    sqlx::query(
        "INSERT INTO background_jobs          (id, job_type, status, object_id, payload_json, attempt_count, max_attempts, last_error)          VALUES ('job-1', 'capture.fetch_url', 'failed', 'object-0002', '{}', 3, 3,          'legacy timeout')",
    )
    .execute(&mut *transaction)
    .await
    .expect("failed job should seed");
    sqlx::query(
        "INSERT INTO model_provider_configs          (id, provider, chat_base_url, default_chat_model, capabilities_json, secret_ref, enabled)          VALUES ('provider-1', 'openai-compatible', 'https://example.invalid/v1',          'legacy-model', '[\"chat\"]', 'memory:model_provider:legacy:api_key', 1)",
    )
    .execute(&mut *transaction)
    .await
    .expect("provider should seed");
    sqlx::query(
        "INSERT INTO deletion_tombstones          (id, object_id, user_id, deletion_mode, purge_status, reason)          VALUES ('tombstone-1', 'deleted-object', 'local-user', 'purge', 'pending',          'legacy deletion')",
    )
    .execute(&mut *transaction)
    .await
    .expect("tombstone should seed");
    sqlx::query(
        "INSERT INTO local_settings (key, value_json)          VALUES ('legacy.setting', '{\"enabled\":true}')",
    )
    .execute(&mut *transaction)
    .await
    .expect("setting should seed");

    transaction
        .commit()
        .await
        .expect("fixture transaction should commit");
}

async fn table_count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .expect("fixture table should be queryable")
}

async fn cleanup(data_dir: PathBuf, pool: &SqlitePool) {
    pool.close().await;
    let _ = fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn upgrades_v1_release_fixture_without_losing_core_or_derived_data() {
    let (data_dir, historical_pool) = historical_database(1).await;
    seed_v1_release_fixture(&historical_pool).await;
    historical_pool.close().await;

    let database = Database::initialize(data_dir.clone())
        .await
        .expect("v1 fixture should migrate to latest");
    database
        .validate_integrity()
        .await
        .expect("upgraded v1 fixture should be valid");
    let pool = database.pool();

    assert_eq!(table_count(pool, "knowledge_objects").await, 1000);
    for table in [
        "source_snapshots",
        "parsed_documents",
        "ai_analysis",
        "ai_traces",
        "evaluation_runs",
        "evaluation_artifacts",
        "background_jobs",
        "model_provider_configs",
        "deletion_tombstones",
        "local_settings",
    ] {
        assert_eq!(table_count(pool, table).await, 1, "{table} row was lost");
    }

    assert_eq!(table_count(pool, "evaluation_traces").await, 0);

    let privacy_levels: Vec<String> = sqlx::query_scalar(
        "SELECT privacy_level FROM knowledge_objects          WHERE id IN ('object-0000', 'object-0001') ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .expect("privacy levels should query");
    assert_eq!(privacy_levels, vec!["sensitive", "secret"]);

    let display_hints: Option<String> =
        sqlx::query_scalar("SELECT display_hints_json FROM ai_analysis WHERE id = 'analysis-1'")
            .fetch_one(pool)
            .await
            .expect("legacy display hints should query");
    assert_eq!(display_hints, None);

    let api_family: String =
        sqlx::query_scalar("SELECT api_family FROM model_provider_configs WHERE id = 'provider-1'")
            .fetch_one(pool)
            .await
            .expect("provider family should query");
    assert_eq!(api_family, "openai_chat_completions");

    let fts_matches: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_fts WHERE knowledge_fts MATCH 'legacy'")
            .fetch_one(pool)
            .await
            .expect("FTS fixture should remain searchable");
    assert_eq!(fts_matches, 1);

    let applied_versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .expect("migration metadata should query");
    assert_eq!(applied_versions, vec![1, 2, 3, 4, 5, 6]);
    let evaluation_contract: (Option<String>, Option<String>, Option<String>, i64, i64, i64) =
        sqlx::query_as(
            "SELECT request_id, correlation_id, retry_of_run_id, plan_schema_version, input_schema_version, output_schema_version FROM evaluation_runs WHERE id = 'evaluation-1'",
        )
        .fetch_one(pool)
        .await
        .expect("legacy evaluation contract should query");
    assert_eq!(evaluation_contract, (None, None, None, 1, 1, 1));

    cleanup(data_dir, pool).await;
}

#[tokio::test]
async fn upgrades_v2_fixture_preserving_display_hints_and_provider_identity() {
    let (data_dir, historical_pool) = historical_database(2).await;
    sqlx::query(
        "INSERT INTO knowledge_objects          (id, user_id, object_type, privacy_level, lifecycle_status)          VALUES ('object-v2', 'local-user', 'article', 'personal', 'enriched')",
    )
    .execute(&historical_pool)
    .await
    .expect("v2 object should seed");
    let display_hints = r#"{"schemaVersion":1,"mode":"article","confidence":0.9}"#;
    sqlx::query(
        "INSERT INTO ai_analysis          (id, object_id, analysis_type, schema_version, summary, display_hints_json)          VALUES ('analysis-v2', 'object-v2', 'general', 2, 'v2 summary', ?1)",
    )
    .bind(display_hints)
    .execute(&historical_pool)
    .await
    .expect("v2 analysis should seed");
    sqlx::query(
        "INSERT INTO model_provider_configs          (id, provider, capabilities_json, enabled)          VALUES ('provider-v2', 'custom-provider', '[\"chat\"]', 1)",
    )
    .execute(&historical_pool)
    .await
    .expect("v2 provider should seed");
    historical_pool.close().await;

    let database = Database::initialize(data_dir.clone())
        .await
        .expect("v2 fixture should migrate to latest");
    let stored_hints: String =
        sqlx::query_scalar("SELECT display_hints_json FROM ai_analysis WHERE id = 'analysis-v2'")
            .fetch_one(database.pool())
            .await
            .expect("display hints should query");
    assert_eq!(stored_hints, display_hints);
    let provider: (String, String) = sqlx::query_as(
        "SELECT provider, api_family FROM model_provider_configs WHERE id = 'provider-v2'",
    )
    .fetch_one(database.pool())
    .await
    .expect("provider should query");
    assert_eq!(
        provider,
        (
            "custom-provider".to_string(),
            "openai_chat_completions".to_string()
        )
    );

    cleanup(data_dir, database.pool()).await;
}

#[tokio::test]
async fn v3_fixture_adds_evaluation_runtime_contract_and_preserves_api_family() {
    let (data_dir, historical_pool) = historical_database(3).await;
    sqlx::query(
        "INSERT INTO model_provider_configs          (id, provider, api_family, capabilities_json, enabled)          VALUES ('provider-v3', 'anthropic', 'anthropic_messages', '[\"chat\"]', 1)",
    )
    .execute(&historical_pool)
    .await
    .expect("v3 provider should seed");
    historical_pool.close().await;

    let database = Database::initialize(data_dir.clone())
        .await
        .expect("current fixture should reopen");
    let api_family: String = sqlx::query_scalar(
        "SELECT api_family FROM model_provider_configs WHERE id = 'provider-v3'",
    )
    .fetch_one(database.pool())
    .await
    .expect("v3 provider should query");
    assert_eq!(api_family, "anthropic_messages");
    assert_eq!(table_count(database.pool(), "_sqlx_migrations").await, 6);
    assert_eq!(table_count(database.pool(), "evaluation_traces").await, 0);
    let evaluation_columns: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('evaluation_runs') WHERE name IN ('request_id', 'correlation_id', 'plan_schema_version', 'input_schema_version', 'output_schema_version')",
    )
    .fetch_one(database.pool())
    .await
    .expect("evaluation runtime columns should query");
    assert_eq!(evaluation_columns, 5);

    cleanup(data_dir, database.pool()).await;
}

#[tokio::test]
async fn rejects_database_from_unknown_future_migration_without_rewriting_user_data() {
    let (data_dir, historical_pool) = historical_database(3).await;
    sqlx::query(
        "INSERT INTO knowledge_objects          (id, user_id, object_type, privacy_level, lifecycle_status)          VALUES ('future-object', 'local-user', 'note', 'personal', 'captured')",
    )
    .execute(&historical_pool)
    .await
    .expect("future object should seed");
    sqlx::query(
        "INSERT INTO _sqlx_migrations          (version, description, success, checksum, execution_time)          VALUES (999, 'future schema', 1, ?1, 0)",
    )
    .bind(vec![0_u8; 48])
    .execute(&historical_pool)
    .await
    .expect("future migration metadata should seed");
    historical_pool.close().await;

    let database = Database::connect_without_migrations(data_dir.clone())
        .await
        .expect("future fixture should reopen without migration");
    let error = database
        .migration_plan()
        .await
        .expect_err("unknown future migration must be rejected");
    assert!(matches!(error, AppError::DbMigration(_)));

    let inspection_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlite_options(&data_dir.join(DATABASE_FILE_NAME)))
        .await
        .expect("rejected fixture should remain readable");
    assert_eq!(table_count(&inspection_pool, "knowledge_objects").await, 1);
    assert_eq!(table_count(&inspection_pool, "_sqlx_migrations").await, 4);

    cleanup(data_dir, &inspection_pool).await;
}

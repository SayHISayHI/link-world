use crate::domain::knowledge::KnowledgeObject;
use crate::domain::search::SearchResult;
use crate::errors::{AppError, AppResult};
use serde_json::json;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

const DEFAULT_SEARCH_LIMIT: i64 = 20;
const MAX_SEARCH_LIMIT: i64 = 50;
const SEARCH_REINDEX_JOB_TYPE: &str = "search.reindex_object";

#[derive(Debug, Clone)]
pub struct SearchRepository {
    pool: SqlitePool,
}

impl SearchRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn search_hybrid(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> AppResult<Vec<SearchResult>> {
        let Some(fts_query) = build_fts_query(query) else {
            return Ok(Vec::new());
        };
        let terms = normalized_terms(query);
        let limit = clamp_limit(limit);
        let rows = sqlx::query(
            r#"
            SELECT
                objects.id,
                objects.user_id,
                objects.object_type,
                objects.title,
                objects.canonical_url,
                objects.source_platform,
                objects.author,
                objects.privacy_level,
                objects.lifecycle_status,
                objects.failure_reason,
                objects.captured_at,
                objects.updated_at,
                fts.title AS indexed_title,
                fts.author AS indexed_author,
                fts.content AS indexed_content,
                fts.ai_summary AS indexed_ai_summary,
                CASE
                    WHEN objects.privacy_level = 'secret' THEN NULL
                    ELSE snippet(knowledge_fts, -1, '[', ']', '...', 16)
                END AS snippet,
                bm25(knowledge_fts, 0.0, 0.0, 8.0, 3.0, 1.0, 4.0) AS rank
            FROM knowledge_fts AS fts
            INNER JOIN knowledge_objects AS objects ON objects.id = fts.object_id
            WHERE knowledge_fts MATCH ?1
              AND objects.lifecycle_status != 'deleted'
            ORDER BY rank ASC, objects.updated_at DESC
            LIMIT ?2
            "#,
        )
        .bind(fts_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(map_search_error)?;

        Ok(rows
            .into_iter()
            .map(|row| search_result_from_row(row, &terms))
            .collect())
    }

    pub async fn rebuild_index_with_job(&self, job_id: &str, now: &str) -> AppResult<i64> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM knowledge_fts")
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"
            INSERT INTO knowledge_fts (
                object_id,
                parsed_document_id,
                title,
                author,
                content,
                ai_summary
            )
            SELECT
                objects.id,
                parsed.id,
                COALESCE(NULLIF(parsed.title, ''), objects.title, objects.canonical_url, objects.id),
                objects.author,
                parsed.text_content,
                (
                    SELECT analysis.summary
                    FROM ai_analysis AS analysis
                    WHERE analysis.object_id = objects.id
                    ORDER BY analysis.created_at DESC
                    LIMIT 1
                )
            FROM knowledge_objects AS objects
            INNER JOIN parsed_documents AS parsed ON parsed.id = (
                SELECT latest.id
                FROM parsed_documents AS latest
                WHERE latest.object_id = objects.id
                ORDER BY latest.created_at DESC
                LIMIT 1
            )
            WHERE objects.lifecycle_status != 'deleted'
            "#,
        )
        .execute(&mut *tx)
        .await?;

        let indexed_objects =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT object_id) FROM knowledge_fts")
                .fetch_one(&mut *tx)
                .await?;

        Self::insert_reindex_job(
            &mut tx,
            job_id,
            None,
            json!({
                "scope": "all",
                "indexedObjects": indexed_objects,
            })
            .to_string(),
            now,
        )
        .await?;

        tx.commit().await?;
        Ok(indexed_objects)
    }

    pub async fn reindex_object_with_job(
        &self,
        object_id: &str,
        job_id: &str,
        now: &str,
    ) -> AppResult<bool> {
        let mut tx = self.pool.begin().await?;
        let active_object_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM knowledge_objects WHERE id = ?1 AND lifecycle_status != 'deleted'",
        )
        .bind(object_id)
        .fetch_one(&mut *tx)
        .await?;

        if active_object_count == 0 {
            return Err(AppError::ObjectNotFound);
        }

        let indexed = Self::reindex_object(&mut tx, object_id).await?;
        Self::insert_reindex_job(
            &mut tx,
            job_id,
            Some(object_id),
            json!({
                "scope": "object",
                "objectId": object_id,
                "indexed": indexed,
            })
            .to_string(),
            now,
        )
        .await?;

        tx.commit().await?;
        Ok(indexed)
    }

    pub async fn reindex_object(
        tx: &mut Transaction<'_, Sqlite>,
        object_id: &str,
    ) -> AppResult<bool> {
        Self::delete_object_index(tx, object_id).await?;

        sqlx::query(
            r#"
            INSERT INTO knowledge_fts (
                object_id,
                parsed_document_id,
                title,
                author,
                content,
                ai_summary
            )
            SELECT
                objects.id,
                parsed.id,
                COALESCE(NULLIF(parsed.title, ''), objects.title, objects.canonical_url, objects.id),
                objects.author,
                parsed.text_content,
                (
                    SELECT analysis.summary
                    FROM ai_analysis AS analysis
                    WHERE analysis.object_id = objects.id
                    ORDER BY analysis.created_at DESC
                    LIMIT 1
                )
            FROM knowledge_objects AS objects
            INNER JOIN parsed_documents AS parsed ON parsed.id = (
                SELECT latest.id
                FROM parsed_documents AS latest
                WHERE latest.object_id = objects.id
                ORDER BY latest.created_at DESC
                LIMIT 1
            )
            WHERE objects.id = ?1
              AND objects.lifecycle_status != 'deleted'
            "#,
        )
        .bind(object_id)
        .execute(&mut **tx)
        .await?;

        let indexed_rows: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM knowledge_fts WHERE object_id = ?1")
                .bind(object_id)
                .fetch_one(&mut **tx)
                .await?;

        Ok(indexed_rows > 0)
    }

    pub async fn delete_object_index(
        tx: &mut Transaction<'_, Sqlite>,
        object_id: &str,
    ) -> AppResult<()> {
        sqlx::query("DELETE FROM knowledge_fts WHERE object_id = ?1")
            .bind(object_id)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    async fn insert_reindex_job(
        tx: &mut Transaction<'_, Sqlite>,
        job_id: &str,
        object_id: Option<&str>,
        payload_json: String,
        now: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO background_jobs (
                id,
                job_type,
                status,
                object_id,
                payload_json,
                attempt_count,
                max_attempts,
                last_error,
                created_at,
                updated_at
            ) VALUES (?1, ?2, 'succeeded', ?3, ?4, 1, 1, NULL, ?5, ?5)
            "#,
        )
        .bind(job_id)
        .bind(SEARCH_REINDEX_JOB_TYPE)
        .bind(object_id)
        .bind(payload_json)
        .bind(now)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

fn search_result_from_row(row: SqliteRow, terms: &[String]) -> SearchResult {
    let rank = row.get::<f64, _>("rank");

    SearchResult {
        object: KnowledgeObject {
            id: row.get("id"),
            user_id: row.get("user_id"),
            object_type: row.get("object_type"),
            title: row.get("title"),
            canonical_url: row.get("canonical_url"),
            source_platform: row.get("source_platform"),
            author: row.get("author"),
            privacy_level: row.get("privacy_level"),
            lifecycle_status: row.get("lifecycle_status"),
            failure_reason: row.get("failure_reason"),
            captured_at: row.get("captured_at"),
            updated_at: row.get("updated_at"),
        },
        matched_fields: matched_fields(&row, terms),
        snippet: row.get("snippet"),
        score: (-rank).max(0.0),
    }
}

fn matched_fields(row: &SqliteRow, terms: &[String]) -> Vec<String> {
    let fields = [
        ("title", row.get::<Option<String>, _>("indexed_title")),
        ("author", row.get::<Option<String>, _>("indexed_author")),
        ("content", row.get::<Option<String>, _>("indexed_content")),
        (
            "aiSummary",
            row.get::<Option<String>, _>("indexed_ai_summary"),
        ),
    ];

    fields
        .into_iter()
        .filter_map(|(name, value)| {
            let value = value?.to_lowercase();
            if terms.iter().any(|term| value.contains(term)) {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn build_fts_query(query: &str) -> Option<String> {
    let tokens = query
        .split_whitespace()
        .map(|token| token.trim_matches(is_ignored_query_edge))
        .filter(|token| !token.is_empty())
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        return None;
    }

    Some(tokens.join(" OR "))
}

fn normalized_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|token| token.trim_matches(is_ignored_query_edge).to_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn is_ignored_query_edge(character: char) -> bool {
    character.is_ascii_punctuation() && !matches!(character, '_' | '-')
}

fn clamp_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT)
}

fn map_search_error(error: sqlx::Error) -> AppError {
    match error {
        sqlx::Error::Database(database_error) => {
            AppError::ParseFailed(format!("invalid search query: {database_error}"))
        }
        other => AppError::Database(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::SearchRepository;
    use crate::storage::database::Database;

    #[tokio::test]
    async fn search_hits_parsed_content_and_ai_summary() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(database.pool()).await;
        reindex(database.pool(), "obj-search").await;
        let repository = SearchRepository::new(database.pool().clone());

        let content_results = repository
            .search_hybrid("durable workflows", Some(10))
            .await
            .expect("content search should work");

        assert_eq!(content_results.len(), 1);
        assert_eq!(content_results[0].object.id, "obj-search");
        assert!(content_results[0]
            .matched_fields
            .iter()
            .any(|field| field == "content"));

        sqlx::query(
            r#"
            INSERT INTO ai_analysis (
                id, object_id, analysis_type, schema_version, summary, tags_json, key_points_json,
                claims_json, action_items_json, risks_json, created_at
            ) VALUES (
                'analysis-search', 'obj-search', 'general_summary', 1, 'Traceable summaries improve recall.',
                '[]', '[]', '[]', '[]', '[]', '2026-06-17T00:00:01Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("analysis should insert");
        reindex(database.pool(), "obj-search").await;

        let summary_results = repository
            .search_hybrid("Traceable summaries", Some(10))
            .await
            .expect("summary search should work");

        assert_eq!(summary_results.len(), 1);
        assert!(summary_results[0]
            .matched_fields
            .iter()
            .any(|field| field == "aiSummary"));
    }

    #[tokio::test]
    async fn search_ranks_title_matches_above_repeated_content_matches() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_custom_searchable_object(
            database.pool(),
            "obj-title-rank",
            "Priority Alpha",
            "Short body without the ranking term.",
            "personal",
        )
        .await;
        seed_custom_searchable_object(
            database.pool(),
            "obj-content-rank",
            "Background Notes",
            "priority priority priority priority priority content-only match",
            "personal",
        )
        .await;
        reindex(database.pool(), "obj-title-rank").await;
        reindex(database.pool(), "obj-content-rank").await;
        let repository = SearchRepository::new(database.pool().clone());

        let results = repository
            .search_hybrid("priority", Some(10))
            .await
            .expect("weighted search should work");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].object.id, "obj-title-rank");
        assert!(results[0]
            .matched_fields
            .iter()
            .any(|field| field == "title"));
    }

    #[tokio::test]
    async fn search_suppresses_secret_object_snippets() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_custom_searchable_object(
            database.pool(),
            "obj-secret-search",
            "Private Launch Plan",
            "private launch code alpha should never appear in snippets",
            "secret",
        )
        .await;
        reindex(database.pool(), "obj-secret-search").await;
        let repository = SearchRepository::new(database.pool().clone());

        let results = repository
            .search_hybrid("private launch", Some(10))
            .await
            .expect("secret object search should work");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].object.id, "obj-secret-search");
        assert!(results[0].snippet.is_none());
        assert!(results[0]
            .matched_fields
            .iter()
            .any(|field| field == "content" || field == "title"));
    }

    #[tokio::test]
    async fn deleted_object_is_removed_from_search_index() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(database.pool()).await;
        reindex(database.pool(), "obj-search").await;

        let mut tx = database.pool().begin().await.expect("tx should begin");
        sqlx::query("UPDATE knowledge_objects SET lifecycle_status = 'deleted' WHERE id = ?1")
            .bind("obj-search")
            .execute(&mut *tx)
            .await
            .expect("object should update");
        SearchRepository::delete_object_index(&mut tx, "obj-search")
            .await
            .expect("index should delete");
        tx.commit().await.expect("tx should commit");

        let repository = SearchRepository::new(database.pool().clone());
        let results = repository
            .search_hybrid("durable", Some(10))
            .await
            .expect("search should work");

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn rebuild_index_recreates_search_rows_and_records_job() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(database.pool()).await;
        seed_unparsed_object(database.pool(), "obj-unparsed").await;
        seed_deleted_searchable_object(database.pool()).await;

        let repository = SearchRepository::new(database.pool().clone());
        let indexed_objects = repository
            .rebuild_index_with_job("job-rebuild-search", "2026-06-17T00:00:02Z")
            .await
            .expect("index should rebuild");

        assert_eq!(indexed_objects, 1);

        let durable_results = repository
            .search_hybrid("durable workflows", Some(10))
            .await
            .expect("active object should be searchable");
        let deleted_results = repository
            .search_hybrid("deleted marker", Some(10))
            .await
            .expect("deleted object search should work");
        let job: (String, Option<String>, String) = sqlx::query_as(
            r#"
            SELECT status, object_id, payload_json
            FROM background_jobs
            WHERE id = 'job-rebuild-search'
            "#,
        )
        .fetch_one(database.pool())
        .await
        .expect("job should be readable");

        assert_eq!(durable_results.len(), 1);
        assert!(deleted_results.is_empty());
        assert_eq!(job.0, "succeeded");
        assert!(job.1.is_none());
        assert!(job.2.contains("\"indexedObjects\":1"));
    }

    #[tokio::test]
    async fn reindex_object_with_job_reports_if_object_has_searchable_document() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(database.pool()).await;
        seed_unparsed_object(database.pool(), "obj-unparsed").await;
        let repository = SearchRepository::new(database.pool().clone());

        let indexed = repository
            .reindex_object_with_job("obj-search", "job-reindex-object", "2026-06-17T00:00:02Z")
            .await
            .expect("parsed object should reindex");
        let unparsed_indexed = repository
            .reindex_object_with_job(
                "obj-unparsed",
                "job-reindex-unparsed",
                "2026-06-17T00:00:03Z",
            )
            .await
            .expect("unparsed object should produce an empty index");
        let job_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM background_jobs WHERE job_type = 'search.reindex_object'",
        )
        .fetch_one(database.pool())
        .await
        .expect("job count should be readable");

        assert!(indexed);
        assert!(!unparsed_indexed);
        assert_eq!(job_count, 2);
    }

    #[tokio::test]
    async fn reindex_object_with_job_rejects_missing_or_deleted_objects() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_deleted_searchable_object(database.pool()).await;
        let repository = SearchRepository::new(database.pool().clone());

        let missing_error = repository
            .reindex_object_with_job("obj-missing", "job-missing", "2026-06-17T00:00:02Z")
            .await
            .expect_err("missing object should fail");
        let deleted_error = repository
            .reindex_object_with_job("obj-deleted", "job-deleted", "2026-06-17T00:00:03Z")
            .await
            .expect_err("deleted object should fail");

        assert!(matches!(
            missing_error,
            crate::errors::AppError::ObjectNotFound
        ));
        assert!(matches!(
            deleted_error,
            crate::errors::AppError::ObjectNotFound
        ));
    }

    async fn seed_searchable_object(pool: &sqlx::SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, author, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (
                'obj-search', 'local', 'article', 'Workflow Notes', 'Author', 'personal', 'parsed',
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("object should insert");

        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash, parser_id, parser_version, created_at
            ) VALUES (
                'parsed-search', 'obj-search', 'Workflow Notes',
                'Durable workflows need retries, evidence and clear status transitions.',
                8, 'hash-search', 'test.parser', '0.1.0', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("parsed document should insert");
    }

    async fn seed_custom_searchable_object(
        pool: &sqlx::SqlitePool,
        object_id: &str,
        title: &str,
        text_content: &str,
        privacy_level: &str,
    ) {
        let parsed_id = format!("parsed-{object_id}");
        let hash = format!("hash-{object_id}");

        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, author, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (
                ?1, 'local', 'article', ?2, 'Author', ?3, 'parsed',
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .bind(object_id)
        .bind(title)
        .bind(privacy_level)
        .execute(pool)
        .await
        .expect("custom object should insert");

        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash, parser_id, parser_version, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, 8, ?5, 'test.parser', '0.1.0', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .bind(parsed_id)
        .bind(object_id)
        .bind(title)
        .bind(text_content)
        .bind(hash)
        .execute(pool)
        .await
        .expect("custom parsed document should insert");
    }

    async fn seed_unparsed_object(pool: &sqlx::SqlitePool, object_id: &str) {
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, author, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (
                ?1, 'local', 'article', 'Unparsed Notes', 'Author', 'personal', 'captured',
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .bind(object_id)
        .execute(pool)
        .await
        .expect("unparsed object should insert");
    }

    async fn seed_deleted_searchable_object(pool: &sqlx::SqlitePool) {
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, author, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (
                'obj-deleted', 'local', 'article', 'Deleted Notes', 'Author', 'personal', 'deleted',
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("deleted object should insert");

        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash, parser_id, parser_version, created_at
            ) VALUES (
                'parsed-deleted', 'obj-deleted', 'Deleted Notes',
                'Deleted marker content must not appear in rebuilt search index.',
                10, 'hash-deleted', 'test.parser', '0.1.0', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .execute(pool)
        .await
        .expect("deleted parsed document should insert");
    }

    async fn reindex(pool: &sqlx::SqlitePool, object_id: &str) {
        let mut tx = pool.begin().await.expect("tx should begin");
        SearchRepository::reindex_object(&mut tx, object_id)
            .await
            .expect("object should reindex");
        tx.commit().await.expect("tx should commit");
    }
}

use crate::domain::knowledge::KnowledgeObject;
use crate::domain::search::{SearchIndexHealthResponse, SearchResult};
use crate::errors::{AppError, AppResult};
use serde_json::json;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

const DEFAULT_SEARCH_LIMIT: i64 = 20;
const MAX_SEARCH_LIMIT: i64 = 50;
const SEARCH_REINDEX_JOB_TYPE: &str = "search.reindex_object";
const MAX_HEALTH_SAMPLE_IDS: i64 = 20;

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
        filter_type: Option<String>,
    ) -> AppResult<Vec<SearchResult>> {
        let Some(fts_query) = build_fts_query(query) else {
            return Ok(Vec::new());
        };
        let terms = normalized_terms(query);
        let limit = clamp_limit(limit);
        let filter_type = normalize_filter(filter_type);
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
              AND (
                ?3 IS NULL
                OR (?3 = 'inbox' AND objects.lifecycle_status IN ('captured', 'parsed'))
                OR (?3 = 'failed' AND objects.lifecycle_status = 'failed')
                OR objects.object_type = ?3
              )
            ORDER BY rank ASC, objects.updated_at DESC
            LIMIT ?2
            "#,
        )
        .bind(fts_query)
        .bind(limit)
        .bind(filter_type)
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

    pub async fn check_index_health(&self) -> AppResult<SearchIndexHealthResponse> {
        let expected_indexed_objects = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM knowledge_objects AS objects
            WHERE objects.lifecycle_status != 'deleted'
              AND EXISTS (
                SELECT 1
                FROM parsed_documents AS parsed
                WHERE parsed.object_id = objects.id
              )
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let actual_indexed_rows =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM knowledge_fts")
                .fetch_one(&self.pool)
                .await?;

        let missing_object_ids = sqlx::query_scalar::<_, String>(
            r#"
            WITH latest_parsed AS (
                SELECT
                    objects.id AS object_id,
                    (
                        SELECT parsed.id
                        FROM parsed_documents AS parsed
                        WHERE parsed.object_id = objects.id
                        ORDER BY parsed.created_at DESC, parsed.id DESC
                        LIMIT 1
                    ) AS parsed_document_id
                FROM knowledge_objects AS objects
                WHERE objects.lifecycle_status != 'deleted'
            )
            SELECT latest_parsed.object_id
            FROM latest_parsed
            LEFT JOIN knowledge_fts AS fts ON fts.object_id = latest_parsed.object_id
            WHERE latest_parsed.parsed_document_id IS NOT NULL
              AND fts.object_id IS NULL
            ORDER BY latest_parsed.object_id
            LIMIT ?1
            "#,
        )
        .bind(MAX_HEALTH_SAMPLE_IDS)
        .fetch_all(&self.pool)
        .await?;

        let stale_object_ids = sqlx::query_scalar::<_, String>(
            r#"
            WITH latest_parsed AS (
                SELECT
                    objects.id AS object_id,
                    (
                        SELECT parsed.id
                        FROM parsed_documents AS parsed
                        WHERE parsed.object_id = objects.id
                        ORDER BY parsed.created_at DESC, parsed.id DESC
                        LIMIT 1
                    ) AS parsed_document_id
                FROM knowledge_objects AS objects
                WHERE objects.lifecycle_status != 'deleted'
            )
            SELECT DISTINCT latest_parsed.object_id
            FROM latest_parsed
            INNER JOIN knowledge_fts AS fts ON fts.object_id = latest_parsed.object_id
            WHERE latest_parsed.parsed_document_id IS NOT NULL
              AND fts.parsed_document_id != latest_parsed.parsed_document_id
            ORDER BY latest_parsed.object_id
            LIMIT ?1
            "#,
        )
        .bind(MAX_HEALTH_SAMPLE_IDS)
        .fetch_all(&self.pool)
        .await?;

        let orphaned_object_ids = sqlx::query_scalar::<_, String>(
            r#"
            WITH latest_parsed AS (
                SELECT
                    objects.id AS object_id,
                    (
                        SELECT parsed.id
                        FROM parsed_documents AS parsed
                        WHERE parsed.object_id = objects.id
                        ORDER BY parsed.created_at DESC, parsed.id DESC
                        LIMIT 1
                    ) AS parsed_document_id
                FROM knowledge_objects AS objects
                WHERE objects.lifecycle_status != 'deleted'
            )
            SELECT DISTINCT fts.object_id
            FROM knowledge_fts AS fts
            LEFT JOIN latest_parsed ON latest_parsed.object_id = fts.object_id
            WHERE latest_parsed.parsed_document_id IS NULL
            ORDER BY fts.object_id
            LIMIT ?1
            "#,
        )
        .bind(MAX_HEALTH_SAMPLE_IDS)
        .fetch_all(&self.pool)
        .await?;

        let duplicate_object_ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT object_id
            FROM knowledge_fts
            GROUP BY object_id
            HAVING COUNT(*) > 1
            ORDER BY object_id
            LIMIT ?1
            "#,
        )
        .bind(MAX_HEALTH_SAMPLE_IDS)
        .fetch_all(&self.pool)
        .await?;

        let missing_objects = count_missing_index_rows(&self.pool).await?;
        let stale_objects = count_stale_index_rows(&self.pool).await?;
        let orphaned_rows = count_orphaned_index_rows(&self.pool).await?;
        let duplicate_rows = count_duplicate_index_rows(&self.pool).await?;

        Ok(SearchIndexHealthResponse {
            healthy: missing_objects == 0
                && stale_objects == 0
                && orphaned_rows == 0
                && duplicate_rows == 0,
            expected_indexed_objects,
            actual_indexed_rows,
            missing_objects,
            stale_objects,
            orphaned_rows,
            duplicate_rows,
            missing_object_ids,
            stale_object_ids,
            orphaned_object_ids,
            duplicate_object_ids,
        })
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

fn normalize_filter(filter_type: Option<String>) -> Option<String> {
    filter_type
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "all")
}

async fn count_missing_index_rows(pool: &SqlitePool) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        r#"
        WITH latest_parsed AS (
            SELECT
                objects.id AS object_id,
                (
                    SELECT parsed.id
                    FROM parsed_documents AS parsed
                    WHERE parsed.object_id = objects.id
                    ORDER BY parsed.created_at DESC, parsed.id DESC
                    LIMIT 1
                ) AS parsed_document_id
            FROM knowledge_objects AS objects
            WHERE objects.lifecycle_status != 'deleted'
        )
        SELECT COUNT(*)
        FROM latest_parsed
        LEFT JOIN knowledge_fts AS fts ON fts.object_id = latest_parsed.object_id
        WHERE latest_parsed.parsed_document_id IS NOT NULL
          AND fts.object_id IS NULL
        "#,
    )
    .fetch_one(pool)
    .await?)
}

async fn count_stale_index_rows(pool: &SqlitePool) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        r#"
        WITH latest_parsed AS (
            SELECT
                objects.id AS object_id,
                (
                    SELECT parsed.id
                    FROM parsed_documents AS parsed
                    WHERE parsed.object_id = objects.id
                    ORDER BY parsed.created_at DESC, parsed.id DESC
                    LIMIT 1
                ) AS parsed_document_id
            FROM knowledge_objects AS objects
            WHERE objects.lifecycle_status != 'deleted'
        )
        SELECT COUNT(DISTINCT latest_parsed.object_id)
        FROM latest_parsed
        INNER JOIN knowledge_fts AS fts ON fts.object_id = latest_parsed.object_id
        WHERE latest_parsed.parsed_document_id IS NOT NULL
          AND fts.parsed_document_id != latest_parsed.parsed_document_id
        "#,
    )
    .fetch_one(pool)
    .await?)
}

async fn count_orphaned_index_rows(pool: &SqlitePool) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        r#"
        WITH latest_parsed AS (
            SELECT
                objects.id AS object_id,
                (
                    SELECT parsed.id
                    FROM parsed_documents AS parsed
                    WHERE parsed.object_id = objects.id
                    ORDER BY parsed.created_at DESC, parsed.id DESC
                    LIMIT 1
                ) AS parsed_document_id
            FROM knowledge_objects AS objects
            WHERE objects.lifecycle_status != 'deleted'
        )
        SELECT COUNT(*)
        FROM knowledge_fts AS fts
        LEFT JOIN latest_parsed ON latest_parsed.object_id = fts.object_id
        WHERE latest_parsed.parsed_document_id IS NULL
        "#,
    )
    .fetch_one(pool)
    .await?)
}

async fn count_duplicate_index_rows(pool: &SqlitePool) -> AppResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COALESCE(SUM(row_count - 1), 0)
        FROM (
            SELECT COUNT(*) AS row_count
            FROM knowledge_fts
            GROUP BY object_id
            HAVING COUNT(*) > 1
        )
        "#,
    )
    .fetch_one(pool)
    .await?)
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
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn search_hits_parsed_content_and_ai_summary() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(database.pool()).await;
        reindex(database.pool(), "obj-search").await;
        let repository = SearchRepository::new(database.pool().clone());

        let content_results = repository
            .search_hybrid("durable workflows", Some(10), None)
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
            .search_hybrid("Traceable summaries", Some(10), None)
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
            "article",
            "Priority Alpha",
            "Short body without the ranking term.",
            "personal",
            "parsed",
        )
        .await;
        seed_custom_searchable_object(
            database.pool(),
            "obj-content-rank",
            "article",
            "Background Notes",
            "priority priority priority priority priority content-only match",
            "personal",
            "parsed",
        )
        .await;
        reindex(database.pool(), "obj-title-rank").await;
        reindex(database.pool(), "obj-content-rank").await;
        let repository = SearchRepository::new(database.pool().clone());

        let results = repository
            .search_hybrid("priority", Some(10), None)
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
            "article",
            "Private Launch Plan",
            "private launch code alpha should never appear in snippets",
            "secret",
            "parsed",
        )
        .await;
        reindex(database.pool(), "obj-secret-search").await;
        let repository = SearchRepository::new(database.pool().clone());

        let results = repository
            .search_hybrid("private launch", Some(10), None)
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
    async fn search_respects_type_lifecycle_and_inbox_filters() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_custom_searchable_object(
            database.pool(),
            "obj-article-filter",
            "article",
            "Article Filter",
            "shared filter marker in an article",
            "personal",
            "parsed",
        )
        .await;
        seed_custom_searchable_object(
            database.pool(),
            "obj-github-filter",
            "github_repo",
            "GitHub Filter",
            "shared filter marker in a repository",
            "personal",
            "parsed",
        )
        .await;
        seed_custom_searchable_object(
            database.pool(),
            "obj-failed-filter",
            "article",
            "Failed Filter",
            "shared filter marker in a failed object",
            "personal",
            "failed",
        )
        .await;
        reindex(database.pool(), "obj-article-filter").await;
        reindex(database.pool(), "obj-github-filter").await;
        reindex(database.pool(), "obj-failed-filter").await;
        let repository = SearchRepository::new(database.pool().clone());

        let github_results = repository
            .search_hybrid(
                "shared filter marker",
                Some(10),
                Some("github_repo".to_string()),
            )
            .await
            .expect("type-filtered search should work");
        let failed_results = repository
            .search_hybrid("shared filter marker", Some(10), Some("failed".to_string()))
            .await
            .expect("failed-filtered search should work");
        let inbox_results = repository
            .search_hybrid("shared filter marker", Some(10), Some("inbox".to_string()))
            .await
            .expect("inbox-filtered search should work");

        assert_eq!(
            github_results
                .iter()
                .map(|result| result.object.id.as_str())
                .collect::<Vec<_>>(),
            vec!["obj-github-filter"]
        );
        assert_eq!(
            failed_results
                .iter()
                .map(|result| result.object.id.as_str())
                .collect::<Vec<_>>(),
            vec!["obj-failed-filter"]
        );
        assert_eq!(inbox_results.len(), 2);
        assert!(inbox_results
            .iter()
            .all(|result| result.object.lifecycle_status == "parsed"));
    }

    #[tokio::test]
    async fn search_index_health_reports_healthy_index() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_searchable_object(database.pool()).await;
        reindex(database.pool(), "obj-search").await;
        let repository = SearchRepository::new(database.pool().clone());

        let health = repository
            .check_index_health()
            .await
            .expect("health check should work");

        assert!(health.healthy);
        assert_eq!(health.expected_indexed_objects, 1);
        assert_eq!(health.actual_indexed_rows, 1);
        assert_eq!(health.missing_objects, 0);
        assert_eq!(health.stale_objects, 0);
        assert_eq!(health.orphaned_rows, 0);
    }

    #[tokio::test]
    async fn search_index_health_reports_missing_stale_and_orphaned_rows() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        seed_custom_searchable_object(
            database.pool(),
            "obj-missing-index",
            "article",
            "Missing Index",
            "missing health marker",
            "personal",
            "parsed",
        )
        .await;
        seed_custom_searchable_object(
            database.pool(),
            "obj-stale-index",
            "article",
            "Stale Index",
            "stale health marker old",
            "personal",
            "parsed",
        )
        .await;
        seed_custom_searchable_object(
            database.pool(),
            "obj-orphan-index",
            "article",
            "Orphan Index",
            "orphan health marker",
            "personal",
            "parsed",
        )
        .await;
        reindex(database.pool(), "obj-stale-index").await;
        reindex(database.pool(), "obj-orphan-index").await;

        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash, parser_id, parser_version, created_at
            ) VALUES (
                'parsed-obj-stale-index-new', 'obj-stale-index', 'Stale Index',
                'stale health marker new latest document',
                6, 'hash-stale-new', 'test.parser', '0.1.0', '2026-06-17T00:01:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("new parsed document should insert");
        sqlx::query("UPDATE knowledge_objects SET lifecycle_status = 'deleted' WHERE id = ?1")
            .bind("obj-orphan-index")
            .execute(database.pool())
            .await
            .expect("orphan fixture object should be marked deleted");
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
                object_id,
                parsed_document_id,
                title,
                author,
                content,
                ai_summary
            FROM knowledge_fts
            WHERE object_id = 'obj-stale-index'
            "#,
        )
        .execute(database.pool())
        .await
        .expect("duplicate FTS row should insert");

        let repository = SearchRepository::new(database.pool().clone());
        let health = repository
            .check_index_health()
            .await
            .expect("health check should detect inconsistencies");

        assert!(!health.healthy);
        assert_eq!(health.expected_indexed_objects, 2);
        assert_eq!(health.actual_indexed_rows, 3);
        assert_eq!(health.missing_objects, 1);
        assert_eq!(health.stale_objects, 1);
        assert_eq!(health.orphaned_rows, 1);
        assert_eq!(health.duplicate_rows, 1);
        assert_eq!(health.missing_object_ids, vec!["obj-missing-index"]);
        assert_eq!(health.stale_object_ids, vec!["obj-stale-index"]);
        assert_eq!(health.orphaned_object_ids, vec!["obj-orphan-index"]);
        assert_eq!(health.duplicate_object_ids, vec!["obj-stale-index"]);
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
            .search_hybrid("durable", Some(10), None)
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
            .search_hybrid("durable workflows", Some(10), None)
            .await
            .expect("active object should be searchable");
        let deleted_results = repository
            .search_hybrid("deleted marker", Some(10), None)
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

    #[tokio::test]
    async fn search_benchmark_fixture_supports_repeatable_corpus() {
        let report = run_search_benchmark(250).await;

        assert_eq!(report.object_count, 250);
        assert_eq!(report.indexed_objects, 250);
        assert_eq!(report.health_expected_objects, 250);
        assert_eq!(report.health_actual_rows, 250);
        assert!(report.health_healthy);
        assert!(report.queries.iter().all(|query| query.result_count > 0));
    }

    #[tokio::test]
    #[ignore]
    async fn search_benchmark_5k_objects_reports_budget() {
        let report = run_search_benchmark(5_000).await;
        eprintln!("{}", report.summary());
        report.assert_within_query_budget(Duration::from_millis(250));
    }

    #[tokio::test]
    #[ignore]
    async fn search_benchmark_20k_objects_reports_budget() {
        let report = run_search_benchmark(20_000).await;
        eprintln!("{}", report.summary());
        report.assert_within_query_budget(Duration::from_millis(500));
    }

    #[derive(Debug)]
    struct SearchBenchmarkReport {
        object_count: usize,
        indexed_objects: i64,
        health_expected_objects: i64,
        health_actual_rows: i64,
        health_healthy: bool,
        seed_elapsed: Duration,
        rebuild_elapsed: Duration,
        queries: Vec<SearchBenchmarkQuery>,
    }

    impl SearchBenchmarkReport {
        fn max_query_elapsed(&self) -> Duration {
            self.queries
                .iter()
                .map(|query| query.elapsed)
                .max()
                .unwrap_or_default()
        }

        fn assert_within_query_budget(&self, budget: Duration) {
            assert!(
                self.health_healthy,
                "benchmark corpus should leave a healthy search index"
            );
            assert_eq!(
                self.indexed_objects, self.object_count as i64,
                "all benchmark objects should be indexed"
            );
            assert!(
                self.queries.iter().all(|query| query.result_count > 0),
                "all benchmark queries should return at least one result: {}",
                self.summary()
            );
            assert!(
                self.max_query_elapsed() <= budget,
                "max search query elapsed {:?} exceeded budget {:?}: {}",
                self.max_query_elapsed(),
                budget,
                self.summary()
            );
        }

        fn summary(&self) -> String {
            let queries = self
                .queries
                .iter()
                .map(|query| {
                    format!(
                        "{} query=\"{}\" filter={} results={} elapsed={}ms",
                        query.name,
                        query.query,
                        query.filter_type.unwrap_or("all"),
                        query.result_count,
                        query.elapsed.as_millis()
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");

            format!(
                "search benchmark objects={} indexed={} seed={}ms rebuild={}ms max_query={}ms queries=[{}]",
                self.object_count,
                self.indexed_objects,
                self.seed_elapsed.as_millis(),
                self.rebuild_elapsed.as_millis(),
                self.max_query_elapsed().as_millis(),
                queries
            )
        }
    }

    #[derive(Debug)]
    struct SearchBenchmarkQuery {
        name: &'static str,
        query: &'static str,
        filter_type: Option<&'static str>,
        result_count: usize,
        elapsed: Duration,
    }

    async fn run_search_benchmark(object_count: usize) -> SearchBenchmarkReport {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");

        let seed_started = Instant::now();
        seed_benchmark_objects(database.pool(), object_count).await;
        let seed_elapsed = seed_started.elapsed();

        let repository = SearchRepository::new(database.pool().clone());
        let rebuild_started = Instant::now();
        let indexed_objects = repository
            .rebuild_index_with_job(
                &format!("job-search-benchmark-{object_count}"),
                "2026-06-17T00:00:02Z",
            )
            .await
            .expect("benchmark search index should rebuild");
        let rebuild_elapsed = rebuild_started.elapsed();

        let health = repository
            .check_index_health()
            .await
            .expect("benchmark search index health should be readable");

        let queries = vec![
            run_benchmark_query(&repository, "common", "durable workflow", None).await,
            run_benchmark_query(&repository, "rare", "priority alpha", None).await,
            run_benchmark_query(
                &repository,
                "type-filter",
                "repository benchmark",
                Some("github_repo"),
            )
            .await,
            run_benchmark_query(&repository, "summary", "retention capsule", None).await,
            run_benchmark_query(
                &repository,
                "failed-filter",
                "failure boundary",
                Some("failed"),
            )
            .await,
        ];

        SearchBenchmarkReport {
            object_count,
            indexed_objects,
            health_expected_objects: health.expected_indexed_objects,
            health_actual_rows: health.actual_indexed_rows,
            health_healthy: health.healthy,
            seed_elapsed,
            rebuild_elapsed,
            queries,
        }
    }

    async fn run_benchmark_query(
        repository: &SearchRepository,
        name: &'static str,
        query: &'static str,
        filter_type: Option<&'static str>,
    ) -> SearchBenchmarkQuery {
        let started = Instant::now();
        let results = repository
            .search_hybrid(query, Some(20), filter_type.map(str::to_string))
            .await
            .expect("benchmark query should work");
        let elapsed = started.elapsed();

        SearchBenchmarkQuery {
            name,
            query,
            filter_type,
            result_count: results.len(),
            elapsed,
        }
    }

    async fn seed_benchmark_objects(pool: &sqlx::SqlitePool, object_count: usize) {
        let mut tx = pool.begin().await.expect("benchmark tx should begin");

        for index in 0..object_count {
            let object_id = format!("bench-object-{index:05}");
            let parsed_id = format!("bench-parsed-{index:05}");
            let analysis_id = format!("bench-analysis-{index:05}");
            let object_type = benchmark_object_type(index);
            let lifecycle_status = benchmark_lifecycle_status(index);
            let privacy_level = benchmark_privacy_level(index);
            let title = benchmark_title(index, object_type);
            let text_content = benchmark_text(index, object_type, lifecycle_status);
            let word_count = text_content.split_whitespace().count() as i64;
            let content_hash = format!("bench-hash-{index:05}");
            let captured_at = format!("2026-06-17T00:{:02}:{:02}Z", (index / 60) % 60, index % 60);
            let canonical_url = format!("https://example.invalid/benchmark/{index:05}");

            sqlx::query(
                r#"
                INSERT INTO knowledge_objects (
                    id,
                    user_id,
                    object_type,
                    title,
                    canonical_url,
                    source_platform,
                    author,
                    privacy_level,
                    lifecycle_status,
                    captured_at,
                    updated_at
                ) VALUES (
                    ?1, 'local', ?2, ?3, ?4, 'benchmark', ?5, ?6, ?7, ?8, ?8
                )
                "#,
            )
            .bind(&object_id)
            .bind(object_type)
            .bind(&title)
            .bind(canonical_url)
            .bind(format!("Benchmark Author {}", index % 17))
            .bind(privacy_level)
            .bind(lifecycle_status)
            .bind(&captured_at)
            .execute(&mut *tx)
            .await
            .expect("benchmark object should insert");

            sqlx::query(
                r#"
                INSERT INTO parsed_documents (
                    id,
                    object_id,
                    title,
                    text_content,
                    word_count,
                    content_hash,
                    parser_id,
                    parser_version,
                    created_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, 'benchmark.parser', '0.1.0', ?7
                )
                "#,
            )
            .bind(parsed_id)
            .bind(&object_id)
            .bind(title)
            .bind(text_content)
            .bind(word_count)
            .bind(content_hash)
            .bind(&captured_at)
            .execute(&mut *tx)
            .await
            .expect("benchmark parsed document should insert");

            if index % 5 == 0 {
                sqlx::query(
                    r#"
                    INSERT INTO ai_analysis (
                        id,
                        object_id,
                        analysis_type,
                        schema_version,
                        summary,
                        tags_json,
                        key_points_json,
                        claims_json,
                        action_items_json,
                        risks_json,
                        created_at
                    ) VALUES (
                        ?1,
                        ?2,
                        'general_summary',
                        1,
                        ?3,
                        '[]',
                        '[]',
                        '[]',
                        '[]',
                        '[]',
                        ?4
                    )
                    "#,
                )
                .bind(analysis_id)
                .bind(&object_id)
                .bind(format!(
                    "Retention capsule summary for benchmark object {index:05} with stable recall markers."
                ))
                .bind(&captured_at)
                .execute(&mut *tx)
                .await
                .expect("benchmark ai analysis should insert");
            }
        }

        tx.commit().await.expect("benchmark tx should commit");
    }

    fn benchmark_object_type(index: usize) -> &'static str {
        match index % 4 {
            0 => "article",
            1 => "github_repo",
            2 => "prompt",
            _ => "article",
        }
    }

    fn benchmark_lifecycle_status(index: usize) -> &'static str {
        if index % 13 == 0 {
            "failed"
        } else {
            "parsed"
        }
    }

    fn benchmark_privacy_level(index: usize) -> &'static str {
        if index % 19 == 0 {
            "secret"
        } else if index % 7 == 0 {
            "sensitive"
        } else {
            "personal"
        }
    }

    fn benchmark_title(index: usize, object_type: &str) -> String {
        match object_type {
            "github_repo" => format!("Repository Benchmark Workflow {index:05}"),
            "prompt" => format!("Prompt Benchmark Capsule {index:05}"),
            _ => format!("Article Benchmark Durable Workflow {index:05}"),
        }
    }

    fn benchmark_text(index: usize, object_type: &str, lifecycle_status: &str) -> String {
        let type_marker = match object_type {
            "github_repo" => {
                " repository benchmark includes github metadata readme stars forks and release notes"
            }
            "prompt" => " prompt benchmark includes instruction quality examples and test cases",
            _ => " article benchmark includes citations paragraphs headings and parser output",
        };
        let lifecycle_marker = if lifecycle_status == "failed" {
            " failure boundary retryable capture diagnostics visible recovery action"
        } else {
            " successful parse boundary visible knowledge library object"
        };
        let rare_marker = if index % 97 == 0 {
            " priority alpha rare marker"
        } else {
            ""
        };

        format!(
            "Durable workflow evidence common benchmark content {index:05}. Search quality regression corpus includes local first notes parser output recall checks ranking coverage and stable snippets.{type_marker}.{lifecycle_marker}.{rare_marker}"
        )
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
        object_type: &str,
        title: &str,
        text_content: &str,
        privacy_level: &str,
        lifecycle_status: &str,
    ) {
        let parsed_id = format!("parsed-{object_id}");
        let hash = format!("hash-{object_id}");

        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, author, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (
                ?1, 'local', ?2, ?3, 'Author', ?4, ?5,
                '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z'
            )
            "#,
        )
        .bind(object_id)
        .bind(object_type)
        .bind(title)
        .bind(privacy_level)
        .bind(lifecycle_status)
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

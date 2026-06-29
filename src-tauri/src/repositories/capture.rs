use crate::domain::capture::{
    CaptureBackgroundJobSubmission, CaptureDomainEventSubmission, CaptureFetchJobRecord,
    CaptureParsedDocumentSubmission, CaptureSnapshotSubmission, CaptureSubmission,
};
use crate::errors::AppResult;
use crate::repositories::search::SearchRepository;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};

pub struct CaptureRepository;

pub struct CaptureFetchCompletion<'a> {
    pub job_id: &'a str,
    pub object_id: &'a str,
    pub user_id: &'a str,
    pub title: Option<&'a str>,
    pub author: Option<&'a str>,
    pub snapshot: &'a CaptureSnapshotSubmission,
    pub parsed_document: &'a CaptureParsedDocumentSubmission,
    pub events: &'a [CaptureDomainEventSubmission],
    pub now: &'a str,
}

pub struct ExistingCaptureRecord {
    pub object_id: String,
    pub snapshot_id: Option<String>,
    pub parsed_document_id: Option<String>,
    pub job_id: Option<String>,
}

impl CaptureRepository {
    pub async fn find_active_by_canonical_url(
        tx: &mut Transaction<'_, Sqlite>,
        user_id: &str,
        canonical_url: &str,
    ) -> AppResult<Option<ExistingCaptureRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                objects.id AS object_id,
                (
                    SELECT snapshots.id
                    FROM source_snapshots AS snapshots
                    WHERE snapshots.object_id = objects.id
                    ORDER BY snapshots.captured_at DESC, snapshots.id DESC
                    LIMIT 1
                ) AS snapshot_id,
                (
                    SELECT parsed.id
                    FROM parsed_documents AS parsed
                    WHERE parsed.object_id = objects.id
                    ORDER BY parsed.created_at DESC, parsed.id DESC
                    LIMIT 1
                ) AS parsed_document_id,
                (
                    SELECT jobs.id
                    FROM background_jobs AS jobs
                    WHERE jobs.object_id = objects.id
                    ORDER BY jobs.created_at DESC, jobs.id DESC
                    LIMIT 1
                ) AS job_id
            FROM knowledge_objects AS objects
            WHERE objects.user_id = ?1
              AND objects.canonical_url = ?2
              AND objects.lifecycle_status != 'deleted'
            ORDER BY objects.updated_at DESC, objects.captured_at DESC, objects.id DESC
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .bind(canonical_url)
        .fetch_optional(&mut **tx)
        .await?;

        Ok(row.map(|row| ExistingCaptureRecord {
            object_id: row.get("object_id"),
            snapshot_id: row.get("snapshot_id"),
            parsed_document_id: row.get("parsed_document_id"),
            job_id: row.get("job_id"),
        }))
    }

    pub async fn insert_submission(
        tx: &mut Transaction<'_, Sqlite>,
        submission: &CaptureSubmission,
    ) -> AppResult<()> {
        insert_knowledge_object(tx, submission).await?;
        insert_source_snapshot(tx, &submission.object_id, &submission.snapshot).await?;

        if let Some(parsed_document) = &submission.parsed_document {
            insert_parsed_document(
                tx,
                &submission.object_id,
                &submission.snapshot.id,
                parsed_document,
            )
            .await?;
            SearchRepository::reindex_object(tx, &submission.object_id).await?;
        }

        insert_background_job(tx, &submission.object_id, &submission.job).await?;

        for event in &submission.events {
            insert_domain_event(tx, &submission.object_id, event).await?;
        }

        Ok(())
    }

    pub async fn claim_fetch_job_by_id(
        tx: &mut Transaction<'_, Sqlite>,
        job_id: &str,
        locked_by: &str,
        now: &str,
    ) -> AppResult<Option<CaptureFetchJobRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                jobs.id,
                jobs.object_id,
                jobs.attempt_count,
                jobs.max_attempts,
                jobs.payload_json,
                objects.user_id,
                objects.canonical_url
            FROM background_jobs AS jobs
            INNER JOIN knowledge_objects AS objects ON objects.id = jobs.object_id
            WHERE jobs.id = ?1
              AND jobs.job_type = 'capture.fetch_url'
              AND jobs.status = 'queued'
            LIMIT 1
            "#,
        )
        .bind(job_id)
        .fetch_optional(&mut **tx)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let record = fetch_job_from_row(row);

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET
                status = 'running',
                attempt_count = attempt_count + 1,
                locked_at = ?2,
                locked_by = ?3,
                updated_at = ?2
            WHERE id = ?1
            "#,
        )
        .bind(job_id)
        .bind(now)
        .bind(locked_by)
        .execute(&mut **tx)
        .await?;

        Ok(Some(record))
    }

    pub async fn complete_fetch_job(
        tx: &mut Transaction<'_, Sqlite>,
        completion: CaptureFetchCompletion<'_>,
    ) -> AppResult<()> {
        insert_source_snapshot(tx, completion.object_id, completion.snapshot).await?;
        insert_parsed_document(
            tx,
            completion.object_id,
            &completion.snapshot.id,
            completion.parsed_document,
        )
        .await?;

        sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET
                title = COALESCE(NULLIF(title, ''), ?1),
                author = COALESCE(NULLIF(author, ''), ?2),
                lifecycle_status = 'parsed',
                failure_reason = NULL,
                updated_at = ?3
            WHERE id = ?4
            "#,
        )
        .bind(completion.title)
        .bind(completion.author)
        .bind(completion.now)
        .bind(completion.object_id)
        .execute(&mut **tx)
        .await?;

        SearchRepository::reindex_object(tx, completion.object_id).await?;

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET
                status = 'succeeded',
                locked_at = NULL,
                locked_by = NULL,
                last_error = NULL,
                updated_at = ?2
            WHERE id = ?1
            "#,
        )
        .bind(completion.job_id)
        .bind(completion.now)
        .execute(&mut **tx)
        .await?;

        for event in completion.events {
            insert_domain_event(tx, completion.object_id, event).await?;
        }

        insert_audit_log(
            tx,
            completion.user_id,
            "capture.fetch_url.succeeded",
            completion.object_id,
            completion.now,
        )
        .await?;

        Ok(())
    }

    pub async fn fail_fetch_job(
        tx: &mut Transaction<'_, Sqlite>,
        job_id: &str,
        object_id: &str,
        user_id: &str,
        failure_reason: &str,
        event: &CaptureDomainEventSubmission,
        now: &str,
    ) -> AppResult<()> {
        let failure_reason = truncate_failure_reason(failure_reason);

        sqlx::query(
            r#"
            UPDATE knowledge_objects
            SET
                lifecycle_status = 'failed',
                failure_reason = ?1,
                updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(&failure_reason)
        .bind(now)
        .bind(object_id)
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET
                status = 'failed',
                locked_at = NULL,
                locked_by = NULL,
                last_error = ?1,
                updated_at = ?2
            WHERE id = ?3
            "#,
        )
        .bind(&failure_reason)
        .bind(now)
        .bind(job_id)
        .execute(&mut **tx)
        .await?;

        insert_domain_event(tx, object_id, event).await?;
        insert_audit_log(tx, user_id, "capture.fetch_url.failed", object_id, now).await?;

        Ok(())
    }
}

fn fetch_job_from_row(row: SqliteRow) -> CaptureFetchJobRecord {
    let object_id: String = row.get("object_id");
    let payload_json: String = row.get("payload_json");
    let correlation_id = serde_json::from_str::<serde_json::Value>(&payload_json)
        .ok()
        .and_then(|payload| {
            payload
                .get("correlationId")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned)
        })
        .filter(|value| uuid::Uuid::parse_str(value).is_ok())
        .unwrap_or_else(|| format!("capture-{object_id}"));

    CaptureFetchJobRecord {
        id: row.get("id"),
        object_id,
        user_id: row.get("user_id"),
        canonical_url: row.get("canonical_url"),
        correlation_id,
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
    }
}

async fn insert_knowledge_object(
    tx: &mut Transaction<'_, Sqlite>,
    submission: &CaptureSubmission,
) -> AppResult<()> {
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
            failure_reason,
            captured_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, ?10, ?11)
        "#,
    )
    .bind(&submission.object_id)
    .bind(&submission.user_id)
    .bind(&submission.object_type)
    .bind(&submission.title)
    .bind(&submission.canonical_url)
    .bind(&submission.source_platform)
    .bind(&submission.author)
    .bind(&submission.privacy_level)
    .bind(&submission.lifecycle_status)
    .bind(&submission.captured_at)
    .bind(&submission.updated_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_source_snapshot(
    tx: &mut Transaction<'_, Sqlite>,
    object_id: &str,
    snapshot: &CaptureSnapshotSubmission,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO source_snapshots (
            id,
            object_id,
            snapshot_type,
            storage_uri,
            content_hash,
            parser_id,
            parser_version,
            captured_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&snapshot.id)
    .bind(object_id)
    .bind(&snapshot.snapshot_type)
    .bind(&snapshot.storage_uri)
    .bind(&snapshot.content_hash)
    .bind(&snapshot.parser_id)
    .bind(&snapshot.parser_version)
    .bind(&snapshot.captured_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_parsed_document(
    tx: &mut Transaction<'_, Sqlite>,
    object_id: &str,
    snapshot_id: &str,
    parsed_document: &CaptureParsedDocumentSubmission,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO parsed_documents (
            id,
            object_id,
            source_snapshot_id,
            title,
            text_content,
            markdown_content,
            language,
            word_count,
            content_hash,
            parser_id,
            parser_version,
            created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&parsed_document.id)
    .bind(object_id)
    .bind(snapshot_id)
    .bind(&parsed_document.title)
    .bind(&parsed_document.text_content)
    .bind(&parsed_document.markdown_content)
    .bind(&parsed_document.language)
    .bind(parsed_document.word_count)
    .bind(&parsed_document.content_hash)
    .bind(&parsed_document.parser_id)
    .bind(&parsed_document.parser_version)
    .bind(&parsed_document.created_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_background_job(
    tx: &mut Transaction<'_, Sqlite>,
    object_id: &str,
    job: &CaptureBackgroundJobSubmission,
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
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)
        "#,
    )
    .bind(&job.id)
    .bind(&job.job_type)
    .bind(&job.status)
    .bind(object_id)
    .bind(&job.payload_json)
    .bind(job.max_attempts)
    .bind(&job.created_at)
    .bind(&job.updated_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_domain_event(
    tx: &mut Transaction<'_, Sqlite>,
    object_id: &str,
    event: &CaptureDomainEventSubmission,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO domain_events (
            id,
            event_type,
            event_version,
            user_id,
            object_id,
            correlation_id,
            payload_json,
            occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
    )
    .bind(&event.id)
    .bind(&event.event_type)
    .bind(event.event_version)
    .bind(&event.user_id)
    .bind(object_id)
    .bind(&event.correlation_id)
    .bind(&event.payload_json)
    .bind(&event.occurred_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn insert_audit_log(
    tx: &mut Transaction<'_, Sqlite>,
    user_id: &str,
    action: &str,
    object_id: &str,
    created_at: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO audit_logs (
            id,
            user_id,
            actor_type,
            actor_id,
            action,
            object_id,
            metadata_json,
            created_at
        ) VALUES (?1, ?2, 'system', 'capture.fetch_url', ?3, ?4, '{}', ?5)
        "#,
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(action)
    .bind(object_id)
    .bind(created_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn truncate_failure_reason(reason: &str) -> String {
    const MAX_FAILURE_REASON_CHARS: usize = 500;

    if reason.chars().count() <= MAX_FAILURE_REASON_CHARS {
        return reason.to_string();
    }

    reason
        .chars()
        .take(MAX_FAILURE_REASON_CHARS)
        .collect::<String>()
}

use crate::domain::capture::{
    CaptureBackgroundJobSubmission, CaptureDomainEventSubmission, CaptureParsedDocumentSubmission,
    CaptureSnapshotSubmission, CaptureSubmission,
};
use crate::errors::AppResult;
use sqlx::{Sqlite, Transaction};

pub struct CaptureRepository;

impl CaptureRepository {
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
        }

        insert_background_job(tx, &submission.object_id, &submission.job).await?;

        for event in &submission.events {
            insert_domain_event(tx, &submission.object_id, event).await?;
        }

        Ok(())
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
            payload_json,
            occurred_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
    )
    .bind(&event.id)
    .bind(&event.event_type)
    .bind(event.event_version)
    .bind(&event.user_id)
    .bind(object_id)
    .bind(&event.payload_json)
    .bind(&event.occurred_at)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

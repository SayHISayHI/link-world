use crate::domain::knowledge::{
    AIAnalysis, EvaluationRun, KnowledgeObject, KnowledgeObjectDetail, ParsedDocument,
    SourceSnapshot,
};
use crate::domain::portable_export::PortableExportSummary;
use crate::errors::{AppError, AppResult};
use crate::repositories::knowledge_objects::KnowledgeObjectRepository;
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const EXPORTS_DIR_NAME: &str = "exports";
const EXPORT_SCHEMA_VERSION: i64 = 1;
const EXPORT_FORMAT: &str = "markdown_json_directory";

#[derive(Debug, Clone)]
pub struct PortableExportService {
    repository: KnowledgeObjectRepository,
    export_root: PathBuf,
    app_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableExportManifest {
    schema_version: i64,
    export_id: String,
    app_version: String,
    created_at: String,
    format: String,
    object_count: usize,
    skipped_secret_count: usize,
    excluded_privacy_levels: Vec<&'static str>,
    objects: Vec<PortableExportManifestObject>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortableExportManifestObject {
    object_id: String,
    title: Option<String>,
    privacy_level: String,
    metadata_path: String,
    markdown_path: Option<String>,
    content_hash: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedKnowledgeObject<'a> {
    schema_version: i64,
    exported_at: &'a str,
    object: &'a KnowledgeObject,
    parsed_document: Option<ExportedParsedDocument<'a>>,
    source_snapshots: Vec<ExportedSourceSnapshot<'a>>,
    ai_analyses: &'a [AIAnalysis],
    evaluations: Vec<ExportedEvaluationRun<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedParsedDocument<'a> {
    id: &'a str,
    object_id: &'a str,
    source_snapshot_id: Option<&'a str>,
    title: Option<&'a str>,
    language: Option<&'a str>,
    word_count: Option<i64>,
    content_hash: &'a str,
    parser_id: &'a str,
    parser_version: &'a str,
    created_at: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedSourceSnapshot<'a> {
    id: &'a str,
    object_id: &'a str,
    snapshot_type: &'a str,
    content_hash: &'a str,
    parser_id: Option<&'a str>,
    parser_version: Option<&'a str>,
    captured_at: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedEvaluationRun<'a> {
    id: &'a str,
    object_id: &'a str,
    evaluator_type: &'a str,
    evaluator_version: &'a str,
    status: &'a str,
    score: Option<f64>,
    verdict: &'a str,
    dimensions: &'a serde_json::Value,
    evidence: &'a [crate::domain::knowledge::EvidenceItem],
    artifacts: Vec<ExportedEvaluationArtifact<'a>>,
    limitations: &'a [String],
    next_actions: &'a [serde_json::Value],
    failure_reason: Option<&'a str>,
    created_at: &'a str,
    completed_at: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportedEvaluationArtifact<'a> {
    kind: &'a str,
    metadata: Option<&'a serde_json::Value>,
}

impl PortableExportService {
    pub fn new(
        repository: KnowledgeObjectRepository,
        data_dir: impl AsRef<Path>,
        app_version: impl Into<String>,
    ) -> Self {
        Self {
            repository,
            export_root: data_dir.as_ref().join(EXPORTS_DIR_NAME),
            app_version: app_version.into(),
        }
    }

    pub async fn export_library(&self) -> AppResult<PortableExportSummary> {
        fs::create_dir_all(&self.export_root)?;

        let created_at = Utc::now().to_rfc3339();
        let export_id = format!(
            "library-{}-{}",
            Utc::now().format("%Y%m%d%H%M%S"),
            Uuid::new_v4()
        );
        let staging_dir = self.export_root.join(format!("{export_id}.staging"));
        let final_dir = self.export_root.join(&export_id);

        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir)?;
        }
        if final_dir.exists() {
            return Err(AppError::Filesystem(format!(
                "portable export destination already exists: {export_id}"
            )));
        }

        fs::create_dir_all(staging_dir.join("objects"))?;

        let result = self
            .write_export(&staging_dir, &export_id, &created_at)
            .await;
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging_dir);
        }
        let mut summary = result?;

        fs::rename(&staging_dir, &final_dir)?;
        summary.export_root = final_dir.display().to_string();
        Ok(summary)
    }

    async fn write_export(
        &self,
        staging_dir: &Path,
        export_id: &str,
        created_at: &str,
    ) -> AppResult<PortableExportSummary> {
        let candidates = self.repository.list_export_candidates().await?;
        let mut manifest_objects = Vec::new();
        let mut jsonl_lines = Vec::new();
        let mut skipped_secret_count = 0;
        let mut markdown_file_count = 0;
        let mut json_file_count = 0;

        for candidate in candidates {
            if candidate.privacy_level == "secret" {
                skipped_secret_count += 1;
                continue;
            }

            let detail = self.repository.get_detail(&candidate.id).await?;
            let object_dir_name = export_object_dir_name(&detail.object);
            let object_dir = staging_dir.join("objects").join(&object_dir_name);
            fs::create_dir_all(&object_dir)?;

            let metadata_path = object_dir.join("metadata.json");
            let relative_metadata_path = format!("objects/{object_dir_name}/metadata.json");
            let metadata = exported_metadata(&detail, created_at);
            write_json(&metadata_path, &metadata)?;
            json_file_count += 1;
            jsonl_lines.push(serde_json::to_string(&metadata).map_err(|error| {
                AppError::Unknown(format!(
                    "failed to serialize portable export metadata: {error}"
                ))
            })?);

            let markdown_path = if let Some(parsed_document) = &detail.parsed_document {
                let markdown = render_markdown(&detail.object, parsed_document, created_at);
                let path = object_dir.join("document.md");
                fs::write(&path, markdown)?;
                markdown_file_count += 1;
                Some(format!("objects/{object_dir_name}/document.md"))
            } else {
                None
            };

            manifest_objects.push(PortableExportManifestObject {
                object_id: detail.object.id,
                title: detail.object.title,
                privacy_level: detail.object.privacy_level,
                metadata_path: relative_metadata_path,
                markdown_path,
                content_hash: detail
                    .parsed_document
                    .as_ref()
                    .map(|document| document.content_hash.clone()),
            });
        }

        let object_count = manifest_objects.len();
        let manifest = PortableExportManifest {
            schema_version: EXPORT_SCHEMA_VERSION,
            export_id: export_id.to_string(),
            app_version: self.app_version.clone(),
            created_at: created_at.to_string(),
            format: EXPORT_FORMAT.to_string(),
            object_count,
            skipped_secret_count,
            excluded_privacy_levels: vec!["secret"],
            objects: manifest_objects,
        };

        write_json(&staging_dir.join("manifest.json"), &manifest)?;
        json_file_count += 1;
        fs::write(staging_dir.join("objects.jsonl"), jsonl_lines.join("\n"))?;
        json_file_count += 1;
        write_export_checksum(staging_dir)?;

        Ok(PortableExportSummary {
            export_id: export_id.to_string(),
            export_root: staging_dir.display().to_string(),
            format: EXPORT_FORMAT.to_string(),
            object_count,
            skipped_secret_count,
            markdown_file_count,
            json_file_count,
            created_at: created_at.to_string(),
        })
    }
}

fn exported_metadata<'a>(
    detail: &'a KnowledgeObjectDetail,
    exported_at: &'a str,
) -> ExportedKnowledgeObject<'a> {
    ExportedKnowledgeObject {
        schema_version: EXPORT_SCHEMA_VERSION,
        exported_at,
        object: &detail.object,
        parsed_document: detail
            .parsed_document
            .as_ref()
            .map(exported_parsed_document),
        source_snapshots: detail
            .snapshots
            .iter()
            .map(exported_source_snapshot)
            .collect(),
        ai_analyses: &detail.ai_analyses,
        evaluations: detail.evaluations.iter().map(exported_evaluation).collect(),
    }
}

fn exported_parsed_document(document: &ParsedDocument) -> ExportedParsedDocument<'_> {
    ExportedParsedDocument {
        id: &document.id,
        object_id: &document.object_id,
        source_snapshot_id: document.source_snapshot_id.as_deref(),
        title: document.title.as_deref(),
        language: document.language.as_deref(),
        word_count: document.word_count,
        content_hash: &document.content_hash,
        parser_id: &document.parser_id,
        parser_version: &document.parser_version,
        created_at: &document.created_at,
    }
}

fn exported_source_snapshot(snapshot: &SourceSnapshot) -> ExportedSourceSnapshot<'_> {
    ExportedSourceSnapshot {
        id: &snapshot.id,
        object_id: &snapshot.object_id,
        snapshot_type: &snapshot.snapshot_type,
        content_hash: &snapshot.content_hash,
        parser_id: snapshot.parser_id.as_deref(),
        parser_version: snapshot.parser_version.as_deref(),
        captured_at: &snapshot.captured_at,
    }
}

fn exported_evaluation(evaluation: &EvaluationRun) -> ExportedEvaluationRun<'_> {
    ExportedEvaluationRun {
        id: &evaluation.id,
        object_id: &evaluation.object_id,
        evaluator_type: &evaluation.evaluator_type,
        evaluator_version: &evaluation.evaluator_version,
        status: &evaluation.status,
        score: evaluation.score,
        verdict: &evaluation.verdict,
        dimensions: &evaluation.dimensions,
        evidence: &evaluation.evidence,
        artifacts: evaluation
            .artifacts
            .iter()
            .map(|artifact| ExportedEvaluationArtifact {
                kind: &artifact.kind,
                metadata: artifact.metadata.as_ref(),
            })
            .collect(),
        limitations: &evaluation.limitations,
        next_actions: &evaluation.next_actions,
        failure_reason: evaluation.failure_reason.as_deref(),
        created_at: &evaluation.created_at,
        completed_at: evaluation.completed_at.as_deref(),
    }
}

fn render_markdown(
    object: &KnowledgeObject,
    parsed_document: &ParsedDocument,
    exported_at: &str,
) -> String {
    let title = object
        .title
        .as_deref()
        .or(parsed_document.title.as_deref())
        .unwrap_or("Untitled");
    let mut markdown = String::new();
    markdown.push_str("<!-- Exported from Link World. This file intentionally omits credentials and local storage paths. -->\n\n");
    markdown.push_str("# ");
    markdown.push_str(title.trim());
    markdown.push_str("\n\n");
    markdown.push_str(&format!("- Object ID: {}\n", object.id));
    markdown.push_str(&format!("- Type: {}\n", object.object_type));
    markdown.push_str(&format!("- Privacy: {}\n", object.privacy_level));
    markdown.push_str(&format!("- Captured: {}\n", object.captured_at));
    markdown.push_str(&format!("- Exported: {}\n", exported_at));
    if let Some(url) = object.canonical_url.as_deref() {
        markdown.push_str(&format!("- Source: {}\n", url));
    }
    markdown.push('\n');

    let body = parsed_document
        .markdown_content
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&parsed_document.text_content);
    markdown.push_str(body.trim());
    markdown.push('\n');
    markdown
}

fn write_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        AppError::Unknown(format!("failed to serialize portable export json: {error}"))
    })?;
    fs::write(path, bytes)?;
    Ok(())
}

fn write_export_checksum(staging_dir: &Path) -> AppResult<()> {
    let manifest = fs::read(staging_dir.join("manifest.json"))?;
    let mut hasher = Sha256::new();
    hasher.update(manifest);
    fs::write(
        staging_dir.join("manifest.sha256"),
        format!("{:x}\n", hasher.finalize()),
    )?;
    Ok(())
}

fn export_object_dir_name(object: &KnowledgeObject) -> String {
    let title = object.title.as_deref().unwrap_or(&object.object_type);
    let slug = sanitize_file_stem(title);
    let short_id = object
        .id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>();
    format!("{slug}-{short_id}")
}

fn sanitize_file_stem(value: &str) -> String {
    let mut output = String::new();
    let mut last_was_dash = false;
    for character in value.chars() {
        let normalized = if character.is_ascii_alphanumeric() {
            Some(character.to_ascii_lowercase())
        } else if character.is_whitespace() || matches!(character, '-' | '_' | '.' | '/') {
            Some('-')
        } else {
            None
        };

        if let Some(character) = normalized {
            if character == '-' {
                if !last_was_dash && !output.is_empty() {
                    output.push('-');
                    last_was_dash = true;
                }
            } else {
                output.push(character);
                last_was_dash = false;
            }
        }

        if output.len() >= 64 {
            break;
        }
    }

    let trimmed = output.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "object".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::PortableExportService;
    use crate::domain::knowledge::NewKnowledgeObject;
    use crate::repositories::knowledge_objects::KnowledgeObjectRepository;
    use crate::storage::database::Database;
    use std::fs;
    use uuid::Uuid;

    #[tokio::test]
    async fn export_library_writes_non_secret_markdown_and_redacted_metadata() {
        let data_dir = std::env::temp_dir().join(format!(
            "link-world-portable-export-test-{}",
            Uuid::new_v4()
        ));
        let database = Database::initialize(data_dir.clone())
            .await
            .expect("database should initialize");
        let repository = KnowledgeObjectRepository::new(database.pool().clone());

        let object = repository
            .insert(NewKnowledgeObject {
                user_id: "local-user".to_string(),
                object_type: "article".to_string(),
                title: Some("Export / Candidate".to_string()),
                canonical_url: Some("https://example.com/article".to_string()),
                source_platform: Some("web".to_string()),
                author: None,
                privacy_level: "personal".to_string(),
            })
            .await
            .expect("object should insert");
        sqlx::query(
            "INSERT INTO source_snapshots (id, object_id, snapshot_type, storage_uri, content_hash) VALUES ('snapshot-export', ?1, 'html', 'local://objects/private/source.html', 'snapshot-hash')",
        )
        .bind(&object.id)
        .execute(database.pool())
        .await
        .expect("snapshot should insert");
        sqlx::query(
            "INSERT INTO parsed_documents (id, object_id, source_snapshot_id, title, text_content, markdown_content, content_hash, parser_id, parser_version) VALUES ('document-export', ?1, 'snapshot-export', 'Exported title', 'Plain export body', '# Exported body', 'document-hash', 'test.parser', '1')",
        )
        .bind(&object.id)
        .execute(database.pool())
        .await
        .expect("document should insert");

        let secret = repository
            .insert(NewKnowledgeObject {
                user_id: "local-user".to_string(),
                object_type: "article".to_string(),
                title: Some("Secret Object".to_string()),
                canonical_url: None,
                source_platform: None,
                author: None,
                privacy_level: "secret".to_string(),
            })
            .await
            .expect("secret should insert");
        sqlx::query(
            "INSERT INTO parsed_documents (id, object_id, title, text_content, content_hash, parser_id, parser_version) VALUES ('document-secret', ?1, 'Secret title', 'secret credential value', 'secret-hash', 'test.parser', '1')",
        )
        .bind(&secret.id)
        .execute(database.pool())
        .await
        .expect("secret document should insert");

        let service = PortableExportService::new(repository, &data_dir, "test-version");
        let summary = service
            .export_library()
            .await
            .expect("library should export");

        assert_eq!(summary.object_count, 1);
        assert_eq!(summary.skipped_secret_count, 1);
        assert_eq!(summary.markdown_file_count, 1);

        let export_dir = data_dir.join("exports").join(&summary.export_id);
        let manifest =
            fs::read_to_string(export_dir.join("manifest.json")).expect("manifest should read");
        assert!(manifest.contains(&object.id));
        assert!(!manifest.contains(&secret.id));
        assert!(!manifest.contains("storageUri"));
        assert!(!manifest.contains("local://objects/private"));
        assert!(!manifest.contains("secret credential value"));

        let object_dir_name = format!(
            "export-candidate-{}",
            object.id.chars().take(8).collect::<String>()
        );
        let markdown = fs::read_to_string(
            export_dir
                .join("objects")
                .join(object_dir_name)
                .join("document.md"),
        )
        .expect("markdown should read");
        assert!(markdown.contains("# Export / Candidate"));
        assert!(markdown.contains("# Exported body"));

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }
}

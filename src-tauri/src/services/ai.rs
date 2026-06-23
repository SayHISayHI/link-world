use crate::domain::ai::{
    AIAnalysisSubmission, AIDisplayHintsV1, AIEnrichmentInput, AIEnrichmentRunResult,
    AIModelAnalysisOutput, AITraceSubmission, ModelProviderConfig, StoredModelProviderConfig,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::ai::AIRepository;
use crate::state::{AppState, SecretStore};
use crate::storage::object_store::sha256_hex;
use chrono::Utc;
use reqwest::Url;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::time::Instant;
use tauri::Emitter;
use uuid::Uuid;

const GENERAL_ENRICHMENT_PROMPT_ID: &str = "builtin.general_enrichment";
const GENERAL_ENRICHMENT_PROMPT_VERSION: &str = "0.2.0";
const MAX_MODEL_INPUT_CHARS: usize = 24_000;

#[derive(Clone)]
pub struct AIEnrichmentService {
    repository: AIRepository,
    http_client: reqwest::Client,
    secrets: SecretStore,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
}

struct ChatModelOutput {
    content: String,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    latency_ms: i64,
}

impl AIEnrichmentService {
    pub fn new(pool: SqlitePool, secrets: SecretStore) -> Self {
        Self {
            repository: AIRepository::new(pool),
            http_client: reqwest::Client::new(),
            secrets,
        }
    }

    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self::new(
            state.database()?.pool().clone(),
            state.secrets().clone(),
        ))
    }

    pub async fn update_model_provider_config(&self, config: ModelProviderConfig) -> AppResult<()> {
        validate_model_provider_config(&config)?;
        let secret_ref = self.store_or_resolve_secret_ref(&config)?;
        self.repository
            .upsert_model_provider_config(&config, secret_ref.as_deref())
            .await
    }

    pub async fn run_auto_enrichment_for_object(
        &self,
        object_id: &str,
    ) -> AppResult<Option<AIEnrichmentRunResult>> {
        if self.repository.get_enabled_chat_config().await?.is_none() {
            return Ok(None);
        }

        self.run_enrichment_for_object(object_id).await.map(Some)
    }

    pub async fn run_enrichment_for_object(
        &self,
        object_id: &str,
    ) -> AppResult<AIEnrichmentRunResult> {
        let job_id = self.repository.create_enrichment_job(object_id).await?;
        let result = self.run_enrichment_job(&job_id, object_id).await;

        match result {
            Ok(analysis_id) => Ok(AIEnrichmentRunResult {
                job_id,
                analysis_id: Some(analysis_id),
                status: "succeeded".to_string(),
                failure_reason: None,
            }),
            Err(error) => {
                let failure_reason = error.to_string();
                self.repository
                    .fail_enrichment_job(&job_id, &failure_reason)
                    .await?;

                Ok(AIEnrichmentRunResult {
                    job_id,
                    analysis_id: None,
                    status: "failed".to_string(),
                    failure_reason: Some(failure_reason),
                })
            }
        }
    }

    async fn run_enrichment_job(&self, job_id: &str, object_id: &str) -> AppResult<String> {
        let config = self
            .repository
            .get_enabled_chat_config()
            .await?
            .ok_or_else(|| {
                AppError::PolicyDenied("no enabled chat model provider configured".to_string())
            })?;
        let input = self.repository.get_enrichment_input(object_id).await?;

        enforce_privacy_policy(&input, &config)?;

        let model = config
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::PolicyDenied("default chat model is required".to_string()))?;
        let prompt = build_general_enrichment_prompt(&input);
        let model_output = self.call_chat_model(&config, &model, &prompt).await?;
        let analysis_output = parse_analysis_output(&model_output.content)?;
        let display_hints_json = normalize_display_hints(analysis_output.display_hints.as_ref())
            .map(|hints| serialize_json(&hints))
            .transpose()?;
        let now = Utc::now().to_rfc3339();
        let analysis_id = Uuid::new_v4().to_string();
        let output_hash = sha256_hex(model_output.content.as_bytes());

        let analysis = AIAnalysisSubmission {
            id: analysis_id.clone(),
            object_id: input.object_id.clone(),
            parsed_document_id: input.parsed_document_id.clone(),
            analysis_type: "general_summary".to_string(),
            schema_version: 2,
            summary: analysis_output.summary,
            category: analysis_output.category,
            tags_json: serialize_json(&analysis_output.tags)?,
            key_points_json: serialize_json(&analysis_output.key_points)?,
            claims_json: serialize_json(&analysis_output.claims)?,
            action_items_json: serialize_json(&analysis_output.action_items)?,
            risks_json: serialize_json(&analysis_output.risks)?,
            quality_score: analysis_output.quality_score,
            confidence: analysis_output.confidence,
            display_hints_json,
            created_at: now.clone(),
        };
        let trace = AITraceSubmission {
            id: Uuid::new_v4().to_string(),
            analysis_id: analysis_id.clone(),
            object_id: input.object_id,
            provider: config.provider,
            model,
            capability: "chat".to_string(),
            prompt_template_id: Some(GENERAL_ENRICHMENT_PROMPT_ID.to_string()),
            prompt_template_version: Some(GENERAL_ENRICHMENT_PROMPT_VERSION.to_string()),
            input_snapshot_id: input.source_snapshot_id,
            input_parsed_document_id: Some(input.parsed_document_id),
            input_hash: Some(input.content_hash),
            output_hash: Some(output_hash),
            prompt_tokens: model_output.prompt_tokens,
            completion_tokens: model_output.completion_tokens,
            estimated_cost_usd: None,
            latency_ms: Some(model_output.latency_ms),
            created_at: now,
        };

        self.repository
            .complete_enrichment_job(job_id, &analysis, &trace)
            .await?;

        Ok(analysis_id)
    }

    async fn call_chat_model(
        &self,
        config: &StoredModelProviderConfig,
        model: &str,
        prompt: &str,
    ) -> AppResult<ChatModelOutput> {
        let endpoint = chat_completions_endpoint(config)?;
        let mut request = self.http_client.post(endpoint).json(&json!({
            "model": model,
            "temperature": 0.2,
            "response_format": { "type": "json_object" },
            "messages": [
                {
                    "role": "system",
                    "content": "You analyze saved knowledge objects. Return strict JSON only."
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        }));

        if let Some(secret_ref) = &config.secret_ref {
            if let Some(api_key) = self.secrets.resolve(secret_ref)? {
                request = request.bearer_auth(api_key);
            } else if !is_local_model_config(config) {
                return Err(AppError::ModelAuth);
            }
        } else if !is_local_model_config(config) {
            return Err(AppError::ModelAuth);
        }

        let started_at = Instant::now();
        let response = request.send().await.map_err(map_model_error)?;
        let status = response.status();

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(AppError::ModelAuth);
        }

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::ModelRateLimit);
        }

        if !status.is_success() {
            return Err(AppError::Unknown(format!(
                "model provider returned HTTP {status}"
            )));
        }

        let body: ChatCompletionResponse = response.json().await.map_err(map_model_error)?;
        let content = body
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| {
                AppError::ModelOutputSchema("missing chat completion content".to_string())
            })?;

        Ok(ChatModelOutput {
            content,
            prompt_tokens: body.usage.as_ref().and_then(|usage| usage.prompt_tokens),
            completion_tokens: body.usage.and_then(|usage| usage.completion_tokens),
            latency_ms: started_at.elapsed().as_millis().min(i64::MAX as u128) as i64,
        })
    }

    fn store_or_resolve_secret_ref(
        &self,
        config: &ModelProviderConfig,
    ) -> AppResult<Option<String>> {
        let provider_id = config
            .provider
            .trim()
            .to_ascii_lowercase()
            .replace(' ', "_");

        if let Some(api_key) = config
            .api_key
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            let secret_ref = format!("memory:model_provider:{provider_id}:api_key");
            self.secrets.set(&secret_ref, api_key.to_string())?;
            return Ok(Some(secret_ref));
        }

        if std::env::var("LINK_WORLD_OPENAI_API_KEY")
            .ok()
            .is_some_and(|value| !value.is_empty())
        {
            return Ok(Some("env:LINK_WORLD_OPENAI_API_KEY".to_string()));
        }

        Ok(None)
    }
}

pub fn spawn_ai_enrichment_runner(
    app_handle: tauri::AppHandle,
    service: AIEnrichmentService,
    object_id: String,
) {
    tauri::async_runtime::spawn(async move {
        let result = service.run_auto_enrichment_for_object(&object_id).await;

        let payload = match result {
            Ok(Some(run)) => json!({
                "jobId": run.job_id,
                "objectId": object_id,
                "status": run.status,
                "analysisId": run.analysis_id,
                "failureReason": run.failure_reason,
            }),
            Ok(None) => return,
            Err(error) => json!({
                "objectId": object_id,
                "status": "failed",
                "failureReason": error.to_string(),
            }),
        };

        let _ = app_handle.emit("ai://enrichment-completed", payload);
        let _ = app_handle.emit("library://objects-updated", ());
    });
}

fn validate_model_provider_config(config: &ModelProviderConfig) -> AppResult<()> {
    if config.provider.trim().is_empty() {
        return Err(AppError::PolicyDenied(
            "model provider is required".to_string(),
        ));
    }

    if !config
        .capabilities
        .iter()
        .any(|capability| capability == "chat")
    {
        return Err(AppError::PolicyDenied(
            "chat capability is required for AI enrichment".to_string(),
        ));
    }

    if config
        .chat_base_url
        .as_ref()
        .map(|url| Url::parse(url.trim()).is_ok())
        != Some(true)
    {
        return Err(AppError::PolicyDenied(
            "valid chat base URL is required".to_string(),
        ));
    }

    if config
        .default_chat_model
        .as_ref()
        .map(|model| !model.trim().is_empty())
        != Some(true)
    {
        return Err(AppError::PolicyDenied(
            "default chat model is required".to_string(),
        ));
    }

    Ok(())
}

fn enforce_privacy_policy(
    input: &AIEnrichmentInput,
    config: &StoredModelProviderConfig,
) -> AppResult<()> {
    if matches!(input.privacy_level.as_str(), "sensitive" | "secret")
        && !is_local_model_config(config)
    {
        return Err(AppError::PolicyDenied(
            "sensitive or secret objects cannot be sent to non-local model providers".to_string(),
        ));
    }

    Ok(())
}

fn chat_completions_endpoint(config: &StoredModelProviderConfig) -> AppResult<String> {
    let base_url = config
        .chat_base_url
        .as_deref()
        .ok_or_else(|| AppError::PolicyDenied("chat base URL is required".to_string()))?
        .trim()
        .trim_end_matches('/');

    if base_url.ends_with("/chat/completions") {
        return Ok(base_url.to_string());
    }

    Ok(format!("{base_url}/chat/completions"))
}

fn is_local_model_config(config: &StoredModelProviderConfig) -> bool {
    config
        .chat_base_url
        .as_deref()
        .and_then(|base_url| Url::parse(base_url).ok())
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .is_some_and(|host| matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1"))
}

fn build_general_enrichment_prompt(input: &AIEnrichmentInput) -> String {
    let text = truncate_chars(&input.text_content, MAX_MODEL_INPUT_CHARS);
    format!(
        r#"Analyze this saved knowledge object and return strict JSON with this schema:
{{
  "summary": "2-4 sentence concise summary",
  "category": "short category",
  "tags": ["tag"],
  "keyPoints": ["important point"],
  "claims": [],
  "actionItems": ["concrete next action"],
  "risks": ["risk or limitation"],
  "qualityScore": 0.0,
  "confidence": 0.0,
  "displayHints": {{
    "schemaVersion": 1,
    "mode": "article | tutorial | reference | code-heavy",
    "confidence": 0.0,
    "reason": "short reason for the suggested reading layout"
  }}
}}

Choose exactly one display mode: article for narrative prose, tutorial for step-by-step guidance,
reference for lookup-oriented specifications or tables, and code-heavy when code is the main content.

Object type: {}
Title: {}
URL: {}

Content:
{}"#,
        input.object_type,
        input.title.as_deref().unwrap_or("untitled"),
        input.canonical_url.as_deref().unwrap_or("none"),
        text
    )
}

fn parse_analysis_output(content: &str) -> AppResult<AIModelAnalysisOutput> {
    let value = extract_json_value(content)?;
    let output: AIModelAnalysisOutput = serde_json::from_value(value)
        .map_err(|error| AppError::ModelOutputSchema(error.to_string()))?;

    if output.summary.trim().is_empty() {
        return Err(AppError::ModelOutputSchema(
            "analysis summary is empty".to_string(),
        ));
    }

    Ok(output)
}

fn normalize_display_hints(value: Option<&Value>) -> Option<AIDisplayHintsV1> {
    let object = value?.as_object()?;
    if object.get("schemaVersion")?.as_i64()? != 1 {
        return None;
    }

    let mode = object.get("mode")?.as_str()?.trim();
    if !matches!(mode, "article" | "tutorial" | "reference" | "code-heavy") {
        return None;
    }

    let confidence = object.get("confidence")?.as_f64()?;
    if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
        return None;
    }

    let reason = object
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(|reason| truncate_chars(reason, 160));

    Some(AIDisplayHintsV1 {
        schema_version: 1,
        mode: mode.to_string(),
        confidence,
        reason,
    })
}

fn extract_json_value(content: &str) -> AppResult<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(content) {
        return Ok(value);
    }

    let start = content
        .find('{')
        .ok_or_else(|| AppError::ModelOutputSchema("missing JSON object start".to_string()))?;
    let end = content
        .rfind('}')
        .ok_or_else(|| AppError::ModelOutputSchema("missing JSON object end".to_string()))?;

    serde_json::from_str::<Value>(&content[start..=end])
        .map_err(|error| AppError::ModelOutputSchema(error.to_string()))
}

fn serialize_json<T>(value: &T) -> AppResult<String>
where
    T: serde::Serialize,
{
    serde_json::to_string(value).map_err(|error| AppError::ModelOutputSchema(error.to_string()))
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    text.chars().take(max_chars).collect()
}

fn map_model_error(error: reqwest::Error) -> AppError {
    if error.is_timeout() {
        AppError::NetworkTimeout
    } else {
        AppError::Unknown(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_general_enrichment_prompt, normalize_display_hints, parse_analysis_output,
        AIEnrichmentService,
    };
    use crate::domain::ai::AIEnrichmentInput;
    use crate::state::SecretStore;
    use crate::storage::database::Database;

    #[test]
    fn parses_json_object_from_model_output() {
        let output = parse_analysis_output(
            r#"{"summary":"Useful technique.","category":"engineering","tags":["rust"],"keyPoints":["Keep traces"],"qualityScore":0.8,"confidence":0.7}"#,
        )
        .expect("model output should parse");

        assert_eq!(output.summary, "Useful technique.");
        assert_eq!(output.tags, vec!["rust"]);
        assert_eq!(output.quality_score, Some(0.8));
    }

    #[test]
    fn accepts_valid_display_hints_and_drops_invalid_hints_without_failing_analysis() {
        let valid = parse_analysis_output(
            r#"{"summary":"Useful technique.","displayHints":{"schemaVersion":1,"mode":"tutorial","confidence":0.9,"reason":"Step-by-step content"}}"#,
        )
        .expect("valid analysis should parse");
        let normalized = normalize_display_hints(valid.display_hints.as_ref())
            .expect("valid display hints should normalize");
        assert_eq!(normalized.mode, "tutorial");
        assert_eq!(normalized.confidence, 0.9);

        let invalid = parse_analysis_output(
            r#"{"summary":"Still useful.","displayHints":{"schemaVersion":1,"mode":"magazine","confidence":4}}"#,
        )
        .expect("invalid optional hints must not fail the main analysis");
        assert!(normalize_display_hints(invalid.display_hints.as_ref()).is_none());

        let malformed =
            parse_analysis_output(r#"{"summary":"Still useful.","displayHints":"not-an-object"}"#)
                .expect("malformed optional hints must not fail the main analysis");
        assert!(normalize_display_hints(malformed.display_hints.as_ref()).is_none());
    }

    #[test]
    fn prompt_contains_object_context() {
        let prompt = build_general_enrichment_prompt(&AIEnrichmentInput {
            object_id: "obj".to_string(),
            object_type: "article".to_string(),
            title: Some("A title".to_string()),
            canonical_url: Some("https://example.com".to_string()),
            privacy_level: "personal".to_string(),
            parsed_document_id: "doc".to_string(),
            source_snapshot_id: None,
            text_content: "A useful paragraph".to_string(),
            content_hash: "hash".to_string(),
        });

        assert!(prompt.contains("A title"));
        assert!(prompt.contains("https://example.com"));
        assert!(prompt.contains("A useful paragraph"));
        assert!(prompt.contains("displayHints"));
        assert!(prompt.contains("code-heavy"));
    }

    #[tokio::test]
    async fn auto_enrichment_skips_without_chat_config_and_does_not_create_job() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let service = AIEnrichmentService::new(database.pool().clone(), SecretStore::default());

        let run = service
            .run_auto_enrichment_for_object("missing-object")
            .await
            .expect("missing config should be a clean skip");

        let job_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM background_jobs WHERE job_type = 'ai.enrich_object'",
        )
        .fetch_one(database.pool())
        .await
        .expect("job count should be readable");

        assert!(run.is_none());
        assert_eq!(job_count, 0);
    }
}

use crate::domain::ai::{
    AIAnalysisSubmission, AIDisplayHintsV1, AIEnrichmentInput, AIEnrichmentRunResult,
    AIModelAnalysisOutput, AIModelTagSuggestion, AITraceSubmission, ModelProviderConfig,
    ModelProviderConfigView, ModelProviderTestResult, StoredModelProviderConfig,
};
use crate::domain::organization::NewTagSuggestion;
use crate::errors::{AppError, AppResult};
use crate::repositories::ai::AIRepository;
use crate::repositories::organization::normalize_name;
use crate::runtime::models::{
    ChatOutputFormat, ModelProviderRegistry, TextGenerationRequest, TextGenerationResponse,
};
use crate::state::{AppState, SecretStore};
use crate::storage::object_store::sha256_hex;
use crate::telemetry::{StructuredLogEvent, StructuredLogger};
use chrono::Utc;
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::BTreeSet;
use tauri::Emitter;
use uuid::Uuid;

const GENERAL_ENRICHMENT_PROMPT_ID: &str = "builtin.general_enrichment";
const GENERAL_ENRICHMENT_PROMPT_VERSION: &str = "0.3.0";
const MAX_MODEL_INPUT_CHARS: usize = 24_000;

#[derive(Clone)]
pub struct AIEnrichmentService {
    repository: AIRepository,
    model_registry: ModelProviderRegistry,
    secrets: SecretStore,
    structured_logger: Option<StructuredLogger>,
}

impl AIEnrichmentService {
    pub fn new(pool: SqlitePool, secrets: SecretStore) -> AppResult<Self> {
        Ok(Self {
            repository: AIRepository::new(pool),
            model_registry: ModelProviderRegistry::new()?,
            secrets,
            structured_logger: None,
        })
    }

    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            repository: AIRepository::new(state.database()?.pool().clone()),
            model_registry: state.model_registry().clone(),
            secrets: state.secrets().clone(),
            structured_logger: state.structured_logger().cloned(),
        })
    }

    #[cfg(test)]
    fn with_structured_logger(mut self, structured_logger: StructuredLogger) -> Self {
        self.structured_logger = Some(structured_logger);
        self
    }

    #[cfg(test)]
    fn with_model_registry(mut self, model_registry: ModelProviderRegistry) -> Self {
        self.model_registry = model_registry;
        self
    }

    pub async fn update_model_provider_config(
        &self,
        mut config: ModelProviderConfig,
    ) -> AppResult<()> {
        if config.id.is_none() {
            config.id = self
                .repository
                .get_model_provider_config(&config.provider)
                .await?
                .map(|stored| stored.id);
        }
        self.save_model_provider_config(config).await.map(|_| ())
    }

    pub async fn save_model_provider_config(
        &self,
        config: ModelProviderConfig,
    ) -> AppResult<ModelProviderConfigView> {
        let config = normalize_model_provider_config(config);
        validate_model_provider_config(&config)?;
        if !self.model_registry.supports(config.api_family) {
            return Err(AppError::PolicyDenied(format!(
                "model API family '{}' is not registered",
                config.api_family.as_str()
            )));
        }

        let config_id = match config.id.as_deref() {
            Some(value) => normalize_model_provider_config_id(value)?,
            None => Uuid::new_v4().to_string(),
        };
        let existing = self
            .repository
            .get_model_provider_config_by_id(&config_id)
            .await?;
        let default_id = self.repository.get_default_chat_config_id().await?;
        if !config.enabled && default_id.as_deref() == Some(config_id.as_str()) {
            return Err(AppError::PolicyDenied(
                "select another default chat model before disabling this config".to_string(),
            ));
        }

        let secret_ref =
            self.store_or_resolve_secret_ref(&config_id, &config, existing.as_ref())?;
        self.repository
            .save_model_provider_config(&config_id, &config, secret_ref.as_deref())
            .await?;

        if default_id.is_none()
            && config.enabled
            && config
                .capabilities
                .iter()
                .any(|capability| capability == "chat")
        {
            self.repository.set_default_chat_config(&config_id).await?;
        }

        let stored = self
            .repository
            .get_model_provider_config_by_id(&config_id)
            .await?
            .ok_or_else(|| {
                AppError::Unknown("saved model provider config is missing".to_string())
            })?;
        self.model_provider_config_view(stored).await
    }

    pub async fn list_model_provider_configs(&self) -> AppResult<Vec<ModelProviderConfigView>> {
        let configs = self.repository.list_model_provider_configs().await?;
        let mut views = Vec::with_capacity(configs.len());
        for config in configs {
            views.push(self.model_provider_config_view(config).await?);
        }
        Ok(views)
    }

    pub async fn delete_model_provider_config(&self, config_id: &str) -> AppResult<()> {
        let config = self
            .repository
            .get_model_provider_config_by_id(config_id)
            .await?
            .ok_or_else(|| AppError::PolicyDenied("model provider config not found".to_string()))?;
        if let Some(secret_ref) = config.secret_ref.as_deref() {
            self.secrets.delete(secret_ref)?;
        }
        self.repository
            .delete_model_provider_config(config_id)
            .await
    }

    pub async fn set_default_model_provider(&self, config_id: &str) -> AppResult<()> {
        self.repository.set_default_chat_config(config_id).await
    }

    pub async fn get_model_provider_config(&self) -> AppResult<Option<ModelProviderConfigView>> {
        let Some(config) = self.repository.get_latest_model_provider_config().await? else {
            return Ok(None);
        };
        self.model_provider_config_view(config).await.map(Some)
    }

    async fn model_provider_config_view(
        &self,
        config: StoredModelProviderConfig,
    ) -> AppResult<ModelProviderConfigView> {
        let default_id = self.repository.get_default_chat_config_id().await?;
        let has_api_key = self.resolve_api_key(&config)?.is_some();

        Ok(ModelProviderConfigView {
            id: config.id.clone(),
            provider: config.provider,
            api_family: config.api_family,
            chat_base_url: config.chat_base_url,
            embeddings_base_url: config.embeddings_base_url,
            default_chat_model: config.default_chat_model,
            default_embedding_model: config.default_embedding_model,
            capabilities: config.capabilities,
            has_api_key,
            enabled: config.enabled,
            is_default: default_id.as_deref() == Some(config.id.as_str()),
        })
    }

    pub async fn test_model_provider_config(
        &self,
        config: ModelProviderConfig,
    ) -> AppResult<ModelProviderTestResult> {
        let config = normalize_model_provider_config(config);
        validate_model_provider_config(&config)?;
        if !self.model_registry.supports(config.api_family) {
            return Err(AppError::PolicyDenied(format!(
                "model API family '{}' is not registered",
                config.api_family.as_str()
            )));
        }

        let base_url = config
            .chat_base_url
            .clone()
            .ok_or_else(|| AppError::PolicyDenied("chat base URL is required".to_string()))?;
        let model = config
            .default_chat_model
            .clone()
            .ok_or_else(|| AppError::PolicyDenied("default chat model is required".to_string()))?;
        let api_key = self.resolve_api_key_for_candidate(&config).await?;
        if api_key.is_none() && !is_local_base_url(&base_url) {
            return Err(AppError::ModelAuth);
        }

        let response = self
            .model_registry
            .generate(TextGenerationRequest {
                provider: config.provider.clone(),
                api_family: config.api_family,
                base_url,
                api_key,
                model: model.clone(),
                system_prompt: "Return strict JSON only.".to_string(),
                user_prompt: "Return exactly {\"status\":\"ok\"}.".to_string(),
                temperature: Some(0.0),
                output_format: ChatOutputFormat::JsonObject,
            })
            .await?;
        let value = extract_json_value(&response.content)?;
        if value.get("status").and_then(Value::as_str) != Some("ok") {
            return Err(AppError::ModelOutputSchema(
                "model connection test returned an unexpected JSON payload".to_string(),
            ));
        }

        Ok(ModelProviderTestResult {
            provider: config.provider,
            api_family: config.api_family,
            model,
            latency_ms: response.latency_ms,
        })
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
        self.run_enrichment_for_object_with_request_id(object_id, None)
            .await
    }

    pub async fn run_enrichment_for_object_with_request_id(
        &self,
        object_id: &str,
        request_id: Option<&str>,
    ) -> AppResult<AIEnrichmentRunResult> {
        let request_id = request_id
            .map(|value| {
                Uuid::parse_str(value)
                    .map(|value| value.to_string())
                    .map_err(|_| AppError::PolicyDenied("ai.request_id_invalid".to_string()))
            })
            .transpose()?;
        if let Some(request_id) = request_id.as_deref() {
            if let Some(existing) = self
                .repository
                .find_enrichment_operation(request_id, object_id)
                .await?
            {
                self.record_log(
                    StructuredLogEvent::info(
                        "ai",
                        "ai.enrichment.reused",
                        "AI enrichment request reused an existing operation.",
                    )
                    .with_correlation_id(request_id)
                    .with_object_id(object_id)
                    .with_job_id(request_id),
                )
                .await;
                return Ok(existing);
            }
        }

        let correlation_id = request_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let job_id = match self
            .repository
            .create_enrichment_job_with_id(object_id, &correlation_id, request_id.as_deref())
            .await
        {
            Ok(job_id) => job_id,
            Err(error) => {
                self.record_log(
                    StructuredLogEvent::error(
                        "ai",
                        "ai.enrichment.submit_failed",
                        "AI enrichment submission failed.",
                    )
                    .with_correlation_id(&correlation_id)
                    .with_object_id(object_id)
                    .with_error_code("ai.local_failure"),
                )
                .await;
                return Err(error);
            }
        };

        self.record_log(
            StructuredLogEvent::info("ai", "ai.enrichment.submitted", "AI enrichment submitted.")
                .with_correlation_id(&correlation_id)
                .with_object_id(object_id)
                .with_job_id(&job_id),
        )
        .await;
        self.record_log(
            StructuredLogEvent::info("ai", "ai.enrichment.started", "AI enrichment started.")
                .with_correlation_id(&correlation_id)
                .with_object_id(object_id)
                .with_job_id(&job_id),
        )
        .await;

        let result = self
            .run_enrichment_job(&job_id, object_id, &correlation_id)
            .await;

        match result {
            Ok(analysis_id) => {
                self.record_log(
                    StructuredLogEvent::info(
                        "ai",
                        "ai.enrichment.succeeded",
                        "AI enrichment succeeded.",
                    )
                    .with_correlation_id(&correlation_id)
                    .with_object_id(object_id)
                    .with_job_id(&job_id),
                )
                .await;

                Ok(AIEnrichmentRunResult {
                    job_id,
                    correlation_id,
                    analysis_id: Some(analysis_id),
                    status: "succeeded".to_string(),
                    failure_reason: None,
                })
            }
            Err(error) => {
                let failure_reason = ai_failure_reason(&error);
                let error_code = ai_failure_code(&failure_reason);
                if let Err(persistence_error) = self
                    .repository
                    .fail_enrichment_job(
                        &job_id,
                        object_id,
                        &correlation_id,
                        &failure_reason,
                        &error_code,
                    )
                    .await
                {
                    self.record_log(
                        StructuredLogEvent::error(
                            "ai",
                            "ai.enrichment.persist_failed",
                            "AI enrichment failure persistence failed.",
                        )
                        .with_correlation_id(&correlation_id)
                        .with_object_id(object_id)
                        .with_job_id(&job_id)
                        .with_error_code("ai.local_failure"),
                    )
                    .await;
                    return Err(persistence_error);
                }

                self.record_log(
                    StructuredLogEvent::error(
                        "ai",
                        "ai.enrichment.failed",
                        "AI enrichment failed.",
                    )
                    .with_correlation_id(&correlation_id)
                    .with_object_id(object_id)
                    .with_job_id(&job_id)
                    .with_error_code(&error_code),
                )
                .await;

                Ok(AIEnrichmentRunResult {
                    job_id,
                    correlation_id,
                    analysis_id: None,
                    status: "failed".to_string(),
                    failure_reason: Some(failure_reason),
                })
            }
        }
    }

    async fn record_log(&self, event: StructuredLogEvent) {
        if let Some(logger) = &self.structured_logger {
            let _ = logger.record(event).await;
        }
    }

    async fn run_enrichment_job(
        &self,
        job_id: &str,
        object_id: &str,
        correlation_id: &str,
    ) -> AppResult<String> {
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
        let normalized_tags = normalize_model_tag_suggestions(&analysis_output.tags);
        let tag_names = normalized_tags
            .iter()
            .map(|(name, _, _, _)| name.clone())
            .collect::<Vec<_>>();
        let tag_suggestions = normalized_tags
            .into_iter()
            .map(
                |(name, normalized_name, confidence, rationale)| NewTagSuggestion {
                    id: Uuid::new_v4().to_string(),
                    object_id: input.object_id.clone(),
                    analysis_id: analysis_id.clone(),
                    name,
                    normalized_name,
                    confidence,
                    rationale,
                    created_at: now.clone(),
                },
            )
            .collect();

        let analysis = AIAnalysisSubmission {
            id: analysis_id.clone(),
            object_id: input.object_id.clone(),
            parsed_document_id: input.parsed_document_id.clone(),
            analysis_type: "general_summary".to_string(),
            schema_version: 3,
            summary: analysis_output.summary,
            category: analysis_output.category,
            tags_json: serialize_json(&tag_names)?,
            key_points_json: serialize_json(&analysis_output.key_points)?,
            claims_json: serialize_json(&analysis_output.claims)?,
            action_items_json: serialize_json(&analysis_output.action_items)?,
            risks_json: serialize_json(&analysis_output.risks)?,
            quality_score: analysis_output.quality_score,
            confidence: analysis_output.confidence,
            display_hints_json,
            tag_suggestions,
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
            .complete_enrichment_job(job_id, correlation_id, &analysis, &trace)
            .await?;

        Ok(analysis_id)
    }

    async fn call_chat_model(
        &self,
        config: &StoredModelProviderConfig,
        model: &str,
        prompt: &str,
    ) -> AppResult<TextGenerationResponse> {
        let base_url = config
            .chat_base_url
            .clone()
            .ok_or_else(|| AppError::PolicyDenied("chat base URL is required".to_string()))?;
        let api_key = self.resolve_api_key(config)?;
        if api_key.is_none() && !is_local_model_config(config) {
            return Err(AppError::ModelAuth);
        }

        self.model_registry
            .generate(TextGenerationRequest {
                provider: config.provider.clone(),
                api_family: config.api_family,
                base_url,
                api_key,
                model: model.to_string(),
                system_prompt: "You analyze saved knowledge objects. Return strict JSON only."
                    .to_string(),
                user_prompt: prompt.to_string(),
                temperature: Some(0.2),
                output_format: ChatOutputFormat::JsonObject,
            })
            .await
    }

    fn store_or_resolve_secret_ref(
        &self,
        config_id: &str,
        config: &ModelProviderConfig,
        existing: Option<&StoredModelProviderConfig>,
    ) -> AppResult<Option<String>> {
        if let Some(api_key) = config
            .api_key
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            let secret_ref = format!("keyring:model-provider:{config_id}");
            self.secrets.set(&secret_ref, api_key.to_string())?;
            return Ok(Some(secret_ref));
        }

        if existing
            .and_then(|stored| stored.secret_ref.as_ref())
            .is_some()
        {
            return Ok(None);
        }

        for env_key in provider_env_keys(&config.provider) {
            if std::env::var(&env_key)
                .ok()
                .is_some_and(|value| !value.is_empty())
            {
                return Ok(Some(format!("env:{env_key}")));
            }
        }

        Ok(None)
    }

    fn resolve_api_key(&self, config: &StoredModelProviderConfig) -> AppResult<Option<String>> {
        if let Some(secret_ref) = &config.secret_ref {
            if let Some(value) = self.secrets.resolve(secret_ref)? {
                return Ok(Some(value));
            }
        }

        resolve_provider_env_api_key(&config.provider)
    }

    async fn resolve_api_key_for_candidate(
        &self,
        config: &ModelProviderConfig,
    ) -> AppResult<Option<String>> {
        if let Some(api_key) = config
            .api_key
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            return Ok(Some(api_key.to_string()));
        }

        let stored = if let Some(config_id) = config.id.as_deref() {
            self.repository
                .get_model_provider_config_by_id(config_id)
                .await?
        } else {
            self.repository
                .get_model_provider_config(&config.provider)
                .await?
        };
        if let Some(stored) = stored {
            if let Some(api_key) = self.resolve_api_key(&stored)? {
                return Ok(Some(api_key));
            }
        }

        resolve_provider_env_api_key(&config.provider)
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
                "correlationId": run.correlation_id,
                "objectId": object_id,
                "status": run.status,
                "analysisId": run.analysis_id,
                "failureReason": run.failure_reason,
            }),
            Ok(None) => return,
            Err(error) => json!({
                "objectId": object_id,
                "status": "failed",
                "failureReason": ai_failure_reason(&error),
            }),
        };

        let _ = app_handle.emit("ai://enrichment-completed", payload);
        let _ = app_handle.emit("library://objects-updated", ());
    });
}

fn normalize_model_provider_config_id(config_id: &str) -> AppResult<String> {
    let config_id = config_id.trim();
    if config_id.is_empty()
        || config_id.len() > 128
        || !config_id.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(AppError::PolicyDenied(
            "invalid model provider config id".to_string(),
        ));
    }

    Ok(config_id.to_string())
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

    let parsed_url = config
        .chat_base_url
        .as_ref()
        .and_then(|url| Url::parse(url.trim()).ok());
    if !parsed_url.as_ref().is_some_and(|url| {
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
    }) {
        return Err(AppError::PolicyDenied(
            "valid HTTP(S) chat base URL without credentials, query, or fragment is required"
                .to_string(),
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

fn normalize_model_provider_config(mut config: ModelProviderConfig) -> ModelProviderConfig {
    config.provider = config.provider.trim().to_string();
    config.chat_base_url = trim_optional(config.chat_base_url);
    config.embeddings_base_url = trim_optional(config.embeddings_base_url);
    config.api_key = trim_optional(config.api_key);
    config.default_chat_model = trim_optional(config.default_chat_model);
    config.default_embedding_model = trim_optional(config.default_embedding_model);
    config.capabilities = config
        .capabilities
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect();
    config.capabilities.sort();
    config.capabilities.dedup();
    config
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalized_provider_id(provider: &str) -> String {
    provider
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn provider_env_keys(provider: &str) -> Vec<String> {
    let provider_key = normalized_provider_id(provider).to_ascii_uppercase();
    let mut keys = vec![format!("LINK_WORLD_{provider_key}_API_KEY")];
    if matches!(provider_key.as_str(), "OPENAI" | "OPENAI_COMPATIBLE") {
        keys.push("LINK_WORLD_OPENAI_API_KEY".to_string());
    }
    keys.sort();
    keys.dedup();
    keys
}

fn resolve_provider_env_api_key(provider: &str) -> AppResult<Option<String>> {
    for env_key in provider_env_keys(provider) {
        if let Ok(value) = std::env::var(&env_key) {
            if !value.is_empty() {
                return Ok(Some(value));
            }
        }
    }
    Ok(None)
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

fn is_local_model_config(config: &StoredModelProviderConfig) -> bool {
    config
        .chat_base_url
        .as_deref()
        .is_some_and(is_local_base_url)
}

fn is_local_base_url(base_url: &str) -> bool {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .is_some_and(|host| matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1"))
}

fn ai_failure_code(failure_reason: &str) -> String {
    let code = failure_reason
        .split_once(':')
        .map(|(code, _)| code)
        .unwrap_or("ai.failed");
    if code.starts_with("ai.")
        && code.len() <= 64
        && code.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_')
        })
    {
        code.to_string()
    } else {
        "ai.failed".to_string()
    }
}

fn ai_failure_reason(error: &AppError) -> String {
    match error {
        AppError::NetworkTimeout => {
            "ai.timeout: The model provider request timed out. Retry later, choose another provider, or use a local model if the remote provider is unavailable.".to_string()
        }
        AppError::ModelAuth => {
            "ai.model_auth: Model authentication failed. Check the provider API key in Settings, then run analysis again.".to_string()
        }
        AppError::ModelRateLimit => {
            "ai.rate_limit: The model provider rate-limited this request. Wait and retry, or switch the default model provider in Settings.".to_string()
        }
        AppError::ModelNotFound => {
            "ai.model_not_found: The configured model or provider endpoint was not found. Check the model name and base URL in Settings.".to_string()
        }
        AppError::ModelOutputSchema(_) => {
            "ai.output_schema: The model returned a response that did not match Link World's analysis schema. Retry with the same provider or choose a stronger JSON-capable model.".to_string()
        }
        AppError::PolicyDenied(message) => ai_policy_failure_reason(message),
        AppError::SecretStorage => {
            "ai.secret_storage: Link World could not read the saved model credential. Re-save the provider API key in Settings.".to_string()
        }
        AppError::ObjectNotFound => {
            "ai.input_unavailable: The selected object or parsed document is no longer available for AI analysis.".to_string()
        }
        AppError::Unknown(message) if looks_like_retryable_provider_failure(message) => {
            "ai.provider_unavailable: The model provider returned a temporary server or network failure after retries. Retry later or switch providers.".to_string()
        }
        AppError::Database(_) | AppError::Filesystem(_) => {
            "ai.local_failure: AI analysis could not update local state. Retry after checking local storage and diagnostics.".to_string()
        }
        _ => {
            "ai.failed: AI analysis failed before a usable result was saved. Retry or check model provider settings.".to_string()
        }
    }
}

fn ai_policy_failure_reason(message: &str) -> String {
    let lower_message = message.to_ascii_lowercase();

    if lower_message.contains("no enabled chat model provider") {
        return "ai.not_configured: No enabled default chat model is configured. Open Settings > Models and choose a default chat provider.".to_string();
    }

    if lower_message.contains("chat base url") || lower_message.contains("default chat model") {
        return "ai.provider_config_invalid: The default chat provider is missing a valid base URL or model name. Update it in Settings, then run analysis again.".to_string();
    }

    if lower_message.contains("sensitive") || lower_message.contains("secret") {
        return "ai.policy_denied: This object's privacy level blocks non-local AI analysis. Use a local model provider or change the object's privacy boundary before retrying.".to_string();
    }

    "ai.policy_denied: Link World policy blocked AI analysis for this object or provider configuration.".to_string()
}

fn looks_like_retryable_provider_failure(message: &str) -> bool {
    let lower_message = message.to_ascii_lowercase();

    lower_message.contains("http 500")
        || lower_message.contains("http 502")
        || lower_message.contains("http 503")
        || lower_message.contains("http 504")
        || lower_message.contains("network request failed")
        || lower_message.contains("provider request failed")
}

fn build_general_enrichment_prompt(input: &AIEnrichmentInput) -> String {
    let text = truncate_chars(&input.text_content, MAX_MODEL_INPUT_CHARS);
    format!(
        r#"Analyze this saved knowledge object and return strict JSON with this schema:
{{
  "summary": "2-4 sentence concise summary",
  "category": "short category",
  "tags": [{{"name":"tag","confidence":0.0,"rationale":"why this tag applies"}}],
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

fn normalize_model_tag_suggestions(
    values: &[AIModelTagSuggestion],
) -> Vec<(String, String, Option<f64>, Option<String>)> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();
    for value in values.iter().take(16) {
        let (name, confidence, rationale) = value.parts();
        let name = truncate_chars(name.trim(), 48);
        if name.chars().count() < 2 || name.chars().any(char::is_control) {
            continue;
        }
        let normalized_name = normalize_name(&name);
        if normalized_name.is_empty() || !seen.insert(normalized_name.clone()) {
            continue;
        }
        let confidence =
            confidence.filter(|value| value.is_finite() && (0.0..=1.0).contains(value));
        let rationale = rationale
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| truncate_chars(value, 160));
        normalized.push((name, normalized_name, confidence, rationale));
        if normalized.len() == 8 {
            break;
        }
    }
    normalized
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

#[cfg(test)]
mod tests {
    use super::{
        ai_failure_code, ai_failure_reason, build_general_enrichment_prompt,
        normalize_display_hints, parse_analysis_output, validate_model_provider_config,
        AIEnrichmentService,
    };
    use crate::domain::ai::{AIEnrichmentInput, ModelApiFamily, ModelProviderConfig};
    use crate::errors::{AppError, AppResult};
    use crate::runtime::models::{
        ModelProviderRegistry, TextGenerationProvider, TextGenerationRequest,
        TextGenerationResponse,
    };
    use crate::state::SecretStore;
    use crate::storage::database::Database;
    use crate::telemetry::StructuredLogger;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use uuid::Uuid;

    struct SuccessfulTextGenerationProvider;

    impl TextGenerationProvider for SuccessfulTextGenerationProvider {
        fn implementation_id(&self) -> &'static str {
            "test-success"
        }

        fn supports(&self, _api_family: ModelApiFamily) -> bool {
            true
        }

        fn generate(
            &self,
            _request: TextGenerationRequest,
        ) -> Pin<Box<dyn Future<Output = AppResult<TextGenerationResponse>> + Send + '_>> {
            Box::pin(async {
                Ok(TextGenerationResponse {
                    content: r#"{"summary":"Synthetic AI summary.","category":"engineering","tags":["rust"],"keyPoints":[],"claims":[],"actionItems":[],"risks":[]}"#.to_string(),
                    prompt_tokens: Some(10),
                    completion_tokens: Some(5),
                    latency_ms: 12,
                })
            })
        }
    }

    #[test]
    fn parses_json_object_from_model_output() {
        let output = parse_analysis_output(
            r#"{"summary":"Useful technique.","category":"engineering","tags":["rust"],"keyPoints":["Keep traces"],"qualityScore":0.8,"confidence":0.7}"#,
        )
        .expect("model output should parse");

        assert_eq!(output.summary, "Useful technique.");
        assert_eq!(output.tags[0].parts().0, "rust");
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

    #[test]
    fn validates_provider_config_without_allowing_credentials_in_base_url() {
        let mut config = ModelProviderConfig {
            id: None,
            provider: "custom".to_string(),
            api_family: ModelApiFamily::OpenAiChatCompletions,
            chat_base_url: Some("https://api.example.com/v1".to_string()),
            embeddings_base_url: None,
            api_key: None,
            default_chat_model: Some("test-model".to_string()),
            default_embedding_model: None,
            capabilities: vec!["chat".to_string()],
            enabled: true,
        };

        validate_model_provider_config(&config).expect("valid provider config should pass");

        config.chat_base_url =
            Some("https://user:secret@api.example.com/v1?key=secret".to_string());
        assert!(validate_model_provider_config(&config).is_err());
    }

    #[test]
    fn ai_failure_reason_maps_provider_failures_to_stable_user_codes() {
        let auth = ai_failure_reason(&AppError::ModelAuth);
        assert!(auth.starts_with("ai.model_auth:"));
        assert!(auth.contains("Settings"));
        assert_eq!(ai_failure_code(&auth), "ai.model_auth");
        assert_eq!(ai_failure_code("raw provider body"), "ai.failed");

        let timeout = ai_failure_reason(&AppError::NetworkTimeout);
        assert!(timeout.starts_with("ai.timeout:"));
        assert!(timeout.contains("Retry"));

        let rate_limit = ai_failure_reason(&AppError::ModelRateLimit);
        assert!(rate_limit.starts_with("ai.rate_limit:"));

        let schema = ai_failure_reason(&AppError::ModelOutputSchema(
            "provider returned raw invalid JSON with details".to_string(),
        ));
        assert!(schema.starts_with("ai.output_schema:"));
        assert!(!schema.contains("raw invalid JSON"));

        let unavailable = ai_failure_reason(&AppError::Unknown(
            "model provider returned HTTP 503 Service Unavailable with body".to_string(),
        ));
        assert!(unavailable.starts_with("ai.provider_unavailable:"));
        assert!(!unavailable.contains("body"));
    }

    #[test]
    fn ai_failure_reason_maps_policy_failures_to_recovery_actions() {
        let not_configured = ai_failure_reason(&AppError::PolicyDenied(
            "no enabled chat model provider configured".to_string(),
        ));
        assert!(not_configured.starts_with("ai.not_configured:"));

        let privacy = ai_failure_reason(&AppError::PolicyDenied(
            "sensitive or secret objects cannot be sent to non-local model providers".to_string(),
        ));
        assert!(privacy.starts_with("ai.policy_denied:"));
        assert!(privacy.contains("local model"));
    }

    #[tokio::test]
    async fn successful_enrichment_persists_and_logs_one_correlation_without_content() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, privacy_level, lifecycle_status,
                captured_at, updated_at
            ) VALUES (
                'obj-ai-success', 'local', 'article', 'AI success fixture', 'personal', 'parsed',
                '2026-06-30T00:00:00Z', '2026-06-30T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("fixture object should insert");
        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash,
                parser_id, parser_version, created_at
            ) VALUES (
                'parsed-ai-success', 'obj-ai-success', 'AI success fixture',
                'Synthetic private body that must never enter operational evidence.',
                9, 'hash-ai-success', 'test.parser', '0.1.0', '2026-06-30T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("parsed document should insert");

        let telemetry_dir =
            std::env::temp_dir().join(format!("link-world-ai-telemetry-{}", Uuid::new_v4()));
        let service = AIEnrichmentService::new(database.pool().clone(), SecretStore::default())
            .expect("AI service should initialize")
            .with_model_registry(ModelProviderRegistry::from_text_generation_provider(
                Arc::new(SuccessfulTextGenerationProvider),
            ))
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));
        service
            .save_model_provider_config(ModelProviderConfig {
                id: Some("local-test-provider".to_string()),
                provider: "local-test".to_string(),
                api_family: ModelApiFamily::OpenAiChatCompletions,
                chat_base_url: Some("http://127.0.0.1:1/v1".to_string()),
                embeddings_base_url: None,
                api_key: None,
                default_chat_model: Some("test-model".to_string()),
                default_embedding_model: None,
                capabilities: vec!["chat".to_string()],
                enabled: true,
            })
            .await
            .expect("local test provider should save");

        let run = service
            .run_enrichment_for_object("obj-ai-success")
            .await
            .expect("enrichment should succeed");

        assert_eq!(run.status, "succeeded");
        assert!(run.analysis_id.is_some());
        assert!(Uuid::parse_str(&run.correlation_id).is_ok());

        let job_payload: String =
            sqlx::query_scalar("SELECT payload_json FROM background_jobs WHERE id = ?1")
                .bind(&run.job_id)
                .fetch_one(database.pool())
                .await
                .expect("AI job payload should be readable");
        let job_payload: serde_json::Value =
            serde_json::from_str(&job_payload).expect("AI job payload should be JSON");
        assert_eq!(
            job_payload["correlationId"].as_str(),
            Some(run.correlation_id.as_str())
        );

        let events = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT event_type, correlation_id, payload_json
            FROM domain_events
            WHERE causation_id = ?1
            ORDER BY occurred_at, event_type
            "#,
        )
        .bind(&run.job_id)
        .fetch_all(database.pool())
        .await
        .expect("AI events should be readable");
        assert_eq!(events.len(), 2);
        assert_eq!(
            events
                .iter()
                .map(|(event_type, _, _)| event_type.as_str())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["analysis.created", "analysis.requested"])
        );
        assert!(events
            .iter()
            .all(|(_, correlation_id, _)| correlation_id == &run.correlation_id));
        assert!(events.iter().all(|(_, _, payload)| {
            !payload.contains("Synthetic private body")
                && !payload.contains("test-model")
                && !payload.contains("local-test")
        }));

        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("AI logs should be readable");
        assert_eq!(
            logs.iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec![
                "ai.enrichment.submitted",
                "ai.enrichment.started",
                "ai.enrichment.succeeded",
            ]
        );
        assert!(logs.iter().all(|entry| {
            entry.correlation_id.as_deref() == Some(run.correlation_id.as_str())
                && entry.object_id.as_deref() == Some("obj-ai-success")
                && entry.job_id.as_deref() == Some(run.job_id.as_str())
                && !entry.message.contains("Synthetic private body")
                && !entry.message.contains("test-model")
        }));

        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn auto_enrichment_skips_without_chat_config_and_does_not_create_job() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let service = AIEnrichmentService::new(database.pool().clone(), SecretStore::default())
            .expect("AI service should initialize");

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

    #[tokio::test]
    async fn manual_enrichment_persists_stable_failure_reason_without_chat_config() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id,
                user_id,
                object_type,
                title,
                privacy_level,
                lifecycle_status,
                captured_at,
                updated_at
            ) VALUES (
                'obj-ai-failure',
                'local',
                'article',
                'AI failure fixture',
                'personal',
                'parsed',
                '2026-06-25T00:00:00Z',
                '2026-06-25T00:00:00Z'
            )
            "#,
        )
        .execute(database.pool())
        .await
        .expect("fixture object should insert");
        let telemetry_dir =
            std::env::temp_dir().join(format!("link-world-ai-failure-{}", Uuid::new_v4()));
        let service = AIEnrichmentService::new(database.pool().clone(), SecretStore::default())
            .expect("AI service should initialize")
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));

        let run = service
            .run_enrichment_for_object("obj-ai-failure")
            .await
            .expect("manual enrichment should record a failed job");

        assert_eq!(run.status, "failed");
        assert!(run
            .failure_reason
            .as_deref()
            .unwrap_or_default()
            .starts_with("ai.not_configured:"));

        let last_error: String =
            sqlx::query_scalar("SELECT last_error FROM background_jobs WHERE id = ?1")
                .bind(&run.job_id)
                .fetch_one(database.pool())
                .await
                .expect("AI job failure should be persisted");

        assert!(last_error.starts_with("ai.not_configured:"));
        assert!(!last_error.contains("no enabled chat model provider configured"));
        assert!(Uuid::parse_str(&run.correlation_id).is_ok());

        let job_payload: String =
            sqlx::query_scalar("SELECT payload_json FROM background_jobs WHERE id = ?1")
                .bind(&run.job_id)
                .fetch_one(database.pool())
                .await
                .expect("failed AI job payload should be readable");
        let job_payload: serde_json::Value =
            serde_json::from_str(&job_payload).expect("failed AI job payload should be JSON");
        assert_eq!(
            job_payload["correlationId"].as_str(),
            Some(run.correlation_id.as_str())
        );

        let events = sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT event_type, correlation_id, payload_json
            FROM domain_events
            WHERE causation_id = ?1
            ORDER BY occurred_at, event_type
            "#,
        )
        .bind(&run.job_id)
        .fetch_all(database.pool())
        .await
        .expect("failed AI events should be readable");
        assert_eq!(events.len(), 2);
        assert_eq!(
            events
                .iter()
                .map(|(event_type, _, _)| event_type.as_str())
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["analysis.failed", "analysis.requested"])
        );
        assert!(events
            .iter()
            .all(|(_, correlation_id, _)| correlation_id == &run.correlation_id));
        let failed_payload = events
            .iter()
            .find(|(event_type, _, _)| event_type == "analysis.failed")
            .map(|(_, _, payload)| payload)
            .expect("failed event should exist");
        assert_eq!(failed_payload, r#"{"errorCode":"ai.not_configured"}"#);
        assert!(!failed_payload.contains("no enabled chat model provider configured"));

        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("failed AI logs should be readable");
        assert_eq!(
            logs.iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec![
                "ai.enrichment.submitted",
                "ai.enrichment.started",
                "ai.enrichment.failed",
            ]
        );
        assert!(logs.iter().all(|entry| {
            entry.correlation_id.as_deref() == Some(run.correlation_id.as_str())
                && entry.object_id.as_deref() == Some("obj-ai-failure")
                && entry.job_id.as_deref() == Some(run.job_id.as_str())
                && !entry
                    .message
                    .contains("no enabled chat model provider configured")
        }));
        assert_eq!(
            logs.last().and_then(|entry| entry.error_code.as_deref()),
            Some("ai.not_configured")
        );

        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn manual_enrichment_request_id_reuses_terminal_operation_and_rejects_cross_object_use() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        for object_id in ["obj-ai-idempotent", "obj-ai-other"] {
            sqlx::query(
                r#"
                INSERT INTO knowledge_objects (
                    id, user_id, object_type, title, privacy_level, lifecycle_status,
                    captured_at, updated_at
                ) VALUES (?1, 'local', 'article', 'AI idempotency fixture', 'personal', 'parsed',
                    '2026-07-03T00:00:00Z', '2026-07-03T00:00:00Z')
                "#,
            )
            .bind(object_id)
            .execute(database.pool())
            .await
            .expect("fixture object should insert");
        }
        let service = AIEnrichmentService::new(database.pool().clone(), SecretStore::default())
            .expect("AI service should initialize");
        let request_id = Uuid::new_v4().to_string();

        let first = service
            .run_enrichment_for_object_with_request_id("obj-ai-idempotent", Some(&request_id))
            .await
            .expect("first request should produce a terminal failed operation");
        let repeated = service
            .run_enrichment_for_object_with_request_id("obj-ai-idempotent", Some(&request_id))
            .await
            .expect("same request should reuse its terminal operation");

        assert_eq!(first.job_id, request_id);
        assert_eq!(repeated.job_id, first.job_id);
        assert_eq!(repeated.status, first.status);
        let job_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM background_jobs WHERE job_type = 'ai.enrich_object'",
        )
        .fetch_one(database.pool())
        .await
        .expect("job count should be readable");
        assert_eq!(job_count, 1);

        let conflict = service
            .run_enrichment_for_object_with_request_id("obj-ai-other", Some(&request_id))
            .await
            .expect_err("request id must remain bound to its object");
        assert!(matches!(conflict, AppError::PolicyDenied(_)));
    }
}

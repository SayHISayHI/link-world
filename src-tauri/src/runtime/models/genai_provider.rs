use super::contracts::{
    ChatOutputFormat, TextGenerationFuture, TextGenerationProvider, TextGenerationRequest,
    TextGenerationResponse,
};
use crate::domain::ai::ModelApiFamily;
use crate::errors::{AppError, AppResult};
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatOptions, ChatRequest, ChatResponseFormat};
use genai::resolver::{AuthData, Endpoint};
use genai::{Client, Error as GenaiError, ModelIden, ModelSpec, ServiceTarget};
use reqwest::StatusCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MODEL_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_MODEL_ATTEMPTS: usize = 3;

pub struct GenaiTextGenerationProvider {
    client: Client,
}

impl GenaiTextGenerationProvider {
    pub fn new() -> AppResult<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(MODEL_REQUEST_TIMEOUT)
            .build()
            .map_err(|error| {
                AppError::Unknown(format!("failed to build model HTTP client: {error}"))
            })?;

        Ok(Self {
            client: Client::builder().with_reqwest(http_client).build(),
        })
    }

    async fn execute(&self, request: TextGenerationRequest) -> AppResult<TextGenerationResponse> {
        let adapter_kind = resolve_adapter_kind(&request.provider, request.api_family);
        let target = ServiceTarget {
            endpoint: Endpoint::from_owned(normalize_base_url(
                &request.base_url,
                request.api_family,
            )),
            auth: request
                .api_key
                .as_ref()
                .map(AuthData::from_single)
                .unwrap_or(AuthData::None),
            model: ModelIden::new(adapter_kind, request.model.clone()),
        };
        let chat_request = ChatRequest::new(vec![ChatMessage::user(request.user_prompt)])
            .with_system(request.system_prompt);
        let mut options = ChatOptions::default();
        if let Some(temperature) = request.temperature {
            options = options.with_temperature(temperature);
        }
        if request.output_format == ChatOutputFormat::JsonObject {
            options = options.with_response_format(ChatResponseFormat::JsonMode);
        }

        let started_at = Instant::now();
        let mut last_error = None;

        for attempt in 0..MAX_MODEL_ATTEMPTS {
            let result = self
                .client
                .exec_chat(
                    ModelSpec::from_target(target.clone()),
                    chat_request.clone(),
                    Some(&options),
                )
                .await;

            match result {
                Ok(response) => {
                    let prompt_tokens = response.usage.prompt_tokens.map(i64::from);
                    let completion_tokens = response.usage.completion_tokens.map(i64::from);
                    let content = response.into_first_text().ok_or_else(|| {
                        AppError::ModelOutputSchema(
                            "model provider returned no text content".to_string(),
                        )
                    })?;

                    return Ok(TextGenerationResponse {
                        content,
                        prompt_tokens,
                        completion_tokens,
                        latency_ms: elapsed_millis(started_at),
                    });
                }
                Err(error) => {
                    let retry_delay = retry_delay(&error, attempt);
                    if let Some(delay) = retry_delay {
                        last_error = Some(error);
                        tokio::time::sleep(delay).await;
                        continue;
                    }

                    return Err(map_genai_error(error));
                }
            }
        }

        Err(map_genai_error(last_error.unwrap_or_else(|| {
            GenaiError::Internal("model request failed without an error".to_string())
        })))
    }
}

impl TextGenerationProvider for GenaiTextGenerationProvider {
    fn implementation_id(&self) -> &'static str {
        "genai"
    }

    fn supports(&self, api_family: ModelApiFamily) -> bool {
        matches!(
            api_family,
            ModelApiFamily::OpenAiChatCompletions
                | ModelApiFamily::OpenAiResponses
                | ModelApiFamily::AnthropicMessages
                | ModelApiFamily::GoogleGenerativeAi
                | ModelApiFamily::Ollama
        )
    }

    fn generate(&self, request: TextGenerationRequest) -> TextGenerationFuture<'_> {
        Box::pin(self.execute(request))
    }
}

fn resolve_adapter_kind(provider: &str, api_family: ModelApiFamily) -> AdapterKind {
    match api_family {
        ModelApiFamily::OpenAiResponses => AdapterKind::OpenAIResp,
        ModelApiFamily::AnthropicMessages => AdapterKind::Anthropic,
        ModelApiFamily::GoogleGenerativeAi => AdapterKind::Gemini,
        ModelApiFamily::Ollama => AdapterKind::Ollama,
        ModelApiFamily::OpenAiChatCompletions => match normalize_provider_key(provider).as_str() {
            "deepseek" => AdapterKind::DeepSeek,
            "openrouter" | "open_router" => AdapterKind::OpenRouter,
            "groq" => AdapterKind::Groq,
            "xai" => AdapterKind::Xai,
            "moonshot" => AdapterKind::Moonshot,
            "zai" => AdapterKind::Zai,
            "together" => AdapterKind::Together,
            _ => AdapterKind::OpenAI,
        },
    }
}

fn normalize_provider_key(provider: &str) -> String {
    provider.trim().to_ascii_lowercase().replace('-', "_")
}

fn normalize_base_url(base_url: &str, api_family: ModelApiFamily) -> String {
    let base_url = base_url.trim().trim_end_matches('/');
    let operation_suffix = match api_family {
        ModelApiFamily::OpenAiChatCompletions => "/chat/completions",
        ModelApiFamily::OpenAiResponses => "/responses",
        ModelApiFamily::AnthropicMessages => "/messages",
        ModelApiFamily::Ollama => "/api/chat",
        ModelApiFamily::GoogleGenerativeAi => "",
    };
    let base_url = base_url
        .strip_suffix(operation_suffix)
        .unwrap_or(base_url)
        .trim_end_matches('/');

    format!("{base_url}/")
}

fn retry_delay(error: &GenaiError, attempt: usize) -> Option<Duration> {
    if attempt + 1 >= MAX_MODEL_ATTEMPTS || !is_retryable(error) {
        return None;
    }

    if let Some(delay) = retry_after_delay(error) {
        return Some(delay);
    }

    let exponential_ms = 250_u64.saturating_mul(1_u64 << attempt.min(4));
    let jitter_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::from(duration.subsec_millis()) % 125)
        .unwrap_or(0);

    Some(Duration::from_millis(exponential_ms + jitter_ms))
}

fn retry_after_delay(error: &GenaiError) -> Option<Duration> {
    let headers = match error {
        GenaiError::WebModelCall {
            webc_error: genai::webc::Error::ResponseFailedStatus { headers, .. },
            ..
        }
        | GenaiError::WebAdapterCall {
            webc_error: genai::webc::Error::ResponseFailedStatus { headers, .. },
            ..
        } => headers,
        _ => return None,
    };
    let seconds = headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .parse::<u64>()
        .ok()?;

    Some(Duration::from_secs(
        seconds.min(MODEL_REQUEST_TIMEOUT.as_secs()),
    ))
}

fn is_retryable(error: &GenaiError) -> bool {
    match error {
        GenaiError::HttpError { status, .. } => is_retryable_status(*status),
        GenaiError::WebModelCall { webc_error, .. }
        | GenaiError::WebAdapterCall { webc_error, .. } => match webc_error {
            genai::webc::Error::ResponseFailedStatus { status, .. } => is_retryable_status(*status),
            genai::webc::Error::Reqwest(error) => {
                error.is_timeout() || error.is_connect() || error.is_request()
            }
            _ => false,
        },
        _ => false,
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn map_genai_error(error: GenaiError) -> AppError {
    match error {
        GenaiError::RequiresApiKey { .. }
        | GenaiError::NoAuthResolver { .. }
        | GenaiError::NoAuthData { .. } => AppError::ModelAuth,
        GenaiError::HttpError { status, .. } => map_http_status(status),
        GenaiError::WebModelCall { webc_error, .. }
        | GenaiError::WebAdapterCall { webc_error, .. } => map_web_error(webc_error),
        GenaiError::NoChatResponse { .. }
        | GenaiError::InvalidJsonResponseElement { .. }
        | GenaiError::ChatResponseGeneration { .. }
        | GenaiError::ChatResponse { .. } => {
            AppError::ModelOutputSchema("model provider returned an invalid response".to_string())
        }
        other => AppError::Unknown(format!("model provider request failed: {other}")),
    }
}

fn map_web_error(error: genai::webc::Error) -> AppError {
    match error {
        genai::webc::Error::ResponseFailedStatus { status, .. } => map_http_status(status),
        genai::webc::Error::Reqwest(error) if error.is_timeout() => AppError::NetworkTimeout,
        genai::webc::Error::Reqwest(error) => {
            AppError::Unknown(format!("model provider network request failed: {error}"))
        }
        genai::webc::Error::ResponseFailedInvalidJson { .. }
        | genai::webc::Error::ResponseFailedNotJson { .. } => {
            AppError::ModelOutputSchema("model provider returned a non-JSON response".to_string())
        }
        other => AppError::Unknown(format!("model provider request failed: {other}")),
    }
}

fn map_http_status(status: StatusCode) -> AppError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => AppError::ModelAuth,
        StatusCode::TOO_MANY_REQUESTS => AppError::ModelRateLimit,
        StatusCode::NOT_FOUND => AppError::ModelNotFound,
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => AppError::NetworkTimeout,
        _ => AppError::Unknown(format!("model provider returned HTTP {status}")),
    }
}

fn elapsed_millis(started_at: Instant) -> i64 {
    started_at.elapsed().as_millis().min(i64::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::{map_http_status, normalize_base_url, resolve_adapter_kind};
    use crate::domain::ai::ModelApiFamily;
    use crate::errors::AppError;
    use genai::adapter::AdapterKind;

    #[test]
    fn resolves_protocol_family_before_provider_brand() {
        assert_eq!(
            resolve_adapter_kind("custom", ModelApiFamily::AnthropicMessages),
            AdapterKind::Anthropic
        );
        assert_eq!(
            resolve_adapter_kind("openai", ModelApiFamily::OpenAiResponses),
            AdapterKind::OpenAIResp
        );
        assert_eq!(
            resolve_adapter_kind("deepseek", ModelApiFamily::OpenAiChatCompletions),
            AdapterKind::DeepSeek
        );
    }

    #[test]
    fn normalizes_base_url_for_native_and_compatible_adapters() {
        assert_eq!(
            normalize_base_url(
                "https://api.example.com/v1",
                ModelApiFamily::OpenAiChatCompletions
            ),
            "https://api.example.com/v1/"
        );
        assert_eq!(
            normalize_base_url(
                "https://api.example.com/v1/chat/completions",
                ModelApiFamily::OpenAiChatCompletions
            ),
            "https://api.example.com/v1/"
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:11434/api/chat", ModelApiFamily::Ollama),
            "http://127.0.0.1:11434/"
        );
    }

    #[test]
    fn maps_provider_statuses_to_typed_errors() {
        assert!(matches!(
            map_http_status(reqwest::StatusCode::UNAUTHORIZED),
            AppError::ModelAuth
        ));
        assert!(matches!(
            map_http_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
            AppError::ModelRateLimit
        ));
        assert!(matches!(
            map_http_status(reqwest::StatusCode::NOT_FOUND),
            AppError::ModelNotFound
        ));
    }
}

use crate::domain::ai::ModelApiFamily;
use crate::errors::AppResult;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatOutputFormat {
    Text,
    JsonObject,
}

#[derive(Clone)]
pub struct TextGenerationRequest {
    pub provider: String,
    pub api_family: ModelApiFamily,
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub temperature: Option<f64>,
    pub output_format: ChatOutputFormat,
}

impl fmt::Debug for TextGenerationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextGenerationRequest")
            .field("provider", &self.provider)
            .field("api_family", &self.api_family)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "REDACTED"))
            .field("model", &self.model)
            .field("temperature", &self.temperature)
            .field("output_format", &self.output_format)
            .finish_non_exhaustive()
    }
}

pub struct TextGenerationResponse {
    pub content: String,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub latency_ms: i64,
}

pub type TextGenerationFuture<'a> =
    Pin<Box<dyn Future<Output = AppResult<TextGenerationResponse>> + Send + 'a>>;

pub trait TextGenerationProvider: Send + Sync {
    fn implementation_id(&self) -> &'static str;

    fn supports(&self, api_family: ModelApiFamily) -> bool;

    fn generate(&self, request: TextGenerationRequest) -> TextGenerationFuture<'_>;
}

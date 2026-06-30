use super::contracts::{TextGenerationProvider, TextGenerationRequest, TextGenerationResponse};
use super::genai_provider::GenaiTextGenerationProvider;
use crate::domain::ai::ModelApiFamily;
use crate::errors::{AppError, AppResult};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct ModelProviderRegistry {
    text_generation: Arc<Vec<Arc<dyn TextGenerationProvider>>>,
}

impl ModelProviderRegistry {
    pub fn new() -> AppResult<Self> {
        let providers: Vec<Arc<dyn TextGenerationProvider>> =
            vec![Arc::new(GenaiTextGenerationProvider::new()?)];

        Ok(Self {
            text_generation: Arc::new(providers),
        })
    }

    pub fn supports(&self, api_family: ModelApiFamily) -> bool {
        self.text_generation
            .iter()
            .any(|provider| provider.supports(api_family))
    }

    #[cfg(test)]
    pub(crate) fn from_text_generation_provider(provider: Arc<dyn TextGenerationProvider>) -> Self {
        Self {
            text_generation: Arc::new(vec![provider]),
        }
    }

    pub async fn generate(
        &self,
        request: TextGenerationRequest,
    ) -> AppResult<TextGenerationResponse> {
        let provider = self
            .text_generation
            .iter()
            .find(|provider| provider.supports(request.api_family))
            .ok_or_else(|| {
                AppError::PolicyDenied(format!(
                    "model API family '{}' is not registered",
                    request.api_family.as_str()
                ))
            })?;

        provider.generate(request).await
    }
}

impl fmt::Debug for ModelProviderRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let implementations = self
            .text_generation
            .iter()
            .map(|provider| provider.implementation_id())
            .collect::<Vec<_>>();

        formatter
            .debug_struct("ModelProviderRegistry")
            .field("text_generation", &implementations)
            .finish()
    }
}

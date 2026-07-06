use crate::domain::ai::AIEnrichmentRunResult;
use crate::domain::capture::CaptureFetchJobRunResult;
use crate::domain::jobs::{BackgroundJob, RetriedBackgroundJob};
use crate::errors::{AppError, AppResult};
use crate::repositories::jobs::JobsRepository;
use crate::services::ai::AIEnrichmentService;
use crate::services::capture::CaptureService;
use crate::state::AppState;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetriedJobRunResult {
    pub capture: Option<CaptureFetchJobRunResult>,
    pub analysis: Option<AIEnrichmentRunResult>,
}

#[derive(Clone)]
pub struct OperationsService {
    repository: JobsRepository,
    capture: CaptureService,
    ai: AIEnrichmentService,
}

impl OperationsService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            repository: JobsRepository::new(state.database()?.pool().clone()),
            capture: CaptureService::from_state(state)?,
            ai: AIEnrichmentService::from_state(state)?,
        })
    }

    pub async fn get_background_job(&self, job_id: &str) -> AppResult<BackgroundJob> {
        self.repository.get_background_job(job_id).await
    }

    pub async fn list_object_jobs(
        &self,
        object_id: &str,
        limit: Option<i64>,
    ) -> AppResult<Vec<BackgroundJob>> {
        self.repository.list_object_jobs(object_id, limit).await
    }

    pub async fn reserve_retry(&self, job_id: &str) -> AppResult<RetriedBackgroundJob> {
        let retried = self.repository.retry_background_job(job_id).await?;
        if retried.job_type != "capture.fetch_url" {
            return Err(AppError::PolicyDenied(
                "retry runner is not registered for this job type".to_string(),
            ));
        }
        Ok(retried)
    }

    pub async fn run_retry(
        &self,
        retried: &RetriedBackgroundJob,
    ) -> AppResult<RetriedJobRunResult> {
        if retried.job_type != "capture.fetch_url" {
            return Err(AppError::PolicyDenied(
                "retry runner is not registered for this job type".to_string(),
            ));
        }
        let capture = self.capture.run_fetch_job(&retried.id).await?;
        let analysis = if let Some(object_id) = capture.as_ref().and_then(|result| {
            (result.status == "succeeded" && result.parsed_document_id.is_some())
                .then(|| result.object_id.clone())
        }) {
            self.ai.run_auto_enrichment_for_object(&object_id).await?
        } else {
            None
        };
        Ok(RetriedJobRunResult { capture, analysis })
    }
}

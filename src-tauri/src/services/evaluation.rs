use crate::domain::evaluation::{
    EvaluationArtifactSubmission, EvaluationFailureSubmission, EvaluationInput,
    EvaluationOperation, EvaluationOutput, EvaluationPlan, EvaluationRunReservation,
    EvaluationRunSubmission, EvaluationTraceCompletion, EvaluatorCapability,
    TriggerEvaluationResponse, DEFAULT_EVALUATION_TIMEOUT_MS, EVALUATION_CAPABILITY_SCHEMA_VERSION,
    EVALUATION_INPUT_SCHEMA_VERSION, EVALUATION_OUTPUT_SCHEMA_VERSION,
    EVALUATION_PLAN_SCHEMA_VERSION, GITHUB_EVALUATION_TIMEOUT_MS,
};
use crate::domain::knowledge::EvaluationRun;
use crate::errors::{AppError, AppResult};
use crate::repositories::evaluation::EvaluationRepository;
use crate::services::github::{
    GitHubMetadataClient, GitHubMetadataOutcome, GitHubRepositoryRef, GITHUB_INVALID_REPOSITORY,
    GITHUB_POLICY_DENIED, GITHUB_TIMEOUT,
};
use crate::services::github_evaluator::evaluate_github_repository;
use crate::services::prompt_evaluator::{evaluate_prompt, looks_like_prompt};
use crate::state::AppState;
use crate::storage::object_store::{sha256_hex, ObjectStore};
use crate::telemetry::{StructuredLogEvent, StructuredLogger};
use chrono::Utc;
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::time::{Duration, Instant};
use uuid::Uuid;

const PROMPT_EVALUATOR_TYPE: &str = "prompt_evaluator";
const GITHUB_REPO_EVALUATOR_TYPE: &str = "github_repo_evaluator";
const AUTO_EVALUATOR_TYPE: &str = "auto";
const EVALUATOR_VERSION: &str = "0.1.0";

#[derive(Clone)]
pub struct EvaluationService {
    repository: EvaluationRepository,
    object_store: ObjectStore,
    structured_logger: Option<StructuredLogger>,
    github_metadata_client: GitHubMetadataClient,
    execution_timeout_override: Option<Duration>,
    execution_delay: Duration,
}

trait EvaluatorPlugin: Send + Sync {
    fn evaluator_type(&self) -> &'static str;
    fn evaluator_version(&self) -> &'static str {
        EVALUATOR_VERSION
    }
    fn capability(&self) -> EvaluatorCapability;
    fn supports(&self, input: &EvaluationInput, requested_type: &str) -> bool;
    fn plan(&self, input: &EvaluationInput) -> EvaluationPlan;
    fn run(
        &self,
        input: &EvaluationInput,
        plan: &EvaluationPlan,
        github_metadata: Option<&GitHubMetadataOutcome>,
    ) -> EvaluationOutput;
}

struct PromptEvaluator;
struct GitHubRepoEvaluator;

impl EvaluationService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            repository: EvaluationRepository::new(state.database()?.pool().clone()),
            object_store: state.object_store()?.clone(),
            structured_logger: state.structured_logger().cloned(),
            github_metadata_client: GitHubMetadataClient::public(
                state.secrets().resolve("env:GITHUB_TOKEN")?,
            ),
            execution_timeout_override: None,
            execution_delay: Duration::ZERO,
        })
    }

    pub fn new(pool: SqlitePool, object_store: ObjectStore) -> Self {
        Self {
            repository: EvaluationRepository::new(pool),
            object_store,
            structured_logger: None,
            github_metadata_client: GitHubMetadataClient::public(None),
            execution_timeout_override: None,
            execution_delay: Duration::ZERO,
        }
    }

    pub fn list_evaluator_capabilities(&self) -> Vec<EvaluatorCapability> {
        evaluator_plugins()
            .into_iter()
            .map(|evaluator| evaluator.capability())
            .collect()
    }

    pub async fn trigger_evaluation(
        &self,
        object_id: &str,
        evaluator_type: &str,
        request_id: Option<&str>,
    ) -> AppResult<TriggerEvaluationResponse> {
        self.trigger_evaluation_with_retry(object_id, evaluator_type, request_id, None)
            .await
    }

    pub async fn retry_evaluation(
        &self,
        run_id: &str,
        request_id: Option<&str>,
    ) -> AppResult<TriggerEvaluationResponse> {
        let candidate = self.repository.get_retry_candidate(run_id).await?;
        if candidate.status != "failed" {
            return Err(AppError::PolicyDenied(
                "evaluation.retry_requires_failed_run".to_string(),
            ));
        }

        let retry_request_id = normalize_request_id(request_id)?;
        self.record_log(
            StructuredLogEvent::info(
                "evaluation",
                "evaluation.retry_requested",
                "Evaluation retry requested.",
            )
            .with_correlation_id(&retry_request_id)
            .with_object_id(&candidate.object_id)
            .with_job_id(&retry_request_id),
        )
        .await;
        self.trigger_evaluation_with_retry(
            &candidate.object_id,
            &candidate.requested_evaluator_type,
            Some(&retry_request_id),
            Some(&candidate.run_id),
        )
        .await
    }

    async fn trigger_evaluation_with_retry(
        &self,
        object_id: &str,
        evaluator_type: &str,
        request_id: Option<&str>,
        retry_of_run_id: Option<&str>,
    ) -> AppResult<TriggerEvaluationResponse> {
        let requested_type = normalize_evaluator_type(evaluator_type)?;
        let request_id = normalize_request_id(request_id)?;
        if let Some(operation) = self.repository.find_operation(&request_id).await? {
            let response = response_from_existing_operation(
                operation,
                object_id,
                &requested_type,
                retry_of_run_id,
            )?;
            self.record_log(
                StructuredLogEvent::info(
                    "evaluation",
                    "evaluation.reused",
                    "Evaluation request reused an existing operation.",
                )
                .with_correlation_id(&response.correlation_id)
                .with_object_id(object_id)
                .with_job_id(&response.job_id),
            )
            .await;
            return Ok(response);
        }

        let input = self.repository.get_evaluation_input(object_id).await?;
        let evaluator = select_evaluator(&input, &requested_type)?;
        let capability = evaluator.capability();
        let execution_timeout = self
            .execution_timeout_override
            .unwrap_or_else(|| evaluation_timeout_for(&capability.evaluator_type));
        let plan = evaluator.plan(&input);
        let input_snapshot = build_input_snapshot(&input, &requested_type);
        let run_id = Uuid::new_v4().to_string();
        let trace_id = Uuid::new_v4().to_string();
        let job_id = request_id.clone();
        let correlation_id = request_id.clone();
        let now = Utc::now().to_rfc3339();
        let reservation = EvaluationRunReservation {
            id: run_id.clone(),
            request_id: request_id.clone(),
            correlation_id: correlation_id.clone(),
            job_id: job_id.clone(),
            object_id: input.object_id.clone(),
            retry_of_run_id: retry_of_run_id.map(str::to_string),
            requested_evaluator_type: requested_type.clone(),
            evaluator_type: capability.evaluator_type.clone(),
            evaluator_version: capability.evaluator_version.clone(),
            plan_schema_version: capability.plan_schema_version,
            input_schema_version: capability.input_schema_version,
            output_schema_version: capability.output_schema_version,
            trace_id,
            execution_kind: capability.execution_kind.clone(),
            input_hash: input.content_hash.clone(),
            timeout_ms: execution_timeout.as_millis().min(i64::MAX as u128) as i64,
            plan_json: serialize_json(&plan)?,
            input_json: serialize_json(&input_snapshot)?,
            created_at: now.clone(),
        };

        match self
            .repository
            .reserve_evaluation(&input.user_id, &reservation)
            .await
        {
            Ok(()) => {}
            Err(AppError::DbConstraint) => {
                let operation = self
                    .repository
                    .find_operation(&request_id)
                    .await?
                    .ok_or_else(|| {
                        AppError::Database(
                            "evaluation idempotency conflict could not be resolved".to_string(),
                        )
                    })?;
                return response_from_existing_operation(
                    operation,
                    object_id,
                    &requested_type,
                    retry_of_run_id,
                );
            }
            Err(error) => return Err(error),
        }

        self.record_log(
            StructuredLogEvent::info(
                "evaluation",
                "evaluation.submitted",
                "Evaluation submitted.",
            )
            .with_correlation_id(&correlation_id)
            .with_object_id(&input.object_id)
            .with_job_id(&job_id),
        )
        .await;

        let execution_started = Instant::now();
        if let Err(error) = self
            .repository
            .mark_evaluation_running(&run_id, &job_id, &correlation_id, &now)
            .await
        {
            let error_code = evaluation_error_code(&error);
            let completed_at = Utc::now().to_rfc3339();
            let _ = self
                .repository
                .fail_evaluation(&EvaluationFailureSubmission {
                    user_id: input.user_id.clone(),
                    run_id: run_id.clone(),
                    job_id: job_id.clone(),
                    correlation_id: correlation_id.clone(),
                    object_id: input.object_id.clone(),
                    evaluator_type: capability.evaluator_type.clone(),
                    error_code: error_code.to_string(),
                    latency_ms: elapsed_ms(execution_started),
                    completed_at,
                })
                .await;
            self.record_log(
                StructuredLogEvent::error("evaluation", "evaluation.failed", "Evaluation failed.")
                    .with_correlation_id(&correlation_id)
                    .with_object_id(&input.object_id)
                    .with_job_id(&job_id)
                    .with_error_code(error_code),
            )
            .await;
            return Err(stable_evaluation_error(error_code));
        }

        self.record_log(
            StructuredLogEvent::info("evaluation", "evaluation.started", "Evaluation started.")
                .with_correlation_id(&correlation_id)
                .with_object_id(&input.object_id)
                .with_job_id(&job_id),
        )
        .await;

        let execution: AppResult<(
            EvaluationRunSubmission,
            EvaluationArtifactSubmission,
            EvaluationTraceCompletion,
        )> = async {
            let github_metadata = self
                .collect_github_metadata(
                    &input,
                    &capability.evaluator_type,
                    execution_timeout,
                    &correlation_id,
                    &job_id,
                )
                .await;
            let output = run_evaluator_with_timeout(
                capability.evaluator_type.clone(),
                input.clone(),
                plan.clone(),
                github_metadata,
                remaining_timeout(execution_started, execution_timeout),
                self.execution_delay,
            )
            .await?;
            validate_evaluation_output(&output)?;
            let output_json = serialize_json(&output)?;
            let output_hash = sha256_hex(output_json.as_bytes());
            let artifact_id = Uuid::new_v4().to_string();
            let completed_at = Utc::now().to_rfc3339();
            let report_bytes = serde_json::to_vec_pretty(&json!({
                "schemaVersion": EVALUATION_OUTPUT_SCHEMA_VERSION,
                "runId": run_id,
                "requestId": request_id,
                "correlationId": correlation_id,
                "objectId": input.object_id,
                "retryOfRunId": retry_of_run_id,
                "capability": capability,
                "plan": plan,
                "input": input_snapshot,
                "output": output,
            }))
            .map_err(|_| {
                AppError::ModelOutputSchema("evaluation report serialization failed".to_string())
            })?;
            let stored_report = self
                .object_store
                .write_evaluation_artifact(
                    &input.object_id,
                    &run_id,
                    &artifact_id,
                    "json",
                    report_bytes,
                )
                .await?;

            let run = EvaluationRunSubmission {
                id: run_id.clone(),
                request_id: request_id.clone(),
                correlation_id: correlation_id.clone(),
                object_id: input.object_id.clone(),
                evaluator_type: capability.evaluator_type.clone(),
                evaluator_version: capability.evaluator_version.clone(),
                plan_schema_version: capability.plan_schema_version,
                input_schema_version: capability.input_schema_version,
                output_schema_version: capability.output_schema_version,
                status: "passed".to_string(),
                plan_json: serialize_json(&plan)?,
                input_json: serialize_json(&input_snapshot)?,
                output_json,
                dimensions_json: serialize_json(&output.dimensions)?,
                evidence_json: serialize_json(&output.evidence)?,
                limitations_json: serialize_json(&output.limitations)?,
                next_actions_json: serialize_json(&output.next_actions)?,
                score: Some(output.score),
                verdict: output.verdict,
                failure_reason: None,
                created_at: now,
                completed_at: Some(completed_at.clone()),
            };
            let artifact = EvaluationArtifactSubmission {
                id: artifact_id,
                evaluation_run_id: run_id.clone(),
                artifact_type: "report".to_string(),
                storage_uri: stored_report.storage_uri,
                content_hash: Some(stored_report.content_hash),
                metadata_json: Some(
                    json!({
                        "schemaVersion": 1,
                        "evaluatorType": run.evaluator_type,
                        "evaluatorVersion": run.evaluator_version,
                        "contentHash": input.content_hash,
                        "inputSchemaVersion": run.input_schema_version,
                        "outputSchemaVersion": run.output_schema_version,
                    })
                    .to_string(),
                ),
                created_at: completed_at.clone(),
            };
            let trace = EvaluationTraceCompletion {
                output_hash,
                latency_ms: elapsed_ms(execution_started),
                completed_at,
            };
            Ok((run, artifact, trace))
        }
        .await;

        let (run, artifact, trace) = match execution {
            Ok(result) => result,
            Err(error) => {
                let error_code = evaluation_error_code(&error);
                let completed_at = Utc::now().to_rfc3339();
                let _ = self
                    .object_store
                    .remove_evaluation_run_artifacts(&input.object_id, &run_id)
                    .await;
                let _ = self
                    .repository
                    .fail_evaluation(&EvaluationFailureSubmission {
                        user_id: input.user_id.clone(),
                        run_id: run_id.clone(),
                        job_id: job_id.clone(),
                        correlation_id: correlation_id.clone(),
                        object_id: input.object_id.clone(),
                        evaluator_type: capability.evaluator_type.clone(),
                        error_code: error_code.to_string(),
                        latency_ms: elapsed_ms(execution_started),
                        completed_at,
                    })
                    .await;
                self.record_log(
                    StructuredLogEvent::error(
                        "evaluation",
                        "evaluation.failed",
                        "Evaluation failed.",
                    )
                    .with_correlation_id(&correlation_id)
                    .with_object_id(&input.object_id)
                    .with_job_id(&job_id)
                    .with_error_code(error_code),
                )
                .await;
                return Err(stable_evaluation_error(error_code));
            }
        };

        if let Err(error) = self
            .repository
            .complete_evaluation(&input.user_id, &job_id, &run, &[artifact], &trace)
            .await
        {
            let error_code = evaluation_error_code(&error);
            let completed_at = Utc::now().to_rfc3339();
            let _ = self
                .object_store
                .remove_evaluation_run_artifacts(&input.object_id, &run_id)
                .await;
            let _ = self
                .repository
                .fail_evaluation(&EvaluationFailureSubmission {
                    user_id: input.user_id.clone(),
                    run_id: run_id.clone(),
                    job_id: job_id.clone(),
                    correlation_id: correlation_id.clone(),
                    object_id: input.object_id.clone(),
                    evaluator_type: capability.evaluator_type.clone(),
                    error_code: error_code.to_string(),
                    latency_ms: elapsed_ms(execution_started),
                    completed_at,
                })
                .await;
            self.record_log(
                StructuredLogEvent::error("evaluation", "evaluation.failed", "Evaluation failed.")
                    .with_correlation_id(&correlation_id)
                    .with_object_id(&input.object_id)
                    .with_job_id(&job_id)
                    .with_error_code(error_code),
            )
            .await;
            return Err(stable_evaluation_error(error_code));
        }

        self.record_log(
            StructuredLogEvent::info(
                "evaluation",
                "evaluation.succeeded",
                "Evaluation succeeded.",
            )
            .with_correlation_id(&correlation_id)
            .with_object_id(&input.object_id)
            .with_job_id(&job_id),
        )
        .await;

        Ok(TriggerEvaluationResponse {
            run_id,
            job_id,
            request_id,
            correlation_id,
            retry_of_run_id: retry_of_run_id.map(str::to_string),
            status: "passed".to_string(),
            reused: false,
        })
    }
    async fn collect_github_metadata(
        &self,
        input: &EvaluationInput,
        evaluator_type: &str,
        execution_timeout: Duration,
        correlation_id: &str,
        job_id: &str,
    ) -> Option<GitHubMetadataOutcome> {
        if evaluator_type != GITHUB_REPO_EVALUATOR_TYPE {
            return None;
        }
        let outcome = if input.privacy_level == "secret" {
            GitHubMetadataOutcome::Unavailable {
                code: GITHUB_POLICY_DENIED.to_string(),
            }
        } else if let Some(reference) = input
            .canonical_url
            .as_deref()
            .and_then(GitHubRepositoryRef::from_github_url)
        {
            let budget = github_metadata_budget(execution_timeout);
            match tokio::time::timeout(
                budget,
                self.github_metadata_client
                    .fetch_public_repository(&reference),
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(_) => GitHubMetadataOutcome::Unavailable {
                    code: GITHUB_TIMEOUT.to_string(),
                },
            }
        } else {
            GitHubMetadataOutcome::Unavailable {
                code: GITHUB_INVALID_REPOSITORY.to_string(),
            }
        };

        let (event, error_code) = match &outcome {
            GitHubMetadataOutcome::Available(metadata) if metadata.limitations.is_empty() => {
                ("evaluation.github_metadata.succeeded", None)
            }
            GitHubMetadataOutcome::Available(metadata) => (
                "evaluation.github_metadata.degraded",
                metadata.limitations.first().map(String::as_str),
            ),
            GitHubMetadataOutcome::Unavailable { code } => {
                ("evaluation.github_metadata.degraded", Some(code.as_str()))
            }
        };
        let log_event =
            StructuredLogEvent::info("evaluation", event, "GitHub metadata collection completed.")
                .with_correlation_id(correlation_id)
                .with_object_id(&input.object_id)
                .with_job_id(job_id);
        let log_event = if let Some(error_code) = error_code {
            log_event.with_error_code(error_code)
        } else {
            log_event
        };
        self.record_log(log_event).await;

        Some(outcome)
    }

    pub async fn get_evaluation_run(&self, run_id: &str) -> AppResult<EvaluationRun> {
        self.repository.get_evaluation_run(run_id).await
    }

    async fn record_log(&self, event: StructuredLogEvent) {
        if let Some(logger) = &self.structured_logger {
            let _ = logger.record(event).await;
        }
    }

    #[cfg(test)]
    fn with_execution_timeout(mut self, execution_timeout: Duration) -> Self {
        self.execution_timeout_override = Some(execution_timeout);
        self
    }

    #[cfg(test)]
    fn with_github_metadata_client(mut self, client: GitHubMetadataClient) -> Self {
        self.github_metadata_client = client;
        self
    }

    #[cfg(test)]
    fn with_execution_delay(mut self, execution_delay: Duration) -> Self {
        self.execution_delay = execution_delay;
        self
    }

    #[cfg(test)]
    fn with_structured_logger(mut self, structured_logger: StructuredLogger) -> Self {
        self.structured_logger = Some(structured_logger);
        self
    }
}

impl EvaluatorPlugin for PromptEvaluator {
    fn evaluator_type(&self) -> &'static str {
        PROMPT_EVALUATOR_TYPE
    }

    fn capability(&self) -> EvaluatorCapability {
        EvaluatorCapability {
            schema_version: EVALUATION_CAPABILITY_SCHEMA_VERSION,
            evaluator_type: self.evaluator_type().to_string(),
            evaluator_version: self.evaluator_version().to_string(),
            display_name: "Prompt quality evaluator".to_string(),
            supported_object_types: vec!["prompt".to_string()],
            execution_kind: "local_deterministic".to_string(),
            requires_network: false,
            requires_model: false,
            requires_sandbox: false,
            plan_schema_version: EVALUATION_PLAN_SCHEMA_VERSION,
            input_schema_version: EVALUATION_INPUT_SCHEMA_VERSION,
            output_schema_version: EVALUATION_OUTPUT_SCHEMA_VERSION,
        }
    }

    fn supports(&self, input: &EvaluationInput, requested_type: &str) -> bool {
        (requested_type == PROMPT_EVALUATOR_TYPE || requested_type == AUTO_EVALUATOR_TYPE)
            && (input.object_type == "prompt" || looks_like_prompt(&input.text_content))
    }

    fn plan(&self, input: &EvaluationInput) -> EvaluationPlan {
        EvaluationPlan {
            schema_version: EVALUATION_PLAN_SCHEMA_VERSION,
            evaluator_type: self.evaluator_type().to_string(),
            evaluator_version: self.evaluator_version().to_string(),
            steps: vec![
                "Inspect the prompt goal and instruction structure.".to_string(),
                "Check whether inputs, constraints and output format are explicit.".to_string(),
                "Generate lightweight test cases and improvement actions.".to_string(),
            ],
            checks: vec![
                format!("content_hash: {}", input.content_hash),
                format!("word_count: {}", input.word_count.unwrap_or_default()),
                "no external model or sandbox execution in MVP evaluator".to_string(),
            ],
        }
    }

    fn run(
        &self,
        input: &EvaluationInput,
        plan: &EvaluationPlan,
        _github_metadata: Option<&GitHubMetadataOutcome>,
    ) -> EvaluationOutput {
        evaluate_prompt(input, plan)
    }
}
impl EvaluatorPlugin for GitHubRepoEvaluator {
    fn evaluator_type(&self) -> &'static str {
        GITHUB_REPO_EVALUATOR_TYPE
    }

    fn capability(&self) -> EvaluatorCapability {
        EvaluatorCapability {
            schema_version: EVALUATION_CAPABILITY_SCHEMA_VERSION,
            evaluator_type: self.evaluator_type().to_string(),
            evaluator_version: self.evaluator_version().to_string(),
            display_name: "GitHub repository evaluator".to_string(),
            supported_object_types: vec!["github_repo".to_string()],
            execution_kind: "network_optional".to_string(),
            requires_network: false,
            requires_model: false,
            requires_sandbox: false,
            plan_schema_version: EVALUATION_PLAN_SCHEMA_VERSION,
            input_schema_version: EVALUATION_INPUT_SCHEMA_VERSION,
            output_schema_version: EVALUATION_OUTPUT_SCHEMA_VERSION,
        }
    }

    fn supports(&self, input: &EvaluationInput, requested_type: &str) -> bool {
        (requested_type == GITHUB_REPO_EVALUATOR_TYPE || requested_type == AUTO_EVALUATOR_TYPE)
            && is_github_repo(input)
    }

    fn plan(&self, input: &EvaluationInput) -> EvaluationPlan {
        EvaluationPlan {
            schema_version: EVALUATION_PLAN_SCHEMA_VERSION,
            evaluator_type: self.evaluator_type().to_string(),
            evaluator_version: self.evaluator_version().to_string(),
            steps: vec![
                "Inspect saved repository content and bounded public GitHub metadata.".to_string(),
                "Check README, license, recent push, release, maintenance and adoption signals."
                    .to_string(),
                "Score usefulness without treating stars as a standalone value conclusion."
                    .to_string(),
                "Record stable limitations for policy, rate limit, private or missing metadata."
                    .to_string(),
            ],
            checks: vec![
                format!("content_hash: {}", input.content_hash),
                "GitHub REST metadata is optional; saved content remains the fallback.".to_string(),
                "No repository code is cloned or executed by the Week 7 evaluator.".to_string(),
            ],
        }
    }

    fn run(
        &self,
        input: &EvaluationInput,
        plan: &EvaluationPlan,
        github_metadata: Option<&GitHubMetadataOutcome>,
    ) -> EvaluationOutput {
        evaluate_github_repository(input, plan, github_metadata)
    }
}

async fn run_evaluator_with_timeout(
    evaluator_type: String,
    input: EvaluationInput,
    plan: EvaluationPlan,
    github_metadata: Option<GitHubMetadataOutcome>,
    execution_timeout: Duration,
    execution_delay: Duration,
) -> AppResult<EvaluationOutput> {
    let task = tokio::task::spawn_blocking(move || {
        if !execution_delay.is_zero() {
            std::thread::sleep(execution_delay);
        }
        let evaluator = evaluator_plugins()
            .into_iter()
            .find(|candidate| candidate.evaluator_type() == evaluator_type)
            .ok_or_else(|| {
                AppError::PolicyDenied("evaluation.unsupported_evaluator_for_object".to_string())
            })?;
        Ok::<_, AppError>(evaluator.run(&input, &plan, github_metadata.as_ref()))
    });

    match tokio::time::timeout(execution_timeout, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(AppError::Unknown("evaluation.runner_stopped".to_string())),
        Err(_) => Err(AppError::NetworkTimeout),
    }
}

fn evaluation_timeout_for(evaluator_type: &str) -> Duration {
    let timeout_ms = if evaluator_type == GITHUB_REPO_EVALUATOR_TYPE {
        GITHUB_EVALUATION_TIMEOUT_MS
    } else {
        DEFAULT_EVALUATION_TIMEOUT_MS
    };
    Duration::from_millis(timeout_ms as u64)
}

fn github_metadata_budget(execution_timeout: Duration) -> Duration {
    let budget = execution_timeout.mul_f64(0.8);
    budget
        .min(Duration::from_secs(12))
        .max(Duration::from_millis(1))
}

fn remaining_timeout(started_at: Instant, execution_timeout: Duration) -> Duration {
    execution_timeout
        .saturating_sub(started_at.elapsed())
        .max(Duration::from_millis(1))
}

fn elapsed_ms(started_at: Instant) -> i64 {
    started_at.elapsed().as_millis().min(i64::MAX as u128) as i64
}

fn evaluator_plugins() -> Vec<Box<dyn EvaluatorPlugin + Send + Sync>> {
    vec![Box::new(GitHubRepoEvaluator), Box::new(PromptEvaluator)]
}

fn select_evaluator(
    input: &EvaluationInput,
    requested_type: &str,
) -> AppResult<Box<dyn EvaluatorPlugin + Send + Sync>> {
    evaluator_plugins()
        .into_iter()
        .find(|evaluator| evaluator.supports(input, requested_type))
        .ok_or_else(|| {
            AppError::PolicyDenied("evaluation.unsupported_evaluator_for_object".to_string())
        })
}

fn normalize_evaluator_type(evaluator_type: &str) -> AppResult<String> {
    let normalized = if evaluator_type.trim().is_empty() {
        AUTO_EVALUATOR_TYPE.to_string()
    } else {
        evaluator_type.trim().to_ascii_lowercase()
    };
    if normalized.len() > 64
        || !normalized
            .chars()
            .all(|character| character.is_ascii_lowercase() || matches!(character, '_' | '-'))
    {
        return Err(AppError::PolicyDenied(
            "evaluation.invalid_evaluator_type".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_request_id(request_id: Option<&str>) -> AppResult<String> {
    match request_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => Uuid::parse_str(value)
            .map(|uuid| uuid.to_string())
            .map_err(|_| AppError::PolicyDenied("evaluation.invalid_request_id".to_string())),
        None => Ok(Uuid::new_v4().to_string()),
    }
}

fn response_from_existing_operation(
    operation: EvaluationOperation,
    object_id: &str,
    requested_evaluator_type: &str,
    retry_of_run_id: Option<&str>,
) -> AppResult<TriggerEvaluationResponse> {
    if operation.object_id != object_id
        || operation.requested_evaluator_type != requested_evaluator_type
        || operation.retry_of_run_id.as_deref() != retry_of_run_id
        || operation.request_id != operation.job_id
        || operation.correlation_id != operation.request_id
    {
        return Err(AppError::PolicyDenied(
            "evaluation.request_id_conflict".to_string(),
        ));
    }
    Ok(TriggerEvaluationResponse {
        run_id: operation.run_id,
        job_id: operation.job_id,
        request_id: operation.request_id,
        correlation_id: operation.correlation_id,
        retry_of_run_id: operation.retry_of_run_id,
        status: operation.status,
        reused: true,
    })
}

fn build_input_snapshot(input: &EvaluationInput, requested_type: &str) -> Value {
    json!({
        "schemaVersion": EVALUATION_INPUT_SCHEMA_VERSION,
        "objectId": input.object_id,
        "objectType": input.object_type,
        "title": input.title,
        "canonicalUrl": input.canonical_url,
        "privacyLevel": input.privacy_level,
        "parsedDocumentId": input.parsed_document_id,
        "contentHash": input.content_hash,
        "wordCount": input.word_count,
        "latestAISummaryPresent": input.latest_ai_summary.is_some(),
        "requestedEvaluatorType": requested_type,
    })
}

fn validate_evaluation_output(output: &EvaluationOutput) -> AppResult<()> {
    if output.schema_version != EVALUATION_OUTPUT_SCHEMA_VERSION
        || !output.score.is_finite()
        || !(0.0..=1.0).contains(&output.score)
        || !matches!(
            output.verdict.as_str(),
            "high_value" | "useful" | "situational" | "low_value" | "unsafe" | "unknown"
        )
        || (output.evidence.is_empty()
            && (output.verdict != "unknown" || output.limitations.is_empty()))
        || output.evidence.iter().any(|evidence| {
            evidence.text.trim().is_empty()
                || evidence.text.chars().count() > 512
                || !matches!(
                    evidence.source.as_str(),
                    "original_content"
                        | "internal_library"
                        | "external_check"
                        | "sandbox_run"
                        | "user_feedback"
                )
        })
    {
        return Err(AppError::ModelOutputSchema(
            "evaluation output contract validation failed".to_string(),
        ));
    }
    Ok(())
}

fn evaluation_error_code(error: &AppError) -> &'static str {
    match error {
        AppError::Filesystem(_) => "evaluation.artifact_write_failed",
        AppError::NetworkTimeout => "evaluation.timeout",
        AppError::Unknown(message) if message == "evaluation.runner_stopped" => {
            "evaluation.runner_stopped"
        }
        AppError::ModelOutputSchema(_) => "evaluation.output_invalid",
        AppError::PolicyDenied(_) => "evaluation.policy_denied",
        AppError::Database(_) | AppError::DbConstraint => "evaluation.persistence_failed",
        _ => "evaluation.failed",
    }
}

fn stable_evaluation_error(error_code: &str) -> AppError {
    AppError::Unknown(error_code.to_string())
}
fn is_github_repo(input: &EvaluationInput) -> bool {
    input.object_type == "github_repo"
        || parse_github_repo(input.canonical_url.as_deref()).is_some()
}

fn parse_github_repo(raw_url: Option<&str>) -> Option<Value> {
    let url = Url::parse(raw_url?).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();

    if host != "github.com" && host != "www.github.com" {
        return None;
    }

    let mut parts = url.path_segments()?.filter(|segment| !segment.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?;

    Some(json!({
        "owner": owner,
        "repo": repo.trim_end_matches(".git"),
        "url": url.as_str(),
    }))
}

fn serialize_json<T>(value: &T) -> AppResult<String>
where
    T: serde::Serialize,
{
    serde_json::to_string(value).map_err(|error| AppError::ModelOutputSchema(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{validate_evaluation_output, EvaluationService};
    use crate::domain::evaluation::{EvaluationOutput, EVALUATION_OUTPUT_SCHEMA_VERSION};
    use crate::errors::AppError;
    use crate::services::github::GitHubMetadataClient;
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use crate::telemetry::StructuredLogger;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn evidence_less_conclusion_requires_unknown_verdict_and_limitation() {
        let invalid = EvaluationOutput {
            schema_version: EVALUATION_OUTPUT_SCHEMA_VERSION,
            score: 0.8,
            verdict: "useful".to_string(),
            dimensions: serde_json::json!({"utility": 0.8}),
            evidence: Vec::new(),
            limitations: Vec::new(),
            next_actions: Vec::new(),
            report: serde_json::json!({}),
        };
        assert!(validate_evaluation_output(&invalid).is_err());

        let valid = EvaluationOutput {
            verdict: "unknown".to_string(),
            limitations: vec!["No source-linked evidence was available.".to_string()],
            ..invalid
        };
        validate_evaluation_output(&valid)
            .expect("unknown verdict with an explicit limitation should be valid");
    }
    #[tokio::test]
    async fn prompt_evaluator_writes_run_artifact_job_event_and_lifecycle() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        seed_parsed_object(
            database.pool(),
            "obj-prompt",
            "prompt",
            Some("Reusable Prompt"),
            Some("You are a precise assistant. Task: summarize {{topic}}. Output format: JSON. Example: input and output."),
        )
        .await;
        let service = EvaluationService::new(database.pool().clone(), object_store);

        let response = service
            .trigger_evaluation("obj-prompt", "prompt_evaluator", None)
            .await
            .expect("evaluation should run");
        let run = service
            .get_evaluation_run(&response.run_id)
            .await
            .expect("run should be readable");

        assert!(!response.reused);
        assert_eq!(response.status, "passed");
        assert_eq!(response.job_id, response.request_id);
        assert_eq!(response.correlation_id, response.request_id);
        uuid::Uuid::parse_str(&response.request_id).expect("request id should be a UUID");
        assert_eq!(run.status, "passed");
        assert_eq!(
            run.request_id.as_deref(),
            Some(response.request_id.as_str())
        );
        assert_eq!(
            run.correlation_id.as_deref(),
            Some(response.correlation_id.as_str())
        );
        assert_eq!(run.evaluator_type, "prompt_evaluator");
        assert_eq!(run.plan_schema_version, 1);
        assert_eq!(run.input_schema_version, 1);
        assert_eq!(run.output_schema_version, 1);
        assert!(run.score.unwrap_or_default() > 0.5);
        assert_eq!(run.artifacts.len(), 1);
        let trace = run
            .trace
            .as_ref()
            .expect("successful run must have a trace");
        assert_eq!(trace.status, "passed");
        assert_eq!(trace.correlation_id, response.correlation_id);
        assert_eq!(trace.execution_kind, "local_deterministic");
        assert_eq!(trace.input_hash, "obj-prompt-hash");
        assert!(trace.output_hash.is_some());
        assert!(trace.latency_ms.is_some());
        assert_eq!(trace.error_code, None);

        let lifecycle_status: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind("obj-prompt")
                .fetch_one(database.pool())
                .await
                .expect("status should be readable");
        let job: (String, String, String, Option<String>) = sqlx::query_as(
            "SELECT job_type, status, payload_json, last_error FROM background_jobs WHERE id = ?1",
        )
        .bind(&response.job_id)
        .fetch_one(database.pool())
        .await
        .expect("job should be readable");
        let events: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT event_type, causation_id, correlation_id FROM domain_events WHERE object_id = ?1 AND event_type LIKE 'evaluation.%' ORDER BY rowid",
        )
        .bind("obj-prompt")
        .fetch_all(database.pool())
        .await
        .expect("events should be readable");
        let versions: (i64, i64, i64, String, String, String) = sqlx::query_as(
            "SELECT plan_schema_version, input_schema_version, output_schema_version, plan_json, input_json, output_json FROM evaluation_runs WHERE id = ?1",
        )
        .bind(&response.run_id)
        .fetch_one(database.pool())
        .await
        .expect("versioned run payload should be readable");

        assert_eq!(lifecycle_status, "evaluated");
        assert_eq!(job.0, "evaluation.run");
        assert_eq!(job.1, "succeeded");
        assert_eq!(job.3, None);
        let job_payload: serde_json::Value =
            serde_json::from_str(&job.2).expect("job payload should be JSON");
        assert_eq!(job_payload["requestId"], response.request_id);
        assert_eq!(job_payload["correlationId"], response.correlation_id);
        assert_eq!(job_payload["runId"], response.run_id);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "evaluation.planned");
        assert_eq!(events[1].0, "evaluation.completed");
        assert!(events.iter().all(|event| {
            event.1.as_deref() == Some(response.job_id.as_str())
                && event.2.as_deref() == Some(response.correlation_id.as_str())
        }));
        assert_eq!((versions.0, versions.1, versions.2), (1, 1, 1));
        for raw in [&versions.3, &versions.4, &versions.5] {
            let value: serde_json::Value =
                serde_json::from_str(raw).expect("versioned payload should be JSON");
            assert_eq!(value["schemaVersion"], 1);
        }
        let output: serde_json::Value =
            serde_json::from_str(&versions.5).expect("prompt output should be JSON");
        assert_eq!(
            output["report"]["executionBoundary"]["inputTreatment"],
            "untrusted_data"
        );
        assert_eq!(
            output["report"]["executionBoundary"]["externalActions"],
            false
        );
        assert_eq!(output["report"]["rubric"]["version"], 1);
        assert_eq!(
            output["report"]["originalPrompt"],
            "You are a precise assistant. Task: summarize {{topic}}. Output format: JSON. Example: input and output."
        );
        assert!(output["report"]["improvementDiff"]
            .as_array()
            .is_some_and(|changes| !changes.is_empty()));
        let capabilities = service.list_evaluator_capabilities();
        assert_eq!(capabilities.len(), 2);
        assert!(capabilities.iter().all(|capability| {
            capability.schema_version == 1
                && capability.plan_schema_version == 1
                && capability.input_schema_version == 1
                && capability.output_schema_version == 1
                && !capability.requires_network
                && !capability.requires_model
                && !capability.requires_sandbox
        }));
        assert_eq!(
            capabilities
                .iter()
                .find(|capability| capability.evaluator_type == "prompt_evaluator")
                .map(|capability| capability.execution_kind.as_str()),
            Some("local_deterministic")
        );
        assert_eq!(
            capabilities
                .iter()
                .find(|capability| capability.evaluator_type == "github_repo_evaluator")
                .map(|capability| capability.execution_kind.as_str()),
            Some("network_optional")
        );
    }

    #[tokio::test]
    async fn repeated_request_id_reuses_one_run_job_and_artifact() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        seed_parsed_object(
            database.pool(),
            "obj-idempotent",
            "prompt",
            Some("Idempotent Prompt"),
            Some("You are a reviewer. Task: assess {{input}}. Output format: JSON."),
        )
        .await;
        let service = EvaluationService::new(database.pool().clone(), object_store);
        let request_id = uuid::Uuid::new_v4().to_string();

        let first = service
            .trigger_evaluation("obj-idempotent", "prompt_evaluator", Some(&request_id))
            .await
            .expect("first evaluation should run");
        let repeated = service
            .trigger_evaluation("obj-idempotent", "prompt_evaluator", Some(&request_id))
            .await
            .expect("repeated evaluation should resolve idempotently");

        assert!(!first.reused);
        assert!(repeated.reused);
        assert_eq!(repeated.run_id, first.run_id);
        assert_eq!(repeated.job_id, first.job_id);
        assert_eq!(repeated.correlation_id, first.correlation_id);
        assert_eq!(repeated.status, "passed");
        let counts: (i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM evaluation_runs), (SELECT COUNT(*) FROM evaluation_traces), (SELECT COUNT(*) FROM background_jobs WHERE job_type = 'evaluation.run'), (SELECT COUNT(*) FROM evaluation_artifacts), (SELECT COUNT(*) FROM domain_events WHERE event_type LIKE 'evaluation.%')",
        )
        .fetch_one(database.pool())
        .await
        .expect("evaluation counts should query");
        assert_eq!(counts, (1, 1, 1, 1, 2));

        seed_parsed_object(
            database.pool(),
            "obj-conflict",
            "prompt",
            Some("Other Prompt"),
            Some("You are a reviewer. Task: assess another input."),
        )
        .await;
        let conflict = service
            .trigger_evaluation("obj-conflict", "prompt_evaluator", Some(&request_id))
            .await
            .expect_err("request id reuse across objects must be rejected");
        assert!(matches!(conflict, AppError::PolicyDenied(_)));
        let run_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evaluation_runs")
            .fetch_one(database.pool())
            .await
            .expect("run count should query");
        assert_eq!(run_count, 1);
    }

    #[tokio::test]
    async fn artifact_failure_converges_reserved_run_and_job_to_failed() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let object_root = object_store.root().to_path_buf();
        std::fs::remove_dir_all(&object_root).expect("object root should remove");
        std::fs::write(&object_root, b"block artifact directory")
            .expect("blocking file should write");
        seed_parsed_object(
            database.pool(),
            "obj-failure",
            "prompt",
            Some("Failure Prompt"),
            Some("You are a reviewer. Task: assess {{input}}. Output format: JSON."),
        )
        .await;
        let service = EvaluationService::new(database.pool().clone(), object_store);
        let request_id = uuid::Uuid::new_v4().to_string();

        let error = service
            .trigger_evaluation("obj-failure", "prompt_evaluator", Some(&request_id))
            .await
            .expect_err("artifact write should fail");
        assert_eq!(
            error.to_string(),
            "unknown error: evaluation.artifact_write_failed"
        );

        let run: (String, String, Option<String>) = sqlx::query_as(
            "SELECT status, verdict, failure_reason FROM evaluation_runs WHERE request_id = ?1",
        )
        .bind(&request_id)
        .fetch_one(database.pool())
        .await
        .expect("failed run should persist");
        let job: (String, Option<String>) =
            sqlx::query_as("SELECT status, last_error FROM background_jobs WHERE id = ?1")
                .bind(&request_id)
                .fetch_one(database.pool())
                .await
                .expect("failed job should persist");
        let events: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT event_type, correlation_id FROM domain_events WHERE object_id = 'obj-failure' ORDER BY rowid",
        )
        .fetch_all(database.pool())
        .await
        .expect("failure events should query");
        assert_eq!(run.0, "failed");
        assert_eq!(run.1, "unknown");
        assert_eq!(run.2.as_deref(), Some("evaluation.artifact_write_failed"));
        assert_eq!(job.0, "failed");
        assert_eq!(job.1.as_deref(), Some("evaluation.artifact_write_failed"));
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, "evaluation.planned");
        assert_eq!(events[1].0, "evaluation.failed");
        assert!(events
            .iter()
            .all(|event| event.1.as_deref() == Some(request_id.as_str())));
        let _ = std::fs::remove_file(object_root);
    }
    #[tokio::test]
    async fn evaluator_timeout_converges_run_job_trace_and_redacted_logs() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        let telemetry_dir = std::env::temp_dir().join(format!(
            "link-world-evaluation-timeout-{}",
            uuid::Uuid::new_v4()
        ));
        seed_parsed_object(
            database.pool(),
            "obj-timeout",
            "prompt",
            Some("Timeout Prompt"),
            Some("You are a reviewer. Task: assess {{input}}. Output format: JSON."),
        )
        .await;
        let service = EvaluationService::new(database.pool().clone(), object_store)
            .with_execution_timeout(Duration::from_millis(5))
            .with_execution_delay(Duration::from_millis(50))
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));
        let request_id = uuid::Uuid::new_v4().to_string();

        let error = service
            .trigger_evaluation("obj-timeout", "prompt_evaluator", Some(&request_id))
            .await
            .expect_err("delayed evaluator should time out");
        assert_eq!(error.to_string(), "unknown error: evaluation.timeout");

        let state: (
            String,
            Option<String>,
            String,
            Option<String>,
            String,
            Option<String>,
            i64,
        ) = sqlx::query_as(
            r#"
                SELECT
                    runs.status, runs.failure_reason, jobs.status, jobs.last_error,
                    traces.status, traces.error_code, traces.timeout_ms
                FROM evaluation_runs AS runs
                INNER JOIN background_jobs AS jobs ON jobs.id = runs.request_id
                INNER JOIN evaluation_traces AS traces ON traces.evaluation_run_id = runs.id
                WHERE runs.request_id = ?1
                "#,
        )
        .bind(&request_id)
        .fetch_one(database.pool())
        .await
        .expect("failed evaluation state should query");
        assert_eq!(state.0, "failed");
        assert_eq!(state.1.as_deref(), Some("evaluation.timeout"));
        assert_eq!(state.2, "failed");
        assert_eq!(state.3.as_deref(), Some("evaluation.timeout"));
        assert_eq!(state.4, "failed");
        assert_eq!(state.5.as_deref(), Some("evaluation.timeout"));
        assert_eq!(state.6, 5);
        let artifact_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evaluation_artifacts")
            .fetch_one(database.pool())
            .await
            .expect("artifact count should query");
        assert_eq!(artifact_count, 0);

        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(10))
            .await
            .expect("evaluation logs should read");
        assert_eq!(
            logs.iter()
                .map(|entry| entry.event.as_str())
                .collect::<Vec<_>>(),
            vec![
                "evaluation.submitted",
                "evaluation.started",
                "evaluation.failed",
            ]
        );
        assert!(logs.iter().all(|entry| {
            entry.correlation_id.as_deref() == Some(request_id.as_str())
                && entry.object_id.as_deref() == Some("obj-timeout")
                && entry.job_id.as_deref() == Some(request_id.as_str())
        }));
        let serialized =
            std::fs::read_to_string(telemetry_dir.join("logs").join("link-world.jsonl"))
                .expect("evaluation log should be readable");
        assert!(!serialized.contains("assess {{input}}"));
        assert!(!serialized.contains("example.com"));
        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn retry_preserves_failed_history_and_is_idempotent_per_new_request() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        seed_parsed_object(
            database.pool(),
            "obj-retry-evaluation",
            "prompt",
            Some("Retry Prompt"),
            Some("You are a reviewer. Task: assess {{input}}. Output format: JSON."),
        )
        .await;
        let failed_service = EvaluationService::new(database.pool().clone(), object_store.clone())
            .with_execution_timeout(Duration::from_millis(5))
            .with_execution_delay(Duration::from_millis(50));
        let original_request_id = uuid::Uuid::new_v4().to_string();
        failed_service
            .trigger_evaluation(
                "obj-retry-evaluation",
                "prompt_evaluator",
                Some(&original_request_id),
            )
            .await
            .expect_err("original evaluation should time out");
        let original_run_id: String =
            sqlx::query_scalar("SELECT id FROM evaluation_runs WHERE request_id = ?1")
                .bind(&original_request_id)
                .fetch_one(database.pool())
                .await
                .expect("original run should query");

        let service = EvaluationService::new(database.pool().clone(), object_store);
        let retry_request_id = uuid::Uuid::new_v4().to_string();
        let retry = service
            .retry_evaluation(&original_run_id, Some(&retry_request_id))
            .await
            .expect("retry should create a new successful run");
        let repeated = service
            .retry_evaluation(&original_run_id, Some(&retry_request_id))
            .await
            .expect("same retry request should resolve idempotently");

        assert_eq!(retry.status, "passed");
        assert_eq!(
            retry.retry_of_run_id.as_deref(),
            Some(original_run_id.as_str())
        );
        assert_ne!(retry.run_id, original_run_id);
        assert!(repeated.reused);
        assert_eq!(repeated.run_id, retry.run_id);
        let runs: Vec<(String, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT id, status, retry_of_run_id
            FROM evaluation_runs
            WHERE object_id = 'obj-retry-evaluation'
            ORDER BY created_at, rowid
            "#,
        )
        .fetch_all(database.pool())
        .await
        .expect("run lineage should query");
        assert_eq!(runs.len(), 2);
        assert_eq!(
            runs[0],
            (original_run_id.clone(), "failed".to_string(), None)
        );
        assert_eq!(
            runs[1],
            (
                retry.run_id.clone(),
                "passed".to_string(),
                Some(original_run_id.clone()),
            )
        );
        let counts: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM evaluation_runs), (SELECT COUNT(*) FROM evaluation_traces), (SELECT COUNT(*) FROM background_jobs WHERE job_type = 'evaluation.run'), (SELECT COUNT(*) FROM evaluation_artifacts)",
        )
        .fetch_one(database.pool())
        .await
        .expect("retry counts should query");
        assert_eq!(counts, (2, 2, 2, 1));
        let retry_payload: String =
            sqlx::query_scalar("SELECT payload_json FROM background_jobs WHERE id = ?1")
                .bind(&retry_request_id)
                .fetch_one(database.pool())
                .await
                .expect("retry job payload should query");
        let retry_payload: serde_json::Value =
            serde_json::from_str(&retry_payload).expect("retry payload should be JSON");
        assert_eq!(retry_payload["retryOfRunId"], original_run_id);

        let trigger_conflict = service
            .trigger_evaluation(
                "obj-retry-evaluation",
                "prompt_evaluator",
                Some(&retry_request_id),
            )
            .await
            .expect_err("retry request id cannot be reused as a root trigger");
        assert!(matches!(trigger_conflict, AppError::PolicyDenied(_)));
        let passed_retry = service
            .retry_evaluation(&retry.run_id, None)
            .await
            .expect_err("passed runs cannot be retried");
        assert!(matches!(passed_retry, AppError::PolicyDenied(_)));
    }

    #[tokio::test]
    async fn github_evaluator_collects_public_fixture_without_token_and_persists_external_evidence()
    {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        seed_parsed_object(
            database.pool(),
            "obj-github-public",
            "github_repo",
            Some("Public Repository"),
            Some("Repository overview with local fallback content."),
        )
        .await;
        sqlx::query(
            "UPDATE knowledge_objects SET canonical_url = 'https://github.com/owner/repo' WHERE id = 'obj-github-public'",
        )
        .execute(database.pool())
        .await
        .expect("GitHub URL should update");
        let repository = r#"{
            "description":"Public fixture",
            "default_branch":"main",
            "language":"Rust",
            "topics":["local-first"],
            "stargazers_count":321,
            "forks_count":42,
            "open_issues_count":7,
            "archived":false,
            "disabled":false,
            "fork":false,
            "private":false,
            "pushed_at":"2026-06-30T12:00:00Z",
            "license":{"name":"MIT License","spdx_id":"MIT"}
        }"#;
        let readme = "# Repo\n## Install\ncargo add repo\n## Usage\nExample workflow";
        let release =
            r#"{"tag_name":"v1.0.0","published_at":"2026-06-29T12:00:00Z","prerelease":false}"#;
        let (base_url, requests) = start_github_fixture_server(vec![
            ("200 OK", "application/json", repository),
            ("200 OK", "text/plain", readme),
            ("200 OK", "application/json", release),
        ])
        .await;
        let telemetry_dir = std::env::temp_dir().join(format!(
            "link-world-github-evaluation-{}",
            uuid::Uuid::new_v4()
        ));
        let service = EvaluationService::new(database.pool().clone(), object_store)
            .with_github_metadata_client(GitHubMetadataClient::for_test(
                &base_url,
                None,
                Duration::from_secs(1),
            ))
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));

        let response = service
            .trigger_evaluation("obj-github-public", "github_repo_evaluator", None)
            .await
            .expect("GitHub evaluation should run");
        let run = service
            .get_evaluation_run(&response.run_id)
            .await
            .expect("GitHub run should query");
        assert_eq!(run.status, "passed");
        assert_eq!(
            run.trace.as_ref().map(|trace| trace.timeout_ms),
            Some(15_000)
        );
        assert!(run
            .evidence
            .iter()
            .any(|item| item.source == "external_check"
                && item.reference.as_deref() == Some("github:readme")));
        assert!(run.evidence.iter().any(|item| {
            item.reference.as_deref() == Some("github:adoption_context")
                && item.text.contains("do not determine")
        }));
        let output_json: String =
            sqlx::query_scalar("SELECT output_json FROM evaluation_runs WHERE id = ?1")
                .bind(&response.run_id)
                .fetch_one(database.pool())
                .await
                .expect("GitHub output should query");
        assert!(output_json.contains("publicMetadata"));
        assert!(output_json.contains("contentHash"));
        assert!(!output_json.contains("cargo add repo"));
        {
            let requests = requests.lock().expect("request log should lock");
            assert_eq!(requests.len(), 3);
            assert!(requests
                .iter()
                .all(|request| !request.to_ascii_lowercase().contains("authorization:")));
        }
        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("GitHub evaluation logs should read");
        assert!(logs.iter().any(|entry| {
            entry.event == "evaluation.github_metadata.succeeded"
                && entry.correlation_id.as_deref() == Some(response.correlation_id.as_str())
        }));
        let serialized_logs =
            std::fs::read_to_string(telemetry_dir.join("logs").join("link-world.jsonl"))
                .expect("GitHub logs should read");
        assert!(!serialized_logs.contains("github.com/owner/repo"));
        assert!(!serialized_logs.contains("cargo add repo"));
        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn github_evaluator_denies_external_metadata_for_secret_object_and_falls_back() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        seed_parsed_object(
            database.pool(),
            "obj-github-secret",
            "github_repo",
            Some("Secret Repository Reference"),
            Some("Saved local repository notes with install guidance."),
        )
        .await;
        sqlx::query(
            "UPDATE knowledge_objects SET canonical_url = 'https://github.com/owner/private', privacy_level = 'secret' WHERE id = 'obj-github-secret'",
        )
        .execute(database.pool())
        .await
        .expect("secret GitHub object should update");
        let telemetry_dir =
            std::env::temp_dir().join(format!("link-world-github-policy-{}", uuid::Uuid::new_v4()));
        let service = EvaluationService::new(database.pool().clone(), object_store)
            .with_github_metadata_client(GitHubMetadataClient::for_test(
                "http://127.0.0.1:1/",
                None,
                Duration::from_millis(10),
            ))
            .with_structured_logger(StructuredLogger::new(&telemetry_dir));

        let response = service
            .trigger_evaluation("obj-github-secret", "github_repo_evaluator", None)
            .await
            .expect("secret object should use saved-content fallback");
        let run = service
            .get_evaluation_run(&response.run_id)
            .await
            .expect("secret fallback run should query");
        assert_eq!(run.status, "passed");
        assert!(run
            .limitations
            .iter()
            .any(|limitation| limitation.contains("privacy policy")));
        assert!(run
            .evidence
            .iter()
            .all(|item| item.source != "external_check"));
        let logs = StructuredLogger::read_recent(&telemetry_dir, Some(20))
            .await
            .expect("policy logs should read");
        assert!(logs.iter().any(|entry| {
            entry.event == "evaluation.github_metadata.degraded"
                && entry.error_code.as_deref() == Some("github.policy_denied")
        }));
        let _ = std::fs::remove_dir_all(telemetry_dir);
    }

    #[tokio::test]
    async fn unsupported_evaluator_is_policy_denied() {
        let database = Database::initialize_in_memory()
            .await
            .expect("database should initialize");
        let object_store = test_object_store();
        seed_parsed_object(
            database.pool(),
            "obj-article",
            "article",
            Some("Article"),
            Some("This is a regular article without repository or prompt structure."),
        )
        .await;
        let service = EvaluationService::new(database.pool().clone(), object_store);

        let error = service
            .trigger_evaluation("obj-article", "unknown_evaluator", None)
            .await
            .expect_err("unsupported evaluator should fail");

        assert!(matches!(error, AppError::PolicyDenied(_)));
    }

    async fn seed_parsed_object(
        pool: &sqlx::SqlitePool,
        object_id: &str,
        object_type: &str,
        title: Option<&str>,
        text: Option<&str>,
    ) {
        sqlx::query(
            r#"
            INSERT INTO knowledge_objects (
                id, user_id, object_type, title, canonical_url, privacy_level, lifecycle_status, captured_at, updated_at
            ) VALUES (?1, 'local', ?2, ?3, 'https://example.com/item', 'personal', 'parsed', '2026-06-17T00:00:00Z', '2026-06-17T00:00:00Z')
            "#,
        )
        .bind(object_id)
        .bind(object_type)
        .bind(title)
        .execute(pool)
        .await
        .expect("object should insert");

        sqlx::query(
            r#"
            INSERT INTO parsed_documents (
                id, object_id, title, text_content, word_count, content_hash, parser_id, parser_version, created_at
            ) VALUES (?1, ?2, ?3, ?4, 12, ?5, 'test.parser', '0.1.0', '2026-06-17T00:00:00Z')
            "#,
        )
        .bind(format!("{object_id}-parsed"))
        .bind(object_id)
        .bind(title)
        .bind(text.unwrap_or("Example content"))
        .bind(format!("{object_id}-hash"))
        .execute(pool)
        .await
        .expect("parsed document should insert");
    }

    async fn start_github_fixture_server(
        responses: Vec<(&'static str, &'static str, &'static str)>,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("GitHub fixture server should bind");
        let address = listener
            .local_addr()
            .expect("GitHub fixture address should be readable");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = requests.clone();
        tokio::spawn(async move {
            for (status, content_type, body) in responses {
                let (mut socket, _) = listener
                    .accept()
                    .await
                    .expect("GitHub fixture should accept request");
                let mut buffer = vec![0_u8; 8192];
                let read = socket
                    .read(&mut buffer)
                    .await
                    .expect("GitHub fixture request should read");
                request_log
                    .lock()
                    .expect("request log should lock")
                    .push(String::from_utf8_lossy(&buffer[..read]).to_string());
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("GitHub fixture response should write");
            }
        });

        (format!("http://{address}/"), requests)
    }

    fn test_object_store() -> ObjectStore {
        let root = std::env::temp_dir().join(format!("link-world-test-{}", uuid::Uuid::new_v4()));
        ObjectStore::initialize(root).expect("object store should initialize")
    }
}

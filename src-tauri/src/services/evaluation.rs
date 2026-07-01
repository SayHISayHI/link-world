use crate::domain::evaluation::{
    EvaluationArtifactSubmission, EvaluationFailureSubmission, EvaluationInput,
    EvaluationOperation, EvaluationOutput, EvaluationPlan, EvaluationRunReservation,
    EvaluationRunSubmission, EvaluationTraceCompletion, EvaluatorCapability,
    TriggerEvaluationResponse, DEFAULT_EVALUATION_TIMEOUT_MS, EVALUATION_CAPABILITY_SCHEMA_VERSION,
    EVALUATION_INPUT_SCHEMA_VERSION, EVALUATION_OUTPUT_SCHEMA_VERSION,
    EVALUATION_PLAN_SCHEMA_VERSION,
};
use crate::domain::knowledge::{EvaluationRun, EvidenceItem};
use crate::errors::{AppError, AppResult};
use crate::repositories::evaluation::EvaluationRepository;
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
    execution_timeout: Duration,
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
    fn run(&self, input: &EvaluationInput, plan: &EvaluationPlan) -> EvaluationOutput;
}

struct PromptEvaluator;
struct GitHubRepoEvaluator;

impl EvaluationService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self {
            repository: EvaluationRepository::new(state.database()?.pool().clone()),
            object_store: state.object_store()?.clone(),
            structured_logger: state.structured_logger().cloned(),
            execution_timeout: Duration::from_millis(DEFAULT_EVALUATION_TIMEOUT_MS as u64),
            execution_delay: Duration::ZERO,
        })
    }

    pub fn new(pool: SqlitePool, object_store: ObjectStore) -> Self {
        Self {
            repository: EvaluationRepository::new(pool),
            object_store,
            structured_logger: None,
            execution_timeout: Duration::from_millis(DEFAULT_EVALUATION_TIMEOUT_MS as u64),
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
        let requested_type = normalize_evaluator_type(evaluator_type)?;
        let request_id = normalize_request_id(request_id)?;
        if let Some(operation) = self.repository.find_operation(&request_id).await? {
            let response = response_from_existing_operation(operation, object_id, &requested_type)?;
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
            requested_evaluator_type: requested_type.clone(),
            evaluator_type: capability.evaluator_type.clone(),
            evaluator_version: capability.evaluator_version.clone(),
            plan_schema_version: capability.plan_schema_version,
            input_schema_version: capability.input_schema_version,
            output_schema_version: capability.output_schema_version,
            trace_id,
            execution_kind: capability.execution_kind.clone(),
            input_hash: input.content_hash.clone(),
            timeout_ms: self.execution_timeout.as_millis().min(i64::MAX as u128) as i64,
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
                return response_from_existing_operation(operation, object_id, &requested_type);
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
            let output = run_evaluator_with_timeout(
                capability.evaluator_type.clone(),
                input.clone(),
                plan.clone(),
                self.execution_timeout,
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
            status: "passed".to_string(),
            reused: false,
        })
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
        self.execution_timeout = execution_timeout;
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

    fn run(&self, input: &EvaluationInput, plan: &EvaluationPlan) -> EvaluationOutput {
        let text = input.text_content.as_str();
        let lower = text.to_lowercase();
        let variable_count = count_prompt_variables(text);
        let has_role = contains_any(&lower, &["you are", "act as", "role:", "system:"]);
        let has_task = contains_any(
            &lower,
            &["task", "goal", "objective", "你是", "请", "目标", "任务"],
        );
        let has_constraints = contains_any(
            &lower,
            &["constraint", "must", "avoid", "不得", "必须", "不要"],
        );
        let has_output_format = contains_any(
            &lower,
            &["json", "markdown", "schema", "format", "输出格式", "表格"],
        );
        let has_examples = contains_any(&lower, &["example", "few-shot", "示例", "例如"]);
        let unsafe_marker = contains_any(
            &lower,
            &["ignore previous", "api key", "cookie", "password", "绕过"],
        );

        let clarity = score_bool(has_task, 0.35) + score_length(text);
        let specificity =
            0.25 + score_bool(has_constraints, 0.25) + (variable_count.min(4) as f64 * 0.08);
        let testability = 0.25 + score_bool(has_output_format, 0.3) + score_bool(has_examples, 0.2);
        let reusability = 0.25 + (variable_count.min(5) as f64 * 0.1) + score_bool(has_role, 0.15);
        let safety = if unsafe_marker { 0.25 } else { 0.85 };
        let dimensions = json!({
            "clarity": clamp_score(clarity),
            "specificity": clamp_score(specificity),
            "testability": clamp_score(testability),
            "reusability": clamp_score(reusability),
            "safety": clamp_score(safety),
        });
        let score = average_dimension_scores(&dimensions);
        let verdict = verdict_from_score(score, safety);
        let evidence = vec![
            EvidenceItem {
                source: "original_content".to_string(),
                text: format!("Detected {variable_count} reusable placeholder(s)."),
                reference: Some("placeholder_scan".to_string()),
            },
            EvidenceItem {
                source: "original_content".to_string(),
                text: if has_output_format {
                    "Prompt declares an output format.".to_string()
                } else {
                    "Prompt does not clearly declare an output format.".to_string()
                },
                reference: Some("output_format_check".to_string()),
            },
            EvidenceItem {
                source: "original_content".to_string(),
                text: if has_examples {
                    "Prompt includes example-style guidance.".to_string()
                } else {
                    "No example or few-shot pattern was detected.".to_string()
                },
                reference: Some("example_check".to_string()),
            },
        ];
        let next_actions = build_prompt_next_actions(
            has_role,
            has_constraints,
            has_output_format,
            has_examples,
            variable_count,
        );
        let limitations = vec![
            "MVP prompt evaluator uses deterministic heuristics and does not execute the prompt against a live model.".to_string(),
            "Scores should be treated as triage signals until sandboxed test runs are added.".to_string(),
        ];
        let report = json!({
            "plan": plan,
            "testCases": build_prompt_test_cases(input),
            "detectedSignals": {
                "hasRole": has_role,
                "hasTask": has_task,
                "hasConstraints": has_constraints,
                "hasOutputFormat": has_output_format,
                "hasExamples": has_examples,
                "variableCount": variable_count,
                "unsafeMarker": unsafe_marker,
            }
        });

        EvaluationOutput {
            schema_version: EVALUATION_OUTPUT_SCHEMA_VERSION,
            score,
            verdict,
            dimensions,
            evidence,
            limitations,
            next_actions,
            report,
        }
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
        (requested_type == GITHUB_REPO_EVALUATOR_TYPE || requested_type == AUTO_EVALUATOR_TYPE)
            && is_github_repo(input)
    }

    fn plan(&self, input: &EvaluationInput) -> EvaluationPlan {
        EvaluationPlan {
            schema_version: EVALUATION_PLAN_SCHEMA_VERSION,
            evaluator_type: self.evaluator_type().to_string(),
            evaluator_version: self.evaluator_version().to_string(),
            steps: vec![
                "Inspect saved repository page or README content.".to_string(),
                "Score documentation, installation path, maintenance signals and risk.".to_string(),
                "Record limitations when live GitHub metadata is unavailable.".to_string(),
            ],
            checks: vec![
                format!(
                    "repo_url: {}",
                    input.canonical_url.as_deref().unwrap_or("unknown")
                ),
                "live GitHub API lookup is intentionally not used in MVP local evaluator"
                    .to_string(),
            ],
        }
    }

    fn run(&self, input: &EvaluationInput, plan: &EvaluationPlan) -> EvaluationOutput {
        let lower = input.text_content.to_lowercase();
        let has_readme = contains_any(&lower, &["readme", "# ", "overview"]);
        let has_install = contains_any(
            &lower,
            &["install", "npm install", "cargo add", "pip install"],
        );
        let has_usage = contains_any(
            &lower,
            &["usage", "quickstart", "example", "getting started"],
        );
        let has_license = contains_any(&lower, &["license", "mit", "apache", "gpl"]);
        let has_activity = contains_any(&lower, &["stars", "forks", "last commit", "contributors"]);
        let has_risk = contains_any(
            &lower,
            &["deprecated", "unmaintained", "archived", "warning"],
        );

        let dimensions = json!({
            "documentation": clamp_score(0.25 + score_bool(has_readme, 0.2) + score_bool(has_usage, 0.25)),
            "adoptionSignals": clamp_score(0.35 + score_bool(has_activity, 0.25)),
            "maintenanceSignals": clamp_score(if has_risk { 0.35 } else { 0.65 } + score_bool(has_activity, 0.15)),
            "actionability": clamp_score(0.25 + score_bool(has_install, 0.25) + score_bool(has_usage, 0.25)),
            "licensing": clamp_score(0.35 + score_bool(has_license, 0.35)),
        });
        let score = average_dimension_scores(&dimensions);
        let verdict = verdict_from_score(score, 0.8);
        let evidence = vec![
            EvidenceItem {
                source: "original_content".to_string(),
                text: if has_install {
                    "Installation guidance was detected in the saved content.".to_string()
                } else {
                    "No clear installation guidance was detected in the saved content.".to_string()
                },
                reference: Some("install_check".to_string()),
            },
            EvidenceItem {
                source: "original_content".to_string(),
                text: if has_license {
                    "License-related signal was detected.".to_string()
                } else {
                    "No license signal was detected from saved content.".to_string()
                },
                reference: Some("license_check".to_string()),
            },
        ];
        let next_actions = vec![
            json!({
                "title": "Open repository and verify stars, latest commit and license before adoption.",
                "priority": "high",
            }),
            json!({
                "title": "Try the documented quickstart in a sandbox before adding it to a workflow.",
                "priority": "medium",
            }),
        ];
        let limitations = vec![
            "MVP GitHub evaluator only uses saved content and URL structure; it does not call GitHub API yet.".to_string(),
            "Stars, last commit and issue health must be verified manually until the GitHub metadata adapter is added.".to_string(),
        ];
        let report = json!({
            "plan": plan,
            "repo": parse_github_repo(input.canonical_url.as_deref()),
            "detectedSignals": {
                "hasReadme": has_readme,
                "hasInstall": has_install,
                "hasUsage": has_usage,
                "hasLicense": has_license,
                "hasActivity": has_activity,
                "hasRisk": has_risk,
            }
        });

        EvaluationOutput {
            schema_version: EVALUATION_OUTPUT_SCHEMA_VERSION,
            score,
            verdict,
            dimensions,
            evidence,
            limitations,
            next_actions,
            report,
        }
    }
}

async fn run_evaluator_with_timeout(
    evaluator_type: String,
    input: EvaluationInput,
    plan: EvaluationPlan,
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
        Ok::<_, AppError>(evaluator.run(&input, &plan))
    });

    match tokio::time::timeout(execution_timeout, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(AppError::Unknown("evaluation.runner_stopped".to_string())),
        Err(_) => Err(AppError::NetworkTimeout),
    }
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
) -> AppResult<TriggerEvaluationResponse> {
    if operation.object_id != object_id
        || operation.requested_evaluator_type != requested_evaluator_type
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
fn build_prompt_next_actions(
    has_role: bool,
    has_constraints: bool,
    has_output_format: bool,
    has_examples: bool,
    variable_count: usize,
) -> Vec<Value> {
    let mut actions = Vec::new();

    if !has_role {
        actions.push(json!({
            "title": "Add an explicit role or operating context.",
            "priority": "medium",
        }));
    }
    if !has_constraints {
        actions.push(json!({
            "title": "Add constraints for scope, exclusions and quality bar.",
            "priority": "high",
        }));
    }
    if !has_output_format {
        actions.push(json!({
            "title": "Declare the desired output schema or format.",
            "priority": "high",
        }));
    }
    if !has_examples {
        actions.push(json!({
            "title": "Add at least one example input and expected output.",
            "priority": "medium",
        }));
    }
    if variable_count == 0 {
        actions.push(json!({
            "title": "Introduce placeholders for reusable inputs.",
            "priority": "low",
        }));
    }

    if actions.is_empty() {
        actions.push(json!({
            "title": "Run this prompt against two realistic samples and compare outputs.",
            "priority": "medium",
        }));
    }

    actions
}

fn build_prompt_test_cases(input: &EvaluationInput) -> Vec<Value> {
    vec![
        json!({
            "name": "happy_path",
            "input": "Use a representative user request with complete context.",
            "expected": "The response follows the requested format and includes actionable output.",
        }),
        json!({
            "name": "missing_context",
            "input": "Remove one required input variable or context block.",
            "expected": "The prompt should ask for clarification instead of hallucinating.",
        }),
        json!({
            "name": "edge_constraints",
            "input": format!(
                "Stress test with a long or ambiguous input for '{}'.",
                input.title.as_deref().unwrap_or("this prompt")
            ),
            "expected": "The response should preserve constraints and avoid unsafe assumptions.",
        }),
    ]
}

fn looks_like_prompt(text: &str) -> bool {
    let lower = text.to_lowercase();
    contains_any(
        &lower,
        &[
            "you are",
            "act as",
            "system:",
            "prompt",
            "输出格式",
            "你是",
            "{{",
            "<input>",
        ],
    )
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

fn count_prompt_variables(text: &str) -> usize {
    let brace_pairs = text.match_indices("{{").count();
    let angle_inputs = text
        .match_indices('<')
        .count()
        .min(text.match_indices('>').count());
    let dollar_vars = text
        .split_whitespace()
        .filter(|token| token.starts_with('$') && token.len() > 1)
        .count();

    brace_pairs + angle_inputs + dollar_vars
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn score_bool(value: bool, weight: f64) -> f64 {
    if value {
        weight
    } else {
        0.0
    }
}

fn score_length(text: &str) -> f64 {
    let chars = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    if chars >= 240 {
        0.35
    } else if chars >= 120 {
        0.25
    } else if chars >= 48 {
        0.15
    } else {
        0.05
    }
}

fn average_dimension_scores(dimensions: &Value) -> f64 {
    let Some(object) = dimensions.as_object() else {
        return 0.0;
    };
    let mut total = 0.0;
    let mut count = 0.0;

    for value in object.values() {
        if let Some(score) = value.as_f64() {
            total += score;
            count += 1.0;
        }
    }

    if count == 0.0 {
        return 0.0;
    }

    round_score(total / count)
}

fn verdict_from_score(score: f64, safety: f64) -> String {
    if safety < 0.35 {
        "unsafe"
    } else if score >= 0.82 {
        "high_value"
    } else if score >= 0.65 {
        "useful"
    } else if score >= 0.45 {
        "situational"
    } else {
        "low_value"
    }
    .to_string()
}

fn clamp_score(score: f64) -> f64 {
    round_score(score.clamp(0.0, 1.0))
}

fn round_score(score: f64) -> f64 {
    (score * 100.0).round() / 100.0
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
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use crate::telemetry::StructuredLogger;
    use std::time::Duration;

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
        let capabilities = service.list_evaluator_capabilities();
        assert_eq!(capabilities.len(), 2);
        assert!(capabilities.iter().all(|capability| {
            capability.schema_version == 1
                && capability.plan_schema_version == 1
                && capability.input_schema_version == 1
                && capability.output_schema_version == 1
                && capability.execution_kind == "local_deterministic"
                && !capability.requires_network
                && !capability.requires_model
                && !capability.requires_sandbox
        }));
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

    fn test_object_store() -> ObjectStore {
        let root = std::env::temp_dir().join(format!("link-world-test-{}", uuid::Uuid::new_v4()));
        ObjectStore::initialize(root).expect("object store should initialize")
    }
}

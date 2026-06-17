use crate::domain::evaluation::{
    EvaluationArtifactSubmission, EvaluationInput, EvaluationOutput, EvaluationPlan,
    EvaluationRunSubmission, TriggerEvaluationResponse,
};
use crate::domain::knowledge::{EvaluationRun, EvidenceItem};
use crate::errors::{AppError, AppResult};
use crate::repositories::evaluation::EvaluationRepository;
use crate::state::AppState;
use crate::storage::object_store::ObjectStore;
use chrono::Utc;
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use uuid::Uuid;

const PROMPT_EVALUATOR_TYPE: &str = "prompt_evaluator";
const GITHUB_REPO_EVALUATOR_TYPE: &str = "github_repo_evaluator";
const AUTO_EVALUATOR_TYPE: &str = "auto";
const EVALUATOR_VERSION: &str = "0.1.0";

#[derive(Clone)]
pub struct EvaluationService {
    repository: EvaluationRepository,
    object_store: ObjectStore,
}

trait EvaluatorPlugin: Send + Sync {
    fn evaluator_type(&self) -> &'static str;
    fn evaluator_version(&self) -> &'static str {
        EVALUATOR_VERSION
    }
    fn supports(&self, input: &EvaluationInput, requested_type: &str) -> bool;
    fn plan(&self, input: &EvaluationInput) -> EvaluationPlan;
    fn run(&self, input: &EvaluationInput, plan: &EvaluationPlan) -> EvaluationOutput;
}

struct PromptEvaluator;
struct GitHubRepoEvaluator;

impl EvaluationService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        Ok(Self::new(
            state.database()?.pool().clone(),
            state.object_store()?.clone(),
        ))
    }

    pub fn new(pool: SqlitePool, object_store: ObjectStore) -> Self {
        Self {
            repository: EvaluationRepository::new(pool),
            object_store,
        }
    }

    pub async fn trigger_evaluation(
        &self,
        object_id: &str,
        evaluator_type: &str,
    ) -> AppResult<TriggerEvaluationResponse> {
        let input = self.repository.get_evaluation_input(object_id).await?;
        let requested_type = normalize_evaluator_type(evaluator_type);
        let evaluator = select_evaluator(&input, &requested_type)?;
        let plan = evaluator.plan(&input);
        let output = evaluator.run(&input, &plan);
        let run_id = Uuid::new_v4().to_string();
        let artifact_id = Uuid::new_v4().to_string();
        let job_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let input_json = build_input_snapshot_json(&input, &requested_type)?;
        let output_json = serialize_json(&output.report)?;
        let report_bytes = serde_json::to_vec_pretty(&json!({
            "runId": run_id,
            "objectId": input.object_id,
            "plan": plan,
            "input": input_json,
            "output": output,
        }))
        .map_err(|error| AppError::ModelOutputSchema(error.to_string()))?;
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
            object_id: input.object_id.clone(),
            evaluator_type: evaluator.evaluator_type().to_string(),
            evaluator_version: evaluator.evaluator_version().to_string(),
            status: "passed".to_string(),
            plan_json: serialize_json(&plan)?,
            input_json,
            output_json,
            dimensions_json: serialize_json(&output.dimensions)?,
            evidence_json: serialize_json(&output.evidence)?,
            limitations_json: serialize_json(&output.limitations)?,
            next_actions_json: serialize_json(&output.next_actions)?,
            score: Some(output.score),
            verdict: output.verdict,
            failure_reason: None,
            created_at: now.clone(),
            completed_at: Some(now.clone()),
        };
        let artifact = EvaluationArtifactSubmission {
            id: artifact_id,
            evaluation_run_id: run_id.clone(),
            artifact_type: "report".to_string(),
            storage_uri: stored_report.storage_uri,
            content_hash: Some(stored_report.content_hash),
            metadata_json: Some(
                json!({
                    "evaluatorType": run.evaluator_type,
                    "evaluatorVersion": run.evaluator_version,
                    "contentHash": input.content_hash,
                })
                .to_string(),
            ),
            created_at: now,
        };

        self.repository
            .insert_completed_evaluation(&input.user_id, &job_id, &run, &[artifact])
            .await?;

        Ok(TriggerEvaluationResponse { run_id })
    }

    pub async fn get_evaluation_run(&self, run_id: &str) -> AppResult<EvaluationRun> {
        self.repository.get_evaluation_run(run_id).await
    }
}

impl EvaluatorPlugin for PromptEvaluator {
    fn evaluator_type(&self) -> &'static str {
        PROMPT_EVALUATOR_TYPE
    }

    fn supports(&self, input: &EvaluationInput, requested_type: &str) -> bool {
        requested_type == PROMPT_EVALUATOR_TYPE
            || (requested_type == AUTO_EVALUATOR_TYPE
                && (input.object_type == "prompt" || looks_like_prompt(&input.text_content)))
    }

    fn plan(&self, input: &EvaluationInput) -> EvaluationPlan {
        EvaluationPlan {
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

    fn supports(&self, input: &EvaluationInput, requested_type: &str) -> bool {
        requested_type == GITHUB_REPO_EVALUATOR_TYPE
            || (requested_type == AUTO_EVALUATOR_TYPE && is_github_repo(input))
    }

    fn plan(&self, input: &EvaluationInput) -> EvaluationPlan {
        EvaluationPlan {
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

fn select_evaluator(
    input: &EvaluationInput,
    requested_type: &str,
) -> AppResult<Box<dyn EvaluatorPlugin + Send + Sync>> {
    let evaluators: Vec<Box<dyn EvaluatorPlugin + Send + Sync>> =
        vec![Box::new(GitHubRepoEvaluator), Box::new(PromptEvaluator)];

    evaluators
        .into_iter()
        .find(|evaluator| evaluator.supports(input, requested_type))
        .ok_or_else(|| {
            AppError::PolicyDenied(format!(
                "no evaluator supports object type '{}' with requested evaluator '{}'",
                input.object_type, requested_type
            ))
        })
}

fn normalize_evaluator_type(evaluator_type: &str) -> String {
    let trimmed = evaluator_type.trim();
    if trimmed.is_empty() {
        AUTO_EVALUATOR_TYPE.to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

fn build_input_snapshot_json(input: &EvaluationInput, requested_type: &str) -> AppResult<String> {
    serialize_json(&json!({
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
    }))
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
    use super::EvaluationService;
    use crate::errors::AppError;
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;

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
            .trigger_evaluation("obj-prompt", "prompt_evaluator")
            .await
            .expect("evaluation should run");
        let run = service
            .get_evaluation_run(&response.run_id)
            .await
            .expect("run should be readable");

        assert_eq!(run.status, "passed");
        assert_eq!(run.evaluator_type, "prompt_evaluator");
        assert!(run.score.unwrap_or_default() > 0.5);
        assert_eq!(run.artifacts.len(), 1);

        let lifecycle_status: String =
            sqlx::query_scalar("SELECT lifecycle_status FROM knowledge_objects WHERE id = ?1")
                .bind("obj-prompt")
                .fetch_one(database.pool())
                .await
                .expect("status should be readable");
        let job_type: String =
            sqlx::query_scalar("SELECT job_type FROM background_jobs WHERE object_id = ?1")
                .bind("obj-prompt")
                .fetch_one(database.pool())
                .await
                .expect("job should be readable");
        let event_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM domain_events WHERE object_id = ?1 AND event_type = 'evaluation.completed'",
        )
        .bind("obj-prompt")
        .fetch_one(database.pool())
        .await
        .expect("event count should be readable");

        assert_eq!(lifecycle_status, "evaluated");
        assert_eq!(job_type, "evaluation.run");
        assert_eq!(event_count, 1);
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
            .trigger_evaluation("obj-article", "unknown_evaluator")
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

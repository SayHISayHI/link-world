use crate::domain::evaluation::{
    EvaluationInput, EvaluationOutput, EvaluationPlan, EVALUATION_OUTPUT_SCHEMA_VERSION,
};
use crate::domain::knowledge::EvidenceItem;
use crate::services::github::{GitHubMetadataOutcome, GitHubRepositoryMetadata};
use chrono::Utc;
use serde_json::{json, Value};

pub fn evaluate_github_repository(
    input: &EvaluationInput,
    plan: &EvaluationPlan,
    github_metadata: Option<&GitHubMetadataOutcome>,
) -> EvaluationOutput {
    let lower = input.text_content.to_ascii_lowercase();
    let saved_has_install = contains_any(
        &lower,
        &["install", "npm install", "cargo add", "pip install"],
    );
    let saved_has_usage = contains_any(
        &lower,
        &["usage", "quickstart", "example", "getting started"],
    );
    let saved_has_license = contains_any(&lower, &["license", "mit", "apache", "gpl"]);
    let saved_has_risk = contains_any(
        &lower,
        &["deprecated", "unmaintained", "archived", "warning"],
    );

    let mut evidence = vec![EvidenceItem {
        source: "original_content".to_string(),
        text: if saved_has_install || saved_has_usage {
            "Saved content contains installation or usage guidance.".to_string()
        } else {
            "Saved content does not contain clear installation or usage guidance.".to_string()
        },
        reference: Some("saved_documentation_check".to_string()),
    }];
    let mut limitations = Vec::new();
    let mut next_actions = Vec::new();
    let mut public_metadata = Value::Null;

    let mut documentation =
        0.25 + score_bool(saved_has_install, 0.2) + score_bool(saved_has_usage, 0.2);
    let mut licensing = 0.25 + score_bool(saved_has_license, 0.25);
    let mut maintenance = if saved_has_risk { 0.3 } else { 0.55 };
    let mut adoption_signals = 0.3;
    let mut actionability =
        0.25 + score_bool(saved_has_install, 0.2) + score_bool(saved_has_usage, 0.2);
    let mut risk = if saved_has_risk { 0.4 } else { 0.75 };
    let mut terminal_repository_risk = false;

    match github_metadata {
        Some(GitHubMetadataOutcome::Available(metadata)) => {
            apply_public_metadata(
                metadata,
                &mut documentation,
                &mut licensing,
                &mut maintenance,
                &mut adoption_signals,
                &mut actionability,
                &mut risk,
                &mut terminal_repository_risk,
                &mut evidence,
                &mut limitations,
                &mut next_actions,
            );
            public_metadata = serde_json::to_value(metadata).unwrap_or(Value::Null);
        }
        Some(GitHubMetadataOutcome::Unavailable { code }) => {
            limitations.push(github_limitation(code));
            next_actions.push(json!({
                "title": "Retry public GitHub metadata collection when the limitation is resolved.",
                "priority": "medium",
            }));
        }
        None => limitations.push(
            "Public GitHub metadata was not collected; scoring uses only saved content."
                .to_string(),
        ),
    }

    documentation = clamp_score(documentation);
    licensing = clamp_score(licensing);
    maintenance = clamp_score(maintenance);
    adoption_signals = clamp_score(adoption_signals);
    actionability = clamp_score(actionability);
    risk = clamp_score(risk);
    let dimensions = json!({
        "documentation": documentation,
        "licensing": licensing,
        "maintenanceSignals": maintenance,
        "adoptionSignals": adoption_signals,
        "actionability": actionability,
        "riskPosture": risk,
    });
    let score = average_dimension_scores(&dimensions);
    let verdict = if terminal_repository_risk {
        "low_value".to_string()
    } else {
        verdict_from_score(score)
    };

    if !next_actions.iter().any(|action| {
        action
            .get("title")
            .and_then(Value::as_str)
            .is_some_and(|title| {
                title.contains("sandbox") || title.contains("isolated environment")
            })
    }) {
        next_actions.push(json!({
            "title": "Run the documented quickstart in an isolated environment before adoption.",
            "priority": "high",
        }));
    }
    next_actions.push(json!({
        "title": "Compare one actively maintained alternative on fit, license and integration cost.",
        "priority": "medium",
    }));

    EvaluationOutput {
        schema_version: EVALUATION_OUTPUT_SCHEMA_VERSION,
        score,
        verdict,
        dimensions,
        evidence,
        limitations,
        next_actions,
        report: json!({
            "plan": plan,
            "repository": {
                "canonical": input.canonical_url,
                "publicMetadata": public_metadata,
            },
            "savedSignals": {
                "hasInstallation": saved_has_install,
                "hasUsage": saved_has_usage,
                "hasLicense": saved_has_license,
                "hasRiskMarker": saved_has_risk,
            },
            "scoringBoundary": "Stars and forks are contextual adoption signals only; they cannot determine the verdict alone.",
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_public_metadata(
    metadata: &GitHubRepositoryMetadata,
    documentation: &mut f64,
    licensing: &mut f64,
    maintenance: &mut f64,
    adoption_signals: &mut f64,
    actionability: &mut f64,
    risk: &mut f64,
    terminal_repository_risk: &mut bool,
    evidence: &mut Vec<EvidenceItem>,
    limitations: &mut Vec<String>,
    next_actions: &mut Vec<Value>,
) {
    *documentation = 0.2
        + score_bool(metadata.readme.available, 0.15)
        + score_bool(metadata.readme.has_installation, 0.2)
        + score_bool(metadata.readme.has_usage, 0.2)
        + score_bool(metadata.readme.has_examples, 0.1);
    *licensing = if metadata.license_spdx_id.is_some() {
        0.9
    } else {
        0.25
    };
    let activity = activity_score(metadata.pushed_at.as_deref());
    *maintenance = 0.1
        + activity * 0.45
        + score_bool(metadata.latest_release.is_some(), 0.15)
        + score_bool(!metadata.archived, 0.15)
        + score_bool(!metadata.disabled, 0.15);
    *adoption_signals = adoption_score(metadata.stars, metadata.forks);
    *actionability = 0.2
        + score_bool(metadata.readme.has_installation, 0.25)
        + score_bool(metadata.readme.has_usage, 0.25)
        + score_bool(metadata.readme.has_examples, 0.1)
        + score_bool(metadata.primary_language.is_some(), 0.1);
    *terminal_repository_risk = metadata.archived || metadata.disabled;
    *risk = if *terminal_repository_risk {
        0.15
    } else {
        0.85
    };

    evidence.push(EvidenceItem {
        source: "external_check".to_string(),
        text: format!(
            "GitHub README check: available={}, installation={}, usage={}, examples={}.",
            metadata.readme.available,
            metadata.readme.has_installation,
            metadata.readme.has_usage,
            metadata.readme.has_examples,
        ),
        reference: Some("github:readme".to_string()),
    });
    evidence.push(EvidenceItem {
        source: "external_check".to_string(),
        text: match (&metadata.license_spdx_id, &metadata.license_name) {
            (Some(spdx), Some(name)) => {
                format!("GitHub detected license {name} with SPDX identifier {spdx}.")
            }
            _ => "GitHub did not report a recognized repository license.".to_string(),
        },
        reference: Some("github:license".to_string()),
    });
    evidence.push(EvidenceItem {
        source: "external_check".to_string(),
        text: format!(
            "Public activity signals: pushed_at={}, latest_release={}.",
            metadata.pushed_at.as_deref().unwrap_or("unavailable"),
            metadata
                .latest_release
                .as_ref()
                .map(|release| release.tag_name.as_str())
                .unwrap_or("unavailable"),
        ),
        reference: Some("github:activity".to_string()),
    });
    evidence.push(EvidenceItem {
        source: "external_check".to_string(),
        text: format!(
            "Adoption context only: {} stars, {} forks and {} open issues; these counts do not determine the verdict.",
            metadata.stars, metadata.forks, metadata.open_issues,
        ),
        reference: Some("github:adoption_context".to_string()),
    });

    if *terminal_repository_risk {
        evidence.push(EvidenceItem {
            source: "external_check".to_string(),
            text: format!(
                "Repository state reports archived={} and disabled={}.",
                metadata.archived, metadata.disabled,
            ),
            reference: Some("github:repository_state".to_string()),
        });
        next_actions.push(json!({
            "title": "Prefer an actively maintained alternative before adopting this repository.",
            "priority": "high",
        }));
    }
    if metadata.license_spdx_id.is_none() {
        next_actions.push(json!({
            "title": "Verify licensing and dependency licenses before reuse.",
            "priority": "high",
        }));
    }
    if metadata.latest_release.is_none() {
        limitations.push(
            "No latest public GitHub release was available; release cadence is unverified."
                .to_string(),
        );
    }
    if metadata.pushed_at.is_none() {
        limitations.push(
            "GitHub did not provide a recent push timestamp; maintenance recency is uncertain."
                .to_string(),
        );
    }
    limitations.extend(
        metadata
            .limitations
            .iter()
            .map(|code| github_limitation(code)),
    );
}

fn github_limitation(code: &str) -> String {
    match code {
        "github.auth_failed" => {
            "GitHub authentication failed; public metadata enrichment was unavailable."
        }
        "github.forbidden" => {
            "GitHub refused the metadata request; saved content remains the evaluation fallback."
        }
        "github.invalid_repository" => {
            "The saved URL is not a valid public github.com repository URL."
        }
        "github.not_found_or_private" | "github.private_repository" => {
            "The repository was missing or private; private repository metadata is not evaluated."
        }
        "github.policy_denied" => {
            "The object's privacy policy denied external GitHub metadata collection."
        }
        "github.rate_limited" => {
            "GitHub rate-limited metadata collection; retry after the provider window resets."
        }
        "github.response_too_large" => {
            "A GitHub metadata response exceeded the local safety limit and was ignored."
        }
        "github.timeout" => {
            "GitHub metadata collection timed out; saved content remains the fallback."
        }
        _ => "GitHub metadata was unavailable; saved content remains the fallback.",
    }
    .to_string()
}

fn activity_score(pushed_at: Option<&str>) -> f64 {
    let Some(pushed_at) = pushed_at
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
    else {
        return 0.25;
    };
    let age_days = Utc::now()
        .signed_duration_since(pushed_at)
        .num_days()
        .max(0);
    match age_days {
        0..=90 => 1.0,
        91..=365 => 0.75,
        366..=730 => 0.5,
        _ => 0.2,
    }
}

fn adoption_score(stars: u64, forks: u64) -> f64 {
    let star_signal = match stars {
        0..=9 => 0.0,
        10..=99 => 0.08,
        100..=999 => 0.14,
        1_000..=9_999 => 0.2,
        _ => 0.25,
    };
    let fork_signal = match forks {
        0..=4 => 0.0,
        5..=49 => 0.05,
        50..=499 => 0.1,
        _ => 0.15,
    };
    0.35 + star_signal + fork_signal
}

fn average_dimension_scores(dimensions: &Value) -> f64 {
    let Some(object) = dimensions.as_object() else {
        return 0.0;
    };
    let (total, count) = object.values().fold((0.0, 0_u64), |(total, count), value| {
        value
            .as_f64()
            .map(|score| (total + score, count + 1))
            .unwrap_or((total, count))
    });
    if count == 0 {
        0.0
    } else {
        round_score(total / count as f64)
    }
}

fn verdict_from_score(score: f64) -> String {
    if score >= 0.82 {
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

fn score_bool(value: bool, weight: f64) -> f64 {
    if value {
        weight
    } else {
        0.0
    }
}

fn clamp_score(score: f64) -> f64 {
    round_score(score.clamp(0.0, 1.0))
}

fn round_score(score: f64) -> f64 {
    (score * 100.0).round() / 100.0
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::evaluate_github_repository;
    use crate::domain::evaluation::{EvaluationInput, EvaluationPlan};
    use crate::services::github::{
        GitHubMetadataOutcome, GitHubReadmeSignals, GitHubReleaseMetadata, GitHubRepositoryMetadata,
    };
    use chrono::{Duration, Utc};

    #[test]
    fn public_fixture_produces_stable_evidence_without_star_only_verdict() {
        let input = fixture_input();
        let plan = fixture_plan();
        let metadata = GitHubRepositoryMetadata {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            description: Some("Fixture".to_string()),
            default_branch: "main".to_string(),
            primary_language: Some("Rust".to_string()),
            topics: vec!["local-first".to_string()],
            stars: 10_000,
            forks: 1_000,
            open_issues: 12,
            archived: false,
            disabled: false,
            fork: false,
            pushed_at: Some((Utc::now() - Duration::days(30)).to_rfc3339()),
            license_spdx_id: Some("MIT".to_string()),
            license_name: Some("MIT License".to_string()),
            readme: GitHubReadmeSignals {
                available: true,
                byte_length: 256,
                content_hash: Some("readme-hash".to_string()),
                has_installation: true,
                has_usage: true,
                has_examples: true,
                has_security_policy: false,
            },
            latest_release: Some(GitHubReleaseMetadata {
                tag_name: "v1.0.0".to_string(),
                published_at: Some((Utc::now() - Duration::days(45)).to_rfc3339()),
                prerelease: false,
            }),
            authenticated: false,
            limitations: Vec::new(),
        };

        let output = evaluate_github_repository(
            &input,
            &plan,
            Some(&GitHubMetadataOutcome::Available(Box::new(metadata))),
        );
        assert!(matches!(output.verdict.as_str(), "useful" | "high_value"));
        assert!(output
            .evidence
            .iter()
            .any(|item| item.reference.as_deref() == Some("github:readme")));
        assert!(output.evidence.iter().any(|item| {
            item.reference.as_deref() == Some("github:adoption_context")
                && item.text.contains("do not determine")
        }));
        assert_eq!(
            output.report["scoringBoundary"],
            "Stars and forks are contextual adoption signals only; they cannot determine the verdict alone."
        );
    }

    #[test]
    fn star_count_cannot_rescue_archived_repository() {
        let input = fixture_input();
        let plan = fixture_plan();
        let metadata = GitHubRepositoryMetadata {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            description: None,
            default_branch: "main".to_string(),
            primary_language: None,
            topics: Vec::new(),
            stars: 1_000_000,
            forks: 100_000,
            open_issues: 0,
            archived: true,
            disabled: false,
            fork: false,
            pushed_at: None,
            license_spdx_id: None,
            license_name: None,
            readme: GitHubReadmeSignals {
                available: false,
                byte_length: 0,
                content_hash: None,
                has_installation: false,
                has_usage: false,
                has_examples: false,
                has_security_policy: false,
            },
            latest_release: None,
            authenticated: false,
            limitations: Vec::new(),
        };

        let output = evaluate_github_repository(
            &input,
            &plan,
            Some(&GitHubMetadataOutcome::Available(Box::new(metadata))),
        );
        assert_eq!(output.verdict, "low_value");
        assert!(output.next_actions.iter().any(|action| action["title"]
            .as_str()
            .is_some_and(|title| title.contains("alternative"))));
    }

    #[test]
    fn unavailable_metadata_falls_back_with_explicit_limitation() {
        let output = evaluate_github_repository(
            &fixture_input(),
            &fixture_plan(),
            Some(&GitHubMetadataOutcome::Unavailable {
                code: "github.rate_limited".to_string(),
            }),
        );
        assert!(!output.evidence.is_empty());
        assert!(output
            .limitations
            .iter()
            .any(|limitation| limitation.contains("rate-limited")));
    }

    fn fixture_input() -> EvaluationInput {
        EvaluationInput {
            object_id: "object-1".to_string(),
            user_id: "local".to_string(),
            object_type: "github_repo".to_string(),
            title: Some("Repository".to_string()),
            canonical_url: Some("https://github.com/owner/repo".to_string()),
            privacy_level: "personal".to_string(),
            parsed_document_id: "parsed-1".to_string(),
            text_content: "Install with cargo add repo. Usage and examples are documented."
                .to_string(),
            word_count: Some(10),
            content_hash: "content-hash".to_string(),
            latest_ai_summary: None,
        }
    }

    fn fixture_plan() -> EvaluationPlan {
        EvaluationPlan {
            schema_version: 1,
            evaluator_type: "github_repo_evaluator".to_string(),
            evaluator_version: "0.1.0".to_string(),
            steps: vec!["Inspect public metadata".to_string()],
            checks: vec!["Stars are contextual only".to_string()],
        }
    }
}

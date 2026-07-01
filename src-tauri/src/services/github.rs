use crate::storage::object_store::sha256_hex;
use reqwest::header::{HeaderMap, ACCEPT, AUTHORIZATION, USER_AGENT};
use reqwest::{Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const GITHUB_API_BASE_URL: &str = "https://api.github.com/";
const GITHUB_API_VERSION: &str = "2022-11-28";
const GITHUB_USER_AGENT: &str = "Link-World/0.1.0";
const GITHUB_REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_JSON_RESPONSE_BYTES: usize = 512 * 1024;
const MAX_README_BYTES: usize = 256 * 1024;

pub const GITHUB_AUTH_FAILED: &str = "github.auth_failed";
pub const GITHUB_FORBIDDEN: &str = "github.forbidden";
pub const GITHUB_INVALID_RESPONSE: &str = "github.invalid_response";
pub const GITHUB_INVALID_REPOSITORY: &str = "github.invalid_repository";
pub const GITHUB_NOT_FOUND_OR_PRIVATE: &str = "github.not_found_or_private";
pub const GITHUB_POLICY_DENIED: &str = "github.policy_denied";
pub const GITHUB_PRIVATE_REPOSITORY: &str = "github.private_repository";
pub const GITHUB_RATE_LIMITED: &str = "github.rate_limited";
pub const GITHUB_RESPONSE_TOO_LARGE: &str = "github.response_too_large";
pub const GITHUB_TIMEOUT: &str = "github.timeout";
pub const GITHUB_UNAVAILABLE: &str = "github.unavailable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepositoryRef {
    pub owner: String,
    pub repo: String,
}

impl GitHubRepositoryRef {
    pub fn from_github_url(raw_url: &str) -> Option<Self> {
        let url = Url::parse(raw_url).ok()?;
        let host = url.host_str()?.to_ascii_lowercase();
        if host != "github.com" && host != "www.github.com" {
            return None;
        }

        let mut segments = url.path_segments()?.filter(|segment| !segment.is_empty());
        let owner = segments.next()?.to_string();
        let repo = segments.next()?.trim_end_matches(".git").to_string();
        if !is_safe_repo_segment(&owner) || !is_safe_repo_segment(&repo) {
            return None;
        }

        Some(Self { owner, repo })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubReadmeSignals {
    pub available: bool,
    pub byte_length: usize,
    pub content_hash: Option<String>,
    pub has_installation: bool,
    pub has_usage: bool,
    pub has_examples: bool,
    pub has_security_policy: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubReleaseMetadata {
    pub tag_name: String,
    pub published_at: Option<String>,
    pub prerelease: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepositoryMetadata {
    pub owner: String,
    pub repo: String,
    pub description: Option<String>,
    pub default_branch: String,
    pub primary_language: Option<String>,
    pub topics: Vec<String>,
    pub stars: u64,
    pub forks: u64,
    pub open_issues: u64,
    pub archived: bool,
    pub disabled: bool,
    pub fork: bool,
    pub pushed_at: Option<String>,
    pub license_spdx_id: Option<String>,
    pub license_name: Option<String>,
    pub readme: GitHubReadmeSignals,
    pub latest_release: Option<GitHubReleaseMetadata>,
    pub authenticated: bool,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum GitHubMetadataOutcome {
    Available(Box<GitHubRepositoryMetadata>),
    Unavailable { code: String },
}

#[derive(Clone)]
pub struct GitHubMetadataClient {
    client: reqwest::Client,
    base_url: Url,
    token: Option<String>,
    request_timeout: Duration,
}

impl GitHubMetadataClient {
    pub fn public(token: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: Url::parse(GITHUB_API_BASE_URL).expect("static GitHub API URL must be valid"),
            token: normalize_token(token),
            request_timeout: GITHUB_REQUEST_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub fn for_test(base_url: &str, token: Option<String>, request_timeout: Duration) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: Url::parse(base_url).expect("test GitHub API URL must be valid"),
            token: normalize_token(token),
            request_timeout,
        }
    }

    pub async fn fetch_public_repository(
        &self,
        reference: &GitHubRepositoryRef,
    ) -> GitHubMetadataOutcome {
        let repository: RepositoryResponse = match self
            .get_json(
                &format!("repos/{}/{}", reference.owner, reference.repo),
                false,
            )
            .await
        {
            Ok(Some(repository)) => repository,
            Ok(None) => {
                return GitHubMetadataOutcome::Unavailable {
                    code: GITHUB_NOT_FOUND_OR_PRIVATE.to_string(),
                };
            }
            Err(code) => return GitHubMetadataOutcome::Unavailable { code },
        };
        if repository.private {
            return GitHubMetadataOutcome::Unavailable {
                code: GITHUB_PRIVATE_REPOSITORY.to_string(),
            };
        }

        let mut limitations = Vec::new();
        let (readme, stop_optional_requests) = match self
            .get_readme(&format!(
                "repos/{}/{}/readme",
                reference.owner, reference.repo
            ))
            .await
        {
            Ok(Some(readme)) => (readme, false),
            Ok(None) => (empty_readme_signals(), false),
            Err(code) => {
                let stop = should_stop_optional_requests(&code);
                limitations.push(code);
                (empty_readme_signals(), stop)
            }
        };
        let latest_release = if stop_optional_requests {
            None
        } else {
            match self
                .get_json::<ReleaseResponse>(
                    &format!(
                        "repos/{}/{}/releases/latest",
                        reference.owner, reference.repo
                    ),
                    true,
                )
                .await
            {
                Ok(release) => release.map(|release| GitHubReleaseMetadata {
                    tag_name: truncate_chars(&release.tag_name, 128),
                    published_at: release.published_at,
                    prerelease: release.prerelease,
                }),
                Err(code) => {
                    limitations.push(code);
                    None
                }
            }
        };

        GitHubMetadataOutcome::Available(Box::new(GitHubRepositoryMetadata {
            owner: reference.owner.clone(),
            repo: reference.repo.clone(),
            description: repository
                .description
                .map(|description| truncate_chars(&description, 512)),
            default_branch: truncate_chars(&repository.default_branch, 128),
            primary_language: repository
                .language
                .map(|language| truncate_chars(&language, 64)),
            topics: repository
                .topics
                .into_iter()
                .take(20)
                .map(|topic| truncate_chars(&topic, 64))
                .collect(),
            stars: repository.stargazers_count,
            forks: repository.forks_count,
            open_issues: repository.open_issues_count,
            archived: repository.archived,
            disabled: repository.disabled,
            fork: repository.fork,
            pushed_at: repository.pushed_at,
            license_spdx_id: repository
                .license
                .as_ref()
                .and_then(|license| license.spdx_id.as_ref())
                .map(|value| truncate_chars(value, 64)),
            license_name: repository
                .license
                .and_then(|license| license.name)
                .map(|value| truncate_chars(&value, 128)),
            readme,
            latest_release,
            authenticated: self.token.is_some(),
            limitations,
        }))
    }

    async fn get_json<T>(&self, path: &str, optional_not_found: bool) -> Result<Option<T>, String>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self.request(path, "application/vnd.github+json").await?;
        if response.status() == StatusCode::NOT_FOUND && optional_not_found {
            return Ok(None);
        }
        ensure_success(&response)?;
        let bytes = read_bounded(response, MAX_JSON_RESPONSE_BYTES).await?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| GITHUB_INVALID_RESPONSE.to_string())
    }

    async fn get_readme(&self, path: &str) -> Result<Option<GitHubReadmeSignals>, String> {
        let response = self
            .request(path, "application/vnd.github.raw+json")
            .await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        ensure_success(&response)?;
        let bytes = read_bounded(response, MAX_README_BYTES).await?;
        let lower = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
        Ok(Some(GitHubReadmeSignals {
            available: true,
            byte_length: bytes.len(),
            content_hash: Some(sha256_hex(&bytes)),
            has_installation: contains_any(
                &lower,
                &[
                    "install",
                    "npm add",
                    "npm install",
                    "cargo add",
                    "pip install",
                ],
            ),
            has_usage: contains_any(&lower, &["usage", "quickstart", "getting started"]),
            has_examples: contains_any(&lower, &["example", "examples", "demo"]),
            has_security_policy: contains_any(
                &lower,
                &["security policy", "security.md", "report a vulnerability"],
            ),
        }))
    }

    async fn request(&self, path: &str, accept: &'static str) -> Result<Response, String> {
        let url = self
            .base_url
            .join(path)
            .map_err(|_| GITHUB_INVALID_RESPONSE.to_string())?;
        let mut request = self
            .client
            .get(url)
            .header(ACCEPT, accept)
            .header(USER_AGENT, GITHUB_USER_AGENT)
            .header("x-github-api-version", GITHUB_API_VERSION)
            .timeout(self.request_timeout);
        if let Some(token) = &self.token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }

        request.send().await.map_err(|error| {
            if error.is_timeout() {
                GITHUB_TIMEOUT.to_string()
            } else {
                GITHUB_UNAVAILABLE.to_string()
            }
        })
    }
}

#[derive(Debug, Deserialize)]
struct RepositoryResponse {
    #[serde(default)]
    description: Option<String>,
    default_branch: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    topics: Vec<String>,
    #[serde(default)]
    stargazers_count: u64,
    #[serde(default)]
    forks_count: u64,
    #[serde(default)]
    open_issues_count: u64,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    disabled: bool,
    #[serde(default)]
    fork: bool,
    #[serde(default)]
    private: bool,
    #[serde(default)]
    pushed_at: Option<String>,
    #[serde(default)]
    license: Option<LicenseResponse>,
}

#[derive(Debug, Deserialize)]
struct LicenseResponse {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    spdx_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
}

fn ensure_success(response: &Response) -> Result<(), String> {
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    if status == StatusCode::NOT_FOUND {
        return Err(GITHUB_NOT_FOUND_OR_PRIVATE.to_string());
    }
    if status == StatusCode::UNAUTHORIZED {
        return Err(GITHUB_AUTH_FAILED.to_string());
    }
    if status == StatusCode::TOO_MANY_REQUESTS
        || (status == StatusCode::FORBIDDEN && is_rate_limited(response.headers()))
    {
        return Err(GITHUB_RATE_LIMITED.to_string());
    }
    if status == StatusCode::FORBIDDEN {
        return Err(GITHUB_FORBIDDEN.to_string());
    }
    Err(GITHUB_UNAVAILABLE.to_string())
}

fn should_stop_optional_requests(code: &str) -> bool {
    matches!(
        code,
        GITHUB_AUTH_FAILED
            | GITHUB_FORBIDDEN
            | GITHUB_RATE_LIMITED
            | GITHUB_TIMEOUT
            | GITHUB_UNAVAILABLE
    )
}

fn is_rate_limited(headers: &HeaderMap) -> bool {
    header_is_zero(headers, "x-ratelimit-remaining") || headers.contains_key("retry-after")
}

fn header_is_zero(headers: &HeaderMap, name: &'static str) -> bool {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "0")
}

async fn read_bounded(mut response: Response, max_bytes: usize) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > max_bytes as u64)
    {
        return Err(GITHUB_RESPONSE_TOO_LARGE.to_string());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| GITHUB_UNAVAILABLE.to_string())?
    {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(GITHUB_RESPONSE_TOO_LARGE.to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn empty_readme_signals() -> GitHubReadmeSignals {
    GitHubReadmeSignals {
        available: false,
        byte_length: 0,
        content_hash: None,
        has_installation: false,
        has_usage: false,
        has_examples: false,
        has_security_policy: false,
    }
}

fn normalize_token(token: Option<String>) -> Option<String> {
    token
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn is_safe_repo_segment(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        GitHubMetadataClient, GitHubMetadataOutcome, GitHubRepositoryRef,
        GITHUB_PRIVATE_REPOSITORY, GITHUB_RATE_LIMITED,
    };
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    type FixtureResponse = (
        &'static str,
        &'static str,
        &'static str,
        Vec<(&'static str, &'static str)>,
    );

    fn fixture_timeout() -> Duration {
        Duration::from_secs(5)
    }

    #[test]
    fn parses_only_safe_github_repository_urls() {
        assert_eq!(
            GitHubRepositoryRef::from_github_url("https://github.com/openai/codex/issues/1"),
            Some(GitHubRepositoryRef {
                owner: "openai".to_string(),
                repo: "codex".to_string(),
            })
        );
        assert!(GitHubRepositoryRef::from_github_url("https://example.com/openai/codex").is_none());
        assert!(GitHubRepositoryRef::from_github_url("https://github.com/openai/%2e%2e").is_none());
    }

    #[tokio::test]
    async fn collects_bounded_public_metadata_without_persisting_readme_body() {
        let repository = r#"{
            "description":"A useful public repository",
            "default_branch":"main",
            "language":"Rust",
            "topics":["ai","cli"],
            "stargazers_count":120,
            "forks_count":14,
            "open_issues_count":3,
            "archived":false,
            "disabled":false,
            "fork":false,
            "private":false,
            "pushed_at":"2026-06-30T12:00:00Z",
            "license":{"name":"MIT License","spdx_id":"MIT"}
        }"#;
        let readme = "# Demo\n## Install\ncargo add demo\n## Usage\nExample workflow";
        let release =
            r#"{"tag_name":"v1.2.3","published_at":"2026-06-29T12:00:00Z","prerelease":false}"#;
        let (base_url, requests) = start_fixture_server(vec![
            ("200 OK", "application/json", repository),
            ("200 OK", "text/plain", readme),
            ("200 OK", "application/json", release),
        ])
        .await;
        let client = GitHubMetadataClient::for_test(
            &base_url,
            Some("fixture-token".to_string()),
            fixture_timeout(),
        );
        let reference = GitHubRepositoryRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
        };

        let outcome = client.fetch_public_repository(&reference).await;
        let GitHubMetadataOutcome::Available(metadata) = outcome else {
            panic!("fixture metadata should be available");
        };
        assert_eq!(metadata.stars, 120);
        assert_eq!(metadata.license_spdx_id.as_deref(), Some("MIT"));
        assert!(metadata.readme.has_installation);
        assert!(metadata.readme.has_usage);
        assert!(metadata.readme.has_examples);
        assert!(metadata.readme.content_hash.is_some());
        assert_eq!(
            metadata
                .latest_release
                .as_ref()
                .map(|item| item.tag_name.as_str()),
            Some("v1.2.3")
        );
        assert!(metadata.authenticated);
        let requests = requests.lock().expect("request log should lock");
        assert_eq!(requests.len(), 3);
        assert!(requests[0].contains("GET /repos/owner/repo HTTP/1.1"));
        assert!(requests[1].contains("GET /repos/owner/repo/readme HTTP/1.1"));
        assert!(requests[2].contains("GET /repos/owner/repo/releases/latest HTTP/1.1"));
        assert!(requests.iter().all(|request| {
            let lower = request.to_ascii_lowercase();
            lower.contains("authorization: bearer fixture-token")
                && lower.contains("x-github-api-version: 2022-11-28")
        }));
        let serialized = serde_json::to_string(&metadata).expect("metadata should serialize");
        assert!(!serialized.contains("cargo add demo"));
    }

    #[tokio::test]
    async fn rejects_private_repository_without_requesting_optional_resources() {
        let repository = r#"{"default_branch":"main","private":true}"#;
        let (base_url, requests) =
            start_fixture_server(vec![("200 OK", "application/json", repository)]).await;
        let client = GitHubMetadataClient::for_test(&base_url, None, fixture_timeout());
        let reference = GitHubRepositoryRef {
            owner: "owner".to_string(),
            repo: "private-repo".to_string(),
        };

        let outcome = client.fetch_public_repository(&reference).await;
        assert!(matches!(
            outcome,
            GitHubMetadataOutcome::Unavailable { code } if code == GITHUB_PRIVATE_REPOSITORY
        ));
        let requests = requests.lock().expect("request log should lock");
        assert_eq!(requests.len(), 1);
        assert!(requests[0].contains("GET /repos/owner/private-repo HTTP/1.1"));
    }

    #[tokio::test]
    async fn classifies_rate_limit_without_retrying_or_reading_error_body() {
        let (base_url, requests) = start_fixture_server_with_headers(vec![(
            "403 Forbidden",
            "application/json",
            "{\"message\":\"secret fixture detail\"}",
            vec![("X-RateLimit-Remaining", "0")],
        )])
        .await;
        let client = GitHubMetadataClient::for_test(&base_url, None, fixture_timeout());
        let reference = GitHubRepositoryRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
        };

        let outcome = client.fetch_public_repository(&reference).await;
        assert!(matches!(
            outcome,
            GitHubMetadataOutcome::Unavailable { code } if code == GITHUB_RATE_LIMITED
        ));
        let requests = requests.lock().expect("request log should lock");
        assert_eq!(requests.len(), 1);
        assert!(!requests[0].to_ascii_lowercase().contains("authorization:"));
    }

    #[tokio::test]
    async fn stops_optional_requests_after_readme_rate_limit() {
        let repository = r#"{
            "default_branch":"main",
            "private":false
        }"#;
        let (base_url, requests) = start_fixture_server_with_headers(vec![
            ("200 OK", "application/json", repository, Vec::new()),
            (
                "403 Forbidden",
                "application/json",
                "{\"message\":\"rate limited\"}",
                vec![("Retry-After", "60")],
            ),
        ])
        .await;
        let client = GitHubMetadataClient::for_test(&base_url, None, fixture_timeout());
        let reference = GitHubRepositoryRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
        };

        let outcome = client.fetch_public_repository(&reference).await;
        let GitHubMetadataOutcome::Available(metadata) = outcome else {
            panic!("repository metadata should remain available");
        };
        assert_eq!(metadata.limitations, vec![GITHUB_RATE_LIMITED.to_string()]);
        let requests = requests.lock().expect("request log should lock");
        assert_eq!(requests.len(), 2);
        assert!(requests[1].contains("/repos/owner/repo/readme"));
        assert!(requests
            .iter()
            .all(|request| !request.contains("releases/latest")));
    }

    async fn start_fixture_server(
        responses: Vec<(&'static str, &'static str, &'static str)>,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
        start_fixture_server_with_headers(
            responses
                .into_iter()
                .map(|(status, content_type, body)| (status, content_type, body, Vec::new()))
                .collect(),
        )
        .await
    }

    async fn start_fixture_server_with_headers(
        responses: Vec<FixtureResponse>,
    ) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture server should bind");
        let address = listener
            .local_addr()
            .expect("fixture address should be readable");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = requests.clone();
        tokio::spawn(async move {
            for (status, content_type, body, headers) in responses {
                let (mut socket, _) = listener
                    .accept()
                    .await
                    .expect("fixture server should accept request");
                let mut buffer = vec![0_u8; 8192];
                let read = socket
                    .read(&mut buffer)
                    .await
                    .expect("fixture request should read");
                request_log
                    .lock()
                    .expect("request log should lock")
                    .push(String::from_utf8_lossy(&buffer[..read]).to_string());
                let extra_headers = headers
                    .into_iter()
                    .map(|(name, value)| format!("{name}: {value}\r\n"))
                    .collect::<String>();
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{extra_headers}Connection: close\r\n\r\n{body}",
                    body.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("fixture response should write");
            }
        });

        (format!("http://{address}/"), requests)
    }
}

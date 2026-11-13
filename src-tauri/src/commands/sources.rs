//! Platform source credentials and adapter probing.
//!
//! Cookies for the platform adapters live in the application secret store, so the
//! UI can report whether a platform is configured without ever reading the value
//! back into the renderer.

use crate::errors::{map_ipc_result, AppError, IpcResponse};
use crate::services::sources::{self, cookies::SourceCookieStore};
use crate::state::AppState;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcePlatformCredentialView {
    pub id: &'static str,
    pub label: &'static str,
    pub cookie_domain: Option<&'static str>,
    pub configured: bool,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCredentialListResponse {
    pub backend: &'static str,
    pub platforms: Vec<SourcePlatformCredentialView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProbeAssetView {
    pub kind: &'static str,
    pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProbeResponse {
    pub matched_adapter: Option<&'static str>,
    pub resolved: bool,
    /// `ok` | `login_required` | `structure_changed` | `no_adapter` | `failed`
    pub status: &'static str,
    pub title: Option<String>,
    pub author: Option<String>,
    pub text_length: usize,
    pub assets: Vec<SourceProbeAssetView>,
    pub excerpt: Option<String>,
    pub message: Option<String>,
}

fn cookie_store(state: &AppState) -> SourceCookieStore {
    SourceCookieStore::new(state.secrets().clone(), state.data_dir())
}

#[tauri::command]
pub async fn list_source_credentials(
    state: tauri::State<'_, AppState>,
) -> Result<IpcResponse<SourceCredentialListResponse>, String> {
    let store = cookie_store(state.inner());
    let result = async {
        let stored = store.list()?;

        let platforms = sources::platforms()
            .into_iter()
            .map(|platform| {
                let entry = platform
                    .cookie_domain
                    .and_then(|domain| stored.iter().find(|cookie| cookie.domain == domain));

                SourcePlatformCredentialView {
                    id: platform.id,
                    label: platform.label,
                    cookie_domain: platform.cookie_domain,
                    configured: entry.is_some(),
                    updated_at: entry.and_then(|cookie| cookie.updated_at.clone()),
                }
            })
            .collect();

        Ok(SourceCredentialListResponse {
            backend: store.backend_name(),
            platforms,
        })
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn save_source_credential(
    state: tauri::State<'_, AppState>,
    domain: String,
    cookie: String,
) -> Result<IpcResponse<SourcePlatformCredentialView>, String> {
    let store = cookie_store(state.inner());
    let result = async {
        let updated_at = chrono::Utc::now().to_rfc3339();
        store.set(&domain, &cookie, &updated_at)?;

        let platform = sources::platforms()
            .into_iter()
            .find(|platform| platform.cookie_domain == Some(domain.as_str()))
            .ok_or_else(|| {
                AppError::PolicyDenied("this domain has no source adapter".to_string())
            })?;

        Ok(SourcePlatformCredentialView {
            id: platform.id,
            label: platform.label,
            cookie_domain: platform.cookie_domain,
            configured: true,
            updated_at: Some(updated_at),
        })
    }
    .await;

    Ok(map_ipc_result(result))
}

#[tauri::command]
pub async fn delete_source_credential(
    state: tauri::State<'_, AppState>,
    domain: String,
) -> Result<IpcResponse<bool>, String> {
    let store = cookie_store(state.inner());
    let result = store.remove(&domain);
    Ok(map_ipc_result(result))
}

/// Run a URL through the adapters without writing anything, so the settings panel
/// can tell "cookie works" apart from "platform changed".
#[tauri::command]
pub async fn probe_source_url(
    state: tauri::State<'_, AppState>,
    url: String,
) -> Result<IpcResponse<SourceProbeResponse>, String> {
    let store = cookie_store(state.inner());

    let result = async {
        let trimmed = url.trim().to_string();
        let parsed = reqwest::Url::parse(&trimmed)
            .map_err(|_| AppError::PolicyDenied("URL is invalid".to_string()))?;

        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(AppError::PolicyDenied(
                "URL must use http or https".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent(format!(
                "NodeTide/{} source probe",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|error| AppError::Unknown(error.to_string()))?;

        let matched = sources::adapters()
            .into_iter()
            .find(|adapter| adapter.matches(&parsed))
            .map(|adapter| adapter.id());

        let Some(matched_adapter) = matched else {
            return Ok(SourceProbeResponse {
                matched_adapter: None,
                resolved: false,
                status: "no_adapter",
                title: None,
                author: None,
                text_length: 0,
                assets: Vec::new(),
                excerpt: None,
                message: Some("No source adapter handles this URL.".to_string()),
            });
        };

        match sources::capture_with_adapter(&client, &parsed, &store).await {
            Ok(Some(capture)) => Ok(SourceProbeResponse {
                matched_adapter: Some(matched_adapter),
                resolved: true,
                status: "ok",
                title: capture.title,
                author: capture.author,
                text_length: capture.text_content.chars().count(),
                assets: capture
                    .assets
                    .iter()
                    .map(|asset| SourceProbeAssetView {
                        kind: asset.kind.as_str(),
                        url: asset.url.clone(),
                    })
                    .collect(),
                excerpt: Some(capture.text_content.chars().take(200).collect()),
                message: None,
            }),
            Ok(None) => Ok(SourceProbeResponse {
                matched_adapter: Some(matched_adapter),
                resolved: false,
                status: "no_adapter",
                title: None,
                author: None,
                text_length: 0,
                assets: Vec::new(),
                excerpt: None,
                message: Some("No source adapter handles this URL.".to_string()),
            }),
            Err(error) => {
                let message = error.to_string();
                let status = if message.contains("cookie") || message.contains("login") {
                    "login_required"
                } else {
                    "structure_changed"
                };

                Ok(SourceProbeResponse {
                    matched_adapter: Some(matched_adapter),
                    resolved: false,
                    status,
                    title: None,
                    author: None,
                    text_length: 0,
                    assets: Vec::new(),
                    excerpt: None,
                    message: Some(message),
                })
            }
        }
    }
    .await;

    Ok(map_ipc_result(result))
}

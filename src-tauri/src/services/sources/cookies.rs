//! Per-domain cookies for the platform source adapters.
//!
//! Some platforms only answer a signed-in session (Xiaohongshu redirects
//! anonymous requests to `/login`, Zhihu returns 403). The adapters therefore need
//! the user's own cookie for those hosts.
//!
//! Cookies are stored through the application `SecretStore`, so Windows keeps them
//! in the Credential Manager next to model API keys and the development fallback
//! uses the same owner-only file. They are never rendered back to the UI, never
//! logged, and never included in support bundles.

use crate::errors::{AppError, AppResult};
use crate::state::SecretStore;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const LEGACY_FILE_NAME: &str = "source-cookies.json";
const SECRET_PREFIX: &str = "keyring:source-cookie:";
const INDEX_SECRET_REF: &str = "keyring:source-cookie:__domains";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCookie {
    pub domain: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SourceCookieStore {
    secrets: SecretStore,
    data_dir: PathBuf,
}

impl SourceCookieStore {
    pub fn new(secrets: SecretStore, data_dir: &Path) -> Self {
        Self {
            secrets,
            data_dir: data_dir.to_path_buf(),
        }
    }

    /// Domains that currently have a stored cookie, newest first.
    pub fn list(&self) -> AppResult<Vec<StoredCookie>> {
        let mut cookies: Vec<StoredCookie> = self
            .index()?
            .into_iter()
            .map(|(domain, updated_at)| StoredCookie {
                domain,
                updated_at: Some(updated_at).filter(|value| !value.is_empty()),
            })
            .collect();

        cookies.sort_by(|left, right| left.domain.cmp(&right.domain));
        Ok(cookies)
    }

    /// Cookie header value for a host, matching the host or any parent suffix.
    pub fn cookie_for(&self, url: &reqwest::Url) -> Option<String> {
        let host = url.host_str()?;
        let index = self.index().ok()?;

        let domain = index
            .keys()
            .filter(|domain| host_matches(host, domain))
            // Prefer the most specific domain (`www.xiaohongshu.com` over `xiaohongshu.com`).
            .max_by_key(|domain| domain.len())?;

        self.resolve(domain)
    }

    pub fn backend_name(&self) -> &'static str {
        self.secrets.backend_name()
    }

    pub fn has(&self, domain: &str) -> AppResult<bool> {
        let domain = normalize_domain(domain)?;
        Ok(self.index()?.contains_key(&domain))
    }

    pub fn resolve(&self, domain: &str) -> Option<String> {
        self.secrets
            .resolve(&format!("{SECRET_PREFIX}{domain}"))
            .ok()
            .flatten()
            .filter(|value| !value.trim().is_empty())
    }

    pub fn set(&self, domain: &str, cookie: &str, updated_at: &str) -> AppResult<()> {
        let domain = normalize_domain(domain)?;
        let cookie = cookie.trim();

        if cookie.is_empty() {
            return Err(AppError::PolicyDenied(
                "cookie value must not be empty".to_string(),
            ));
        }

        self.secrets
            .set(&format!("{SECRET_PREFIX}{domain}"), cookie.to_string())?;

        let mut index = self.index()?;
        index.insert(domain, updated_at.to_string());
        self.persist_index(&index)
    }

    pub fn remove(&self, domain: &str) -> AppResult<bool> {
        let domain = normalize_domain(domain)?;
        let mut index = self.index()?;

        if index.remove(&domain).is_none() {
            return Ok(false);
        }

        self.secrets.delete(&format!("{SECRET_PREFIX}{domain}"))?;
        self.persist_index(&index)?;
        Ok(true)
    }

    /// Move cookies written by the CLI-only implementation into the secret store.
    ///
    /// Returns how many domains were migrated. A missing file means "nothing to do".
    pub fn migrate_legacy_file(&self) -> AppResult<usize> {
        let path = self.data_dir.join(LEGACY_FILE_NAME);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(_) => return Ok(0),
        };

        let legacy: BTreeMap<String, String> = match serde_json::from_slice(&bytes) {
            Ok(legacy) => legacy,
            Err(_) => return Ok(0),
        };

        let mut index = self.index()?;
        let mut migrated = 0;

        for (domain, cookie) in legacy {
            let Ok(domain) = normalize_domain(&domain) else {
                continue;
            };

            if cookie.trim().is_empty() {
                continue;
            }

            if self
                .secrets
                .set(&format!("{SECRET_PREFIX}{domain}"), cookie)
                .is_err()
            {
                continue;
            }

            index.insert(domain, String::new());
            migrated += 1;
        }

        if migrated > 0 {
            self.persist_index(&index)?;
        }

        let _ = std::fs::remove_file(&path);
        Ok(migrated)
    }

    fn index(&self) -> AppResult<BTreeMap<String, String>> {
        let Some(raw) = self.secrets.resolve(INDEX_SECRET_REF)? else {
            return Ok(BTreeMap::new());
        };

        serde_json::from_str(&raw).map_err(|_| AppError::SecretStorage)
    }

    fn persist_index(&self, index: &BTreeMap<String, String>) -> AppResult<()> {
        let raw = serde_json::to_string(index).map_err(|_| AppError::SecretStorage)?;
        self.secrets.set(INDEX_SECRET_REF, raw)
    }
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn normalize_domain(domain: &str) -> AppResult<String> {
    let domain = domain.trim().trim_start_matches('.').to_lowercase();

    if domain.is_empty()
        || !domain
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '.'))
    {
        return Err(AppError::PolicyDenied(
            "cookie domain must be a plain host name".to_string(),
        ));
    }

    Ok(domain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("node-tide-cookies-{}", Uuid::new_v4()))
    }

    fn store(dir: &Path) -> SourceCookieStore {
        SourceCookieStore::new(SecretStore::system(dir).expect("secret store"), dir)
    }

    #[test]
    fn stores_lists_and_matches_by_host_suffix() {
        let dir = temp_dir();
        let store = store(&dir);
        store
            .set(
                "xiaohongshu.com",
                "a1=abc; web_session=def",
                "2026-11-13T00:00:00Z",
            )
            .expect("set");

        let exact = reqwest::Url::parse("https://xiaohongshu.com/explore/1").unwrap();
        let subdomain = reqwest::Url::parse("https://www.xiaohongshu.com/explore/1").unwrap();
        let other = reqwest::Url::parse("https://example.com/").unwrap();

        assert_eq!(
            store.cookie_for(&exact).as_deref(),
            Some("a1=abc; web_session=def")
        );
        assert_eq!(
            store.cookie_for(&subdomain).as_deref(),
            Some("a1=abc; web_session=def")
        );
        assert_eq!(store.cookie_for(&other), None);

        let listed = store.list().expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].domain, "xiaohongshu.com");
        assert_eq!(
            listed[0].updated_at.as_deref(),
            Some("2026-11-13T00:00:00Z")
        );

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prefers_the_most_specific_domain() {
        let dir = temp_dir();
        let store = store(&dir);
        store.set("xiaohongshu.com", "broad=1", "").expect("set");
        store
            .set("www.xiaohongshu.com", "narrow=2", "")
            .expect("set");

        let host = reqwest::Url::parse("https://www.xiaohongshu.com/explore/1").unwrap();
        assert_eq!(store.cookie_for(&host).as_deref(), Some("narrow=2"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn removes_and_reports_missing_domains() {
        let dir = temp_dir();
        let store = store(&dir);
        store.set("zhihu.com", "z_c0=token", "").expect("set");

        assert!(store.remove("zhihu.com").expect("remove"));
        assert!(!store.remove("zhihu.com").expect("remove again"));
        assert_eq!(store.resolve("zhihu.com"), None);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_empty_values_and_bad_domains() {
        let dir = temp_dir();
        let store = store(&dir);

        assert!(store.set("xiaohongshu.com", "   ", "").is_err());
        assert!(store.set("http://bad/", "a=1", "").is_err());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migrates_the_legacy_cookie_file() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(
            dir.join(LEGACY_FILE_NAME),
            br#"{"douyin.com":"sessionid=abc"}"#,
        )
        .expect("legacy file");

        let store = store(&dir);
        assert_eq!(store.migrate_legacy_file().expect("migrate"), 1);

        assert_eq!(
            store.resolve("douyin.com").as_deref(),
            Some("sessionid=abc")
        );
        assert!(
            !dir.join(LEGACY_FILE_NAME).exists(),
            "legacy file is removed"
        );

        let _ = std::fs::remove_dir_all(dir);
    }
}

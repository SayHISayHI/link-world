//! Per-domain cookie store for platform adapters.
//!
//! Some platforms only serve their payload to a signed-in browser (Xiaohongshu
//! redirects anonymous requests to `/login`, Zhihu answers 403). The adapter needs
//! the user's own cookie for those hosts.
//!
//! Cookies live in `<app data>/source-cookies.json` with `0600` permissions, are
//! only sent to the matching host, and never appear in logs, error messages or
//! support bundles.

use crate::errors::{AppError, AppResult};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "source-cookies.json";

#[derive(Debug, Clone)]
pub struct SourceCookieStore {
    path: PathBuf,
}

impl SourceCookieStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join(FILE_NAME),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> AppResult<BTreeMap<String, String>> {
        match std::fs::read(&self.path) {
            Ok(bytes) if bytes.is_empty() => Ok(BTreeMap::new()),
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
                AppError::PolicyDenied("source cookie file is not valid JSON".to_string())
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(_) => Err(AppError::PolicyDenied(
                "source cookie file could not be read".to_string(),
            )),
        }
    }

    /// Cookie header value for a host, matching the host or any parent suffix.
    pub fn cookie_for(&self, url: &reqwest::Url) -> Option<String> {
        let host = url.host_str()?;
        let cookies = self.load().ok()?;

        cookies
            .iter()
            .filter(|(domain, _)| host_matches(host, domain))
            // Prefer the most specific domain (`www.xiaohongshu.com` over `xiaohongshu.com`).
            .max_by_key(|(domain, _)| domain.len())
            .map(|(_, cookie)| cookie.clone())
    }

    pub fn set(&self, domain: &str, cookie: &str) -> AppResult<()> {
        let domain = normalize_domain(domain)?;
        let cookie = cookie.trim();
        if cookie.is_empty() {
            return Err(AppError::PolicyDenied(
                "cookie value must not be empty".to_string(),
            ));
        }

        let mut cookies = self.load()?;
        cookies.insert(domain, cookie.to_string());
        self.persist(&cookies)
    }

    pub fn remove(&self, domain: &str) -> AppResult<bool> {
        let domain = normalize_domain(domain)?;
        let mut cookies = self.load()?;
        let removed = cookies.remove(&domain).is_some();
        if removed {
            self.persist(&cookies)?;
        }
        Ok(removed)
    }

    fn persist(&self, cookies: &BTreeMap<String, String>) -> AppResult<()> {
        use std::io::Write;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| {
                AppError::PolicyDenied("source cookie directory could not be created".to_string())
            })?;
        }

        let payload = serde_json::to_vec(cookies).map_err(|_| {
            AppError::PolicyDenied("source cookies could not be encoded".to_string())
        })?;
        let temp_path = self.path.with_extension("json.tmp");

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        {
            let mut file = options.open(&temp_path).map_err(|_| {
                AppError::PolicyDenied("source cookie file could not be written".to_string())
            })?;
            file.write_all(&payload)
                .map_err(|_| AppError::PolicyDenied("source cookie write failed".to_string()))?;
            file.sync_all()
                .map_err(|_| AppError::PolicyDenied("source cookie sync failed".to_string()))?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&temp_path, permissions).map_err(|_| {
                AppError::PolicyDenied("source cookie permissions could not be set".to_string())
            })?;
        }

        std::fs::rename(&temp_path, &self.path).map_err(|_| {
            AppError::PolicyDenied("source cookie file could not be replaced".to_string())
        })?;

        Ok(())
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

    fn temp_store() -> (SourceCookieStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("node-tide-cookies-{}", Uuid::new_v4()));
        (SourceCookieStore::new(&dir), dir)
    }

    #[test]
    fn stores_and_matches_by_host_suffix() {
        let (store, dir) = temp_store();
        store
            .set("xiaohongshu.com", "a1=abc; web_session=def")
            .unwrap();

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

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prefers_the_most_specific_domain() {
        let (store, dir) = temp_store();
        store.set("xiaohongshu.com", "broad=1").unwrap();
        store.set("www.xiaohongshu.com", "narrow=2").unwrap();

        let host = reqwest::Url::parse("https://www.xiaohongshu.com/explore/1").unwrap();
        assert_eq!(store.cookie_for(&host).as_deref(), Some("narrow=2"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn keeps_the_file_owner_only() {
        let (store, dir) = temp_store();
        store.set("zhihu.com", "z_c0=token").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(store.path())
                .expect("cookie file")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_empty_values_and_bad_domains() {
        let (store, dir) = temp_store();

        assert!(store.set("xiaohongshu.com", "  ").is_err());
        assert!(store.set("http://bad/", "a=1").is_err());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn removes_and_reports_missing_domains() {
        let (store, dir) = temp_store();
        store.set("xiaohongshu.com", "a=1").unwrap();

        assert!(store.remove("xiaohongshu.com").unwrap());
        assert!(!store.remove("xiaohongshu.com").unwrap());

        let _ = std::fs::remove_dir_all(dir);
    }
}

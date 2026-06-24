use crate::domain::backup::{
    BackupFileEntry, BackupManifest, BackupSummary, BackupVerification,
    BACKUP_MANIFEST_SCHEMA_VERSION,
};
use crate::errors::{AppError, AppResult};
use crate::state::AppState;
use chrono::Utc;
use serde_json::from_slice;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Connection, SqliteConnection, SqlitePool};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const BACKUPS_DIR_NAME: &str = "backups";
pub(crate) const DATABASE_BACKUP_NAME: &str = "database.sqlite3";
pub(crate) const MANIFEST_FILE_NAME: &str = "manifest.json";
pub(crate) const MANIFEST_HASH_FILE_NAME: &str = "manifest.sha256";
pub(crate) const OBJECTS_BACKUP_DIR_NAME: &str = "objects";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_HASH_BYTES: u64 = 256;
const MAX_OBJECT_DIRECTORY_DEPTH: usize = 32;

#[derive(Debug, Clone)]
pub struct BackupService {
    pool: SqlitePool,
    object_root: PathBuf,
    backup_root: PathBuf,
    app_version: String,
}

impl BackupService {
    pub fn new(
        pool: SqlitePool,
        object_root: PathBuf,
        backup_root: PathBuf,
        app_version: String,
    ) -> Self {
        Self {
            pool,
            object_root,
            backup_root,
            app_version,
        }
    }

    pub fn from_state(state: &AppState) -> AppResult<Self> {
        let database = state.database()?;
        let data_dir = database.path().parent().ok_or_else(|| {
            AppError::Filesystem("database has no parent data directory".to_string())
        })?;

        Ok(Self::new(
            database.pool().clone(),
            state.object_store()?.root().to_path_buf(),
            data_dir.join(BACKUPS_DIR_NAME),
            state.backend_version().to_string(),
        ))
    }

    pub(crate) fn backup_root(&self) -> &Path {
        &self.backup_root
    }

    pub(crate) fn app_version(&self) -> &str {
        &self.app_version
    }

    pub async fn create_backup(&self) -> AppResult<BackupSummary> {
        let backup_id = Uuid::new_v4().to_string();
        let staging_dir = self.backup_root.join(format!(".staging-{backup_id}"));
        let final_dir = self.backup_root.join(&backup_id);
        let database_path = staging_dir.join(DATABASE_BACKUP_NAME);

        let backup_root = self.backup_root.clone();
        let staging_for_create = staging_dir.clone();
        tokio::task::spawn_blocking(move || -> AppResult<()> {
            fs::create_dir_all(backup_root)?;
            fs::create_dir(&staging_for_create)?;
            Ok(())
        })
        .await
        .map_err(|error| AppError::Filesystem(error.to_string()))??;

        let result = async {
            self.create_sqlite_snapshot(&database_path).await?;

            let object_root = self.object_root.clone();
            let staging_for_finalize = staging_dir.clone();
            let final_for_finalize = final_dir.clone();
            let backup_id_for_finalize = backup_id.clone();
            let app_version = self.app_version.clone();

            tokio::task::spawn_blocking(move || {
                finalize_backup(
                    &object_root,
                    &staging_for_finalize,
                    &final_for_finalize,
                    backup_id_for_finalize,
                    app_version,
                )
            })
            .await
            .map_err(|error| AppError::Filesystem(error.to_string()))?
        }
        .await;

        if result.is_err() {
            let staging_for_cleanup = staging_dir;
            let _ = tokio::task::spawn_blocking(move || {
                if staging_for_cleanup.exists() {
                    fs::remove_dir_all(staging_for_cleanup)
                } else {
                    Ok(())
                }
            })
            .await;
        }

        result
    }

    pub async fn list_backups(&self) -> AppResult<Vec<BackupSummary>> {
        let backup_root = self.backup_root.clone();
        tokio::task::spawn_blocking(move || list_backup_summaries(&backup_root))
            .await
            .map_err(|error| AppError::Filesystem(error.to_string()))?
    }

    pub async fn verify_backup(&self, backup_id: &str) -> AppResult<BackupVerification> {
        let backup_id = normalize_backup_id(backup_id)?;
        let backup_dir = self.backup_root.join(&backup_id);
        if !backup_dir.is_dir() {
            return Err(AppError::BackupInvalid("backup not found".to_string()));
        }
        verify_backup_directory(backup_dir, backup_id).await
    }

    async fn create_sqlite_snapshot(&self, destination: &Path) -> AppResult<()> {
        let destination = destination
            .to_str()
            .ok_or_else(|| AppError::Filesystem("backup path is not valid UTF-8".to_string()))?;

        sqlx::query("VACUUM INTO ?1")
            .bind(destination)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub(crate) async fn verify_backup_directory(
    backup_dir: PathBuf,
    backup_id: String,
) -> AppResult<BackupVerification> {
    let database_path = backup_dir.join(DATABASE_BACKUP_NAME);
    let backup_id_for_scan = backup_id.clone();
    let scan =
        tokio::task::spawn_blocking(move || verify_backup_files(&backup_dir, &backup_id_for_scan))
            .await
            .map_err(|error| AppError::Filesystem(error.to_string()))?;

    let mut verification = match scan {
        Ok(verification) => verification,
        Err(error) => BackupVerification {
            backup_id,
            valid: false,
            checked_file_count: 0,
            issues: vec![error.to_string()],
        },
    };

    if verification.issues.is_empty() {
        if let Err(issue) = verify_sqlite_snapshot(&database_path).await {
            verification.issues.push(issue);
        }
    }

    verification.valid = verification.issues.is_empty();
    Ok(verification)
}

fn finalize_backup(
    object_root: &Path,
    staging_dir: &Path,
    final_dir: &Path,
    backup_id: String,
    app_version: String,
) -> AppResult<BackupSummary> {
    let objects = copy_object_files(object_root, &staging_dir.join(OBJECTS_BACKUP_DIR_NAME))?;
    let database = hash_file_entry(
        &staging_dir.join(DATABASE_BACKUP_NAME),
        DATABASE_BACKUP_NAME.to_string(),
    )?;
    let total_size_bytes =
        database.size_bytes + objects.iter().map(|entry| entry.size_bytes).sum::<u64>();
    let manifest = BackupManifest {
        schema_version: BACKUP_MANIFEST_SCHEMA_VERSION,
        backup_id,
        app_version,
        created_at: Utc::now().to_rfc3339(),
        database,
        objects,
        total_size_bytes,
        contains_user_content: true,
        credentials_included: false,
    };

    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| AppError::BackupInvalid(error.to_string()))?;
    manifest_bytes.push(b'\n');
    write_new_file(&staging_dir.join(MANIFEST_FILE_NAME), &manifest_bytes)?;

    let manifest_hash = sha256_hex(&manifest_bytes);
    write_new_file(
        &staging_dir.join(MANIFEST_HASH_FILE_NAME),
        format!("{manifest_hash}\n").as_bytes(),
    )?;

    fs::rename(staging_dir, final_dir)?;
    Ok(BackupSummary::from(&manifest))
}

fn copy_object_files(
    source_root: &Path,
    destination_root: &Path,
) -> AppResult<Vec<BackupFileEntry>> {
    if !source_root.exists() {
        return Ok(Vec::new());
    }

    let mut source_files = Vec::new();
    collect_regular_files(source_root, source_root, &mut source_files, 0)?;
    source_files.sort();

    let mut entries = Vec::with_capacity(source_files.len());
    for source_path in source_files {
        let relative = source_path
            .strip_prefix(source_root)
            .map_err(|error| AppError::BackupInvalid(error.to_string()))?;
        let relative_string = safe_relative_path(relative)?;
        let destination = destination_root.join(relative);
        let manifest_path = format!("{OBJECTS_BACKUP_DIR_NAME}/{relative_string}");
        entries.push(copy_file_with_hash(
            &source_path,
            &destination,
            manifest_path,
        )?);
    }

    Ok(entries)
}

fn collect_regular_files(
    root: &Path,
    current: &Path,
    output: &mut Vec<PathBuf>,
    depth: usize,
) -> AppResult<()> {
    if depth > MAX_OBJECT_DIRECTORY_DEPTH {
        return Err(AppError::BackupInvalid(
            "object store directory depth exceeds backup limit".to_string(),
        ));
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();

        if is_link_like(&metadata) {
            return Err(AppError::BackupInvalid(format!(
                "object store contains a symbolic link: {}",
                display_relative(root, &path)
            )));
        }
        if file_type.is_dir() {
            collect_regular_files(root, &path, output, depth + 1)?;
        } else if file_type.is_file() {
            output.push(path);
        } else {
            return Err(AppError::BackupInvalid(format!(
                "object store contains an unsupported file type: {}",
                display_relative(root, &path)
            )));
        }
    }

    Ok(())
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }

    #[cfg(not(target_os = "windows"))]
    false
}

fn copy_file_with_hash(
    source: &Path,
    destination: &Path,
    relative_path: String,
) -> AppResult<BackupFileEntry> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut reader = File::open(source)?;
    let mut writer = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let (size_bytes, sha256) = copy_and_hash(&mut reader, &mut writer)?;
    writer.sync_all()?;

    Ok(BackupFileEntry {
        relative_path,
        size_bytes,
        sha256,
    })
}

fn hash_file_entry(path: &Path, relative_path: String) -> AppResult<BackupFileEntry> {
    let mut reader = File::open(path)?;
    let mut sink = std::io::sink();
    let (size_bytes, sha256) = copy_and_hash(&mut reader, &mut sink)?;

    Ok(BackupFileEntry {
        relative_path,
        size_bytes,
        sha256,
    })
}

fn copy_and_hash(reader: &mut impl Read, writer: &mut impl Write) -> AppResult<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size_bytes = 0_u64;

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        size_bytes += read as u64;
    }

    Ok((size_bytes, digest_to_hex(hasher.finalize())))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn list_backup_summaries(backup_root: &Path) -> AppResult<Vec<BackupSummary>> {
    fs::create_dir_all(backup_root)?;
    let mut summaries = Vec::new();

    for entry in fs::read_dir(backup_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(backup_id) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if backup_id.starts_with(".staging-") {
            continue;
        }

        match normalize_backup_id(&backup_id)
            .and_then(|_| read_checked_manifest(&entry.path()))
            .and_then(|manifest| validate_manifest_identity(manifest, &backup_id))
        {
            Ok(manifest) => summaries.push(BackupSummary::from(&manifest)),
            Err(_) => summaries.push(BackupSummary {
                backup_id,
                app_version: None,
                created_at: None,
                object_file_count: 0,
                total_size_bytes: 0,
                status: "invalid".to_string(),
            }),
        }
    }

    summaries.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.backup_id.cmp(&left.backup_id))
    });
    Ok(summaries)
}

fn verify_backup_files(
    backup_dir: &Path,
    expected_backup_id: &str,
) -> AppResult<BackupVerification> {
    let manifest =
        validate_manifest_identity(read_checked_manifest(backup_dir)?, expected_backup_id)?;
    let mut issues = Vec::new();
    let mut checked_file_count = 0_usize;

    if manifest.total_size_bytes
        != manifest.database.size_bytes
            + manifest
                .objects
                .iter()
                .map(|entry| entry.size_bytes)
                .sum::<u64>()
    {
        issues.push("manifest total size does not match its file entries".to_string());
    }
    if !manifest.contains_user_content {
        issues.push("manifest must disclose that it contains user content".to_string());
    }
    if manifest.credentials_included {
        issues.push("manifest unexpectedly reports included credentials".to_string());
    }

    let mut expected_object_paths = BTreeSet::new();
    verify_entry(
        backup_dir,
        &manifest.database,
        Some(DATABASE_BACKUP_NAME),
        &mut checked_file_count,
        &mut issues,
    );

    for entry in &manifest.objects {
        if !entry.relative_path.starts_with("objects/") {
            issues.push(format!(
                "object entry has invalid prefix: {}",
                entry.relative_path
            ));
            continue;
        }
        if !expected_object_paths.insert(entry.relative_path.clone()) {
            issues.push(format!("duplicate object entry: {}", entry.relative_path));
            continue;
        }
        verify_entry(
            backup_dir,
            entry,
            None,
            &mut checked_file_count,
            &mut issues,
        );
    }

    let actual_object_paths = collect_manifest_paths(
        &backup_dir.join(OBJECTS_BACKUP_DIR_NAME),
        OBJECTS_BACKUP_DIR_NAME,
    )?;
    for unexpected in actual_object_paths.difference(&expected_object_paths) {
        issues.push(format!("unexpected object file: {unexpected}"));
    }

    Ok(BackupVerification {
        backup_id: expected_backup_id.to_string(),
        valid: issues.is_empty(),
        checked_file_count,
        issues,
    })
}

fn verify_entry(
    backup_dir: &Path,
    entry: &BackupFileEntry,
    required_path: Option<&str>,
    checked_file_count: &mut usize,
    issues: &mut Vec<String>,
) {
    if required_path.is_some_and(|required| entry.relative_path != required) {
        issues.push(format!(
            "manifest database path must be {DATABASE_BACKUP_NAME}"
        ));
        return;
    }

    let relative_path = match parse_safe_relative_path(&entry.relative_path) {
        Ok(path) => path,
        Err(error) => {
            issues.push(error.to_string());
            return;
        }
    };
    let path = backup_dir.join(relative_path);
    *checked_file_count += 1;

    match hash_file_entry(&path, entry.relative_path.clone()) {
        Ok(actual) => {
            if actual.size_bytes != entry.size_bytes {
                issues.push(format!("file size mismatch: {}", entry.relative_path));
            }
            if actual.sha256 != entry.sha256 {
                issues.push(format!("file hash mismatch: {}", entry.relative_path));
            }
        }
        Err(error) => issues.push(format!("cannot read {}: {error}", entry.relative_path)),
    }
}

fn collect_manifest_paths(root: &Path, prefix: &str) -> AppResult<BTreeSet<String>> {
    if !root.exists() {
        return Ok(BTreeSet::new());
    }

    let mut files = Vec::new();
    collect_regular_files(root, root, &mut files, 0)?;
    let mut paths = BTreeSet::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .map_err(|error| AppError::BackupInvalid(error.to_string()))?;
        paths.insert(format!("{prefix}/{}", safe_relative_path(relative)?));
    }
    Ok(paths)
}

pub(crate) fn read_checked_manifest(backup_dir: &Path) -> AppResult<BackupManifest> {
    let manifest_path = backup_dir.join(MANIFEST_FILE_NAME);
    let hash_path = backup_dir.join(MANIFEST_HASH_FILE_NAME);
    if fs::metadata(&manifest_path)?.len() > MAX_MANIFEST_BYTES {
        return Err(AppError::BackupInvalid(
            "manifest exceeds size limit".to_string(),
        ));
    }
    if fs::metadata(&hash_path)?.len() > MAX_MANIFEST_HASH_BYTES {
        return Err(AppError::BackupInvalid(
            "manifest hash sidecar exceeds size limit".to_string(),
        ));
    }

    let bytes = fs::read(manifest_path)?;
    let expected_hash = fs::read_to_string(hash_path)?;
    if sha256_hex(&bytes) != expected_hash.trim() {
        return Err(AppError::BackupInvalid(
            "manifest hash does not match".to_string(),
        ));
    }

    from_slice(&bytes).map_err(|error| AppError::BackupInvalid(error.to_string()))
}

pub(crate) fn validate_manifest_identity(
    manifest: BackupManifest,
    expected_backup_id: &str,
) -> AppResult<BackupManifest> {
    if manifest.schema_version != BACKUP_MANIFEST_SCHEMA_VERSION {
        return Err(AppError::BackupInvalid(format!(
            "unsupported manifest schema version: {}",
            manifest.schema_version
        )));
    }
    if manifest.backup_id != expected_backup_id {
        return Err(AppError::BackupInvalid(
            "manifest backup id does not match its directory".to_string(),
        ));
    }

    Ok(manifest)
}

async fn verify_sqlite_snapshot(path: &Path) -> Result<(), String> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .create_if_missing(false);
    let mut connection = SqliteConnection::connect_with(&options)
        .await
        .map_err(|error| format!("cannot open backup database: {error}"))?;
    let result: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&mut connection)
        .await
        .map_err(|error| format!("database integrity check failed: {error}"))?;

    if result == "ok" {
        Ok(())
    } else {
        Err(format!("database integrity check returned: {result}"))
    }
}

pub(crate) fn normalize_backup_id(backup_id: &str) -> AppResult<String> {
    let backup_id = backup_id.trim();
    if backup_id.is_empty()
        || backup_id.len() > 128
        || !backup_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(AppError::BackupInvalid(
            "invalid backup identifier".to_string(),
        ));
    }

    Ok(backup_id.to_string())
}

pub(crate) fn parse_safe_relative_path(relative_path: &str) -> AppResult<PathBuf> {
    let path = Path::new(relative_path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(AppError::BackupInvalid(format!(
            "unsafe manifest path: {relative_path}"
        )));
    }
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(AppError::BackupInvalid(format!(
                "unsafe manifest path: {relative_path}"
            )));
        };
        let segment = segment.to_str().ok_or_else(|| {
            AppError::BackupInvalid(format!("manifest path is not UTF-8: {relative_path}"))
        })?;
        validate_path_segment(segment)?;
    }
    Ok(path.to_path_buf())
}

fn validate_path_segment(segment: &str) -> AppResult<()> {
    if segment.is_empty()
        || segment.contains(':')
        || segment.ends_with('.')
        || segment.ends_with(' ')
    {
        return Err(AppError::BackupInvalid(format!(
            "unsafe path segment: {segment}"
        )));
    }
    Ok(())
}

fn safe_relative_path(path: &Path) -> AppResult<String> {
    let mut segments = Vec::new();
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(AppError::BackupInvalid(
                "object path contains unsafe components".to_string(),
            ));
        };
        let segment = segment
            .to_str()
            .ok_or_else(|| AppError::BackupInvalid("object path is not valid UTF-8".to_string()))?;
        validate_path_segment(segment)?;
        segments.push(segment);
    }

    if segments.is_empty() {
        return Err(AppError::BackupInvalid("object path is empty".to_string()));
    }
    Ok(segments.join("/"))
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|relative| safe_relative_path(relative).ok())
        .unwrap_or_else(|| "<invalid-path>".to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest_to_hex(Sha256::digest(bytes))
}

fn digest_to_hex(digest: impl AsRef<[u8]>) -> String {
    let bytes = digest.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{parse_safe_relative_path, BackupService};
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn rejects_unsafe_manifest_paths() {
        assert!(parse_safe_relative_path("../outside").is_err());
        assert!(parse_safe_relative_path("objects/file.json:stream").is_err());
        assert!(parse_safe_relative_path("objects/trailing.").is_err());
        assert!(parse_safe_relative_path("objects/object-1/snapshot-1.json").is_ok());
    }

    #[tokio::test]
    async fn creates_lists_and_verifies_atomic_backup() {
        let data_root =
            std::env::temp_dir().join(format!("link-world-backup-test-{}", Uuid::new_v4()));
        let backup_root = data_root.join("backups");
        let database = Database::initialize(data_root.clone())
            .await
            .expect("database should initialize");
        let object_store =
            ObjectStore::initialize(data_root.clone()).expect("object store should initialize");
        object_store
            .write_capture_snapshot("object-1", "snapshot-1", br#"{"ok":true}"#.to_vec())
            .await
            .expect("object should write");

        let service = BackupService::new(
            database.pool().clone(),
            object_store.root().to_path_buf(),
            backup_root.clone(),
            "0.1.0-test".to_string(),
        );
        let summary = service.create_backup().await.expect("backup should create");

        assert_eq!(summary.status, "ready");
        assert_eq!(summary.object_file_count, 1);
        assert!(backup_root.join(&summary.backup_id).is_dir());
        assert!(!backup_root
            .join(format!(".staging-{}", summary.backup_id))
            .exists());

        let listed = service.list_backups().await.expect("backups should list");
        assert_eq!(listed, vec![summary.clone()]);

        let verification = service
            .verify_backup(&summary.backup_id)
            .await
            .expect("backup should verify");
        assert!(verification.valid, "{:?}", verification.issues);
        assert_eq!(verification.checked_file_count, 2);

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_root);
    }

    #[tokio::test]
    async fn detects_tampered_object_file() {
        let data_root =
            std::env::temp_dir().join(format!("link-world-backup-test-{}", Uuid::new_v4()));
        let backup_root = data_root.join("backups");
        let database = Database::initialize(data_root.clone())
            .await
            .expect("database should initialize");
        let object_store =
            ObjectStore::initialize(data_root.clone()).expect("object store should initialize");
        object_store
            .write_capture_snapshot("object-1", "snapshot-1", b"original".to_vec())
            .await
            .expect("object should write");

        let service = BackupService::new(
            database.pool().clone(),
            object_store.root().to_path_buf(),
            backup_root.clone(),
            "0.1.0-test".to_string(),
        );
        let summary = service.create_backup().await.expect("backup should create");
        fs::write(
            backup_root
                .join(&summary.backup_id)
                .join("objects")
                .join("object-1")
                .join("snapshot-1.json"),
            b"tampered",
        )
        .expect("fixture should tamper");

        let verification = service
            .verify_backup(&summary.backup_id)
            .await
            .expect("verification should return a result");
        assert!(!verification.valid);
        assert!(verification
            .issues
            .iter()
            .any(|issue| issue.contains("hash mismatch")));

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_root);
    }
    #[tokio::test]
    async fn removes_staging_directory_when_object_copy_fails() {
        let data_root =
            std::env::temp_dir().join(format!("link-world-backup-test-{}", Uuid::new_v4()));
        let backup_root = data_root.join("backups");
        let invalid_object_root = data_root.join("objects-as-file");
        let database = Database::initialize(data_root.clone())
            .await
            .expect("database should initialize");
        fs::write(&invalid_object_root, b"not a directory").expect("fixture should write");

        let service = BackupService::new(
            database.pool().clone(),
            invalid_object_root,
            backup_root.clone(),
            "0.1.0-test".to_string(),
        );
        assert!(service.create_backup().await.is_err());

        let entries = fs::read_dir(&backup_root)
            .expect("backup root should exist")
            .collect::<Result<Vec<_>, _>>()
            .expect("backup root should be readable");
        assert!(
            entries.is_empty(),
            "failed backup must leave no staging directory"
        );

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_root);
    }
}

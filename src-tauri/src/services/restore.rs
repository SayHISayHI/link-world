use crate::domain::backup::{BackupFileEntry, BackupManifest, RestorePreparation, RestoreStatus};
use crate::errors::{AppError, AppResult};
use crate::services::backup::{
    normalize_backup_id, parse_safe_relative_path, read_checked_manifest,
    validate_manifest_identity, verify_backup_directory, BackupService, DATABASE_BACKUP_NAME,
    MANIFEST_FILE_NAME, MANIFEST_HASH_FILE_NAME, OBJECTS_BACKUP_DIR_NAME,
};
use crate::state::AppState;
use crate::storage::database::{Database, DATABASE_FILE_NAME};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const RESTORE_DIR_NAME: &str = "restore";
const PREPARE_LOCK_NAME: &str = "prepare.lock";
const LAST_RESULT_NAME: &str = "last-result.json";
const RESTORE_CONTROL_SCHEMA_VERSION: i64 = 1;
const MAX_CONTROL_FILE_BYTES: u64 = 64 * 1024;
const PHASE_PREPARED: &str = "pending.prepared.json";
const PHASE_MOVING_LIVE: &str = "pending.moving-live.json";
const PHASE_LIVE_MOVED: &str = "pending.live-moved.json";
const PHASE_CANDIDATE_INSTALLED: &str = "pending.candidate-installed.json";
const PHASE_FILES: [&str; 4] = [
    PHASE_PREPARED,
    PHASE_MOVING_LIVE,
    PHASE_LIVE_MOVED,
    PHASE_CANDIDATE_INSTALLED,
];
const WINDOWS_FILE_OPERATION_RETRIES: usize = 20;
const WINDOWS_FILE_OPERATION_RETRY_DELAY: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RestorePhase {
    Prepared,
    MovingLive,
    LiveMoved,
    CandidateInstalled,
}

impl RestorePhase {
    fn file_name(self) -> &'static str {
        match self {
            Self::Prepared => PHASE_PREPARED,
            Self::MovingLive => PHASE_MOVING_LIVE,
            Self::LiveMoved => PHASE_LIVE_MOVED,
            Self::CandidateInstalled => PHASE_CANDIDATE_INSTALLED,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreMarker {
    schema_version: i64,
    transaction_id: String,
    backup_id: String,
    safety_backup_id: String,
    candidate_manifest_sha256: String,
    created_at: String,
}

#[derive(Debug)]
struct PrepareLock {
    path: PathBuf,
}

impl Drop for PrepareLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug, Clone)]
pub struct RestoreService {
    backup_service: BackupService,
    data_dir: PathBuf,
}

impl RestoreService {
    pub fn from_state(state: &AppState) -> AppResult<Self> {
        let backup_service = BackupService::from_state(state)?;
        let data_dir = state
            .database()?
            .path()
            .parent()
            .ok_or_else(|| {
                AppError::Filesystem("database has no parent data directory".to_string())
            })?
            .to_path_buf();
        Ok(Self::new(backup_service, data_dir))
    }

    pub fn new(backup_service: BackupService, data_dir: PathBuf) -> Self {
        Self {
            backup_service,
            data_dir,
        }
    }

    pub async fn prepare_restore(&self, backup_id: &str) -> AppResult<RestorePreparation> {
        let backup_id = normalize_backup_id(backup_id)?;
        let restore_root = self.data_dir.join(RESTORE_DIR_NAME);
        let _lock = acquire_prepare_lock(&restore_root)?;

        if find_pending_marker(&restore_root)?.is_some() {
            return Err(AppError::RestoreInvalid(
                "another restore is already pending".to_string(),
            ));
        }

        let verification = self.backup_service.verify_backup(&backup_id).await?;
        if !verification.valid {
            return Err(AppError::RestoreInvalid(format!(
                "backup verification failed: {}",
                verification.issues.join("; ")
            )));
        }

        let safety_backup = self.backup_service.create_backup().await?;
        let transaction_id = Uuid::new_v4().to_string();
        let candidate_dir = restore_root.join(candidate_dir_name(&transaction_id));
        let source_dir = self.backup_service.backup_root().join(&backup_id);
        let manifest = validate_manifest_identity(read_checked_manifest(&source_dir)?, &backup_id)?;

        let prepare_result: AppResult<RestorePreparation> = async {
            let source_for_stage = source_dir.clone();
            let candidate_for_stage = candidate_dir.clone();
            let manifest_for_stage = manifest.clone();
            tokio::task::spawn_blocking(move || {
                stage_candidate(&source_for_stage, &candidate_for_stage, &manifest_for_stage)
            })
            .await
            .map_err(|error| AppError::Filesystem(error.to_string()))??;

            Database::validate_restore_candidate(&candidate_dir.join(DATABASE_BACKUP_NAME)).await?;

            let candidate_manifest_sha256 = refresh_candidate_manifest(
                &candidate_dir,
                manifest,
                self.backup_service.app_version(),
            )?;

            let candidate_verification =
                verify_backup_directory(candidate_dir.clone(), backup_id.clone()).await?;
            if !candidate_verification.valid {
                return Err(AppError::RestoreInvalid(format!(
                    "prepared restore candidate is invalid: {}",
                    candidate_verification.issues.join("; ")
                )));
            }

            let marker = RestoreMarker {
                schema_version: RESTORE_CONTROL_SCHEMA_VERSION,
                transaction_id,
                backup_id: backup_id.clone(),
                safety_backup_id: safety_backup.backup_id.clone(),
                candidate_manifest_sha256,
                created_at: Utc::now().to_rfc3339(),
            };
            write_new_json(&restore_root.join(PHASE_PREPARED), &marker)?;

            Ok(RestorePreparation {
                backup_id,
                safety_backup_id: safety_backup.backup_id,
                restart_required: true,
            })
        }
        .await;

        if prepare_result.is_err() {
            let _ = fs::remove_dir_all(candidate_dir);
        }

        prepare_result
    }

    pub fn get_last_status(&self) -> AppResult<Option<RestoreStatus>> {
        read_last_status(&self.data_dir)
    }

    pub fn has_pending_restore(&self) -> AppResult<bool> {
        Ok(find_pending_marker(&self.data_dir.join(RESTORE_DIR_NAME))?.is_some())
    }
}

#[derive(Debug)]
pub struct RestoreTransaction {
    data_dir: PathBuf,
    marker: RestoreMarker,
}

impl RestoreTransaction {
    pub fn backup_id(&self) -> &str {
        &self.marker.backup_id
    }

    pub fn complete(self) -> AppResult<()> {
        let restore_root = self.data_dir.join(RESTORE_DIR_NAME);
        let rollback_dir = restore_root.join(rollback_dir_name(&self.marker.transaction_id));
        let candidate_dir = restore_root.join(candidate_dir_name(&self.marker.transaction_id));

        remove_dir_if_exists(&rollback_dir)?;
        remove_dir_if_exists(&candidate_dir)?;
        clear_phase_markers(&restore_root)?;

        let _ = write_last_status(
            &self.data_dir,
            RestoreStatus {
                backup_id: self.marker.backup_id,
                safety_backup_id: self.marker.safety_backup_id,
                status: "succeeded".to_string(),
                completed_at: Utc::now().to_rfc3339(),
                message: None,
            },
        );
        Ok(())
    }

    pub fn rollback(self, reason: &str) -> AppResult<()> {
        rollback_transaction(
            &self.data_dir,
            &self.marker,
            RestorePhase::CandidateInstalled,
        )?;
        let _ = write_last_status(
            &self.data_dir,
            RestoreStatus {
                backup_id: self.marker.backup_id,
                safety_backup_id: self.marker.safety_backup_id,
                status: "rolled_back".to_string(),
                completed_at: Utc::now().to_rfc3339(),
                message: Some(sanitize_status_message(&self.data_dir, reason)),
            },
        );
        Ok(())
    }
}

pub async fn begin_pending_restore(data_dir: &Path) -> AppResult<Option<RestoreTransaction>> {
    let restore_root = data_dir.join(RESTORE_DIR_NAME);
    remove_file_if_exists(&restore_root.join(PREPARE_LOCK_NAME))?;
    let Some((phase, marker)) = find_pending_marker(&restore_root)? else {
        return Ok(None);
    };
    validate_marker(&marker)?;

    match phase {
        RestorePhase::MovingLive | RestorePhase::LiveMoved => {
            rollback_transaction(data_dir, &marker, phase)?;
            let _ = write_last_status(
                data_dir,
                RestoreStatus {
                    backup_id: marker.backup_id,
                    safety_backup_id: marker.safety_backup_id,
                    status: "rolled_back".to_string(),
                    completed_at: Utc::now().to_rfc3339(),
                    message: Some(
                        "an interrupted restore was rolled back before startup".to_string(),
                    ),
                },
            );
            Ok(None)
        }
        RestorePhase::CandidateInstalled => Ok(Some(RestoreTransaction {
            data_dir: data_dir.to_path_buf(),
            marker,
        })),
        RestorePhase::Prepared => apply_prepared_restore(data_dir, marker).await,
    }
}

pub fn read_last_status(data_dir: &Path) -> AppResult<Option<RestoreStatus>> {
    let path = data_dir.join(RESTORE_DIR_NAME).join(LAST_RESULT_NAME);
    if !path.exists() {
        return Ok(None);
    }
    read_bounded_json(&path).map(Some)
}

pub fn has_pending_restore_in_dir(data_dir: &Path) -> AppResult<bool> {
    Ok(find_pending_marker(&data_dir.join(RESTORE_DIR_NAME))?.is_some())
}

async fn apply_prepared_restore(
    data_dir: &Path,
    marker: RestoreMarker,
) -> AppResult<Option<RestoreTransaction>> {
    let restore_root = data_dir.join(RESTORE_DIR_NAME);
    let candidate_dir = restore_root.join(candidate_dir_name(&marker.transaction_id));
    let candidate_hash = fs::read_to_string(candidate_dir.join(MANIFEST_HASH_FILE_NAME))?;
    if candidate_hash.trim() != marker.candidate_manifest_sha256 {
        abandon_prepared_restore(
            data_dir,
            &marker,
            "prepared candidate manifest no longer matches the pending restore",
        )?;
        return Ok(None);
    }

    let verification =
        verify_backup_directory(candidate_dir.clone(), marker.backup_id.clone()).await?;
    if !verification.valid {
        abandon_prepared_restore(
            data_dir,
            &marker,
            &format!(
                "prepared candidate failed startup verification: {}",
                verification.issues.join("; ")
            ),
        )?;
        return Ok(None);
    }

    transition_phase(
        &restore_root,
        RestorePhase::Prepared,
        RestorePhase::MovingLive,
    )?;
    let rollback_dir = restore_root.join(rollback_dir_name(&marker.transaction_id));
    fs::create_dir(&rollback_dir)?;

    let move_result = move_live_to_rollback(data_dir, &rollback_dir)
        .and_then(|_| {
            transition_phase(
                &restore_root,
                RestorePhase::MovingLive,
                RestorePhase::LiveMoved,
            )
        })
        .and_then(|_| install_candidate(data_dir, &candidate_dir))
        .and_then(|_| {
            transition_phase(
                &restore_root,
                RestorePhase::LiveMoved,
                RestorePhase::CandidateInstalled,
            )
        });

    if let Err(error) = move_result {
        let rollback_phase = find_pending_marker(&restore_root)?
            .map(|(phase, _)| phase)
            .unwrap_or(RestorePhase::MovingLive);
        rollback_transaction(data_dir, &marker, rollback_phase)?;
        let _ = write_last_status(
            data_dir,
            RestoreStatus {
                backup_id: marker.backup_id,
                safety_backup_id: marker.safety_backup_id,
                status: "rolled_back".to_string(),
                completed_at: Utc::now().to_rfc3339(),
                message: Some(sanitize_status_message(
                    data_dir,
                    &format!("restore switch failed and was rolled back: {error}"),
                )),
            },
        );
        return Ok(None);
    }

    Ok(Some(RestoreTransaction {
        data_dir: data_dir.to_path_buf(),
        marker,
    }))
}

fn stage_candidate(
    source_dir: &Path,
    candidate_dir: &Path,
    manifest: &BackupManifest,
) -> AppResult<()> {
    fs::create_dir_all(candidate_dir.join(OBJECTS_BACKUP_DIR_NAME))?;
    copy_manifest_entry(source_dir, candidate_dir, &manifest.database)?;
    for entry in &manifest.objects {
        copy_manifest_entry(source_dir, candidate_dir, entry)?;
    }
    Ok(())
}

fn copy_manifest_entry(
    source_root: &Path,
    destination_root: &Path,
    entry: &BackupFileEntry,
) -> AppResult<()> {
    let relative = parse_safe_relative_path(&entry.relative_path)?;
    let source = source_root.join(&relative);
    let destination = destination_root.join(&relative);
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

    if size_bytes != entry.size_bytes || sha256 != entry.sha256 {
        return Err(AppError::RestoreInvalid(format!(
            "backup changed while preparing restore: {}",
            entry.relative_path
        )));
    }
    Ok(())
}

fn refresh_candidate_manifest(
    candidate_dir: &Path,
    mut manifest: BackupManifest,
    app_version: &str,
) -> AppResult<String> {
    manifest.app_version = app_version.to_string();
    manifest.database = hash_file_entry(
        &candidate_dir.join(DATABASE_BACKUP_NAME),
        DATABASE_BACKUP_NAME.to_string(),
    )?;
    manifest.total_size_bytes = manifest.database.size_bytes
        + manifest
            .objects
            .iter()
            .map(|entry| entry.size_bytes)
            .sum::<u64>();

    let mut bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| AppError::RestoreInvalid(error.to_string()))?;
    bytes.push(b'\n');
    write_new_bytes(&candidate_dir.join(MANIFEST_FILE_NAME), &bytes)?;
    let manifest_hash = sha256_bytes(&bytes);
    write_new_bytes(
        &candidate_dir.join(MANIFEST_HASH_FILE_NAME),
        format!("{manifest_hash}\n").as_bytes(),
    )?;
    Ok(manifest_hash)
}

fn move_live_to_rollback(data_dir: &Path, rollback_dir: &Path) -> AppResult<()> {
    move_required(
        &data_dir.join(DATABASE_FILE_NAME),
        &rollback_dir.join(DATABASE_FILE_NAME),
    )?;
    for suffix in ["-wal", "-shm"] {
        move_optional(
            &data_dir.join(format!("{DATABASE_FILE_NAME}{suffix}")),
            &rollback_dir.join(format!("{DATABASE_FILE_NAME}{suffix}")),
        )?;
    }
    move_required(
        &data_dir.join(OBJECTS_BACKUP_DIR_NAME),
        &rollback_dir.join(OBJECTS_BACKUP_DIR_NAME),
    )?;
    Ok(())
}

fn install_candidate(data_dir: &Path, candidate_dir: &Path) -> AppResult<()> {
    move_required(
        &candidate_dir.join(DATABASE_BACKUP_NAME),
        &data_dir.join(DATABASE_FILE_NAME),
    )?;
    move_required(
        &candidate_dir.join(OBJECTS_BACKUP_DIR_NAME),
        &data_dir.join(OBJECTS_BACKUP_DIR_NAME),
    )?;
    Ok(())
}

fn rollback_transaction(
    data_dir: &Path,
    marker: &RestoreMarker,
    phase: RestorePhase,
) -> AppResult<()> {
    let restore_root = data_dir.join(RESTORE_DIR_NAME);
    let rollback_dir = restore_root.join(rollback_dir_name(&marker.transaction_id));
    let live_was_replaced = matches!(
        phase,
        RestorePhase::LiveMoved | RestorePhase::CandidateInstalled
    );

    restore_rollback_file(
        &rollback_dir.join(DATABASE_FILE_NAME),
        &data_dir.join(DATABASE_FILE_NAME),
        live_was_replaced,
        live_was_replaced,
    )?;
    for suffix in ["-wal", "-shm"] {
        restore_rollback_file(
            &rollback_dir.join(format!("{DATABASE_FILE_NAME}{suffix}")),
            &data_dir.join(format!("{DATABASE_FILE_NAME}{suffix}")),
            false,
            live_was_replaced,
        )?;
    }
    restore_rollback_directory(
        &rollback_dir.join(OBJECTS_BACKUP_DIR_NAME),
        &data_dir.join(OBJECTS_BACKUP_DIR_NAME),
        matches!(
            phase,
            RestorePhase::LiveMoved | RestorePhase::CandidateInstalled
        ),
    )?;

    remove_dir_if_exists(&rollback_dir)?;
    remove_dir_if_exists(&restore_root.join(candidate_dir_name(&marker.transaction_id)))?;
    clear_phase_markers(&restore_root)?;
    Ok(())
}

fn abandon_prepared_restore(
    data_dir: &Path,
    marker: &RestoreMarker,
    reason: &str,
) -> AppResult<()> {
    let restore_root = data_dir.join(RESTORE_DIR_NAME);
    remove_dir_if_exists(&restore_root.join(candidate_dir_name(&marker.transaction_id)))?;
    clear_phase_markers(&restore_root)?;
    let _ = write_last_status(
        data_dir,
        RestoreStatus {
            backup_id: marker.backup_id.clone(),
            safety_backup_id: marker.safety_backup_id.clone(),
            status: "failed".to_string(),
            completed_at: Utc::now().to_rfc3339(),
            message: Some(sanitize_status_message(data_dir, reason)),
        },
    );
    Ok(())
}

fn restore_rollback_file(
    rollback_path: &Path,
    live_path: &Path,
    required: bool,
    remove_live_when_missing: bool,
) -> AppResult<()> {
    if rollback_path.exists() {
        remove_file_if_exists(live_path)?;
        rename_path(rollback_path, live_path)?;
    } else if required {
        return Err(AppError::RestoreInvalid(
            "rollback payload is missing".to_string(),
        ));
    } else if remove_live_when_missing {
        remove_file_if_exists(live_path)?;
    }
    Ok(())
}

fn restore_rollback_directory(
    rollback_path: &Path,
    live_path: &Path,
    required: bool,
) -> AppResult<()> {
    if rollback_path.exists() {
        remove_dir_if_exists(live_path)?;
        rename_path(rollback_path, live_path)?;
    } else if required {
        return Err(AppError::RestoreInvalid(
            "rollback payload is missing".to_string(),
        ));
    }
    Ok(())
}

fn move_required(source: &Path, destination: &Path) -> AppResult<()> {
    if !source.exists() {
        return Err(AppError::RestoreInvalid(
            "required restore payload is missing".to_string(),
        ));
    }
    fs::rename(source, destination)?;
    Ok(())
}

fn move_optional(source: &Path, destination: &Path) -> AppResult<()> {
    if source.exists() {
        rename_path(source, destination)?;
    }
    Ok(())
}

fn transition_phase(restore_root: &Path, from: RestorePhase, to: RestorePhase) -> AppResult<()> {
    rename_path(
        restore_root.join(from.file_name()),
        restore_root.join(to.file_name()),
    )?;
    Ok(())
}

fn find_pending_marker(restore_root: &Path) -> AppResult<Option<(RestorePhase, RestoreMarker)>> {
    let candidates = [
        (RestorePhase::Prepared, PHASE_PREPARED),
        (RestorePhase::MovingLive, PHASE_MOVING_LIVE),
        (RestorePhase::LiveMoved, PHASE_LIVE_MOVED),
        (RestorePhase::CandidateInstalled, PHASE_CANDIDATE_INSTALLED),
    ];
    let mut found = Vec::new();
    for (phase, file_name) in candidates {
        let path = restore_root.join(file_name);
        if path.exists() {
            found.push((phase, path));
        }
    }

    if found.len() > 1 {
        return Err(AppError::RestoreInvalid(
            "multiple pending restore phase markers exist".to_string(),
        ));
    }
    let Some((phase, path)) = found.pop() else {
        return Ok(None);
    };
    Ok(Some((phase, read_bounded_json(&path)?)))
}

fn validate_marker(marker: &RestoreMarker) -> AppResult<()> {
    if marker.schema_version != RESTORE_CONTROL_SCHEMA_VERSION {
        return Err(AppError::RestoreInvalid(format!(
            "unsupported restore control schema version: {}",
            marker.schema_version
        )));
    }
    normalize_backup_id(&marker.transaction_id)?;
    normalize_backup_id(&marker.backup_id)?;
    normalize_backup_id(&marker.safety_backup_id)?;
    if marker.candidate_manifest_sha256.len() != 64
        || !marker
            .candidate_manifest_sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(AppError::RestoreInvalid(
            "invalid candidate manifest hash".to_string(),
        ));
    }
    Ok(())
}

fn acquire_prepare_lock(restore_root: &Path) -> AppResult<PrepareLock> {
    fs::create_dir_all(restore_root)?;
    let path = restore_root.join(PREPARE_LOCK_NAME);
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                AppError::RestoreInvalid("restore preparation is already running".to_string())
            } else {
                AppError::Filesystem(error.to_string())
            }
        })?;
    Ok(PrepareLock { path })
}

fn write_new_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| AppError::RestoreInvalid(error.to_string()))?;
    bytes.push(b'\n');
    write_new_bytes(path, &bytes)
}

fn write_new_bytes(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn read_bounded_json<T: for<'de> Deserialize<'de>>(path: &Path) -> AppResult<T> {
    if fs::metadata(path)?.len() > MAX_CONTROL_FILE_BYTES {
        return Err(AppError::RestoreInvalid(
            "restore control file exceeds size limit".to_string(),
        ));
    }
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|error| AppError::RestoreInvalid(error.to_string()))
}

fn sanitize_status_message(data_dir: &Path, message: &str) -> String {
    let data_dir = data_dir.to_string_lossy();
    let redacted = message.replace(data_dir.as_ref(), "<app-data>");
    let mut characters = redacted.chars();
    let mut output: String = characters.by_ref().take(512).collect();
    if characters.next().is_some() {
        output.push('…');
    }
    output
}

fn write_last_status(data_dir: &Path, status: RestoreStatus) -> AppResult<()> {
    let restore_root = data_dir.join(RESTORE_DIR_NAME);
    fs::create_dir_all(&restore_root)?;
    let path = restore_root.join(LAST_RESULT_NAME);
    let temporary = restore_root.join(format!(".last-result-{}.tmp", Uuid::new_v4()));
    write_new_json(&temporary, &status)?;
    remove_file_if_exists(&path)?;
    rename_path(&temporary, &path)?;
    Ok(())
}

fn clear_phase_markers(restore_root: &Path) -> AppResult<()> {
    for file_name in PHASE_FILES {
        remove_file_if_exists(&restore_root.join(file_name))?;
    }
    Ok(())
}

fn candidate_dir_name(transaction_id: &str) -> String {
    format!("candidate-{transaction_id}")
}

fn rollback_dir_name(transaction_id: &str) -> String {
    format!("rollback-{transaction_id}")
}

fn remove_file_if_exists(path: &Path) -> AppResult<()> {
    match retry_windows_file_operation(|| fs::remove_file(path)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn remove_dir_if_exists(path: &Path) -> AppResult<()> {
    match retry_windows_file_operation(|| fs::remove_dir_all(path)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn rename_path(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> AppResult<()> {
    retry_windows_file_operation(|| fs::rename(source.as_ref(), destination.as_ref()))?;
    Ok(())
}

fn retry_windows_file_operation(mut operation: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let mut attempts = 0;
    loop {
        match operation() {
            Ok(()) => return Ok(()),
            Err(error)
                if should_retry_file_operation(&error)
                    && attempts < WINDOWS_FILE_OPERATION_RETRIES =>
            {
                attempts += 1;
                std::thread::sleep(WINDOWS_FILE_OPERATION_RETRY_DELAY);
            }
            Err(error) => return Err(error),
        }
    }
}

fn should_retry_file_operation(error: &io::Error) -> bool {
    if !cfg!(windows) {
        return false;
    }

    matches!(error.raw_os_error(), Some(32 | 33))
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

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    digest_to_hex(hasher.finalize())
}

fn digest_to_hex(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        begin_pending_restore, candidate_dir_name, copy_manifest_entry, find_pending_marker,
        move_live_to_rollback, move_required, restore_rollback_file, rollback_dir_name,
        sanitize_status_message, transition_phase, RestorePhase, RestoreService,
    };
    use crate::domain::backup::BackupFileEntry;
    use crate::errors::AppError;
    use crate::services::backup::BackupService;
    use crate::storage::database::Database;
    use crate::storage::object_store::ObjectStore;
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    #[test]
    fn restore_status_redacts_app_data_paths_and_bounds_messages() {
        let data_dir = PathBuf::from("C:/Users/example/AppData/Link World");
        let message = format!("failed at {} {}", data_dir.display(), "x".repeat(600));
        let sanitized = sanitize_status_message(&data_dir, &message);
        assert!(!sanitized.contains("C:/Users/example"));
        assert!(sanitized.chars().count() <= 513);
    }

    #[test]
    fn candidate_copy_rechecks_hash_after_source_verification() {
        let root =
            std::env::temp_dir().join(format!("link-world-copy-race-test-{}", Uuid::new_v4()));
        let source_root = root.join("source");
        let destination_root = root.join("destination");
        fs::create_dir_all(source_root.join("objects")).expect("source directory should create");
        fs::create_dir_all(&destination_root).expect("destination directory should create");
        fs::write(source_root.join("objects").join("payload.bin"), b"changed")
            .expect("changed source should write");
        let entry = BackupFileEntry {
            relative_path: "objects/payload.bin".to_string(),
            size_bytes: 7,
            sha256: "0".repeat(64),
        };

        let error = copy_manifest_entry(&source_root, &destination_root, &entry)
            .expect_err("copy must reject a source changed after verification");
        assert!(matches!(error, AppError::RestoreInvalid(_)));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_live_move_preserves_an_unmoved_sqlite_sidecar() {
        let data_dir =
            std::env::temp_dir().join(format!("link-world-sidecar-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&data_dir).expect("test directory should create");
        let live_sidecar = data_dir.join("link-world.sqlite3-wal");
        let missing_rollback_sidecar = data_dir.join("rollback.sqlite3-wal");
        fs::write(&live_sidecar, b"old-wal").expect("sidecar fixture should write");

        restore_rollback_file(&missing_rollback_sidecar, &live_sidecar, false, false)
            .expect("moving-live recovery should preserve an unmoved sidecar");

        assert_eq!(
            fs::read(&live_sidecar).expect("sidecar should remain readable"),
            b"old-wal"
        );
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn startup_clears_a_stale_prepare_lock() {
        let data_dir =
            std::env::temp_dir().join(format!("link-world-lock-test-{}", Uuid::new_v4()));
        let restore_dir = data_dir.join("restore");
        fs::create_dir_all(&restore_dir).expect("restore directory should create");
        let lock = restore_dir.join("prepare.lock");
        fs::write(&lock, b"").expect("stale lock should write");

        assert!(begin_pending_restore(&data_dir)
            .await
            .expect("startup restore scan should succeed")
            .is_none());
        assert!(!lock.exists());
        let _ = fs::remove_dir_all(data_dir);
    }

    async fn fixture() -> (PathBuf, Database, ObjectStore, BackupService) {
        let data_dir =
            std::env::temp_dir().join(format!("link-world-restore-test-{}", Uuid::new_v4()));
        let database = Database::initialize(data_dir.clone())
            .await
            .expect("database should initialize");
        let object_store =
            ObjectStore::initialize(data_dir.clone()).expect("object store should initialize");
        let backup_service = BackupService::new(
            database.pool().clone(),
            object_store.root().to_path_buf(),
            data_dir.join("backups"),
            "0.1.0-test".to_string(),
        );
        (data_dir, database, object_store, backup_service)
    }

    async fn prepared_restore_fixture() -> (PathBuf, Database, RestoreService, String, String) {
        let (data_dir, database, object_store, backup_service) = fixture().await;
        sqlx::query("CREATE TABLE restore_probe(value TEXT NOT NULL)")
            .execute(database.pool())
            .await
            .expect("probe table should create");
        sqlx::query("INSERT INTO restore_probe(value) VALUES ('target')")
            .execute(database.pool())
            .await
            .expect("target probe should insert");
        object_store
            .write_capture_snapshot("object-1", "snapshot-1", b"target".to_vec())
            .await
            .expect("target object should write");
        let target = backup_service
            .create_backup()
            .await
            .expect("target backup should create");

        sqlx::query("UPDATE restore_probe SET value = 'current'")
            .execute(database.pool())
            .await
            .expect("current probe should update");
        fs::write(
            object_store.root().join("object-1").join("snapshot-1.json"),
            b"current",
        )
        .expect("current object should write");

        let restore_service = RestoreService::new(backup_service, data_dir.clone());
        let preparation = restore_service
            .prepare_restore(&target.backup_id)
            .await
            .expect("restore should prepare");
        assert!(preparation.restart_required);
        assert_ne!(preparation.backup_id, preparation.safety_backup_id);

        (
            data_dir,
            database,
            restore_service,
            preparation.backup_id,
            preparation.safety_backup_id,
        )
    }

    async fn assert_live_state(data_dir: &Path, expected: &str) {
        let database = Database::initialize(data_dir.to_path_buf())
            .await
            .expect("live database should initialize");
        let value: String = sqlx::query_scalar("SELECT value FROM restore_probe")
            .fetch_one(database.pool())
            .await
            .expect("probe should query");
        assert_eq!(value, expected);
        assert_eq!(
            fs::read(
                data_dir
                    .join("objects")
                    .join("object-1")
                    .join("snapshot-1.json")
            )
            .expect("live object should read"),
            expected.as_bytes()
        );
        database.pool().close().await;
    }

    #[tokio::test]
    async fn existing_pending_restore_rejects_second_prepare_without_new_safety_backup() {
        let (data_dir, database, restore_service, target_id, _safety_id) =
            prepared_restore_fixture().await;
        let backup_root = data_dir.join("backups");
        let before_count = fs::read_dir(&backup_root)
            .expect("backup root should read")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .count();

        let error = restore_service
            .prepare_restore(&target_id)
            .await
            .expect_err("second prepare must be rejected");
        assert!(matches!(error, AppError::RestoreInvalid(_)));

        let after_count = fs::read_dir(&backup_root)
            .expect("backup root should read")
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .count();
        assert_eq!(after_count, before_count);

        database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn moving_live_interruption_restores_only_payloads_already_moved() {
        let (data_dir, database, restore_service, _target_id, safety_id) =
            prepared_restore_fixture().await;
        database.pool().close().await;

        let restore_root = data_dir.join("restore");
        let (phase, marker) = find_pending_marker(&restore_root)
            .expect("pending marker should read")
            .expect("prepared marker should exist");
        assert_eq!(phase, RestorePhase::Prepared);
        transition_phase(
            &restore_root,
            RestorePhase::Prepared,
            RestorePhase::MovingLive,
        )
        .expect("phase should advance");
        let rollback_dir = restore_root.join(rollback_dir_name(&marker.transaction_id));
        fs::create_dir(&rollback_dir).expect("rollback directory should create");
        move_required(
            &data_dir.join("link-world.sqlite3"),
            &rollback_dir.join("link-world.sqlite3"),
        )
        .expect("database should move before simulated interruption");

        assert!(begin_pending_restore(&data_dir)
            .await
            .expect("moving-live recovery should succeed")
            .is_none());
        assert_live_state(&data_dir, "current").await;
        let status = restore_service
            .get_last_status()
            .expect("restore status should read")
            .expect("restore status should exist");
        assert_eq!(status.status, "rolled_back");
        assert!(data_dir.join("backups").join(safety_id).is_dir());
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn live_moved_interruption_removes_partial_candidate_and_restores_old_data() {
        let (data_dir, database, restore_service, _target_id, _safety_id) =
            prepared_restore_fixture().await;
        database.pool().close().await;

        let restore_root = data_dir.join("restore");
        let (_, marker) = find_pending_marker(&restore_root)
            .expect("pending marker should read")
            .expect("prepared marker should exist");
        transition_phase(
            &restore_root,
            RestorePhase::Prepared,
            RestorePhase::MovingLive,
        )
        .expect("phase should advance");
        let rollback_dir = restore_root.join(rollback_dir_name(&marker.transaction_id));
        fs::create_dir(&rollback_dir).expect("rollback directory should create");
        move_live_to_rollback(&data_dir, &rollback_dir).expect("all live payloads should move");
        transition_phase(
            &restore_root,
            RestorePhase::MovingLive,
            RestorePhase::LiveMoved,
        )
        .expect("phase should advance");
        let candidate_dir = restore_root.join(candidate_dir_name(&marker.transaction_id));
        move_required(
            &candidate_dir.join("database.sqlite3"),
            &data_dir.join("link-world.sqlite3"),
        )
        .expect("candidate database should install before simulated interruption");

        assert!(begin_pending_restore(&data_dir)
            .await
            .expect("live-moved recovery should succeed")
            .is_none());
        assert_live_state(&data_dir, "current").await;
        let status = restore_service
            .get_last_status()
            .expect("restore status should read")
            .expect("restore status should exist");
        assert_eq!(status.status, "rolled_back");
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn candidate_installed_interruption_resumes_validation_and_completion() {
        let (data_dir, database, restore_service, target_id, _safety_id) =
            prepared_restore_fixture().await;
        database.pool().close().await;

        let interrupted_transaction = begin_pending_restore(&data_dir)
            .await
            .expect("prepared restore should apply")
            .expect("restore transaction should exist");
        assert_eq!(interrupted_transaction.backup_id(), target_id);
        drop(interrupted_transaction);

        let resumed_transaction = begin_pending_restore(&data_dir)
            .await
            .expect("candidate-installed restore should resume")
            .expect("resumed transaction should exist");
        assert_eq!(resumed_transaction.backup_id(), target_id);
        assert_live_state(&data_dir, "target").await;
        resumed_transaction
            .complete()
            .expect("resumed restore should complete");

        let status = restore_service
            .get_last_status()
            .expect("restore status should read")
            .expect("restore status should exist");
        assert_eq!(status.status, "succeeded");
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn prepared_candidate_tampering_fails_without_touching_live_data() {
        let (data_dir, database, restore_service, _target_id, _safety_id) =
            prepared_restore_fixture().await;
        database.pool().close().await;

        let restore_root = data_dir.join("restore");
        let (_, marker) = find_pending_marker(&restore_root)
            .expect("pending marker should read")
            .expect("prepared marker should exist");
        fs::write(
            restore_root
                .join(candidate_dir_name(&marker.transaction_id))
                .join("objects")
                .join("object-1")
                .join("snapshot-1.json"),
            b"tampered",
        )
        .expect("candidate should be tampered");

        assert!(begin_pending_restore(&data_dir)
            .await
            .expect("invalid prepared candidate should be abandoned")
            .is_none());
        assert_live_state(&data_dir, "current").await;
        let status = restore_service
            .get_last_status()
            .expect("restore status should read")
            .expect("restore status should exist");
        assert_eq!(status.status, "failed");
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn missing_required_rollback_payload_fails_closed() {
        let (data_dir, database, _restore_service, _target_id, safety_id) =
            prepared_restore_fixture().await;
        database.pool().close().await;

        let restore_root = data_dir.join("restore");
        let (_, marker) = find_pending_marker(&restore_root)
            .expect("pending marker should read")
            .expect("prepared marker should exist");
        transition_phase(
            &restore_root,
            RestorePhase::Prepared,
            RestorePhase::MovingLive,
        )
        .expect("phase should advance");
        let rollback_dir = restore_root.join(rollback_dir_name(&marker.transaction_id));
        fs::create_dir(&rollback_dir).expect("rollback directory should create");
        move_live_to_rollback(&data_dir, &rollback_dir).expect("all live payloads should move");
        transition_phase(
            &restore_root,
            RestorePhase::MovingLive,
            RestorePhase::LiveMoved,
        )
        .expect("phase should advance");
        fs::remove_file(rollback_dir.join("link-world.sqlite3"))
            .expect("rollback database should be removed for fault injection");

        let error = begin_pending_restore(&data_dir)
            .await
            .expect_err("missing rollback payload must stop startup");
        assert!(matches!(error, AppError::RestoreInvalid(_)));
        assert!(data_dir.join("backups").join(safety_id).is_dir());
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn prepares_and_applies_restore_after_restart_boundary() {
        let (data_dir, database, object_store, backup_service) = fixture().await;
        sqlx::query("CREATE TABLE restore_probe(value TEXT NOT NULL)")
            .execute(database.pool())
            .await
            .expect("probe table should create");
        sqlx::query("INSERT INTO restore_probe(value) VALUES ('target')")
            .execute(database.pool())
            .await
            .expect("probe row should insert");
        object_store
            .write_capture_snapshot("object-1", "snapshot-1", b"target".to_vec())
            .await
            .expect("target object should write");
        let target = backup_service
            .create_backup()
            .await
            .expect("target backup should create");

        sqlx::query("UPDATE restore_probe SET value = 'current'")
            .execute(database.pool())
            .await
            .expect("probe row should mutate");
        fs::write(
            object_store.root().join("object-1").join("snapshot-1.json"),
            b"current",
        )
        .expect("object should mutate");

        let restore_service = RestoreService::new(backup_service, data_dir.clone());
        let preparation = restore_service
            .prepare_restore(&target.backup_id)
            .await
            .expect("restore should prepare");
        assert!(preparation.restart_required);
        assert_ne!(preparation.backup_id, preparation.safety_backup_id);

        database.pool().close().await;
        let transaction = begin_pending_restore(&data_dir)
            .await
            .expect("pending restore should start")
            .expect("restore transaction should exist");
        let restored_database = Database::initialize(data_dir.clone())
            .await
            .expect("restored database should initialize");
        restored_database
            .validate_integrity()
            .await
            .expect("restored database should pass integrity checks");

        let value: String = sqlx::query_scalar("SELECT value FROM restore_probe")
            .fetch_one(restored_database.pool())
            .await
            .expect("restored probe should be readable");
        assert_eq!(value, "target");
        assert_eq!(
            fs::read(
                data_dir
                    .join("objects")
                    .join("object-1")
                    .join("snapshot-1.json")
            )
            .expect("restored object should read"),
            b"target"
        );

        transaction.complete().expect("restore should complete");
        restored_database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn rolls_back_when_restored_database_cannot_initialize() {
        let (data_dir, database, _object_store, backup_service) = fixture().await;
        sqlx::query("CREATE TABLE restore_probe(value TEXT NOT NULL)")
            .execute(database.pool())
            .await
            .expect("probe table should create");
        sqlx::query("INSERT INTO restore_probe(value) VALUES ('target')")
            .execute(database.pool())
            .await
            .expect("probe row should insert");
        let target = backup_service
            .create_backup()
            .await
            .expect("target backup should create");
        sqlx::query("UPDATE restore_probe SET value = 'current'")
            .execute(database.pool())
            .await
            .expect("probe row should mutate");

        let restore_service = RestoreService::new(backup_service, data_dir.clone());
        restore_service
            .prepare_restore(&target.backup_id)
            .await
            .expect("restore should prepare");
        database.pool().close().await;

        let transaction = begin_pending_restore(&data_dir)
            .await
            .expect("pending restore should start")
            .expect("restore transaction should exist");
        fs::write(data_dir.join("link-world.sqlite3"), b"not sqlite")
            .expect("restored database should be corrupted");
        assert!(Database::initialize(data_dir.clone()).await.is_err());

        transaction
            .rollback("test injected database failure")
            .expect("restore should roll back");
        let current_database = Database::initialize(data_dir.clone())
            .await
            .expect("rollback database should initialize");
        let value: String = sqlx::query_scalar("SELECT value FROM restore_probe")
            .fetch_one(current_database.pool())
            .await
            .expect("rollback probe should be readable");
        assert_eq!(value, "current");

        let status = restore_service
            .get_last_status()
            .expect("status should read")
            .expect("status should exist");
        assert_eq!(status.status, "rolled_back");

        current_database.pool().close().await;
        let _ = fs::remove_dir_all(data_dir);
    }
}

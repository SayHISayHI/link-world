use crate::errors::{AppError, AppResult};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const OBJECTS_DIR_NAME: &str = "objects";

#[derive(Debug, Clone)]
pub struct ObjectStore {
    root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StoredObject {
    pub storage_uri: String,
    pub content_hash: String,
}

impl ObjectStore {
    pub fn initialize(data_dir: PathBuf) -> AppResult<Self> {
        let root = data_dir.join(OBJECTS_DIR_NAME);
        std::fs::create_dir_all(&root)?;

        Ok(Self { root })
    }

    pub async fn write_capture_snapshot(
        &self,
        object_id: &str,
        snapshot_id: &str,
        bytes: Vec<u8>,
    ) -> AppResult<StoredObject> {
        self.write_capture_artifact(object_id, snapshot_id, "json", bytes)
            .await
    }

    pub async fn write_capture_artifact(
        &self,
        object_id: &str,
        snapshot_id: &str,
        extension: &str,
        bytes: Vec<u8>,
    ) -> AppResult<StoredObject> {
        self.write_object_file(object_id, snapshot_id, extension, bytes)
            .await
    }

    pub async fn write_evaluation_artifact(
        &self,
        object_id: &str,
        evaluation_run_id: &str,
        artifact_id: &str,
        extension: &str,
        bytes: Vec<u8>,
    ) -> AppResult<StoredObject> {
        let extension = normalize_extension(extension)?;
        let object_id = normalize_storage_segment(object_id)?;
        let evaluation_run_id = normalize_storage_segment(evaluation_run_id)?;
        let artifact_id = normalize_storage_segment(artifact_id)?;
        let content_hash = sha256_hex(&bytes);
        let artifact_dir = self
            .root
            .join(&object_id)
            .join("evaluations")
            .join(&evaluation_run_id);
        let path = artifact_dir.join(format!("{artifact_id}.{extension}"));
        let storage_uri = format!(
            "local://objects/{object_id}/evaluations/{evaluation_run_id}/{artifact_id}.{extension}"
        );

        tokio::task::spawn_blocking(move || -> AppResult<()> {
            std::fs::create_dir_all(&artifact_dir)?;
            std::fs::write(path, bytes)?;
            Ok(())
        })
        .await
        .map_err(|error| AppError::Filesystem(error.to_string()))??;

        Ok(StoredObject {
            storage_uri,
            content_hash,
        })
    }

    async fn write_object_file(
        &self,
        object_id: &str,
        file_id: &str,
        extension: &str,
        bytes: Vec<u8>,
    ) -> AppResult<StoredObject> {
        let extension = normalize_extension(extension)?;
        let object_id = normalize_storage_segment(object_id)?;
        let file_id = normalize_storage_segment(file_id)?;
        let content_hash = sha256_hex(&bytes);
        let object_dir = self.root.join(&object_id);
        let path = object_dir.join(format!("{file_id}.{extension}"));
        let storage_uri = format!("local://objects/{object_id}/{file_id}.{extension}");

        tokio::task::spawn_blocking(move || -> AppResult<()> {
            std::fs::create_dir_all(&object_dir)?;
            std::fs::write(path, bytes)?;
            Ok(())
        })
        .await
        .map_err(|error| AppError::Filesystem(error.to_string()))??;

        Ok(StoredObject {
            storage_uri,
            content_hash,
        })
    }
}

fn normalize_extension(extension: &str) -> AppResult<String> {
    let trimmed = extension.trim().trim_start_matches('.');

    if trimmed.is_empty()
        || trimmed.len() > 16
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        return Err(AppError::Filesystem(format!(
            "invalid object store extension: {extension}"
        )));
    }

    Ok(trimmed.to_ascii_lowercase())
}

fn normalize_storage_segment(segment: &str) -> AppResult<String> {
    let trimmed = segment.trim();

    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Filesystem(format!(
            "invalid object store path segment: {segment}"
        )));
    }

    Ok(trimmed.to_string())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);

    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::{sha256_hex, ObjectStore};

    #[tokio::test]
    async fn writes_capture_snapshot_and_returns_stable_local_uri() {
        let root = std::env::temp_dir().join(format!("link-world-test-{}", uuid::Uuid::new_v4()));
        let store = ObjectStore::initialize(root).expect("object store should initialize");

        let stored = store
            .write_capture_snapshot("object-1", "snapshot-1", b"{\"ok\":true}".to_vec())
            .await
            .expect("snapshot should be written");

        assert_eq!(
            stored.storage_uri,
            "local://objects/object-1/snapshot-1.json"
        );
        assert_eq!(stored.content_hash, sha256_hex(b"{\"ok\":true}"));
    }

    #[tokio::test]
    async fn writes_evaluation_artifact_under_run_directory() {
        let root = std::env::temp_dir().join(format!("link-world-test-{}", uuid::Uuid::new_v4()));
        let store = ObjectStore::initialize(root).expect("object store should initialize");

        let stored = store
            .write_evaluation_artifact(
                "object-1",
                "run-1",
                "artifact-1",
                "json",
                b"{\"score\":0.8}".to_vec(),
            )
            .await
            .expect("artifact should be written");

        assert_eq!(
            stored.storage_uri,
            "local://objects/object-1/evaluations/run-1/artifact-1.json"
        );
        assert_eq!(stored.content_hash, sha256_hex(b"{\"score\":0.8}"));
    }
}

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
        let content_hash = sha256_hex(&bytes);
        let object_dir = self.root.join(object_id);
        let path = object_dir.join(format!("{snapshot_id}.json"));
        let storage_uri = format!("local://objects/{object_id}/{snapshot_id}.json");

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

        assert_eq!(stored.storage_uri, "local://objects/object-1/snapshot-1.json");
        assert_eq!(stored.content_hash, sha256_hex(b"{\"ok\":true}"));
    }
}

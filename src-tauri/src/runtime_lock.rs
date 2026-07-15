use crate::errors::{AppError, AppResult};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

const RUNTIME_DIR_NAME: &str = "runtime";
// Legacy storage ABI shared with pre-rename desktop and CLI builds.
const RUNTIME_LOCK_FILE_NAME: &str = "link-world.lock";

#[derive(Debug)]
pub struct RuntimeLock {
    _file: File,
    path: PathBuf,
}

impl RuntimeLock {
    pub fn acquire(data_dir: impl AsRef<Path>) -> AppResult<Self> {
        let runtime_dir = data_dir.as_ref().join(RUNTIME_DIR_NAME);
        fs::create_dir_all(&runtime_dir)?;
        let path = runtime_dir.join(RUNTIME_LOCK_FILE_NAME);
        let file = open_exclusive(&path)?;
        Ok(Self { _file: file, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(windows)]
fn open_exclusive(path: &Path) -> AppResult<File> {
    use std::os::windows::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(path)
        .map_err(|error| {
            if matches!(error.raw_os_error(), Some(32 | 33))
                || matches!(error.kind(), std::io::ErrorKind::PermissionDenied)
            {
                AppError::RuntimeBusy
            } else {
                AppError::Filesystem("runtime lock could not be opened".to_string())
            }
        })
}

#[cfg(not(windows))]
fn open_exclusive(path: &Path) -> AppResult<File> {
    use fs2::FileExt;

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| AppError::Filesystem("runtime lock could not be opened".to_string()))?;

    file.try_lock_exclusive().map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            AppError::RuntimeBusy
        } else {
            AppError::Filesystem("runtime lock could not be acquired".to_string())
        }
    })?;

    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::RuntimeLock;
    use crate::errors::AppError;
    use uuid::Uuid;

    #[test]
    fn rejects_a_second_runtime_for_the_same_data_directory() {
        let data_dir =
            std::env::temp_dir().join(format!("node-tide-runtime-lock-{}", Uuid::new_v4()));
        let first = RuntimeLock::acquire(&data_dir).expect("first runtime should acquire lock");
        let error = RuntimeLock::acquire(&data_dir).expect_err("second runtime should be rejected");

        assert!(matches!(error, AppError::RuntimeBusy));
        assert!(first.path().ends_with("link-world.lock"));

        drop(first);
        RuntimeLock::acquire(&data_dir).expect("released runtime should be reacquired");
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

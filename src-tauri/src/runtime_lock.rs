use crate::errors::{AppError, AppResult};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

const RUNTIME_DIR_NAME: &str = "runtime";
use crate::legacy_abi::RUNTIME_LOCK_FILE_NAME;

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

#[cfg(unix)]
fn open_exclusive(path: &Path) -> AppResult<File> {
    use std::os::unix::io::AsRawFd;

    // Node Tide ships on Windows. This fallback keeps single-process behavior for
    // development builds on other targets, but is not a release gate. `flock` is
    // released by the kernel when the holder exits, so a killed dev run cannot
    // leave a stale lock file that blocks the next launch.
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| AppError::Filesystem("runtime lock could not be opened".to_string()))?;

    // SAFETY: the file descriptor stays open for the lifetime of `file`, and
    // `flock` only touches the kernel lock table for that descriptor.
    let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if locked == 0 {
        return Ok(file);
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
        return Err(AppError::RuntimeBusy);
    }

    Err(AppError::Filesystem(
        "runtime lock could not be opened".to_string(),
    ))
}

#[cfg(all(not(windows), not(unix)))]
fn open_exclusive(path: &Path) -> AppResult<File> {
    let _ = path;
    Err(AppError::Filesystem(
        "runtime lock is not implemented for this target".to_string(),
    ))
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

    #[cfg(unix)]
    #[test]
    fn stale_lock_file_from_a_killed_process_does_not_block_startup() {
        let data_dir =
            std::env::temp_dir().join(format!("node-tide-runtime-stale-{}", Uuid::new_v4()));
        let runtime_dir = data_dir.join("runtime");
        std::fs::create_dir_all(&runtime_dir).expect("runtime directory should be created");
        std::fs::write(runtime_dir.join("link-world.lock"), b"")
            .expect("stale lock file should be written");

        let lock = RuntimeLock::acquire(&data_dir)
            .expect("a stale lock file left by a killed process must not block startup");
        drop(lock);

        let _ = std::fs::remove_dir_all(data_dir);
    }
}

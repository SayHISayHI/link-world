//! App data directory resolution shared with the desktop app.

use super::*;

pub(super) fn resolve_data_dir() -> Result<PathBuf, CliError> {
    #[cfg(debug_assertions)]
    if let Some(path) =
        env::var_os("NODE_TIDE_DATA_DIR").or_else(|| env::var_os("LINK_WORLD_DATA_DIR"))
    {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(CliError::invalid(
                "NODE_TIDE_DATA_DIR must be an absolute path in development builds",
            ));
        }
        return Ok(path);
    }

    #[cfg(target_os = "windows")]
    {
        env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_DATA_DIR_IDENTIFIER))
            .ok_or_else(|| CliError::invalid("APPDATA is unavailable"))
    }

    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| {
                path.join(format!(
                    "Library/Application Support/{APP_DATA_DIR_IDENTIFIER}"
                ))
            })
            .ok_or_else(|| CliError::invalid("HOME is unavailable"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(path) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(path).join(APP_DATA_DIR_IDENTIFIER));
        }
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(format!(".local/share/{APP_DATA_DIR_IDENTIFIER}")))
            .ok_or_else(|| CliError::invalid("HOME is unavailable"))
    }
}

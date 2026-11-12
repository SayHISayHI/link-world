//! Frozen storage ABI from the former Link World product identity.
//!
//! These strings are already on disk for anyone who ever ran a build, so they
//! must not be renamed without an explicit data migration. ADR-0010 renamed the
//! product to Node Tide but deliberately kept the on-disk identifiers.
//!
//! Changing any constant below is a **breaking data change**, not a rename:
//!
//! - `DATABASE_FILE_NAME` and `RUNTIME_LOCK_FILE_NAME` live inside the app data
//!   directory. A new name means an empty library for existing users.
//! - `APP_DATA_DIR_IDENTIFIER` comes from `tauri.conf.json.identifier`. Changing
//!   it moves the whole data directory, so it needs a migration that copies or
//!   links the old directory.
//! - `LOG_FILE_NAME` / `ROTATED_LOG_FILE_NAME` continue the same bounded log
//!   stream that support bundles and diagnostics already read.
//! - `KEYRING_SERVICE` is the OS credential-manager service name holding model
//!   provider API keys.
//!
//! Procedure for a real rename: ship a migration that reads the legacy names,
//! writes the new ones, verifies both, then keeps the legacy names readable for
//! at least one release. See `docs/storage_abi.md`.

/// SQLite database file name inside the app data directory.
pub const DATABASE_FILE_NAME: &str = "link-world.sqlite3";

/// Bounded structured log file name inside `<app data>/logs`.
pub const LOG_FILE_NAME: &str = "link-world.jsonl";

/// First rotated log file name, preserving the legacy suffix shape.
pub const ROTATED_LOG_FILE_NAME: &str = "link-world.jsonl.1";

/// Single-process runtime lock file name inside `<app data>/runtime`.
pub const RUNTIME_LOCK_FILE_NAME: &str = "link-world.lock";

/// Tauri bundle identifier, which also decides the app data directory name.
pub const APP_DATA_DIR_IDENTIFIER: &str = "com.linkworld.app";

/// OS credential-manager service name for model provider API keys.
pub const KEYRING_SERVICE: &str = "com.linkworld.app.model-provider";

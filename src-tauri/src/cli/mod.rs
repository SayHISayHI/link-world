//! Command-line interface.
//!
//! The CLI shares the desktop services through `AppState`; it never talks to the
//! database directly. Modules are split by concern: `args` (clap surface),
//! `commands` (execution), `output` (rendering), `error` (stable codes),
//! `redact` (path redaction), `validate` (argument checks) and `app_data`
//! (directory resolution shared with the desktop app).

mod app_data;
mod args;
mod commands;
mod error;
mod output;
mod redact;
mod validate;

use app_data::*;
use args::*;
use commands::*;
use error::*;
use output::*;
use redact::*;
use validate::*;

use crate::domain::capture::{PermissionContext, RawCaptureItem};
use crate::domain::knowledge::DeleteObjectMode;
use crate::domain::portable_export::PortableExportFormat;
use crate::errors::AppError;
use crate::runtime_lock::RuntimeLock;
use crate::services::ai::AIEnrichmentService;
use crate::services::backup::{BackupCatalog, BackupService, BACKUPS_DIR_NAME};
use crate::services::capture::CaptureService;
use crate::services::evaluation::EvaluationService;
use crate::services::library::LibraryService;
use crate::services::operations::OperationsService;
use crate::services::portable_export::PortableExportService;
use crate::services::search::SearchService;
use crate::services::support_bundle::SupportBundleService;
use crate::services::system::SystemService;
use crate::state::AppState;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::legacy_abi::APP_DATA_DIR_IDENTIFIER;
use uuid::Uuid;

const CLI_SCHEMA_VERSION: u32 = 1;
const DEFAULT_LIMIT: i64 = 30;
const MAX_LIMIT: i64 = 200;

pub async fn run() -> i32 {
    let wants_json = env::args_os().any(|argument| argument == "--output=json")
        || env::args_os()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|arguments| arguments[0] == "--output" && arguments[1] == "json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code();
            if wants_json && exit_code != 0 {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "schemaVersion": CLI_SCHEMA_VERSION,
                        "ok": false,
                        "command": "parse",
                        "error": {
                            "code": "ERR_INVALID_ARGUMENT",
                            "message": "Command arguments are invalid. Run with --help for usage.",
                            "correlationId": Value::Null,
                            "retryable": false,
                        }
                    }))
                    .expect("JSON parse error should serialize")
                );
                return 2;
            }
            let _ = error.print();
            return exit_code;
        }
    };

    if let Commands::Completion { shell } = cli.command {
        let mut command = Cli::command();
        generate(shell, &mut command, "node-tide-cli", &mut io::stdout());
        return 0;
    }

    if matches!(cli.command, Commands::Version) {
        let result = CliSuccess {
            command: "version",
            data: json!({
                "cliVersion": env!("CARGO_PKG_VERSION"),
                "backendVersion": env!("CARGO_PKG_VERSION"),
                "schemaVersion": CLI_SCHEMA_VERSION,
            }),
        };
        return write_success(&cli, result);
    }

    if let Commands::Sources(args) = &cli.command {
        return match execute_sources_probe(args).await {
            Ok(result) => write_success(&cli, result),
            Err(error) => write_error(&cli, error),
        };
    }

    let data_dir = match resolve_data_dir() {
        Ok(path) => path,
        Err(error) => return write_error(&cli, error),
    };
    let state = match AppState::initialize_from_data_dir(data_dir.clone()).await {
        Ok(state) => state,
        Err(error) => {
            let error = CliError::from_app(error);
            if matches!(
                cli.command,
                Commands::Backup(BackupArgs {
                    command: BackupCommands::List | BackupCommands::Verify { .. }
                })
            ) {
                return match execute_recovery_backup(&cli, &data_dir).await {
                    Ok(result) => write_success(&cli, result),
                    Err(error) => write_error(&cli, error),
                };
            }
            return write_error(&cli, error);
        }
    };

    match execute(&cli, &state, &data_dir).await {
        Ok(result) => write_success(&cli, result),
        Err(error) => write_error(&cli, error),
    }
}

#[cfg(test)]
mod tests;

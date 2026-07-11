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
use uuid::Uuid;

const CLI_SCHEMA_VERSION: u32 = 1;
const DEFAULT_LIMIT: i64 = 30;
const MAX_LIMIT: i64 = 200;

#[derive(Debug, Parser)]
#[command(
    name = "node-tide-cli",
    version,
    about = "Automation interface for the Node Tide local application",
    after_help = "Concurrency limitation: the desktop app and CLI cannot use the same Node Tide data directory at the same time. If another process owns it, the command exits with code 5 and ERR_RUNTIME_BUSY.",
    arg_required_else_help = true
)]
pub struct Cli {
    /// Output format for command results.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    output: OutputFormat,

    /// Suppress non-essential progress and privacy notices.
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print CLI and backend version information without opening local data.
    Version,
    /// Check whether the local runtime can initialize safely.
    Status,
    /// Read diagnostics or export a privacy-bounded support bundle.
    Diagnostics(DiagnosticsArgs),
    /// List, inspect, or delete knowledge objects.
    Object(ObjectArgs),
    /// Search the local knowledge library.
    Search(SearchArgs),
    /// Capture content into the local library.
    Capture(CaptureArgs),
    /// Run AI analysis for an object.
    Analysis(AnalysisArgs),
    /// Run and inspect evaluators.
    Evaluation(EvaluationArgs),
    /// Inspect or retry durable background jobs.
    Job(JobArgs),
    /// Inspect and maintain the full-text search index.
    SearchIndex(SearchIndexArgs),
    /// Export portable library data.
    Export(ExportArgs),
    /// Create and verify local backups.
    Backup(BackupArgs),
    /// Generate a shell completion script without opening local data.
    Completion { shell: Shell },
}

#[derive(Debug, Args)]
struct DiagnosticsArgs {
    #[command(subcommand)]
    command: DiagnosticsCommands,
}

#[derive(Debug, Subcommand)]
enum DiagnosticsCommands {
    /// Show a redacted local health snapshot.
    Show,
    /// Export a redacted support bundle to the application data directory.
    Export {
        /// Confirm creation of the support bundle.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Args)]
struct ObjectArgs {
    #[command(subcommand)]
    command: ObjectCommands,
}

#[derive(Debug, Subcommand)]
enum ObjectCommands {
    /// List recent objects with bounded pagination.
    List {
        /// Filter by object type or lifecycle navigation value.
        #[arg(long = "type")]
        object_type: Option<String>,
        /// Maximum objects to return (1-200).
        #[arg(long, default_value_t = DEFAULT_LIMIT, value_parser = clap::value_parser!(i64).range(1..=MAX_LIMIT))]
        limit: i64,
        /// Opaque cursor returned by the previous page.
        #[arg(long)]
        cursor: Option<String>,
    },
    /// Show object metadata; content requires an explicit privacy upgrade.
    Show {
        /// Stable internal object identifier.
        object_id: String,
        /// Explicitly include parsed content, analyses, evidence and evaluations.
        #[arg(long)]
        include_content: bool,
    },
    /// Delete an object using the existing lifecycle and cleanup rules.
    Delete {
        /// Stable internal object identifier.
        object_id: String,
        /// Deletion lifecycle mode.
        #[arg(long, value_enum, default_value_t = DeleteModeArg::Soft)]
        mode: DeleteModeArg,
        /// Skip the interactive destructive confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DeleteModeArg {
    Soft,
    Purge,
    ExportThenDelete,
}

impl From<DeleteModeArg> for DeleteObjectMode {
    fn from(value: DeleteModeArg) -> Self {
        match value {
            DeleteModeArg::Soft => Self::SoftDelete,
            DeleteModeArg::Purge => Self::Purge,
            DeleteModeArg::ExportThenDelete => Self::ExportThenDelete,
        }
    }
}

#[derive(Debug, Args)]
struct SearchArgs {
    /// Full-text query.
    query: String,
    /// Filter by object type or lifecycle navigation value.
    #[arg(long = "type")]
    object_type: Option<String>,
    /// Maximum results to return (1-200).
    #[arg(long, default_value_t = DEFAULT_LIMIT, value_parser = clap::value_parser!(i64).range(1..=MAX_LIMIT))]
    limit: i64,
}

#[derive(Debug, Args)]
struct CaptureArgs {
    #[command(subcommand)]
    command: CaptureCommands,
}

#[derive(Debug, Subcommand)]
enum CaptureCommands {
    /// Capture and parse a user-confirmed URL.
    Url {
        /// HTTP(S) URL explicitly submitted by the user.
        url: String,
        /// Privacy classification stored with the captured object.
        #[arg(long, value_enum, default_value_t = PrivacyArg::Personal)]
        privacy: PrivacyArg,
        /// Stable UUID used for operation correlation and idempotency.
        #[arg(long)]
        request_id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PrivacyArg {
    Public,
    Personal,
    Sensitive,
    Secret,
}

impl PrivacyArg {
    fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Personal => "personal",
            Self::Sensitive => "sensitive",
            Self::Secret => "secret",
        }
    }
}

#[derive(Debug, Args)]
struct AnalysisArgs {
    #[command(subcommand)]
    command: AnalysisCommands,
}

#[derive(Debug, Subcommand)]
enum AnalysisCommands {
    /// Run model enrichment to a durable terminal state.
    Run {
        /// Stable internal object identifier.
        object_id: String,
        /// Optional idempotency/correlation UUID.
        #[arg(long)]
        request_id: Option<String>,
    },
}

#[derive(Debug, Args)]
struct EvaluationArgs {
    #[command(subcommand)]
    command: EvaluationCommands,
}

#[derive(Debug, Subcommand)]
enum EvaluationCommands {
    /// List installed evaluator capabilities.
    List,
    /// Run an evaluator to a durable terminal state.
    Run {
        /// Stable internal object identifier.
        object_id: String,
        /// Evaluator type or `auto`.
        evaluator: String,
        /// Optional idempotency/correlation UUID.
        #[arg(long)]
        request_id: Option<String>,
    },
    /// Show a persisted evaluation run.
    Show {
        /// Stable evaluation run identifier.
        run_id: String,
    },
    /// Retry a failed evaluation without overwriting its history.
    Retry {
        /// Failed parent evaluation run identifier.
        run_id: String,
        /// New idempotency/correlation UUID for the retry operation.
        #[arg(long)]
        request_id: Option<String>,
    },
}

#[derive(Debug, Args)]
struct JobArgs {
    #[command(subcommand)]
    command: JobCommands,
}

#[derive(Debug, Subcommand)]
enum JobCommands {
    /// Show a durable background job.
    Show {
        /// Stable job identifier.
        job_id: String,
    },
    /// Retry a supported failed/cancelled/blocked job to terminal state.
    Retry {
        /// Stable job identifier.
        job_id: String,
    },
}

#[derive(Debug, Args)]
struct SearchIndexArgs {
    #[command(subcommand)]
    command: SearchIndexCommands,
}

#[derive(Debug, Subcommand)]
enum SearchIndexCommands {
    /// Check missing, stale, orphaned and duplicate FTS rows.
    Check,
    /// Rebuild the full-text index atomically to terminal state.
    Rebuild,
    /// Show a persisted rebuild job status.
    Status {
        /// Search rebuild job identifier.
        job_id: String,
    },
    /// Cancel a cancellable persisted rebuild job.
    Cancel {
        /// Search rebuild job identifier.
        job_id: String,
    },
    /// Reindex one object from its source-of-truth parsed document.
    Reindex {
        /// Stable internal object identifier.
        object_id: String,
    },
}

#[derive(Debug, Args)]
struct ExportArgs {
    #[command(subcommand)]
    command: ExportCommands,
}

#[derive(Debug, Subcommand)]
enum ExportCommands {
    /// Export non-secret objects to a private app-data directory.
    Library {
        /// Payload formats; manifest/checksum metadata remains JSON.
        #[arg(long, value_enum, default_value_t = ExportFormatArg::Both)]
        format: ExportFormatArg,
        /// Confirm that exported files may contain user content.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExportFormatArg {
    Json,
    Markdown,
    Both,
}

impl From<ExportFormatArg> for PortableExportFormat {
    fn from(value: ExportFormatArg) -> Self {
        match value {
            ExportFormatArg::Json => Self::Json,
            ExportFormatArg::Markdown => Self::Markdown,
            ExportFormatArg::Both => Self::Both,
        }
    }
}

#[derive(Debug, Args)]
struct BackupArgs {
    #[command(subcommand)]
    command: BackupCommands,
}

#[derive(Debug, Subcommand)]
enum BackupCommands {
    /// Create and verify an atomic local restore point.
    Create,
    /// List local restore points, including in startup recovery mode.
    List,
    /// Verify manifest, hashes and SQLite integrity for a restore point.
    Verify {
        /// Stable backup identifier from `backup list`.
        backup_id: String,
    },
}

#[derive(Debug)]
struct CliSuccess {
    command: &'static str,
    data: Value,
}

#[derive(Debug)]
struct CliError {
    code: String,
    message: String,
    retryable: bool,
    exit_code: i32,
    correlation_id: Option<String>,
    operation: Option<Value>,
}

impl CliError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "ERR_INVALID_ARGUMENT".to_string(),
            message: message.into(),
            retryable: false,
            exit_code: 2,
            correlation_id: None,
            operation: None,
        }
    }

    fn from_app(error: AppError) -> Self {
        let (code, message, retryable, exit_code) = match error {
            AppError::ObjectNotFound => ("ERR_OBJECT_NOT_FOUND", "Object was not found.", false, 3),
            AppError::JobNotFound => ("ERR_JOB_NOT_FOUND", "Job was not found.", false, 3),
            AppError::RuntimeBusy => (
                "ERR_RUNTIME_BUSY",
                "Another Node Tide process is using the local data directory.",
                true,
                5,
            ),
            AppError::PolicyDenied(_) => (
                "ERR_POLICY_DENIED",
                "The operation was denied by local policy.",
                false,
                4,
            ),
            AppError::PluginPermission(_) => (
                "ERR_PLUGIN_PERMISSION",
                "The operation was denied by plugin policy.",
                false,
                4,
            ),
            AppError::NetworkTimeout => (
                "ERR_NETWORK_TIMEOUT",
                "The network operation timed out.",
                true,
                6,
            ),
            AppError::ModelAuth => ("ERR_MODEL_AUTH", "Model authentication failed.", false, 6),
            AppError::ModelRateLimit => (
                "ERR_MODEL_RATE_LIMIT",
                "The model provider rate limit was reached.",
                true,
                6,
            ),
            AppError::ModelNotFound => (
                "ERR_MODEL_NOT_FOUND",
                "The model or provider endpoint was not found.",
                false,
                6,
            ),
            AppError::ModelOutputSchema(_) => (
                "ERR_MODEL_OUTPUT_SCHEMA",
                "The model returned an invalid structured result.",
                true,
                6,
            ),
            AppError::SecretStorage => (
                "ERR_SECRET_STORAGE",
                "The system credential store is unavailable.",
                false,
                7,
            ),
            AppError::BackupInvalid(_) => (
                "ERR_BACKUP_INVALID",
                "The backup is invalid or incomplete.",
                false,
                7,
            ),
            AppError::RestoreInvalid(_) => (
                "ERR_RESTORE_INVALID",
                "Restore state is invalid or incomplete.",
                false,
                7,
            ),
            AppError::DbMigration(_) => (
                "ERR_DB_MIGRATION",
                "The local database requires migration recovery.",
                false,
                7,
            ),
            AppError::DbConstraint => (
                "ERR_DB_CONSTRAINT",
                "The operation conflicts with existing local data.",
                false,
                7,
            ),
            AppError::Database(_) => (
                "ERR_DATABASE",
                "The local database operation failed.",
                true,
                7,
            ),
            AppError::Filesystem(_) => (
                "ERR_FILESYSTEM",
                "The local filesystem operation failed.",
                true,
                7,
            ),
            AppError::ParseFailed(_) => (
                "ERR_PARSE_FAILED",
                "The submitted content could not be parsed.",
                false,
                2,
            ),
            AppError::Unknown(_) => (
                "ERR_UNKNOWN",
                "An internal Node Tide operation failed.",
                false,
                10,
            ),
        };
        Self {
            code: code.to_string(),
            message: message.to_string(),
            retryable,
            exit_code,
            correlation_id: None,
            operation: None,
        }
    }

    fn terminal(
        code: String,
        message: &'static str,
        retryable: bool,
        exit_code: i32,
        correlation_id: Option<String>,
        operation: Value,
    ) -> Self {
        Self {
            code,
            message: message.to_string(),
            retryable,
            exit_code,
            correlation_id,
            operation: Some(operation),
        }
    }
}

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

async fn execute_recovery_backup(cli: &Cli, data_dir: &Path) -> Result<CliSuccess, CliError> {
    let _runtime_lock = RuntimeLock::acquire(data_dir).map_err(CliError::from_app)?;
    let catalog = BackupCatalog::new(data_dir.join(BACKUPS_DIR_NAME));
    match &cli.command {
        Commands::Backup(BackupArgs {
            command: BackupCommands::List,
        }) => {
            let result = catalog.list_backups().await.map_err(CliError::from_app)?;
            value_success("backup.list", &result, data_dir)
        }
        Commands::Backup(BackupArgs {
            command: BackupCommands::Verify { backup_id },
        }) => {
            validate_identifier(backup_id, "backup ID")?;
            let result = catalog
                .verify_backup(backup_id)
                .await
                .map_err(CliError::from_app)?;
            value_success("backup.verify", &result, data_dir)
        }
        _ => Err(CliError::invalid(
            "this command is unavailable while startup recovery is active",
        )),
    }
}

async fn execute(cli: &Cli, state: &AppState, data_dir: &Path) -> Result<CliSuccess, CliError> {
    match &cli.command {
        Commands::Status => {
            let service = SystemService::new(state);
            Ok(success(
                "status",
                json!({
                    "ready": true,
                    "backendVersion": service.backend_version(),
                    "dataDirectory": "<app-data>",
                }),
            ))
        }
        Commands::Diagnostics(args) => execute_diagnostics(args, state, data_dir).await,
        Commands::Object(args) => execute_object(args, state, cli, data_dir).await,
        Commands::Search(args) => {
            let service = SearchService::from_state(state).map_err(CliError::from_app)?;
            let results = service
                .search_hybrid(&args.query, Some(args.limit), args.object_type.clone())
                .await
                .map_err(CliError::from_app)?;
            value_success("search", &results, data_dir)
        }
        Commands::Capture(args) => execute_capture(args, state, data_dir).await,
        Commands::Analysis(args) => execute_analysis(args, state, data_dir).await,
        Commands::Evaluation(args) => execute_evaluation(args, state, data_dir).await,
        Commands::Job(args) => execute_job(args, state, data_dir).await,
        Commands::SearchIndex(args) => execute_search_index(args, state, data_dir).await,
        Commands::Export(args) => execute_export(args, state, cli, data_dir).await,
        Commands::Backup(args) => execute_backup(args, state, data_dir).await,
        Commands::Version | Commands::Completion { .. } => {
            unreachable!("handled before state init")
        }
    }
}

async fn execute_diagnostics(
    args: &DiagnosticsArgs,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    match &args.command {
        DiagnosticsCommands::Show => {
            let service = SystemService::new(state);
            let snapshot = service
                .local_metrics_snapshot(data_dir)
                .await
                .map_err(CliError::from_app)?;
            value_success("diagnostics.show", &snapshot, data_dir)
        }
        DiagnosticsCommands::Export { yes } => {
            if !yes {
                return Err(CliError::invalid(
                    "support bundle export requires explicit --yes confirmation",
                ));
            }
            let service = SupportBundleService::new(
                state.database().map_err(CliError::from_app)?.clone(),
                state.object_store().map_err(CliError::from_app)?.clone(),
                data_dir,
                state.backend_version().to_string(),
            );
            let result = service
                .export_support_bundle(true)
                .await
                .map_err(CliError::from_app)?;
            value_success("diagnostics.export", &result, data_dir)
        }
    }
}

async fn execute_object(
    args: &ObjectArgs,
    state: &AppState,
    cli: &Cli,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    let service = LibraryService::from_state(state).map_err(CliError::from_app)?;
    match &args.command {
        ObjectCommands::List {
            object_type,
            limit,
            cursor,
        } => {
            let offset = parse_cursor(cursor.as_deref())?;
            let objects = service
                .list_recent(Some(*limit), Some(offset), object_type.clone())
                .await
                .map_err(CliError::from_app)?;
            let next_cursor = if objects.len() == *limit as usize {
                Some((offset + *limit).to_string())
            } else {
                None
            };
            value_success(
                "object.list",
                &json!({ "items": objects, "nextCursor": next_cursor }),
                data_dir,
            )
        }
        ObjectCommands::Show {
            object_id,
            include_content,
        } => {
            validate_identifier(object_id, "object ID")?;
            let detail = service
                .get_detail(object_id)
                .await
                .map_err(CliError::from_app)?;
            if *include_content {
                if !cli.quiet {
                    eprintln!(
                        "Warning: object content may be captured by terminal history, redirection, or CI logs."
                    );
                }
                value_success("object.show", &detail, data_dir)
            } else {
                value_success("object.show", &detail.object, data_dir)
            }
        }
        ObjectCommands::Delete {
            object_id,
            mode,
            yes,
        } => {
            validate_identifier(object_id, "object ID")?;
            confirm_destructive("delete this object", *yes)?;
            let result = service
                .delete_object(object_id, (*mode).into())
                .await
                .map_err(CliError::from_app)?;
            value_success("object.delete", &result, data_dir)
        }
    }
}

async fn execute_capture(
    args: &CaptureArgs,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    match &args.command {
        CaptureCommands::Url {
            url,
            privacy,
            request_id,
        } => {
            if let Some(request_id) = request_id {
                validate_uuid(request_id, "request ID")?;
            }
            let item = RawCaptureItem {
                id: request_id.clone(),
                user_id: None,
                source_type: "url".to_string(),
                source_platform: Some("web".to_string()),
                source_url: Some(url.clone()),
                canonical_url: None,
                title: None,
                author: None,
                captured_at: None,
                raw_html: None,
                raw_text: None,
                assets: Vec::new(),
                metadata: json!({}),
                privacy_level: privacy.as_str().to_string(),
                permission_context: PermissionContext {
                    acquisition_mode: "user_action".to_string(),
                    user_confirmed: true,
                    platform_terms_hint: None,
                    allowed_for_cloud_processing: false,
                    allowed_for_third_party_ai: false,
                },
            };
            let capture = CaptureService::from_state(state).map_err(CliError::from_app)?;
            let submitted = capture
                .submit_with_request_id(item, request_id.as_deref())
                .await
                .map_err(CliError::from_app)?;
            let mut fetch = None;
            let mut analysis = None;
            if !submitted.deduplicated {
                if let Some(job_id) = submitted
                    .job_id
                    .as_deref()
                    .filter(|_| submitted.parsed_document_id.is_none())
                {
                    fetch = capture
                        .run_fetch_job(job_id)
                        .await
                        .map_err(CliError::from_app)?;
                    if let Some(failed) = fetch.as_ref().filter(|run| run.status == "failed") {
                        return Err(CliError::terminal(
                            stable_cli_error_code(
                                failed.failure_reason.as_deref(),
                                "ERR_CAPTURE_FAILED",
                            ),
                            "Capture fetch reached a failed terminal state.",
                            true,
                            6,
                            request_id.clone(),
                            json!({
                                "jobId": failed.job_id,
                                "objectId": failed.object_id,
                            }),
                        ));
                    }
                    let should_analyze = fetch.as_ref().is_some_and(|run| {
                        run.status == "succeeded" && run.parsed_document_id.is_some()
                    });
                    if should_analyze {
                        let ai =
                            AIEnrichmentService::from_state(state).map_err(CliError::from_app)?;
                        analysis = ai
                            .run_auto_enrichment_for_object(&submitted.object_id)
                            .await
                            .map_err(CliError::from_app)?;
                        if let Some(failed) = analysis.as_ref().filter(|run| run.status == "failed")
                        {
                            return Err(CliError::terminal(
                                stable_cli_error_code(
                                    failed.failure_reason.as_deref(),
                                    "ERR_AI_FAILED",
                                ),
                                "Automatic AI enrichment reached a failed terminal state.",
                                true,
                                6,
                                Some(failed.correlation_id.clone()),
                                json!({
                                    "jobId": failed.job_id,
                                    "objectId": submitted.object_id,
                                }),
                            ));
                        }
                    }
                }
            }
            value_success(
                "capture.url",
                &json!({ "submitted": submitted, "fetch": fetch, "analysis": analysis }),
                data_dir,
            )
        }
    }
}

async fn execute_analysis(
    args: &AnalysisArgs,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    match &args.command {
        AnalysisCommands::Run {
            object_id,
            request_id,
        } => {
            validate_identifier(object_id, "object ID")?;
            if let Some(request_id) = request_id {
                validate_uuid(request_id, "request ID")?;
            }
            let service = AIEnrichmentService::from_state(state).map_err(CliError::from_app)?;
            let result = service
                .run_enrichment_for_object_with_request_id(object_id, request_id.as_deref())
                .await
                .map_err(CliError::from_app)?;
            if result.status == "failed" {
                return Err(CliError::terminal(
                    stable_cli_error_code(result.failure_reason.as_deref(), "ERR_AI_FAILED"),
                    "AI enrichment reached a failed terminal state.",
                    true,
                    6,
                    Some(result.correlation_id.clone()),
                    json!({ "jobId": result.job_id, "objectId": object_id }),
                ));
            }
            value_success("analysis.run", &result, data_dir)
        }
    }
}

async fn execute_evaluation(
    args: &EvaluationArgs,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    let service = EvaluationService::from_state(state).map_err(CliError::from_app)?;
    match &args.command {
        EvaluationCommands::List => value_success(
            "evaluation.list",
            &service.list_evaluator_capabilities(),
            data_dir,
        ),
        EvaluationCommands::Run {
            object_id,
            evaluator,
            request_id,
        } => {
            validate_identifier(object_id, "object ID")?;
            if let Some(request_id) = request_id {
                validate_uuid(request_id, "request ID")?;
            }
            let result = service
                .trigger_evaluation(object_id, evaluator, request_id.as_deref())
                .await
                .map_err(CliError::from_app)?;
            if result.status == "failed" {
                return Err(CliError::terminal(
                    "ERR_EVALUATION_FAILED".to_string(),
                    "Evaluation reached a failed terminal state.",
                    false,
                    10,
                    Some(result.correlation_id.clone()),
                    json!({ "jobId": result.job_id, "runId": result.run_id, "objectId": object_id }),
                ));
            }
            value_success("evaluation.run", &result, data_dir)
        }
        EvaluationCommands::Show { run_id } => {
            validate_identifier(run_id, "run ID")?;
            let result = service
                .get_evaluation_run(run_id)
                .await
                .map_err(CliError::from_app)?;
            value_success("evaluation.show", &result, data_dir)
        }
        EvaluationCommands::Retry { run_id, request_id } => {
            validate_identifier(run_id, "run ID")?;
            if let Some(request_id) = request_id {
                validate_uuid(request_id, "request ID")?;
            }
            let result = service
                .retry_evaluation(run_id, request_id.as_deref())
                .await
                .map_err(CliError::from_app)?;
            if result.status == "failed" {
                return Err(CliError::terminal(
                    "ERR_EVALUATION_FAILED".to_string(),
                    "Evaluation retry reached a failed terminal state.",
                    false,
                    10,
                    Some(result.correlation_id.clone()),
                    json!({ "jobId": result.job_id, "runId": result.run_id }),
                ));
            }
            value_success("evaluation.retry", &result, data_dir)
        }
    }
}

async fn execute_job(
    args: &JobArgs,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    let service = OperationsService::from_state(state).map_err(CliError::from_app)?;
    match &args.command {
        JobCommands::Show { job_id } => {
            validate_identifier(job_id, "job ID")?;
            let result = service
                .get_background_job(job_id)
                .await
                .map_err(CliError::from_app)?;
            value_success("job.show", &result, data_dir)
        }
        JobCommands::Retry { job_id } => {
            validate_identifier(job_id, "job ID")?;
            let retried = service
                .reserve_retry(job_id)
                .await
                .map_err(CliError::from_app)?;
            let result = service
                .run_retry(&retried)
                .await
                .map_err(CliError::from_app)?;
            if let Some(failed) = result
                .capture
                .as_ref()
                .filter(|capture| capture.status == "failed")
            {
                return Err(CliError::terminal(
                    stable_cli_error_code(failed.failure_reason.as_deref(), "ERR_CAPTURE_FAILED"),
                    "Retried capture reached a failed terminal state.",
                    true,
                    6,
                    None,
                    json!({ "jobId": failed.job_id, "objectId": failed.object_id }),
                ));
            }
            value_success("job.retry", &result, data_dir)
        }
    }
}

async fn execute_search_index(
    args: &SearchIndexArgs,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    let service = SearchService::from_state(state).map_err(CliError::from_app)?;
    match &args.command {
        SearchIndexCommands::Check => {
            let result = service
                .check_search_index()
                .await
                .map_err(CliError::from_app)?;
            value_success("search-index.check", &result, data_dir)
        }
        SearchIndexCommands::Rebuild => {
            let queued = service
                .rebuild_search_index()
                .await
                .map_err(CliError::from_app)?;
            let result = service
                .run_rebuild_search_index(&queued.job_id)
                .await
                .map_err(CliError::from_app)?;
            if result.status == "failed" {
                return Err(CliError::terminal(
                    "ERR_SEARCH_REBUILD_FAILED".to_string(),
                    "Search index rebuild reached a failed terminal state.",
                    true,
                    7,
                    Some(result.job_id.clone()),
                    json!({ "jobId": result.job_id }),
                ));
            }
            value_success("search-index.rebuild", &result, data_dir)
        }
        SearchIndexCommands::Status { job_id } => {
            validate_identifier(job_id, "job ID")?;
            let result = service
                .get_rebuild_search_index_status(job_id)
                .await
                .map_err(CliError::from_app)?;
            value_success("search-index.status", &result, data_dir)
        }
        SearchIndexCommands::Cancel { job_id } => {
            validate_identifier(job_id, "job ID")?;
            let result = service
                .cancel_rebuild_search_index(job_id)
                .await
                .map_err(CliError::from_app)?;
            value_success("search-index.cancel", &result, data_dir)
        }
        SearchIndexCommands::Reindex { object_id } => {
            validate_identifier(object_id, "object ID")?;
            let result = service
                .reindex_object(object_id)
                .await
                .map_err(CliError::from_app)?;
            value_success("search-index.reindex", &result, data_dir)
        }
    }
}

async fn execute_export(
    args: &ExportArgs,
    state: &AppState,
    cli: &Cli,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    match &args.command {
        ExportCommands::Library { format, yes } => {
            confirm_destructive("export the portable library", *yes)?;
            if !cli.quiet {
                eprintln!(
                    "Portable exports may contain user content; keep the output directory private."
                );
            }
            let service =
                PortableExportService::from_state(state, data_dir).map_err(CliError::from_app)?;
            let result = service
                .export_library_with_format((*format).into())
                .await
                .map_err(CliError::from_app)?;
            value_success("export.library", &result, data_dir)
        }
    }
}

async fn execute_backup(
    args: &BackupArgs,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    let service = BackupService::from_state(state).map_err(CliError::from_app)?;
    match &args.command {
        BackupCommands::Create => {
            let result = service.create_backup().await.map_err(CliError::from_app)?;
            value_success("backup.create", &result, data_dir)
        }
        BackupCommands::List => {
            let result = service.list_backups().await.map_err(CliError::from_app)?;
            value_success("backup.list", &result, data_dir)
        }
        BackupCommands::Verify { backup_id } => {
            validate_identifier(backup_id, "backup ID")?;
            let result = service
                .verify_backup(backup_id)
                .await
                .map_err(CliError::from_app)?;
            value_success("backup.verify", &result, data_dir)
        }
    }
}

fn success(command: &'static str, data: Value) -> CliSuccess {
    CliSuccess { command, data }
}

fn value_success<T: Serialize>(
    command: &'static str,
    data: &T,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
    let value = serde_json::to_value(data).map_err(|_| CliError {
        code: "ERR_SERIALIZATION".to_string(),
        message: "The command result could not be serialized.".to_string(),
        retryable: false,
        exit_code: 10,
        correlation_id: None,
        operation: None,
    })?;
    Ok(success(command, redact_local_paths(value, data_dir)))
}

fn write_success(cli: &Cli, result: CliSuccess) -> i32 {
    match cli.output {
        OutputFormat::Json => {
            let envelope = json!({
                "schemaVersion": CLI_SCHEMA_VERSION,
                "ok": true,
                "command": result.command,
                "data": result.data,
            });
            println!(
                "{}",
                serde_json::to_string(&envelope).expect("JSON envelope should serialize")
            );
        }
        OutputFormat::Text => print_text_value(&result.data, 0),
    }
    0
}

fn write_error(cli: &Cli, error: CliError) -> i32 {
    match cli.output {
        OutputFormat::Json => {
            let envelope = json!({
                "schemaVersion": CLI_SCHEMA_VERSION,
                "ok": false,
                "command": command_name(&cli.command),
                "error": {
                    "code": error.code,
                    "message": error.message,
                    "correlationId": error.correlation_id,
                    "retryable": error.retryable,
                    "operation": error.operation,
                },
            });
            println!(
                "{}",
                serde_json::to_string(&envelope).expect("JSON envelope should serialize")
            );
        }
        OutputFormat::Text => {
            eprintln!("{}: {}", error.code, error.message);
            if let Some(correlation_id) = &error.correlation_id {
                eprintln!("correlationId: {correlation_id}");
            }
            if let Some(operation) = &error.operation {
                eprintln!("operation: {operation}");
            }
        }
    }
    error.exit_code
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Version => "version",
        Commands::Status => "status",
        Commands::Diagnostics(DiagnosticsArgs {
            command: DiagnosticsCommands::Show,
        }) => "diagnostics.show",
        Commands::Diagnostics(DiagnosticsArgs {
            command: DiagnosticsCommands::Export { .. },
        }) => "diagnostics.export",
        Commands::Object(ObjectArgs {
            command: ObjectCommands::List { .. },
        }) => "object.list",
        Commands::Object(ObjectArgs {
            command: ObjectCommands::Show { .. },
        }) => "object.show",
        Commands::Object(ObjectArgs {
            command: ObjectCommands::Delete { .. },
        }) => "object.delete",
        Commands::Search(_) => "search",
        Commands::Capture(_) => "capture.url",
        Commands::Analysis(_) => "analysis.run",
        Commands::Evaluation(EvaluationArgs {
            command: EvaluationCommands::List,
        }) => "evaluation.list",
        Commands::Evaluation(EvaluationArgs {
            command: EvaluationCommands::Run { .. },
        }) => "evaluation.run",
        Commands::Evaluation(EvaluationArgs {
            command: EvaluationCommands::Show { .. },
        }) => "evaluation.show",
        Commands::Evaluation(EvaluationArgs {
            command: EvaluationCommands::Retry { .. },
        }) => "evaluation.retry",
        Commands::Job(JobArgs {
            command: JobCommands::Show { .. },
        }) => "job.show",
        Commands::Job(JobArgs {
            command: JobCommands::Retry { .. },
        }) => "job.retry",
        Commands::SearchIndex(SearchIndexArgs {
            command: SearchIndexCommands::Check,
        }) => "search-index.check",
        Commands::SearchIndex(SearchIndexArgs {
            command: SearchIndexCommands::Rebuild,
        }) => "search-index.rebuild",
        Commands::SearchIndex(SearchIndexArgs {
            command: SearchIndexCommands::Status { .. },
        }) => "search-index.status",
        Commands::SearchIndex(SearchIndexArgs {
            command: SearchIndexCommands::Cancel { .. },
        }) => "search-index.cancel",
        Commands::SearchIndex(SearchIndexArgs {
            command: SearchIndexCommands::Reindex { .. },
        }) => "search-index.reindex",
        Commands::Export(_) => "export.library",
        Commands::Backup(BackupArgs {
            command: BackupCommands::Create,
        }) => "backup.create",
        Commands::Backup(BackupArgs {
            command: BackupCommands::List,
        }) => "backup.list",
        Commands::Backup(BackupArgs {
            command: BackupCommands::Verify { .. },
        }) => "backup.verify",
        Commands::Completion { .. } => "completion",
    }
}

fn print_text_value(value: &Value, indent: usize) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                match value {
                    Value::Object(_) | Value::Array(_) => {
                        println!("{}{}:", " ".repeat(indent), key);
                        print_text_value(value, indent + 2);
                    }
                    _ => println!("{}{}: {}", " ".repeat(indent), key, scalar_text(value)),
                }
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                println!("{}[]", " ".repeat(indent));
                return;
            }
            for item in items {
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        println!("{}-", " ".repeat(indent));
                        print_text_value(item, indent + 2);
                    }
                    _ => println!("{}- {}", " ".repeat(indent), scalar_text(item)),
                }
            }
        }
        _ => println!("{}{}", " ".repeat(indent), scalar_text(value)),
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

fn stable_cli_error_code(reason: Option<&str>, fallback: &str) -> String {
    let Some(prefix) = reason
        .and_then(|reason| reason.split_once(':').map(|(prefix, _)| prefix))
        .filter(|prefix| {
            !prefix.is_empty()
                && prefix.len() <= 96
                && prefix.chars().all(|character| {
                    character.is_ascii_lowercase()
                        || character.is_ascii_digit()
                        || matches!(character, '.' | '_' | '-')
                })
        })
    else {
        return fallback.to_string();
    };
    format!(
        "ERR_{}",
        prefix
            .chars()
            .map(|character| match character {
                '.' | '-' => '_',
                other => other.to_ascii_uppercase(),
            })
            .collect::<String>()
    )
}

fn redact_local_paths(value: Value, data_dir: &Path) -> Value {
    let data_dir = data_dir.to_string_lossy().replace('\\', "/");
    redact_value(value, &data_dir)
}

fn redact_value(value: Value, data_dir: &str) -> Value {
    match value {
        Value::String(value) => {
            let normalized = value.replace('\\', "/");
            if let Some(suffix) = local_path_suffix(&normalized, data_dir) {
                Value::String(format!("<app-data>{suffix}"))
            } else {
                Value::String(value)
            }
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| redact_value(value, data_dir))
                .collect(),
        ),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, redact_value(value, data_dir)))
                .collect::<Map<_, _>>(),
        ),
        other => other,
    }
}

fn local_path_suffix<'a>(value: &'a str, data_dir: &str) -> Option<&'a str> {
    if let Some(suffix) = value.strip_prefix(data_dir) {
        return Some(suffix);
    }
    if !value
        .to_ascii_lowercase()
        .starts_with(&data_dir.to_ascii_lowercase())
    {
        return None;
    }
    let byte_offset = value
        .char_indices()
        .nth(data_dir.chars().count())
        .map(|(offset, _)| offset)
        .unwrap_or(value.len());
    value.get(byte_offset..)
}

fn parse_cursor(cursor: Option<&str>) -> Result<i64, CliError> {
    cursor
        .unwrap_or("0")
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .ok_or_else(|| CliError::invalid("cursor must be a non-negative integer"))
}

fn validate_uuid(value: &str, label: &str) -> Result<(), CliError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| CliError::invalid(format!("{label} must be a UUID")))
}

fn validate_identifier(value: &str, label: &str) -> Result<(), CliError> {
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
    {
        return Err(CliError::invalid(format!("{label} is invalid")));
    }
    Ok(())
}

fn confirm_destructive(action: &str, yes: bool) -> Result<(), CliError> {
    if yes {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err(CliError::invalid(format!(
            "{action} requires explicit --yes confirmation in non-interactive mode"
        )));
    }

    eprint!("Confirm {action}? Type 'yes' to continue: ");
    io::stderr()
        .flush()
        .map_err(|_| CliError::invalid("confirmation prompt failed"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|_| CliError::invalid("confirmation input failed"))?;
    if input.trim() == "yes" {
        Ok(())
    } else {
        Err(CliError::invalid("operation was not confirmed"))
    }
}

fn resolve_data_dir() -> Result<PathBuf, CliError> {
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
            .map(|path| path.join("com.linkworld.app"))
            .ok_or_else(|| CliError::invalid("APPDATA is unavailable"))
    }

    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library/Application Support/com.linkworld.app"))
            .ok_or_else(|| CliError::invalid("HOME is unavailable"))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(path) = env::var_os("XDG_DATA_HOME") {
            return Ok(PathBuf::from(path).join("com.linkworld.app"));
        }
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".local/share/com.linkworld.app"))
            .ok_or_else(|| CliError::invalid("HOME is unavailable"))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute, execute_recovery_backup, parse_cursor, redact_local_paths, validate_identifier,
        Cli, Commands, OutputFormat,
    };
    use crate::errors::AppError;
    use crate::services::backup::BackupService;
    use crate::state::AppState;
    use clap::{CommandFactory, Parser};
    use serde_json::json;
    use std::path::Path;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use uuid::Uuid;

    #[test]
    fn command_contract_is_valid() {
        Cli::command().debug_assert();

        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("cannot use the same Node Tide data directory"));
        assert!(help.contains("ERR_RUNTIME_BUSY"));
    }

    #[test]
    fn parses_machine_output_and_nested_command() {
        let cli = Cli::try_parse_from([
            "node-tide-cli",
            "--output",
            "json",
            "object",
            "list",
            "--limit",
            "10",
        ])
        .expect("CLI should parse");

        assert_eq!(cli.output, OutputFormat::Json);
        assert!(matches!(cli.command, Commands::Object(_)));
    }

    #[test]
    fn cursor_and_identifier_validation_are_bounded() {
        assert_eq!(parse_cursor(Some("30")).expect("cursor should parse"), 30);
        assert!(parse_cursor(Some("-1")).is_err());
        assert!(validate_identifier("valid-id_1", "test").is_ok());
        assert!(validate_identifier("bad/id", "test").is_err());
    }

    #[test]
    fn redacts_application_paths_recursively() {
        let value = json!({
            "path": "C:\\Users\\tester\\AppData\\Roaming\\com.linkworld.app\\exports\\one",
            "nested": ["safe", "C:\\Users\\tester\\AppData\\Roaming\\com.linkworld.app\\logs"]
        });
        let redacted = redact_local_paths(
            value,
            Path::new("C:\\Users\\tester\\AppData\\Roaming\\com.linkworld.app"),
        );

        assert_eq!(redacted["path"], "<app-data>/exports/one");
        assert_eq!(redacted["nested"][1], "<app-data>/logs");
    }

    #[tokio::test]
    async fn capture_search_show_and_delete_share_the_application_services() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fixture server should bind");
        let address = listener
            .local_addr()
            .expect("fixture server address should resolve");
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("request should connect");
            let mut request = vec![0_u8; 4096];
            let _ = socket.read(&mut request).await;
            let body = r#"<!doctype html><html><head><title>CLI integration article</title></head><body><main><article><h1>CLI integration article</h1><p>A durable command-line capture should reuse the same application services as the desktop adapter.</p><p>This second paragraph provides enough readable content for deterministic parsing and search indexing.</p></article></main></body></html>"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response should write");
        });

        let data_dir = std::env::temp_dir().join(format!("拾海-cli-flow-{}", Uuid::new_v4()));
        let state = AppState::initialize_from_data_dir(data_dir.clone())
            .await
            .expect("application state should initialize");
        let request_id = Uuid::new_v4().to_string();
        let capture_cli = Cli::try_parse_from([
            "node-tide-cli",
            "--output",
            "json",
            "capture",
            "url",
            &format!("http://{address}/article"),
            "--request-id",
            &request_id,
        ])
        .expect("capture command should parse");
        let captured = execute(&capture_cli, &state, &data_dir)
            .await
            .expect("capture command should succeed");
        server.await.expect("fixture server should finish");

        assert_eq!(captured.command, "capture.url");
        assert_eq!(captured.data["fetch"]["status"], "succeeded");
        assert_eq!(captured.data["submitted"]["jobId"], request_id);
        let object_id = captured.data["submitted"]["objectId"]
            .as_str()
            .expect("captured object id should exist")
            .to_string();

        let search_cli =
            Cli::try_parse_from(["node-tide-cli", "search", "durable command-line capture"])
                .expect("search command should parse");
        let searched = execute(&search_cli, &state, &data_dir)
            .await
            .expect("search command should succeed");
        assert_eq!(searched.data[0]["object"]["id"], object_id);

        let show_cli = Cli::try_parse_from(["node-tide-cli", "object", "show", &object_id])
            .expect("show command should parse");
        let shown = execute(&show_cli, &state, &data_dir)
            .await
            .expect("show command should succeed");
        assert!(shown.data.get("parsedDocument").is_none());
        assert_eq!(shown.data["id"], object_id);

        let analysis_request_id = Uuid::new_v4().to_string();
        let analysis_cli = Cli::try_parse_from([
            "node-tide-cli",
            "analysis",
            "run",
            &object_id,
            "--request-id",
            &analysis_request_id,
        ])
        .expect("analysis command should parse");
        let analysis_error = execute(&analysis_cli, &state, &data_dir)
            .await
            .expect_err("missing model config should be a terminal CLI failure");
        assert_eq!(analysis_error.code, "ERR_AI_NOT_CONFIGURED");
        assert_eq!(analysis_error.exit_code, 6);
        assert_eq!(
            analysis_error.operation.as_ref().and_then(|operation| {
                operation.get("jobId").and_then(serde_json::Value::as_str)
            }),
            Some(analysis_request_id.as_str())
        );

        let delete_cli =
            Cli::try_parse_from(["node-tide-cli", "object", "delete", &object_id, "--yes"])
                .expect("delete command should parse");
        execute(&delete_cli, &state, &data_dir)
            .await
            .expect("delete command should succeed");

        drop(state);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    #[tokio::test]
    async fn recovery_mode_keeps_backup_list_and_verify_available() {
        let data_dir = std::env::temp_dir().join(format!("拾海-cli-recovery-{}", Uuid::new_v4()));
        let state = AppState::initialize_from_data_dir(data_dir.clone())
            .await
            .expect("application state should initialize");
        let backup = BackupService::from_state(&state)
            .expect("backup service should initialize")
            .create_backup()
            .await
            .expect("backup should be created");
        drop(state);

        let migration_dir = data_dir.join("migration");
        std::fs::create_dir_all(&migration_dir).expect("migration directory should create");
        std::fs::write(migration_dir.join("guard.running.json"), b"{malformed")
            .expect("malformed guard fixture should write");
        let startup_error = AppState::initialize_from_data_dir(data_dir.clone())
            .await
            .expect_err("malformed guard should force startup recovery");
        assert!(matches!(startup_error, AppError::DbMigration(_)));

        let list_cli = Cli::try_parse_from(["node-tide-cli", "backup", "list"])
            .expect("backup list should parse");
        let listed = execute_recovery_backup(&list_cli, &data_dir)
            .await
            .expect("backup list should remain available in recovery mode");
        assert!(listed.data.as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item["backupId"] == backup.backup_id)
        }));

        let verify_cli =
            Cli::try_parse_from(["node-tide-cli", "backup", "verify", &backup.backup_id])
                .expect("backup verify should parse");
        let verified = execute_recovery_backup(&verify_cli, &data_dir)
            .await
            .expect("backup verify should remain available in recovery mode");
        assert_eq!(verified.data["valid"], true);

        let _ = std::fs::remove_dir_all(data_dir);
    }
}

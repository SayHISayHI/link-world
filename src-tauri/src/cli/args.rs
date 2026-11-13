//! clap definitions for the CLI surface.

use super::*;

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
    pub(super) output: OutputFormat,

    /// Suppress non-essential progress and privacy notices.
    #[arg(long, global = true)]
    pub(super) quiet: bool,

    #[command(subcommand)]
    pub(super) command: Commands,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub(super) enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
pub(super) enum Commands {
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
    /// Inspect platform source adapters without opening local data.
    Sources(SourcesArgs),
    /// Generate a shell completion script without opening local data.
    Completion { shell: Shell },
}

#[derive(Debug, Args)]
pub(super) struct SourcesArgs {
    #[command(subcommand)]
    pub(super) command: SourcesCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum SourcesCommands {
    /// Resolve a URL through the platform adapters and print what they extract.
    Probe {
        /// URL to probe, for example a shared X or Xiaohongshu link.
        url: String,
    },
}

#[derive(Debug, Args)]
pub(super) struct DiagnosticsArgs {
    #[command(subcommand)]
    pub(super) command: DiagnosticsCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum DiagnosticsCommands {
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
pub(super) struct ObjectArgs {
    #[command(subcommand)]
    pub(super) command: ObjectCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum ObjectCommands {
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
pub(super) enum DeleteModeArg {
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
pub(super) struct SearchArgs {
    /// Full-text query.
    pub(super) query: String,
    /// Filter by object type or lifecycle navigation value.
    #[arg(long = "type")]
    pub(super) object_type: Option<String>,
    /// Maximum results to return (1-200).
    #[arg(long, default_value_t = DEFAULT_LIMIT, value_parser = clap::value_parser!(i64).range(1..=MAX_LIMIT))]
    pub(super) limit: i64,
}

#[derive(Debug, Args)]
pub(super) struct CaptureArgs {
    #[command(subcommand)]
    pub(super) command: CaptureCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum CaptureCommands {
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
pub(super) enum PrivacyArg {
    Public,
    Personal,
    Sensitive,
    Secret,
}

impl PrivacyArg {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Personal => "personal",
            Self::Sensitive => "sensitive",
            Self::Secret => "secret",
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct AnalysisArgs {
    #[command(subcommand)]
    pub(super) command: AnalysisCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum AnalysisCommands {
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
pub(super) struct EvaluationArgs {
    #[command(subcommand)]
    pub(super) command: EvaluationCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum EvaluationCommands {
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
pub(super) struct JobArgs {
    #[command(subcommand)]
    pub(super) command: JobCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum JobCommands {
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
pub(super) struct SearchIndexArgs {
    #[command(subcommand)]
    pub(super) command: SearchIndexCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum SearchIndexCommands {
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
pub(super) struct ExportArgs {
    #[command(subcommand)]
    pub(super) command: ExportCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum ExportCommands {
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
pub(super) enum ExportFormatArg {
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
pub(super) struct BackupArgs {
    #[command(subcommand)]
    pub(super) command: BackupCommands,
}

#[derive(Debug, Subcommand)]
pub(super) enum BackupCommands {
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

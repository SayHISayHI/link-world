//! JSON/text rendering and exit-code reporting.

use super::*;

pub(super) fn success(command: &'static str, data: Value) -> CliSuccess {
    CliSuccess { command, data }
}

pub(super) fn value_success<T: Serialize>(
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

pub(super) fn write_success(cli: &Cli, result: CliSuccess) -> i32 {
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

pub(super) fn write_error(cli: &Cli, error: CliError) -> i32 {
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

pub(super) fn command_name(command: &Commands) -> &'static str {
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

pub(super) fn print_text_value(value: &Value, indent: usize) {
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

pub(super) fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => "-".to_string(),
        other => other.to_string(),
    }
}

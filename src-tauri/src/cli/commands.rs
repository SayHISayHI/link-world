//! Command implementations; each resolves its service from `AppState`.

use super::*;

pub(super) async fn execute_recovery_backup(
    cli: &Cli,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
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

pub(super) async fn execute(
    cli: &Cli,
    state: &AppState,
    data_dir: &Path,
) -> Result<CliSuccess, CliError> {
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

pub(super) async fn execute_diagnostics(
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

pub(super) async fn execute_object(
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

pub(super) async fn execute_capture(
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

pub(super) async fn execute_analysis(
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

pub(super) async fn execute_evaluation(
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

pub(super) async fn execute_job(
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

pub(super) async fn execute_search_index(
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

pub(super) async fn execute_export(
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

pub(super) async fn execute_backup(
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

use super::{
    execute, execute_recovery_backup, parse_cursor, redact_local_paths, validate_identifier, Cli,
    Commands, OutputFormat,
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
        analysis_error
            .operation
            .as_ref()
            .and_then(|operation| { operation.get("jobId").and_then(serde_json::Value::as_str) }),
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

    let list_cli =
        Cli::try_parse_from(["node-tide-cli", "backup", "list"]).expect("backup list should parse");
    let listed = execute_recovery_backup(&list_cli, &data_dir)
        .await
        .expect("backup list should remain available in recovery mode");
    assert!(listed.data.as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["backupId"] == backup.backup_id)
    }));

    let verify_cli = Cli::try_parse_from(["node-tide-cli", "backup", "verify", &backup.backup_id])
        .expect("backup verify should parse");
    let verified = execute_recovery_backup(&verify_cli, &data_dir)
        .await
        .expect("backup verify should remain available in recovery mode");
    assert_eq!(verified.data["valid"], true);

    let _ = std::fs::remove_dir_all(data_dir);
}

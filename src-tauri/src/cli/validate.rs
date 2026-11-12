//! Argument validation and destructive-action confirmation.

use super::*;

pub(super) fn parse_cursor(cursor: Option<&str>) -> Result<i64, CliError> {
    cursor
        .unwrap_or("0")
        .parse::<i64>()
        .ok()
        .filter(|value| *value >= 0)
        .ok_or_else(|| CliError::invalid("cursor must be a non-negative integer"))
}

pub(super) fn validate_uuid(value: &str, label: &str) -> Result<(), CliError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| CliError::invalid(format!("{label} must be a UUID")))
}

pub(super) fn validate_identifier(value: &str, label: &str) -> Result<(), CliError> {
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

pub(super) fn confirm_destructive(action: &str, yes: bool) -> Result<(), CliError> {
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

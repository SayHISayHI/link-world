//! Local path redaction for machine output.

use super::*;

pub(super) fn redact_local_paths(value: Value, data_dir: &Path) -> Value {
    let data_dir = data_dir.to_string_lossy().replace('\\', "/");
    redact_value(value, &data_dir)
}

pub(super) fn redact_value(value: Value, data_dir: &str) -> Value {
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

pub(super) fn local_path_suffix<'a>(value: &'a str, data_dir: &str) -> Option<&'a str> {
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

use std::fs;
use std::path::Path;

use tracing_subscriber::prelude::*;

use crate::error::AppError;

const DEFAULT_FILTER: &str =
    "furniture_shop_lib=info,tauri=warn,tauri_runtime=warn,wry=warn,sqlx=warn,tracing_appender=warn";

/// Install the structured, rolling file logger. When `verbose` is set the
/// same stream is also mirrored to stderr for development.
pub fn install(log_dir: &Path, verbose: bool) -> Result<(), AppError> {
    fs::create_dir_all(log_dir)?;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_FILTER));

    let file_appender = tracing_appender::rolling::daily(log_dir, "furniture-shop.log");
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .compact();

    let registry = tracing_subscriber::registry().with(filter).with(file_layer);

    if verbose {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::stderr)
                    .with_ansi(true),
            )
            .init();
    } else {
        registry.init();
    }

    Ok(())
}

/// Mask obvious secret material inside a string before it reaches a log file
/// or the audit store. This is a defensive backstop; the primary rule is to
/// never log command arguments or user data at all.
///
/// Two passes:
/// - If the value is JSON, every sensitive key (password, token, secret,
///   salt, key material) is masked recursively so `before_json`/`after_json`
///   can never leak secrets stored under such keys.
/// - Otherwise a substring pass masks `password=`, `token=`, bearer headers,
///   etc. for plain log lines.
pub fn redact(value: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(value) {
        return redact_json(parsed).to_string();
    }
    redact_substrings(value)
}

fn redact_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, child)| {
                if is_sensitive_key(&key) {
                    (key, serde_json::Value::String("[REDACTED]".into()))
                } else {
                    (key, redact_json(child))
                }
            })
            .collect(),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(redact_json).collect())
        }
        other => other,
    }
}

fn is_sensitive_key(key: &str) -> bool {
    const EXACT: [&str; 10] = [
        "password",
        "password_hash",
        "current_password",
        "new_password",
        "token",
        "secret",
        "salt",
        "license_key",
        "backup_password",
        "authorization",
    ];
    let lower = key.to_ascii_lowercase();
    if EXACT.contains(&lower.as_str()) {
        return true;
    }
    lower.ends_with("_password")
        || lower.ends_with("_hash")
        || lower.ends_with("_token")
        || lower.ends_with("_secret")
        || lower.ends_with("_salt")
        || lower.ends_with("_key")
}

fn redact_substrings(value: &str) -> String {
    const PATTERNS: [&str; 9] = [
        "password=",
        "password:",
        "token=",
        "token:",
        "secret=",
        "secret:",
        "authorization=",
        "authorization:",
        "bearer ",
    ];

    let lower = value.to_ascii_lowercase();

    // Track the keyword occurrence whose value text begins the deepest into the
    // string (e.g. "bearer " inside an "authorization:" header) so the header
    // name survives while only the secret value is masked.
    let mut best: Option<(usize, usize)> = None;
    for pattern in PATTERNS {
        let mut from = 0;
        while let Some(relative) = lower[from..].find(pattern) {
            let occurrence = from + relative;
            let value_start = occurrence + pattern.len();
            let end = value[value_start..]
                .find(|c: char| " ,&\n;".contains(c))
                .map_or(value.len(), |offset| value_start + offset);
            if best
                .map(|(current_start, current_end)| {
                    value_start > current_start
                        || (value_start == current_start && end < current_end)
                })
                .unwrap_or(true)
            {
                best = Some((value_start, end));
            }
            from = occurrence + pattern.len().max(1);
        }
    }

    let mut out = value.to_owned();
    if let Some((start, end)) = best {
        out.replace_range(start..end, "[REDACTED]");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn masks_secret_values() {
        assert_eq!(redact("password=sup3rs3cret"), "password=[REDACTED]");
        assert_eq!(
            redact("Authorization: Bearer abc.def"),
            "Authorization: Bearer [REDACTED]"
        );
        assert_eq!(redact("token=abc, next=1"), "token=[REDACTED], next=1");
        assert_eq!(redact("no secrets here"), "no secrets here");
    }

    #[test]
    fn masks_secrets_inside_json() {
        let input = r#"{"user": 1, "password": "hunter2", "new_password": "hunter3", "nested": {"token": "abc", "safe": "kept"}, "password_changed": true, "numbers": [1, 2]}"#;
        let out = redact(input);
        assert!(!out.contains("hunter2"));
        assert!(!out.contains("hunter3"));
        assert!(!out.contains("\"abc\""));
        assert!(out.contains("password_changed"));
        assert_eq!(out.matches("[REDACTED]").count(), 3);
    }

    #[test]
    fn redacts_non_json_password_hashes_in_arrays() {
        let input = r#"["owner", {"account": {"secret": "s3cr3t"}, "data": "2000-01-01"}]"#;
        let out = redact(input);
        assert!(!out.contains("s3cr3t"));
        assert!(out.contains("2000-01-01"));
    }
}

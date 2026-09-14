use std::{fs, path::PathBuf};

use serde::Serialize;
use serde_json::{Map, Value, json};
use thiserror::Error;

const HOOK_EVENTS: [&str; 3] = ["PreToolUse", "PostToolUse", "PostToolUseFailure"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct InstallHooksResult {
    pub changed: bool,
    pub installed_hooks: u64,
}

pub fn install_claude_hooks(
    settings_path: &std::path::Path,
    executable: &std::path::Path,
    home: &std::path::Path,
) -> Result<InstallHooksResult, SettingsError> {
    let existing = if settings_path.exists() {
        Some(fs::read(settings_path).map_err(io_error)?)
    } else {
        None
    };
    let mut settings = match existing.as_deref() {
        Some(content) => serde_json::from_slice::<Value>(content)?,
        None => json!({}),
    };
    let root = settings
        .as_object_mut()
        .ok_or(SettingsError::RootMustBeObject)?;
    let hooks = object_entry(root, "hooks")?;
    let command = hook_command(executable, home)?;
    let mut installed_hooks = 0_u64;

    for event in HOOK_EVENTS {
        let entries = array_entry(hooks, event)?;
        let duplicate = entries.iter().any(|entry| {
            entry.get("matcher").and_then(Value::as_str) == Some("Edit|Write")
                && entry
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|handlers| {
                        handlers.iter().any(|handler| {
                            handler.get("type").and_then(Value::as_str) == Some("command")
                                && handler.get("command").and_then(Value::as_str)
                                    == Some(command.as_str())
                        })
                    })
        });
        if !duplicate {
            entries.push(json!({
                "matcher": "Edit|Write",
                "hooks": [{
                    "type": "command",
                    "command": command,
                    "timeout": 30
                }]
            }));
            installed_hooks += 1;
        }
    }

    if installed_hooks == 0 {
        return Ok(InstallHooksResult {
            changed: false,
            installed_hooks: 0,
        });
    }

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    if let Some(content) = existing {
        fs::write(backup_path(settings_path), content).map_err(io_error)?;
    }
    fs::write(settings_path, serde_json::to_vec_pretty(&settings)?).map_err(io_error)?;

    Ok(InstallHooksResult {
        changed: true,
        installed_hooks,
    })
}

fn object_entry<'a>(
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, SettingsError> {
    parent
        .entry(key.to_owned())
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| SettingsError::ExpectedObject(key.to_owned()))
}

fn array_entry<'a>(
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Vec<Value>, SettingsError> {
    parent
        .entry(key.to_owned())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| SettingsError::ExpectedArray(key.to_owned()))
}

fn hook_command(
    executable: &std::path::Path,
    home: &std::path::Path,
) -> Result<String, SettingsError> {
    Ok(format!(
        "{} --home {} hook",
        shell_quote(executable)?,
        shell_quote(home)?
    ))
}

#[cfg(windows)]
fn shell_quote(path: &std::path::Path) -> Result<String, SettingsError> {
    let value = path.to_string_lossy();
    if value.contains('"') {
        return Err(SettingsError::UnsafePath(path.to_path_buf()));
    }
    Ok(format!("\"{value}\""))
}

#[cfg(not(windows))]
fn shell_quote(path: &std::path::Path) -> Result<String, SettingsError> {
    let value = path
        .to_str()
        .ok_or_else(|| SettingsError::UnsafePath(path.to_path_buf()))?;
    Ok(format!("'{}'", value.replace('\'', "'\"'\"'")))
}

fn backup_path(settings_path: &std::path::Path) -> PathBuf {
    let mut name = settings_path.file_name().unwrap_or_default().to_os_string();
    name.push(".svn-ai.bak");
    settings_path.with_file_name(name)
}

fn io_error(error: std::io::Error) -> SettingsError {
    SettingsError::Io(error.to_string())
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("Claude settings root must be a JSON object")]
    RootMustBeObject,
    #[error("Claude settings field `{0}` must be a JSON object")]
    ExpectedObject(String),
    #[error("Claude hook event `{0}` must be a JSON array")]
    ExpectedArray(String),
    #[error("path cannot be represented safely in a Claude command hook: {0}")]
    UnsafePath(PathBuf),
    #[error("Claude settings I/O failed: {0}")]
    Io(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

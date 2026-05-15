//! User-level configuration management for the desktop app.
//!
//! Reads and writes `~/.batchalign.ini`, the same INI file that the CLI
//! `batchalign3 setup` command manages. The desktop setup wizard uses these
//! commands instead of the terminal-based interactive flow.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::protocol::{DesktopCommandAck, UserConfig};

/// Path to `~/.batchalign.ini`.
fn config_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".batchalign.ini")
}

/// Best-effort user home directory lookup across Unix and Windows shells.
fn home_dir() -> Option<PathBuf> {
    home_dir_from_env(|key| std::env::var_os(key))
}

/// Resolve the config path from an injected environment lookup.
#[cfg(test)]
fn config_path_from_env(mut get_var: impl FnMut(&str) -> Option<OsString>) -> PathBuf {
    home_dir_from_env(&mut get_var)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".batchalign.ini")
}

/// Best-effort user home directory lookup across Unix and Windows shells.
fn home_dir_from_env(mut get_var: impl FnMut(&str) -> Option<OsString>) -> Option<PathBuf> {
    get_var("HOME")
        .map(PathBuf::from)
        .or_else(|| get_var("USERPROFILE").map(PathBuf::from))
        .or_else(|| {
            let drive = get_var("HOMEDRIVE")?;
            let path = get_var("HOMEPATH")?;
            Some(PathBuf::from(drive).join(path))
        })
}

fn default_user_config() -> UserConfig {
    UserConfig {
        engine: "whisper".into(),
        rev_key: None,
    }
}

fn is_first_launch_at(path: &Path) -> bool {
    !path.is_file()
}

fn read_config_from_path(path: &Path) -> UserConfig {
    if !path.is_file() {
        return default_user_config();
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return default_user_config(),
    };

    // Parse INI manually (same format as batchalign2/batchalign3 CLI).
    // Format: [asr] section with engine and engine.rev.key keys.
    let mut values: HashMap<String, String> = HashMap::new();
    let mut in_asr = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_asr = trimmed == "[asr]";
            continue;
        }
        if in_asr {
            if let Some((key, value)) = trimmed.split_once('=') {
                values.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
    }

    UserConfig {
        engine: values
            .get("engine")
            .cloned()
            .unwrap_or_else(|| "whisper".into()),
        rev_key: values.get("engine.rev.key").cloned(),
    }
}

fn write_config_to_path(path: &Path, config: &UserConfig) -> Result<DesktopCommandAck, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to prepare config directory: {e}"))?;
    }

    let mut content = String::from("[asr]\n");
    content.push_str(&format!("engine = {}\n", config.engine));
    if let Some(key) = &config.rev_key {
        if !key.is_empty() {
            content.push_str(&format!("engine.rev.key = {key}\n"));
        }
    }

    std::fs::write(path, content).map_err(|e| format!("Failed to write config: {e}"))?;
    Ok(DesktopCommandAck {
        message: format!("Config saved to {}", path.display()),
    })
}

/// True if `~/.batchalign.ini` does NOT exist — triggers the setup wizard.
#[tauri::command]
pub fn is_first_launch() -> bool {
    is_first_launch_at(&config_path())
}

/// Read the current user config from `~/.batchalign.ini`.
///
/// Returns default fields if the file doesn't exist or is missing keys.
#[tauri::command]
pub fn read_config() -> UserConfig {
    read_config_from_path(&config_path())
}

/// Write user config to `~/.batchalign.ini`.
///
/// Creates the file if it doesn't exist. Overwrites any existing content.
#[tauri::command]
pub fn write_config(config: UserConfig) -> Result<DesktopCommandAck, String> {
    write_config_to_path(&config_path(), &config)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new() -> Self {
            let unique = format!(
                "batchalign-dashboard-desktop-config-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time before unix epoch")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn config_roundtrip_preserves_engine_and_key() {
        let temp = TempDirGuard::new();
        let path = temp.path().join(".batchalign.ini");
        let config = UserConfig {
            engine: "rev".into(),
            rev_key: Some("secret-key".into()),
        };

        let message = write_config_to_path(&path, &config).expect("write config");

        assert!(message.message.contains(".batchalign.ini"));
        assert_eq!(read_config_from_path(&path), config);
        assert!(!is_first_launch_at(&path));
    }

    #[test]
    fn missing_config_uses_default_values() {
        let temp = TempDirGuard::new();
        let path = temp.path().join(".batchalign.ini");

        assert_eq!(read_config_from_path(&path), default_user_config());
        assert!(is_first_launch_at(&path));
    }

    #[test]
    fn config_path_prefers_home_then_windows_fallbacks() {
        let home_path = config_path_from_env(|key| match key {
            "HOME" => Some(OsString::from("unix-home")),
            "USERPROFILE" => Some(OsString::from("windows-home")),
            "HOMEDRIVE" => Some(OsString::from("drive")),
            "HOMEPATH" => Some(OsString::from("fallback-home")),
            _ => None,
        });
        assert_eq!(
            home_path,
            PathBuf::from("unix-home").join(".batchalign.ini")
        );

        let userprofile_path = config_path_from_env(|key| match key {
            "HOME" => None,
            "USERPROFILE" => Some(OsString::from("windows-home")),
            "HOMEDRIVE" => Some(OsString::from("drive")),
            "HOMEPATH" => Some(OsString::from("fallback-home")),
            _ => None,
        });
        assert_eq!(
            userprofile_path,
            PathBuf::from("windows-home").join(".batchalign.ini")
        );

        let drive_home_path = config_path_from_env(|key| match key {
            "HOME" => None,
            "USERPROFILE" => None,
            "HOMEDRIVE" => Some(OsString::from("drive")),
            "HOMEPATH" => Some(OsString::from("fallback-home")),
            _ => None,
        });
        assert_eq!(
            drive_home_path,
            PathBuf::from("drive")
                .join("fallback-home")
                .join(".batchalign.ini")
        );
    }
}

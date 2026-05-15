//! Config loading and error types.
//!
//! Reads `server.yaml` from disk and deserializes into [`ServerConfig`].
//! Falls back to defaults when the file does not exist.

use std::path::{Path, PathBuf};

use super::{RuntimeLayout, ServerConfig};

/// Load `ServerConfig` using an explicit runtime layout when no config path is
/// passed.
pub fn load_config_from_layout(
    layout: &RuntimeLayout,
    path: Option<&Path>,
) -> Result<ServerConfig, ConfigError> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => layout.config_path().to_path_buf(),
    };

    if !path.exists() {
        return Ok(ServerConfig::default());
    }

    let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::Io(path.clone(), e))?;
    let config: ServerConfig = serde_yaml::from_str(&contents)
        .map_err(|e| ConfigError::Parse(path.clone(), e.to_string()))?;
    Ok(config)
}

/// Load [`ServerConfig`] and apply non-fatal validation/clamping.
///
/// Returns the validated config plus any warning messages produced by
/// [`ServerConfig::validate`]. Callers that need a working runtime config but
/// still want to surface bad values should prefer this helper.
pub fn load_validated_config_from_layout(
    layout: &RuntimeLayout,
    path: Option<&Path>,
) -> Result<(ServerConfig, Vec<String>), ConfigError> {
    let mut config = load_config_from_layout(layout, path)?;
    let warnings = config.validate();
    Ok((config, warnings))
}

/// Load ServerConfig from a YAML file. Falls back to defaults if the file
/// doesn't exist.
pub fn load_config(path: Option<&Path>) -> Result<ServerConfig, ConfigError> {
    let layout = RuntimeLayout::from_env();
    load_config_from_layout(&layout, path)
}

/// Errors that can occur when loading config.
///
/// Callers should distinguish between these variants to provide actionable
/// messages: `Io` typically means a permissions problem, while `Parse` means
/// the user has a syntax error in their YAML.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The config file exists but could not be read (e.g. permission denied,
    /// I/O error).  Contains the path that was attempted and the underlying
    /// OS error.
    #[error("failed to read config at {0}: {1}")]
    Io(PathBuf, #[source] std::io::Error),
    /// The config file was read but its contents are not valid YAML or do
    /// not match the expected `ServerConfig` schema.  Contains the path and
    /// a human-readable parse error.
    #[error("failed to parse config at {0}: {1}")]
    Parse(PathBuf, String),
}

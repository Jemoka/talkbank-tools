//! Rev.AI credential loading for the Rust server.
//!
//! The server currently supports two sources, in priority order:
//! 1. the legacy `~/.batchalign.ini` file written by `batchalign3 setup`
//! 2. explicit environment variables as a supplemental fallback
//!
//! That lets the control plane move Rev.AI orchestration out of Python now
//! without weakening the long-standing config file contract.

use std::path::PathBuf;

/// Typed wrapper around a Rev.AI API key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RevAiApiKey(String);

impl RevAiApiKey {
    /// Borrow the raw API key string for HTTP calls.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Errors that can occur while loading Rev.AI credentials.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RevAiCredentialError {
    /// No supported credential source produced a non-empty key.
    #[error(
        "no Rev.AI API key configured; run 'batchalign3 setup' or set BATCHALIGN_REV_API_KEY / REVAI_API_KEY"
    )]
    Missing,
    /// The legacy config file existed but could not be read.
    #[error("failed to read legacy config {path}: {source}")]
    Io {
        /// Path that could not be read.
        path: PathBuf,
        /// Underlying I/O failure.
        source: std::io::Error,
    },
}

/// Runtime-owned sources used to resolve Rev.AI credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RevAiCredentialSources {
    legacy_ini_path: PathBuf,
    batchalign_rev_api_key: Option<String>,
    revai_api_key: Option<String>,
}

impl RevAiCredentialSources {
    /// Resolve credential inputs from ambient environment variables.
    pub(crate) fn from_env() -> Self {
        Self::from_sources(
            std::env::var("BATCHALIGN_REV_API_KEY").ok().as_deref(),
            std::env::var("REVAI_API_KEY").ok().as_deref(),
            std::env::var("HOME").ok().as_deref(),
        )
    }

    /// Build credential inputs from explicit environment/home sources.
    pub(crate) fn from_sources(
        batchalign_rev_api_key: Option<&str>,
        revai_api_key: Option<&str>,
        home_env: Option<&str>,
    ) -> Self {
        let home = home_env.unwrap_or(".");
        Self {
            legacy_ini_path: PathBuf::from(home).join(".batchalign.ini"),
            batchalign_rev_api_key: normalized_env_key(batchalign_rev_api_key),
            revai_api_key: normalized_env_key(revai_api_key),
        }
    }

    /// Load the Rev.AI API key from the owned resolution chain.
    pub(crate) fn load_api_key(&self) -> Result<RevAiApiKey, RevAiCredentialError> {
        match self.read_legacy_ini_key() {
            Ok(Some(key)) => Ok(RevAiApiKey(key)),
            Ok(None) => self
                .env_key()
                .map(RevAiApiKey)
                .ok_or(RevAiCredentialError::Missing),
            Err(err) => Err(err),
        }
    }

    fn env_key(&self) -> Option<String> {
        self.batchalign_rev_api_key
            .clone()
            .or_else(|| self.revai_api_key.clone())
    }

    fn read_legacy_ini_key(&self) -> Result<Option<String>, RevAiCredentialError> {
        let path = self.legacy_ini_path.clone();
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(RevAiCredentialError::Io { path, source: err });
            }
        };

        Ok(parse_legacy_ini_revai_key(&contents))
    }
}

/// Load the Rev.AI API key from the Rust-owned resolution chain.
pub(crate) fn load_revai_api_key() -> Result<RevAiApiKey, RevAiCredentialError> {
    RevAiCredentialSources::from_env().load_api_key()
}

fn normalized_env_key(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_legacy_ini_revai_key(contents: &str) -> Option<String> {
    let mut current_section = String::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            current_section = trimmed[1..trimmed.len() - 1].trim().to_ascii_lowercase();
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };

        if current_section == "asr" && key.trim() == "engine.rev.key" {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{RevAiCredentialError, RevAiCredentialSources, parse_legacy_ini_revai_key};

    #[test]
    fn parser_reads_revai_key_from_asr_section() {
        let contents = "[asr]\nengine = rev\nengine.rev.key = secret\n";
        assert_eq!(
            parse_legacy_ini_revai_key(contents).as_deref(),
            Some("secret")
        );
    }

    #[test]
    fn parser_ignores_other_sections_and_comments() {
        let contents = "; comment\n[ud]\nmodel_version = 1.7.0\n[asr]\nengine = whisper\n";
        assert_eq!(parse_legacy_ini_revai_key(contents), None);
    }

    #[test]
    fn revai_credential_sources_fall_back_to_env_keys() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().to_string_lossy().to_string();
        let key =
            RevAiCredentialSources::from_sources(Some(" primary "), Some("secondary"), Some(&home))
                .load_api_key()
                .expect("load key");
        assert_eq!(key.as_str(), "primary");
    }

    #[test]
    fn revai_credential_sources_prefer_legacy_ini_over_env() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path();
        std::fs::write(
            home.join(".batchalign.ini"),
            "[asr]\nengine.rev.key = from-ini\n",
        )
        .expect("write ini");

        let key = RevAiCredentialSources::from_sources(
            Some("from-env"),
            Some("from-revai-env"),
            Some(home.to_str().unwrap()),
        )
        .load_api_key()
        .expect("load key");
        assert_eq!(key.as_str(), "from-ini");
    }

    #[test]
    fn revai_credential_sources_report_missing_when_all_sources_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().to_string_lossy().to_string();
        let err = RevAiCredentialSources::from_sources(Some(" "), None, Some(&home))
            .load_api_key()
            .unwrap_err();
        assert!(matches!(err, RevAiCredentialError::Missing));
    }
}

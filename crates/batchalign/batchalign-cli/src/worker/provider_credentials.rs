//! HK ASR credential resolution for worker bootstrap.
//!
//! Rust owns legacy credential discovery for worker-launched HK providers and
//! injects the resolved values into child env vars so Python stays on the
//! provider SDK boundary instead of reopening `~/.batchalign.ini`.

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::PathBuf;

use tracing::warn;

const BATCHALIGN_TENCENT_ID: &str = "BATCHALIGN_TENCENT_ID";
const BATCHALIGN_TENCENT_KEY: &str = "BATCHALIGN_TENCENT_KEY";
const BATCHALIGN_TENCENT_REGION: &str = "BATCHALIGN_TENCENT_REGION";
const BATCHALIGN_TENCENT_BUCKET: &str = "BATCHALIGN_TENCENT_BUCKET";
const BATCHALIGN_ALIYUN_AK_ID: &str = "BATCHALIGN_ALIYUN_AK_ID";
const BATCHALIGN_ALIYUN_AK_SECRET: &str = "BATCHALIGN_ALIYUN_AK_SECRET";
const BATCHALIGN_ALIYUN_AK_APPKEY: &str = "BATCHALIGN_ALIYUN_AK_APPKEY";

const ENGINE_TENCENT_ID: &str = "engine.tencent.id";
const ENGINE_TENCENT_KEY: &str = "engine.tencent.key";
const ENGINE_TENCENT_REGION: &str = "engine.tencent.region";
const ENGINE_TENCENT_BUCKET: &str = "engine.tencent.bucket";
const ENGINE_ALIYUN_AK_ID: &str = "engine.aliyun.ak_id";
const ENGINE_ALIYUN_AK_SECRET: &str = "engine.aliyun.ak_secret";
const ENGINE_ALIYUN_AK_APPKEY: &str = "engine.aliyun.ak_appkey";

#[derive(Debug, Clone, PartialEq, Eq)]
struct TencentAsrCredentials {
    secret_id: String,
    secret_key: String,
    region: String,
    bucket: String,
}

impl TencentAsrCredentials {
    fn as_env(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            (BATCHALIGN_TENCENT_ID.to_string(), self.secret_id.clone()),
            (BATCHALIGN_TENCENT_KEY.to_string(), self.secret_key.clone()),
            (BATCHALIGN_TENCENT_REGION.to_string(), self.region.clone()),
            (BATCHALIGN_TENCENT_BUCKET.to_string(), self.bucket.clone()),
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AliyunAsrCredentials {
    ak_id: String,
    ak_secret: String,
    appkey: String,
}

impl AliyunAsrCredentials {
    fn as_env(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            (BATCHALIGN_ALIYUN_AK_ID.to_string(), self.ak_id.clone()),
            (
                BATCHALIGN_ALIYUN_AK_SECRET.to_string(),
                self.ak_secret.clone(),
            ),
            (BATCHALIGN_ALIYUN_AK_APPKEY.to_string(), self.appkey.clone()),
        ])
    }
}

/// Ambient sources Rust may use to resolve HK provider credentials before
/// spawning a Python worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HkAsrCredentialSources {
    legacy_ini_path: PathBuf,
    tencent_id: Option<String>,
    tencent_key: Option<String>,
    tencent_region: Option<String>,
    tencent_bucket: Option<String>,
    aliyun_ak_id: Option<String>,
    aliyun_ak_secret: Option<String>,
    aliyun_ak_appkey: Option<String>,
}

impl HkAsrCredentialSources {
    /// Resolve ambient worker-launch credential sources from the current process
    /// environment plus the default legacy config location.
    pub(crate) fn from_env() -> Self {
        Self::from_sources(
            std::env::var(BATCHALIGN_TENCENT_ID).ok().as_deref(),
            std::env::var(BATCHALIGN_TENCENT_KEY).ok().as_deref(),
            std::env::var(BATCHALIGN_TENCENT_REGION).ok().as_deref(),
            std::env::var(BATCHALIGN_TENCENT_BUCKET).ok().as_deref(),
            std::env::var(BATCHALIGN_ALIYUN_AK_ID).ok().as_deref(),
            std::env::var(BATCHALIGN_ALIYUN_AK_SECRET).ok().as_deref(),
            std::env::var(BATCHALIGN_ALIYUN_AK_APPKEY).ok().as_deref(),
            std::env::var("HOME").ok().as_deref(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_sources(
        tencent_id: Option<&str>,
        tencent_key: Option<&str>,
        tencent_region: Option<&str>,
        tencent_bucket: Option<&str>,
        aliyun_ak_id: Option<&str>,
        aliyun_ak_secret: Option<&str>,
        aliyun_ak_appkey: Option<&str>,
        home_env: Option<&str>,
    ) -> Self {
        let home = home_env.unwrap_or(".");
        Self {
            legacy_ini_path: PathBuf::from(home).join(".batchalign.ini"),
            tencent_id: normalize_value(tencent_id),
            tencent_key: normalize_value(tencent_key),
            tencent_region: normalize_value(tencent_region),
            tencent_bucket: normalize_value(tencent_bucket),
            aliyun_ak_id: normalize_value(aliyun_ak_id),
            aliyun_ak_secret: normalize_value(aliyun_ak_secret),
            aliyun_ak_appkey: normalize_value(aliyun_ak_appkey),
        }
    }

    /// Return worker env vars for the requested ASR override, if Rust can
    /// resolve a complete credential set from env or the legacy ini file.
    pub(crate) fn provider_envs_for_asr_override(
        &self,
        asr_override: Option<&str>,
    ) -> BTreeMap<String, String> {
        match asr_override {
            Some("tencent") => self
                .load_tencent()
                .map_or_else(BTreeMap::new, |creds| creds.as_env()),
            Some("aliyun") => self
                .load_aliyun()
                .map_or_else(BTreeMap::new, |creds| creds.as_env()),
            _ => BTreeMap::new(),
        }
    }

    fn load_tencent(&self) -> Option<TencentAsrCredentials> {
        complete_tencent(
            self.tencent_id.as_deref(),
            self.tencent_key.as_deref(),
            self.tencent_region.as_deref(),
            self.tencent_bucket.as_deref(),
        )
        .or_else(|| {
            let section = self.read_legacy_ini_asr_section()?;
            complete_tencent(
                section.get(ENGINE_TENCENT_ID).map(String::as_str),
                section.get(ENGINE_TENCENT_KEY).map(String::as_str),
                section.get(ENGINE_TENCENT_REGION).map(String::as_str),
                section.get(ENGINE_TENCENT_BUCKET).map(String::as_str),
            )
        })
    }

    fn load_aliyun(&self) -> Option<AliyunAsrCredentials> {
        complete_aliyun(
            self.aliyun_ak_id.as_deref(),
            self.aliyun_ak_secret.as_deref(),
            self.aliyun_ak_appkey.as_deref(),
        )
        .or_else(|| {
            let section = self.read_legacy_ini_asr_section()?;
            complete_aliyun(
                section.get(ENGINE_ALIYUN_AK_ID).map(String::as_str),
                section.get(ENGINE_ALIYUN_AK_SECRET).map(String::as_str),
                section.get(ENGINE_ALIYUN_AK_APPKEY).map(String::as_str),
            )
        })
    }

    fn read_legacy_ini_asr_section(&self) -> Option<BTreeMap<String, String>> {
        let contents = match std::fs::read_to_string(&self.legacy_ini_path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return None,
            Err(error) => {
                warn!(
                    path = %self.legacy_ini_path.display(),
                    error = %error,
                    "failed to read legacy HK credential config"
                );
                return None;
            }
        };
        let section = parse_legacy_ini_asr_section(&contents);
        (!section.is_empty()).then_some(section)
    }
}

fn normalize_value(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn complete_tencent(
    secret_id: Option<&str>,
    secret_key: Option<&str>,
    region: Option<&str>,
    bucket: Option<&str>,
) -> Option<TencentAsrCredentials> {
    Some(TencentAsrCredentials {
        secret_id: normalize_value(secret_id)?,
        secret_key: normalize_value(secret_key)?,
        region: normalize_value(region)?,
        bucket: normalize_value(bucket)?,
    })
}

fn complete_aliyun(
    ak_id: Option<&str>,
    ak_secret: Option<&str>,
    appkey: Option<&str>,
) -> Option<AliyunAsrCredentials> {
    Some(AliyunAsrCredentials {
        ak_id: normalize_value(ak_id)?,
        ak_secret: normalize_value(ak_secret)?,
        appkey: normalize_value(appkey)?,
    })
}

fn parse_legacy_ini_asr_section(contents: &str) -> BTreeMap<String, String> {
    let mut in_asr = false;
    let mut entries = BTreeMap::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if let Some(section) = trimmed
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            in_asr = section.trim().eq_ignore_ascii_case("asr");
            continue;
        }
        if !in_asr {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            entries.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::{
        BATCHALIGN_ALIYUN_AK_APPKEY, BATCHALIGN_ALIYUN_AK_ID, BATCHALIGN_ALIYUN_AK_SECRET,
        BATCHALIGN_TENCENT_BUCKET, BATCHALIGN_TENCENT_ID, BATCHALIGN_TENCENT_KEY,
        BATCHALIGN_TENCENT_REGION, HkAsrCredentialSources,
    };

    #[test]
    fn provider_envs_prefer_explicit_tencent_env() {
        let sources = HkAsrCredentialSources::from_sources(
            Some(" env-id "),
            Some(" env-key "),
            Some(" ap-hongkong "),
            Some(" env-bucket "),
            None,
            None,
            None,
            Some("/tmp/unused-home"),
        );

        let envs = sources.provider_envs_for_asr_override(Some("tencent"));

        assert_eq!(
            envs.get(BATCHALIGN_TENCENT_ID).map(String::as_str),
            Some("env-id")
        );
        assert_eq!(
            envs.get(BATCHALIGN_TENCENT_KEY).map(String::as_str),
            Some("env-key")
        );
        assert_eq!(
            envs.get(BATCHALIGN_TENCENT_REGION).map(String::as_str),
            Some("ap-hongkong")
        );
        assert_eq!(
            envs.get(BATCHALIGN_TENCENT_BUCKET).map(String::as_str),
            Some("env-bucket")
        );
    }

    #[test]
    fn provider_envs_fall_back_to_legacy_ini() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::write(
            tempdir.path().join(".batchalign.ini"),
            "[asr]\nengine.aliyun.ak_id = legacy-id\nengine.aliyun.ak_secret = legacy-secret\nengine.aliyun.ak_appkey = legacy-appkey\n",
        )
        .unwrap();
        let sources = HkAsrCredentialSources::from_sources(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            tempdir.path().to_str(),
        );

        let envs = sources.provider_envs_for_asr_override(Some("aliyun"));

        assert_eq!(
            envs.get(BATCHALIGN_ALIYUN_AK_ID).map(String::as_str),
            Some("legacy-id")
        );
        assert_eq!(
            envs.get(BATCHALIGN_ALIYUN_AK_SECRET).map(String::as_str),
            Some("legacy-secret")
        );
        assert_eq!(
            envs.get(BATCHALIGN_ALIYUN_AK_APPKEY).map(String::as_str),
            Some("legacy-appkey")
        );
    }

    #[test]
    fn provider_envs_ignore_incomplete_env_and_other_overrides() {
        let tempdir = tempfile::tempdir().unwrap();
        std::fs::write(
            tempdir.path().join(".batchalign.ini"),
            "[asr]\nengine.tencent.id = legacy-id\nengine.tencent.key = legacy-key\nengine.tencent.region = ap-guangzhou\nengine.tencent.bucket = legacy-bucket\n",
        )
        .unwrap();
        let sources = HkAsrCredentialSources::from_sources(
            Some("partial-only"),
            None,
            None,
            None,
            None,
            None,
            None,
            tempdir.path().to_str(),
        );

        let envs = sources.provider_envs_for_asr_override(Some("tencent"));
        let empty_envs = sources.provider_envs_for_asr_override(Some("whisper"));

        assert_eq!(
            envs.get(BATCHALIGN_TENCENT_ID).map(String::as_str),
            Some("legacy-id")
        );
        assert_eq!(
            envs.get(BATCHALIGN_TENCENT_BUCKET).map(String::as_str),
            Some("legacy-bucket")
        );
        assert!(empty_envs.is_empty());
    }
}

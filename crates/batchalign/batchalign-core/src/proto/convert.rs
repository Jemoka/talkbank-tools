//! Media conversion wire types.

use crate::cache::{CacheKey, hash_serialized};
use crate::utils::{PreparedAudio, SourceId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// Output container/codec requested from the conversion backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum MediaFormat {
    Mp3,
    Wav,
}

impl MediaFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
        }
    }
}

impl std::str::FromStr for MediaFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "mp3" => Ok(Self::Mp3),
            "wav" => Ok(Self::Wav),
            _ => Err(format!(
                "unsupported media output format {value:?}; expected mp3 or wav"
            )),
        }
    }
}

/// Decoded, interleaved PCM submitted to the native conversion backend.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct ConvertInput {
    pub source_id: SourceId,
    pub audio: PreparedAudio,
}

impl CacheKey for ConvertInput {
    fn hash(&self, hasher: &mut blake3::Hasher) {
        hash_serialized(&self.audio, hasher);
    }
}

/// Encoded media artifact produced by conversion.
///
/// This is intentionally distinct from `MediaInput`: an input remains a
/// reference to an existing source path, while an output owns new bytes and
/// can only be persisted to a newly-created destination.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct MediaOutput {
    pub source_id: SourceId,
    pub format: MediaFormat,
    #[serde(with = "crate::utils::base64_bytes")]
    #[schemars(schema_with = "crate::utils::base64_bytes::json_schema")]
    pub encoded_bytes: Vec<u8>,
}

impl MediaOutput {
    /// Write without replacing any existing file.
    pub fn write(&self, path: &Path) -> crate::utils::BAResult<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&self.encoded_bytes)?;
        file.flush()?;
        Ok(())
    }
}

crate::register_proto_schema!(MediaFormat);
crate::register_proto_schema!(ConvertInput);
crate::register_proto_schema!(MediaOutput);

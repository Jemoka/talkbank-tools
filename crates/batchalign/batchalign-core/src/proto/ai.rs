//! Generic AI edit proto types.
//!
//! The backend proposes utterance-block edits; the Rust task runner owns
//! applying them to the typed CHAT AST.

use crate::cache::{CacheKey, hash_serialized};
use crate::register_proto_schema;
use crate::utils::SourceId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One rendered CHAT utterance sent to an AI backend.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AiUtterance {
    /// Zero-based utterance index in CHAT order.
    pub index: u32,
    /// Raw CHAT for this utterance, rendered from the typed AST.
    pub chat: String,
    /// Nearby utterances as raw CHAT snippets, ordered as provided by the runner.
    #[serde(default)]
    pub context: Vec<String>,
}

/// Input for a generic AI edit pass.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AiInput {
    /// Identity dedupe key.
    pub source_id: SourceId,
    /// Instruction applied to every utterance in this input.
    pub instruction: String,
    /// Utterances in CHAT order.
    pub utterances: Vec<AiUtterance>,
}

impl CacheKey for AiInput {
    /// Excludes `source_id`. Same instruction + utterance payloads → same key.
    fn hash(&self, hasher: &mut blake3::Hasher) {
        #[derive(Serialize)]
        struct K<'a> {
            instruction: &'a str,
            utterances: &'a [AiUtterance],
        }
        hash_serialized(
            &K {
                instruction: &self.instruction,
                utterances: &self.utterances,
            },
            hasher,
        );
    }
}

/// One raw CHAT block replacement returned by an AI backend.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AiRevision {
    /// Zero-based source utterance index this replacement targets.
    pub index: u32,
    /// Replacement raw CHAT. May contain one or more utterance blocks.
    pub chat: String,
}

/// Output from a generic AI edit pass.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct AiOutput {
    /// Echoes input.
    pub source_id: SourceId,
    /// Sparse replacements. Missing indices are treated as keep.
    #[serde(default)]
    pub revisions: Vec<AiRevision>,
}

register_proto_schema!(AiUtterance);
register_proto_schema!(AiInput);
register_proto_schema!(AiRevision);
register_proto_schema!(AiOutput);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::TaskInput;

    fn sample_input(source_id: &str, instruction: &str) -> AiInput {
        AiInput {
            source_id: SourceId::try_new(source_id).unwrap(),
            instruction: instruction.to_owned(),
            utterances: vec![AiUtterance {
                index: 0,
                chat: "*PAR:\thello .\n".to_owned(),
                context: vec!["*CHI:\thi .\n".to_owned()],
            }],
        }
    }

    fn digest(value: &impl CacheKey) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();
        value.hash(&mut hasher);
        hasher.finalize()
    }

    #[test]
    fn ai_cache_key_includes_instruction() {
        let first = sample_input("file-a", "translate to chinese");
        let second = sample_input("file-a", "split into CHAT utterances");

        assert_ne!(digest(&first), digest(&second));
        assert_ne!(
            digest(&TaskInput::Ai(first)),
            digest(&TaskInput::Ai(second))
        );
    }

    #[test]
    fn ai_cache_key_excludes_source_id() {
        let first = sample_input("file-a", "translate to chinese");
        let second = sample_input("file-b", "translate to chinese");

        assert_eq!(digest(&first), digest(&second));
        assert_eq!(
            digest(&TaskInput::Ai(first)),
            digest(&TaskInput::Ai(second))
        );
    }
}

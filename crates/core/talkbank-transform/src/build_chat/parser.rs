use talkbank_parser::TreeSitterParser;

use super::TranscriptDescription;

/// Shared parser and language defaults for one `build_chat` invocation.
pub(super) struct BuildChatContext {
    parser: TreeSitterParser,
    langs: Vec<String>,
    primary_lang: String,
}

impl BuildChatContext {
    /// Create the parser and normalize transcript-level language defaults once.
    pub(super) fn new(desc: &TranscriptDescription) -> Result<Self, String> {
        let parser =
            TreeSitterParser::new().map_err(|e| format!("Failed to create parser: {e}"))?;
        let langs = if desc.langs.is_empty() {
            vec!["eng".to_string()]
        } else {
            desc.langs.clone()
        };
        let primary_lang = langs.first().cloned().unwrap_or_else(|| "eng".to_string());

        Ok(Self {
            parser,
            langs,
            primary_lang,
        })
    }

    pub(super) fn parser(&self) -> &TreeSitterParser {
        &self.parser
    }

    pub(super) fn langs(&self) -> &[String] {
        &self.langs
    }

    pub(super) fn primary_lang(&self) -> &str {
        &self.primary_lang
    }
}

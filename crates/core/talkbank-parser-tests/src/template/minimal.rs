//! Minimal CHAT file template for simple tests and CLI usage.

/// Configuration for generating a minimal valid CHAT file.
#[derive(Debug, Clone)]
pub struct MinimalChatFile {
    /// Speaker code (e.g., "CHI", "MOT", "INV")
    pub speaker: String,
    /// ISO 639-3 language code (e.g., "eng", "spa", "fra")
    pub language: String,
    /// Participant role (e.g., "Target_Child", "Mother", "Investigator")
    pub role: String,
    /// Corpus identifier (e.g., "corpus", "test")
    pub corpus: String,
    /// Optional utterance content (what the speaker says)
    pub utterance: Option<String>,
}

impl Default for MinimalChatFile {
    /// Build the default minimal CHAT-file template configuration.
    fn default() -> Self {
        Self {
            speaker: "CHI".to_string(),
            language: "eng".to_string(),
            role: "Target_Child".to_string(),
            corpus: "corpus".to_string(),
            utterance: None,
        }
    }
}

impl MinimalChatFile {
    /// Create a new minimal CHAT file configuration with defaults.
    ///
    /// Defaults:
    /// - Speaker: CHI
    /// - Language: eng
    /// - Role: Target_Child
    /// - Corpus: corpus
    /// - No utterance (empty file)
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the speaker code.
    pub fn speaker(mut self, speaker: impl Into<String>) -> Self {
        self.speaker = speaker.into();
        self
    }

    /// Set the language code.
    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// Set the participant role.
    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.role = role.into();
        self
    }

    /// Set the corpus identifier.
    pub fn corpus(mut self, corpus: impl Into<String>) -> Self {
        self.corpus = corpus.into();
        self
    }

    /// Set the utterance content.
    pub fn utterance(mut self, utterance: impl Into<String>) -> Self {
        self.utterance = Some(utterance.into());
        self
    }

    /// Generate the CHAT file content as a string.
    ///
    /// The output includes:
    /// - `@UTF8` header (required for parsing)
    /// - `@Begin` marker
    /// - `@Languages` header
    /// - `@Participants` header
    /// - `@ID` header
    /// - Optional utterance line
    /// - `@End` marker
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_parser_tests::MinimalChatFile;
    ///
    /// let content = MinimalChatFile::new()
    ///     .speaker("MOT")
    ///     .role("Mother")
    ///     .utterance("hello world .")
    ///     .to_chat_string();
    ///
    /// assert!(content.contains("@UTF8"));
    /// assert!(content.contains("*MOT:\thello world ."));
    /// ```
    pub fn to_chat_string(&self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for MinimalChatFile {
    /// Render this template as CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Required UTF-8 header
        writeln!(f, "@UTF8")?;

        // Begin marker
        writeln!(f, "@Begin")?;

        // Languages header
        writeln!(f, "@Languages:\t{}", self.language)?;

        // Participants header
        writeln!(f, "@Participants:\t{} {}", self.speaker, self.role)?;

        // ID header
        writeln!(
            f,
            "@ID:\t{}|{}|{}|||||{}|||",
            self.language, self.corpus, self.speaker, self.role
        )?;

        // Optional utterance
        if let Some(ref utterance) = self.utterance {
            writeln!(f, "*{}:\t{}", self.speaker, utterance)?;
        }

        // End marker
        writeln!(f, "@End")
    }
}

/// Generate a minimal valid CHAT file with default settings.
///
/// This is a convenience function equivalent to `MinimalChatFile::new().to_string()`.
///
/// # Example
///
/// ```
/// use talkbank_parser_tests::minimal_chat_file;
///
/// let content = minimal_chat_file();
/// assert!(content.contains("@UTF8"));
/// assert!(content.contains("@Begin"));
/// assert!(content.contains("@End"));
/// ```
pub fn minimal_chat_file() -> String {
    MinimalChatFile::new().to_string()
}

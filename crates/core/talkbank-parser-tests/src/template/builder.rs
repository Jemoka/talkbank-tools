//! Full-featured CHAT file builder for complex validation tests.

/// Full-featured CHAT file builder for validation tests.
///
/// Supports:
/// - Multiple speakers
/// - Multiple utterances (for cross-utterance linker validation)
/// - Bullet timing (for monotonicity validation)
/// - Dependent tiers (for alignment validation)
/// - Custom headers
///
/// # Example
///
/// ```
/// use talkbank_parser_tests::ChatFileBuilder;
///
/// let content = ChatFileBuilder::new()
///     .language("eng")
///     .speaker("CHI", "Target_Child")
///     .speaker("MOT", "Mother")
///     .utterance("CHI", "this is first [>] .")
///     .utterance("CHI", "and [<] this continues .")
///     .utterance("MOT", "very good !")
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct ChatFileBuilder {
    language: String,
    corpus: String,
    speakers: Vec<(String, String)>, // (code, role)
    utterances: Vec<Utterance>,
    headers: Vec<String>, // Additional custom headers
}

/// Data container for Utterance.
#[derive(Debug, Clone)]
struct Utterance {
    speaker: String,
    content: String,
    timing: Option<(u64, u64)>, // (start_ms, end_ms)
    dependent_tiers: Vec<DependentTier>,
}

/// Data container for DependentTier.
#[derive(Debug, Clone)]
struct DependentTier {
    tier_type: String, // "mor", "gra", "pho", etc.
    content: String,
}

impl Default for ChatFileBuilder {
    /// Build an empty `ChatFileBuilder` with required defaults.
    fn default() -> Self {
        Self {
            language: "eng".to_string(),
            corpus: "corpus".to_string(),
            speakers: vec![],
            utterances: vec![],
            headers: vec![],
        }
    }
}

impl ChatFileBuilder {
    /// Create a new CHAT file builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the language code.
    pub fn language(mut self, lang: impl Into<String>) -> Self {
        self.language = lang.into();
        self
    }

    /// Set the corpus identifier.
    pub fn corpus(mut self, corpus: impl Into<String>) -> Self {
        self.corpus = corpus.into();
        self
    }

    /// Add a speaker.
    ///
    /// # Example
    /// ```
    /// # use talkbank_parser_tests::ChatFileBuilder;
    /// let builder = ChatFileBuilder::new()
    ///     .speaker("CHI", "Target_Child")
    ///     .speaker("MOT", "Mother");
    /// ```
    pub fn speaker(mut self, code: impl Into<String>, role: impl Into<String>) -> Self {
        self.speakers.push((code.into(), role.into()));
        self
    }

    /// Add an utterance without timing.
    ///
    /// # Example
    /// ```
    /// # use talkbank_parser_tests::ChatFileBuilder;
    /// let builder = ChatFileBuilder::new()
    ///     .speaker("CHI", "Target_Child")
    ///     .utterance("CHI", "hello world .");
    /// ```
    pub fn utterance(mut self, speaker: impl Into<String>, content: impl Into<String>) -> Self {
        self.utterances.push(Utterance {
            speaker: speaker.into(),
            content: content.into(),
            timing: None,
            dependent_tiers: vec![],
        });
        self
    }

    /// Add an utterance with bullet timing.
    ///
    /// # Example
    /// ```
    /// # use talkbank_parser_tests::ChatFileBuilder;
    /// let builder = ChatFileBuilder::new()
    ///     .speaker("CHI", "Target_Child")
    ///     .utterance_with_timing("CHI", "hello .", 1000, 2000)
    ///     .utterance_with_timing("CHI", "world .", 2500, 3500);
    /// ```
    pub fn utterance_with_timing(
        mut self,
        speaker: impl Into<String>,
        content: impl Into<String>,
        start_ms: u64,
        end_ms: u64,
    ) -> Self {
        self.utterances.push(Utterance {
            speaker: speaker.into(),
            content: content.into(),
            timing: Some((start_ms, end_ms)),
            dependent_tiers: vec![],
        });
        self
    }

    /// Add a dependent tier to the last utterance.
    ///
    /// # Example
    /// ```
    /// # use talkbank_parser_tests::ChatFileBuilder;
    /// let builder = ChatFileBuilder::new()
    ///     .speaker("CHI", "Target_Child")
    ///     .utterance("CHI", "I want cookie .")
    ///     .dependent_tier("mor", "pro|I v|want n|cookie .")
    ///     .dependent_tier("gra", "1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT");
    /// ```
    pub fn dependent_tier(
        mut self,
        tier_type: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        if let Some(last_utterance) = self.utterances.last_mut() {
            last_utterance.dependent_tiers.push(DependentTier {
                tier_type: tier_type.into(),
                content: content.into(),
            });
        }
        self
    }

    /// Add a custom header line (e.g., @Comment, @Date, @Location).
    ///
    /// # Example
    /// ```
    /// # use talkbank_parser_tests::ChatFileBuilder;
    /// let builder = ChatFileBuilder::new()
    ///     .custom_header("@Comment:\tThis is a test file")
    ///     .custom_header("@Date:\t01-JAN-2024");
    /// ```
    pub fn custom_header(mut self, header: impl Into<String>) -> Self {
        self.headers.push(header.into());
        self
    }

    /// Build the CHAT file content.
    pub fn build(self) -> String {
        let mut output = String::new();

        // UTF-8 header
        output.push_str("@UTF8\n");

        // Begin marker
        output.push_str("@Begin\n");

        // Languages
        output.push_str(&format!("@Languages:\t{}\n", self.language));

        // Participants
        if !self.speakers.is_empty() {
            output.push_str("@Participants:\t");
            let participants: Vec<String> = self
                .speakers
                .iter()
                .map(|(code, role)| format!("{} {}", code, role))
                .collect();
            output.push_str(&participants.join(", "));
            output.push('\n');
        }

        // ID headers (one per speaker)
        for (code, role) in &self.speakers {
            output.push_str(&format!(
                "@ID:\t{}|{}|{}|||||{}|||\n",
                self.language, self.corpus, code, role
            ));
        }

        // Custom headers
        for header in &self.headers {
            output.push_str(header);
            if !header.ends_with('\n') {
                output.push('\n');
            }
        }

        // Utterances
        for utterance in &self.utterances {
            // Main tier with optional timing
            output.push_str(&format!("*{}:\t", utterance.speaker));
            if let Some((start, end)) = utterance.timing {
                output.push_str(&format!("\u{0015}{}_{}\u{0015}", start, end));
            }
            output.push_str(&utterance.content);
            output.push('\n');

            // Dependent tiers
            for tier in &utterance.dependent_tiers {
                output.push_str(&format!("%{}:\t{}\n", tier.tier_type, tier.content));
            }
        }

        // End marker
        output.push_str("@End\n");

        output
    }
}

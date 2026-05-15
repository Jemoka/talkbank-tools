//! [`AlignmentHoverInfo`] data model for hover card content.
//!
//! Renderer-oriented: fields are pre-formatted strings ready for Markdown
//! assembly rather than rich typed model nodes. This keeps the formatting
//! layer thin and testable independently of the model.
/// Aggregated hover payload for one aligned tier element.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AlignmentHoverInfo {
    /// Type of element (e.g., "Main Tier Word", "Morphology Element")
    pub element_type: String,
    /// Brief description of the element
    pub element_content: String,
    /// Main tier alignment (backward: ← what main tier word this aligns from)
    pub aligned_to_main: Option<String>,
    /// Mor tier alignment (forward/backward: ↔ what mor item this aligns to/from)
    pub aligned_to_mor: Option<String>,
    /// Gra tier alignment (forward/backward: ↔ what gra relation this aligns to/from)
    pub aligned_to_gra: Option<String>,
    /// Pho tier alignment (forward/backward: ↔ what pho item this aligns to/from)
    pub aligned_to_pho: Option<String>,
    /// Mod tier alignment (forward/backward: ↔ what mod item this aligns to/from)
    pub aligned_to_mod: Option<String>,
    /// Sin tier alignment (forward/backward: ↔ what sin item this aligns to/from)
    pub aligned_to_sin: Option<String>,
    /// Additional detail rows shown below the alignment table.
    pub details: Vec<(String, String)>,
}

#[allow(dead_code)]
impl AlignmentHoverInfo {
    /// Create hover info with required identity fields.
    pub fn new(element_type: impl Into<String>, element_content: impl Into<String>) -> Self {
        Self {
            element_type: element_type.into(),
            element_content: element_content.into(),
            ..Default::default()
        }
    }

    /// Attach alignment text for the main tier.
    pub fn with_main(mut self, text: impl Into<String>) -> Self {
        self.aligned_to_main = Some(text.into());
        self
    }

    /// Attach alignment text for `%mor`.
    pub fn with_mor(mut self, text: impl Into<String>) -> Self {
        self.aligned_to_mor = Some(text.into());
        self
    }

    /// Attach alignment text for `%gra`.
    pub fn with_gra(mut self, text: impl Into<String>) -> Self {
        self.aligned_to_gra = Some(text.into());
        self
    }

    /// Attach alignment text for `%pho`.
    pub fn with_pho(mut self, text: impl Into<String>) -> Self {
        self.aligned_to_pho = Some(text.into());
        self
    }

    /// Attach alignment text for `%mod`.
    pub fn with_mod(mut self, text: impl Into<String>) -> Self {
        self.aligned_to_mod = Some(text.into());
        self
    }

    /// Attach alignment text for `%sin`.
    pub fn with_sin(mut self, text: impl Into<String>) -> Self {
        self.aligned_to_sin = Some(text.into());
        self
    }

    /// Replace detail rows with the provided key/value list.
    pub fn with_details(mut self, details: Vec<(String, String)>) -> Self {
        self.details = details;
        self
    }
}

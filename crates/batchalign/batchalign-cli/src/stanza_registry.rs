//! Runtime Stanza capability registry.
//!
//! Populated from the worker's `stanza_capabilities` field in
//! `WorkerCapabilities`. Provides typed queries for per-language
//! processor availability, replacing scattered hardcoded tables.
//!
//! During the transition period, the registry falls back to the
//! hardcoded `SUPPORTED_STANZA_CODES` when empty (old workers).

use std::collections::BTreeMap;

use crate::types::worker::StanzaLanguageProcessors;

/// Runtime Stanza capability registry.
///
/// Built from the first worker's capability report. Stored in
/// `AppState` and queried for submission validation and dispatch.
#[derive(Debug, Clone, Default)]
pub struct StanzaRegistry {
    languages: BTreeMap<String, StanzaLanguageProcessors>,
}

impl StanzaRegistry {
    /// Build from a worker's `stanza_capabilities` field.
    pub fn from_capabilities(caps: &BTreeMap<String, StanzaLanguageProcessors>) -> Self {
        Self {
            languages: caps.clone(),
        }
    }

    /// Whether the registry has been populated (non-empty).
    pub fn is_populated(&self) -> bool {
        !self.languages.is_empty()
    }

    /// Check if a language has the minimum processors for morphosyntax
    /// (tokenize + pos + lemma + depparse).
    pub fn supports_morphosyntax(&self, iso3: &str) -> bool {
        self.has_all_processors(iso3, &["tokenize", "pos", "lemma", "depparse"])
    }

    /// Check if a language supports MWT expansion.
    pub fn has_mwt(&self, iso3: &str) -> bool {
        self.has_processor(iso3, "mwt")
    }

    /// Check if a language supports constituency parsing (for utseg).
    pub fn has_constituency(&self, iso3: &str) -> bool {
        self.has_processor(iso3, "constituency")
    }

    /// Get the Stanza alpha-2 code for a language.
    pub fn alpha2(&self, iso3: &str) -> Option<&str> {
        self.languages.get(iso3).map(|p| p.alpha2.as_str())
    }

    /// All supported ISO-639-3 codes.
    pub fn supported_languages(&self) -> Vec<&str> {
        self.languages.keys().map(String::as_str).collect()
    }

    fn has_processor(&self, iso3: &str, processor: &str) -> bool {
        self.languages
            .get(iso3)
            .is_some_and(|p| p.processors.iter().any(|s| s == processor))
    }

    fn has_all_processors(&self, iso3: &str, required: &[&str]) -> bool {
        self.languages.get(iso3).is_some_and(|p| {
            required
                .iter()
                .all(|req| p.processors.iter().any(|s| s == *req))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry() -> StanzaRegistry {
        let mut caps = BTreeMap::new();
        caps.insert(
            "eng".to_string(),
            StanzaLanguageProcessors {
                alpha2: "en".to_string(),
                processors: vec![
                    "tokenize",
                    "pos",
                    "lemma",
                    "depparse",
                    "mwt",
                    "constituency",
                ]
                .into_iter()
                .map(String::from)
                .collect(),
            },
        );
        caps.insert(
            "nld".to_string(),
            StanzaLanguageProcessors {
                alpha2: "nl".to_string(),
                processors: vec!["tokenize", "pos", "lemma", "depparse"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            },
        );
        StanzaRegistry::from_capabilities(&caps)
    }

    #[test]
    fn english_supports_morphosyntax() {
        let reg = test_registry();
        assert!(reg.supports_morphosyntax("eng"));
    }

    #[test]
    fn dutch_supports_morphosyntax() {
        let reg = test_registry();
        assert!(reg.supports_morphosyntax("nld"));
    }

    #[test]
    fn english_has_constituency() {
        let reg = test_registry();
        assert!(reg.has_constituency("eng"));
    }

    #[test]
    fn dutch_has_no_constituency() {
        let reg = test_registry();
        assert!(!reg.has_constituency("nld"));
    }

    #[test]
    fn english_has_mwt() {
        let reg = test_registry();
        assert!(reg.has_mwt("eng"));
    }

    #[test]
    fn dutch_has_no_mwt() {
        let reg = test_registry();
        assert!(!reg.has_mwt("nld"));
    }

    #[test]
    fn unknown_language_not_supported() {
        let reg = test_registry();
        assert!(!reg.supports_morphosyntax("xyz"));
        assert!(!reg.has_constituency("xyz"));
    }

    #[test]
    fn alpha2_lookup() {
        let reg = test_registry();
        assert_eq!(reg.alpha2("eng"), Some("en"));
        assert_eq!(reg.alpha2("nld"), Some("nl"));
        assert_eq!(reg.alpha2("xyz"), None);
    }

    #[test]
    fn empty_registry_is_not_populated() {
        let reg = StanzaRegistry::default();
        assert!(!reg.is_populated());
        assert!(!reg.supports_morphosyntax("eng"));
    }
}

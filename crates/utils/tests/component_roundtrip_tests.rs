//! Component Roundtrip Tests
//!
//! These tests verify roundtrip for INDIVIDUAL CHAT COMPONENTS (not complete files):
//! - Words (with categories, form types, shortenings)
//! - Individual dependent tiers (%mor, %gra, %pho, %sin, %act, %cod, text tiers)
//! - Individual utterances
//!
//! Each test performs the cycle:
//! 1. CHAT text → parse → data model
//! 2. Validate data model (if validation exists for component)
//! 3. Serialize to JSON (validated against minimal JSON Schema)
//! 4. Deserialize JSON → data model
//! 5. Serialize to CHAT text
//! 6. Compare: original CHAT ≈ final CHAT
//!
//! For COMPLETE FILE roundtrips (.cha files), see:
//! - tests/json_roundtrip.rs - Full file: CHAT → parse → validate → JSON → CHAT
//! - tests/chat_roundtrip.rs - Full file: CHAT → parse → CHAT (no JSON)

#[path = "component_roundtrip_tests/error_corpus.rs"]
mod error_corpus;
#[path = "component_roundtrip_tests/helpers.rs"]
mod helpers;
#[path = "component_roundtrip_tests/roundtrip.rs"]
mod roundtrip;
#[path = "component_roundtrip_tests/utterance_roundtrip.rs"]
mod utterance_roundtrip;
#[path = "component_roundtrip_tests/validation_utterance.rs"]
mod validation_utterance;
#[path = "component_roundtrip_tests/validation_word.rs"]
mod validation_word;
#[path = "component_roundtrip_tests/word_roundtrip.rs"]
mod word_roundtrip;

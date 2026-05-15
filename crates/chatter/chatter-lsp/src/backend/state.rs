//! Shared backend state for document caches, language services, and validation bookkeeping.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use tower_lsp::Client;
use tower_lsp::lsp_types::Url;
use tree_sitter::Tree;

use talkbank_model::model::ChatFile;

use super::language_services::LanguageServices;
use crate::backend::validation_cache::ValidationCache;

/// Initialization failures for backend subsystems.
#[derive(Clone, Debug, thiserror::Error)]
pub enum BackendInitError {
    /// Tree-sitter parser failed to initialize.
    #[error("failed to initialize tree-sitter parser: {0}")]
    Parser(String),
    /// Semantic tokens provider failed to initialize.
    #[error("failed to initialize semantic tokens provider: {0}")]
    SemanticTokens(String),
}

/// Mutable server-wide state shared across LSP request/notification handlers.
#[derive(Clone)]
pub struct Backend {
    /// LSP client for sending notifications
    pub client: Client,

    /// Cache of document contents (URI -> text)
    pub documents: Arc<DashMap<Url, String>>,

    /// Thread-local parser and semantic-token services.
    pub(crate) language_services: LanguageServices,

    /// Pending validation IDs per document (for debouncing)
    pub pending_validations: Arc<DashMap<Url, u64>>,

    /// Counter for generating unique validation IDs
    pub validation_counter: Arc<AtomicU64>,

    /// Cache of parse trees per document (for incremental parsing)
    pub parse_trees: Arc<DashMap<Url, Tree>>,

    /// Cache of parsed ChatFiles per document (for incremental features)
    ///
    /// This is updated alongside parse_trees and provides quick access
    /// to the parsed model for hover, completion, and other features
    /// without needing to re-parse.
    pub chat_files: Arc<DashMap<Url, Arc<ChatFile>>>,

    /// Track whether the last parse had no syntax errors (per document).
    pub parse_clean: Arc<DashMap<Url, bool>>,

    /// Cache of validation errors per document (for incremental validation).
    pub validation_cache: Arc<DashMap<Url, ValidationCache>>,

    /// Cache of last-published LSP diagnostics per document (for pull model).
    pub last_diagnostics: Arc<DashMap<Url, Vec<tower_lsp::lsp_types::Diagnostic>>>,
}

/// The parse state of a document as observed by a feature handler.
///
/// The backend keeps the last-good parse tree / `ChatFile` alive even
/// when a transient edit makes the document un-parseable — feature
/// handlers like hover still render against the baseline so the user
/// doesn't see features disappear mid-keystroke. This enum lets
/// handlers classify the baseline age at the entry point:
///
/// - [`ParseState::Clean`] — baseline matches the current document text.
///   Feature results are authoritative.
/// - [`ParseState::StaleBaseline`] — last parse failed; the baseline
///   `ChatFile` exists but reflects an older version of the document.
///   Feature output is best-effort and may mismatch visible text.
/// - [`ParseState::Absent`] — no baseline exists (document never
///   successfully parsed). Feature output is not meaningful; callers
///   should return empty responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseState {
    /// Last parse succeeded; baseline is authoritative.
    Clean,
    /// Last parse failed; baseline is stale but still usable for
    /// graceful degradation.
    StaleBaseline,
    /// No baseline exists.
    Absent,
}

impl Backend {
    /// Create a backend with initialized language services and empty caches.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(DashMap::new()),
            language_services: LanguageServices::new(),
            pending_validations: Arc::new(DashMap::new()),
            validation_counter: Arc::new(AtomicU64::new(0)),
            parse_trees: Arc::new(DashMap::new()),
            chat_files: Arc::new(DashMap::new()),
            parse_clean: Arc::new(DashMap::new()),
            validation_cache: Arc::new(DashMap::new()),
            last_diagnostics: Arc::new(DashMap::new()),
        }
    }

    /// Classify the parse state of a document for a feature handler.
    ///
    /// Reads [`Self::parse_clean`] and the [`Self::chat_files`] cache
    /// together so callers can distinguish "authoritative", "stale but
    /// usable", and "no baseline at all" without touching the two
    /// maps themselves. This is the primitive that resolves KIB-013
    /// (feature handlers had zero readers of `parse_clean` before
    /// 2026-04-16).
    pub fn parse_state(&self, uri: &Url) -> ParseState {
        let has_baseline = self.chat_files.contains_key(uri);
        match (
            self.parse_clean.get(uri).map(|entry| *entry.value()),
            has_baseline,
        ) {
            (Some(true), true) => ParseState::Clean,
            // `parse_clean` flipped to false on a bad edit but the
            // orchestrator intentionally keeps the last-good baseline
            // alive (validation_orchestrator.rs comment at the
            // `parse_clean.insert(uri.clone(), false)` call site).
            (Some(false), true) => ParseState::StaleBaseline,
            // No entry in either map, or parse_clean is true but the
            // baseline was evicted (should be rare; conservative to
            // treat as Absent).
            _ => ParseState::Absent,
        }
    }
}

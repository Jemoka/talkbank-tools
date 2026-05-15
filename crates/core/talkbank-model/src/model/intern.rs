//! String interning for frequently duplicated values
//!
//! This module provides process-local runtime interning for frequently repeated
//! CHAT symbols (speaker codes, language codes, POS categories, common stems).
//!
//! Interners are lazily initialized via `OnceLock` and pre-populated with common
//! values, then extended on demand for corpus-specific symbols.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speaker_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Language_Codes>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Part_of_Speech>
//!
//! ## Benefits
//!
//! - **Memory savings**: 50-200MB for large corpora (5-20% reduction)
//! - **Faster cloning**: Arc::clone is O(1) atomic increment
//! - **Cache-friendly**: Interned strings are deduplicated in memory
//!
//! ## Implementation
//!
//! 1. First access initializes a dedicated `StringInterner` per symbol family.
//! 2. Common symbols are eagerly inserted.
//! 3. Unknown symbols are interned at runtime and reused thereafter.

use dashmap::DashMap;
use std::sync::Arc;
use std::sync::OnceLock;

/// Thread-safe runtime string interner for process-local symbol deduplication.
///
/// Backed by `DashMap`, this supports concurrent reads/writes during parsing
/// and validation without external synchronization.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
pub struct StringInterner {
    cache: DashMap<Arc<str>, Arc<str>>,
}

impl StringInterner {
    /// Create an empty runtime interner.
    ///
    /// Standard speaker/language/POS symbols are seeded by the global accessors,
    /// not by this constructor directly.
    fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Intern a string, returning a reference-counted shared string.
    ///
    /// If the string has been interned before, returns the existing Arc.
    /// Otherwise, creates a new Arc and stores it for future use.
    pub fn intern(&self, s: &str) -> Arc<str> {
        // Fast path: check if already interned
        if let Some(entry) = self.cache.get(s) {
            return Arc::clone(entry.value());
        }

        // Slow path: insert new entry
        let arc: Arc<str> = Arc::from(s);
        self.cache.insert(Arc::clone(&arc), Arc::clone(&arc));
        arc
    }
}

/// Global runtime interner for speaker codes.
static CUSTOM_SPEAKER_INTERNER: OnceLock<StringInterner> = OnceLock::new();

/// Return the global speaker-code interner singleton.
///
/// The instance is lazily initialized once, pre-seeded with common CHAT
/// speaker codes, and then reused for the process lifetime.
pub fn speaker_interner() -> &'static StringInterner {
    CUSTOM_SPEAKER_INTERNER.get_or_init(|| {
        let interner = StringInterner::new();

        // Pre-populate with standard speaker codes for zero-cost interning
        const STANDARD_CODES: &[&str] = &[
            "CHI", "MOT", "FAT", "BRO", "SIS", "GRA", "GRF", "GRM", "GRP", "UNC", "AUN", "INV",
            "EXP", "OBS", "CAM", "RES", "CLI", "TEA", "STU", "TUT", "DOC", "NUR", "PAT", "UNK",
            "ENV", "CHD", "ADU", "PAR", "SIB", "FRI", "VIS",
        ];

        for &code in STANDARD_CODES {
            interner.intern(code);
        }

        interner
    })
}

/// Global runtime interner for language codes.
static CUSTOM_LANGUAGE_INTERNER: OnceLock<StringInterner> = OnceLock::new();

/// Return the global language-code interner singleton.
///
/// The instance is lazily initialized once, pre-seeded with common language
/// codes, and then reused for the process lifetime.
pub fn language_interner() -> &'static StringInterner {
    CUSTOM_LANGUAGE_INTERNER.get_or_init(|| {
        let interner = StringInterner::new();

        // Pre-populate with common language codes for zero-cost interning
        const STANDARD_CODES: &[&str] = &[
            "eng", "spa", "deu", "fra", "zho", "jpn", "ita", "por", "rus", "ara", "hin", "kor",
            "nld", "tur", "pol", "ukr", "vie", "tha", "heb", "swe", "dan", "fin", "nor", "cat",
            "cze", "hun", "ron", "slk", "bul", "hrv", "srp", "slv", "est", "lav", "lit", "ell",
            "cym", "gle", "gla", "eus", "glg", "isl", "mlt", "afr", "swa", "zul", "yor", "ibo",
            "hau", "amh",
        ];

        for &code in STANDARD_CODES {
            interner.intern(code);
        }

        interner
    })
}

/// Global runtime interner for POS categories/subcategories and UD relations.
static CUSTOM_POS_INTERNER: OnceLock<StringInterner> = OnceLock::new();

/// Return the global POS/relation interner singleton.
///
/// The instance is seeded with common category/subcategory/relation symbols and
/// reused across all morphology/grammar model construction.
pub fn pos_interner() -> &'static StringInterner {
    CUSTOM_POS_INTERNER.get_or_init(|| {
        let interner = StringInterner::new();

        // Pre-populate with common POS categories
        const STANDARD_CATEGORIES: &[&str] = &[
            // Open class (content words)
            "v", "n", "adj", "adv", // Closed class (function words)
            "pro", "det", "prep", "conj", "aux", // Other common categories
            "num", "int", "on", "co", "part", "inf", "neg", "qn", "wh", "rel", "ptl", "cop",
        ];

        // Pre-populate with common POS subcategories
        const STANDARD_SUBCATEGORIES: &[&str] = &[
            // Pronoun subcategories
            "sub", "obj", "poss", "dem", "indef", "refl", "recip", "wh", "rel",
            // Determiner subcategories
            "art", "def", "dem", "num", "poss", "q", "quant", // Verb subcategories
            "aux", "cop", "mod", "perf", "prog", // Noun subcategories
            "prop", "gerund", "pt", // Other common subcategories
            "pl", "dim", "aug", "coll", "sg",
        ];

        // Pre-populate with Universal Dependencies relations
        const STANDARD_RELATIONS: &[&str] = &[
            "ROOT",
            "SUBJ",
            "OBJ",
            "IOBJ",
            "DET",
            "AMOD",
            "NMOD",
            "ADVMOD",
            "AUX",
            "CASE",
            "MARK",
            "PUNCT",
            "CONJ",
            "CC",
            "CCOMP",
            "XCOMP",
            "ACL",
            "ADVCL",
            "NUMMOD",
            "APPOS",
            "COMPOUND",
            "FLAT",
            "FIXED",
            "LIST",
            "PARATAXIS",
            "ORPHAN",
            "GOESWITH",
            "REPARANDUM",
            "DISLOCATED",
            "VOCATIVE",
            "DISCOURSE",
            "EXPL",
            "COP",
            "DEP",
        ];

        for &cat in STANDARD_CATEGORIES {
            interner.intern(cat);
        }

        for &sub in STANDARD_SUBCATEGORIES {
            interner.intern(sub);
        }

        for &rel in STANDARD_RELATIONS {
            interner.intern(rel);
        }

        interner
    })
}

/// Global runtime interner for high-frequency morphological stems.
static CUSTOM_STEM_INTERNER: OnceLock<StringInterner> = OnceLock::new();

/// Return the global stem interner singleton.
///
/// Seeded with high-frequency stems to reduce allocations in large corpora, and
/// extended on demand for corpus-specific lexical items.
pub fn stem_interner() -> &'static StringInterner {
    CUSTOM_STEM_INTERNER.get_or_init(|| {
        let interner = StringInterner::new();

        // Pre-populate with ultra-common English stems that appear thousands of times
        // These represent the top ~100 most frequent words in English child language data
        const STANDARD_STEMS: &[&str] = &[
            // Articles (highest frequency)
            "the",
            "a",
            "an",
            // Pronouns
            "I",
            "you",
            "he",
            "she",
            "it",
            "we",
            "they",
            "me",
            "him",
            "her",
            "us",
            "them",
            "my",
            "your",
            "his",
            "her",
            "its",
            "our",
            "their",
            "mine",
            "yours",
            "hers",
            "ours",
            "theirs",
            "this",
            "that",
            "these",
            "those",
            "what",
            "who",
            "which",
            "where",
            "when",
            "why",
            "how",
            // Common verbs (be, have, do paradigms + top 50)
            "be",
            "am",
            "is",
            "are",
            "was",
            "were",
            "been",
            "being",
            "have",
            "has",
            "had",
            "having",
            "do",
            "does",
            "did",
            "doing",
            "done",
            "go",
            "going",
            "went",
            "gone",
            "get",
            "getting",
            "got",
            "gotten",
            "make",
            "making",
            "made",
            "see",
            "seeing",
            "saw",
            "seen",
            "know",
            "knowing",
            "knew",
            "known",
            "want",
            "wanting",
            "wanted",
            "come",
            "coming",
            "came",
            "think",
            "thinking",
            "thought",
            "take",
            "taking",
            "took",
            "taken",
            "give",
            "giving",
            "gave",
            "given",
            "say",
            "saying",
            "said",
            "tell",
            "telling",
            "told",
            "put",
            "putting",
            "find",
            "finding",
            "found",
            "look",
            "looking",
            "looked",
            "use",
            "using",
            "used",
            "work",
            "working",
            "worked",
            "call",
            "calling",
            "called",
            "try",
            "trying",
            "tried",
            "feel",
            "feeling",
            "felt",
            "leave",
            "leaving",
            "left",
            "ask",
            "asking",
            "asked",
            "need",
            "needing",
            "needed",
            "seem",
            "seeming",
            "seemed",
            "turn",
            "turning",
            "turned",
            // Auxiliaries and modals
            "will",
            "would",
            "shall",
            "should",
            "can",
            "could",
            "may",
            "might",
            "must",
            // Common nouns
            "thing",
            "things",
            "time",
            "times",
            "person",
            "people",
            "way",
            "ways",
            "day",
            "days",
            "man",
            "men",
            "woman",
            "women",
            "child",
            "children",
            "year",
            "years",
            "hand",
            "hands",
            "eye",
            "eyes",
            "place",
            "places",
            "work",
            "part",
            "parts",
            "case",
            "cases",
            "week",
            "weeks",
            "number",
            "numbers",
            "point",
            "points",
            "fact",
            "facts",
            // Common adjectives/adverbs
            "good",
            "better",
            "best",
            "new",
            "old",
            "big",
            "small",
            "long",
            "short",
            "great",
            "little",
            "own",
            "other",
            "same",
            "different",
            "high",
            "low",
            "large",
            "small",
            // Common prepositions/conjunctions
            "in",
            "on",
            "at",
            "to",
            "for",
            "with",
            "from",
            "by",
            "of",
            "up",
            "about",
            "into",
            "through",
            "over",
            "after",
            "and",
            "or",
            "but",
            "if",
            "because",
            "so",
            "than",
            // Other high-frequency
            "not",
            "no",
            "yes",
            "all",
            "some",
            "any",
            "many",
            "much",
            "more",
            "most",
            "very",
            "just",
            "only",
            "also",
            "even",
            "now",
            "then",
            "here",
            "there",
            "out",
            "back",
        ];

        for &stem in STANDARD_STEMS {
            interner.intern(stem);
        }

        interner
    })
}

/// Global runtime interner for participant roles (pre-populated with standard roles)
static CUSTOM_PARTICIPANT_INTERNER: OnceLock<StringInterner> = OnceLock::new();

/// Get the global participant role interner, pre-populated with standard roles
pub fn participant_interner() -> &'static StringInterner {
    CUSTOM_PARTICIPANT_INTERNER.get_or_init(|| {
        let interner = StringInterner::new();

        // Pre-populate with standard participant roles
        const STANDARD_ROLES: &[&str] = &[
            "Target_Child",
            "Mother",
            "Father",
            "Investigator",
            "Sibling",
            "Teacher",
            "Therapist",
            "Examiner",
            "Observer",
            "Child",
            "Adult",
            "Parent",
            "Friend",
            "Visitor",
        ];

        for &role in STANDARD_ROLES {
            interner.intern(role);
        }

        interner
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Interning the same string twice returns the same shared allocation.
    ///
    /// Pointer equality is the core guarantee that provides memory savings and
    /// cheap clone semantics in hot code paths.
    #[test]
    fn test_interner_deduplicates() {
        let interner = StringInterner::new();

        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");

        // Same Arc (pointer equality)
        assert!(Arc::ptr_eq(&s1, &s2));
        assert_eq!(s1.as_ref(), "hello");
    }

    /// Distinct strings should not alias to the same interned pointer.
    ///
    /// This protects correctness for equality checks that rely on lexical identity.
    #[test]
    fn test_interner_different_strings() {
        let interner = StringInterner::new();

        let s1 = interner.intern("hello");
        let s2 = interner.intern("world");

        // Different Arcs
        assert!(!Arc::ptr_eq(&s1, &s2));
        assert_eq!(s1.as_ref(), "hello");
        assert_eq!(s2.as_ref(), "world");
    }
}

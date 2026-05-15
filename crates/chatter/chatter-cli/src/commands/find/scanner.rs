//! Lightweight CHAT file scanner for `chatter find`.
//!
//! A [`ChatHeaderScanner`] reads a single CHAT file once and records:
//!
//! - the typed `@Languages` codes (parsed via the tree-sitter header fragment
//!   parser — no regex hacking);
//! - the number of main-tier lines (`*SPK:` prefixes), used by consumers to
//!   gauge transcript length without a full parse;
//! - the number of occurrences of a caller-supplied substring pattern in the
//!   body (e.g. `"@s"`), scanned as raw bytes so the cost is linear in file
//!   size regardless of CHAT structure.
//!
//! The scanner is intentionally cheaper than a full `talkbank_parser`
//! parse: we walk the file once, dispatch each `@Languages:` header line to
//! the fragment parser, and count body-substring occurrences with a simple
//! non-overlapping match. This is fast enough to filter corpora with 100K+
//! files in a few minutes on a developer machine while still deriving
//! language information through the typed parser path rather than ad-hoc
//! regex.
//!
//! Counting caveat: `--has-token` is a substring match, not a
//! CHAT-semantic token match. For the `@s` use case (finding files with
//! code-switched words), the over-count is negligible because `@s` is
//! vanishingly rare in prose. Consumers that need exact CHAT-token counts
//! should run a full `chatter validate` / custom analyzer on the filtered
//! set.

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use talkbank_model::LanguageCodes;
use talkbank_model::NullErrorSink;
use talkbank_model::model::Header;
use talkbank_parser::TreeSitterParser;

/// Number of `*SPK:` lines observed in a CHAT file.
///
/// Proxies transcript length. Each main-tier line starts with `*` followed
/// by a speaker code. Multi-line utterances (continuation with `\t` leader)
/// only increment the count once, matching user intuition of "number of
/// utterances".
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct UtteranceLineCount(pub u32);

impl UtteranceLineCount {
    /// Returns the inner count.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Number of occurrences of the `--has-token` substring in the body of a
/// CHAT file.
///
/// Substring count, not CHAT-semantic token count — see module docs.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct TokenPatternCount(pub u32);

impl TokenPatternCount {
    /// Returns the inner count.
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Result of scanning a single CHAT file.
#[derive(Clone, Debug)]
pub struct ChatFileScan {
    /// Absolute or caller-supplied path to the scanned file.
    pub path: PathBuf,
    /// Typed `@Languages` payload. Empty [`LanguageCodes`] when the header
    /// was absent or unparseable — distinguishable from "parsed but empty"
    /// only by consulting the parser in stricter contexts; for `find` the
    /// semantics collapse to "no language information available".
    pub languages: LanguageCodes,
    /// Number of `*` main-tier lines observed.
    pub utterance_count: UtteranceLineCount,
    /// Occurrences of the scanner's token pattern in the body. Zero when
    /// the caller did not supply a pattern.
    pub token_count: TokenPatternCount,
    /// File size on disk, in bytes. Useful for reporting and CSV output.
    pub file_bytes: u64,
}

/// Byte pattern searched in the body of each scanned file.
///
/// Held as a `String` rather than `&str` so the scanner owns the pattern
/// and can be passed across thread boundaries. Construction validates
/// non-emptiness: an empty pattern would cause `str::match_indices` to
/// match at every offset, which is never useful.
#[derive(Clone, Debug)]
pub struct TokenPattern(String);

impl TokenPattern {
    /// Creates a new pattern, rejecting the empty string.
    pub fn new(pattern: impl Into<String>) -> Result<Self, EmptyTokenPattern> {
        let pattern = pattern.into();
        if pattern.is_empty() {
            return Err(EmptyTokenPattern);
        }
        Ok(Self(pattern))
    }

    /// Returns the pattern as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Error returned when attempting to construct a [`TokenPattern`] from an
/// empty string.
#[derive(Clone, Copy, Debug, thiserror::Error)]
#[error("token pattern must be non-empty")]
pub struct EmptyTokenPattern;

/// Errors emitted by [`ChatHeaderScanner`].
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// The file could not be opened or read.
    #[error("I/O error scanning {path}: {source}")]
    Io {
        /// Path we tried to scan.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
}

/// Per-thread scanner that owns a tree-sitter parser.
///
/// `TreeSitterParser` is `!Send + !Sync`, so each worker thread must own
/// its own `ChatHeaderScanner`. Create one at the entry point of a
/// sequential scan, or one per rayon thread for a parallel scan.
pub struct ChatHeaderScanner {
    parser: TreeSitterParser,
    pattern: Option<TokenPattern>,
}

impl ChatHeaderScanner {
    /// Creates a scanner with no body token pattern.
    ///
    /// # Errors
    ///
    /// Returns a parser initialization error when the tree-sitter grammar
    /// cannot be loaded (e.g. ABI version mismatch).
    pub fn new() -> Result<Self, talkbank_parser::ParserInitError> {
        Ok(Self {
            parser: TreeSitterParser::new()?,
            pattern: None,
        })
    }

    /// Sets the body token pattern to count during each scan.
    pub fn with_pattern(mut self, pattern: TokenPattern) -> Self {
        self.pattern = Some(pattern);
        self
    }

    /// Scans one CHAT file.
    ///
    /// Reads the file line-by-line. Header lines (starting with `@`) that
    /// match `@Languages:` are dispatched to the tree-sitter header
    /// fragment parser. Once the first main-tier line (`*`) is seen or
    /// `@End` is reached, header scanning stops and body scanning begins.
    /// The body is scanned for token-pattern matches and main-tier-line
    /// counts.
    pub fn scan(&self, path: &Path) -> Result<ChatFileScan, ScanError> {
        let file = File::open(path).map_err(|source| ScanError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let file_bytes = file
            .metadata()
            .map(|m| m.len())
            .map_err(|source| ScanError::Io {
                path: path.to_path_buf(),
                source,
            })?;

        let reader = BufReader::new(file);
        self.scan_reader(path, file_bytes, reader)
    }

    /// Scans from an arbitrary reader, primarily for tests.
    pub fn scan_reader<R: BufRead>(
        &self,
        path: &Path,
        file_bytes: u64,
        mut reader: R,
    ) -> Result<ChatFileScan, ScanError> {
        let mut languages = LanguageCodes::default();
        let mut utterance_count: u32 = 0;
        let mut token_count: u32 = 0;
        let mut in_body = false;

        let mut line = String::new();
        loop {
            line.clear();
            let read = reader
                .read_line(&mut line)
                .map_err(|source| ScanError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            if read == 0 {
                break;
            }

            if !in_body {
                // Tab-leader continuation lines belong to the previous
                // header and should not trigger main-tier detection.
                if line.starts_with('*') {
                    in_body = true;
                    utterance_count = utterance_count.saturating_add(1);
                    // Fall through to body counting for this line.
                } else if let Some(rest) = line.strip_prefix("@Languages:") {
                    // Reconstruct the fragment input (header line without
                    // trailing newline; parser handles internal whitespace).
                    let fragment = format!("@Languages:{}", rest.trim_end_matches('\n'));
                    let sink = NullErrorSink;
                    if let Some(Header::Languages { codes }) = self
                        .parser
                        .parse_header_fragment(&fragment, 0, &sink)
                        .into_option()
                    {
                        languages = codes;
                    }
                    continue;
                } else {
                    // Other header or blank line — skip.
                    continue;
                }
            } else if line.starts_with('*') {
                utterance_count = utterance_count.saturating_add(1);
            }

            if let Some(pattern) = &self.pattern {
                token_count =
                    token_count.saturating_add(count_substring(&line, pattern.as_str()) as u32);
            }
        }

        Ok(ChatFileScan {
            path: path.to_path_buf(),
            languages,
            utterance_count: UtteranceLineCount(utterance_count),
            token_count: TokenPatternCount(token_count),
            file_bytes,
        })
    }
}

/// Counts non-overlapping occurrences of `needle` in `haystack`.
///
/// Pulled out as a free function so it can be unit-tested directly.
fn count_substring(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

#[cfg(test)]
mod scanner_tests {
    use super::*;
    use std::io::Cursor;

    fn scan_bytes(scanner: &ChatHeaderScanner, bytes: &[u8]) -> ChatFileScan {
        scanner
            .scan_reader(
                Path::new("test.cha"),
                bytes.len() as u64,
                Cursor::new(bytes),
            )
            .expect("scan ok")
    }

    #[test]
    fn extracts_bilingual_languages_header() {
        let scanner = ChatHeaderScanner::new().expect("parser init");
        let input = b"@UTF8\n@Begin\n@Languages:\tspa, eng\n@Participants:\tMOT Mother Mother\n*MOT:\thola .\n@End\n";
        let scan = scan_bytes(&scanner, input);
        let codes: Vec<_> = scan
            .languages
            .iter()
            .map(|c| c.as_str().to_string())
            .collect();
        assert_eq!(codes, vec!["spa", "eng"]);
    }

    #[test]
    fn counts_utterance_lines() {
        let scanner = ChatHeaderScanner::new().expect("parser init");
        let input =
            b"@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello .\n*MOT:\thi .\n*CHI:\tbye .\n@End\n";
        let scan = scan_bytes(&scanner, input);
        assert_eq!(scan.utterance_count.get(), 3);
    }

    #[test]
    fn counts_at_s_tokens_in_body_only() {
        let scanner = ChatHeaderScanner::new()
            .expect("parser init")
            .with_pattern(TokenPattern::new("@s").expect("non-empty"));
        let input = b"@UTF8\n@Begin\n@Languages:\tspa, eng\n*MOT:\tmira el mall@s .\n*CHI:\tok@s yes@s .\n@End\n";
        let scan = scan_bytes(&scanner, input);
        assert_eq!(scan.token_count.get(), 3);
    }

    #[test]
    fn missing_languages_header_yields_empty_codes() {
        let scanner = ChatHeaderScanner::new().expect("parser init");
        let input = b"@UTF8\n@Begin\n@Participants:\tMOT Mother Mother\n*MOT:\thola .\n@End\n";
        let scan = scan_bytes(&scanner, input);
        assert!(scan.languages.is_empty());
    }

    #[test]
    fn trilingual_languages_header_parses_all_three() {
        let scanner = ChatHeaderScanner::new().expect("parser init");
        let input =
            b"@UTF8\n@Begin\n@Languages:\tzho, eng, yue\n*MOT:\t\xe4\xbd\xa0\xe5\xa5\xbd .\n@End\n";
        let scan = scan_bytes(&scanner, input);
        let codes: Vec<_> = scan
            .languages
            .iter()
            .map(|c| c.as_str().to_string())
            .collect();
        assert_eq!(codes, vec!["zho", "eng", "yue"]);
    }

    #[test]
    fn token_pattern_must_be_nonempty() {
        assert!(TokenPattern::new("").is_err());
        assert!(TokenPattern::new("@s").is_ok());
    }
}

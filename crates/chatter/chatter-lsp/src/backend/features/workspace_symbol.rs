//! Workspace symbol provider for CHAT files.
//!
//! Searches across all open CHAT documents for headers (`@`-lines) and
//! speaker utterances (`*SPEAKER:`) matching the query string.

use tower_lsp::lsp_types::*;

/// Search a single document for symbols matching the query.
pub fn workspace_symbols_for_document(uri: &Url, doc: &str, query: &str) -> Vec<SymbolInformation> {
    let query_lower = query.to_lowercase();
    let mut symbols = Vec::new();

    for (line_idx, line) in doc.lines().enumerate() {
        let name = if line.starts_with('@') {
            // Header line: use the header name as the symbol.
            line.split('\t').next().unwrap_or(line).to_string()
        } else if line.starts_with('*') {
            // Main tier: use speaker code.
            line.split('\t').next().unwrap_or(line).to_string()
        } else {
            continue;
        };

        if !query.is_empty() && !name.to_lowercase().contains(&query_lower) {
            continue;
        }

        let kind = if line.starts_with('@') {
            SymbolKind::PROPERTY
        } else {
            SymbolKind::FUNCTION
        };

        #[allow(deprecated)]
        symbols.push(SymbolInformation {
            name,
            kind,
            tags: None,
            deprecated: None,
            location: Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: 0,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line.len() as u32,
                    },
                },
            },
            container_name: None,
        });
    }

    symbols
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_symbols_finds_headers_and_speakers() {
        let uri = Url::parse("file:///test.cha").unwrap();
        let doc = "@UTF8\n@Begin\n@Participants:\tCHI Child\n*CHI:\thello .\n*MOT:\thi .\n@End\n";

        let all = workspace_symbols_for_document(&uri, doc, "");
        assert_eq!(all.len(), 6); // 4 headers + 2 speakers

        let chi_only = workspace_symbols_for_document(&uri, doc, "CHI");
        assert_eq!(chi_only.len(), 1); // *CHI: line only (header name is @Participants:, not CHI)
    }

    #[test]
    fn workspace_symbols_query_is_case_insensitive() {
        let uri = Url::parse("file:///test.cha").unwrap();
        let doc = "@UTF8\n@Begin\n@Participants:\tCHI Child\n*CHI:\thello .\n@End\n";

        let upper = workspace_symbols_for_document(&uri, doc, "CHI");
        let lower = workspace_symbols_for_document(&uri, doc, "chi");
        assert_eq!(upper.len(), lower.len(), "Query should be case-insensitive");
    }

    #[test]
    fn workspace_symbols_assigns_correct_kinds() {
        let uri = Url::parse("file:///test.cha").unwrap();
        let doc = "@UTF8\n@Begin\n@Participants:\tCHI Child\n*CHI:\thello .\n@End\n";

        let all = workspace_symbols_for_document(&uri, doc, "");
        for sym in &all {
            if sym.name.starts_with('@') {
                assert_eq!(
                    sym.kind,
                    SymbolKind::PROPERTY,
                    "Headers should be PROPERTY kind, got {:?} for {}",
                    sym.kind,
                    sym.name
                );
            } else if sym.name.starts_with('*') {
                assert_eq!(
                    sym.kind,
                    SymbolKind::FUNCTION,
                    "Speaker lines should be FUNCTION kind, got {:?} for {}",
                    sym.kind,
                    sym.name
                );
            }
        }
    }

    #[test]
    fn workspace_symbols_line_positions_are_correct() {
        let uri = Url::parse("file:///test.cha").unwrap();
        let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";

        let all = workspace_symbols_for_document(&uri, doc, "");
        // @UTF8 is on line 0, @Begin on line 1, *CHI: on line 2, @End on line 3.
        let utf8 = all.iter().find(|s| s.name == "@UTF8").unwrap();
        assert_eq!(utf8.location.range.start.line, 0);

        let begin = all.iter().find(|s| s.name == "@Begin").unwrap();
        assert_eq!(begin.location.range.start.line, 1);

        let chi = all.iter().find(|s| s.name == "*CHI:").unwrap();
        assert_eq!(chi.location.range.start.line, 2);

        let end = all.iter().find(|s| s.name == "@End").unwrap();
        assert_eq!(end.location.range.start.line, 3);
    }
}

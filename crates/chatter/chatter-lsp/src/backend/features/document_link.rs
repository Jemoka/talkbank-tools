//! Document link provider for CHAT files.
//!
//! Detects `@Media` header lines and creates clickable links to the
//! referenced media file when it exists on disk alongside the CHAT file.

use std::path::Path;

use tower_lsp::lsp_types::*;

/// Find document links in a CHAT file (e.g., @Media references).
pub fn document_links(uri: &Url, document: &str) -> Vec<DocumentLink> {
    let base_dir = match uri
        .to_file_path()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        Some(dir) => dir,
        None => return Vec::new(),
    };

    let mut links = Vec::new();

    for (line_idx, line) in document.lines().enumerate() {
        if !line.starts_with("@Media:") {
            continue;
        }

        // @Media:\tfilename, audio/video
        let content = match line.split('\t').nth(1) {
            Some(c) => c.trim(),
            None => continue,
        };

        // Extract filename (before the comma).
        let filename = content.split(',').next().unwrap_or(content).trim();
        if filename.is_empty() {
            continue;
        }

        // Try common media extensions.
        let media_path = resolve_media_file(&base_dir, filename);
        let target_uri = match media_path {
            Some(path) => Url::from_file_path(path).ok(),
            None => None,
        };

        // Link range covers the filename portion.
        let tab_offset = line.find('\t').map_or(0, |i| i + 1);
        let filename_end = tab_offset + filename.len();

        links.push(DocumentLink {
            range: Range {
                start: Position {
                    line: line_idx as u32,
                    character: tab_offset as u32,
                },
                end: Position {
                    line: line_idx as u32,
                    character: filename_end as u32,
                },
            },
            target: target_uri,
            tooltip: Some(format!("Open media file: {filename}")),
            data: None,
        });
    }

    links
}

/// Try to resolve a media filename to an actual file path.
fn resolve_media_file(base_dir: &Path, filename: &str) -> Option<std::path::PathBuf> {
    // Try the filename as-is first.
    let direct = base_dir.join(filename);
    if direct.exists() {
        return Some(direct);
    }

    // Try common extensions if the filename has none.
    if Path::new(filename).extension().is_none() {
        for ext in &["wav", "mp3", "mp4", "m4a", "m4v", "mov", "ogg", "flac"] {
            let with_ext = base_dir.join(format!("{filename}.{ext}"));
            if with_ext.exists() {
                return Some(with_ext);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_links_finds_media_header() {
        let uri = Url::parse("file:///tmp/test.cha").unwrap();
        let doc = "@UTF8\n@Begin\n@Media:\tmyfile, audio\n*CHI:\thello .\n@End\n";
        let links = document_links(&uri, doc);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].tooltip.as_deref(), Some("Open media file: myfile"));
        assert_eq!(links[0].range.start.line, 2);
    }

    #[test]
    fn test_no_links_without_media() {
        let uri = Url::parse("file:///tmp/test.cha").unwrap();
        let doc = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";
        let links = document_links(&uri, doc);
        assert!(links.is_empty());
    }
}

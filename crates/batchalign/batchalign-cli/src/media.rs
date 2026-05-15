//! Media resolution — port of `batchalign/serve/media.py`.
//!
//! Searches configured media_roots for audio/video files. Results are cached
//! with a 60-second TTL to avoid rescanning NFS mounts on every request.

use std::path::{Path, PathBuf};
use std::time::Instant;

use dashmap::DashMap;
use tracing::{debug, warn};

/// File extensions treated as audio/video during media walks.
///
/// This list intentionally mirrors the formats that batchalign's ASR and
/// forced-alignment engines can consume. Adding an extension here is enough
/// to make it discoverable via `resolve()`, `list_files()`, and the
/// `/media/list` endpoint -- no engine changes are needed.
const MEDIA_EXTENSIONS: &[&str] = &[".wav", ".mp3", ".mp4", ".m4a", ".flac", ".ogg", ".aac"];

/// Walk cache TTL in seconds.
///
/// Media roots are often NFS-mounted volumes with tens of thousands of files.
/// Caching walk results for 60 seconds avoids re-scanning on every request
/// while still picking up new files within a reasonable window. The cache is
/// per-root, so adding a new root does not invalidate existing entries.
const CACHE_TTL_SECS: u64 = 60;

/// One discovered media file: the directory it lives in and its filename.
#[derive(Debug, Clone)]
pub struct MediaEntry {
    /// Parent directory path (e.g. `/data/media/subdir`).
    pub dir_path: String,
    /// Filename with extension (e.g. `interview.wav`).
    pub filename: String,
}

impl MediaEntry {
    /// Full path to the media file.
    ///
    /// Uses `Path::join` for platform-safe path construction (correct on
    /// Windows where the separator is `\`, not `/`).
    pub fn full_path(&self) -> String {
        std::path::Path::new(&self.dir_path)
            .join(&self.filename)
            .to_string_lossy()
            .into_owned()
    }
}

/// Cached walk results: `(timestamp, entries)`.
type CacheEntry = (Instant, Vec<MediaEntry>);

/// Locates audio/video files across configured media roots and named
/// media mappings.
///
/// Provides two resolution strategies:
/// - **Root search** (`resolve`/`list_files`): walks all `media_roots`
///   recursively, matching by exact filename or stem (e.g., `sample` finds
///   `sample.wav`).
/// - **Mapped search** (`resolve_mapped`/`list_mapped`): restricts to a
///   specific `mapping_root/subdir` path with traversal protection.
///
/// Walk results are cached in a concurrent `DashMap` with a 60-second TTL
/// to avoid rescanning large NFS volumes on every request. The cache can be
/// invalidated per-root or globally via [`invalidate`](Self::invalidate).
pub struct MediaResolver {
    cache: DashMap<String, CacheEntry>,
}

impl MediaResolver {
    /// Create a resolver with an empty walk cache.
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Invalidate a specific root or the entire cache.
    pub fn invalidate(&self, root_dir: Option<&str>) {
        match root_dir {
            Some(root) => {
                self.cache.remove(root);
            }
            None => {
                self.cache.clear();
            }
        }
    }

    /// Walk a directory tree and return discovered media entries.
    ///
    /// Results are cached for `CACHE_TTL_SECS` seconds.
    fn walk_media(&self, root_dir: &str) -> Vec<MediaEntry> {
        let now = Instant::now();

        // Check cache
        if let Some(entry) = self.cache.get(root_dir) {
            let (ts, ref cached) = *entry;
            if now.duration_since(ts).as_secs() < CACHE_TTL_SECS {
                return cached.clone();
            }
        }

        // Cache miss — do the walk
        let mut entries = Vec::new();
        let root_path = Path::new(root_dir);
        if root_path.is_dir() {
            for entry in walkdir::WalkDir::new(root_path) {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        warn!(
                            root = %root_dir,
                            error = %error,
                            "Skipping unreadable media walk entry"
                        );
                        continue;
                    }
                };
                if entry.file_type().is_file() {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    if is_media_extension(&filename) {
                        let Some(parent) = entry.path().parent() else {
                            warn!(
                                path = %entry.path().display(),
                                "Skipping media file without parent directory"
                            );
                            continue;
                        };
                        let dir_path = parent.to_string_lossy().to_string();
                        entries.push(MediaEntry { dir_path, filename });
                    }
                }
            }
        } else if root_path.exists() {
            warn!(root = %root_dir, "Configured media root is not a directory");
        } else {
            debug!(root = %root_dir, "Configured media root does not exist");
        }

        self.cache
            .insert(root_dir.to_string(), (now, entries.clone()));
        entries
    }

    /// Find a media file matching `name` under the configured media_roots.
    ///
    /// Search strategy:
    /// 1. Exact filename match (recursive) in each root
    /// 2. Stem match — if `name` has no media extension, try matching stem
    ///    against files with known audio/video extensions
    ///
    /// Returns the absolute path to the first match, or None.
    pub fn resolve(&self, name: &str, media_roots: &[String]) -> Option<String> {
        if name.is_empty() {
            return None;
        }

        let name_path = Path::new(name);
        let name_stem = name_path.file_stem().unwrap_or_default().to_string_lossy();
        let name_has_media_ext = name_path
            .extension()
            .map(|e| {
                let ext = format!(".{}", e.to_string_lossy().to_lowercase());
                MEDIA_EXTENSIONS.contains(&ext.as_str())
            })
            .unwrap_or(false);

        for root in media_roots {
            let entries = self.walk_media(root);

            // Pass 1: exact filename match
            for entry in &entries {
                if entry.filename == name {
                    return Some(entry.full_path());
                }
            }

            // Pass 2: stem match with known extensions (only if name isn't already exact)
            if !name_has_media_ext {
                for entry in &entries {
                    let fp = Path::new(&entry.filename);
                    let f_stem = fp.file_stem().unwrap_or_default().to_string_lossy();
                    if f_stem == name_stem && is_media_extension(&entry.filename) {
                        return Some(entry.full_path());
                    }
                }
            }
        }

        None
    }

    /// Find a media file in a specific mapped directory.
    ///
    /// Uses a deterministic path: `mapping_root / subdir`.
    pub fn resolve_mapped(&self, name: &str, mapping_root: &str, subdir: &str) -> Option<String> {
        if name.is_empty() {
            return None;
        }

        let search_dir = PathBuf::from(mapping_root).join(subdir);
        let search_dir = match search_dir.canonicalize() {
            Ok(p) => p,
            Err(_) => return None,
        };
        let root_resolved = match PathBuf::from(mapping_root).canonicalize() {
            Ok(p) => p,
            Err(_) => return None,
        };

        if !search_dir.starts_with(&root_resolved) {
            return None; // path traversal
        }
        if !search_dir.is_dir() {
            return None;
        }

        let name_path = Path::new(name);
        let name_stem = name_path.file_stem().unwrap_or_default().to_string_lossy();
        let name_has_media_ext = name_path
            .extension()
            .map(|e| {
                let ext = format!(".{}", e.to_string_lossy().to_lowercase());
                MEDIA_EXTENSIONS.contains(&ext.as_str())
            })
            .unwrap_or(false);

        // Pass 1: exact filename match (flat)
        let candidate = search_dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }

        // Pass 2: stem match with known extensions
        if !name_has_media_ext {
            let mut exts: Vec<&&str> = MEDIA_EXTENSIONS.iter().collect();
            exts.sort();
            for ext in exts {
                let candidate = search_dir.join(format!("{name_stem}{ext}"));
                if candidate.is_file() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }

        None
    }

    /// List audio/video filenames under a mapping root + subdir.
    pub fn list_mapped(&self, mapping_root: &str, subdir: &str) -> Vec<String> {
        let search_dir = if subdir.is_empty() {
            PathBuf::from(mapping_root)
        } else {
            PathBuf::from(mapping_root).join(subdir)
        };
        let search_dir = match search_dir.canonicalize() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };
        let root_resolved = match PathBuf::from(mapping_root).canonicalize() {
            Ok(p) => p,
            Err(_) => return Vec::new(),
        };

        if !search_dir.starts_with(&root_resolved) {
            return Vec::new(); // path traversal
        }
        if !search_dir.is_dir() {
            return Vec::new();
        }

        let entries = self.walk_media(&search_dir.to_string_lossy());
        let mut names: Vec<String> = entries.into_iter().map(|e| e.filename).collect();
        names.sort();
        names.dedup();
        names
    }

    /// List audio/video filenames available under media_roots.
    pub fn list_files(&self, media_roots: &[String], subdir: &str) -> Vec<String> {
        let mut found = Vec::new();

        for root in media_roots {
            let search_dir = if subdir.is_empty() {
                root.clone()
            } else {
                let search_path = PathBuf::from(root).join(subdir);
                let search_resolved = match search_path.canonicalize() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let root_resolved = match PathBuf::from(root).canonicalize() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if !search_resolved.starts_with(&root_resolved) {
                    continue; // path traversal
                }
                search_resolved.to_string_lossy().to_string()
            };

            let entries = self.walk_media(&search_dir);
            for entry in entries {
                found.push(entry.filename);
            }
        }

        found.sort();
        found.dedup();
        found
    }
}

impl Default for MediaResolver {
    fn default() -> Self {
        Self::new()
    }
}

fn is_media_extension(filename: &str) -> bool {
    let lower = filename.to_lowercase();
    MEDIA_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_media_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("audio.wav"), b"fake wav").unwrap();
        fs::write(root.join("video.mp4"), b"fake mp4").unwrap();
        fs::write(root.join("song.mp3"), b"fake mp3").unwrap();
        fs::write(root.join("notes.txt"), b"not media").unwrap();

        let sub = root.join("subdir");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("deep.flac"), b"fake flac").unwrap();
        dir
    }

    #[test]
    fn exact_match() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let roots = vec![dir.path().to_string_lossy().to_string()];

        let result = resolver.resolve("audio.wav", &roots);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("audio.wav"));
    }

    #[test]
    fn stem_match() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let roots = vec![dir.path().to_string_lossy().to_string()];

        // Search by stem without extension
        let result = resolver.resolve("audio", &roots);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("audio.wav"));
    }

    #[test]
    fn not_found() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let roots = vec![dir.path().to_string_lossy().to_string()];

        let result = resolver.resolve("nonexistent.wav", &roots);
        assert!(result.is_none());
    }

    #[test]
    fn list_files_all() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let roots = vec![dir.path().to_string_lossy().to_string()];

        let files = resolver.list_files(&roots, "");
        assert!(files.contains(&"audio.wav".to_string()));
        assert!(files.contains(&"video.mp4".to_string()));
        assert!(files.contains(&"deep.flac".to_string()));
        assert!(!files.contains(&"notes.txt".to_string()));
    }

    #[test]
    fn cache_invalidation() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let root = dir.path().to_string_lossy().to_string();
        let roots = vec![root.clone()];

        // Populate cache
        let _ = resolver.resolve("audio.wav", &roots);
        assert!(!resolver.cache.is_empty());

        // Invalidate
        resolver.invalidate(Some(&root));
        assert!(resolver.cache.is_empty());
    }

    #[test]
    fn mapped_resolve() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let root = dir.path().to_string_lossy().to_string();

        let result = resolver.resolve_mapped("deep.flac", &root, "subdir");
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("deep.flac"));
    }

    #[test]
    fn mapped_stem_match() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let root = dir.path().to_string_lossy().to_string();

        let result = resolver.resolve_mapped("deep", &root, "subdir");
        assert!(result.is_some());
    }

    #[test]
    fn list_mapped() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let root = dir.path().to_string_lossy().to_string();

        let files = resolver.list_mapped(&root, "subdir");
        assert_eq!(files, vec!["deep.flac"]);
    }

    #[test]
    fn empty_name_returns_none() {
        let resolver = MediaResolver::new();
        assert!(resolver.resolve("", &[]).is_none());
        assert!(resolver.resolve_mapped("", "/tmp", "").is_none());
    }

    #[test]
    fn cache_hit_avoids_rewalk() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let root = dir.path().to_string_lossy().to_string();
        let roots = vec![root.clone()];

        // First call populates the cache.
        let files1 = resolver.list_files(&roots, "");
        assert!(!files1.is_empty());

        // Add a new file to disk.
        fs::write(dir.path().join("new.wav"), b"new wav").unwrap();

        // Second call should use cached results (within TTL) and NOT see the new file.
        let files2 = resolver.list_files(&roots, "");
        assert_eq!(files1, files2, "cache hit should return same results");
        assert!(
            !files2.contains(&"new.wav".to_string()),
            "new file should not appear until cache expires"
        );
    }

    #[test]
    fn invalidation_reveals_new_files() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let root = dir.path().to_string_lossy().to_string();
        let roots = vec![root.clone()];

        // Populate cache.
        let files1 = resolver.list_files(&roots, "");
        assert!(!files1.contains(&"new.wav".to_string()));

        // Add a file and invalidate.
        fs::write(dir.path().join("new.wav"), b"new wav").unwrap();
        resolver.invalidate(Some(&root));

        // Now the new file should appear.
        let files2 = resolver.list_files(&roots, "");
        assert!(
            files2.contains(&"new.wav".to_string()),
            "after invalidation, new file should be discovered"
        );
    }

    #[test]
    fn global_invalidation_clears_all_roots() {
        let dir1 = setup_media_dir();
        let dir2 = tempfile::tempdir().unwrap();
        fs::write(dir2.path().join("track.mp3"), b"mp3").unwrap();

        let resolver = MediaResolver::new();
        let root1 = dir1.path().to_string_lossy().to_string();
        let root2 = dir2.path().to_string_lossy().to_string();
        let roots = vec![root1, root2];

        // Populate cache for both roots.
        let _ = resolver.list_files(&roots, "");
        assert!(resolver.cache.len() >= 2, "both roots should be cached");

        // Global invalidation.
        resolver.invalidate(None);
        assert!(resolver.cache.is_empty(), "all entries should be cleared");
    }

    #[test]
    fn nonexistent_root_returns_empty() {
        let resolver = MediaResolver::new();
        let roots = vec!["/nonexistent/path/that/does/not/exist".to_string()];

        let files = resolver.list_files(&roots, "");
        assert!(files.is_empty(), "non-existent root should yield no files");
    }

    #[test]
    fn non_media_files_excluded() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let roots = vec![dir.path().to_string_lossy().to_string()];

        let files = resolver.list_files(&roots, "");
        assert!(
            !files.contains(&"notes.txt".to_string()),
            ".txt should not appear in media listing"
        );
        assert!(
            files.contains(&"audio.wav".to_string()),
            ".wav should appear"
        );
        assert!(
            files.contains(&"video.mp4".to_string()),
            ".mp4 should appear"
        );
    }

    #[test]
    fn mapped_traversal_blocked() {
        let dir = setup_media_dir();
        let resolver = MediaResolver::new();
        let root = dir.path().to_string_lossy().to_string();

        // Attempting path traversal via ".." should return None.
        let result = resolver.resolve_mapped("audio.wav", &root, "../../../etc");
        assert!(result.is_none(), "path traversal should be blocked");
    }
}

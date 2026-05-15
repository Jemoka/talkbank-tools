//! Provenance-tracking path newtypes for the job submission system.
//!
//! Paths in batchalign3 cross machine boundaries: a client submits paths
//! from their filesystem, the server resolves them on its own filesystem.
//! These newtypes make the provenance explicit so the compiler prevents
//! accidentally reading a client path on the server's filesystem.
//!
//! # Types
//!
//! - [`ClientPath`] — path on the submitting client's machine
//! - [`ServerPath`] — path on the server's machine (safe for I/O)
//! - [`RepoRelativePath`] — path relative to a data repo root
//! - [`MediaMappingKey`] — config key for media volume lookup

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ClientPath — from the submitting client's filesystem
// ---------------------------------------------------------------------------

/// A path on the submitting client's filesystem.
///
/// The server MUST NOT do filesystem I/O on this directly — it's metadata
/// only.  The only way to convert it to a [`ServerPath`] for I/O is via
/// [`assume_shared_filesystem`](Self::assume_shared_filesystem), which
/// requires the caller to verify that the server shares the client's
/// filesystem (paths_mode with a local daemon).
///
/// Deliberately does NOT implement `AsRef<Path>` to prevent accidental
/// filesystem operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(transparent)]
pub struct ClientPath(String);

impl ClientPath {
    /// Create a new client path.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The raw path string as submitted by the client.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert to a [`ServerPath`] by asserting that the server shares
    /// the client's filesystem.
    ///
    /// This is the ONLY sanctioned conversion from client to server path.
    /// It is valid when `paths_mode` is true and the server is a local
    /// daemon on the same machine as the client.
    ///
    /// Callers must verify the shared-filesystem precondition.  Using
    /// this on a remote client's path will produce a [`ServerPath`] that
    /// points to a nonexistent location on the server.
    pub fn assume_shared_filesystem(&self) -> ServerPath {
        ServerPath(PathBuf::from(&self.0))
    }

    /// Check whether this client path contains a given path component.
    ///
    /// Used for inferring the media mapping key from a client's source
    /// directory path.
    pub fn contains_component(&self, component: &str) -> bool {
        let pattern = format!("/{component}/");
        let suffix = format!("/{component}");
        self.0.contains(&pattern) || self.0.ends_with(&suffix)
    }

    /// Append a relative path component to this client path.
    ///
    /// Returns a new [`ClientPath`] with `relative` joined under this path.
    /// This is the only sanctioned way to extend a client path — it keeps the
    /// result typed as a `ClientPath`, preventing accidental I/O on the server.
    ///
    /// If `relative` is empty, returns a clone of `self`.
    ///
    /// # When to use
    ///
    /// When `source_dir` is a top-level data root (e.g.
    /// `/Volumes/data-drive/talkbank-data`) and filenames carry a
    /// repo-key prefix (`aphasia-data/English/...`), join the parent of the
    /// filename onto `source_dir` before calling `infer_media_mapping` so the
    /// repo key becomes visible in the path.
    ///
    /// # Type safety
    ///
    /// The result is a [`ClientPath`], not a [`ServerPath`].  The compiler
    /// will reject any attempt to use it for direct filesystem I/O:
    ///
    /// ```compile_fail
    /// use std::path::Path;
    /// use batchalign_types::paths::ClientPath;
    /// let base = ClientPath::new("/data");
    /// let joined = base.join(Path::new("repo/sub"));
    /// let _: &Path = joined.as_ref(); // ERROR: ClientPath has no AsRef<Path>
    /// ```
    pub fn join(&self, relative: impl AsRef<Path>) -> Self {
        let r = relative.as_ref();
        if r.as_os_str().is_empty() {
            return self.clone();
        }
        let r_str = r.to_string_lossy();
        if self.0.is_empty() {
            Self(r_str.into_owned())
        } else {
            Self(format!("{}/{}", self.0, r_str))
        }
    }

    /// Extract the portion of the path after a given component.
    ///
    /// Returns `Some(suffix)` where `suffix` is everything after `/{component}/`
    /// in the path, or `Some("")` if the path ends with `/{component}` (i.e. the
    /// client directory IS the repo root).  Returns `None` if the component is
    /// not present at all.
    ///
    /// Examples:
    /// - `"/Users/operator/chat-data/slabank-data/French/Newcastle"` → `Some("French/Newcastle")`
    /// - `"/Users/operator/chat-data/aphasia-data"` → `Some("")`  (at repo root)
    /// - `"/Users/operator/Desktop/other"` → `None`
    pub fn suffix_after_component(&self, component: &str) -> Option<&str> {
        let interior = format!("/{component}/");
        let terminal = format!("/{component}");
        if let Some((_, suffix)) = self.0.split_once(&interior) {
            Some(suffix)
        } else if self.0.ends_with(&terminal) {
            Some("")
        } else {
            None
        }
    }
}

impl std::fmt::Display for ClientPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ClientPath {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ClientPath {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// ServerPath — on the server's filesystem (safe for I/O)
// ---------------------------------------------------------------------------

/// A path on the server's filesystem.
///
/// Safe for the server to read, write, and check existence.
/// Implements `AsRef<Path>` so it can be passed directly to
/// `tokio::fs::read_to_string`, `std::fs::write`, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServerPath(PathBuf);

impl ServerPath {
    /// Create from an absolute path on the server.
    pub fn new(p: impl Into<PathBuf>) -> Self {
        Self(p.into())
    }

    /// The underlying `Path` reference — safe for filesystem I/O.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Join a relative component to produce a new server path.
    pub fn join(&self, component: impl AsRef<Path>) -> Self {
        Self(self.0.join(component))
    }

    /// The raw string representation.
    pub fn as_str(&self) -> &str {
        self.0.to_str().unwrap_or("")
    }
}

impl AsRef<Path> for ServerPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for ServerPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl From<PathBuf> for ServerPath {
    fn from(p: PathBuf) -> Self {
        Self(p)
    }
}

impl Default for ServerPath {
    fn default() -> Self {
        Self(PathBuf::new())
    }
}

impl From<&str> for ServerPath {
    fn from(s: &str) -> Self {
        Self(PathBuf::from(s))
    }
}

impl From<String> for ServerPath {
    fn from(s: String) -> Self {
        Self(PathBuf::from(s))
    }
}

// ---------------------------------------------------------------------------
// RepoRelativePath — relative to a data repo root
// ---------------------------------------------------------------------------

/// A path relative to a data repo root (e.g. `"French/Newcastle/Photos/13"`).
///
/// Valid on any machine that has the repo cloned, but must be combined
/// with a server root path to produce a [`ServerPath`] for I/O.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(transparent)]
pub struct RepoRelativePath(String);

impl RepoRelativePath {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Combine with a server root to produce an absolute server path.
    pub fn resolve_on_server(&self, root: &ServerPath) -> ServerPath {
        if self.0.is_empty() {
            root.clone()
        } else {
            root.join(&self.0)
        }
    }

    /// Append a subdirectory.
    pub fn join(&self, sub: &str) -> Self {
        if self.0.is_empty() {
            Self(sub.to_owned())
        } else if sub.is_empty() {
            self.clone()
        } else {
            Self(format!("{}/{sub}", self.0))
        }
    }
}

impl std::fmt::Display for RepoRelativePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for RepoRelativePath {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for RepoRelativePath {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// MediaMappingKey — config key, not a path
// ---------------------------------------------------------------------------

/// Key into `ServerConfig.media_mappings` (e.g. `"slabank-data"`).
///
/// Not a filesystem path — it's a logical name that maps to a
/// [`ServerPath`] via the server's configuration.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Default,
    Serialize,
    Deserialize,
    utoipa::ToSchema,
)]
#[serde(transparent)]
pub struct MediaMappingKey(String);

impl MediaMappingKey {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MediaMappingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for MediaMappingKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MediaMappingKey {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Media mapping inference — pure function, fully typed
// ---------------------------------------------------------------------------

/// Infer the media mapping from a client's source directory path.
///
/// Checks whether any key in `mappings` appears as a path component in
/// `client_dir`.  If found, returns the key, server root, and the
/// repo-relative subdir (everything after the key in the client path).
///
/// Example:
/// - client_dir: `/Users/operator/chat-data/slabank-data/French/Newcastle/Photos`
/// - mappings: `{"slabank-data" → "/Volumes/Other/slabank"}`
/// - returns: `("slabank-data", "/Volumes/Other/slabank", "French/Newcastle/Photos")`
pub fn infer_media_mapping<'a>(
    client_dir: &ClientPath,
    mappings: impl IntoIterator<Item = (&'a MediaMappingKey, &'a ServerPath)>,
) -> Option<(MediaMappingKey, ServerPath, RepoRelativePath)> {
    for (key, root) in mappings {
        if let Some(suffix) = client_dir.suffix_after_component(key.as_str()) {
            return Some((key.clone(), root.clone(), RepoRelativePath::new(suffix)));
        }
    }
    None
}

// Additional impls needed for serde(default) and validation

impl ClientPath {
    /// Whether the path is empty (not set).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl MediaMappingKey {
    /// Whether the key is empty (not set).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn client_path_does_not_impl_as_ref_path() {
        // This is a compile-time guarantee — ClientPath deliberately
        // does NOT implement AsRef<Path>.  If this test compiles,
        // the guarantee holds.  (We verify by using ServerPath's
        // AsRef<Path> and NOT having a corresponding ClientPath call.)
        let server = ServerPath::new("/tmp/test");
        let _: &Path = server.as_ref(); // OK — ServerPath impls AsRef<Path>
        // ClientPath does NOT have this — uncomment to verify compile error:
        // let client = ClientPath::new("/tmp/test");
        // let _: &Path = client.as_ref(); // Would not compile
    }

    #[test]
    fn server_path_as_path() {
        let sp = ServerPath::new("/Volumes/Other/slabank");
        assert_eq!(sp.as_path(), Path::new("/Volumes/Other/slabank"));
    }

    #[test]
    fn client_path_assume_shared_filesystem() {
        let cp = ClientPath::new("/Users/operator/chat-data/file.cha");
        let sp = cp.assume_shared_filesystem();
        assert_eq!(
            sp.as_path(),
            Path::new("/Users/operator/chat-data/file.cha")
        );
    }

    #[test]
    fn client_path_contains_component() {
        let cp = ClientPath::new("/Users/operator/chat-data/slabank-data/French/Newcastle");
        assert!(cp.contains_component("slabank-data"));
        assert!(!cp.contains_component("childes-data"));
        assert!(!cp.contains_component("slabank")); // partial match rejected
    }

    #[test]
    fn client_path_suffix_after_component() {
        let cp = ClientPath::new("/Users/operator/chat-data/slabank-data/French/Newcastle/Photos");
        assert_eq!(
            cp.suffix_after_component("slabank-data"),
            Some("French/Newcastle/Photos")
        );
        assert_eq!(cp.suffix_after_component("childes-data"), None);
    }

    #[test]
    fn repo_relative_path_resolve_on_server() {
        let root = ServerPath::new("/Volumes/Other/slabank");
        let rel = RepoRelativePath::new("French/Newcastle/Photos/13");
        let resolved = rel.resolve_on_server(&root);
        assert_eq!(
            resolved.as_path(),
            Path::new("/Volumes/Other/slabank/French/Newcastle/Photos/13")
        );
    }

    #[test]
    fn repo_relative_path_join() {
        let base = RepoRelativePath::new("French/Newcastle/Photos");
        let joined = base.join("13");
        assert_eq!(joined.as_str(), "French/Newcastle/Photos/13");

        let empty = RepoRelativePath::default();
        assert_eq!(empty.join("13").as_str(), "13");
    }

    #[test]
    fn media_mapping_key_serde_roundtrip() {
        let key = MediaMappingKey::new("slabank-data");
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, "\"slabank-data\"");
        let back: MediaMappingKey = serde_json::from_str(&json).unwrap();
        assert_eq!(back, key);
    }

    // -----------------------------------------------------------------------
    // an operator's bug: infer media mapping from client path
    // -----------------------------------------------------------------------

    #[test]
    fn infer_media_mapping_from_client_path() {
        let client_dir =
            ClientPath::new("/Users/operator/chat-data/slabank-data/French/Newcastle/Photos");
        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("slabank-data"),
            ServerPath::new("/Volumes/Other/slabank"),
        );

        let result = infer_media_mapping(&client_dir, &mappings);
        assert!(result.is_some(), "Should infer slabank-data from path");

        let (key, root, subdir) = result.unwrap();
        assert_eq!(key.as_str(), "slabank-data");
        assert_eq!(root.as_path(), Path::new("/Volumes/Other/slabank"));
        assert_eq!(subdir.as_str(), "French/Newcastle/Photos");
    }

    #[test]
    fn infer_media_mapping_no_match() {
        let client_dir = ClientPath::new("/Users/operator/Desktop/random");
        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("slabank-data"),
            ServerPath::new("/Volumes/Other/slabank"),
        );

        assert!(infer_media_mapping(&client_dir, &mappings).is_none());
    }

    #[test]
    fn infer_media_mapping_multiple_keys() {
        let client_dir =
            ClientPath::new("/Users/operator/chat-data/childes-eng-na-data/MacWhinney/01");
        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("slabank-data"),
            ServerPath::new("/Volumes/Other/slabank"),
        );
        mappings.insert(
            MediaMappingKey::new("childes-eng-na-data"),
            ServerPath::new("/Volumes/CHILDES/CHILDES"),
        );

        let (key, root, subdir) = infer_media_mapping(&client_dir, &mappings).unwrap();
        assert_eq!(key.as_str(), "childes-eng-na-data");
        assert_eq!(root.as_path(), Path::new("/Volumes/CHILDES/CHILDES"));
        assert_eq!(subdir.as_str(), "MacWhinney/01");
    }

    /// Regression: when the client's source_dir IS the repo root (ends with the
    /// repo component, no trailing slash), suffix_after_component must return
    /// Some("") so infer_media_mapping can still fire.
    ///
    /// This happens when a --file-list batch spans multiple subdirectories
    /// (e.g. Cantonese/ and Croatian/): infer_base_dir finds the common ancestor
    /// which is the repo root itself, not a subdirectory within it.
    #[test]
    fn suffix_after_component_at_repo_root() {
        let cp = ClientPath::new("/Users/operator/chat-data/aphasia-data");
        assert_eq!(
            cp.suffix_after_component("aphasia-data"),
            Some(""),
            "path ending with the component should yield empty suffix, not None"
        );
    }

    #[test]
    fn infer_media_mapping_when_source_dir_is_repo_root() {
        // Regression: source_dir at the repo root (e.g.
        // /Users/operator/chat-data/aphasia-data). Previously
        // infer_media_mapping returned None here, causing
        // "Cannot find audio file" for all aphasia-data align jobs submitted
        // from a --file-list spanning multiple subdirectories.
        let client_dir = ClientPath::new("/Users/operator/chat-data/aphasia-data");
        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("aphasia-data"),
            ServerPath::new("/Users/operator/media/aphasia"),
        );

        let result = infer_media_mapping(&client_dir, &mappings);
        assert!(
            result.is_some(),
            "should match even when client dir is the repo root"
        );

        let (key, root, subdir) = result.unwrap();
        assert_eq!(key.as_str(), "aphasia-data");
        assert_eq!(root.as_path(), Path::new("/Users/operator/media/aphasia"));
        assert_eq!(
            subdir.as_str(),
            "",
            "repo_subdir should be empty at the root"
        );

        // Simulate the rest of the resolution: file is Cantonese/Protocol/HKU/PWA/A023.cha
        let file_subdir = "Cantonese/Protocol/HKU/PWA";
        let full_subdir = subdir.join(file_subdir);
        assert_eq!(full_subdir.as_str(), "Cantonese/Protocol/HKU/PWA");

        let search_dir = full_subdir.resolve_on_server(&root);
        assert_eq!(
            search_dir.as_path(),
            Path::new("/Users/operator/media/aphasia/Cantonese/Protocol/HKU/PWA")
        );
    }

    #[test]
    fn full_media_resolution_brians_scenario() {
        // Simulate: client_dir = .../slabank-data/French/Newcastle/Photos
        // filename = 13/p08aul13.cha → stem = p08aul13, file_subdir = 13
        // media mapping: slabank-data → /Volumes/Other/slabank
        // Expected search: /Volumes/Other/slabank/French/Newcastle/Photos/13/p08aul13.mp3

        let client_dir =
            ClientPath::new("/Users/operator/chat-data/slabank-data/French/Newcastle/Photos");

        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("slabank-data"),
            ServerPath::new("/Volumes/Other/slabank"),
        );

        let (_, root, repo_subdir) = infer_media_mapping(&client_dir, &mappings).unwrap();

        // File-level subdir from the filename's parent
        let file_subdir = "13";
        let full_subdir = repo_subdir.join(file_subdir);
        assert_eq!(full_subdir.as_str(), "French/Newcastle/Photos/13");

        let search_dir = full_subdir.resolve_on_server(&root);
        assert_eq!(
            search_dir.as_path(),
            Path::new("/Volumes/Other/slabank/French/Newcastle/Photos/13")
        );
    }

    // -----------------------------------------------------------------------
    // ClientPath::join
    // -----------------------------------------------------------------------

    #[test]
    fn client_path_join_appends_relative_component() {
        let base = ClientPath::new("/Volumes/data-drive/talkbank-data");
        let joined = base.join(Path::new("aphasia-data/English/Protocol/APROCSA"));
        assert_eq!(
            joined.as_str(),
            "/Volumes/data-drive/talkbank-data/aphasia-data/English/Protocol/APROCSA"
        );
    }

    #[test]
    fn client_path_join_empty_relative_returns_clone() {
        let base = ClientPath::new("/some/path");
        let joined = base.join(Path::new(""));
        assert_eq!(joined.as_str(), "/some/path");
    }

    #[test]
    fn client_path_join_result_is_client_path_not_server_path() {
        // ClientPath::join returns ClientPath, not ServerPath — the type annotation
        // below enforces that the compiler rejects any implicit widening.
        let base = ClientPath::new("/Volumes/data-drive/talkbank-data");
        let _joined: ClientPath = base.join(Path::new("aphasia-data/English"));
    }

    // -----------------------------------------------------------------------
    // Regression: content-mode --server with top-level data root
    // -----------------------------------------------------------------------

    /// Regression: when `--server` is used the client's `source_dir` is often a
    /// top-level data root like `/Volumes/data-drive/talkbank-data` that does
    /// NOT contain the repo key.  The filenames carry the key as a prefix (e.g.
    /// `aphasia-data/English/Protocol/APROCSA/2256_T4.cha`).  Passing only
    /// `source_dir` to `infer_media_mapping` returned `None` → every file failed
    /// with "Cannot find audio file".
    ///
    /// Fix: join `source_dir` with `parent(filename)` via `ClientPath::join`
    /// before inference.  The result still stays a `ClientPath` (safe —
    /// no I/O), but the repo key is now visible to `infer_media_mapping`.
    #[test]
    fn infer_media_mapping_content_mode_top_level_data_root() {
        let source_dir = ClientPath::new("/Volumes/data-drive/talkbank-data");
        let filename = "aphasia-data/English/Protocol/APROCSA/2256_T4.cha";

        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("aphasia-data"),
            ServerPath::new("/Users/operator/media/aphasia"),
        );

        let file_parent = Path::new(filename)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap();
        let infer_client = source_dir.join(file_parent);
        assert_eq!(
            infer_client.as_str(),
            "/Volumes/data-drive/talkbank-data/aphasia-data/English/Protocol/APROCSA"
        );

        let result = infer_media_mapping(&infer_client, &mappings);
        assert!(result.is_some(), "repo key must be found after join");

        let (key, root, repo_subdir) = result.unwrap();
        assert_eq!(key.as_str(), "aphasia-data");
        assert_eq!(root.as_path(), Path::new("/Users/operator/media/aphasia"));
        // repo_subdir is the correct path within the media volume — no further
        // joining needed.
        assert_eq!(repo_subdir.as_str(), "English/Protocol/APROCSA");

        let search_dir = repo_subdir.resolve_on_server(&root);
        assert_eq!(
            search_dir.as_path(),
            Path::new("/Users/operator/media/aphasia/English/Protocol/APROCSA")
        );
    }

    /// Variant of the content-mode regression: file at repo root (no subdir).
    /// e.g. source_dir="/Volumes/data-drive/talkbank-data",
    ///      filename="aphasia-data/A023.cha"
    #[test]
    fn infer_media_mapping_content_mode_file_directly_in_repo() {
        let source_dir = ClientPath::new("/Volumes/data-drive/talkbank-data");
        let filename = "aphasia-data/A023.cha";

        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("aphasia-data"),
            ServerPath::new("/Users/operator/media/aphasia"),
        );

        let file_parent = Path::new(filename)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap(); // parent = "aphasia-data"

        let infer_client = source_dir.join(file_parent);
        // infer_client ends with "aphasia-data" — suffix_after_component returns ""
        assert_eq!(
            infer_client.as_str(),
            "/Volumes/data-drive/talkbank-data/aphasia-data"
        );

        let result = infer_media_mapping(&infer_client, &mappings);
        assert!(result.is_some());
        let (_, root, repo_subdir) = result.unwrap();
        // repo_subdir is "" — file is at the root of the media volume
        assert_eq!(repo_subdir.as_str(), "");
        let search_dir = repo_subdir.resolve_on_server(&root);
        assert_eq!(
            search_dir.as_path(),
            Path::new("/Users/operator/media/aphasia")
        );
    }

    /// Ensure an operator's existing scenario still works with the fixed join-first
    /// approach: source_dir already embeds the full subdir, filename has a
    /// shallow dir component.
    #[test]
    fn infer_media_mapping_brians_scenario_join_first() {
        let source_dir =
            ClientPath::new("/Users/operator/chat-data/slabank-data/French/Newcastle/Photos");
        let filename = "13/p08aul13.cha";

        let mut mappings = BTreeMap::new();
        mappings.insert(
            MediaMappingKey::new("slabank-data"),
            ServerPath::new("/Volumes/Other/slabank"),
        );

        let file_parent = Path::new(filename)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap(); // "13"

        let infer_client = source_dir.join(file_parent);
        // → ".../slabank-data/French/Newcastle/Photos/13"

        let result = infer_media_mapping(&infer_client, &mappings);
        assert!(result.is_some());
        let (key, root, repo_subdir) = result.unwrap();
        assert_eq!(key.as_str(), "slabank-data");
        assert_eq!(root.as_path(), Path::new("/Volumes/Other/slabank"));
        assert_eq!(repo_subdir.as_str(), "French/Newcastle/Photos/13");

        let search_dir = repo_subdir.resolve_on_server(&root);
        assert_eq!(
            search_dir.as_path(),
            Path::new("/Volumes/Other/slabank/French/Newcastle/Photos/13")
        );
    }
}

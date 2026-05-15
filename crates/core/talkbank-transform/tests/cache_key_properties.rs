//! Property-based tests for cache key determinism.
//!
//! The cache utilities in `talkbank-transform` compute keys from file paths
//! and content hashes. These tests verify that the hashing is deterministic
//! and that distinct inputs produce distinct keys (with high probability).

use proptest::prelude::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Reproduce the cache key computation from `cache_utils::get_cache_key_with_suffix`.
///
/// We inline the algorithm here rather than importing the private function,
/// since the property tests verify the mathematical properties of the approach
/// rather than the module's exact API.
fn compute_cache_key(path: &str, suffix: &str) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    suffix.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

proptest! {
    /// Hashing the same (path, suffix) pair twice produces the same key.
    ///
    /// `DefaultHasher` is deterministic within a single process (though not
    /// across Rust versions). This test verifies that repeated calls with
    /// identical inputs produce identical output.
    #[test]
    fn same_content_same_key(
        path in "[a-zA-Z0-9/_.-]{1,100}",
        suffix in "[a-zA-Z0-9_-]{0,20}"
    ) {
        let key1 = compute_cache_key(&path, &suffix);
        let key2 = compute_cache_key(&path, &suffix);
        prop_assert_eq!(
            &key1, &key2,
            "Same inputs must produce same key: path={:?}, suffix={:?}",
            path, suffix
        );
    }

    /// Hashing different paths (with the same suffix) produces different keys.
    ///
    /// This is a probabilistic property: hash collisions are possible but
    /// astronomically unlikely for the 64-bit `DefaultHasher`. We filter to
    /// ensure the paths actually differ.
    #[test]
    fn different_content_different_key(
        path1 in "[a-zA-Z0-9/_.-]{1,100}",
        path2 in "[a-zA-Z0-9/_.-]{1,100}",
        suffix in "[a-zA-Z0-9_-]{0,20}"
    ) {
        // Only test when inputs actually differ.
        prop_assume!(path1 != path2);

        let key1 = compute_cache_key(&path1, &suffix);
        let key2 = compute_cache_key(&path2, &suffix);
        prop_assert_ne!(
            &key1, &key2,
            "Different paths should produce different keys (collision): {:?} vs {:?}",
            path1, path2
        );
    }
}

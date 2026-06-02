//! Shared caching utilities for the batchalign runtime.
//!
//! Today this module owns the [`CacheKey`] trait and its helper
//! [`hash_serialized`]; new cache-related primitives that are useful
//! across crates (key namespacing, content fingerprints, on-disk format
//! versioning, etc.) should land here too so there is exactly one place
//! to look for the cache contract.
//!
//! ## Why a trait
//!
//! The result cache is shared across runs and across files. A cache
//! entry is determined by the *computation*: the task, the backend
//! identity, and the content-identifying fields of the input.
//! Routing-only fields (`source_id`, `utterance_id`) MUST NOT
//! participate — two byte-identical inputs that arrive from different
//! files or different utterance slots must collapse to the same key,
//! otherwise the cache silently degrades to per-file storage and we
//! re-run the backend on every copy of a file.
//!
//! Each `*Input` proto opts into a precise field set by implementing
//! [`CacheKey`]. The engine wraps the per-input hash with
//! `(task, backend_name)` so identical content from two different
//! tasks or two different backend versions still misses correctly.

use blake3::Hasher;

/// Mixes content-identifying bytes into `hasher`.
///
/// Implementors choose exactly which fields participate. Implementations
/// MUST be stable: a given (logical) input always produces the same bytes
/// across runs and across hosts. Implementations MUST NOT include fields
/// whose only purpose is to route a result back to its source.
pub trait CacheKey {
    fn hash(&self, hasher: &mut Hasher);
}

/// Helper for `CacheKey` impls: serialize a borrowed view of the
/// content-identifying fields to JSON and fold it into `hasher`.
///
/// Each `*Input` proto defines a tiny private `K<'a>` struct holding
/// references to exactly the fields that participate in cache identity,
/// then calls `hash_serialized(&k, hasher)`. Borrowing avoids cloning
/// audio payloads, and the explicit struct prevents accidental inclusion
/// of routing fields when the proto gains new members.
pub fn hash_serialized<T: serde::Serialize>(value: &T, hasher: &mut Hasher) {
    // serde_json on a serde-derive struct of `Copy`/`String`/`Vec`/etc.
    // fields is infallible for well-formed data; on the unexpected error
    // path we still mix in a stable discriminator so a half-formed input
    // can't collide with a fully-hashed one.
    match serde_json::to_vec(value) {
        Ok(bytes) => hasher.update(&bytes),
        Err(_) => hasher.update(b"\0__cache_key_serialize_failed__\0"),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::asr::LanguageSpec;
    use crate::proto::morphosyntax::MorphosyntaxInput;
    use crate::utils::SourceId;
    use smol_str::SmolStr;

    fn mk(sid: &str, uid: u32, tokens: &[&str]) -> MorphosyntaxInput {
        MorphosyntaxInput {
            source_id: SourceId::try_new(sid).unwrap(),
            utterance_id: uid,
            language: LanguageSpec::Code(SmolStr::new("eng")),
            tokens: tokens.iter().map(|s| s.to_string()).collect(),
            retokenize: false,
            text: tokens.join(" "),
        }
    }

    fn digest(input: &MorphosyntaxInput) -> [u8; 32] {
        let mut h = Hasher::new();
        input.hash(&mut h);
        *h.finalize().as_bytes()
    }

    #[test]
    fn routing_fields_do_not_affect_key() {
        // Identical content from different files / utterance slots must
        // collapse to one entry. This is the whole point of the trait.
        let a = mk("/path/to/a.cha", 0, &["hello", "world"]);
        let b = mk("/totally/different/b.cha", 17, &["hello", "world"]);
        assert_eq!(digest(&a), digest(&b));
    }

    #[test]
    fn content_changes_diverge() {
        let a = mk("/x.cha", 0, &["hello", "world"]);
        let b = mk("/x.cha", 0, &["hello", "world", "more"]);
        assert_ne!(digest(&a), digest(&b));
    }
}

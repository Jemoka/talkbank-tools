//! `%mor` item and tier behavior tests.
//!
//! Focuses on chunk counting, post-clitic expansion, and serialization shape
//! guarantees used by downstream `%gra` alignment logic.

use super::*;
use crate::WriteChat;

/// Builds a single-chunk `%mor` item.
#[test]
fn test_mor_simple_word() {
    let word = MorWord::new(PosCategory::new("pron"), "I");
    let mor = Mor::new(word);

    assert_eq!(mor.count_chunks(), 1);
    assert!(mor.post_clitics.is_empty());
}

/// Adds one post-clitic chunk to a `%mor` item.
#[test]
fn test_mor_with_post_clitic() {
    let main_word = MorWord::new(PosCategory::new("pron"), "it");
    let post_word =
        MorWord::new(PosCategory::new("aux"), "be").with_feature(MorFeature::new("Fin"));

    let mor = Mor::new(main_word).with_post_clitic(post_word);

    assert_eq!(mor.count_chunks(), 2); // main + post-clitic
    assert_eq!(mor.post_clitics.len(), 1);
}

/// Serializes a `%mor` item with inflectional features.
#[test]
fn test_mor_write_chat() {
    let word = MorWord::new(PosCategory::new("noun"), "dog").with_feature(MorFeature::new("Plur"));
    let mor = Mor::new(word);
    assert_eq!(mor.to_chat_string(), "noun|dog-Plur");
}

/// Serializes `%mor` item plus post-clitic in CHAT format.
#[test]
fn test_mor_write_chat_with_post_clitic() {
    let main = MorWord::new(PosCategory::new("pron"), "she")
        .with_feature(MorFeature::new("Prs"))
        .with_feature(MorFeature::new("Nom"))
        .with_feature(MorFeature::new("S3"));
    let post = MorWord::new(PosCategory::new("aux"), "be")
        .with_feature(MorFeature::new("Fin"))
        .with_feature(MorFeature::new("Ind"))
        .with_feature(MorFeature::new("Pres"))
        .with_feature(MorFeature::new("S3"));
    let mor = Mor::new(main).with_post_clitic(post);
    assert_eq!(
        mor.to_chat_string(),
        "pron|she-Prs-Nom-S3~aux|be-Fin-Ind-Pres-S3"
    );
}

/// Preserves feature ordering on `MorWord`.
#[test]
fn test_mor_word_features() {
    let word = MorWord::new(PosCategory::new("verb"), "run")
        .with_feature(MorFeature::new("Fin"))
        .with_feature(MorFeature::new("Ind"))
        .with_feature(MorFeature::new("Pres"))
        .with_feature(MorFeature::new("S3"));

    assert_eq!(word.pos.as_str(), "verb");
    assert_eq!(word.lemma.as_str(), "run");
    assert_eq!(word.features.len(), 4);
    assert_eq!(word.features[0].value(), "Fin");
}

/// Counts total `%mor` chunks across items, clitics, and the trailing terminator.
#[test]
fn test_mor_tier_count_chunks() {
    // word1: 1 chunk, word2 with post-clitic: 2 chunks, terminator: 1 chunk = 4 total
    let word1 = MorWord::new(PosCategory::new("verb"), "go");
    let mor1 = Mor::new(word1);

    let main = MorWord::new(PosCategory::new("pron"), "it");
    let post = MorWord::new(PosCategory::new("aux"), "be");
    let mor2 = Mor::new(main).with_post_clitic(post);

    let tier = MorTier::new_mor(
        vec![mor1, mor2],
        crate::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    assert_eq!(tier.count_chunks(), 4);
}

/// Enumerate the chunk sequence of `pron|it~aux|be noun|cookie .`: the iterator
/// must yield main, post-clitic, main, terminator in that exact order, with
/// the kinds and lemmas the CHAT manual specifies for `%gra` alignment.
///
/// This is the primitive every downstream consumer (LSP, CLI, CLAN) routes
/// through. If this test breaks, expect every `%gra` hover / graph edge /
/// diagnostic to read or render the wrong chunk.
#[test]
fn chunks_expand_items_with_post_clitics_then_terminator() {
    let its = Mor::new(MorWord::new(PosCategory::new("pron"), "it"))
        .with_post_clitic(MorWord::new(PosCategory::new("aux"), "be"));
    let cookie = Mor::new(MorWord::new(PosCategory::new("noun"), "cookie"));
    let tier = MorTier::new_mor(
        vec![its, cookie],
        crate::model::content::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    let chunks: Vec<_> = tier.chunks().collect();
    assert_eq!(chunks.len(), 4);
    assert_eq!(tier.count_chunks(), chunks.len());

    assert_eq!(chunks[0].kind(), MorChunkKind::Main);
    assert_eq!(chunks[0].lemma(), Some("it"));

    assert_eq!(chunks[1].kind(), MorChunkKind::PostClitic);
    assert_eq!(chunks[1].lemma(), Some("be"));

    assert_eq!(chunks[2].kind(), MorChunkKind::Main);
    assert_eq!(chunks[2].lemma(), Some("cookie"));

    assert_eq!(chunks[3].kind(), MorChunkKind::Terminator);
    assert_eq!(chunks[3].lemma(), None);
    assert_eq!(
        chunks[3].terminator().map(|t| t.to_string()),
        Some(".".to_string())
    );
}

/// `chunk_at` indexes into the sequence `chunks()` produces, so the main and
/// its post-clitic share the same `host_item`. This is the property the
/// main↔mor alignment projection relies on (one alignment pair per item).
#[test]
fn chunk_at_resolves_host_item_for_clitic() {
    let its = Mor::new(MorWord::new(PosCategory::new("pron"), "it"))
        .with_post_clitic(MorWord::new(PosCategory::new("aux"), "be"));
    let cookie = Mor::new(MorWord::new(PosCategory::new("noun"), "cookie"));
    let tier = MorTier::new_mor(
        vec![its, cookie],
        crate::model::content::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    let main = tier.chunk_at(0).expect("main chunk");
    let clitic = tier.chunk_at(1).expect("clitic chunk");
    let cookie_chunk = tier.chunk_at(2).expect("cookie chunk");

    // Main and post-clitic point at the same host Mor — this is the invariant
    // downstream projection to the main tier depends on.
    assert!(std::ptr::eq(
        main.host_item().unwrap(),
        clitic.host_item().unwrap(),
    ));
    // A later item has a distinct host.
    assert!(!std::ptr::eq(
        main.host_item().unwrap(),
        cookie_chunk.host_item().unwrap(),
    ));
    // Terminator has no host item.
    assert!(tier.chunk_at(3).unwrap().host_item().is_none());
    // Out-of-range returns None.
    assert!(tier.chunk_at(4).is_none());
}

/// `item_index_of_chunk` collapses main + post-clitic chunks to the same
/// host item, skips the terminator, and rejects out-of-range indices. The
/// LSP's `%gra`-tier highlight handler relies on exactly this mapping to
/// project a chunk-indexed alignment pair back through the main↔mor
/// alignment without silently landing on the wrong word.
#[test]
fn item_index_of_chunk_collapses_clitic_to_host_item() {
    let its = Mor::new(MorWord::new(PosCategory::new("pron"), "it"))
        .with_post_clitic(MorWord::new(PosCategory::new("aux"), "be"));
    let cookie = Mor::new(MorWord::new(PosCategory::new("noun"), "cookie"));
    let tier = MorTier::new_mor(
        vec![its, cookie],
        crate::model::content::Terminator::Period {
            span: crate::Span::DUMMY,
        },
    );

    assert_eq!(tier.item_index_of_chunk(0), Some(0)); // main of it's
    assert_eq!(tier.item_index_of_chunk(1), Some(0)); // post-clitic shares host 0
    assert_eq!(tier.item_index_of_chunk(2), Some(1)); // cookie
    assert_eq!(tier.item_index_of_chunk(3), None); // terminator is tier-level
    assert_eq!(tier.item_index_of_chunk(4), None); // out of range
}

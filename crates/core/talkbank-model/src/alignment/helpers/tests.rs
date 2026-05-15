//! Domain-specific alignment-unit counting tests.
//!
//! These cases lock down domain policy differences (`%mor/%pho/%sin/%wor`) so
//! helper-level behavior remains stable across aligner refactors.

use super::{TierDomain, count_tier_positions};
use crate::Span;
use crate::model::{
    BracketedContent, BracketedItem, Pause, PauseDuration, PhoGroup, ReplacedWord, Replacement,
    Retrace, RetraceKind, Separator, SinGroup, UtteranceContent, Word, WordCategory,
};

/// Confirms `%mor` skips retraced items while `%pho` and `%wor` still count them.
#[test]
fn mor_skips_retrace_content() {
    let word = Word::new_unchecked("hello", "hello");
    let bracketed = BracketedContent::new(vec![BracketedItem::Word(Box::new(word))]);
    let retrace = Retrace::new(bracketed, RetraceKind::Full);
    let items = vec![UtteranceContent::Retrace(Box::new(retrace))];

    assert_eq!(count_tier_positions(&items, TierDomain::Mor), 0);
    assert_eq!(count_tier_positions(&items, TierDomain::Pho), 1);
    assert_eq!(count_tier_positions(&items, TierDomain::Wor), 1);
}

/// Confirms `%mor` follows replacement words but `%wor` follows the original spoken form.
#[test]
fn mor_counts_replacement_words_but_wor_uses_original_surface() {
    let base = Word::new_unchecked("goed", "goed");
    let replacement = Replacement::new(vec![
        Word::new_unchecked("went", "went"),
        Word::new_unchecked("home", "home"),
    ]);
    let replaced = UtteranceContent::ReplacedWord(Box::new(ReplacedWord::new(base, replacement)));

    assert_eq!(
        count_tier_positions(std::slice::from_ref(&replaced), TierDomain::Mor),
        2
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&replaced), TierDomain::Pho),
        1
    );
    // %wor aligns to the originally spoken token, not the correction.
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&replaced), TierDomain::Wor),
        1
    );
}

/// Confirms untranscribed tokens are excluded from both `%mor` and `%wor`,
/// but kept for `%pho`.
///
/// `xxx`/`yyy`/`www` have no known phoneme sequence — CTC forced alignment
/// cannot produce timings for them. Including them in `%wor` contributes no
/// timing information and wastes cursor slots that would shift subsequent word
/// timings. `%pho` still counts them because phonological tier annotation may
/// independently note the presence of an unintelligible vocalization.
#[test]
fn wor_excludes_untranscribed_tokens() {
    for text in &["xxx", "yyy", "www"] {
        let word = Word::new_unchecked(*text, *text);
        let items = vec![UtteranceContent::Word(Box::new(word))];

        // Mor skips untranscribed material - no linguistic content
        assert_eq!(
            count_tier_positions(&items, TierDomain::Mor),
            0,
            "{text}: Mor must skip untranscribed"
        );
        // %wor must also skip untranscribed tokens — no alignment timing is possible
        assert_eq!(
            count_tier_positions(&items, TierDomain::Wor),
            0,
            "{text}: Wor must exclude untranscribed (no alignable phoneme sequence)"
        );
        // Pho still counts the unintelligible vocalization event
        assert_eq!(
            count_tier_positions(&items, TierDomain::Pho),
            1,
            "{text}: Pho must count untranscribed vocalizations"
        );
    }
}

/// Uppercase `XXX` is illegal CHAT (E241) but still represents untranscribed
/// material. The extraction layer must recognize it case-insensitively so that
/// morphotag does not produce a spurious `x|XXX` entry on the `%mor` tier.
#[test]
fn mor_skips_uppercase_untranscribed() {
    for text in &["XXX", "Xxx", "YYY", "Yyy", "WWW", "Www"] {
        let word = Word::new_unchecked(*text, *text);
        let items = vec![UtteranceContent::Word(Box::new(word))];
        assert_eq!(
            count_tier_positions(&items, TierDomain::Mor),
            0,
            "{text} should be non-alignable for Mor (case-insensitive untranscribed)"
        );
    }
}

/// Confirms timestamp-shaped tokens are excluded from `%wor` counts.
#[test]
fn wor_skips_timestamp_tokens() {
    let items = vec![UtteranceContent::Word(Box::new(Word::new_unchecked(
        "100_200", "100_200",
    )))];

    // Timestamp-shaped tokens are %wor alignment metadata, not lexical tokens.
    assert_eq!(count_tier_positions(&items, TierDomain::Wor), 0);
    // Keep existing morphological behavior unchanged.
    assert_eq!(count_tier_positions(&items, TierDomain::Mor), 1);
}

/// Confirms `%mor` counts tag-marker separators (comma/tag/vocative) as alignable.
#[test]
fn mor_counts_tag_markers_including_comma() {
    let comma = UtteranceContent::Separator(Separator::Comma { span: Span::DUMMY });
    let colon = UtteranceContent::Separator(Separator::Colon { span: Span::DUMMY });
    let tag = UtteranceContent::Separator(Separator::Tag { span: Span::DUMMY });
    let vocative = UtteranceContent::Separator(Separator::Vocative { span: Span::DUMMY });

    // Comma counts as tag marker for mor (cm|cm in mor tier)
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&comma), TierDomain::Mor),
        1
    );
    // Colon does not count as tag marker
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&colon), TierDomain::Mor),
        0
    );
    // Tag and vocative count
    assert_eq!(count_tier_positions(&[tag, vocative], TierDomain::Mor), 2);
    // None count for Pho
    assert_eq!(
        count_tier_positions(&[comma.clone(), colon.clone()], TierDomain::Pho),
        0
    );
    assert_eq!(
        count_tier_positions(&[comma.clone(), colon], TierDomain::Wor),
        0
    );
}

/// Confirms retraced groups are skipped in `%mor` but counted for `%pho/%sin/%wor`.
#[test]
fn mor_skips_retrace_group_but_pho_sin_wor_count() {
    // Retraced groups skip ONLY for Mor (no morphological analysis for false starts)
    // but Pho/Sin/Wor count them (the content WAS produced phonologically and gets timed)
    let inner_words = vec![
        BracketedItem::Word(Box::new(Word::new_unchecked("hi", "hi"))),
        BracketedItem::Word(Box::new(Word::new_unchecked("there", "there"))),
    ];
    let bracketed = BracketedContent::new(inner_words);
    let retrace = Retrace::new(bracketed, RetraceKind::Full).as_group();
    let items = vec![UtteranceContent::Retrace(Box::new(retrace))];

    assert_eq!(count_tier_positions(&items, TierDomain::Mor), 0);
    assert_eq!(count_tier_positions(&items, TierDomain::Pho), 2);
    assert_eq!(count_tier_positions(&items, TierDomain::Sin), 2);
    assert_eq!(count_tier_positions(&items, TierDomain::Wor), 2); // Wor includes retraced content!
}

/// Confirms pauses contribute only to `%pho` unit counts.
#[test]
fn pho_counts_pause_but_other_domains_ignore() {
    let pause = UtteranceContent::Pause(Pause::new(PauseDuration::Short));
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&pause), TierDomain::Pho),
        1
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&pause), TierDomain::Mor),
        0
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&pause), TierDomain::Sin),
        0
    );
    // Pauses don't appear in %wor tiers - only words with timing bullets appear there
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&pause), TierDomain::Wor),
        0
    );
}

/// Confirms `%pho` groups count as one `%pho` unit but expand for `%mor/%wor`.
#[test]
fn pho_group_counts_as_single_unit_for_pho() {
    let inner = vec![
        BracketedItem::Word(Box::new(Word::new_unchecked("hi", "hi"))),
        BracketedItem::Word(Box::new(Word::new_unchecked("there", "there"))),
    ];
    let group = UtteranceContent::PhoGroup(PhoGroup::new(BracketedContent::new(inner)));
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&group), TierDomain::Pho),
        1
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&group), TierDomain::Wor),
        2
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&group), TierDomain::Mor),
        2
    );
}

/// Confirms `%sin` groups count as one `%sin` unit but expand for `%mor/%wor`.
#[test]
fn sin_group_counts_as_single_unit_for_sin() {
    let inner = vec![
        BracketedItem::Word(Box::new(Word::new_unchecked("hi", "hi"))),
        BracketedItem::Word(Box::new(Word::new_unchecked("there", "there"))),
    ];
    let group = UtteranceContent::SinGroup(SinGroup::new(BracketedContent::new(inner)));
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&group), TierDomain::Sin),
        1
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&group), TierDomain::Wor),
        2
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&group), TierDomain::Mor),
        2
    );
}

/// Confirms phonological fragments with replacements are excluded from `%pho`.
#[test]
fn pho_skips_fragment_with_replacement() {
    let base = Word::new_unchecked("&+fr", "fr").with_category(WordCategory::PhonologicalFragment);
    let replacement = Replacement::new(vec![Word::new_unchecked("word", "word")]);
    let replaced = UtteranceContent::ReplacedWord(Box::new(ReplacedWord::new(base, replacement)));

    assert_eq!(
        count_tier_positions(std::slice::from_ref(&replaced), TierDomain::Pho),
        0
    );
    // Fragments are excluded from %wor regardless of replacement context.
    assert_eq!(count_tier_positions(&[replaced], TierDomain::Wor), 0);
}

/// Confirms `%wor` excludes fragments (`&+`) and nonwords (`&~`) but includes
/// fillers (`&-`). This matches BA2's policy: BA2 classified `&+` and `&~` as
/// `TokenType.ANNOT` (excluded from %wor) while `&-` was `TokenType.FP`
/// (included). Untranscribed tokens (`xxx`/`yyy`/`www`) are also excluded —
/// see `wor_excludes_untranscribed_tokens`.
#[test]
fn wor_excludes_fragments_and_nonwords_but_includes_fillers() {
    // Nonwords (&~) are excluded from %wor — no meaningful timing for gestural/interactional sounds
    let nonword = UtteranceContent::Word(Box::new(
        Word::new_unchecked("&~gaga", "gaga").with_category(WordCategory::Nonword),
    ));
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&nonword), TierDomain::Wor),
        0,
        "nonword (&~) must be excluded from %wor"
    );

    // Fragments (&+) are excluded from %wor — incomplete phoneme sequences
    let fragment = UtteranceContent::Word(Box::new(
        Word::new_unchecked("&+fr", "fr").with_category(WordCategory::PhonologicalFragment),
    ));
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&fragment), TierDomain::Wor),
        0,
        "fragment (&+) must be excluded from %wor"
    );

    // Fillers (&-) ARE included in %wor — they are real spoken words with alignable phoneme sequences
    let filler = UtteranceContent::Word(Box::new(
        Word::new_unchecked("&-um", "um").with_category(WordCategory::Filler),
    ));
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&filler), TierDomain::Wor),
        1,
        "filler (&-) must be included in %wor"
    );

    // Untranscribed tokens have no alignable phoneme sequence — excluded from %wor.
    let untranscribed = UtteranceContent::Word(Box::new(Word::new_unchecked("xxx", "xxx")));
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&untranscribed), TierDomain::Wor),
        0,
        "untranscribed (xxx) must be excluded from %wor"
    );

    // %pho still counts all vocalizations, including untranscribed and gestural ones.
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&nonword), TierDomain::Pho),
        1
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&fragment), TierDomain::Pho),
        1
    );
    assert_eq!(
        count_tier_positions(std::slice::from_ref(&untranscribed), TierDomain::Pho),
        1
    );
}

/// OCSC 4009: retraced content counts for `%wor`, but fragments within the
/// retrace are excluded (fragments are excluded from `%wor` regardless of
/// retrace context).
#[test]
fn wor_counts_retraced_words_but_not_retraced_fragments_from_ocsc_4009() {
    let retrace = Retrace::new(
        BracketedContent::new(vec![
            BracketedItem::Word(Box::new(Word::new_unchecked("one", "one"))),
            BracketedItem::Word(Box::new(
                Word::new_unchecked("&+ss", "ss").with_category(WordCategory::PhonologicalFragment),
            )),
        ]),
        RetraceKind::Partial,
    );
    let items = vec![UtteranceContent::Retrace(Box::new(retrace))];

    assert_eq!(count_tier_positions(&items, TierDomain::Mor), 0);
    // Fragment (&+ss) is excluded from %wor; only "one" counts.
    assert_eq!(count_tier_positions(&items, TierDomain::Wor), 1);
}

/// OCSC 4026: retraced regular words still count for `%wor`; the `xxx`
/// placeholder in the original retrace does NOT count because untranscribed
/// tokens have no alignable phoneme sequence. The count is 4 (not 5).
#[test]
fn wor_counts_retraced_words_from_ocsc_4026_excluding_untranscribed() {
    let retrace = Retrace::new(
        BracketedContent::new(vec![
            BracketedItem::Word(Box::new(Word::new_unchecked("a", "a"))),
            BracketedItem::Word(Box::new(Word::new_unchecked("pumpkin", "pumpkin"))),
            BracketedItem::Word(Box::new(Word::new_unchecked("and", "and"))),
            BracketedItem::Word(Box::new(Word::new_unchecked("a", "a"))),
            // xxx is untranscribed — not counted by %wor (no phoneme sequence to align)
            BracketedItem::Word(Box::new(Word::new_unchecked("xxx", "xxx"))),
        ]),
        RetraceKind::Partial,
    );
    let items = vec![UtteranceContent::Retrace(Box::new(retrace))];

    // 4 real words; xxx excluded from %wor alignment count
    assert_eq!(count_tier_positions(&items, TierDomain::Wor), 4);
}

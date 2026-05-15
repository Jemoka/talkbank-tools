//! ReplacedWord extraction/injection cursor accounting and compound-filler underscore-splitting tests.

#![allow(unused_imports, dead_code)]

use super::*;

use talkbank_model::UtteranceIdx;
use talkbank_model::model::{Line, UtteranceContent, WriteChat};
use talkbank_parser::TreeSitterParser;

#[test]
fn compound_filler_extracted_as_separate_words_for_fa() {
    let input = include_str!("../../../../../../../resources/fixtures/fa_compound_filler.cha");
    let chat = parse_chat(input);

    let utt = chat
        .lines
        .iter()
        .find_map(|l| match l {
            Line::Utterance(u) => Some(u),
            _ => None,
        })
        .expect("fixture should have one utterance");

    let mut words = Vec::new();
    extraction::collect_fa_words(&utt.main.content.content, &mut words);

    // &-you_know should produce TWO words for FA: "you" and "know"
    // (not one compound "you_know" that Whisper can't match)
    assert!(
        words.contains(&"you".to_string()) && words.contains(&"know".to_string()),
        "compound filler &-you_know should be split into 'you' and 'know' for FA, got: {words:?}"
    );

    // &-um (if present) should remain as single "um"
    // Regular words should be unchanged
    assert!(
        words.contains(&"I".to_string()),
        "regular word 'I' missing: {words:?}"
    );
}

#[test]
fn test_fa_extraction_replaced_word_uses_original() {
    // "foo [: bar baz] qux ." → extraction should see [foo, qux], NOT [bar, baz, qux]
    let chat = parse_chat(&proof_chat("foo [: bar baz] qux ."));
    let utt = get_utterance(&chat, 0);
    let fa_words = {
        let mut v = Vec::new();
        extraction::collect_fa_words(&utt.main.content.content, &mut v);
        v
    };
    assert_eq!(
        fa_words,
        vec!["foo", "qux"],
        "extraction should use original replaced word (foo), not replacements (bar baz)"
    );
}

#[test]
fn test_fa_injection_replaced_word_uses_original_and_cursor_stays_in_sync() {
    // "foo [: bar baz] qux ." — injection must advance cursor by exactly 2 total:
    //   slot 0 → timing for foo (original replaced word)
    //   slot 1 → timing for qux (plain word after the replacement)
    let timings = vec![
        Some(WordTiming {
            start_ms: 100,
            end_ms: 500,
        }), // slot 0: foo
        Some(WordTiming {
            start_ms: 600,
            end_ms: 1000,
        }), // slot 1: qux
    ];

    let mut chat = parse_chat(&proof_chat("foo [: bar baz] qux ."));
    let utt = get_test_utterance(&mut chat, 0);
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    // Cursor must advance by exactly 2 (one for the original replaced word,
    // one for qux). The old bug advanced by 3 (two for bar+baz, one for qux),
    // leaving qux to take timing slot 2 which is out-of-bounds → None.
    assert_eq!(
        offset, 2,
        "cursor must advance by 2 (1 for original + 1 for qux), not 3 (2 for replacements + 1 for qux)"
    );

    // Generate %wor and check that qux carries timing slot 1 (600_1000).
    add_wor_tier(get_test_utterance(&mut chat, 0));
    let output = chat.to_chat_string();
    assert!(
        output.contains("qux \u{15}600_1000\u{15}"),
        "qux should have timing 600_1000 (slot 1); old bug put it at slot 2 → untimed:\n{output}"
    );
}

#[test]
fn test_fa_injection_cursor_stays_in_sync_across_utterance_boundary_with_replaced_word() {
    // FA group: [utt0: "a [: x y] .", utt1: "hello world ."]
    // Extraction (new): [a, hello, world] = 3 words (original for utt0)
    // FA returns 3 timings.
    // Old injection for utt0 consumed 2 (x, y) + 0 for 'a' = 2 slots,
    // leaving hello at slot 2 (T_world) and world at slot 3 (None).
    // Fixed injection for utt0 consumes 1 (a) = 1 slot → hello at slot 1, world at slot 2.
    let timings = vec![
        Some(WordTiming {
            start_ms: 100,
            end_ms: 200,
        }), // slot 0: a
        Some(WordTiming {
            start_ms: 300,
            end_ms: 500,
        }), // slot 1: hello
        Some(WordTiming {
            start_ms: 600,
            end_ms: 900,
        }), // slot 2: world
    ];

    // Build a two-utterance chat file
    let chat_text = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\ta [: x y] .\n*CHI:\thello world .\n@End\n".to_string();
    let mut chat = parse_chat(&chat_text);

    let mut offset = 0;
    {
        let utt0 = get_test_utterance(&mut chat, 0);
        inject_timings_for_utterance(utt0, &timings, &mut offset);
    }
    // After utt0: cursor should be at 1 (one original word consumed)
    assert_eq!(
        offset, 1,
        "cursor after utt0 must be 1 (only 'a' consumed), not 2 (old: bar+baz)"
    );

    {
        let utt1 = get_test_utterance(&mut chat, 1);
        inject_timings_for_utterance(utt1, &timings, &mut offset);
    }
    // After utt1: cursor at 3 (hello + world)
    assert_eq!(offset, 3, "cursor after utt1 must be 3");

    // Generate %wor for utt1 and verify hello/world timings are correct
    add_wor_tier(get_test_utterance(&mut chat, 1));
    let output = chat.to_chat_string();
    assert!(
        output.contains("hello \u{15}300_500\u{15}"),
        "hello should have timing 300_500 (slot 1):\n{output}"
    );
    assert!(
        output.contains("world \u{15}600_900\u{15}"),
        "world should have timing 600_900 (slot 2):\n{output}"
    );
}

#[test]
fn test_collect_existing_fa_word_timings_replaced_word_returns_one_entry_for_original() {
    // "foo [: bar baz] qux ." has 2 FA words (foo and qux) after my fix.
    // After injection: foo gets timing 100_500, qux gets timing 600_1000.
    // collect_existing_fa_word_timings must return exactly 2 entries —
    // not 3 (bar:None, baz:None, qux:Some) as the old code did.
    let timings = vec![
        Some(WordTiming {
            start_ms: 100,
            end_ms: 500,
        }), // foo
        Some(WordTiming {
            start_ms: 600,
            end_ms: 1000,
        }), // qux
    ];

    let mut chat = parse_chat(&proof_chat("foo [: bar baz] qux ."));
    let utt = get_test_utterance(&mut chat, 0);
    let mut offset = 0;
    inject_timings_for_utterance(utt, &timings, &mut offset);

    let utt = get_utterance(&chat, 0);
    let existing_timings = collect_existing_fa_word_timings(utt);

    assert_eq!(
        existing_timings.len(),
        2,
        "must return 2 entries (foo, qux) — old code returned 3 (bar:None, baz:None, qux:Some) causing collect_preserved_group_timings to return None on every run: {existing_timings:?}"
    );
    assert_eq!(
        existing_timings[0],
        Some(WordTiming {
            start_ms: 100,
            end_ms: 500
        }),
        "foo should carry its injected timing"
    );
    assert_eq!(
        existing_timings[1],
        Some(WordTiming {
            start_ms: 600,
            end_ms: 1000
        }),
        "qux should carry its injected timing"
    );
}

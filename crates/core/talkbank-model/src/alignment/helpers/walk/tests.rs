//! Unit tests for the content tree walkers.

use super::*;
use crate::Span;
use crate::annotation::AnnotatedContentAnnotations;
use crate::model::{
    Annotated, BracketedContent, BracketedItem, Event, Group, OverlapPoint, OverlapPointKind,
    Pause, PauseDuration, PhoGroup, Retrace, RetraceKind, Separator, UtteranceContent, Word,
};

fn word(text: &str) -> Word {
    Word::simple(text)
}

fn boxed_word(text: &str) -> Box<Word> {
    Box::new(word(text))
}

// ---------------------------------------------------------------------------
// walk_words tests (backward-compat with old walk_words tests)
// ---------------------------------------------------------------------------

/// Collects leaf word texts from content using the walker.
fn collect_word_texts(content: &[UtteranceContent], domain: Option<TierDomain>) -> Vec<String> {
    let mut texts = Vec::new();
    walk_words(content, domain, &mut |leaf| {
        if let WordItem::Word(w) = leaf {
            texts.push(w.cleaned_text().to_string());
        }
    });
    texts
}

#[test]
fn flat_words() {
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Word(boxed_word("world")),
    ];
    assert_eq!(collect_word_texts(&content, None), ["hello", "world"]);
}

#[test]
fn words_inside_group() {
    let group = Group::new(BracketedContent::new(vec![
        BracketedItem::Word(boxed_word("in")),
        BracketedItem::Word(boxed_word("group")),
    ]));
    let content = vec![UtteranceContent::Group(group)];
    assert_eq!(collect_word_texts(&content, None), ["in", "group"]);
}

#[test]
fn retrace_group_skipped_for_mor() {
    let bracketed = BracketedContent::new(vec![BracketedItem::Word(boxed_word("retraced"))]);
    let retrace = Retrace::new(bracketed, RetraceKind::Full).as_group();
    let content = vec![
        UtteranceContent::Retrace(Box::new(retrace)),
        UtteranceContent::Word(boxed_word("kept")),
    ];

    // Mor domain: retrace is skipped
    assert_eq!(
        collect_word_texts(&content, Some(TierDomain::Mor)),
        ["kept"]
    );
    // No domain: retrace is included
    assert_eq!(collect_word_texts(&content, None), ["retraced", "kept"]);
    // Wor domain: retrace is included
    assert_eq!(
        collect_word_texts(&content, Some(TierDomain::Wor)),
        ["retraced", "kept"]
    );
}

#[test]
fn pho_group_skipped_for_pho_domain() {
    let pho = PhoGroup::new(BracketedContent::new(vec![BracketedItem::Word(
        boxed_word("phonological"),
    )]));
    let content = vec![
        UtteranceContent::PhoGroup(pho),
        UtteranceContent::Word(boxed_word("after")),
    ];

    // Pho domain: PhoGroup skipped
    assert_eq!(
        collect_word_texts(&content, Some(TierDomain::Pho)),
        ["after"]
    );
    // Mor domain: PhoGroup recursed
    assert_eq!(
        collect_word_texts(&content, Some(TierDomain::Mor)),
        ["phonological", "after"]
    );
    // No domain: PhoGroup recursed
    assert_eq!(
        collect_word_texts(&content, None),
        ["phonological", "after"]
    );
}

#[test]
fn separator_yielded() {
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Separator(Separator::Comma { span: Span::DUMMY }),
        UtteranceContent::Word(boxed_word("world")),
    ];
    let mut count = 0;
    walk_words(&content, None, &mut |leaf| {
        if let WordItem::Separator(_) = leaf {
            count += 1;
        }
    });
    assert_eq!(count, 1);
}

#[test]
fn mut_walker_modifies_words() {
    let mut content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Word(boxed_word("world")),
    ];
    walk_words_mut(&mut content, None, &mut |leaf| {
        if let WordItemMut::Word(w) = leaf {
            w.inline_bullet = Some(crate::model::Bullet::new(0, 100));
        }
    });
    // Verify modification took effect
    let mut count = 0;
    walk_words(&content, None, &mut |leaf| {
        if let WordItem::Word(w) = leaf {
            assert!(w.inline_bullet.is_some());
            count += 1;
        }
    });
    assert_eq!(count, 2);
}

#[test]
fn nested_quotation_recursion() {
    let quot = crate::model::Quotation::new(BracketedContent::new(vec![BracketedItem::Word(
        boxed_word("quoted"),
    )]));
    let content = vec![UtteranceContent::Quotation(quot)];
    assert_eq!(collect_word_texts(&content, None), ["quoted"]);
}

// ---------------------------------------------------------------------------
// walk_content tests — verify non-word items are emitted
// ---------------------------------------------------------------------------

/// Helper: count how many items of each kind walk_content emits.
#[derive(Default, Debug)]
struct ContentCounts {
    words: usize,
    replaced_words: usize,
    separators: usize,
    events: usize,
    pauses: usize,
    actions: usize,
    overlap_points: usize,
    other_spoken_events: usize,
    freecodes: usize,
    internal_bullets: usize,
    long_feature_begins: usize,
    long_feature_ends: usize,
    underline_begins: usize,
    underline_ends: usize,
    nonvocal_begins: usize,
    nonvocal_ends: usize,
    nonvocal_simples: usize,
}

fn count_content_items(content: &[UtteranceContent], domain: Option<TierDomain>) -> ContentCounts {
    let mut counts = ContentCounts::default();
    walk_content(content, domain, &mut |item| match item {
        ContentItem::Word(_) => counts.words += 1,
        ContentItem::ReplacedWord(_) => counts.replaced_words += 1,
        ContentItem::Separator(_) => counts.separators += 1,
        ContentItem::Event(_) => counts.events += 1,
        ContentItem::Pause(_) => counts.pauses += 1,
        ContentItem::Action(_) => counts.actions += 1,
        ContentItem::OverlapPoint(_) => counts.overlap_points += 1,
        ContentItem::OtherSpokenEvent(_) => counts.other_spoken_events += 1,
        ContentItem::Freecode(_) => counts.freecodes += 1,
        ContentItem::InternalBullet(_) => counts.internal_bullets += 1,
        ContentItem::LongFeatureBegin(_) => counts.long_feature_begins += 1,
        ContentItem::LongFeatureEnd(_) => counts.long_feature_ends += 1,
        ContentItem::UnderlineBegin(_) => counts.underline_begins += 1,
        ContentItem::UnderlineEnd(_) => counts.underline_ends += 1,
        ContentItem::NonvocalBegin(_) => counts.nonvocal_begins += 1,
        ContentItem::NonvocalEnd(_) => counts.nonvocal_ends += 1,
        ContentItem::NonvocalSimple(_) => counts.nonvocal_simples += 1,
    });
    counts
}

#[test]
fn walk_content_emits_events_and_pauses() {
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Event(Event::new("laughs")),
        UtteranceContent::Pause(Pause {
            duration: PauseDuration::Short,
            span: Span::DUMMY,
        }),
        UtteranceContent::Word(boxed_word("world")),
    ];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 2);
    assert_eq!(counts.events, 1);
    assert_eq!(counts.pauses, 1);
}

#[test]
fn walk_content_emits_annotated_event_inner() {
    let event = Event::new("coughs");
    let annotated = Annotated {
        inner: event,
        scoped_annotations: AnnotatedContentAnnotations::new(vec![]),
        span: Span::DUMMY,
    };
    let content = vec![UtteranceContent::AnnotatedEvent(annotated)];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.events, 1);
}

#[test]
fn walk_content_emits_overlap_points() {
    let op = OverlapPoint::new(OverlapPointKind::TopOverlapBegin, None);
    let content = vec![
        UtteranceContent::Word(boxed_word("hi")),
        UtteranceContent::OverlapPoint(op),
    ];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 1);
    assert_eq!(counts.overlap_points, 1);
}

#[test]
fn walk_content_recurses_into_groups() {
    let event = Event::new("claps");
    let group = Group::new(BracketedContent::new(vec![
        BracketedItem::Word(boxed_word("inside")),
        BracketedItem::Event(event),
    ]));
    let content = vec![UtteranceContent::Group(group)];
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 1);
    assert_eq!(counts.events, 1);
}

#[test]
fn walk_content_skips_pho_group_for_pho_domain() {
    let pho = PhoGroup::new(BracketedContent::new(vec![BracketedItem::Word(
        boxed_word("phonological"),
    )]));
    let content = vec![
        UtteranceContent::PhoGroup(pho),
        UtteranceContent::Word(boxed_word("after")),
    ];

    // Pho domain: PhoGroup skipped
    let counts = count_content_items(&content, Some(TierDomain::Pho));
    assert_eq!(counts.words, 1); // only "after"

    // No domain: PhoGroup recursed
    let counts = count_content_items(&content, None);
    assert_eq!(counts.words, 2); // "phonological" + "after"
}

#[test]
fn walk_content_words_match_walk_words() {
    // Verify walk_content produces the same words as walk_words for simple content.
    let content = vec![
        UtteranceContent::Word(boxed_word("hello")),
        UtteranceContent::Separator(Separator::Comma { span: Span::DUMMY }),
        UtteranceContent::Word(boxed_word("world")),
    ];

    let mut content_words = Vec::new();
    walk_content(&content, None, &mut |item| {
        if let ContentItem::Word(w) = item {
            content_words.push(w.cleaned_text().to_string());
        }
    });

    let walk_words_result = collect_word_texts(&content, None);
    assert_eq!(content_words, walk_words_result);
}

// ---------------------------------------------------------------------------
// Deprecated alias tests — verify they still work
// ---------------------------------------------------------------------------

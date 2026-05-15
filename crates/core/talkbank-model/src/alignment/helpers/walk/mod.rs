//! Content tree walkers for traversing [`UtteranceContent`] and [`BracketedItem`].
//!
//! Centralizes the recursive traversal of [`UtteranceContent`] (24 variants) and
//! [`BracketedItem`] (22 variants) so callers provide only item-handling logic.
//! Domain-aware group gating (retrace skip for Mor, PhoGroup/SinGroup skip for
//! Pho/Sin) is handled once here.
//!
//! # Walkers
//!
//! - [`walk_content`] / [`walk_content_mut`] — emit ALL non-container items
//! - [`walk_words`] / [`walk_words_mut`] — convenience filter for word-like items only
//!
//! # Deprecated aliases
//!
//! [`walk_words`] / [`walk_words_mut`] delegate to [`walk_words`] / [`walk_words_mut`].
//! [`ContentLeaf`] / [`ContentLeafMut`] are type aliases for [`WordItem`] / [`WordItemMut`].

use crate::alignment::helpers::{domain::TierDomain, rules::should_skip_group};
use crate::model::{
    Action, BracketedItem, Bullet, ContentAnnotation, Event, Freecode, LongFeatureBegin,
    LongFeatureEnd, NonvocalBegin, NonvocalEnd, NonvocalSimple, OtherSpokenEvent, OverlapPoint,
    Pause, ReplacedWord, Separator, UnderlineMarker, UtteranceContent, Word,
};

// ---------------------------------------------------------------------------
// ContentItem — every non-container item
// ---------------------------------------------------------------------------

/// Every non-container content item visited during in-order traversal.
/// Groups are descended into transparently. Annotated wrappers are
/// unwrapped to expose the inner item.
pub enum ContentItem<'a> {
    /// Plain word or inner word of an `AnnotatedWord`.
    Word(&'a Word),
    /// Replaced word (`word [: replacement]`).
    ReplacedWord(&'a ReplacedWord),
    /// Separator (comma, tag, vocative, etc.).
    Separator(&'a Separator),
    /// Sound event (`&=laughs`) or inner event of an `AnnotatedEvent`.
    Event(&'a Event),
    /// Pause (`(.)`, `(..)`, `(...)`, or timed).
    Pause(&'a Pause),
    /// Action (`&%action`) or inner action of an `AnnotatedAction`.
    Action(&'a Action),
    /// CA overlap boundary marker.
    OverlapPoint(&'a OverlapPoint),
    /// Other-speaker spoken event (`&*SPK:word`).
    OtherSpokenEvent(&'a OtherSpokenEvent),
    /// Freecode inline annotation (`[^ comment]`).
    Freecode(&'a Freecode),
    /// Internal timing bullet (mid-utterance media timestamp).
    InternalBullet(&'a Bullet),
    /// Long feature scope begin (`&{l=LABEL`).
    LongFeatureBegin(&'a LongFeatureBegin),
    /// Long feature scope end (`&}l=LABEL`).
    LongFeatureEnd(&'a LongFeatureEnd),
    /// Underline begin marker.
    UnderlineBegin(&'a UnderlineMarker),
    /// Underline end marker.
    UnderlineEnd(&'a UnderlineMarker),
    /// Nonvocal scope begin (`&{n=LABEL`).
    NonvocalBegin(&'a NonvocalBegin),
    /// Nonvocal scope end (`&}n=LABEL`).
    NonvocalEnd(&'a NonvocalEnd),
    /// Simple nonvocal marker (`&{n=LABEL}`).
    NonvocalSimple(&'a NonvocalSimple),
}

/// Mutable version of [`ContentItem`].
pub enum ContentItemMut<'a> {
    /// Mutable word reference.
    Word(&'a mut Word),
    /// Mutable replaced word reference.
    ReplacedWord(&'a mut ReplacedWord),
    /// Mutable separator reference.
    Separator(&'a mut Separator),
    /// Mutable event reference.
    Event(&'a mut Event),
    /// Mutable pause reference.
    Pause(&'a mut Pause),
    /// Mutable action reference.
    Action(&'a mut Action),
    /// Mutable overlap point reference.
    OverlapPoint(&'a mut OverlapPoint),
    /// Mutable other-speaker spoken event reference.
    OtherSpokenEvent(&'a mut OtherSpokenEvent),
    /// Mutable freecode reference.
    Freecode(&'a mut Freecode),
    /// Mutable internal bullet reference.
    InternalBullet(&'a mut Bullet),
    /// Mutable long feature begin reference.
    LongFeatureBegin(&'a mut LongFeatureBegin),
    /// Mutable long feature end reference.
    LongFeatureEnd(&'a mut LongFeatureEnd),
    /// Mutable underline begin reference.
    UnderlineBegin(&'a mut UnderlineMarker),
    /// Mutable underline end reference.
    UnderlineEnd(&'a mut UnderlineMarker),
    /// Mutable nonvocal begin reference.
    NonvocalBegin(&'a mut NonvocalBegin),
    /// Mutable nonvocal end reference.
    NonvocalEnd(&'a mut NonvocalEnd),
    /// Mutable nonvocal simple reference.
    NonvocalSimple(&'a mut NonvocalSimple),
}

// ---------------------------------------------------------------------------
// WordItem — word-like leaf items only
// ---------------------------------------------------------------------------

/// Word-like leaf item yielded by [`walk_words`].
///
/// A word-like content item visited during in-order traversal.
/// Groups are descended into transparently. AnnotatedWord is unwrapped.
pub enum WordItem<'a> {
    /// A word (bare or unwrapped from AnnotatedWord).
    Word(&'a Word),
    /// Replaced word (`word [: replacement]`).
    ReplacedWord(&'a ReplacedWord),
    /// Separator (comma, tag, vocative, etc.).
    Separator(&'a Separator),
}

/// Mutable version of [`WordItem`].
pub enum WordItemMut<'a> {
    /// Mutable word reference.
    Word(&'a mut Word),
    /// Mutable replaced word reference.
    ReplacedWord(&'a mut ReplacedWord),
    /// Mutable separator reference.
    Separator(&'a mut Separator),
}

// ---------------------------------------------------------------------------
// Deprecated aliases
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// walk_content — emit ALL non-container items
// ---------------------------------------------------------------------------

/// Walk utterance content and call `f` for every non-container item.
///
/// Groups are descended into transparently. Annotated wrappers are unwrapped
/// to expose the inner item. Domain gating applies as with [`walk_words`]:
/// `Some(Mor)` skips retrace/reformulation groups, `Some(Pho|Sin)` skips
/// PhoGroup/SinGroup.
pub fn walk_content<'a>(
    content: &'a [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItem<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(ContentItem::Word(word));
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                // Single-word retraces (e.g. `cup [/]`) are AnnotatedWord with
                // retrace annotations — skip them in the Mor domain just like
                // multi-word AnnotatedGroup retraces.
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(ContentItem::Word(&annotated.inner));
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(ContentItem::ReplacedWord(replaced));
            }
            UtteranceContent::Separator(sep) => {
                f(ContentItem::Separator(sep));
            }
            UtteranceContent::Event(event) => {
                f(ContentItem::Event(event));
            }
            UtteranceContent::AnnotatedEvent(annotated) => {
                f(ContentItem::Event(&annotated.inner));
            }
            UtteranceContent::Pause(pause) => {
                f(ContentItem::Pause(pause));
            }
            UtteranceContent::AnnotatedAction(annotated) => {
                f(ContentItem::Action(&annotated.inner));
            }
            UtteranceContent::Freecode(fc) => {
                f(ContentItem::Freecode(fc));
            }
            UtteranceContent::OverlapPoint(op) => {
                f(ContentItem::OverlapPoint(op));
            }
            UtteranceContent::InternalBullet(bullet) => {
                f(ContentItem::InternalBullet(bullet));
            }
            UtteranceContent::LongFeatureBegin(lfb) => {
                f(ContentItem::LongFeatureBegin(lfb));
            }
            UtteranceContent::LongFeatureEnd(lfe) => {
                f(ContentItem::LongFeatureEnd(lfe));
            }
            UtteranceContent::UnderlineBegin(marker) => {
                f(ContentItem::UnderlineBegin(marker));
            }
            UtteranceContent::UnderlineEnd(marker) => {
                f(ContentItem::UnderlineEnd(marker));
            }
            UtteranceContent::NonvocalBegin(nv) => {
                f(ContentItem::NonvocalBegin(nv));
            }
            UtteranceContent::NonvocalEnd(nv) => {
                f(ContentItem::NonvocalEnd(nv));
            }
            UtteranceContent::NonvocalSimple(nv) => {
                f(ContentItem::NonvocalSimple(nv));
            }
            UtteranceContent::OtherSpokenEvent(ose) => {
                f(ContentItem::OtherSpokenEvent(ose));
            }
            // Groups: descend into content
            UtteranceContent::Group(group) => {
                walk_bracketed_content(&group.content.content, domain, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content(&annotated.inner.content.content, domain, f);
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&pho.content.content, domain, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&sin.content.content, domain, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_content(&quot.content.content, domain, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content(&retrace.content.content, domain, f);
                }
            }
        }
    }
}

/// Walk utterance content mutably and call `f` for every non-container item.
///
/// Same domain-aware gating as [`walk_content`].
pub fn walk_content_mut<'a>(
    content: &'a mut [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItemMut<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(ContentItemMut::Word(word));
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(ContentItemMut::Word(&mut annotated.inner));
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(ContentItemMut::ReplacedWord(replaced));
            }
            UtteranceContent::Separator(sep) => {
                f(ContentItemMut::Separator(sep));
            }
            UtteranceContent::Event(event) => {
                f(ContentItemMut::Event(event));
            }
            UtteranceContent::AnnotatedEvent(annotated) => {
                f(ContentItemMut::Event(&mut annotated.inner));
            }
            UtteranceContent::Pause(pause) => {
                f(ContentItemMut::Pause(pause));
            }
            UtteranceContent::AnnotatedAction(annotated) => {
                f(ContentItemMut::Action(&mut annotated.inner));
            }
            UtteranceContent::Freecode(fc) => {
                f(ContentItemMut::Freecode(fc));
            }
            UtteranceContent::OverlapPoint(op) => {
                f(ContentItemMut::OverlapPoint(op));
            }
            UtteranceContent::InternalBullet(bullet) => {
                f(ContentItemMut::InternalBullet(bullet));
            }
            UtteranceContent::LongFeatureBegin(lfb) => {
                f(ContentItemMut::LongFeatureBegin(lfb));
            }
            UtteranceContent::LongFeatureEnd(lfe) => {
                f(ContentItemMut::LongFeatureEnd(lfe));
            }
            UtteranceContent::UnderlineBegin(marker) => {
                f(ContentItemMut::UnderlineBegin(marker));
            }
            UtteranceContent::UnderlineEnd(marker) => {
                f(ContentItemMut::UnderlineEnd(marker));
            }
            UtteranceContent::NonvocalBegin(nv) => {
                f(ContentItemMut::NonvocalBegin(nv));
            }
            UtteranceContent::NonvocalEnd(nv) => {
                f(ContentItemMut::NonvocalEnd(nv));
            }
            UtteranceContent::NonvocalSimple(nv) => {
                f(ContentItemMut::NonvocalSimple(nv));
            }
            UtteranceContent::OtherSpokenEvent(ose) => {
                f(ContentItemMut::OtherSpokenEvent(ose));
            }
            // Groups: descend into content
            UtteranceContent::Group(group) => {
                walk_bracketed_content_mut(&mut group.content.content, domain, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content_mut(&mut annotated.inner.content.content, domain, f);
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(&mut pho.content.content, domain, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(&mut sin.content.content, domain, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_content_mut(&mut quot.content.content, domain, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content_mut(&mut retrace.content.content, domain, f);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// walk_words — word-like items only (replacement for walk_words)
// ---------------------------------------------------------------------------

/// Walk utterance content and call `f` for each word-like leaf item.
///
/// This is a convenience filter over [`walk_content`] that only emits
/// words, replaced words, and separators. It replaces the deprecated
/// [`walk_words`] function.
///
/// When `domain` is `Some(Mor)`, annotated groups with retrace/reformulation
/// annotations are skipped. When `domain` is `Some(Pho)` or `Some(Sin)`,
/// PhoGroup and SinGroup are skipped (treated as atomic units by those domains).
/// When `domain` is `None`, all groups are recursed unconditionally.
pub fn walk_words<'a>(
    content: &'a [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItem<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(WordItem::Word(word));
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(WordItem::Word(&annotated.inner));
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(WordItem::ReplacedWord(replaced));
            }
            UtteranceContent::Separator(sep) => {
                f(WordItem::Separator(sep));
            }
            UtteranceContent::Group(group) => {
                walk_bracketed_words(&group.content.content, domain, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_words(&annotated.inner.content.content, domain, f);
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&pho.content.content, domain, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&sin.content.content, domain, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_words(&quot.content.content, domain, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words(&retrace.content.content, domain, f);
                }
            }
            // Non-word items: events, pauses, actions, overlap markers, bullets,
            // freecodes, long features, underline markers, nonvocal markers,
            // other spoken events — none produce alignable leaf items.
            UtteranceContent::Event(_)
            | UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::OverlapPoint(_)
            | UtteranceContent::InternalBullet(_)
            | UtteranceContent::LongFeatureBegin(_)
            | UtteranceContent::LongFeatureEnd(_)
            | UtteranceContent::UnderlineBegin(_)
            | UtteranceContent::UnderlineEnd(_)
            | UtteranceContent::NonvocalBegin(_)
            | UtteranceContent::NonvocalEnd(_)
            | UtteranceContent::NonvocalSimple(_)
            | UtteranceContent::OtherSpokenEvent(_) => {}
        }
    }
}

/// Walk utterance content mutably and call `f` for each word-like leaf item.
///
/// Same domain-aware gating as [`walk_words`].
pub fn walk_words_mut<'a>(
    content: &'a mut [UtteranceContent],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItemMut<'a>),
) {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                f(WordItemMut::Word(word));
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                // Split borrow: mut inner + shared annotations (disjoint fields).
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    let a = annotated.as_mut();
                    f(WordItemMut::Word(&mut a.inner));
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                f(WordItemMut::ReplacedWord(replaced));
            }
            UtteranceContent::Separator(sep) => {
                f(WordItemMut::Separator(sep));
            }
            UtteranceContent::Group(group) => {
                walk_bracketed_words_mut(&mut group.content.content, domain, f);
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_words_mut(&mut annotated.inner.content.content, domain, f);
                }
            }
            UtteranceContent::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(&mut pho.content.content, domain, f);
                }
            }
            UtteranceContent::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(&mut sin.content.content, domain, f);
                }
            }
            UtteranceContent::Quotation(quot) => {
                walk_bracketed_words_mut(&mut quot.content.content, domain, f);
            }
            UtteranceContent::Retrace(retrace) => {
                // Retrace content is excluded from %mor (not morphologically analyzed),
                // but included in %pho/%sin/%wor and for domain-unspecified walks.
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words_mut(&mut retrace.content.content, domain, f);
                }
            }
            UtteranceContent::Event(_)
            | UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::OverlapPoint(_)
            | UtteranceContent::InternalBullet(_)
            | UtteranceContent::LongFeatureBegin(_)
            | UtteranceContent::LongFeatureEnd(_)
            | UtteranceContent::UnderlineBegin(_)
            | UtteranceContent::UnderlineEnd(_)
            | UtteranceContent::NonvocalBegin(_)
            | UtteranceContent::NonvocalEnd(_)
            | UtteranceContent::NonvocalSimple(_)
            | UtteranceContent::OtherSpokenEvent(_) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Deprecated aliases — delegate to walk_words / walk_words_mut
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Bracketed-level helpers for walk_content
// ---------------------------------------------------------------------------

fn walk_bracketed_content<'a>(
    items: &'a [BracketedItem],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItem<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(ContentItem::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(ContentItem::Word(&annotated.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(ContentItem::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(ContentItem::Separator(sep));
            }
            BracketedItem::Event(event) => {
                f(ContentItem::Event(event));
            }
            BracketedItem::AnnotatedEvent(annotated) => {
                f(ContentItem::Event(&annotated.inner));
            }
            BracketedItem::Pause(pause) => {
                f(ContentItem::Pause(pause));
            }
            BracketedItem::Action(action) => {
                f(ContentItem::Action(action));
            }
            BracketedItem::AnnotatedAction(annotated) => {
                f(ContentItem::Action(&annotated.inner));
            }
            BracketedItem::OverlapPoint(op) => {
                f(ContentItem::OverlapPoint(op));
            }
            BracketedItem::InternalBullet(bullet) => {
                f(ContentItem::InternalBullet(bullet));
            }
            BracketedItem::Freecode(fc) => {
                f(ContentItem::Freecode(fc));
            }
            BracketedItem::LongFeatureBegin(lfb) => {
                f(ContentItem::LongFeatureBegin(lfb));
            }
            BracketedItem::LongFeatureEnd(lfe) => {
                f(ContentItem::LongFeatureEnd(lfe));
            }
            BracketedItem::UnderlineBegin(marker) => {
                f(ContentItem::UnderlineBegin(marker));
            }
            BracketedItem::UnderlineEnd(marker) => {
                f(ContentItem::UnderlineEnd(marker));
            }
            BracketedItem::NonvocalBegin(nv) => {
                f(ContentItem::NonvocalBegin(nv));
            }
            BracketedItem::NonvocalEnd(nv) => {
                f(ContentItem::NonvocalEnd(nv));
            }
            BracketedItem::NonvocalSimple(nv) => {
                f(ContentItem::NonvocalSimple(nv));
            }
            BracketedItem::OtherSpokenEvent(ose) => {
                f(ContentItem::OtherSpokenEvent(ose));
            }
            // Groups: descend into content
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content(&annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content(&sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_content(&quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content(&retrace.content.content, domain, f);
                }
            }
        }
    }
}

fn walk_bracketed_content_mut<'a>(
    items: &'a mut [BracketedItem],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(ContentItemMut<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(ContentItemMut::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(ContentItemMut::Word(&mut annotated.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(ContentItemMut::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(ContentItemMut::Separator(sep));
            }
            BracketedItem::Event(event) => {
                f(ContentItemMut::Event(event));
            }
            BracketedItem::AnnotatedEvent(annotated) => {
                f(ContentItemMut::Event(&mut annotated.inner));
            }
            BracketedItem::Pause(pause) => {
                f(ContentItemMut::Pause(pause));
            }
            BracketedItem::Action(action) => {
                f(ContentItemMut::Action(action));
            }
            BracketedItem::AnnotatedAction(annotated) => {
                f(ContentItemMut::Action(&mut annotated.inner));
            }
            BracketedItem::OverlapPoint(op) => {
                f(ContentItemMut::OverlapPoint(op));
            }
            BracketedItem::InternalBullet(bullet) => {
                f(ContentItemMut::InternalBullet(bullet));
            }
            BracketedItem::Freecode(fc) => {
                f(ContentItemMut::Freecode(fc));
            }
            BracketedItem::LongFeatureBegin(lfb) => {
                f(ContentItemMut::LongFeatureBegin(lfb));
            }
            BracketedItem::LongFeatureEnd(lfe) => {
                f(ContentItemMut::LongFeatureEnd(lfe));
            }
            BracketedItem::UnderlineBegin(marker) => {
                f(ContentItemMut::UnderlineBegin(marker));
            }
            BracketedItem::UnderlineEnd(marker) => {
                f(ContentItemMut::UnderlineEnd(marker));
            }
            BracketedItem::NonvocalBegin(nv) => {
                f(ContentItemMut::NonvocalBegin(nv));
            }
            BracketedItem::NonvocalEnd(nv) => {
                f(ContentItemMut::NonvocalEnd(nv));
            }
            BracketedItem::NonvocalSimple(nv) => {
                f(ContentItemMut::NonvocalSimple(nv));
            }
            BracketedItem::OtherSpokenEvent(ose) => {
                f(ContentItemMut::OtherSpokenEvent(ose));
            }
            // Groups: descend into content
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_content_mut(&mut annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(&mut pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_content_mut(&mut sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_content_mut(&mut quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_content_mut(&mut retrace.content.content, domain, f);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bracketed-level helpers for walk_words
// ---------------------------------------------------------------------------

fn walk_bracketed_words<'a>(
    items: &'a [BracketedItem],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItem<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(WordItem::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    f(WordItem::Word(&annotated.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(WordItem::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(WordItem::Separator(sep));
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_words(&annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words(&sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_words(&quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words(&retrace.content.content, domain, f);
                }
            }
            // Non-word bracketed items.
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

fn walk_bracketed_words_mut<'a>(
    items: &'a mut [BracketedItem],
    domain: Option<TierDomain>,
    f: &mut impl FnMut(WordItemMut<'a>),
) {
    for item in items {
        match item {
            BracketedItem::Word(word) => {
                f(WordItemMut::Word(word));
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    let a = annotated.as_mut();
                    f(WordItemMut::Word(&mut a.inner));
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                f(WordItemMut::ReplacedWord(replaced));
            }
            BracketedItem::Separator(sep) => {
                f(WordItemMut::Separator(sep));
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                if !should_skip_annotated_group(&annotated.scoped_annotations, domain) {
                    walk_bracketed_words_mut(&mut annotated.inner.content.content, domain, f);
                }
            }
            BracketedItem::PhoGroup(pho) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(&mut pho.content.content, domain, f);
                }
            }
            BracketedItem::SinGroup(sin) => {
                if !should_skip_pho_sin_group(domain) {
                    walk_bracketed_words_mut(&mut sin.content.content, domain, f);
                }
            }
            BracketedItem::Quotation(quot) => {
                walk_bracketed_words_mut(&mut quot.content.content, domain, f);
            }
            BracketedItem::Retrace(retrace) => {
                if !matches!(domain, Some(TierDomain::Mor)) {
                    walk_bracketed_words_mut(&mut retrace.content.content, domain, f);
                }
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::InternalBullet(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Returns `true` when an annotated word/group should be skipped for the given domain.
///
/// Checks for alignment-ignore annotations (currently `[e]` exclude marker).
/// Retrace skipping is handled by the `Retrace` content variant directly.
fn should_skip_annotated_group(
    annotations: &[ContentAnnotation],
    domain: Option<TierDomain>,
) -> bool {
    match domain {
        Some(d) => should_skip_group(annotations, d),
        None => false,
    }
}

/// Returns `true` when PhoGroup/SinGroup should be skipped.
///
/// Pho and Sin domains treat these as atomic units rather than recursing
/// into their word content.
fn should_skip_pho_sin_group(domain: Option<TierDomain>) -> bool {
    matches!(domain, Some(TierDomain::Pho | TierDomain::Sin))
}

#[cfg(test)]
mod tests;

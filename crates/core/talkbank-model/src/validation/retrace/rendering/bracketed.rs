//! Rendering of bracketed content with span tracking.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use crate::Span;
use crate::model::{BracketedContent, BracketedItem, WriteChat};
use std::fmt::Write;

/// Render bracketed content while collecting retrace annotation spans.
///
/// Spacing mirrors CHAT serialization rules so captured spans align with the
/// exact text fed into retrace diagnostics.
pub fn render_bracketed_content(
    content: &BracketedContent,
    rendered: &mut String,
    retrace_spans: &mut Vec<Span>,
) {
    for (i, item) in content.content.iter().enumerate() {
        if i > 0 && !matches!(item, BracketedItem::InternalBullet(_)) {
            rendered.push(' ');
        }
        render_bracketed_item(item, rendered, retrace_spans);
    }
}

/// Render one bracketed item and record retrace-marker spans when present.
pub fn render_bracketed_item(
    item: &BracketedItem,
    rendered: &mut String,
    retrace_spans: &mut Vec<Span>,
) {
    match item {
        BracketedItem::Word(word) => {
            word.write_chat(rendered).ok();
        }
        BracketedItem::AnnotatedWord(ann) => {
            ann.inner.write_chat(rendered).ok();
            render_scoped_annotations(&ann.scoped_annotations, rendered);
        }
        BracketedItem::ReplacedWord(rw) => {
            rw.write_chat(rendered).ok();
            render_scoped_annotations(rw.scoped_annotations.iter(), rendered);
        }
        BracketedItem::Event(event) => {
            event.write_chat(rendered).ok();
        }
        BracketedItem::AnnotatedEvent(ann) => {
            ann.inner.write_chat(rendered).ok();
            render_scoped_annotations(ann.scoped_annotations.iter(), rendered);
        }
        BracketedItem::Pause(pause) => {
            pause.write_chat(rendered).ok();
        }
        BracketedItem::OtherSpokenEvent(event) => {
            event.write_chat(rendered).ok();
        }
        BracketedItem::AnnotatedGroup(ann) => {
            rendered.push('<');
            render_bracketed_content(&ann.inner.content, rendered, retrace_spans);
            if let Some(space) = &ann.inner.trailing_space {
                rendered.push_str(space);
            }
            rendered.push('>');
            render_scoped_annotations(ann.scoped_annotations.iter(), rendered);
        }
        BracketedItem::PhoGroup(pho) => {
            rendered.push('‹');
            render_bracketed_content(&pho.content, rendered, retrace_spans);
            rendered.push('›');
        }
        BracketedItem::SinGroup(sin) => {
            rendered.push('〔');
            render_bracketed_content(&sin.content, rendered, retrace_spans);
            rendered.push('〕');
        }
        BracketedItem::Quotation(quot) => {
            rendered.push(crate::chars::LEFT_DOUBLE_QUOTE);
            render_bracketed_content(&quot.content, rendered, retrace_spans);
            rendered.push(crate::chars::RIGHT_DOUBLE_QUOTE);
        }
        BracketedItem::Separator(sep) => {
            sep.write_chat(rendered).ok();
        }
        BracketedItem::OverlapPoint(marker) => {
            marker.write_chat(rendered).ok();
        }
        BracketedItem::InternalBullet(bullet) => {
            bullet.write_chat(rendered).ok();
        }
        BracketedItem::Freecode(freecode) => {
            freecode.write_chat(rendered).ok();
        }
        BracketedItem::LongFeatureBegin(marker) => {
            marker.write_chat(rendered).ok();
        }
        BracketedItem::LongFeatureEnd(marker) => {
            marker.write_chat(rendered).ok();
        }
        BracketedItem::UnderlineBegin(_) => {
            write_with_span(rendered, |w| {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0001}')
            });
        }
        BracketedItem::UnderlineEnd(_) => {
            write_with_span(rendered, |w| {
                w.write_char('\u{0002}')?;
                w.write_char('\u{0002}')
            });
        }
        BracketedItem::NonvocalBegin(marker) => {
            marker.write_chat(rendered).ok();
        }
        BracketedItem::NonvocalEnd(marker) => {
            marker.write_chat(rendered).ok();
        }
        BracketedItem::NonvocalSimple(marker) => {
            marker.write_chat(rendered).ok();
        }
        BracketedItem::Action(action) => {
            action.write_chat(rendered).ok();
        }
        BracketedItem::AnnotatedAction(ann) => {
            ann.inner.write_chat(rendered).ok();
            render_scoped_annotations(&ann.scoped_annotations, rendered);
        }
        BracketedItem::Retrace(retrace) => {
            // Render retrace content with optional angle brackets
            if retrace.is_group {
                rendered.push('<');
            }
            render_bracketed_content(&retrace.content, rendered, retrace_spans);
            if retrace.is_group {
                rendered.push('>');
            }
            // Capture the retrace marker span for validation diagnostics
            rendered.push(' ');
            let span = write_with_span(rendered, |w| retrace.kind.write_chat(w));
            retrace_spans.push(span);
            // Render any additional non-retrace annotations
            render_scoped_annotations(retrace.annotations.iter(), rendered);
        }
    }
}

/// Render scoped annotations (none of which are retrace markers post-redesign).
fn render_scoped_annotations<'a>(
    annotations: impl IntoIterator<Item = &'a crate::model::ContentAnnotation>,
    rendered: &mut String,
) {
    for ann in annotations {
        rendered.push(' ');
        ann.write_chat(rendered).ok();
    }
}

/// Write into `rendered` and return the written byte span.
///
/// This keeps span bookkeeping consistent across bracketed rendering branches.
fn write_with_span<F>(rendered: &mut String, mut write: F) -> Span
where
    F: FnMut(&mut String) -> std::fmt::Result,
{
    let start = rendered.len();
    write(rendered).ok();
    let end = rendered.len();
    Span::from_usize(start, end)
}

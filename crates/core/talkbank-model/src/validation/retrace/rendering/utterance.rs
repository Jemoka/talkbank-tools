//! Rendering of utterance content with span tracking.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

use super::bracketed;
use crate::Span;
use crate::model::{UtteranceContent, WriteChat};

/// Render one utterance-content item while recording retrace annotation spans.
///
/// The emitted text is CHAT-formatted; span offsets are byte indices into the
/// `rendered` buffer and are later consumed by retrace validators.
pub fn render_utterance_content(
    item: &UtteranceContent,
    rendered: &mut String,
    retrace_spans: &mut Vec<Span>,
) {
    match item {
        UtteranceContent::Word(word) => {
            word.write_chat(rendered).ok();
        }
        UtteranceContent::AnnotatedWord(ann) => {
            ann.inner.write_chat(rendered).ok();
            render_scoped_annotations(ann.scoped_annotations.iter(), rendered);
        }
        UtteranceContent::ReplacedWord(rw) => {
            rw.write_chat(rendered).ok();
            render_scoped_annotations(rw.scoped_annotations.iter(), rendered);
        }
        UtteranceContent::Event(event) => {
            event.write_chat(rendered).ok();
        }
        UtteranceContent::AnnotatedEvent(ann) => {
            ann.inner.write_chat(rendered).ok();
            render_scoped_annotations(ann.scoped_annotations.iter(), rendered);
        }
        UtteranceContent::Pause(pause) => {
            pause.write_chat(rendered).ok();
        }
        UtteranceContent::OtherSpokenEvent(event) => {
            event.write_chat(rendered).ok();
        }
        UtteranceContent::Group(group) => {
            rendered.push('<');
            bracketed::render_bracketed_content(&group.content, rendered, retrace_spans);
            if let Some(space) = &group.trailing_space {
                rendered.push_str(space);
            }
            rendered.push('>');
        }
        UtteranceContent::AnnotatedGroup(ann) => {
            rendered.push('<');
            bracketed::render_bracketed_content(&ann.inner.content, rendered, retrace_spans);
            if let Some(space) = &ann.inner.trailing_space {
                rendered.push_str(space);
            }
            rendered.push('>');
            render_scoped_annotations(ann.scoped_annotations.iter(), rendered);
        }
        UtteranceContent::PhoGroup(pho) => {
            rendered.push('‹');
            bracketed::render_bracketed_content(&pho.content, rendered, retrace_spans);
            rendered.push('›');
        }
        UtteranceContent::SinGroup(sin) => {
            rendered.push('〔');
            bracketed::render_bracketed_content(&sin.content, rendered, retrace_spans);
            rendered.push('〕');
        }
        UtteranceContent::Quotation(quot) => {
            rendered.push(crate::chars::LEFT_DOUBLE_QUOTE);
            bracketed::render_bracketed_content(&quot.content, rendered, retrace_spans);
            rendered.push(crate::chars::RIGHT_DOUBLE_QUOTE);
        }
        UtteranceContent::Separator(sep) => {
            sep.write_chat(rendered).ok();
        }
        UtteranceContent::OverlapPoint(marker) => {
            marker.write_chat(rendered).ok();
        }
        UtteranceContent::InternalBullet(bullet) => {
            bullet.write_chat(rendered).ok();
        }
        UtteranceContent::Freecode(freecode) => {
            freecode.write_chat(rendered).ok();
        }
        UtteranceContent::LongFeatureBegin(marker) => {
            marker.write_chat(rendered).ok();
        }
        UtteranceContent::LongFeatureEnd(marker) => {
            marker.write_chat(rendered).ok();
        }
        UtteranceContent::UnderlineBegin(_) => {
            rendered.push('\u{0002}');
            rendered.push('\u{0001}');
        }
        UtteranceContent::UnderlineEnd(_) => {
            rendered.push('\u{0002}');
            rendered.push('\u{0002}');
        }
        UtteranceContent::NonvocalBegin(marker) => {
            marker.write_chat(rendered).ok();
        }
        UtteranceContent::NonvocalEnd(marker) => {
            marker.write_chat(rendered).ok();
        }
        UtteranceContent::NonvocalSimple(marker) => {
            marker.write_chat(rendered).ok();
        }
        UtteranceContent::AnnotatedAction(ann) => {
            ann.inner.write_chat(rendered).ok();
            render_scoped_annotations(&ann.scoped_annotations, rendered);
        }
        UtteranceContent::Retrace(retrace) => {
            // Render retrace content with optional angle brackets
            if retrace.is_group {
                rendered.push('<');
            }
            bracketed::render_bracketed_content(&retrace.content, rendered, retrace_spans);
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
/// This helper centralizes offset bookkeeping so higher-level renderers can
/// record exact source slices for downstream diagnostics.
fn write_with_span<F>(rendered: &mut String, mut write: F) -> Span
where
    F: FnMut(&mut String) -> std::fmt::Result,
{
    let start = rendered.len();
    write(rendered).ok();
    let end = rendered.len();
    Span::from_usize(start, end)
}

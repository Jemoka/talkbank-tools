//! Main-tier `UtteranceContent` snippet formatter for hover display.
//!
//! Renders a compact text summary of the main-tier item. Words use
//! `cleaned_text()`; groups/quotations/annotations are recursively summarised.
//! Non-word items (pauses, events) collapse to short placeholders (`(pause)`,
//! `&=event`) to keep hovers readable.

use talkbank_model::model::{BracketedItem, UtteranceContent};

/// Render one main-tier content item for hover display.
pub fn format_content_item(item: &UtteranceContent) -> String {
    match item {
        UtteranceContent::Word(w) => w.cleaned_text().to_string(),
        UtteranceContent::AnnotatedWord(aw) => aw.inner.cleaned_text().to_string(),
        UtteranceContent::ReplacedWord(rw) => rw.word.cleaned_text().to_string(),
        UtteranceContent::Group(g) => {
            let mut text = String::from("[");
            for (idx, gc) in g.content.content.iter().enumerate() {
                if idx > 0 {
                    text.push(' ');
                }
                text.push_str(&format_bracketed_item(gc));
            }
            text.push(']');
            text
        }
        UtteranceContent::AnnotatedGroup(ag) => {
            format_content_item(&UtteranceContent::Group(ag.inner.clone()))
        }
        UtteranceContent::Pause(p) => format!("({})", p),
        UtteranceContent::Event(e) => format!("&{}", e),
        UtteranceContent::AnnotatedEvent(ae) => format!("&{}", ae.inner),
        UtteranceContent::AnnotatedAction(_) => String::from("action"),
        UtteranceContent::Freecode(f) => f.text.to_string(),
        UtteranceContent::Separator(_) => String::new(),
        // Handle other variants (PhoGroup, SinGroup, Quotation, etc.)
        _ => String::from("..."),
    }
}

/// Render one bracketed-group item for hover display.
fn format_bracketed_item(item: &BracketedItem) -> String {
    match item {
        BracketedItem::Word(w) => w.cleaned_text().to_string(),
        BracketedItem::AnnotatedWord(aw) => aw.inner.cleaned_text().to_string(),
        BracketedItem::ReplacedWord(rw) => rw.word.cleaned_text().to_string(),
        BracketedItem::AnnotatedGroup(annotated) => {
            let mut text = String::from("[");
            for (idx, gc) in annotated.inner.content.content.iter().enumerate() {
                if idx > 0 {
                    text.push(' ');
                }
                text.push_str(&format_bracketed_item(gc));
            }
            text.push(']');
            text
        }
        BracketedItem::Pause(p) => format!("({})", p),
        BracketedItem::Event(e) => format!("&{}", e.event_type),
        BracketedItem::AnnotatedEvent(ae) => format!("&{}", ae.inner.event_type),
        BracketedItem::Action(_) => String::from("action"),
        BracketedItem::AnnotatedAction(_) => String::from("action"),
        // Handle other variants with simple representations
        _ => String::from("..."),
    }
}

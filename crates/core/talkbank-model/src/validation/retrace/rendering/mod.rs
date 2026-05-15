//! Rendering orchestration with span tracking.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>

mod bracketed;
mod utterance;

use super::types::RenderedSpans;
use crate::model::MainTier;
use crate::model::WriteChat;
use utterance::render_utterance_content;

/// Render a main tier into CHAT text while capturing retrace-marker spans.
///
/// The output spans are later joined with collected retrace annotations to build
/// precise diagnostics over rendered source fragments.
pub fn render_with_spans(main_tier: &MainTier) -> RenderedSpans {
    let mut rendered = String::new();
    let mut retrace_spans = Vec::new();

    rendered.push('*');
    main_tier.speaker.write_chat(&mut rendered).ok();
    rendered.push_str(":\t");

    for (i, linker) in main_tier.content.linkers.iter().enumerate() {
        if i > 0 {
            rendered.push(' ');
        }
        linker.write_chat(&mut rendered).ok();
    }

    if let Some(lang_code) = &main_tier.content.language_code {
        if !main_tier.content.linkers.is_empty() {
            rendered.push(' ');
        }
        rendered.push_str("[- ");
        lang_code.write_chat(&mut rendered).ok();
        rendered.push(']');
    }

    for (i, item) in main_tier.content.content.iter().enumerate() {
        let needs_space = i > 0
            || !main_tier.content.linkers.is_empty()
            || main_tier.content.language_code.is_some();
        if needs_space {
            rendered.push(' ');
        }
        render_utterance_content(item, &mut rendered, &mut retrace_spans);
    }

    if let Some(term) = &main_tier.content.terminator {
        if !main_tier.content.content.is_empty()
            || !main_tier.content.linkers.is_empty()
            || main_tier.content.language_code.is_some()
        {
            rendered.push(' ');
        }
        term.write_chat(&mut rendered).ok();
    }

    RenderedSpans { retrace_spans }
}

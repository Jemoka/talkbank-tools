//! AST content rebuilding: walks old content, replacing/splicing words to
//! match NLP tokenization.

use std::collections::HashSet;

use talkbank_model::alignment::helpers::{TierDomain, counts_for_tier, is_tag_marker_separator};
use talkbank_model::model::content::BracketedItems;
use talkbank_model::model::{BracketedItem, Mor, UtteranceContent, Word};

use crate::extract::ExtractedWord;

use super::{
    WordTokenMapping, handle_ending_punct_skip, is_tag_marker_text, resolve_token_text,
    try_parse_token_as_bracketed_item, try_parse_token_as_utterance_content,
    try_parse_token_as_word,
};

/// Mutable state threaded through the retokenize AST walk.
pub(super) struct RetokenizeContext<'a> {
    /// Tree-sitter parser for validating tokens as CHAT words.
    pub parser: &'a talkbank_parser::TreeSitterParser,
    /// Maps original word index to token indices.
    pub mapping: &'a WordTokenMapping,
    /// NLP tokenized output.
    pub stanza_tokens: &'a [String],
    /// Original words extracted from the utterance.
    pub original_words: &'a [ExtractedWord],
    /// Parsed morphosyntax items from the NLP pipeline.
    pub mors: &'a [Mor],
    /// Expected utterance terminator for parse validation.
    pub expected_terminator: Option<&'a str>,
    /// Current position in the original word list.
    pub word_counter: usize,
    /// Current position in the MOR list.
    pub mor_cursor: usize,
    /// Warnings accumulated during retokenization.
    pub diagnostics: Vec<String>,
    /// Tracks which token indices have already produced a word node.
    pub emitted_tokens: HashSet<usize>,
}

fn should_retokenize(word: &Word) -> bool {
    counts_for_tier(word, TierDomain::Mor)
}

/// Rebuild content vector, replacing alignable words with retokenized versions.
pub(super) fn rebuild_content(
    old_content: Vec<UtteranceContent>,
    ctx: &mut RetokenizeContext<'_>,
    new_content: &mut Vec<UtteranceContent>,
) {
    for item in old_content {
        match item {
            UtteranceContent::Word(word) => {
                if should_retokenize(&word) {
                    handle_word_retokenize(*word, ctx, new_content);
                } else {
                    new_content.push(UtteranceContent::Word(word));
                }
            }
            UtteranceContent::AnnotatedWord(mut annotated) => {
                if should_retokenize(&annotated.inner) {
                    handle_annotated_word_retokenize(&mut annotated.inner, ctx);
                }
                new_content.push(UtteranceContent::AnnotatedWord(annotated));
            }
            UtteranceContent::ReplacedWord(mut replaced) => {
                if replaced.replacement.words.is_empty() {
                    if should_retokenize(&replaced.word) {
                        handle_annotated_word_retokenize(&mut replaced.word, ctx);
                    }
                    new_content.push(UtteranceContent::ReplacedWord(replaced));
                } else {
                    for word in &mut replaced.replacement.words {
                        if should_retokenize(word) {
                            handle_annotated_word_retokenize(word, ctx);
                        }
                    }
                    new_content.push(UtteranceContent::ReplacedWord(replaced));
                }
            }
            UtteranceContent::Group(mut group) => {
                let old_bracketed = std::mem::take(&mut group.content.content.0);
                let mut new_bracketed = Vec::with_capacity(old_bracketed.len());
                rebuild_bracketed_content(old_bracketed, ctx, &mut new_bracketed);
                group.content.content = BracketedItems(new_bracketed);
                new_content.push(UtteranceContent::Group(group));
            }
            UtteranceContent::AnnotatedGroup(mut annotated) => {
                let old_bracketed = std::mem::take(&mut annotated.inner.content.content.0);
                let mut new_bracketed = Vec::with_capacity(old_bracketed.len());
                rebuild_bracketed_content(old_bracketed, ctx, &mut new_bracketed);
                annotated.inner.content.content = BracketedItems(new_bracketed);
                new_content.push(UtteranceContent::AnnotatedGroup(annotated));
            }
            UtteranceContent::PhoGroup(mut pho) => {
                let old_bracketed = std::mem::take(&mut pho.content.content.0);
                let mut new_bracketed = Vec::with_capacity(old_bracketed.len());
                rebuild_bracketed_content(old_bracketed, ctx, &mut new_bracketed);
                pho.content.content = BracketedItems(new_bracketed);
                new_content.push(UtteranceContent::PhoGroup(pho));
            }
            UtteranceContent::SinGroup(mut sin) => {
                let old_bracketed = std::mem::take(&mut sin.content.content.0);
                let mut new_bracketed = Vec::with_capacity(old_bracketed.len());
                rebuild_bracketed_content(old_bracketed, ctx, &mut new_bracketed);
                sin.content.content = BracketedItems(new_bracketed);
                new_content.push(UtteranceContent::SinGroup(sin));
            }
            UtteranceContent::Quotation(mut quot) => {
                let old_bracketed = std::mem::take(&mut quot.content.content.0);
                let mut new_bracketed = Vec::with_capacity(old_bracketed.len());
                rebuild_bracketed_content(old_bracketed, ctx, &mut new_bracketed);
                quot.content.content = BracketedItems(new_bracketed);
                new_content.push(UtteranceContent::Quotation(quot));
            }
            UtteranceContent::Separator(ref sep) if is_tag_marker_separator(sep) => {
                ctx.word_counter += 1;
                new_content.push(item);
            }
            other => new_content.push(other),
        }
    }
}

fn handle_word_retokenize(
    word: Word,
    ctx: &mut RetokenizeContext<'_>,
    new_content: &mut Vec<UtteranceContent>,
) {
    let orig_idx = ctx.word_counter;
    ctx.word_counter += 1;

    let token_indices = match ctx.mapping.get_nonempty(orig_idx) {
        Some(indices) => indices.to_vec(),
        None => {
            ctx.diagnostics.push(format!(
                "word {orig_idx} has no character-level match in Stanza tokens; keeping original"
            ));
            if ctx.mor_cursor < ctx.mors.len() {
                ctx.mor_cursor += 1;
            }
            new_content.push(UtteranceContent::Word(Box::new(word)));
            return;
        }
    };

    if token_indices.is_empty() {
        if ctx.mor_cursor < ctx.mors.len() {
            ctx.mor_cursor += 1;
        }
        new_content.push(UtteranceContent::Word(Box::new(word)));
        return;
    }

    if token_indices
        .iter()
        .all(|ti| ctx.emitted_tokens.contains(ti))
    {
        return;
    }

    for &ti in &token_indices {
        let token_text = resolve_token_text(&ctx.stanza_tokens[ti], orig_idx, ctx.original_words);
        if token_indices.len() == 1 && word.cleaned_text() == token_text {
            ctx.mor_cursor += 1;
            ctx.emitted_tokens.insert(ti);
            new_content.push(UtteranceContent::Word(Box::new(word)));
            return;
        }
        ctx.mor_cursor += 1;
        ctx.emitted_tokens.insert(ti);
        match try_parse_token_as_utterance_content(
            ctx.parser,
            &token_text,
            ctx.expected_terminator,
            &mut ctx.diagnostics,
        ) {
            Some(content) => new_content.push(content),
            None => {
                new_content.push(UtteranceContent::Word(Box::new(word)));
                #[allow(clippy::unwrap_used)]
                let pos = token_indices.iter().position(|&x| x == ti).unwrap();
                for &remaining_ti in &token_indices[(pos + 1)..] {
                    ctx.emitted_tokens.insert(remaining_ti);
                    ctx.mor_cursor += 1;
                }
                return;
            }
        }
    }
}

fn handle_annotated_word_retokenize(word: &mut Word, ctx: &mut RetokenizeContext<'_>) {
    let orig_idx = ctx.word_counter;
    ctx.word_counter += 1;

    let token_indices = match ctx.mapping.get_nonempty(orig_idx) {
        Some(indices) => indices.to_vec(),
        None => {
            ctx.diagnostics.push(format!(
                "word {orig_idx} has no character-level match in Stanza tokens; keeping original"
            ));
            if ctx.mor_cursor < ctx.mors.len() {
                ctx.mor_cursor += 1;
            }
            return;
        }
    };

    if token_indices.is_empty() {
        if ctx.mor_cursor < ctx.mors.len() {
            ctx.mor_cursor += 1;
        }
        return;
    }

    if token_indices
        .iter()
        .all(|ti| ctx.emitted_tokens.contains(ti))
    {
        return;
    }

    let ti = token_indices[0];
    let token_text = resolve_token_text(&ctx.stanza_tokens[ti], orig_idx, ctx.original_words);
    if word.cleaned_text() != token_text
        && !is_tag_marker_text(&token_text)
        && !handle_ending_punct_skip(&token_text, ctx.expected_terminator, &mut ctx.diagnostics)
        && let Some(parsed) = try_parse_token_as_word(ctx.parser, &token_text, &mut ctx.diagnostics)
    {
        *word = parsed;
    }

    for &ti in &token_indices {
        ctx.emitted_tokens.insert(ti);
    }
    ctx.mor_cursor += token_indices.len();
}

fn rebuild_bracketed_content(
    old_items: Vec<BracketedItem>,
    ctx: &mut RetokenizeContext<'_>,
    new_items: &mut Vec<BracketedItem>,
) {
    for item in old_items {
        match item {
            BracketedItem::Word(word) => {
                if should_retokenize(&word) {
                    handle_bracketed_word_retokenize(*word, ctx, new_items);
                } else {
                    new_items.push(BracketedItem::Word(word));
                }
            }
            BracketedItem::AnnotatedWord(mut annotated) => {
                if should_retokenize(&annotated.inner) {
                    handle_annotated_word_retokenize(&mut annotated.inner, ctx);
                }
                new_items.push(BracketedItem::AnnotatedWord(annotated));
            }
            BracketedItem::ReplacedWord(mut replaced) => {
                if replaced.replacement.words.is_empty() {
                    if should_retokenize(&replaced.word) {
                        handle_annotated_word_retokenize(&mut replaced.word, ctx);
                    }
                } else {
                    for word in &mut replaced.replacement.words {
                        if should_retokenize(word) {
                            handle_annotated_word_retokenize(word, ctx);
                        }
                    }
                }
                new_items.push(BracketedItem::ReplacedWord(replaced));
            }
            BracketedItem::AnnotatedGroup(mut annotated) => {
                let old_bracketed = std::mem::replace(
                    &mut annotated.inner.content.content,
                    BracketedItems(Vec::new()),
                );
                let mut sub_items = Vec::with_capacity(old_bracketed.0.len());
                rebuild_bracketed_content(old_bracketed.0, ctx, &mut sub_items);
                annotated.inner.content.content = BracketedItems(sub_items);
                new_items.push(BracketedItem::AnnotatedGroup(annotated));
            }
            BracketedItem::PhoGroup(mut pho) => {
                let old_bracketed =
                    std::mem::replace(&mut pho.content.content, BracketedItems(Vec::new()));
                let mut sub_items = Vec::with_capacity(old_bracketed.0.len());
                rebuild_bracketed_content(old_bracketed.0, ctx, &mut sub_items);
                pho.content.content = BracketedItems(sub_items);
                new_items.push(BracketedItem::PhoGroup(pho));
            }
            BracketedItem::SinGroup(mut sin) => {
                let old_bracketed =
                    std::mem::replace(&mut sin.content.content, BracketedItems(Vec::new()));
                let mut sub_items = Vec::with_capacity(old_bracketed.0.len());
                rebuild_bracketed_content(old_bracketed.0, ctx, &mut sub_items);
                sin.content.content = BracketedItems(sub_items);
                new_items.push(BracketedItem::SinGroup(sin));
            }
            BracketedItem::Quotation(mut quot) => {
                let old_bracketed =
                    std::mem::replace(&mut quot.content.content, BracketedItems(Vec::new()));
                let mut sub_items = Vec::with_capacity(old_bracketed.0.len());
                rebuild_bracketed_content(old_bracketed.0, ctx, &mut sub_items);
                quot.content.content = BracketedItems(sub_items);
                new_items.push(BracketedItem::Quotation(quot));
            }
            BracketedItem::Separator(ref sep) if is_tag_marker_separator(sep) => {
                ctx.word_counter += 1;
                new_items.push(item);
            }
            other => new_items.push(other),
        }
    }
}

fn handle_bracketed_word_retokenize(
    word: Word,
    ctx: &mut RetokenizeContext<'_>,
    new_items: &mut Vec<BracketedItem>,
) {
    let orig_idx = ctx.word_counter;
    ctx.word_counter += 1;

    let token_indices = match ctx.mapping.get_nonempty(orig_idx) {
        Some(indices) => indices.to_vec(),
        None => {
            ctx.diagnostics.push(format!(
                "word {orig_idx} has no character-level match in Stanza tokens; keeping original"
            ));
            if ctx.mor_cursor < ctx.mors.len() {
                ctx.mor_cursor += 1;
            }
            new_items.push(BracketedItem::Word(Box::new(word)));
            return;
        }
    };

    if token_indices.is_empty() {
        if ctx.mor_cursor < ctx.mors.len() {
            ctx.mor_cursor += 1;
        }
        new_items.push(BracketedItem::Word(Box::new(word)));
        return;
    }

    if token_indices
        .iter()
        .all(|ti| ctx.emitted_tokens.contains(ti))
    {
        return;
    }

    for &ti in &token_indices {
        let token_text = resolve_token_text(&ctx.stanza_tokens[ti], orig_idx, ctx.original_words);
        if token_indices.len() == 1 && word.cleaned_text() == token_text {
            ctx.mor_cursor += 1;
            ctx.emitted_tokens.insert(ti);
            new_items.push(BracketedItem::Word(Box::new(word)));
            return;
        }
        ctx.mor_cursor += 1;
        ctx.emitted_tokens.insert(ti);
        match try_parse_token_as_bracketed_item(
            ctx.parser,
            &token_text,
            ctx.expected_terminator,
            &mut ctx.diagnostics,
        ) {
            Some(item) => new_items.push(item),
            None => {
                new_items.push(BracketedItem::Word(Box::new(word)));
                #[allow(clippy::unwrap_used)]
                let pos = token_indices.iter().position(|&x| x == ti).unwrap();
                for &remaining_ti in &token_indices[(pos + 1)..] {
                    ctx.emitted_tokens.insert(remaining_ti);
                    ctx.mor_cursor += 1;
                }
                return;
            }
        }
    }
}

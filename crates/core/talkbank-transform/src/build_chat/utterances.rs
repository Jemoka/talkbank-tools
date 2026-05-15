use talkbank_model::Span;
use talkbank_model::model::{
    BracketedContent, BracketedItem, Bullet, DependentTier, LanguageCode, Line, Retrace,
    RetraceKind, Separator, Terminator, Utterance, UtteranceContent, Word,
};
use talkbank_parser::TreeSitterParser;

use crate::asr_postprocess;

use super::{TranscriptDescription, WordDesc};

pub(super) fn build_utterance_lines(
    desc: &TranscriptDescription,
    parser: &TreeSitterParser,
    langs: &[String],
    primary_lang: &str,
) -> Result<Vec<Line>, String> {
    let mut lines = Vec::with_capacity(desc.utterances.len());

    for utterance in &desc.utterances {
        let words = utterance.words.as_deref().unwrap_or(&[]);
        let should_apply_language_override = !words.is_empty();

        let built = if words.is_empty() {
            utterance.text.as_ref().map_or(Ok(None), |text| {
                build_text_utterance(
                    parser,
                    &utterance.speaker,
                    text,
                    utterance.start_ms,
                    utterance.end_ms,
                    langs,
                )
            })?
        } else {
            build_word_utterance(parser, &utterance.speaker, words, desc.write_wor)?
        };

        if let Some(mut line) = built {
            if should_apply_language_override {
                apply_utterance_language_override(
                    &mut line,
                    utterance.lang.as_deref(),
                    primary_lang,
                );
            }
            lines.push(line);
        }
    }

    Ok(lines)
}

fn apply_utterance_language_override(
    line: &mut Line,
    utterance_lang: Option<&str>,
    primary_lang: &str,
) {
    if let Some(utterance_lang) = utterance_lang
        && utterance_lang != primary_lang
        && let Line::Utterance(utterance) = line
    {
        utterance.main.content.language_code = Some(LanguageCode::new(utterance_lang));
    }
}

/// If `text` is a tag-marker separator (comma, tag marker, vocative marker),
/// return the corresponding [`Separator`] model type. Otherwise return `None`.
pub fn tag_marker_separator(text: &str) -> Option<Separator> {
    match text {
        "," => Some(Separator::Comma { span: Span::DUMMY }),
        "\u{201E}" => Some(Separator::Tag { span: Span::DUMMY }),
        "\u{2021}" => Some(Separator::Vocative { span: Span::DUMMY }),
        _ => None,
    }
}

/// Build a text-level utterance by parsing through tree-sitter.
///
/// This path constructs a minimal valid CHAT document around the input text
/// and parses it with `parse_strict()`. The mini-document hack is necessary
/// because tree-sitter requires complete document context (headers, `@Begin`,
/// `@End`) to parse a single utterance correctly.
///
/// **Callers:** This function is used by the `UtteranceDesc.text` API path —
/// when a caller provides a pre-formatted CHAT utterance string instead of
/// word-level tokens. It has zero production callers in the current codebase
/// (the ASR pipeline always uses word-level `WordDesc` tokens), but it
/// preserves the JSON API contract for external callers who construct
/// `TranscriptDescription` directly. The PyO3 bridge tests exercise this path.
fn build_text_utterance(
    parser: &TreeSitterParser,
    speaker: &str,
    text: &str,
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    langs: &[String],
) -> Result<Option<Line>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }

    let bullet_str = match (start_ms, end_ms) {
        (Some(start), Some(end)) => format!(" \x15{start}_{end}\x15"),
        _ => String::new(),
    };

    let lang_code = langs.first().map(String::as_str).unwrap_or("eng");
    let mini_chat = format!(
        "@UTF8\n@Begin\n@Languages:\t{lang}\n@Participants:\t{speaker} Participant Participant\n\
         @ID:\t{lang}|corpus_name|{speaker}|||||Participant|||\n*{speaker}:\t{text}{bullet}\n@End\n",
        lang = lang_code,
        speaker = speaker,
        text = text,
        bullet = bullet_str,
    );

    let parsed = crate::parse::parse_strict(parser, &mini_chat).map_err(|error| {
        format!("Failed to parse text utterance for speaker {speaker}: {error}")
    })?;

    for parsed_line in parsed.lines.into_iter() {
        if let Line::Utterance(utterance) = parsed_line {
            return Ok(Some(Line::Utterance(utterance)));
        }
    }

    Ok(None)
}

/// Parse a single word, falling back to unchecked for ASR tokens.
fn parse_asr_word(parser: &TreeSitterParser, text: &str) -> Word {
    let errors = talkbank_model::NullErrorSink;
    match parser.parse_word_fragment(text, 0, &errors).into_option() {
        Some(parsed) => parsed,
        None => {
            tracing::warn!(
                word = text,
                "ASR word is not valid CHAT syntax; using unchecked fallback"
            );
            Word::new_unchecked(text, text)
        }
    }
}

/// Parse a word and attach inline bullet timing, updating utterance-level
/// timing bookkeeping. Returns the parsed `Word` and whether timing was present.
fn parse_and_time_word(
    parser: &TreeSitterParser,
    text: &str,
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    utt_start_ms: &mut Option<u64>,
    utt_end_ms: &mut Option<u64>,
    has_timing: &mut bool,
) -> Word {
    let mut word = parse_asr_word(parser, text);
    if let (Some(start), Some(end)) = (start_ms, end_ms) {
        word.inline_bullet = Some(Bullet::new(start, end));
        *has_timing = true;
        if utt_start_ms.is_none() {
            *utt_start_ms = Some(start);
        }
        *utt_end_ms = Some(end);
    }
    word
}

/// Build a word-level utterance from individual word tokens.
///
/// When `write_wor` is `true` and word-level timing is present, a `%wor`
/// dependent tier is generated. When `false`, the `%wor` tier is omitted
/// regardless of timing (BA2 default for transcribe).
///
/// Words marked with `WordKind::Retrace` are grouped into consecutive runs
/// and wrapped in proper CHAT retrace AST nodes:
/// - A single retrace word → one `[/]` annotated-word node (`word [/]`).
/// - A run of N > 1 Retrace words that are all the **same** word (unigram
///   run, e.g. `"a a a"` where the first two `a`s are marked Retrace) →
///   N separate `[/]` annotated-word nodes (`a [/] a [/]`…). In CHAT
///   convention the bracket form `<w1 w2> [/]` means a repeated *phrase*
///   (multi-word unit); a string of identical unigrams is semantically
///   N separate repetitions of the same word.
/// - A run of N > 1 Retrace words with differing text → one bracketed
///   annotated-group node (`<I want> [/] I want cookie`).
fn build_word_utterance(
    parser: &TreeSitterParser,
    speaker: &str,
    words: &[WordDesc],
    write_wor: bool,
) -> Result<Option<Line>, String> {
    let mut content: Vec<UtteranceContent> = Vec::new();
    let mut utt_start_ms: Option<u64> = None;
    let mut utt_end_ms: Option<u64> = None;
    let mut has_timing = false;

    let last_text = words.last().map(|word| word.text.as_str()).unwrap_or(".");
    let terminator = Terminator::try_from_chat_str(last_text)
        .unwrap_or(Terminator::Period { span: Span::DUMMY });

    let mut index = 0;
    while index < words.len() {
        let word = &words[index];
        let text = word.text.as_str().trim();

        if text.is_empty() {
            index += 1;
            continue;
        }

        if Terminator::is_chat_terminator(text) {
            index += 1;
            continue;
        }

        if let Some(separator) = tag_marker_separator(text) {
            content.push(UtteranceContent::Separator(separator));
            index += 1;
            continue;
        }

        if word.kind == asr_postprocess::WordKind::Retrace {
            index = push_retrace_run(
                parser,
                words,
                index,
                &mut content,
                &mut utt_start_ms,
                &mut utt_end_ms,
                &mut has_timing,
            );
            continue;
        }

        let parsed = parse_and_time_word(
            parser,
            text,
            word.start_ms,
            word.end_ms,
            &mut utt_start_ms,
            &mut utt_end_ms,
            &mut has_timing,
        );
        content.push(UtteranceContent::Word(Box::new(parsed)));
        index += 1;
    }

    if content.is_empty() {
        return Ok(None);
    }

    let mut main = talkbank_model::model::MainTier::new(speaker, content, terminator);
    if let (Some(start), Some(end)) = (utt_start_ms, utt_end_ms) {
        main = main.with_bullet(Bullet::new(start, end));
    }

    let mut utterance = Utterance::new(main);
    if write_wor && has_timing {
        let wor_tier = utterance.main.generate_wor_tier();
        utterance.dependent_tiers.push(DependentTier::Wor(wor_tier));
    }

    Ok(Some(Line::utterance(utterance)))
}

fn push_retrace_run(
    parser: &TreeSitterParser,
    words: &[WordDesc],
    start_index: usize,
    content: &mut Vec<UtteranceContent>,
    utt_start_ms: &mut Option<u64>,
    utt_end_ms: &mut Option<u64>,
    has_timing: &mut bool,
) -> usize {
    let mut end_index = start_index;
    while end_index < words.len() && words[end_index].kind == asr_postprocess::WordKind::Retrace {
        end_index += 1;
    }

    let mut parsed: Vec<Word> = Vec::new();
    for retrace_word in &words[start_index..end_index] {
        let text = retrace_word.text.as_str().trim();
        if text.is_empty() {
            continue;
        }
        let word = parse_and_time_word(
            parser,
            text,
            retrace_word.start_ms,
            retrace_word.end_ms,
            utt_start_ms,
            utt_end_ms,
            has_timing,
        );
        parsed.push(word);
    }

    push_retrace_content(parsed, content);
    end_index
}

fn push_retrace_content(parsed: Vec<Word>, content: &mut Vec<UtteranceContent>) {
    if parsed.is_empty() {
        return;
    }

    let first_text = parsed[0].cleaned_text();
    let all_same_text = parsed.len() > 1
        && parsed
            .iter()
            .skip(1)
            .all(|word| word.cleaned_text().eq_ignore_ascii_case(first_text));

    if parsed.len() == 1 || all_same_text {
        for word in parsed {
            let bracketed = BracketedContent::new(vec![BracketedItem::Word(Box::new(word))]);
            let retrace = Retrace::new(bracketed, RetraceKind::Partial);
            content.push(UtteranceContent::Retrace(Box::new(retrace)));
        }
        return;
    }

    let items: Vec<BracketedItem> = parsed
        .into_iter()
        .map(|word| BracketedItem::Word(Box::new(word)))
        .collect();
    let bracketed = BracketedContent::new(items);
    let retrace = Retrace::new(bracketed, RetraceKind::Partial).as_group();
    content.push(UtteranceContent::Retrace(Box::new(retrace)));
}

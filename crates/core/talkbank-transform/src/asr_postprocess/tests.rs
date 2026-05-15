use super::chunking::{split_long_turns, split_on_long_pauses};
use super::expand_numbers_in_words;
use super::prepare::{extract_timed_words, split_multiword_tokens};
use super::utterance::retokenize;
use super::*;

fn elem(value: &str, ts: f64, end_ts: f64) -> AsrElement {
    AsrElement {
        value: AsrRawText::new(value),
        ts: AsrTimestampSecs(ts),
        end_ts: AsrTimestampSecs(end_ts),
        kind: AsrElementKind::Text,
    }
}

#[test]
fn bare_quote_element_is_stripped_at_stage_2c() {
    // A standalone `"` ASR element has no semantic content. Stage 2c
    // (boundary-quote strip) must drop it before it reaches the gate.
    let elements = vec![
        elem("\"", 0.0, 0.137),
        elem("Ross", 0.137, 0.685),
        elem("said", 0.685, 1.0),
    ];
    let words = prepare_words_pre_expansion(&elements, "eng");
    let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    assert!(
        !texts.iter().any(|t| *t == "\""),
        "bare `\"` element should not survive Stage 2c, got: {texts:?}"
    );
}

#[test]
fn embedded_quote_in_multi_word_element_is_stripped_at_stage_3c() {
    // ASR sometimes emits one element whose value glues a `"` to
    // adjacent punctuation: `Ross." said.`. Stage 3 splits on `.`
    // and whitespace, producing a standalone `"` part that bypasses
    // Stage 2c (which ran before the split). Stage 3c re-runs the
    // boundary-quote strip after the split to drop it.
    let elements = vec![elem("Ross.\" said.", 0.0, 1.0)];
    let words = prepare_words_pre_expansion(&elements, "eng");
    let texts: Vec<&str> = words.iter().map(|w| w.text.as_str()).collect();
    assert!(
        !texts.iter().any(|t| *t == "\""),
        "post-split `\"` part should not survive Stage 3c, got: {texts:?}"
    );
}

#[test]
fn full_transcribe_pipeline_drops_isolated_quote_element() {
    // End-to-end check via `process_raw_asr`: when ASR emits a bare
    // `"` element between sentences (Whisper's verbatim quoted-speech
    // rendering), the resulting utterances contain no `"` token.
    let asr_output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("He's", 0.0, 0.3),
                elem("a", 0.3, 0.4),
                elem("droid", 0.4, 0.7),
                elem(".", 0.7, 0.7),
                elem("\"", 0.8, 0.95),
                elem("Ross", 0.95, 1.5),
                elem("said", 1.5, 2.0),
                elem(".", 2.0, 2.0),
            ],
        }],
    };
    let utterances = process_raw_asr(&asr_output, "eng");
    let bare_quote_utt = utterances
        .iter()
        .position(|u| u.words.iter().any(|w| w.text.as_str() == "\""));
    assert!(
        bare_quote_utt.is_none(),
        "bare `\"` should not survive the full pipeline; found in utt {bare_quote_utt:?}"
    );
}

#[test]
fn test_extract_timed_words_filters_pauses() {
    let elems = vec![
        elem("hello", 0.0, 0.5),
        elem("<pause>", 0.5, 1.0),
        elem("world", 1.0, 1.5),
    ];
    let words = extract_timed_words(&elems);
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].text, "hello");
    assert_eq!(words[1].text, "world");
}

#[test]
fn test_extract_timed_words_converts_to_ms() {
    let elems = vec![elem("hello", 1.234, 2.567)];
    let words = extract_timed_words(&elems);
    assert_eq!(words[0].start_ms, Some(1234));
    assert_eq!(words[0].end_ms, Some(2567));
}

#[test]
fn test_extract_timed_words_treats_zero_duration_as_untimed() {
    let elems = vec![elem("hello", 0.0, 0.0)];
    let words = extract_timed_words(&elems);
    assert_eq!(words[0].start_ms, None);
    assert_eq!(words[0].end_ms, None);
}

#[test]
fn test_split_multiword_tokens() {
    let words = vec![AsrWord::new("hello world", Some(0), Some(1000))];
    let result = split_multiword_tokens(words, "eng");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].text, "hello");
    assert_eq!(result[0].start_ms, Some(0));
    assert_eq!(result[0].end_ms, Some(500));
    assert_eq!(result[1].text, "world");
    assert_eq!(result[1].start_ms, Some(500));
    assert_eq!(result[1].end_ms, Some(1000));
}

#[test]
fn test_split_multiword_tokens_splits_embedded_sentence_punctuation() {
    let words = vec![AsrWord::new("hello?world!", None, None)];
    let result = split_multiword_tokens(words, "eng");
    let texts: Vec<&str> = result.iter().map(|word| word.text.as_str()).collect();
    assert_eq!(texts, vec!["hello", "?", "world", "!"]);
    assert!(
        result
            .iter()
            .all(|word| word.start_ms.is_none() && word.end_ms.is_none())
    );
}

#[test]
fn test_hyphen_joining() {
    let words = vec![
        AsrWord::new("hello", Some(0), Some(500)),
        AsrWord::new("-world", Some(500), Some(1000)),
    ];
    let result = split_multiword_tokens(words, "eng");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].text, "hello-world");
    assert_eq!(result[0].start_ms, Some(0));
    assert_eq!(result[0].end_ms, Some(1000));
}

#[test]
fn test_split_long_turns() {
    let words: Vec<AsrWord> = (0..650)
        .map(|i| AsrWord::new(format!("word{i}"), Some(i as i64), Some(i as i64 + 1)))
        .collect();
    let chunks = split_long_turns(words);
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0].len(), 300);
    assert_eq!(chunks[1].len(), 300);
    assert_eq!(chunks[2].len(), 50);
}

#[test]
fn test_split_on_long_pauses_uses_sentence_starters() {
    let chunks = split_on_long_pauses(vec![vec![
        AsrWord::new("On", Some(65), Some(285)),
        AsrWord::new("television", Some(285), Some(765)),
        AsrWord::new("Have", Some(1595), Some(1885)),
        AsrWord::new("you", Some(1885), Some(1965)),
        AsrWord::new("ever", Some(1965), Some(2085)),
        AsrWord::new("been", Some(2085), Some(2285)),
        AsrWord::new("on", Some(2285), Some(2445)),
        AsrWord::new("television", Some(2445), Some(2925)),
        AsrWord::new("Well", Some(4845), Some(5135)),
        AsrWord::new("you", Some(5135), Some(5295)),
        AsrWord::new("know", Some(5295), Some(5375)),
        AsrWord::new("we", Some(5375), Some(5535)),
        AsrWord::new("bring", Some(5535), Some(5735)),
        AsrWord::new("some", Some(5735), Some(5935)),
        AsrWord::new("kids", Some(5935), Some(6135)),
        AsrWord::new("Do", Some(7875), Some(8095)),
        AsrWord::new("you", Some(8095), Some(8175)),
        AsrWord::new("like", Some(8175), Some(8375)),
        AsrWord::new("to", Some(8375), Some(8495)),
        AsrWord::new("play", Some(8495), Some(8695)),
    ]]);

    let texts: Vec<String> = chunks
        .into_iter()
        .map(|chunk| {
            chunk
                .into_iter()
                .map(|word| word.text.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();

    assert_eq!(
        texts,
        vec![
            "On television",
            "Have you ever been on television",
            "Well you know we bring some kids",
            "Do you like to play",
        ]
    );
}

#[test]
fn test_retokenize_simple() {
    let words = vec![
        AsrWord::new("hello", Some(0), Some(500)),
        AsrWord::new("world", Some(500), Some(1000)),
        AsrWord::new(".", None, None),
    ];
    let utts = retokenize(SpeakerIndex(0), words);
    assert_eq!(utts.len(), 1);
    assert_eq!(utts[0].speaker, SpeakerIndex(0));
    assert_eq!(utts[0].words.len(), 3);
    assert_eq!(utts[0].words[2].text, ".");
}

#[test]
fn test_retokenize_splits_on_period() {
    let words = vec![
        AsrWord::new("hello", Some(0), Some(500)),
        AsrWord::new(".", Some(500), Some(600)),
        AsrWord::new("world", Some(600), Some(1000)),
    ];
    let utts = retokenize(SpeakerIndex(0), words);
    assert_eq!(utts.len(), 2);
    assert_eq!(utts[0].words.len(), 2); // hello .
    assert_eq!(utts[0].words[0].text, "hello");
    assert_eq!(utts[0].words[1].text, ".");
    assert_eq!(utts[1].words.len(), 2); // world .
    assert_eq!(utts[1].words[0].text, "world");
    assert_eq!(utts[1].words[1].text, "."); // auto-appended
}

#[test]
fn test_retokenize_trailing_no_terminator() {
    let words = vec![
        AsrWord::new("hello", Some(0), Some(500)),
        AsrWord::new("world", Some(500), Some(1000)),
    ];
    let utts = retokenize(SpeakerIndex(0), words);
    assert_eq!(utts.len(), 1);
    assert_eq!(utts[0].words.last().unwrap().text, "."); // auto-appended
}

#[test]
fn test_retokenize_rtl_punct() {
    let words = vec![
        AsrWord::new("hello", Some(0), Some(500)),
        AsrWord::new("؟", None, None),
    ];
    let utts = retokenize(SpeakerIndex(0), words);
    assert_eq!(utts.len(), 1);
    assert_eq!(utts[0].words[1].text, "?"); // normalized
}

/// Golden test: matches Python `_process_raw_asr` output for simple input.
#[test]
fn test_process_raw_asr_golden_simple() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("hello", 0.0, 0.5),
                elem("world", 0.5, 1.0),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.0),
                    end_ts: AsrTimestampSecs(1.1),
                    kind: AsrElementKind::Punctuation,
                },
                elem("how", 1.5, 2.0),
                elem("are", 2.0, 2.3),
                elem("you", 2.3, 2.5),
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    assert_eq!(utts.len(), 2);

    // First utterance: Hello world .
    // (Utterance-initial cap fires on English — rule landed 2026-04-23.)
    assert_eq!(utts[0].words[0].text, "Hello");
    assert_eq!(utts[0].words[0].start_ms, Some(0));
    assert_eq!(utts[0].words[1].text, "world");
    assert_eq!(utts[0].words[1].start_ms, Some(500));
    assert_eq!(utts[0].words[2].text, ".");

    // Second utterance: How are you .
    assert_eq!(utts[1].words[0].text, "How");
    assert_eq!(utts[1].words[0].start_ms, Some(1500));
    assert_eq!(utts[1].words.last().unwrap().text, ".");
}

/// Golden test: compound merging in pipeline.
#[test]
fn test_process_raw_asr_golden_compound() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("the", 0.0, 0.3),
                elem("air", 0.3, 0.6),
                elem("plane", 0.6, 0.9),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(0.9),
                    end_ts: AsrTimestampSecs(1.0),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    assert_eq!(utts.len(), 1);
    // Utterance-initial cap (2026-04-23 rule) uppercases `The`.
    assert_eq!(utts[0].words[0].text, "The");
    assert_eq!(utts[0].words[1].text, "airplane");
}

/// Golden test: number expansion in pipeline.
#[test]
fn test_process_raw_asr_golden_number() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("I", 0.0, 0.3),
                elem("have", 0.3, 0.6),
                elem("5", 0.6, 0.9),
                elem("cats", 0.9, 1.2),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.2),
                    end_ts: AsrTimestampSecs(1.3),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    assert_eq!(utts.len(), 1);
    assert_eq!(utts[0].words[2].text, "five");
}

#[test]
fn test_process_raw_asr_splits_unpunctuated_turn_on_long_pause_starters() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("On", 0.065, 0.285),
                elem("television", 0.285, 0.765),
                elem("Have", 1.595, 1.885),
                elem("you", 1.885, 1.965),
                elem("ever", 1.965, 2.085),
                elem("been", 2.085, 2.285),
                elem("on", 2.285, 2.445),
                elem("television", 2.445, 2.925),
                elem("Well", 4.845, 5.135),
                elem("you", 5.135, 5.295),
                elem("know", 5.295, 5.375),
                elem("we", 5.375, 5.535),
                elem("bring", 5.535, 5.735),
                elem("some", 5.735, 5.935),
                elem("kids", 5.935, 6.135),
                elem("in", 6.135, 6.335),
                elem("here", 6.335, 6.455),
                elem("and", 6.455, 6.575),
                elem("we'd", 6.575, 6.815),
                elem("be", 6.815, 6.895),
                elem("playing", 6.895, 7.135),
                elem("all", 7.275, 7.495),
                elem("the", 7.495, 7.615),
                elem("time", 7.615, 7.775),
                elem("Do", 7.875, 8.095),
                elem("you", 8.095, 8.175),
                elem("like", 8.175, 8.375),
                elem("to", 8.375, 8.495),
                elem("play", 8.495, 8.695),
                elem("What", 10.405, 10.695),
                elem("do", 10.695, 10.815),
                elem("you", 10.815, 10.935),
            ],
        }],
    };

    let utts = process_raw_asr(&output, "eng");
    let texts: Vec<String> = utts
        .iter()
        .map(|utt| {
            utt.words
                .iter()
                .map(|word| word.text.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();

    assert_eq!(utts.len(), 4);
    assert_eq!(
        texts,
        vec![
            "On television .",
            "Have you ever been on television .",
            "Well you know we bring some kids in here and we'd be playing all the time Do you like to play .",
            "What do you .",
        ]
    );
}

#[test]
fn test_process_raw_asr_preserves_same_speaker_monologue_boundaries() {
    let output = AsrOutput {
        monologues: vec![
            AsrMonologue {
                speaker: SpeakerIndex(0),
                elements: vec![elem("on", 0.065, 0.285), elem("television", 0.285, 0.765)],
            },
            AsrMonologue {
                speaker: SpeakerIndex(0),
                elements: vec![
                    elem("have", 1.595, 1.885),
                    elem("you", 1.885, 1.965),
                    elem("ever", 1.965, 2.085),
                    elem("been", 2.085, 2.285),
                    elem("on", 2.285, 2.445),
                    elem("television", 2.445, 2.925),
                ],
            },
        ],
    };

    let utts = process_raw_asr(&output, "eng");
    let texts: Vec<String> = utts
        .iter()
        .map(|utt| {
            utt.words
                .iter()
                .map(|word| word.text.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();

    assert_eq!(
        texts,
        vec!["On television .", "Have you ever been on television ."]
    );
}

#[test]
fn test_split_prepared_chunk_by_assignments_preserves_speaker_and_groups() {
    let chunk = PreparedMonologueChunk {
        speaker: SpeakerIndex(2),
        words: vec![
            AsrWord::new("on", Some(0), Some(100)),
            AsrWord::new("television", Some(100), Some(200)),
            AsrWord::new("have", Some(300), Some(400)),
            AsrWord::new("you", Some(400), Some(500)),
        ],
    };

    let split = split_prepared_chunk_by_assignments(&chunk, &[0, 0, 1, 1]);
    assert_eq!(split.len(), 2);
    assert_eq!(split[0].speaker, SpeakerIndex(2));
    assert_eq!(
        split[0]
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>(),
        vec!["on", "television"]
    );
    assert_eq!(
        split[1]
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>(),
        vec!["have", "you"]
    );
}

/// Golden test: Cantonese normalization in pipeline.
#[test]
fn test_process_raw_asr_golden_cantonese() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("你", 0.0, 0.3),
                elem("真系", 0.3, 0.6),
                elem("好", 0.6, 0.9),
                elem("吵", 0.9, 1.2),
                elem("呀", 1.2, 1.5),
            ],
        }],
    };
    let utts = process_raw_asr(&output, "yue");
    assert_eq!(utts.len(), 1);
    let tokens: Vec<&str> = utts[0]
        .words
        .iter()
        .map(|word| word.text.as_str())
        .collect();
    assert_eq!(tokens, vec!["你", "真", "係", "好", "嘈", "啊", "."]);
}

/// Cantonese normalization should NOT activate for non-yue languages.
#[test]
fn test_process_raw_asr_no_cantonese_for_eng() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![elem("系", 0.0, 0.5)],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    assert_eq!(utts[0].words[0].text, "系"); // NOT normalized
}

#[test]
fn test_process_raw_asr_handles_single_chunk_cantonese_whisper_output() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![elem(
                "這麼搞笑?我還清了啊!我還覺得奇怪為什麼在一個三次頭的電話打工呢?",
                0.0,
                0.0,
            )],
        }],
    };

    let utts = process_raw_asr(&output, "yue");
    assert_eq!(utts.len(), 3);
    assert_eq!(utts[0].words.last().unwrap().text, "?");
    assert_eq!(utts[1].words.last().unwrap().text, "!");
    assert_eq!(utts[2].words.last().unwrap().text, "?");
    assert_eq!(
        utts[0]
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>(),
        vec!["這", "麼", "搞", "笑", "?"]
    );
    assert!(
        utts.iter()
            .flat_map(|utt| utt.words.iter())
            .filter(|word| !matches!(word.text.as_str(), "." | "!" | "?"))
            .count()
            > 10
    );
    assert!(
        utts.iter()
            .flat_map(|utt| utt.words.iter())
            .all(|word| !(word.start_ms == Some(0) && word.end_ms == Some(0)))
    );
}

#[test]
fn test_process_raw_asr_keeps_ascii_words_intact_for_yue() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![elem("hello", 0.0, 0.5)],
        }],
    };

    let utts = process_raw_asr(&output, "yue");
    assert_eq!(utts.len(), 1);
    assert_eq!(
        utts[0]
            .words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>(),
        vec!["hello", "."]
    );
}

// ── MOR_PUNCT / separator stripping ─────────────────────────────
//
// BA2 stripped ALL MOR_PUNCT (comma `,`, tag `„`, vocative `‡`) and
// RTL punctuation from ASR word tokens BEFORE utseg/CHAT building:
//
//   for j in MOR_PUNCT + ENDING_PUNCT + ["؟", "۔", "،", "؛"]:
//       i[0] = i[0].strip(j).lower()
//   utterance = [i for i in utterance if i[0].strip() != ""]
//
// This prevents separators from surviving into CHAT as misplaced
// Separator nodes or invariant-violating Word nodes.

/// A standalone comma word token must be stripped from ASR output.
/// BA2 stripped it in utils.py:108. Without stripping, it survives
/// into CHAT and can land at utterance boundaries after utseg split.
#[test]
fn mor_punct_comma_stripped_from_asr_words() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("hello", 0.0, 0.5),
                elem(",", 0.5, 0.6), // comma as a word token, not Punctuation
                elem("world", 0.6, 1.0),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.0),
                    end_ts: AsrTimestampSecs(1.1),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    let words: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
    assert!(
        !words.contains(&","),
        "standalone comma word should be stripped, got: {words:?}"
    );
}

/// A word with trailing comma ("dishes,") must have the comma stripped.
#[test]
fn mor_punct_trailing_comma_stripped() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("dishes,", 0.0, 0.5),
                elem("or", 0.6, 0.8),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(0.8),
                    end_ts: AsrTimestampSecs(0.9),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    let words: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
    // Utterance-initial cap (2026-04-23) uppercases the first
    // word; the assertion here is about comma-stripping, which
    // continues to work regardless of case.
    assert_eq!(
        words[0], "Dishes",
        "trailing comma should be stripped from 'dishes,', got: {words:?}"
    );
}

/// Tag marker `„` must be stripped from ASR word tokens.
#[test]
fn mor_punct_tag_marker_stripped() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("hello", 0.0, 0.5),
                elem("\u{201E}", 0.5, 0.6), // „ tag marker
                elem("world", 0.6, 1.0),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.0),
                    end_ts: AsrTimestampSecs(1.1),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    let words: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
    assert!(
        !words.contains(&"\u{201E}"),
        "tag marker „ should be stripped, got: {words:?}"
    );
}

/// Vocative marker `‡` must be stripped from ASR word tokens.
#[test]
fn mor_punct_vocative_marker_stripped() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("hello", 0.0, 0.5),
                elem("\u{2021}", 0.5, 0.6), // ‡ vocative
                elem("world", 0.6, 1.0),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.0),
                    end_ts: AsrTimestampSecs(1.1),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    let words: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
    assert!(
        !words.contains(&"\u{2021}"),
        "vocative marker ‡ should be stripped, got: {words:?}"
    );
}

/// RTL comma `،` must be stripped (BA2: in the explicit RTL punct list).
#[test]
fn rtl_comma_stripped() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("hello", 0.0, 0.5),
                elem("،", 0.5, 0.6), // Arabic comma
                elem("world", 0.6, 1.0),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.0),
                    end_ts: AsrTimestampSecs(1.1),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    let words: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
    assert!(
        !words.contains(&"،"),
        "RTL comma ، should be stripped, got: {words:?}"
    );
}

/// After stripping, words that become empty must be removed entirely.
/// BA2: `utterance = [i for i in utterance if i[0].strip() != ""]`
#[test]
fn stripped_empty_words_removed() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("hello", 0.0, 0.5),
                elem(",", 0.5, 0.6),  // becomes empty after strip
                elem(",,", 0.6, 0.7), // also becomes empty
                elem("world", 0.7, 1.0),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.0),
                    end_ts: AsrTimestampSecs(1.1),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };
    let utts = process_raw_asr(&output, "eng");
    let words: Vec<&str> = utts[0].words.iter().map(|w| w.text.as_str()).collect();
    assert_eq!(
        words,
        vec!["Hello", "world", "."],
        "empty words after stripping should be removed, got: {words:?}"
    );
}

// ── Split pipeline equivalence tests ────────────────────────────────

/// Verify that the split pipeline (pre_expansion → expand → finalize)
/// produces identical output to the monolithic `prepare_asr_chunks`.
#[test]
fn split_pipeline_matches_monolithic_simple() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("I", 0.0, 0.3),
                elem("have", 0.3, 0.6),
                elem("5", 0.6, 0.9),
                elem("cats", 0.9, 1.2),
                AsrElement {
                    value: AsrRawText::new("."),
                    ts: AsrTimestampSecs(1.2),
                    end_ts: AsrTimestampSecs(1.3),
                    kind: AsrElementKind::Punctuation,
                },
            ],
        }],
    };

    let monolithic = prepare_asr_chunks(&output, "eng");

    // Split path: pre-expand → expand → finalize per monologue.
    let mut split_result = Vec::new();
    for monologue in &output.monologues {
        let words = prepare_words_pre_expansion(&monologue.elements, "eng");
        let words = expand_numbers_in_words(words, "eng");
        split_result.extend(finalize_words_to_chunks(words, monologue.speaker, "eng"));
    }

    assert_eq!(monolithic.len(), split_result.len());
    for (m, s) in monolithic.iter().zip(split_result.iter()) {
        assert_eq!(m.speaker, s.speaker);
        let m_texts: Vec<&str> = m.words.iter().map(|w| w.text.as_str()).collect();
        let s_texts: Vec<&str> = s.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(m_texts, s_texts, "chunk word texts differ");
    }
}

/// Split pipeline with Cantonese normalization.
#[test]
fn split_pipeline_matches_monolithic_cantonese() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![
                elem("你", 0.0, 0.3),
                elem("真系", 0.3, 0.6),
                elem("好", 0.6, 0.9),
            ],
        }],
    };

    let monolithic = prepare_asr_chunks(&output, "yue");

    let mut split_result = Vec::new();
    for monologue in &output.monologues {
        let words = prepare_words_pre_expansion(&monologue.elements, "yue");
        let words = expand_numbers_in_words(words, "yue");
        split_result.extend(finalize_words_to_chunks(words, monologue.speaker, "yue"));
    }

    assert_eq!(monolithic.len(), split_result.len());
    for (m, s) in monolithic.iter().zip(split_result.iter()) {
        let m_texts: Vec<&str> = m.words.iter().map(|w| w.text.as_str()).collect();
        let s_texts: Vec<&str> = s.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(m_texts, s_texts);
    }
}

/// Split pipeline with multiple monologues.
#[test]
fn split_pipeline_matches_monolithic_multi_monologue() {
    let output = AsrOutput {
        monologues: vec![
            AsrMonologue {
                speaker: SpeakerIndex(0),
                elements: vec![elem("hello", 0.0, 0.5), elem("world", 0.5, 1.0)],
            },
            AsrMonologue {
                speaker: SpeakerIndex(1),
                elements: vec![elem("42", 2.0, 2.5), elem("things", 2.5, 3.0)],
            },
        ],
    };

    let monolithic = prepare_asr_chunks(&output, "eng");

    let mut split_result = Vec::new();
    for monologue in &output.monologues {
        let words = prepare_words_pre_expansion(&monologue.elements, "eng");
        let words = expand_numbers_in_words(words, "eng");
        split_result.extend(finalize_words_to_chunks(words, monologue.speaker, "eng"));
    }

    assert_eq!(monolithic.len(), split_result.len());
    for (m, s) in monolithic.iter().zip(split_result.iter()) {
        assert_eq!(m.speaker, s.speaker);
        let m_texts: Vec<&str> = m.words.iter().map(|w| w.text.as_str()).collect();
        let s_texts: Vec<&str> = s.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(m_texts, s_texts);
    }
}

/// Currency tokens must survive the split pipeline. Regression test for
/// a bug where the Rust fallback pass had a digits-only guard that
/// silently dropped "$12" because it starts with '$'.
#[test]
fn split_pipeline_expands_currency_tokens() {
    let output = AsrOutput {
        monologues: vec![AsrMonologue {
            speaker: SpeakerIndex(0),
            elements: vec![elem("costs", 0.0, 0.5), elem("$12", 0.5, 1.0)],
        }],
    };

    // The monolithic path calls expand_number on every word, which
    // handles currency via try_expand_currency.
    let monolithic = prepare_asr_chunks(&output, "eng");
    let m_texts: Vec<&str> = monolithic[0]
        .words
        .iter()
        .map(|w| w.text.as_str())
        .collect();
    assert!(
        m_texts.iter().any(|w| w.contains("dollars")),
        "monolithic pipeline should expand $12: {m_texts:?}"
    );

    // The split path must also expand currency via the Rust residual pass.
    let mut split_result = Vec::new();
    for monologue in &output.monologues {
        let mut words = prepare_words_pre_expansion(&monologue.elements, "eng");
        // No Python expansion — currency is Rust-only.
        // Simulate what the pipeline does: call expand_number on each word.
        for word in &mut words {
            let text = word.text.as_str();
            let expanded = expand_number(text, "eng");
            if expanded != text {
                word.text = AsrNormalizedText::new(&expanded);
            }
        }
        split_result.extend(finalize_words_to_chunks(words, monologue.speaker, "eng"));
    }
    let s_texts: Vec<&str> = split_result[0]
        .words
        .iter()
        .map(|w| w.text.as_str())
        .collect();
    assert!(
        s_texts.iter().any(|w| w.contains("dollars")),
        "split pipeline should expand $12 via Rust residual: {s_texts:?}"
    );
}

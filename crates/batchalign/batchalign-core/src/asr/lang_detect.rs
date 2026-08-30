//! Per-utterance language detection using trigram analysis.
//!
//! Uses the `whatlang` crate for fast trigram-based language identification.
//! This module provides two capabilities:
//!
//! 1. **Primary language detection** — determine the dominant language of a
//!    full transcript when the ASR engine returned "auto" without resolving.
//!    Uses majority-vote across utterances, not blob detection.
//!
//! 2. **Per-utterance language tagging** — tag each utterance with its detected
//!    language for code-switching markup (`[- lang]` precodes in CHAT).
//!
//! A confidence threshold prevents false positives on short utterances where
//! trigram analysis is unreliable.

/// Minimum confidence score for accepting a per-utterance language detection.
/// Below this threshold, the utterance is left untagged (assumes primary lang).
const UTTERANCE_CONFIDENCE_THRESHOLD: f64 = 0.5;

/// Minimum number of alphabetic characters for reliable trigram detection.
/// Utterances shorter than this are left untagged.
const MIN_CHARS_FOR_DETECTION: usize = 40;

/// Minimum number of utterances a secondary language must appear in to be
/// included in the `@Languages` header. Prevents false positives from
/// trigram confusion on short or ambiguous text.
const MIN_UTTERANCES_FOR_SECONDARY: usize = 3;

/// Detect the primary language of a transcript by majority vote across
/// utterances.
///
/// Detects the language of each utterance independently, then returns the
/// language that appears most often. This avoids the problem where
/// concatenating all text into one blob biases toward whichever language
/// has longer utterances (e.g., English code-switches in a Spanish file
/// can tip the balance if concatenated).
///
/// Returns the ISO 639-3 code of the majority language, or `None` if no
/// utterances are long enough for reliable detection.
pub fn detect_primary_language(utterance_texts: &[&str]) -> Option<String> {
    let mut lang_counts: Vec<(String, usize)> = Vec::new();

    for text in utterance_texts {
        if let Some(lang) = detect_utterance_language(text) {
            if let Some(entry) = lang_counts.iter_mut().find(|(l, _)| l == &lang) {
                entry.1 += 1;
            } else {
                lang_counts.push((lang, 1));
            }
        }
    }

    if lang_counts.is_empty() {
        return None;
    }

    // Return the language with the most utterances
    lang_counts.sort_by_key(|b| std::cmp::Reverse(b.1));
    Some(lang_counts[0].0.clone())
}

/// Detect the language of a single utterance.
///
/// Returns `Some(iso639_3_code)` when detection is confident enough,
/// `None` otherwise (caller should assume primary language).
pub fn detect_utterance_language(text: &str) -> Option<String> {
    let clean = text.trim();
    let alpha_count = clean.chars().filter(|c| c.is_alphabetic()).count();
    if alpha_count < MIN_CHARS_FOR_DETECTION {
        return None;
    }

    let info = whatlang::detect(clean)?;
    if info.confidence() < UTTERANCE_CONFIDENCE_THRESHOLD {
        return None;
    }
    Some(whatlang_to_iso639_3(info.lang()))
}

/// Collect all unique detected languages from utterance texts, ordered by
/// frequency (most common first).
///
/// `primary_lang` is always included as the first element. Secondary
/// languages must appear in at least [`MIN_UTTERANCES_FOR_SECONDARY`]
/// utterances to be included (prevents false positives from trigram
/// confusion on short text).
pub fn collect_detected_languages(utterance_texts: &[&str], primary_lang: &str) -> Vec<String> {
    let mut lang_counts: Vec<(String, usize)> = Vec::new();

    for text in utterance_texts {
        if let Some(lang) = detect_utterance_language(text) {
            if let Some(entry) = lang_counts.iter_mut().find(|(l, _)| l == &lang) {
                entry.1 += 1;
            } else {
                lang_counts.push((lang, 1));
            }
        }
    }

    lang_counts.sort_by_key(|b| std::cmp::Reverse(b.1));

    let mut result: Vec<String> = Vec::new();
    result.push(primary_lang.to_string());
    for (lang, count) in &lang_counts {
        if lang != primary_lang && !result.contains(lang) && *count >= MIN_UTTERANCES_FOR_SECONDARY
        {
            result.push(lang.clone());
        }
    }
    result
}

/// Map `whatlang::Lang` to ISO 639-3 codes used by CHAT.
fn whatlang_to_iso639_3(lang: whatlang::Lang) -> String {
    use whatlang::Lang;
    let code = match lang {
        Lang::Eng => "eng",
        Lang::Spa => "spa",
        Lang::Fra => "fra",
        Lang::Deu => "deu",
        Lang::Ita => "ita",
        Lang::Por => "por",
        Lang::Nld => "nld",
        Lang::Jpn => "jpn",
        Lang::Kor => "kor",
        Lang::Rus => "rus",
        Lang::Ara => "ara",
        Lang::Tur => "tur",
        Lang::Cmn => "zho",
        Lang::Pol => "pol",
        Lang::Ces => "ces",
        Lang::Ron => "ron",
        Lang::Hun => "hun",
        Lang::Bul => "bul",
        Lang::Hrv => "hrv",
        Lang::Srp => "srp",
        Lang::Slk => "slk",
        Lang::Slv => "slv",
        Lang::Ukr => "ukr",
        Lang::Lit => "lit",
        Lang::Lav => "lav",
        Lang::Est => "est",
        Lang::Fin => "fin",
        Lang::Dan => "dan",
        Lang::Swe => "swe",
        Lang::Cym => "cym",
        Lang::Cat => "cat",
        Lang::Hin => "hin",
        Lang::Urd => "urd",
        Lang::Ben => "ben",
        Lang::Tam => "tam",
        Lang::Tel => "tel",
        Lang::Kan => "kan",
        Lang::Mal => "mal",
        Lang::Mar => "mar",
        Lang::Nep => "nep",
        Lang::Tha => "tha",
        Lang::Vie => "vie",
        Lang::Ind => "ind",
        Lang::Tgl => "tgl",
        Lang::Kat => "kat",
        Lang::Pes => "fas",
        Lang::Heb => "heb",
        Lang::Afr => "afr",
        Lang::Guj => "guj",
        Lang::Pan => "pan",
        Lang::Ell => "ell",
        Lang::Mkd => "mkd",
        Lang::Aze => "aze",
        Lang::Uzb => "uzb",
        Lang::Nob => "nor",
        Lang::Epo => "epo",
        Lang::Jav => "jav",
        Lang::Lat => "lat",
        Lang::Ori => "ori",
        Lang::Amh => "amh",
        Lang::Hye => "hye",
        Lang::Yid => "yid",
        Lang::Khm => "khm",
        Lang::Mya => "mya",
        Lang::Sin => "sin",
        Lang::Tuk => "tuk",
        Lang::Bel => "bel",
        Lang::Aka => "aka",
        Lang::Sna => "sna",
        Lang::Zul => "zul",
    };
    code.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Basic detection
    // -----------------------------------------------------------------------

    #[test]
    fn detects_spanish_from_long_text() {
        let text = "Hola me llamo María y vivo en la ciudad de Madrid con mi familia desde hace muchos años";
        let lang = detect_utterance_language(text);
        assert_eq!(lang.as_deref(), Some("spa"));
    }

    #[test]
    fn detects_english_from_long_text() {
        let text = "Hello my name is John and I live in the city of New York with my family";
        let lang = detect_utterance_language(text);
        assert_eq!(lang.as_deref(), Some("eng"));
    }

    #[test]
    fn short_text_returns_none() {
        // Under MIN_CHARS_FOR_DETECTION — should not attempt detection
        let text = "hello world";
        let lang = detect_utterance_language(text);
        assert!(
            lang.is_none(),
            "Short text should not be confidently detected"
        );
    }

    // -----------------------------------------------------------------------
    // Primary language: majority vote (Problem 2 fix)
    // -----------------------------------------------------------------------

    #[test]
    fn primary_language_majority_vote_spanish_dominant() {
        // Simulates a Spanish-primary bilingual file: 4 Spanish utterances,
        // 1 English utterance. The old blob-concatenation approach could
        // classify this as English if the English text was long enough.
        let texts = vec![
            "Hola me llamo María y vivo en la ciudad de Madrid con mi familia",
            "Buenos días cómo estás hoy en este día tan bonito y maravilloso",
            "Estoy bien gracias por preguntar y por venir a visitarme hoy aquí",
            "Ella trabaja en una tienda de muebles en el centro de la ciudad",
            "Hello my name is John and I live in the city of New York with my family",
        ];
        let lang = detect_primary_language(&texts);
        assert_eq!(
            lang.as_deref(),
            Some("spa"),
            "Spanish should win majority vote (4 vs 1)"
        );
    }

    #[test]
    fn primary_language_majority_vote_english_dominant() {
        let texts = vec![
            "Hello my name is John and I live in the city of New York with my family",
            "I went to the store yesterday and bought some groceries for the week",
            "The weather has been really nice lately and I enjoy going for walks",
            "Hola me llamo María y vivo en la ciudad de Madrid con mi familia",
        ];
        let lang = detect_primary_language(&texts);
        assert_eq!(
            lang.as_deref(),
            Some("eng"),
            "English should win majority vote (3 vs 1)"
        );
    }

    #[test]
    fn primary_language_returns_none_when_all_short() {
        let texts = vec!["hello", "sí", "okay", "no"];
        let lang = detect_primary_language(&texts);
        assert!(
            lang.is_none(),
            "All short utterances — no reliable detection"
        );
    }

    // -----------------------------------------------------------------------
    // False positive filtering (Problem 3 fix)
    // -----------------------------------------------------------------------

    #[test]
    fn collect_languages_excludes_rare_false_positives() {
        // One utterance misdetected as Portuguese shouldn't appear in
        // @Languages. Only languages with >= MIN_UTTERANCES_FOR_SECONDARY
        // utterances should be listed.
        let texts = vec![
            "Hola me llamo María y vivo en la ciudad de Madrid con mi familia",
            "Buenos días cómo estás hoy en este día tan bonito y maravilloso",
            "Estoy bien gracias por preguntar y por venir a visitarme hoy aquí",
            "Ella trabaja en una tienda de muebles en el centro de la ciudad",
            "Hello my name is John and I live in the city of New York with my family",
            "I went to the store yesterday and bought some groceries for the week",
            "The weather has been really nice lately and I enjoy going for walks",
        ];
        let langs = collect_detected_languages(&texts, "spa");
        assert_eq!(langs[0], "spa", "Primary must be first");
        // English appears 3 times — above threshold
        assert!(
            langs.contains(&"eng".to_string()),
            "English (3+ utterances) should be included: {langs:?}"
        );
        // No false positives like "dan" or "por" should appear
        assert!(
            !langs.contains(&"dan".to_string()),
            "Danish should not appear: {langs:?}"
        );
        assert!(
            !langs.contains(&"por".to_string()),
            "Portuguese should not appear: {langs:?}"
        );
    }

    #[test]
    fn collect_languages_primary_always_first() {
        let texts = vec![
            "Hola me llamo María y vivo en la ciudad de Madrid con mi familia",
            "Buenos días cómo estás hoy en este día tan bonito y maravilloso",
            "Hello my name is John and I live in the city of New York with my family",
            "I went to the store yesterday and bought some groceries for the week",
            "The weather has been really nice lately and I enjoy going for walks",
        ];
        let langs = collect_detected_languages(&texts, "spa");
        assert_eq!(langs[0], "spa");
    }

    #[test]
    fn collect_languages_single_occurrence_excluded() {
        // One English utterance among Spanish should NOT add English to
        // @Languages (below MIN_UTTERANCES_FOR_SECONDARY threshold).
        let texts = vec![
            "Hola me llamo María y vivo en la ciudad de Madrid con mi familia",
            "Buenos días cómo estás hoy en este día tan bonito y maravilloso",
            "Estoy bien gracias por preguntar y por venir a visitarme hoy aquí",
            "Hello my name is John and I live in the city of New York with my family",
        ];
        let langs = collect_detected_languages(&texts, "spa");
        assert_eq!(
            langs,
            vec!["spa"],
            "Single English utterance should be excluded from @Languages"
        );
    }

    // -----------------------------------------------------------------------
    // Rev.AI integration: "auto" echo handling
    // -----------------------------------------------------------------------

    #[test]
    fn primary_detection_works_as_revai_fallback() {
        // When Rev.AI echoes "auto" back, we fall through to whatlang.
        // The pipeline calls detect_primary_language() on all ASR tokens.
        // This test verifies the fallback produces correct results for
        // a clearly Spanish transcript.
        let spanish_utterances = vec![
            "Me dice ella que trabaja en una tienda de muebles y entonces le digo yo",
            "Sí porque ella no quería ir al trabajo y estaba muy cansada",
            "Buenos días cómo estás hoy me alegro de verte por aquí otra vez",
            "Ella trabaja en una tienda de muebles en el centro de la ciudad",
            "Le dije pasa por ahí me dice ah yo no puedo salir de la oficina",
        ];
        let lang = detect_primary_language(&spanish_utterances);
        assert_eq!(lang.as_deref(), Some("spa"));
    }
}

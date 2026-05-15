use talkbank_model::ErrorCollector;
use talkbank_model::model::{Bullet, ParseHealthState, WordCategory};
use talkbank_parser::TreeSitterParser;

fn parsed_filler_fixture(main_filler: &str, wor_token: &str) -> talkbank_model::model::ChatFile {
    let bullet1 = "\u{0015}0_120\u{0015}";
    let bullet2 = "\u{0015}120_260\u{0015}";
    let input = format!(
        "@UTF8\n@Begin\n@Languages:\teng\n*PAR:\t{main_filler} there .\n%wor:\t{wor_token} {bullet1} there {bullet2} .\n@End\n"
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(&input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4009_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*CHI:\t<one &+ss> [/] one play ground . \u{0015}321008_322890\u{0015}\n",
        "%mor:\tnum|one noun|play noun|ground .\n",
        "%gra:\t1|3|NUMMOD 2|3|COMPOUND 3|0|ROOT 4|3|PUNCT\n",
        "%wor:\tone \u{0015}321008_321148\u{0015} &+ss \u{0015}321148_321368\u{0015} one \u{0015}321809_321969\u{0015} play \u{0015}322049_322310\u{0015} ground \u{0015}322390_322890\u{0015} .\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4009 fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4016_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*EXP:\t&+ih <the what> [/] what's letter &+th is this ? \u{0015}49103_51586\u{0015}\n",
        "%mor:\tdet|what-Def-Int noun|letter aux|be-Fin-Ind-Pres-S3 pron|this-Dem-S1 ?\n",
        "%gra:\t1|2|DET 2|4|NSUBJ 3|4|COP 4|0|ROOT 5|4|PUNCT\n",
        "%wor:\tthe \u{0015}49103_49163\u{0015} what \u{0015}49183_50205\u{0015} what's \u{0015}50205_50405\u{0015} letter \u{0015}50405_50685\u{0015} is \u{0015}50946_51046\u{0015} this \u{0015}51086_51586\u{0015} ?\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4016 fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4016_timed_filler_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*CHI:\t&-um <I &+th> [/] I think \u{21ab}th\u{21ab}there's animals at the zoo . \u{0015}2182271_2189380\u{0015}\n",
        "%mor:\tpron|I-Prs-Nom-S1 verb|think-Fin-Ind-Pres-S1 noun|ththere noun|animal-Plur-Acc adp|at det|the-Def-Art noun|zoo .\n",
        "%gra:\t1|2|NSUBJ 2|0|ROOT 3|4|COMPOUND 4|2|OBJ 5|7|CASE 6|7|DET 7|4|NMOD 8|2|PUNCT\n",
        "%wor:\tum \u{0015}2182271_2182411\u{0015} I \u{0015}2182511_2182531\u{0015} &+th \u{0015}2182531_2184874\u{0015} I \u{0015}2184894_2184914\u{0015} think \u{0015}2185075_2185355\u{0015} there's \u{0015}2187238_2187518\u{0015} animals \u{0015}2187558_2188399\u{0015} at \u{0015}2188479_2188619\u{0015} the \u{0015}2188680_2188780\u{0015} zoo \u{0015}2188880_2189380\u{0015} .\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4016 timed-filler fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4016_omitted_filler_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*CHI:\t&-mm [<] bananas are good . \u{0015}1949566_1950567\u{0015}\n",
        "%mor:\tnoun|banana-Plur aux|be-Fin-Ind-Pres-P3 adj|good-S1 .\n",
        "%gra:\t1|3|NSUBJ 2|3|COP 3|0|ROOT 4|3|PUNCT\n",
        "%wor:\tbananas \u{0015}1949566_1949766\u{0015} are \u{0015}1949846_1949987\u{0015} good \u{0015}1950067_1950567\u{0015} .\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4016 omitted-filler fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4016_replacement_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*EXP:\twhat's is dis [: this] ? \u{0015}37050_38131\u{0015}\n",
        "%mor:\tpron|what-Int-S1 aux|be-Fin-Ind-Pres-S3 pron|this-Dem-S1 ?\n",
        "%gra:\t1|0|ROOT 2|3|COP 3|1|NSUBJ 4|1|PUNCT\n",
        "%wor:\twhat's \u{0015}37050_37471\u{0015} is \u{0015}37491_37631\u{0015} dis \u{0015}37631_38131\u{0015} ?\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4016 replacement fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4016_standalone_xxx_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*CHI:\txxx snack . \u{0015}884668_885168\u{0015}\n",
        "%mor:\tnoun|snack .\n",
        "%gra:\t1|0|ROOT 2|1|PUNCT\n",
        "%wor:\tsnack \u{0015}884668_885168\u{0015} .\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4016 standalone-xxx fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4016_standalone_nonword_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*CHI:\t&~um a boat . \u{0015}1073779_1077361\u{0015}\n",
        "%mor:\tdet|a-Masc-Ind-Art noun|boat .\n",
        "%gra:\t1|2|DET 2|0|ROOT 3|2|PUNCT\n",
        "%wor:\ta \u{0015}1073779_1073799\u{0015} boat \u{0015}1076861_1077361\u{0015} .\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4016 standalone-nonword fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

fn parsed_ocsc_4026_fixture() -> talkbank_model::model::ChatFile {
    let input = concat!(
        "@UTF8\n",
        "@Begin\n",
        "@Languages:\teng\n",
        "*CHI:\tand one time we growed <a pumpkin and a xxx> [/] some pumpkins and some watermelons . \u{0015}940315_946947\u{0015}\n",
        "%mor:\tcconj|and num|one noun|time pron|we-Prs-Nom-P1 verb|grow-Fin-Ind-Past-P1 det|some-Def-Ind noun|pumpkin-Plur-Acc cconj|and det|some-Def-Ind noun|watermelon-Plur .\n",
        "%gra:\t1|5|CC 2|3|NUMMOD 3|5|OBL-UNMARKED 4|5|NSUBJ 5|0|ROOT 6|7|DET 7|5|OBJ 8|10|CC 9|10|DET 10|7|CONJ 11|5|PUNCT\n",
        "%wor:\tand \u{0015}940315_940435\u{0015} one \u{0015}940535_940655\u{0015} time \u{0015}940695_940956\u{0015} we \u{0015}940976_941036\u{0015} growed \u{0015}941176_941557\u{0015} a \u{0015}941998_942018\u{0015} pumpkin \u{0015}942138_942679\u{0015} and \u{0015}942880_943040\u{0015} a \u{0015}944022_944042\u{0015} xxx \u{0015}944122_944463\u{0015} some \u{0015}944543_944763\u{0015} pumpkins \u{0015}944803_945485\u{0015} and \u{0015}945545_945725\u{0015} some \u{0015}946126_946346\u{0015} watermelons \u{0015}946447_946947\u{0015} .\n",
        "@End\n",
    );

    let parser = TreeSitterParser::new().expect("tree-sitter grammar should load");
    let parse_errors = ErrorCollector::new();
    let file = parser.parse_chat_file_streaming(input, &parse_errors);
    let parse_error_vec = parse_errors.into_vec();
    assert!(
        parse_error_vec.is_empty(),
        "Expected OCSC 4026 fixture to parse cleanly, got: {parse_error_vec:#?}"
    );

    file
}

#[test]
fn parsed_wor_filler_keeps_category_and_timing() {
    let file = parsed_filler_fixture("&-dt", "&-dt");
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");
    assert_eq!(utterance.parse_health, ParseHealthState::Clean);

    let wor = utterance
        .wor_tier()
        .expect("fixture should contain a %wor tier");
    let words: Vec<_> = wor.words().collect();

    assert_eq!(words.len(), 2);
    assert_eq!(words[0].raw_text(), "&-dt");
    assert_eq!(words[0].cleaned_text(), "dt");
    assert_eq!(words[0].category, Some(WordCategory::Filler));
    assert_eq!(words[0].inline_bullet, Some(Bullet::new(0, 120)));
}

#[test]
fn validate_alignments_accepts_timed_filler_wor_tokens() {
    for (main_filler, wor_token) in [
        ("&-dt", "&-dt"),
        ("&-dt", "dt"),
        ("&-you_know", "&-you_know"),
        ("&-you_know", "you_know"),
    ] {
        let file = parsed_filler_fixture(main_filler, wor_token);
        let errors = file.validate_alignments();

        assert!(
            errors.is_empty(),
            "Expected timed filler fixture to align cleanly for main={main_filler:?}, wor={wor_token:?}, got: {errors:#?}"
        );
    }
}

#[test]
fn validate_alignments_accepts_ocsc_4009_fragment_timing() {
    let file = parsed_ocsc_4009_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");
    let wor = utterance
        .wor_tier()
        .expect("fixture should contain a %wor tier");
    let words: Vec<_> = wor.words().collect();

    assert_eq!(words[1].raw_text(), "&+ss");
    assert_eq!(words[1].category, Some(WordCategory::PhonologicalFragment));
    assert_eq!(words[1].inline_bullet, Some(Bullet::new(321148, 321368)));

    let errors = file.validate_alignments();
    assert!(
        errors.is_empty(),
        "Expected exact OCSC 4009 fragment timing case to align cleanly, got: {errors:#?}"
    );
}

/// OCSC 4016 has `&+ih` and `&+th` (phonological fragments) omitted from its
/// `%wor` tier. These fragments ARE included in `TierDomain::Wor`, so the
/// `%wor` word count is lower than the main-tier Wor count. However, `%wor`
/// count mismatch validation has been removed (2026-04) because `%wor` is a
/// timing-annotation tier with no downstream positional indexing. The file
/// must now validate cleanly regardless of the count difference.
#[test]
fn validate_alignments_accepts_ocsc_4016_fragment_omissions_from_wor_no_count_validation() {
    let file = parsed_ocsc_4016_fixture();
    let errors = file.validate_alignments();

    assert!(
        errors.is_empty(),
        "Expected OCSC 4016 legacy fragment omission case to accept without count validation, got: {errors:#?}"
    );
}

#[test]
fn validate_alignments_accepts_ocsc_4016_timed_filler_and_retraced_fragment() {
    let file = parsed_ocsc_4016_timed_filler_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");
    let wor = utterance
        .wor_tier()
        .expect("fixture should contain a %wor tier");
    let words: Vec<_> = wor.words().collect();

    assert_eq!(words[0].cleaned_text(), "um");
    assert_eq!(words[2].raw_text(), "&+th");

    let errors = file.validate_alignments();
    assert!(
        errors.is_empty(),
        "Expected exact OCSC 4016 timed-filler case to align cleanly, got: {errors:#?}"
    );
}

/// OCSC 4016 omitted-filler: `&-mm` is a filler included in `TierDomain::Wor`,
/// but the `%wor` tier omits it. With `%wor` count validation removed (2026-04),
/// the mismatch no longer produces an error.
#[test]
fn validate_alignments_accepts_ocsc_4016_omitted_filler_from_wor_no_count_validation() {
    let file = parsed_ocsc_4016_omitted_filler_fixture();
    let errors = file.validate_alignments();

    assert!(
        errors.is_empty(),
        "Expected OCSC 4016 omitted-filler case to accept without count validation, got: {errors:#?}"
    );
}

#[test]
fn validate_alignments_accepts_ocsc_4016_original_surface_replacement_word_on_wor() {
    let file = parsed_ocsc_4016_replacement_fixture();
    let errors = file.validate_alignments();

    assert!(
        errors.is_empty(),
        "Expected exact OCSC 4016 replacement case to align cleanly, got: {errors:#?}"
    );
}

/// OCSC 4016 standalone-xxx: main tier `xxx snack .` with `%wor` = `snack .`
/// Under the new policy, `xxx` is excluded from `TierDomain::Wor` (no phoneme
/// sequence to align), so the main Wor count is 1 and the `%wor` count is 1.
/// Both the count match and the removed `%wor` validation mean this case now
/// produces no errors.
#[test]
fn validate_alignments_accepts_ocsc_4016_standalone_xxx_without_wor_slot() {
    let file = parsed_ocsc_4016_standalone_xxx_fixture();
    let errors = file.validate_alignments();

    assert!(
        errors.is_empty(),
        "Expected OCSC 4016 standalone-xxx case to accept (xxx excluded from Wor count), got: {errors:#?}"
    );
}

/// OCSC 4016 standalone-nonword: `&~um a boat .` with `%wor` = `a boat .` (no `um`).
/// `&~um` is a nonword (real phoneme sequence, included in Wor domain), so the
/// main Wor count is 3 while the `%wor` count is 2 — a legitimate count mismatch.
/// However, `%wor` count validation has been removed (2026-04); the mismatch no
/// longer produces an error.
#[test]
fn validate_alignments_accepts_ocsc_4016_standalone_nonword_omitted_from_wor_no_count_validation() {
    let file = parsed_ocsc_4016_standalone_nonword_fixture();
    let errors = file.validate_alignments();

    assert!(
        errors.is_empty(),
        "Expected OCSC 4016 standalone-nonword omission case to accept without count validation, got: {errors:#?}"
    );
}

#[test]
fn validate_alignments_accepts_standalone_spoken_tokens_on_wor() {
    for (main_token, wor_token) in [
        ("&+ih", "&+ih"),
        ("&~um", "&~um"),
        ("xxx", "xxx"),
        ("yyy", "yyy"),
        ("www", "www"),
    ] {
        let file = parsed_filler_fixture(main_token, wor_token);
        let errors = file.validate_alignments();

        assert!(
            errors.is_empty(),
            "Expected spoken token fixture to align cleanly for main={main_token:?}, wor={wor_token:?}, got: {errors:#?}"
        );
    }
}

#[test]
fn validate_alignments_accepts_ocsc_4026_retraced_xxx_timing() {
    let file = parsed_ocsc_4026_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");
    let wor = utterance
        .wor_tier()
        .expect("fixture should contain a %wor tier");
    let words: Vec<_> = wor.words().collect();

    assert_eq!(words[9].raw_text(), "xxx");
    assert!(words[9].untranscribed().is_some());
    assert_eq!(words[9].inline_bullet, Some(Bullet::new(944122, 944463)));

    let errors = file.validate_alignments();
    assert!(
        errors.is_empty(),
        "Expected exact OCSC 4026 retraced-xxx timing case to align cleanly, got: {errors:#?}"
    );
}

/// Fragments (`&+`) are excluded from `%wor` even inside retraced groups.
/// `%wor` includes all spoken content (retrace context included), but
/// fragments and nonwords are excluded regardless of retrace ancestry.
#[test]
fn generate_wor_tier_from_ocsc_4009_excludes_retraced_fragment() {
    let file = parsed_ocsc_4009_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");

    let generated = utterance.main.generate_wor_tier();
    let words: Vec<_> = generated
        .words()
        .map(|word| word.cleaned_text().to_string())
        .collect();

    // Fragment "ss" (&+ss) is excluded; "one" from the retrace and regular words remain.
    assert_eq!(words, vec!["one", "one", "play", "ground"]);
}

/// Fragments (`&+`) are excluded from `%wor`. The fixture contains `&+ih`
/// and `&+th` — both are excluded. Only regular words remain.
#[test]
fn generate_wor_tier_from_ocsc_4016_excludes_standalone_fragments() {
    let file = parsed_ocsc_4016_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");

    let generated = utterance.main.generate_wor_tier();
    let words: Vec<_> = generated
        .words()
        .map(|word| word.cleaned_text().to_string())
        .collect();

    // Fragments "ih" (&+ih) and "th" (&+th) excluded; regular words remain.
    assert_eq!(words, vec!["the", "what", "what's", "letter", "is", "this"]);
}

#[test]
fn generate_wor_tier_from_ocsc_4016_uses_original_surface_words() {
    let file = parsed_ocsc_4016_replacement_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");

    let generated = utterance.main.generate_wor_tier();
    let words: Vec<_> = generated
        .words()
        .map(|word| word.cleaned_text().to_string())
        .collect();

    assert_eq!(words, vec!["what's", "is", "dis"]);
}

/// `xxx` is untranscribed — no phoneme sequence to align. `generate_wor_tier`
/// now excludes it. Main tier `xxx snack .` → %wor contains only `snack`.
#[test]
fn generate_wor_tier_from_ocsc_4016_excludes_standalone_xxx() {
    let file = parsed_ocsc_4016_standalone_xxx_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");

    let generated = utterance.main.generate_wor_tier();
    let words: Vec<_> = generated
        .words()
        .map(|word| word.cleaned_text().to_string())
        .collect();

    // xxx excluded (no alignable phoneme sequence); snack is present.
    assert_eq!(words, vec!["snack"]);
}

/// Nonwords (`&~`) are excluded from `%wor`. The fixture contains `&~um` —
/// it is excluded. Only regular words remain.
#[test]
fn generate_wor_tier_from_ocsc_4016_excludes_standalone_nonword() {
    let file = parsed_ocsc_4016_standalone_nonword_fixture();
    let utterance = file
        .utterances()
        .next()
        .expect("fixture should contain one utterance");

    let generated = utterance.main.generate_wor_tier();
    let words: Vec<_> = generated
        .words()
        .map(|word| word.cleaned_text().to_string())
        .collect();

    // Nonword "um" (&~um) excluded; "a" and "boat" remain.
    assert_eq!(words, vec!["a", "boat"]);
}

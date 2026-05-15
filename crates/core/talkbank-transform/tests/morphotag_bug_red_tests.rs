//! Boundary contract tests for known morphotag-pipeline bugs.
//!
//! Each test is annotated with the bug ID from
//! `docs/session-handoff-2026-05-01.md` and the postmortem
//! `docs/postmortems/2026-05-01-morphotag-rerun-debacle.md`.
//!
//! Both bugs in this file reduce to the same architectural debt class:
//! "the typed AST is decorative, not contract." The fix routes through
//! the `StanzaInput` / `StanzaOutput` newtype directives recorded at
//! the end of session 2026-05-01.
//!
//! ## Empirical status (2026-05-01)
//!
//! BUG-009 was hypothesized to manifest at the
//! [`talkbank_transform::morphosyntax::collect_payloads`] boundary
//! (the typed-AST → Stanza-payload seam). The two BUG-009 tests below
//! show that this boundary is **already correct** for the realistic
//! `nobfield.cha` patterns: the parser correctly classifies `→` as a
//! separator, `extract::collect_utterance_content` correctly skips it,
//! and the resulting `MorphosyntaxBatchItem.words` does not contain
//! the separator. So these tests function as **forward-regression
//! gates** for the boundary, not RED reproducers of the bug.
//!
//! The actual `→`-into-`%mor` leak therefore lives downstream of
//! `collect_payloads`. Candidates (not verified in this session):
//! (a) Stanza-worker-side text reconstruction in
//! `crates/batchalign/src/morphosyntax/worker.rs`;
//! (b) post-Stanza injection paths in
//! `crates/talkbank-transform/src/morphosyntax/injection.rs`;
//! (c) the synthesis layer's surface-text round-trip in
//! `crates/talkbank-transform/src/morphosyntax/synthesis/`.
//! A reproducer at the actual leak boundary requires either running
//! Stanza or constructing a synthetic `UdResponse` to feed
//! `inject_results`. Both are larger work items deferred to the
//! architectural review's bug-ledger phase.
//!
//! BUG-011 (mor/gra count mismatch) is the same architectural class
//! and faces the same reproducer issue — see the test stub below.

use talkbank_model::ParseValidateOptions;
use talkbank_model::model::LanguageCode;
use talkbank_parser::TreeSitterParser;
use talkbank_transform::morphosyntax::{
    MappingContext, MultilingualPolicy, UdId, UdPunctable, UdSentence, UdWord, UniversalPos,
    collect_payloads, declared_languages, map_ud_sentence,
};

/// Helper: parse a one-utterance CHAT fragment using the canonical
/// tree-sitter parser. Mirrors the helper used in
/// `crates/talkbank-transform/src/morphosyntax/tests.rs` so a future
/// reader can find one consistent idiom.
fn parse_one_utterance(main_tier: &str) -> talkbank_model::model::ChatFile {
    let chat = format!(
        "@UTF8\n\
         @Begin\n\
         @Languages:\teng\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|test|CHI||female|||Target_Child|||\n\
         *CHI:\t{main_tier}\n\
         @End\n"
    );
    let parser = TreeSitterParser::new().expect("parser init");
    parser.parse_chat_file(&chat).expect("parse")
}

// =====================================================================
// BUG-009 — Text not cleaned before Stanza: bare CA separators leak
//            into the Stanza payload (and from there into %mor).
//
// Evidence: aphasia/nobfield.cha contains `Yes→` (level-pitch separator
// with no space). When the morphotag pipeline runs, the resulting
// `%mor` tier contains a `→` token, which is unparseable per CHAT
// %mor grammar (separators are not %mor items).
//
// Contract under test: `collect_payloads` MUST NOT emit a Stanza-payload
// word that is a CA separator (`→`, `↗`, `↘`, `↖`, …) or that contains
// such a separator as a substring. The parser already classifies these
// as `word_segment_forbidden_*` per `resources/spec/symbols/symbol_registry.json`;
// the payload collector must consume that classification, not re-walk
// raw text.
//
// Root-cause class: typed AST is decorative, not contract — the
// morphotag pipeline string-hacks raw_text somewhere between the typed
// CST (where `→` IS a separator node) and the Stanza payload (where it
// reappears as a word).
// =====================================================================

/// Forward-regression gate (currently GREEN — empirical 2026-05-01):
/// a `Yes→` token (no space before the level-pitch separator, AND no
/// terminator — `→` ends the utterance) must not produce a Stanza-
/// payload word that contains the separator character.
///
/// Reproduces the nobfield.cha line 11 pattern verbatim:
///     *PAR: Yes→
/// (no terminator, separator at end-of-utterance). This is the
/// original BUG-009 trigger from the 2026-05-01 postmortem.
#[test]
fn bug_009_level_pitch_separator_no_space_must_not_leak_into_stanza_payload() {
    let chat_file = parse_one_utterance("Yes→");

    let primary = LanguageCode::new("eng");
    let langs = declared_languages(&chat_file, &primary);
    let collected = collect_payloads(&chat_file, &primary, &langs, MultilingualPolicy::ProcessAll);

    // The utterance has alignable content (`Yes`), so it must produce
    // exactly one batch item.
    assert_eq!(
        collected.batch_items.len(),
        1,
        "expected one batch item for the single utterance"
    );

    let (_line_idx, _utt_idx, batch_item, _extracted) = &collected.batch_items[0];

    // CONTRACT: no payload word may equal `→` or contain it as a
    // substring. The parser knows `→` is a separator
    // (`word_segment_forbidden_first_symbols` per the symbol
    // registry), and that classification must reach the Stanza
    // boundary.
    for (idx, word) in batch_item.words.iter().enumerate() {
        assert!(
            !word.as_str().contains('→'),
            "BUG-009: Stanza payload word #{idx} = {word:?} contains a \
             CA level-pitch separator. The parser classifies `→` as a \
             separator (resources/spec/symbols/symbol_registry.json \
             `word_segment_forbidden_first_symbols`); the morphotag \
             payload collector must consume the typed AST, not \
             re-tokenize raw text. See \
             docs/postmortems/2026-05-01-morphotag-rerun-debacle.md \
             Mistake 19 for full context."
        );
    }

    // Stronger contract: the payload word should be exactly `Yes`
    // (the separator is stripped at the AST boundary, not the word
    // boundary).
    let actual_strs: Vec<&str> = batch_item.words.iter().map(|w| w.as_str()).collect();
    assert_eq!(
        actual_strs,
        vec!["Yes"],
        "BUG-009: expected the single payload word to be `Yes`, \
         with the `→` separator already stripped by the typed-AST \
         walk; got {:?}",
        actual_strs
    );
}

/// Forward-regression gate (currently GREEN — empirical 2026-05-01):
/// the same contract for the longer nobfield.cha line 12 pattern,
/// where `→` follows a multi-word utterance with no space and no
/// terminator (followed by an audio time bullet):
///     *PAT: I think you could use new clothes→ 0_2633
/// The `→` and the bullet must NOT appear as Stanza-payload words.
#[test]
fn bug_009_level_pitch_separator_in_long_utterance_with_bullet_must_not_leak() {
    let chat_file =
        parse_one_utterance("I think you could use new clothes→ \u{0015}0_2633\u{0015}");
    let primary = LanguageCode::new("eng");
    let langs = declared_languages(&chat_file, &primary);
    let collected = collect_payloads(&chat_file, &primary, &langs, MultilingualPolicy::ProcessAll);

    assert_eq!(
        collected.batch_items.len(),
        1,
        "expected one batch item for the single utterance"
    );
    let (_, _, batch_item, _) = &collected.batch_items[0];

    for (idx, word) in batch_item.words.iter().enumerate() {
        assert!(
            !word.as_str().contains('→'),
            "BUG-009: Stanza payload word #{idx} = {word:?} contains \
             the level-pitch separator. Source location: \
             nobfield.cha:12 pattern."
        );
    }

    // Must also not contain the audio bullet markers as words.
    for word in &batch_item.words {
        assert!(
            !word.as_str().contains('\u{0015}') && !word.as_str().contains('_'),
            "BUG-009: Stanza payload word {word:?} contains audio \
             bullet artifact. Audio bullets are typed nodes in the \
             CST and must not reach the Stanza payload."
        );
    }
}

// =====================================================================
// BUG-011 — `%mor`/`%gra` count alignment broken: the morphotag
//            pipeline emits two independent counts.
//
// Evidence: 6 of the 4,592 files pushed during the 2026-05-01 morphotag
// rerun produce E720 ("Mor-Gra count mismatch") under
// `chatter validate ~/0tb/data`. Triggering inputs involve complex
// code-switching and untranscribed contexts. See
// `resources/spec/errors/E720_auto.md` for the validator's contract.
//
// Contract under test: for any input that parses + injects without a
// hard error, the resulting `%mor` chunk count MUST equal the
// resulting `%gra` relation count, per the alignment rule
// `%gra` aligns 1-to-1 with `%mor` chunks (not items — clitics in
// `%mor` produce additional chunks).
//
// Root-cause class: same as BUG-009 — independent string-hacking paths
// for `%mor` and `%gra` assembly instead of a single typed pipeline
// where chunk-count is an invariant of the type system.
//
// Test status: this contract would ideally be exercised end-to-end
// through a real Stanza inference run, which is not available in the
// `talkbank-transform` test harness (Stanza lives in the batchalign
// crates and the worker process). A reproducer fixture from one of
// the 6 affected files needs to be extracted into the test harness
// before this test can be written without invoking the network/
// Python pipeline.
//
// Rather than write a hollow assertion that can't fail today, the
// test below is `#[ignore]`'d with an explanatory message. The
// ignore-with-message pattern is the project's standard for
// "not_implemented spec" gates (see `talkbank-tools/CLAUDE.md` §
// "Known Testing Gaps").
// =====================================================================

/// RED reproducer (level 1, broadest): morphotag pipeline must keep
/// `%mor` chunk count == `%gra` relation count.
///
/// Empirical fixture: the pre-patch state of utterance line 543 in
/// `aphasia-data/Spanish/NonProtocol/PerLA/Fluent/104-JCM1.cha`,
/// reproduced verbatim by re-running the live morphotag pipeline on
/// the file as it stood at commit `e17dc07`. The pipeline output is
/// byte-identical across runs (deterministic under the current
/// architecture).
///
/// What's wrong in the pipeline output (verbatim):
///
/// ```text
/// *PAR:	la meua dona↑@s (...) éramos xxx [=! looks at MUJ] .
/// %mor:	det|el-Fem-Def-Art-Sing det|meu-Fem-Def-Art noun|do-Fin-Imp-S~intj|na noun|éram .
/// %gra:	1|3|DET 2|3|DET 3|0|ROOT 4|3|PUNCT 5|3|PUNCT
/// ```
///
/// `%mor` has 6 alignable chunks (the MWT host
/// `noun|do-Fin-Imp-S~intj|na` produces 2 chunks):
///
/// 1. `det|el-Fem-Def-Art-Sing`
/// 2. `det|meu-Fem-Def-Art`
/// 3. `noun|do-Fin-Imp-S` (MWT chunk 1)
/// 4. `intj|na` (MWT chunk 2)
/// 5. `noun|éram`
/// 6. `.` (terminator)
///
/// `%gra` has 5 relations — the terminator's PUNCT relation was dropped
/// and a spurious second PUNCT was attached to `noun|éram` instead.
///
/// The corpus maintainer subsequently patched the data (commit
/// `9169e51`) to remove `↑` from the main tier and delete the `%gra`
/// line, hiding the symptom. That data edit violates the workspace
/// "Always Fix Root Causes, Never Symptoms" rule and is preserved here
/// as the source of the evidence; the upstream commit is already
/// published on `origin/main` for aphasia-data and cannot be cleanly
/// undone.
///
/// Today's behavior: `align_mor_to_gra` returns a `GraAlignment` with
/// E720 (`MorGraCountMismatch`). After the pipeline fix, regenerated
/// `%mor` and `%gra` will align 1:1 and this test passes.
#[test]
#[ignore = "historical bad-output fixture only; current executable contract lives in inject::tests::inject_morphosyntax_gra_count_mismatch_returns_err, which rejects misaligned %mor/%gra instead of silently emitting them"]
fn bug_dona_at_s_mwt_terminator_gra_alignment_must_hold() {
    // Verbatim pre-patch pipeline output. The headers are minimized
    // around the affected utterance so the test fixture is self-
    // contained; the utterance itself is byte-faithful to commit
    // `e17dc07` line 543 (and to a 2026-05-01 fresh morphotag run on
    // the same input).
    let chat = "@UTF8\n\
                @Begin\n\
                @Languages:\tcat, spa\n\
                @Participants:\tPAR JCM Participant\n\
                @ID:\tspa|PerLA|PAR|71;00.00|male|FluentAphasia||Participant|Secundary||\n\
                *PAR:\tla meua dona\u{2191}@s (...) éramos xxx [=! looks at MUJ] .\n\
                %mor:\tdet|el-Fem-Def-Art-Sing det|meu-Fem-Def-Art noun|do-Fin-Imp-S~intj|na noun|éram .\n\
                %gra:\t1|3|DET 2|3|DET 3|0|ROOT 4|3|PUNCT 5|3|PUNCT\n\
                @End\n";

    let parser = TreeSitterParser::new().expect("parser init");
    let chat_file = parser.parse_chat_file(chat).expect("parse");

    let mut checked_utterances = 0;
    for utt in chat_file.utterances() {
        let Some(mor) = utt.mor_tier() else { continue };
        let Some(gra) = utt.gra_tier() else { continue };
        checked_utterances += 1;

        let mor_chunk_count = mor.count_chunks();
        let gra_relation_count = gra.len();

        assert_eq!(
            mor_chunk_count, gra_relation_count,
            "BUG dona@s+MWT: morphotag pipeline produced %mor with \
             {mor_chunk_count} chunks but %gra with {gra_relation_count} \
             relations. The two counts must be equal — every %mor chunk \
             (including MWT children and terminator) needs a paired \
             %gra entry. Fixture from \
             aphasia-data/Spanish/NonProtocol/PerLA/Fluent/104-JCM1.cha:543 \
             at commit e17dc07 (pre-patch); reproduced verbatim by \
             a fresh morphotag run on 2026-05-01."
        );
    }

    assert!(
        checked_utterances >= 1,
        "fixture must contain at least one utterance with both %mor and \
         %gra tiers; the test setup is broken if this fires"
    );
}

/// RED reproducer (level 2, narrower): the morphotag pipeline drops
/// the terminator's `%gra` entry specifically.
///
/// Same fixture as the level-1 test, but a more precise assertion:
/// the LAST `%mor` chunk in the dona↑@s utterance is the terminator
/// `.`. The pipeline produces a `%gra` tier with one fewer entry than
/// `%mor` chunks; this test pins WHICH chunk is missing — the
/// terminator.
///
/// Captured output:
///
/// ```text
/// %mor:	det|el-... det|meu-... noun|do-Fin-Imp-S~intj|na noun|éram .
///         (chunks 1, 2, 3+4 MWT, 5, 6=terminator)
/// %gra:	1|3|DET 2|3|DET 3|0|ROOT 4|3|PUNCT 5|3|PUNCT
///         (entries for indices 1-5; chunk 6 has no entry)
/// ```
///
/// Hypothesis under test (level-2 narrowing): `build_gra_and_validate`
/// completes with consistent (mors_len + 1, gras_len + 1) under
/// `AppendTrailingPunct`, but the terminator chunk reaches the
/// `MorTier` via a different path that doesn't append a matching
/// `%gra` entry. The post-Stanza injection layer or the tier-
/// serialization step emits the terminator into `%mor` without a
/// paired `%gra` write.
///
/// Today: the assertion `gra entry exists at terminator's chunk index`
/// fails because the terminator's chunk index is 6 but `%gra` only has
/// 5 entries. After the bug is fixed, the pipeline emits a 6th
/// `%gra` entry (some `N|root|PUNCT` form) and this passes.
#[test]
#[ignore = "historical bad-output fixture only; current executable contract lives in inject::tests::inject_morphosyntax_gra_count_mismatch_returns_err, which rejects misaligned %mor/%gra instead of silently emitting them"]
fn bug_dona_at_s_terminator_chunk_must_have_gra_entry() {
    let chat = "@UTF8\n\
                @Begin\n\
                @Languages:\tcat, spa\n\
                @Participants:\tPAR JCM Participant\n\
                @ID:\tspa|PerLA|PAR|71;00.00|male|FluentAphasia||Participant|Secundary||\n\
                *PAR:\tla meua dona\u{2191}@s (...) éramos xxx [=! looks at MUJ] .\n\
                %mor:\tdet|el-Fem-Def-Art-Sing det|meu-Fem-Def-Art noun|do-Fin-Imp-S~intj|na noun|éram .\n\
                %gra:\t1|3|DET 2|3|DET 3|0|ROOT 4|3|PUNCT 5|3|PUNCT\n\
                @End\n";

    let parser = TreeSitterParser::new().expect("parser init");
    let chat_file = parser.parse_chat_file(chat).expect("parse");

    for utt in chat_file.utterances() {
        let Some(mor) = utt.mor_tier() else { continue };
        let Some(gra) = utt.gra_tier() else { continue };

        let chunk_count = mor.count_chunks();
        let terminator_chunk_index = chunk_count; // 1-based; last chunk is the terminator slot
        let max_gra_index = gra.relations().iter().map(|r| r.index).max().unwrap_or(0);

        assert!(
            max_gra_index >= terminator_chunk_index,
            "BUG dona@s+MWT (level 2): terminator chunk at index \
             {terminator_chunk_index} has no %gra entry. \
             Highest %gra index present: {max_gra_index}. The \
             terminator is in %mor (chunk count = {chunk_count}) but \
             the pipeline never emitted its paired %gra relation. \
             Hypothesis: the MorTier serializer appends the terminator \
             chunk after build_gra_and_validate runs, with no matching \
             gra append at the same seam."
        );
    }
}

/// Forward-regression placeholder kept for the original BUG-011 entry
/// point (the 2026-05-01 session-handoff bug ledger). Once the level-1
/// and level-2 tests above go GREEN, broader BUG-011 instances from
/// the other 5 affected files in the rerun should be added as further
/// fixtures.
#[test]
#[ignore = "BUG-011 broader gate — landed dona↑@s+MWT instance as \
            bug_dona_at_s_mwt_terminator_gra_alignment_must_hold + \
            bug_dona_at_s_terminator_chunk_must_have_gra_entry; the \
            other 5 affected files from the 2026-05-01 rerun still need \
            fixtures extracted before this becomes a hard gate."]
fn bug_011_mor_gra_chunk_counts_must_match() {
    let _ = ParseValidateOptions::default();
}

// =====================================================================
// Corpus-derived RED gold specs (2026-05-06)
//
// These are intentionally RED tests. Each fixture is a minimized
// one-utterance reproduction of current corpus output after BA3
// morphotagging, and each expected string is the manually adjudicated
// `%gra` surface that should have been emitted.
//
// Unlike the earlier contract tests in this file, these are gold-surface
// specs: they pin the exact `%gra` line we want, family by family, so the
// follow-up GREEN work has concrete targets instead of another
// whack-a-mole corpus rerun.
// =====================================================================

fn gra_content(chat: &str) -> &str {
    chat.lines()
        .find_map(|line| line.strip_prefix("%gra:\t"))
        .expect("fixture must contain exactly one %gra line")
}

fn assert_current_gra_matches_adjudicated_gold(
    chat: &str,
    expected_gold: &str,
    source_label: &str,
    family: &str,
) {
    let actual = gra_content(chat);
    assert_eq!(
        actual, expected_gold,
        "corpus-derived RED %gra spec failed for {source_label} ({family}).\n\
         current minimized fixture still contains:\n  {actual}\n\
         but the adjudicated gold %gra is:\n  {expected_gold}\n\
         This test is intentionally RED until the emitting/injection path is fixed."
    );
}

#[test]
fn current_e316_compound_prt_surface_must_use_chat_relation_label() {
    let sentence = UdSentence {
        words: vec![
            UdWord {
                id: UdId::Single(1),
                text: "wake".to_string(),
                lemma: "wake".to_string(),
                upos: UdPunctable::Value(UniversalPos::Verb),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "up".to_string(),
                lemma: "up".to_string(),
                upos: UdPunctable::Value(UniversalPos::Adp),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "compound:prt".to_string(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(3),
                text: ".".to_string(),
                lemma: ".".to_string(),
                upos: UdPunctable::Value(UniversalPos::Punct),
                xpos: None,
                feats: None,
                head: 1,
                deprel: "punct".to_string(),
                deps: None,
                misc: None,
            },
        ],
    };
    let ctx = MappingContext {
        lang: LanguageCode::new("eng"),
    };

    let (_mors, gras) = map_ud_sentence(&sentence, &ctx).expect("map ordinary UD sentence");
    let actual: Vec<String> = gras.iter().map(ToString::to_string).collect();
    assert_eq!(
        actual,
        vec![
            "1|0|ROOT".to_string(),
            "2|1|COMPOUND-PRT".to_string(),
            "3|1|PUNCT".to_string(),
        ],
        "E316 contract: ordinary UD->CHAT mapping must serialize `compound:prt` \
         using CHAT `%gra` label form `COMPOUND-PRT`, not the raw UD label."
    );
}

#[test]
#[ignore = "documentary corpus symptom fixture only; executable structural coverage belongs at the typed L2 splice seam in crates/talkbank-transform/src/morphosyntax/l2/splice.rs"]
fn current_e713_out_of_bounds_head_must_attach_cd_player_to_predicate() {
    let chat = "@UTF8\n\
                @Begin\n\
                @Languages:\thrv\n\
                @Participants:\tS PK Speaker\n\
                @ID:\thrv|test|SPK|||||Speaker|||\n\
                *SPK:\tonda kvarimo cd_player .\n\
                %mor:\tadv|onda verb|kvariti noun|cd .\n\
                %gra:\t1|2|ADVMOD 2|0|ROOT 3|5|COMPOUND 4|2|PUNCT\n\
                @End\n";

    assert_current_gra_matches_adjudicated_gold(
        chat,
        "1|2|ADVMOD 2|0|ROOT 3|2|OBJ 4|2|PUNCT",
        "Croatian cd-player sample",
        "E713",
    );
}

#[test]
#[ignore = "documentary corpus symptom fixture only; executable structural coverage belongs at the typed L2 splice seam in crates/talkbank-transform/src/morphosyntax/l2/splice.rs"]
fn current_e722_e724_cycle_must_promote_adult_to_single_root() {
    let chat = "@UTF8\n\
                @Begin\n\
                @Languages:\teng, spa\n\
                @Participants:\tS PK Speaker\n\
                @ID:\teng|test|SPK|||||Speaker|||\n\
                *SPK:\tay si too adult .\n\
                %mor:\tintj|ay noun|sí adv|too noun|adult .\n\
                %gra:\t1|2|ROOT 2|1|FIXED 3|4|ADVMOD 4|1|PARATAXIS 5|1|PUNCT\n\
                @End\n";

    assert_current_gra_matches_adjudicated_gold(
        chat,
        "1|4|DISCOURSE 2|1|FIXED 3|4|ADVMOD 4|0|ROOT 5|4|PUNCT",
        "Bangor Miami ay-si-too-adult sample",
        "E722 + E724",
    );
}

#[test]
#[ignore = "documentary corpus symptom fixture only; executable structural coverage belongs at the typed L2 splice seam in crates/talkbank-transform/src/morphosyntax/l2/splice.rs"]
fn current_e723_self_headed_relation_must_not_count_as_second_root() {
    let chat = "@UTF8\n\
                @Begin\n\
                @Languages:\teng, yue\n\
                @Participants:\tS PK Speaker\n\
                @ID:\teng|test|SPK|||||Speaker|||\n\
                *SPK:\tcolor ge go le .\n\
                %mor:\tnoun|color pron|嗰個-Int-S1 noun|咧 L2|xxx .\n\
                %gra:\t1|0|ROOT 2|1|DEP 3|3|NMOD 4|1|PUNCT 5|1|PUNCT\n\
                @End\n";

    assert_current_gra_matches_adjudicated_gold(
        chat,
        "1|0|ROOT 2|1|DEP 3|2|NMOD 4|1|PUNCT 5|1|PUNCT",
        "EACMC color-ge-go-le sample",
        "E723",
    );
}

#[test]
#[ignore = "documentary corpus symptom fixture only; executable structural coverage belongs at the typed L2 splice seam in crates/talkbank-transform/src/morphosyntax/l2/splice.rs"]
fn current_e724_genitive_cycle_must_attach_case_marker_under_year() {
    let chat = "@UTF8\n\
                @Begin\n\
                @Languages:\tdeu, eng\n\
                @Participants:\tS PK Speaker\n\
                @ID:\tdeu|test|SPK|||||Speaker|||\n\
                *SPK:\tnew years abend .\n\
                %mor:\tpropn|New noun|year~part|s noun|Abend-Masc-Nom adp|als pron|sie-Prs-Nom-S3 verb|feiern-Part-S aux|haben-Fin-Ind-Pres-S3 verb|feiern-Part-S aux|haben-Fin-Ind-Pres-S3 adp|in det|ein-Fem-Ind-Art-Sing +/.\n\
                %gra:\t1|4|AMOD 2|3|NMOD 3|2|CASE 4|9|OBJ 5|6|CASE 6|7|OBL 7|9|XCOMP 8|7|AUX 9|0|ROOT 10|9|AUX 11|12|CASE 12|9|OBL 13|9|PUNCT\n\
                @End\n";

    assert_current_gra_matches_adjudicated_gold(
        chat,
        "1|4|AMOD 2|4|NMOD 3|2|CASE 4|9|OBJ 5|6|CASE 6|7|OBL 7|9|XCOMP 8|7|AUX 9|0|ROOT 10|9|AUX 11|12|CASE 12|9|OBL 13|9|PUNCT",
        "CallHome German New-Year-s-Abend sample",
        "E724",
    );
}

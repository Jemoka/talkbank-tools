//! Parity tests: validate the generated `GrammarTraversal` against real CHAT data.
//!
//! These tests parse the 74-file reference corpus with tree-sitter and exercise
//! the generated extraction methods on every matching node. They verify:
//! 1. Extraction doesn't panic on any real-world CST
//! 2. Required children are Present in well-formed CHAT
//! 3. Speaker text, tier body, headers extract correctly

use std::collections::BTreeMap;

use talkbank_parser_tests::generated_traversal::*;

/// Minimal traversal implementation — uses all generated defaults.
struct TestTraversal;
impl GrammarTraversal for TestTraversal {}

/// Walk a tree-sitter tree, calling `callback` on every node.
fn walk_all<'tree, F>(node: tree_sitter::Node<'tree>, callback: &mut F)
where
    F: FnMut(tree_sitter::Node<'tree>),
{
    callback(node);
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            walk_all(cursor.node(), callback);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

fn parse_chat(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");
    parser.parse(source, None).expect("parse")
}

fn corpus_dir() -> Option<std::path::PathBuf> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .parent()?
        .join("resources/corpus/reference");
    dir.exists().then_some(dir)
}

// ---------------------------------------------------------------------------
// Test 1: Simple utterance — verify every field of main_tier
// ---------------------------------------------------------------------------

#[test]
fn test_main_tier_all_fields_present() {
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n*CHI:\thello world .\n@End\n";
    let tree = parse_chat(source);
    let mut t = TestTraversal;
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "main_tier" {
            let c = t.extract_main_tier(node);
            assert!(c.child_0.is_present(), "star: {:?}", c.child_0);
            assert!(c.speaker.is_present(), "speaker: {:?}", c.speaker);
            assert!(c.child_2.is_present(), "colon: {:?}", c.child_2);
            assert!(c.child_3.is_present(), "tab: {:?}", c.child_3);
            assert!(c.child_4.is_present(), "tier_body: {:?}", c.child_4);

            if let NodeSlot::Present(spk) = &c.speaker {
                assert_eq!(spk.text(source), "CHI");
            }
            found = true;
        }
    });
    assert!(found);
}

// ---------------------------------------------------------------------------
// Test 2: Header extraction
// ---------------------------------------------------------------------------

#[test]
fn test_participants_header() {
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child, MOT Mother\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut t = TestTraversal;
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "participants_header" {
            let c = t.extract_participants_header(node);
            // Should have all required children present
            assert!(c.child_0.is_present(), "prefix: {:?}", c.child_0);
            assert!(c.child_1.is_present(), "header_sep: {:?}", c.child_1);
            assert!(c.child_2.is_present(), "contents: {:?}", c.child_2);
            found = true;
        }
    });
    assert!(found);
}

#[test]
fn test_date_header() {
    let source =
        "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n@Date:\t01-JAN-2000\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut t = TestTraversal;
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "date_header" {
            let c = t.extract_date_header(node);
            assert!(c.child_0.is_present(), "date_prefix: {:?}", c.child_0);
            assert!(c.child_1.is_present(), "header_sep: {:?}", c.child_1);
            assert!(c.child_2.is_present(), "date_contents: {:?}", c.child_2);

            if let NodeSlot::Present(date) = &c.child_2 {
                assert_eq!(date.text(source), "01-JAN-2000");
            }
            found = true;
        }
    });
    assert!(found);
}

// ---------------------------------------------------------------------------
// Test 3: Document structure
// ---------------------------------------------------------------------------

#[test]
fn test_full_document_extraction() {
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut t = TestTraversal;

    // The root is source_file (CHOICE), its first child is full_document (SEQ)
    let root = tree.root_node();
    assert_eq!(root.kind(), "source_file");
    let full_doc = root.child(0).expect("should have full_document child");
    assert_eq!(full_doc.kind(), "full_document");

    let c = t.extract_full_document(full_doc);
    assert!(c.child_0.is_present(), "utf8_header: {:?}", c.child_0);
}

// ---------------------------------------------------------------------------
// Test 4: Corpus-wide extraction — every node, every method, no panics
// ---------------------------------------------------------------------------

#[test]
fn test_corpus_wide_extraction() {
    let Some(dir) = corpus_dir() else {
        eprintln!("Skipping: resources/corpus/reference not found");
        return;
    };

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut files_parsed = 0;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
    {
        let source = std::fs::read_to_string(entry.path()).expect("read file");
        let tree = parser.parse(&source, None).expect("parse");
        let mut t = TestTraversal;

        // Walk every node and call the matching extraction method.
        // The key validation: this doesn't panic on any real CST node.
        walk_all(tree.root_node(), &mut |node| {
            let kind = node.kind();
            *kind_counts.entry(kind.to_string()).or_default() += 1;

            // Call extraction for key SEQ rules to verify they work
            match kind {
                "full_document" => {
                    let _ = t.extract_full_document(node);
                }
                "main_tier" => {
                    let _ = t.extract_main_tier(node);
                }
                "utterance" => {
                    let _ = t.extract_utterance(node);
                }
                "tier_body" => {
                    let _ = t.extract_tier_body(node);
                }
                "utterance_end" => {
                    let _ = t.extract_utterance_end(node);
                }
                "participants_header" => {
                    let _ = t.extract_participants_header(node);
                }
                "languages_header" => {
                    let _ = t.extract_languages_header(node);
                }
                "id_header" => {
                    let _ = t.extract_id_header(node);
                }
                "date_header" => {
                    let _ = t.extract_date_header(node);
                }
                "media_header" => {
                    let _ = t.extract_media_header(node);
                }
                "comment_header" => {
                    let _ = t.extract_comment_header(node);
                }
                "mor_dependent_tier" => {
                    let _ = t.extract_mor_dependent_tier(node);
                }
                "gra_dependent_tier" => {
                    let _ = t.extract_gra_dependent_tier(node);
                }
                "pho_dependent_tier" => {
                    let _ = t.extract_pho_dependent_tier(node);
                }
                "com_dependent_tier" => {
                    let _ = t.extract_com_dependent_tier(node);
                }
                "word_with_optional_annotations" => {
                    let _ = t.extract_word_with_optional_annotations(node);
                }
                "nonword_with_optional_annotations" => {
                    let _ = t.extract_nonword_with_optional_annotations(node);
                }
                "mor_word" => {
                    let _ = t.extract_mor_word(node);
                }
                "mor_content" => {
                    let _ = t.extract_mor_content(node);
                }
                "gra_relation" => {
                    let _ = t.extract_gra_relation(node);
                }
                "replacement" => {
                    let _ = t.extract_replacement(node);
                }
                "group_with_annotations" => {
                    let _ = t.extract_group_with_annotations(node);
                }
                "begin_header" => {
                    let _ = t.extract_begin_header(node);
                }
                "end_header" => {
                    let _ = t.extract_end_header(node);
                }
                "utf8_header" => {
                    let _ = t.extract_utf8_header(node);
                }
                _ => {}
            }
        });

        files_parsed += 1;
    }

    assert!(
        files_parsed >= 74,
        "Should parse all 74 files, got {files_parsed}"
    );

    // Print corpus stats
    let total_nodes: usize = kind_counts.values().sum();
    let extracted_kinds = [
        "full_document",
        "main_tier",
        "utterance",
        "tier_body",
        "utterance_end",
        "participants_header",
        "languages_header",
        "id_header",
        "date_header",
        "media_header",
        "comment_header",
        "mor_dependent_tier",
        "gra_dependent_tier",
        "pho_dependent_tier",
        "com_dependent_tier",
        "word_with_optional_annotations",
        "nonword_with_optional_annotations",
        "mor_word",
        "mor_content",
        "gra_relation",
        "replacement",
        "group_with_annotations",
        "begin_header",
        "end_header",
        "utf8_header",
    ];
    let extracted_total: usize = extracted_kinds
        .iter()
        .map(|k| kind_counts.get(*k).copied().unwrap_or(0))
        .sum();

    eprintln!("=== Corpus-wide extraction stats ===");
    eprintln!("Files: {files_parsed}");
    eprintln!("Total CST nodes: {total_nodes}");
    eprintln!("Nodes with extraction methods called: {extracted_total}");
    eprintln!("Key rule counts:");
    for kind in &extracted_kinds {
        if let Some(&count) = kind_counts.get(*kind) {
            eprintln!("  {kind}: {count}");
        }
    }
}

// ---------------------------------------------------------------------------
// Test 5: Speaker extraction parity with hand-written parser
// ---------------------------------------------------------------------------

#[test]
fn test_speaker_parity_with_existing_parser() {
    let Some(dir) = corpus_dir() else {
        eprintln!("Skipping: resources/corpus/reference not found");
        return;
    };

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let chat_parser = talkbank_parser::TreeSitterParser::new().expect("grammar loads");

    let mut total_tiers = 0;
    let mut matching_speakers = 0;
    let mut files = 0;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
    {
        let source = std::fs::read_to_string(entry.path()).expect("read file");
        let tree = parser.parse(&source, None).expect("parse");
        let mut t = TestTraversal;

        // Parse with the Rust parser
        let Ok(existing) = chat_parser.parse_chat_file(&source) else {
            continue; // Skip files that fail to parse
        };

        // Collect speakers from generated traversal
        let mut gen_speakers = Vec::new();
        walk_all(tree.root_node(), &mut |node| {
            if node.kind() == "main_tier" {
                let c = t.extract_main_tier(node);
                if let NodeSlot::Present(spk) = &c.speaker {
                    gen_speakers.push(spk.text(&source).to_string());
                }
            }
        });

        // Compare with existing parser
        let existing_speakers: Vec<String> = existing
            .utterances()
            .map(|u| u.main.speaker.as_str().to_string())
            .collect();

        total_tiers += gen_speakers.len();
        for (g, e) in gen_speakers.iter().zip(existing_speakers.iter()) {
            if g == e {
                matching_speakers += 1;
            }
        }

        files += 1;
    }

    eprintln!("Speaker parity: {matching_speakers}/{total_tiers} match across {files} files");
    assert_eq!(
        matching_speakers, total_tiers,
        "All speakers should match between generated and existing parser"
    );
}

// ---------------------------------------------------------------------------
// Test 6: ALL 116 extraction methods on every node — zero panics
// ---------------------------------------------------------------------------

/// Dispatch to the appropriate extraction method based on node kind.
/// Returns true if an extraction was performed.
#[allow(non_snake_case, clippy::too_many_lines)]
fn try_extract(t: &mut TestTraversal, node: tree_sitter::Node) -> bool {
    match node.kind() {
        "_id_demographic_fields" => {
            let _ = t.extract__id_demographic_fields(node);
            true
        }
        "_id_identity_fields" => {
            let _ = t.extract__id_identity_fields(node);
            true
        }
        "_id_role_fields" => {
            let _ = t.extract__id_role_fields(node);
            true
        }
        "act_dependent_tier" => {
            let _ = t.extract_act_dependent_tier(node);
            true
        }
        "activities_header" => {
            let _ = t.extract_activities_header(node);
            true
        }
        "add_dependent_tier" => {
            let _ = t.extract_add_dependent_tier(node);
            true
        }
        "alt_dependent_tier" => {
            let _ = t.extract_alt_dependent_tier(node);
            true
        }
        "bck_header" => {
            let _ = t.extract_bck_header(node);
            true
        }
        "begin_header" => {
            let _ = t.extract_begin_header(node);
            true
        }
        "bg_header" => {
            let _ = t.extract_bg_header(node);
            true
        }
        "birth_of_header" => {
            let _ = t.extract_birth_of_header(node);
            true
        }
        "birthplace_of_header" => {
            let _ = t.extract_birthplace_of_header(node);
            true
        }
        "blank_header" => {
            let _ = t.extract_blank_header(node);
            true
        }
        "cod_dependent_tier" => {
            let _ = t.extract_cod_dependent_tier(node);
            true
        }
        "coh_dependent_tier" => {
            let _ = t.extract_coh_dependent_tier(node);
            true
        }
        "color_words_header" => {
            let _ = t.extract_color_words_header(node);
            true
        }
        "com_dependent_tier" => {
            let _ = t.extract_com_dependent_tier(node);
            true
        }
        "comment_header" => {
            let _ = t.extract_comment_header(node);
            true
        }
        "date_header" => {
            let _ = t.extract_date_header(node);
            true
        }
        "def_dependent_tier" => {
            let _ = t.extract_def_dependent_tier(node);
            true
        }
        "full_document" => {
            let _ = t.extract_full_document(node);
            true
        }
        "eg_header" => {
            let _ = t.extract_eg_header(node);
            true
        }
        "end_header" => {
            let _ = t.extract_end_header(node);
            true
        }
        "eng_dependent_tier" => {
            let _ = t.extract_eng_dependent_tier(node);
            true
        }
        "err_dependent_tier" => {
            let _ = t.extract_err_dependent_tier(node);
            true
        }
        "event" => {
            let _ = t.extract_event(node);
            true
        }
        "exp_dependent_tier" => {
            let _ = t.extract_exp_dependent_tier(node);
            true
        }
        "fac_dependent_tier" => {
            let _ = t.extract_fac_dependent_tier(node);
            true
        }
        "flo_dependent_tier" => {
            let _ = t.extract_flo_dependent_tier(node);
            true
        }
        "font_header" => {
            let _ = t.extract_font_header(node);
            true
        }
        "g_header" => {
            let _ = t.extract_g_header(node);
            true
        }
        "gls_dependent_tier" => {
            let _ = t.extract_gls_dependent_tier(node);
            true
        }
        "gpx_dependent_tier" => {
            let _ = t.extract_gpx_dependent_tier(node);
            true
        }
        "gra_contents" => {
            let _ = t.extract_gra_contents(node);
            true
        }
        "gra_dependent_tier" => {
            let _ = t.extract_gra_dependent_tier(node);
            true
        }
        "gra_relation" => {
            let _ = t.extract_gra_relation(node);
            true
        }
        "group_with_annotations" => {
            let _ = t.extract_group_with_annotations(node);
            true
        }
        "header_sep" => {
            let _ = t.extract_header_sep(node);
            true
        }
        "id_contents" => {
            let _ = t.extract_id_contents(node);
            true
        }
        "id_header" => {
            let _ = t.extract_id_header(node);
            true
        }
        "int_dependent_tier" => {
            let _ = t.extract_int_dependent_tier(node);
            true
        }
        "l1_of_header" => {
            let _ = t.extract_l1_of_header(node);
            true
        }
        "languages_contents" => {
            let _ = t.extract_languages_contents(node);
            true
        }
        "languages_header" => {
            let _ = t.extract_languages_header(node);
            true
        }
        "location_header" => {
            let _ = t.extract_location_header(node);
            true
        }
        "long_feature_begin" => {
            let _ = t.extract_long_feature_begin(node);
            true
        }
        "long_feature_end" => {
            let _ = t.extract_long_feature_end(node);
            true
        }
        "main_pho_group" => {
            let _ = t.extract_main_pho_group(node);
            true
        }
        "main_sin_group" => {
            let _ = t.extract_main_sin_group(node);
            true
        }
        "main_tier" => {
            let _ = t.extract_main_tier(node);
            true
        }
        "media_contents" => {
            let _ = t.extract_media_contents(node);
            true
        }
        "media_header" => {
            let _ = t.extract_media_header(node);
            true
        }
        "mod_dependent_tier" => {
            let _ = t.extract_mod_dependent_tier(node);
            true
        }
        "modsyl_dependent_tier" => {
            let _ = t.extract_modsyl_dependent_tier(node);
            true
        }
        "mor_content" => {
            let _ = t.extract_mor_content(node);
            true
        }
        "mor_contents" => {
            let _ = t.extract_mor_contents(node);
            true
        }
        "mor_dependent_tier" => {
            let _ = t.extract_mor_dependent_tier(node);
            true
        }
        "mor_feature" => {
            let _ = t.extract_mor_feature(node);
            true
        }
        "mor_post_clitic" => {
            let _ = t.extract_mor_post_clitic(node);
            true
        }
        "mor_word" => {
            let _ = t.extract_mor_word(node);
            true
        }
        "new_episode_header" => {
            let _ = t.extract_new_episode_header(node);
            true
        }
        "nonvocal_begin" => {
            let _ = t.extract_nonvocal_begin(node);
            true
        }
        "nonvocal_end" => {
            let _ = t.extract_nonvocal_end(node);
            true
        }
        "nonvocal_simple" => {
            let _ = t.extract_nonvocal_simple(node);
            true
        }
        "nonword_with_optional_annotations" => {
            let _ = t.extract_nonword_with_optional_annotations(node);
            true
        }
        "number_header" => {
            let _ = t.extract_number_header(node);
            true
        }
        "options_contents" => {
            let _ = t.extract_options_contents(node);
            true
        }
        "options_header" => {
            let _ = t.extract_options_header(node);
            true
        }
        "ort_dependent_tier" => {
            let _ = t.extract_ort_dependent_tier(node);
            true
        }
        "other_spoken_event" => {
            let _ = t.extract_other_spoken_event(node);
            true
        }
        "page_header" => {
            let _ = t.extract_page_header(node);
            true
        }
        "par_dependent_tier" => {
            let _ = t.extract_par_dependent_tier(node);
            true
        }
        "participant" => {
            let _ = t.extract_participant(node);
            true
        }
        "participants_contents" => {
            let _ = t.extract_participants_contents(node);
            true
        }
        "participants_header" => {
            let _ = t.extract_participants_header(node);
            true
        }
        "pho_dependent_tier" => {
            let _ = t.extract_pho_dependent_tier(node);
            true
        }
        "pho_grouped_content" => {
            let _ = t.extract_pho_grouped_content(node);
            true
        }
        "pho_groups" => {
            let _ = t.extract_pho_groups(node);
            true
        }
        "pho_words" => {
            let _ = t.extract_pho_words(node);
            true
        }
        "phoaln_dependent_tier" => {
            let _ = t.extract_phoaln_dependent_tier(node);
            true
        }
        "phosyl_dependent_tier" => {
            let _ = t.extract_phosyl_dependent_tier(node);
            true
        }
        "pid_header" => {
            let _ = t.extract_pid_header(node);
            true
        }
        "quotation" => {
            let _ = t.extract_quotation(node);
            true
        }
        "recording_quality_header" => {
            let _ = t.extract_recording_quality_header(node);
            true
        }
        "replacement" => {
            let _ = t.extract_replacement(node);
            true
        }
        "room_layout_header" => {
            let _ = t.extract_room_layout_header(node);
            true
        }
        "sin_dependent_tier" => {
            let _ = t.extract_sin_dependent_tier(node);
            true
        }
        "sin_grouped_content" => {
            let _ = t.extract_sin_grouped_content(node);
            true
        }
        "sin_groups" => {
            let _ = t.extract_sin_groups(node);
            true
        }
        "sit_dependent_tier" => {
            let _ = t.extract_sit_dependent_tier(node);
            true
        }
        "situation_header" => {
            let _ = t.extract_situation_header(node);
            true
        }
        "spa_dependent_tier" => {
            let _ = t.extract_spa_dependent_tier(node);
            true
        }
        "t_header" => {
            let _ = t.extract_t_header(node);
            true
        }
        "tape_location_header" => {
            let _ = t.extract_tape_location_header(node);
            true
        }
        "thumbnail_header" => {
            let _ = t.extract_thumbnail_header(node);
            true
        }
        "tier_body" => {
            let _ = t.extract_tier_body(node);
            true
        }
        "tier_sep" => {
            let _ = t.extract_tier_sep(node);
            true
        }
        "tim_dependent_tier" => {
            let _ = t.extract_tim_dependent_tier(node);
            true
        }
        "time_duration_header" => {
            let _ = t.extract_time_duration_header(node);
            true
        }
        "time_start_header" => {
            let _ = t.extract_time_start_header(node);
            true
        }
        "transcriber_header" => {
            let _ = t.extract_transcriber_header(node);
            true
        }
        "transcription_header" => {
            let _ = t.extract_transcription_header(node);
            true
        }
        "types_header" => {
            let _ = t.extract_types_header(node);
            true
        }
        "unsupported_dependent_tier" => {
            let _ = t.extract_unsupported_dependent_tier(node);
            true
        }
        "unsupported_header" => {
            let _ = t.extract_unsupported_header(node);
            true
        }
        "unsupported_line" => {
            let _ = t.extract_unsupported_line(node);
            true
        }
        "utf8_header" => {
            let _ = t.extract_utf8_header(node);
            true
        }
        "utterance" => {
            let _ = t.extract_utterance(node);
            true
        }
        "utterance_end" => {
            let _ = t.extract_utterance_end(node);
            true
        }
        "videos_header" => {
            let _ = t.extract_videos_header(node);
            true
        }
        "warning_header" => {
            let _ = t.extract_warning_header(node);
            true
        }
        "window_header" => {
            let _ = t.extract_window_header(node);
            true
        }
        "wor_dependent_tier" => {
            let _ = t.extract_wor_dependent_tier(node);
            true
        }
        "wor_tier_body" => {
            let _ = t.extract_wor_tier_body(node);
            true
        }
        "word_with_optional_annotations" => {
            let _ = t.extract_word_with_optional_annotations(node);
            true
        }
        "x_dependent_tier" => {
            let _ = t.extract_x_dependent_tier(node);
            true
        }
        _ => false,
    }
}

#[test]
fn test_all_116_extraction_methods_no_panics() {
    let dir = {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("resources/corpus/reference"));
        match p {
            Some(d) if d.exists() => d,
            _ => {
                eprintln!("Skipping: resources/corpus/reference not found");
                return;
            }
        }
    };

    let mut parser = tree_sitter::Parser::new();
    let lang: tree_sitter::Language = tree_sitter_talkbank::LANGUAGE.into();
    parser.set_language(&lang).expect("set language");

    let mut total_nodes = 0usize;
    let mut extracted_nodes = 0usize;
    let mut kind_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut files = 0;

    for entry in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "cha"))
    {
        let source = std::fs::read_to_string(entry.path()).expect("read file");
        let tree = parser.parse(&source, None).expect("parse");
        let mut t = TestTraversal;

        walk_all(tree.root_node(), &mut |node| {
            total_nodes += 1;
            if try_extract(&mut t, node) {
                extracted_nodes += 1;
                *kind_counts.entry(node.kind().to_string()).or_default() += 1;
            }
        });

        files += 1;
    }

    assert!(files >= 74, "Should parse all 74 files");

    let unique_kinds_extracted = kind_counts.len();
    eprintln!("=== ALL 116 METHODS: Corpus-wide results ===");
    eprintln!("Files: {files}");
    eprintln!("Total CST nodes: {total_nodes}");
    eprintln!("Nodes extracted by generated methods: {extracted_nodes}");
    eprintln!("Unique rule kinds with extraction: {unique_kinds_extracted}/116");
    eprintln!();
    eprintln!("Per-kind counts (top 30):");
    let mut sorted_kinds: Vec<_> = kind_counts.iter().collect();
    sorted_kinds.sort_by(|a, b| b.1.cmp(a.1));
    for (kind, count) in sorted_kinds.iter().take(30) {
        eprintln!("  {kind}: {count}");
    }

    // The key assertion: zero panics across all 37,000+ nodes
    assert!(
        extracted_nodes > 5000,
        "Should extract thousands of nodes, got {extracted_nodes}"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Semantic conversion — @Options header → OptionNameValue
// ---------------------------------------------------------------------------

#[test]
fn test_options_header_semantic_conversion() {
    // File with @Options: CA
    let source =
        "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n@Options:\tCA\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut t = TestTraversal;
    let mut found_option = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "options_header" {
            let children = t.extract_options_header(node);

            // The payload is options_contents (child_2)
            if let NodeSlot::Present(contents_node) = &children.child_2 {
                // Extract option_name from options_contents
                let contents_children = t.extract_options_contents(contents_node.0);

                if let NodeSlot::Present(option_node) = &contents_children.child_0 {
                    let option_text = option_node.text(source);

                    // Use the generated OptionNameValue enum
                    let value = OptionNameValue::from_text(option_text);
                    assert_eq!(value, OptionNameValue::CA);
                    assert!(value.is_known());
                    found_option = true;
                }
            }
        }
    });

    assert!(found_option, "Should have found and converted @Options: CA");
}

#[test]
fn test_options_header_unknown_value() {
    // File with @Options: SomeUnknownOption
    let source = "@UTF8\n@Begin\n@Participants:\tCHI Target_Child\n@Options:\tSomeUnknownOption\n*CHI:\thi .\n@End\n";
    let tree = parse_chat(source);
    let mut t = TestTraversal;
    let mut found = false;

    walk_all(tree.root_node(), &mut |node| {
        if node.kind() == "options_header" {
            let children = t.extract_options_header(node);
            if let NodeSlot::Present(contents_node) = &children.child_2 {
                let contents_children = t.extract_options_contents(contents_node.0);
                if let NodeSlot::Present(option_node) = &contents_children.child_0 {
                    let value = OptionNameValue::from_text(option_node.text(source));
                    assert!(
                        matches!(value, OptionNameValue::Other(ref s) if s == "SomeUnknownOption"),
                        "Unknown option should be Other, got {value:?}"
                    );
                    assert!(!value.is_known());
                    found = true;
                }
            }
        }
    });

    assert!(found, "Should have found @Options with unknown value");
}

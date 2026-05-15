//! Insta snapshot tests for lexer token streams.
//!
//! These tests produce YAML snapshots of token streams that can be
//! visually inspected to verify correctness, especially for rich tokens
//! with internal structure (IdFields, GraRelation, MorWord, TypesFields).

use insta::assert_yaml_snapshot;
use serde::Serialize;
use talkbank_parser_re2c::lexer::{
    COND_GRA_CONTENT, COND_ID_CONTENT, COND_INITIAL, COND_LANGUAGES_CONTENT, COND_MAIN_CONTENT,
    COND_MEDIA_CONTENT, COND_MOR_CONTENT, COND_PARTICIPANTS_CONTENT, COND_PHO_CONTENT,
    COND_SIN_CONTENT, COND_TIER_CONTENT, COND_TYPES_CONTENT, Lexer,
};
use talkbank_parser_re2c::token::Token;

/// A serializable view of a token for snapshots.
#[derive(Serialize)]
struct TokenView {
    kind: String,
    text: String,
}

fn lex_snapshot(input: &str, condition: usize) -> Vec<TokenView> {
    let mut s = input.to_string();
    s.push('\0');
    let s: &str = Box::leak(s.into_boxed_str());
    Lexer::new(s, condition)
        .map(|(tok, _span)| {
            let kind = format!("{:?}", std::mem::discriminant(&tok));
            // Use the variant name from Debug output
            let debug = format!("{tok:?}");
            let variant_name = debug.split('(').next().unwrap_or("?").to_string();
            TokenView {
                kind: variant_name,
                text: tok.text().to_string(),
            }
        })
        .collect()
}

// ═══════════════════════════════════════════════════════════════
// @ID — verify all 10 fields are captured in one token
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_id_full() {
    let tokens = lex_snapshot(
        "@ID:\teng|corpus|CHI|3;00.|female|typical||Child|||\n",
        COND_INITIAL,
    );
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_id_minimal() {
    let tokens = lex_snapshot("@ID:\teng|corpus|MOT|||||Mother|||\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_id_content_isolated() {
    // Lex just the @ID body (after :\t) in isolation
    let tokens = lex_snapshot(
        "eng|corpus|CHI|3;00.|female|typical||Child|||\n",
        COND_ID_CONTENT,
    );
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// @Types — verify 3 comma-separated fields
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_types() {
    let tokens = lex_snapshot("@Types:\tlongitudinal, naturalistic, TD\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_types_content_isolated() {
    let tokens = lex_snapshot("cross, toyplay, TD\n", COND_TYPES_CONTENT);
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// @Languages — comma-separated language codes
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_languages() {
    let tokens = lex_snapshot("@Languages:\teng, fra, zho\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// @Participants — structured speaker+name entries
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_participants() {
    let tokens = lex_snapshot(
        "@Participants:\tCHI Target_Child, MOT Mother, FAT Father\n",
        COND_INITIAL,
    );
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// @Media — filename, type, status
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_media_simple() {
    let tokens = lex_snapshot("@Media:\trecording, audio\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_media_with_status() {
    let tokens = lex_snapshot("@Media:\trecording, video, missing\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// %mor — rich MorWord tokens
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_mor_simple() {
    let tokens = lex_snapshot("pro|I v|want n|cookie-PL .\n", COND_MOR_CONTENT);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_mor_with_clitic() {
    let tokens = lex_snapshot("pron|it~aux|be-Fin-Ind-Pres-S3 .\n", COND_MOR_CONTENT);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_mor_subcategory() {
    let tokens = lex_snapshot("pro:sub|I det:art|the .\n", COND_MOR_CONTENT);
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// %gra — rich GraRelation tokens
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_gra() {
    let tokens = lex_snapshot(
        "1|4|NSUBJ 2|1|AUX 3|4|NSUBJ 4|0|ROOT 5|4|OBJ 6|4|PUNCT\n",
        COND_GRA_CONTENT,
    );
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// %pho — phonological words
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_pho() {
    let tokens = lex_snapshot("wɑ+kɪŋ hɛloʊ .\n", COND_PHO_CONTENT);
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// %sin — sign/gesture
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_sin() {
    let tokens = lex_snapshot("g:toy:dpoint hold give\n", COND_SIN_CONTENT);
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// Main tier — full token stream
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_main_tier_simple() {
    let tokens = lex_snapshot("*CHI:\thello world .\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_main_tier_compound() {
    let tokens = lex_snapshot("*CHI:\tI want ice+cream .\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_main_tier_annotations() {
    let tokens = lex_snapshot("*CHI:\tthe the [/] dog [= puppy] .\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_main_tier_rich() {
    let tokens = lex_snapshot(
        "*CHI:\t&-um (be)cause mama@f no:: +... [+ bch]\n",
        COND_INITIAL,
    );
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_main_tier_ca() {
    let tokens = lex_snapshot("*SPK:\trising to high ⇗\n", COND_INITIAL);
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// Full file snippet
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_full_file_snippet() {
    let tokens = lex_snapshot(
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child, MOT Mother\n@ID:\teng|corpus|CHI|3;00.||||Child|||\n*CHI:\thello .\n%mor:\tn|hello .\n@End\n",
        COND_INITIAL,
    );
    assert_yaml_snapshot!(tokens);
}

// ═══════════════════════════════════════════════════════════════
// Tier content with bullets
// ═══════════════════════════════════════════════════════════════

#[test]
fn snapshot_tier_with_bullet() {
    let tokens = lex_snapshot(
        "some text \u{0015}100_200\u{0015} more text\n",
        COND_TIER_CONTENT,
    );
    assert_yaml_snapshot!(tokens);
}

#[test]
fn snapshot_comment_with_continuation() {
    let tokens = lex_snapshot(
        "@Comment:\tThis is a long comment\n\tthat continues here\n",
        COND_INITIAL,
    );
    assert_yaml_snapshot!(tokens);
}

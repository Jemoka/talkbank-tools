//! Benchmarks for the tree-sitter CHAT parser.
//!
//! Run with: `cargo bench -p talkbank-parser`
//!
//! Note: `TreeSitterParser` is `!Sync`, so parser creation is included in the
//! benchmark. This is intentional — it measures the full parse cost as callers
//! experience it.

use talkbank_parser::TreeSitterParser;

fn main() {
    divan::main();
}

// Small file (13 lines): minimal headers + a few utterances.
#[divan::bench]
fn parse_basic_conversation(bencher: divan::Bencher) {
    let input = include_str!("../../../../resources/corpus/reference/core/basic-conversation.cha");
    bencher.bench_local(|| {
        let parser = TreeSitterParser::new().expect("grammar loads");
        parser.parse_chat_file(divan::black_box(input))
    });
}

// Medium file (18 lines): exercises %mor and %gra tier parsing.
#[divan::bench]
fn parse_mor_gra_tiers(bencher: divan::Bencher) {
    let input = include_str!("../../../../resources/corpus/reference/tiers/mor-gra.cha");
    bencher.bench_local(|| {
        let parser = TreeSitterParser::new().expect("grammar loads");
        parser.parse_chat_file(divan::black_box(input))
    });
}

// Large file (31 lines): many linker types in utterance content.
#[divan::bench]
fn parse_linkers(bencher: divan::Bencher) {
    let input = include_str!("../../../../resources/corpus/reference/content/linkers.cha");
    bencher.bench_local(|| {
        let parser = TreeSitterParser::new().expect("grammar loads");
        parser.parse_chat_file(divan::black_box(input))
    });
}

// Language-specific file (25 lines): Japanese with complex characters.
#[divan::bench]
fn parse_jpn_conversation(bencher: divan::Bencher) {
    let input = include_str!("../../../../resources/corpus/reference/languages/jpn-conversation.cha");
    bencher.bench_local(|| {
        let parser = TreeSitterParser::new().expect("grammar loads");
        parser.parse_chat_file(divan::black_box(input))
    });
}

// Annotation-heavy file (18 lines): error markers and replacements.
#[divan::bench]
fn parse_errors_and_replacements(bencher: divan::Bencher) {
    let input = include_str!("../../../../resources/corpus/reference/annotation/errors-and-replacements.cha");
    bencher.bench_local(|| {
        let parser = TreeSitterParser::new().expect("grammar loads");
        parser.parse_chat_file(divan::black_box(input))
    });
}

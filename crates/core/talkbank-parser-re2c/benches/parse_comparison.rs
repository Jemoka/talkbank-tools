//! Rigorous benchmark: TreeSitterParser vs Re2cParser (chumsky).
//!
//! Run: `cargo bench -p talkbank-parser-re2c --bench parse_comparison`
//!
//! All content pre-loaded via `include_str!`. Zero I/O during timed sections.
//! `divan::black_box()` prevents dead-code elimination.

use talkbank_parser::TreeSitterParser;

fn main() {
    divan::main();
}

// ═══════════════════════════════════════════════════════════════
// Group 1: Per-file parse — representative files, both parsers
// ═══════════════════════════════════════════════════════════════

mod file_parse {
    use super::*;

    const BASIC: &str = include_str!("../../../../resources/corpus/reference/core/basic-conversation.cha");
    const MOR_GRA: &str = include_str!("../../../../resources/corpus/reference/tiers/mor-gra.cha");
    const INTONATION: &str = include_str!("../../../../resources/corpus/reference/ca/intonation.cha");
    const CJK: &str = include_str!("../../../../resources/corpus/reference/languages/zho-conversation.cha");
    const COMPOUNDS: &str = include_str!("../../../../resources/corpus/reference/word-features/impdenis.cha");

    // ── TreeSitter with constructor cost ────────────────────────

    #[divan::bench(name = "basic__ts_ctor")]
    fn basic_ts_ctor(b: divan::Bencher) {
        b.bench_local(|| {
            let p = TreeSitterParser::new().expect("grammar");
            p.parse_chat_file(divan::black_box(BASIC))
        });
    }
    #[divan::bench(name = "mor_gra__ts_ctor")]
    fn mor_gra_ts_ctor(b: divan::Bencher) {
        b.bench_local(|| {
            let p = TreeSitterParser::new().expect("grammar");
            p.parse_chat_file(divan::black_box(MOR_GRA))
        });
    }
    #[divan::bench(name = "ca__ts_ctor")]
    fn ca_ts_ctor(b: divan::Bencher) {
        b.bench_local(|| {
            let p = TreeSitterParser::new().expect("grammar");
            p.parse_chat_file(divan::black_box(INTONATION))
        });
    }
    #[divan::bench(name = "cjk__ts_ctor")]
    fn cjk_ts_ctor(b: divan::Bencher) {
        b.bench_local(|| {
            let p = TreeSitterParser::new().expect("grammar");
            p.parse_chat_file(divan::black_box(CJK))
        });
    }
    #[divan::bench(name = "complex__ts_ctor")]
    fn complex_ts_ctor(b: divan::Bencher) {
        b.bench_local(|| {
            let p = TreeSitterParser::new().expect("grammar");
            p.parse_chat_file(divan::black_box(COMPOUNDS))
        });
    }

    // ── TreeSitter reuse (parse only, no constructor) ───────────

    #[divan::bench(name = "basic__ts_reuse")]
    fn basic_ts_reuse(b: divan::Bencher) {
        let p = TreeSitterParser::new().expect("grammar");
        b.bench_local(|| p.parse_chat_file(divan::black_box(BASIC)));
    }
    #[divan::bench(name = "mor_gra__ts_reuse")]
    fn mor_gra_ts_reuse(b: divan::Bencher) {
        let p = TreeSitterParser::new().expect("grammar");
        b.bench_local(|| p.parse_chat_file(divan::black_box(MOR_GRA)));
    }
    #[divan::bench(name = "ca__ts_reuse")]
    fn ca_ts_reuse(b: divan::Bencher) {
        let p = TreeSitterParser::new().expect("grammar");
        b.bench_local(|| p.parse_chat_file(divan::black_box(INTONATION)));
    }
    #[divan::bench(name = "cjk__ts_reuse")]
    fn cjk_ts_reuse(b: divan::Bencher) {
        let p = TreeSitterParser::new().expect("grammar");
        b.bench_local(|| p.parse_chat_file(divan::black_box(CJK)));
    }
    #[divan::bench(name = "complex__ts_reuse")]
    fn complex_ts_reuse(b: divan::Bencher) {
        let p = TreeSitterParser::new().expect("grammar");
        b.bench_local(|| p.parse_chat_file(divan::black_box(COMPOUNDS)));
    }

    // ── Re2c (chumsky) — zero-cost construction ─────────────────

    #[divan::bench(name = "basic__re2c")]
    fn basic_re2c(b: divan::Bencher) {
        b.bench_local(|| talkbank_parser_re2c::parser::parse_chat_file(divan::black_box(BASIC)));
    }
    #[divan::bench(name = "mor_gra__re2c")]
    fn mor_gra_re2c(b: divan::Bencher) {
        b.bench_local(|| talkbank_parser_re2c::parser::parse_chat_file(divan::black_box(MOR_GRA)));
    }
    #[divan::bench(name = "ca__re2c")]
    fn ca_re2c(b: divan::Bencher) {
        b.bench_local(|| {
            talkbank_parser_re2c::parser::parse_chat_file(divan::black_box(INTONATION))
        });
    }
    #[divan::bench(name = "cjk__re2c")]
    fn cjk_re2c(b: divan::Bencher) {
        b.bench_local(|| talkbank_parser_re2c::parser::parse_chat_file(divan::black_box(CJK)));
    }
    #[divan::bench(name = "complex__re2c")]
    fn complex_re2c(b: divan::Bencher) {
        b.bench_local(|| {
            talkbank_parser_re2c::parser::parse_chat_file(divan::black_box(COMPOUNDS))
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// Group 2: Batch parse — all reference corpus files
// ═══════════════════════════════════════════════════════════════

mod batch_parse {
    use super::*;

    fn corpus() -> &'static [&'static str] {
        &[
            include_str!("../../../../resources/corpus/reference/core/basic-conversation.cha"),
            include_str!("../../../../resources/corpus/reference/core/headers-comments.cha"),
            include_str!("../../../../resources/corpus/reference/core/multiline-continuation.cha"),
            include_str!("../../../../resources/corpus/reference/tiers/mor-gra.cha"),
            include_str!("../../../../resources/corpus/reference/tiers/pho.cha"),
            include_str!("../../../../resources/corpus/reference/tiers/sin.cha"),
            include_str!("../../../../resources/corpus/reference/tiers/wor.cha"),
            include_str!("../../../../resources/corpus/reference/tiers/user-defined.cha"),
            include_str!("../../../../resources/corpus/reference/tiers/coding.cha"),
            include_str!("../../../../resources/corpus/reference/tiers/descriptive.cha"),
            include_str!("../../../../resources/corpus/reference/annotation/errors-and-replacements.cha"),
            include_str!("../../../../resources/corpus/reference/annotation/groups-regular.cha"),
            include_str!("../../../../resources/corpus/reference/annotation/groups-phonological.cha"),
            include_str!("../../../../resources/corpus/reference/annotation/retrace.cha"),
            include_str!("../../../../resources/corpus/reference/annotation/overlap-markers.cha"),
            include_str!("../../../../resources/corpus/reference/annotation/scope-markers.cha"),
            include_str!("../../../../resources/corpus/reference/ca/intonation.cha"),
            include_str!("../../../../resources/corpus/reference/ca/overlaps.cha"),
            include_str!("../../../../resources/corpus/reference/ca/stacked-markers.cha"),
            include_str!("../../../../resources/corpus/reference/content/linkers.cha"),
            include_str!("../../../../resources/corpus/reference/content/pauses-and-events.cha"),
            include_str!("../../../../resources/corpus/reference/content/media-bullets.cha"),
            include_str!("../../../../resources/corpus/reference/content/quotations.cha"),
            include_str!("../../../../resources/corpus/reference/content/separators.cha"),
            include_str!("../../../../resources/corpus/reference/content/shortenings-in-words.cha"),
            include_str!("../../../../resources/corpus/reference/content/words-basic.cha"),
            include_str!("../../../../resources/corpus/reference/languages/zho-conversation.cha"),
            include_str!("../../../../resources/corpus/reference/languages/jpn-conversation.cha"),
            include_str!("../../../../resources/corpus/reference/languages/fra-conversation.cha"),
            include_str!("../../../../resources/corpus/reference/languages/rus-conversation.cha"),
            include_str!("../../../../resources/corpus/reference/languages/spa-conversation.cha"),
            include_str!("../../../../resources/corpus/reference/languages/eng-conversation.cha"),
            include_str!("../../../../resources/corpus/reference/word-features/impdenis.cha"),
            include_str!("../../../../resources/corpus/reference/word-features/1082.cha"),
            include_str!("../../../../resources/corpus/reference/word-features/000829.cha"),
        ]
    }

    #[divan::bench(name = "batch_35_ts")]
    fn batch_ts(b: divan::Bencher) {
        let files = corpus();
        b.bench_local(|| {
            let p = TreeSitterParser::new().expect("grammar");
            for input in files {
                let _ = p.parse_chat_file(divan::black_box(input));
            }
        });
    }

    #[divan::bench(name = "batch_35_re2c")]
    fn batch_re2c(b: divan::Bencher) {
        let files = corpus();
        b.bench_local(|| {
            for input in files {
                let _ = talkbank_parser_re2c::parser::parse_chat_file(divan::black_box(input));
            }
        });
    }
}

// ═══════════════════════════════════════════════════════════════
// Group 3: Tier-level parse — isolated content parsing
// ═══════════════════════════════════════════════════════════════

mod tier_parse {
    use super::*;

    const MAIN: &str = "*CHI:\thello world , do you want ice+cream ?\n";
    const MOR: &str = "pro|I v|want det|a n|cookie-PL .\n";
    const GRA: &str = "1|2|SUBJ 2|0|ROOT 3|2|DET 4|2|OBJ 5|2|PUNCT\n";

    // TreeSitter fragment parsers use parse_main_tier(input) -> ParseResult
    #[divan::bench(name = "main_tier__ts")]
    fn main_tier_ts(b: divan::Bencher) {
        let p = TreeSitterParser::new().expect("grammar");
        b.bench_local(|| p.parse_main_tier(divan::black_box(MAIN)));
    }

    #[divan::bench(name = "main_tier__re2c")]
    fn main_tier_re2c(b: divan::Bencher) {
        b.bench_local(|| talkbank_parser_re2c::parser::parse_main_tier(divan::black_box(MAIN)));
    }

    #[divan::bench(name = "mor_tier__re2c")]
    fn mor_tier_re2c(b: divan::Bencher) {
        b.bench_local(|| talkbank_parser_re2c::parser::parse_mor_tier(divan::black_box(MOR)));
    }

    #[divan::bench(name = "gra_tier__re2c")]
    fn gra_tier_re2c(b: divan::Bencher) {
        b.bench_local(|| talkbank_parser_re2c::parser::parse_gra_tier(divan::black_box(GRA)));
    }
}

// ═══════════════════════════════════════════════════════════════
// Group 4: Lex-only — re2c DFA baseline
// ═══════════════════════════════════════════════════════════════

mod lex_only {
    use talkbank_parser_re2c::lexer;

    const MAIN: &str = "*CHI:\thello world , do you want ice+cream ?\n";
    const MOR: &str = "pro|I v|want det|a n|cookie-PL .\n";
    const FILE: &str = include_str!("../../../../resources/corpus/reference/tiers/mor-gra.cha");

    #[divan::bench(name = "lex_main_tier")]
    fn lex_main(b: divan::Bencher) {
        b.bench_local(|| {
            talkbank_parser_re2c::lex_line(divan::black_box(MAIN), lexer::COND_INITIAL)
        });
    }

    #[divan::bench(name = "lex_mor_tier")]
    fn lex_mor(b: divan::Bencher) {
        b.bench_local(|| {
            talkbank_parser_re2c::lex_line(divan::black_box(MOR), lexer::COND_MOR_CONTENT)
        });
    }

    #[divan::bench(name = "lex_full_file")]
    fn lex_file(b: divan::Bencher) {
        b.bench_local(|| {
            talkbank_parser_re2c::lex_line(divan::black_box(FILE), lexer::COND_INITIAL)
        });
    }
}

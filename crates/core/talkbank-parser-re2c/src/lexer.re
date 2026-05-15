// -*- text -*-
// CHAT lexer — mechanically translated from grammar.js
// Source of truth: ~/talkbank/talkbank-tools/grammar/grammar.js

/*!conditions:re2c*/

use tracing::{debug, instrument};
use crate::token::Token;

/// A span of source code.
pub type LexerSpan = std::ops::Range<usize>;

/// State for our lexer.
pub struct Lexer<'a> {
    /// String with NUL sentinel.
    pub nul_terminated: &'a str,

    /// Used by generated lexer for conditions.
    /// Set via Lexer::new(input, condition) for multiple entry points.
    /// E.g., start in MOR_CONTENT to parse a %mor tier in isolation.
    pub condition: usize,

    /// Used by generated lexer while lexing.
    pub cursor: usize,

    /// Used by generated lexer while lexing.
    pub marker: usize,

    /*!stags:re2c format = "/** An intermediate tag for lexer. */\npub @@: usize,\n"; */
    /*!svars:re2c format = "/** A tag for semantic actions. */\npub @@: usize,\n"; */
}

impl<'a> Lexer<'a> {
    /// Create a lexer starting in the given condition.
    /// Use condition 0 for INITIAL (full file parsing).
    /// Use named condition constants (e.g., YYC_MOR_CONTENT) for fragment parsing.
    pub fn new(nul_terminated: &'a str, condition: usize) -> Self {
        let cursor = 0;
        let marker = 0;
        Self {
            nul_terminated,
            cursor,
            marker,
            condition,
            /*!stags:re2c format = "@@: NONE,"; */
            /*!svars:re2c format = "@@: NONE,"; */
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = (Token<'a>, LexerSpan);

    /// Return the next token with its span.
    #[instrument(level = "debug", skip(self))]
    fn next(&mut self) -> Option<Self::Item> {
        let yyinput = self.nul_terminated;
        let buffer = yyinput.as_bytes();
        let start = self.cursor;

        debug!(cursor = start, "Lexing");

        macro_rules! emit {
            ($variant:ident) => {
                {
                    let end = self.cursor;
                    return Some((Token::$variant(&yyinput[start..end]), start..end));
                }
            };
        }

        /// Emit token whose content is the tag-extracted slice t1..cursor.
        /// The LexerSpan still covers the full match (start..end).
        macro_rules! emit_t1 {
            ($variant:ident) => {{
                let end = self.cursor;
                return Some((Token::$variant(&yyinput[self.t1..end]), start..end));
            }};
        }

        /// Emit token whose content is the tag-extracted slice t1..t2.
        /// Whitespace is stripped from both ends (CHAT convention: field
        /// content never has semantically meaningful leading/trailing spaces).
        /// The LexerSpan still covers the full match (start..end).
        macro_rules! emit_t1t2 {
            ($variant:ident) => {{
                let end = self.cursor;
                return Some((Token::$variant(yyinput[self.t1..self.t2].trim()), start..end));
            }};
        }

    /*!re2c
        re2c:tags = 1;
        re2c:yyfill:enable = 0;
        re2c:YYCTYPE = "u8";
        re2c:YYPEEK = "*buffer.get_unchecked(self.cursor)";
        re2c:YYSKIP = "self.cursor += 1;";
        re2c:YYBACKUP = "self.marker = self.cursor;";
        re2c:YYRESTORE = "self.cursor = self.marker;";
        re2c:YYSTAGP = "self.@@{tag} = self.cursor;";
        re2c:YYSTAGN = "self.@@{tag} = NONE;";
        re2c:YYCOPYSTAG = "self.@@{lhs} = self.@@{rhs};";
        re2c:YYSHIFTSTAG  = "self.@@{tag} = (self.@@{tag} as isize + @@{shift}) as usize;";
        re2c:YYGETCONDITION = "self.condition";
        re2c:YYSETCONDITION = "self.condition = @@;";

        // ═══════════════════════════════════════════════════════
        //
        // NOTE: re2c reports "2nd degree of nondeterminism" for tags t2-t4
        // in the Word rules (~lines 726-749) and TYPES_CONTENT (~line 416).
        // These use tagged sub-match extraction with optional groups and
        // alternations (optional form marker `@f`, optional lang suffix
        // `@s:eng`, comma-separated type fields). re2c's DFA cannot
        // statically resolve which path sets the tag. At runtime, re2c
        // correctly resolves the tags via leftmost-longest matching.
        // Validated on all 99,907 CHAT files with zero errors.
        // Restructuring to avoid this would require multi-pass lexing or
        // intermediate states, adding complexity with no correctness benefit.
        // Suppressed via -Wno-nondeterministic-tags in build.rs.
        //
        // ═══════════════════════════════════════════════════════
        // NAMED DEFINITIONS — Word character classes
        //
        // Translated from grammar.js WORD_SEGMENT_FORBIDDEN_FIRST/REST.
        // ws_first: first char of a word_segment (excludes 0 and all structural chars)
        // ws_rest:  rest chars (allows 0, excludes backslash)
        // These define valid word TEXT characters. Other word-internal
        // constructs (shortenings, lengthening, CA markers, etc.) are
        // separate alternatives in the word body pattern.
        // ═══════════════════════════════════════════════════════

        // Event segment character: same as ws_first but allows colon (:)
        // for compound events like &=clears:throat.
        // Translated from grammar.js EVENT_SEGMENT_FORBIDDEN.
        // Event description chars: characters allowed after &= in event markers.
        // TODO: pending CHAT-spec adjudication on whether & should be allowed (for &=&=squeals).
        // Currently & is excluded, matching the original re2c lexer. TreeSitter's
        // grammar allows & in event_segment but this may be an oversight.
        // grammar.js's `EVENT_SEGMENT_FORBIDDEN` permits `^`, `{`, `}`, and `$`
        // inside event_segment; earlier this rule forbade all four, splitting
        // otherwise-whole events such as `&=reads:boardLas^_Palmas`
        // (shaddend.cha, found by the 2026-04-30 parity oracle) into
        // separate event + word tokens. Aligned to grammar.js — only the
        // chars that grammar.js itself forbids are excluded here.
        ev_char = [^ \t\r\n\x00,;!?.()[\]~+@&*%"<>\\\u0015\u0001\u0002\u0003\u0004\u0007\u0008\u2308\u2309\u230A\u230B\u3014\u3015\u2039\u203A\u02C8\u02CC\u201C\u201D\u201E\u2021\u2248\u224B\u221E\u2261\u21D7\u2197\u2192\u2198\u21D8\u2051\u2191\u2193\u21BB\u2260\u2219\u223E\u2906\u2907\u1F29\u2047\u00A7\u204E\u00B0\u21AB\u2206\u2207\u222C\u222E\u2581\u2594\u25C9\u263A\u264B\u03AB];

        // Word segment first character: excludes 0 and all structural/CA chars
        ws_first = [^ \t\r\n\x00,;:!?.()[\]{}~^+$@&*%"<>\\\u0015\u0001\u0002\u0003\u0004\u0007\u0008\u2308\u2309\u230A\u230B\u3014\u3015\u2039\u203A\u02C8\u02CC\u201C\u201D\u201E\u2021\u2248\u224B\u221E\u2261\u21D7\u2197\u2192\u2198\u21D8\u2051\u2191\u2193\u21BB\u2260\u2219\u223E\u2906\u2907\u1F29\u2047\u00A7\u204E\u00B0\u21AB\u2206\u2207\u222C\u222E\u2581\u2594\u25C9\u263A\u264B\u03AB0];

        // Word segment rest characters: allows 0, excludes backslash
        ws_rest = [^ \t\r\n\x00,;:!?.()[\]{}~^+$@&*%"<>\\\u0015\u0001\u0002\u0003\u0004\u0007\u0008\u2308\u2309\u230A\u230B\u3014\u3015\u2039\u203A\u02C8\u02CC\u201C\u201D\u201E\u2021\u2248\u224B\u221E\u2261\u21D7\u2197\u2192\u2198\u21D8\u2051\u2191\u2193\u21BB\u2260\u2219\u223E\u2906\u2907\u1F29\u2047\u00A7\u204E\u00B0\u21AB\u2206\u2207\u222C\u222E\u2581\u2594\u25C9\u263A\u264B\u03AB];

        // Word text segment: first char + zero or more rest chars
        w_text = ws_first ws_rest*;

        // Shortening: (text) where text is a word_segment inside parens
        w_short = "(" [^\x00 \t\r\n)]+ ")";

        // Stress markers
        w_stress = [\u02C8\u02CC];

        // Overlap points with optional digit
        w_overlap = [\u2308\u2309\u230A\u230B] [1-9]?;

        // CA element symbols (from symbol registry)
        w_ca_elem = [\u2260\u223E\u2051\u2907\u2219\u1F29\u2193\u21BB\u2191\u2906];

        // CA delimiter symbols (from symbol registry)
        w_ca_delim = [\u2047\u00A7\u204E\u00B0\u21AB\u2206\u2207\u222C\u222E\u2581\u2594\u25C9\u263A\u264B\u03AB];

        // Underline markers
        w_ul_begin = "\u0002\u0001";
        w_ul_end = "\u0002\u0002";

        // A word body "atom": any single construct that can appear in word body
        // (everything that ISN'T a compound marker, which needs special handling)
        w_atom = w_text | w_short | ":"+ | w_stress | w_overlap | "^" | "~"
               | w_ca_elem | w_ca_delim | w_ul_begin | w_ul_end;

        // Word body: text-initial or marker-initial, then continuation
        // Text-initial: starts with text, shortening, or stress
        // Marker-initial: starts with marker(s) then text (grammar.js: marker_start pattern)
        w_text_start = (w_text | w_short | w_stress);
        w_marker_start = (w_overlap | w_ca_elem | w_ca_delim | w_ul_begin)+
                         (w_text | w_short | w_stress);
        w_body_start = w_text_start | w_marker_start;

        // Continuation: atom or compound (+atom). + only consumed when followed by atom.
        w_cont = w_atom | ("+" w_atom);

        // Complete word body
        w_body = w_body_start w_cont*;

        // Word prefix
        w_prefix = "&-" | "&~" | "&+";

        // Form marker suffix: @code with optional :extended
        w_form = "@" ("u" | "b" | "c" | "d" | "f" | "fp" | "g" | "i" | "k" | "l"
               | "ls" | "n" | "o" | "p" | "q" | "sas" | "si" | "sl" | "t" | "wp"
               | "x" | "z") (":" [a-zA-Z0-9_]+)?;

        // Language suffix: @s or @s:codes
        w_lang = "@s" (":" [a-z][a-z][a-z]? ([+&] [a-z][a-z][a-z]?)*)?;

        // POS tag suffix: $tag
        w_pos = "$" [a-zA-Z:]+;

        // ═══════════════════════════════════════════════════════
        // GLOBAL RULES (all conditions)
        // ═══════════════════════════════════════════════════════

        // EOF sentinel — NUL byte terminates input.
        <*> [\x00] {
            return None;
        }

        // grammar.js: continuation = /[\r\n]+\t/
        // Continuation line: newline(s) followed by tab.
        // CRITICAL: do NOT reset condition — continuation content stays
        // in the same mode as the previous line (e.g., MAIN_CONTENT,
        // MOR_CONTENT, HEADER_CONTENT, etc.)
        <*> [\r\n]+ [\t] {
            emit!(Continuation);
        }

        // grammar.js: newline = /[\r\n]+/
        // End of line resets to INITIAL.
        <*> [\r\n]+ => INITIAL {
            emit!(Newline);
        }

        // ═══════════════════════════════════════════════════════
        // INITIAL — Top-level line classification
        // grammar.js: line → choice(header, utterance)
        //             header starts with @
        //             main_tier starts with *
        //             dependent_tier starts with %
        // ═══════════════════════════════════════════════════════

        <INITIAL> [\uFEFF] {
            emit!(BOM);
        }

        // ── Headers with no content (just prefix + newline) ──
        <INITIAL> "@UTF8" { emit!(HeaderUtf8); }
        <INITIAL> "@Begin" { emit!(HeaderBegin); }
        <INITIAL> "@End" { emit!(HeaderEnd); }
        <INITIAL> "@Blank" { emit!(HeaderBlank); }
        <INITIAL> "@New Episode" { emit!(HeaderNewEpisode); }

        // ── Headers with structured content (specific conditions) ──
        <INITIAL> "@ID:\t" => ID_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Types:\t" => TYPES_CONTENT { emit!(HeaderPrefix); }

        // ── Speaker-embedded headers ──
        // @Birth of SPK:\t, @Birthplace of SPK:\t, @L1 of SPK:\t
        // Tag extracts the speaker code directly.
        <INITIAL> "@Birth of" [ \t]+ @t1 [A-Za-z0-9_'+\-]+ @t2 ":\t" => HEADER_CONTENT {
            emit_t1t2!(HeaderBirthOf);
        }
        <INITIAL> "@Birthplace of" [ \t]+ @t1 [A-Za-z0-9_'+\-]+ @t2 ":\t" => HEADER_CONTENT {
            emit_t1t2!(HeaderBirthplaceOf);
        }
        <INITIAL> "@L1 of" [ \t]+ @t1 [A-Za-z0-9_'+\-]+ @t2 ":\t" => HEADER_CONTENT {
            emit_t1t2!(HeaderL1Of);
        }

        // ── Headers with bullet-aware content (text_with_bullets) ──
        // @Comment uses text_with_bullets_and_pics per grammar.js
        // @Comment uses text_with_bullets_and_pics (same as %com)
        <INITIAL> "@Comment:\t" => COM_CONTENT { emit!(HeaderPrefix); }
        // @Bg, @Eg, @G optional content variants also get bullet support
        // (handled in optional-content section below)

        // ── Headers with structured content (specific conditions) ──
        // These have internal structure the lexer should extract.
        <INITIAL> "@Languages:\t" => LANGUAGES_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Participants:\t" => PARTICIPANTS_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Date:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Media:\t" => MEDIA_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Options:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Recording Quality:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Transcription:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Number:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }

        // ── Headers with free text content (truly opaque) ──
        <INITIAL> "@Location:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Situation:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Activities:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Room Layout:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Tape Location:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Time Duration:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Time Start:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Transcriber:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Warning:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Page:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Videos:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Thumbnail:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@T:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Bck:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@PID:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Font:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Window:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Color words:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }

        // ── Optional-content headers (can appear with or without :\t) ──
        <INITIAL> "@Bg:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Bg" { emit!(HeaderPrefix); }
        <INITIAL> "@Eg:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@Eg" { emit!(HeaderPrefix); }
        <INITIAL> "@G:\t" => HEADER_CONTENT { emit!(HeaderPrefix); }
        <INITIAL> "@G" { emit!(HeaderPrefix); }

        // ── Catch-all for unknown @headers ──
        <INITIAL> "@" [^\x00:\r\n]* => HEADER_AFTER_NAME {
            emit!(HeaderPrefix);
        }

        // grammar.js: star = '*'
        <INITIAL> "*" => SPEAKER {
            emit!(Star);
        }

        // grammar.js: dependent tier starts with %label:\t
        // Dispatch to tier-specific conditions based on label.
        // Rich token: the entire %label:\t is one token, and the lexer
        // enters the correct content condition directly.
        // This avoids the parser having to inspect the label and re-lex.
        // ── Tier dispatch: each tier class gets its own lexer condition ──

        // Structured tiers (parsed into typed AST)
        <INITIAL> "%mor:\t" => MOR_CONTENT { emit!(TierPrefix); }
        <INITIAL> "%trn:\t" => MOR_CONTENT { emit!(TierPrefix); }
        <INITIAL> "%gra:\t" => GRA_CONTENT { emit!(TierPrefix); }
        <INITIAL> "%pho:\t" => PHO_CONTENT { emit!(TierPrefix); }
        <INITIAL> "%mod:\t" => PHO_CONTENT { emit!(TierPrefix); }
        <INITIAL> "%sin:\t" => SIN_CONTENT { emit!(TierPrefix); }

        // %wor uses MAIN_CONTENT rules — same word tokenization as main tier
        <INITIAL> "%wor:\t" => MAIN_CONTENT { emit!(TierPrefix); }

        // %com: text_with_bullets_and_pics (adds inline_pic)
        <INITIAL> "%com:\t" => COM_CONTENT { emit!(TierPrefix); }

        // User-defined tiers (%x*): text_with_bullets
        // Must come before the catch-all since re2c uses first-match.
        <INITIAL> "%x" [a-zA-Z] [a-zA-Z0-9]* ":\t" => USER_TIER_CONTENT {
            emit!(TierPrefix);
        }

        // Phon project tiers: text_with_bullets
        <INITIAL> "%modsyl:\t" => TIER_CONTENT { emit!(TierPrefix); }
        <INITIAL> "%phosyl:\t" => TIER_CONTENT { emit!(TierPrefix); }
        <INITIAL> "%phoaln:\t" => TIER_CONTENT { emit!(TierPrefix); }

        // All other known tiers: text_with_bullets
        <INITIAL> "%" [a-zA-Z][a-zA-Z0-9]* ":\t" => TIER_CONTENT {
            emit!(TierPrefix);
        }
        // Dependent tier prefix without :\t (malformed)
        <INITIAL> "%" [a-zA-Z][a-zA-Z0-9]* => TIER_AFTER_LABEL {
            emit!(TierPrefix);
        }

        // Whitespace at line start (before @, *, %)
        <INITIAL> [ \t]+ {
            emit!(Whitespace);
        }

        // Anything else at line start is an error.
        <INITIAL> [^\x00@*% \t\r\n] [^\x00\r\n]* {
            emit!(ErrorLine);
        }

        // ═══════════════════════════════════════════════════════
        // HEADER_AFTER_NAME — After @ prefix, expect : + tab or newline
        // grammar.js: header_sep = seq(colon, tab)
        // Some headers have no sep (@UTF8, @Begin, @End, @Blank, @New Episode)
        // ═══════════════════════════════════════════════════════

        <HEADER_AFTER_NAME> ":\t" => HEADER_CONTENT {
            emit!(HeaderSep);
        }

        // Header with no colon — newline will reset to INITIAL via <*> rule.
        // ErrorHeaderAfterName catches junk after header name.
        <HEADER_AFTER_NAME> [^\x00:\r\n] {
            emit!(ErrorHeaderAfterName);
        }

        // ═══════════════════════════════════════════════════════
        // ID_CONTENT — @ID header body
        //
        // ULTRA-RICH TOKEN: The entire 10-field pipe-delimited content is
        // ONE token with tags marking each field boundary.
        //
        // Format: lang|corpus|speaker|age|sex|group|ses|role|education|custom|
        // Model: talkbank_model::IDHeader
        //
        // Tags: @t1=after lang, @t2=after corpus, ..., allowing zero-copy
        // field extraction: lang=input[start..t1], corpus=input[t1+1..t2], etc.
        // ═══════════════════════════════════════════════════════

        <ID_CONTENT> [^\x00|\r\n]* @t1 "|" [^\x00|\r\n]* @t2 "|" [^\x00|\r\n]* @t3 "|" [^\x00|\r\n]* @t4 "|" [^\x00|\r\n]* @t5 "|" [^\x00|\r\n]* @t6 "|" [^\x00|\r\n]* @t7 "|" [^\x00|\r\n]* @t8 "|" [^\x00|\r\n]* @t9 "|" [^\x00|\r\n]* "|" {
            // Tags mark pipe positions. Fields are between pipes.
            // Trim trailing whitespace from each field — CHAT convention
            // is that field content excludes trailing spaces.
            let end = self.cursor;
            return Some((Token::IdFields {
                language: yyinput[start..self.t1].trim(),
                corpus: yyinput[self.t1+1..self.t2].trim(),
                speaker: yyinput[self.t2+1..self.t3].trim(),
                age: yyinput[self.t3+1..self.t4].trim(),
                sex: yyinput[self.t4+1..self.t5].trim(),
                group: yyinput[self.t5+1..self.t6].trim(),
                ses: yyinput[self.t6+1..self.t7].trim(),
                role: yyinput[self.t7+1..self.t8].trim(),
                education: yyinput[self.t8+1..self.t9].trim(),
                custom: yyinput[self.t9+1..end-1].trim(), // -1 for trailing |
            }, start..end));
        }

        // Malformed @ID — doesn't have 10 pipe fields. Error recovery.
        <ID_CONTENT> [^\x00\r\n]+ {
            emit!(ErrorInIdContent);
        }

        // ═══════════════════════════════════════════════════════
        // TYPES_CONTENT — @Types header body
        //
        // RICH TOKEN: Three comma-separated fields.
        // Format: design, activity, group
        // Model: talkbank_model::TypesHeader (design, activity, group)
        // ═══════════════════════════════════════════════════════

        <TYPES_CONTENT> @t1 [^\x00,\r\n]+ @t4 "," [ ]* @t2 [^\x00,\r\n]+ @t5 "," [ ]* @t3 [^\x00\r\n]+ {
            // t1..t4 = design, t2..t5 = activity, t3..end = group
            let end = self.cursor;
            return Some((Token::TypesFields {
                design: &yyinput[self.t1..self.t4],
                activity: &yyinput[self.t2..self.t5],
                group: &yyinput[self.t3..end],
            }, start..end));
        }

        // Malformed @Types
        <TYPES_CONTENT> [^\x00\r\n]+ {
            emit!(ErrorInTypesContent);
        }

        // ═══════════════════════════════════════════════════════
        // HEADER_CONTENT — After colon+tab, capture header value
        // grammar.js: free_text = /[^\r\n]+/ (most headers)
        //             Some headers have structured content (ID, date, etc.)
        //             We capture as opaque text; parser structures it.
        // ═══════════════════════════════════════════════════════

        <HEADER_CONTENT> [^\x00\r\n]+ {
            emit!(HeaderContent);
        }

        // ═══════════════════════════════════════════════════════
        // LANGUAGES_CONTENT — @Languages header body
        //
        // grammar.js: languages_contents = seq(language_code, repeat(seq(comma, ws, language_code)))
        // language_code = /[a-z]{2,4}/
        //
        // RICH TOKEN: Each language code is a separate token.
        // ═══════════════════════════════════════════════════════

        <LANGUAGES_CONTENT> [a-z][a-z][a-z]?[a-z]? {
            emit!(LanguageCode);
        }
        <LANGUAGES_CONTENT> "," { emit!(Comma); }
        <LANGUAGES_CONTENT> " "+ { emit!(Whitespace); }
        <LANGUAGES_CONTENT> [^\x00\r\n] { emit!(ErrorInLanguagesContent); }

        // ═══════════════════════════════════════════════════════
        // PARTICIPANTS_CONTENT — @Participants header body
        //
        // grammar.js: participants_contents = seq(participant, repeat(seq(comma, ws, participant)))
        // participant = seq(speaker, repeat(seq(ws, participant_word)))
        // speaker = /[A-Za-z0-9_\'+\-]+/
        // participant_word = /[^, \r\n\t]+/
        //
        // We lex: speaker codes, participant words, commas, whitespace.
        // The structure is: SPK Name Role, SPK Name Role, ...
        // ═══════════════════════════════════════════════════════

        <PARTICIPANTS_CONTENT> [^, \t\r\n\x00]+ {
            // Participant word (speaker code or name/role word)
            emit!(ParticipantWord);
        }
        <PARTICIPANTS_CONTENT> "," { emit!(Comma); }
        <PARTICIPANTS_CONTENT> " "+ { emit!(Whitespace); }
        <PARTICIPANTS_CONTENT> [^\x00\r\n] { emit!(ErrorInParticipantsContent); }

        // ═══════════════════════════════════════════════════════
        // MEDIA_CONTENT — @Media header body
        //
        // grammar.js: media_contents = seq(media_filename, comma, ws, media_type, optional(seq(comma, ws, media_status)))
        // media_filename = choice(quoted_url, /[a-zA-Z0-9_-]+/)
        // media_type = choice('video', 'audio', 'missing', generic)
        // media_status = choice('missing', 'unlinked', 'notrans', generic)
        //
        // RICH TOKEN approach: lex each field separately.
        // ═══════════════════════════════════════════════════════

        <MEDIA_CONTENT> "\"" [^\x00"\r\n]+ "\"" {
            // Quoted filename
            emit!(MediaFilename);
        }
        <MEDIA_CONTENT> [a-zA-Z0-9_\-]+ {
            // Simple filename or media type/status word
            emit!(MediaWord);
        }
        <MEDIA_CONTENT> "," { emit!(Comma); }
        <MEDIA_CONTENT> " "+ { emit!(Whitespace); }
        <MEDIA_CONTENT> [^\x00\r\n] { emit!(ErrorInMediaContent); }

        // ═══════════════════════════════════════════════════════
        // SPEAKER — After *, capture speaker code
        // grammar.js: speaker = /[A-Za-z0-9_\'+\-]+/
        // ═══════════════════════════════════════════════════════

        <SPEAKER> [A-Za-z0-9_'+\-]+ => TIER_SEP {
            emit!(Speaker);
        }

        <SPEAKER> [^\x00\r\n] {
            emit!(ErrorSpeaker);
        }

        // ═══════════════════════════════════════════════════════
        // TIER_AFTER_LABEL — After %label, expect : + tab
        // ═══════════════════════════════════════════════════════

        <TIER_AFTER_LABEL> ":\t" {
            // Dispatch to tier-specific condition based on label.
            // The TierPrefix token already contains the label (e.g., "%mor").
            // Parser decides which condition to enter; lexer enters
            // a generic tier content mode.
            // For structured tiers, the parser will re-lex with a
            // specific start condition.

            // For now, enter TIER_CONTENT (generic text).
            // The parser can re-lex %mor, %gra, etc. with specific conditions.
            self.condition = YYC_TIER_CONTENT;
            let end = self.cursor;
            return Some((Token::TierSep(&yyinput[start..end]), start..end));
        }

        <TIER_AFTER_LABEL> [^\x00:\r\n] {
            emit!(ErrorTierAfterLabel);
        }

        // ═══════════════════════════════════════════════════════
        // TIER_SEP — After speaker code, expect : + tab
        // grammar.js: seq(colon, tab) between speaker and tier_body
        // ═══════════════════════════════════════════════════════

        <TIER_SEP> ":\t" => MAIN_CONTENT {
            emit!(TierSep);
        }

        <TIER_SEP> [^\x00:\r\n] {
            emit!(ErrorTierSep);
        }

        // ═══════════════════════════════════════════════════════
        // MAIN_CONTENT — Main tier body
        // grammar.js: contents = repeat1(choice(whitespaces, content_item, separator, overlap_point))
        // ═══════════════════════════════════════════════════════

        // ── Whitespace ──
        // grammar.js: whitespaces = token(repeat1(choice(' ', /[\r\n]+\t/)))
        <MAIN_CONTENT> " "+ {
            emit!(Whitespace);
        }

        // ── Terminators (grammar.js: all with prec(10)) ──
        // Listed longest-first for re2c precedence.

        // Extended terminators — must come before single-char '+' compound marker
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+..." { emit!(TrailingOff); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+//." { emit!(SelfInterruption); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+/." { emit!(Interruption); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+//?" { emit!(SelfInterruptedQuestion); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+/?" { emit!(InterruptedQuestion); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+!?" { emit!(BrokenQuestion); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+\"/." { emit!(QuotedNewLine); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+\"." { emit!(QuotedPeriodSimple); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+..?" { emit!(TrailingOffQuestion); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "+." { emit!(BreakForCoding); }

        // Basic terminators
        // grammar.js: period = '.', question = '?', exclamation = '!'
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "." { emit!(Period); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "?" { emit!(Question); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "!" { emit!(Exclamation); }

        // CA terminators
        // grammar.js: ca_no_break = token(prec(10, '≈'))
        // grammar.js: ca_technical_break = token(prec(10, '\u224B'))
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "\u2248" { emit!(CaNoBreak); }
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "\u224B" { emit!(CaTechnicalBreak); }

        // ── Linkers (grammar.js: all with prec(10)) ──
        // These must come before single '+' compound marker.
        // grammar.js: linker_lazy_overlap = token(prec(10, '+<'))
        <MAIN_CONTENT> "+<" { emit!(LinkerLazyOverlap); }
        // grammar.js: linker_quick_uptake = token(prec(10, '++'))
        <MAIN_CONTENT> "++" { emit!(LinkerQuickUptake); }
        // grammar.js: linker_quick_uptake_overlap = token(prec(10, '+^'))
        <MAIN_CONTENT> "+^" { emit!(LinkerQuickUptakeOverlap); }
        // grammar.js: linker_quotation_follows = token(prec(10, '+"'))
        <MAIN_CONTENT> "+\"" { emit!(LinkerQuotationFollows); }
        // grammar.js: linker_self_completion = token(prec(10, '+,'))
        <MAIN_CONTENT> "+," { emit!(LinkerSelfCompletion); }
        // grammar.js: ca_no_break_linker = token(prec(10, '+≈'))
        <MAIN_CONTENT> "+\u2248" { emit!(CaNoBreakLinker); }
        // grammar.js: ca_technical_break_linker = token(prec(10, '+≋'))
        <MAIN_CONTENT> "+\u224B" { emit!(CaTechnicalBreakLinker); }

        // ── Annotations (grammar.js: atomic tokens) ──
        // These are complete bracket annotations lexed as single tokens.

        // grammar.js: retrace_complete = token('[//]')
        <MAIN_CONTENT> "[//]" { emit!(RetraceComplete); }
        // grammar.js: retrace_partial = token('[/]')
        <MAIN_CONTENT> "[/]" { emit!(RetracePartial); }
        // grammar.js: retrace_multiple = token('[///]')
        <MAIN_CONTENT> "[///]" { emit!(RetraceMultiple); }
        // grammar.js: retrace_reformulation = token('[/-]')
        <MAIN_CONTENT> "[/-]" { emit!(RetraceReformulation); }
        // grammar.js: scoped_stressing = token('[!]')
        <MAIN_CONTENT> "[!]" { emit!(ScopedStressing); }
        // grammar.js: scoped_contrastive_stressing = token('[!!]')
        <MAIN_CONTENT> "[!!]" { emit!(ScopedContrastiveStressing); }
        // grammar.js: scoped_uncertain = token('[?]')
        <MAIN_CONTENT> "[?]" { emit!(ScopedUncertain); }
        // grammar.js: exclude_marker = token('[e]')
        <MAIN_CONTENT> "[e]" { emit!(ExcludeMarker); }
        // grammar.js: freecode = token(/\[\^ [^\]\r\n]+\]/)
        // Must come before ca_continuation_marker since [^c] is a special case of freecode.
        // Tag marks the content (between [^ and ])
        <MAIN_CONTENT> "[^ " @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(Freecode); }

        // grammar.js: ca_continuation_marker = token('[^c]')
        <MAIN_CONTENT> "[^c]" { emit!(CaContinuationMarker); }

        // grammar.js: error_marker_annotation = token(prec(8, /\[\*[^\]]*\]/))
        // Content can contain spaces (e.g., [* s: ur]), so only exclude ] and NUL.
        <MAIN_CONTENT> "[*" " "? @t1 [^\x00\]]* @t2 "]" { emit_t1t2!(ErrorMarkerAnnotation); }

        // grammar.js: indexed_overlap_precedes/follows — tag marks optional index digit
        <MAIN_CONTENT> "[<" " "? @t1 [1-9]? @t2 " "? "]" { emit_t1t2!(OverlapPrecedes); }
        <MAIN_CONTENT> "[>" " "? @t1 [1-9]? @t2 " "? "]" { emit_t1t2!(OverlapFollows); }

        // Annotations with content — tags extract content directly (zero-copy).
        <MAIN_CONTENT> "[= " @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(ExplanationAnnotation); }
        <MAIN_CONTENT> "[=! " @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(ParaAnnotation); }
        <MAIN_CONTENT> "[=? " @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(AltAnnotation); }
        <MAIN_CONTENT> "[% " @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(PercentAnnotation); }

        <MAIN_CONTENT> "[+ " @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(Postcode); }

        <MAIN_CONTENT> "[- " @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(Langcode); }

        // grammar.js: replacement = seq('[', ':', words..., ']')
        <MAIN_CONTENT> "[:" @t1 [^\x00\]\r\n]+ @t2 "]" { emit_t1t2!(Replacement); }

        // ── Pauses (grammar.js: pause_token with prec(10)) ──
        // grammar.js: token(prec(10, choice('(.)', '(..)', '(...)', /\(\d+(?::\d+)?\.\d*\)/)))
        <MAIN_CONTENT> "(...)" { emit!(PauseLong); }
        <MAIN_CONTENT> "(..)" { emit!(PauseMedium); }
        <MAIN_CONTENT> "(.)" { emit!(PauseShort); }
        // PauseTimed: tags mark the numeric content
        <MAIN_CONTENT> "(" @t1 [0-9]+ (":" [0-9]+)? "." [0-9]* @t2 ")" { emit_t1t2!(PauseTimed); }

        // NOTE: Shortening and ErrorUnclosedParen rules moved AFTER the Word
        // rules (below) so that standalone `(parens)` matches as a rich Word
        // token. The Word rule's w_body includes w_short, so it handles
        // shortenings within word bodies AND standalone shortenings.

        // ── Separators (grammar.js: non_colon_separator) ──
        <MAIN_CONTENT> "," { emit!(Comma); }
        <MAIN_CONTENT> ";" { emit!(Semicolon); }
        // grammar.js: tag_marker = token(prec(10, '\u201E'))
        <MAIN_CONTENT> "\u201E" { emit!(TagMarker); }
        // grammar.js: vocative_marker = token(prec(10, '\u2021'))
        <MAIN_CONTENT> "\u2021" { emit!(VocativeMarker); }
        // grammar.js: unmarked_ending = token(prec(10, '\u221E'))
        <MAIN_CONTENT> "\u221E" { emit!(UnmarkedEnding); }
        // grammar.js: uptake_symbol = token(prec(10, '\u2261'))
        <MAIN_CONTENT> "\u2261" { emit!(UptakeSymbol); }

        // Intonation contours (grammar.js: all prec(10))
        <MAIN_CONTENT> "\u21D7" { emit!(RisingToHigh); }
        <MAIN_CONTENT> "\u2197" { emit!(RisingToMid); }
        <MAIN_CONTENT> "\u2192" { emit!(LevelPitch); }
        <MAIN_CONTENT> "\u2198" { emit!(FallingToMid); }
        <MAIN_CONTENT> "\u21D8" { emit!(FallingToLow); }

        // ── Groups ──
        // grammar.js: less_than = '<', greater_than = '>'
        <MAIN_CONTENT> "<" { emit!(LessThan); }
        <MAIN_CONTENT> ">" { emit!(GreaterThan); }

        // grammar.js: left_double_quote = token(prec(10, '\u201C'))
        <MAIN_CONTENT> "\u201C" { emit!(LeftDoubleQuote); }
        // grammar.js: right_double_quote = token(prec(10, '\u201D'))
        <MAIN_CONTENT> "\u201D" { emit!(RightDoubleQuote); }

        // ═══════════════════════════════════════════════════════
        // WORD — Rich token matching a complete word.
        //
        // grammar.js: standalone_word = prefix? word_body form_marker? lang_suffix? pos_tag?
        //
        // Tags mark field boundaries for zero-copy extraction:
        //   @t1 = end of prefix (if present) / start of body
        //   @t2 = end of body
        //   @t3..@t4 = form marker content (without @)
        //   @t5 = start of lang suffix codes (after @s:)
        //   @t6 = start of POS tag content (after $)
        //
        // re2c longest-match ensures terminators (+..., +/., etc.) and
        // linkers (+<, ++, etc.) are NOT consumed by the + in compounds,
        // because + in the word body requires a following w_atom.
        // ═══════════════════════════════════════════════════════

        // ── Standalone overlap points (must come BEFORE Word rule) ──
        // When ⌈2 has whitespace after it, both the Word rule and the overlap rule
        // match the same 2 chars. re2c first-rule wins, so overlap must come first.
        <MAIN_CONTENT> "\u2308" [1-9]? { emit!(OverlapTopBegin); }    // ⌈
        <MAIN_CONTENT> "\u2309" [1-9]? { emit!(OverlapTopEnd); }      // ⌉
        <MAIN_CONTENT> "\u230A" [1-9]? { emit!(OverlapBottomBegin); } // ⌊
        <MAIN_CONTENT> "\u230B" [1-9]? { emit!(OverlapBottomEnd); }   // ⌋

        // Word with prefix (&-, &~, &+)
        <MAIN_CONTENT> w_prefix @t1 w_body @t2 ("@" @t3 ("u" | "b" | "c" | "d" | "f" | "fp" | "g" | "i" | "k" | "l" | "ls" | "n" | "o" | "p" | "q" | "sas" | "si" | "sl" | "t" | "wp" | "x" | "z") (":" [a-zA-Z0-9_]+)? @t4)? ("@s" (":" @t5 [a-z][a-z][a-z]? ([+&] [a-z][a-z][a-z]?)*)? @t6)? ("$" @t7 [a-zA-Z:]+ @t8)? {
            let end = self.cursor;
            let raw_text = &yyinput[start..end];
            let prefix = Some(&yyinput[start..self.t1]);
            let body = &yyinput[self.t1..self.t2];
            let form_marker = if self.t3 != NONE && self.t4 != NONE { Some(&yyinput[self.t3..self.t4]) } else { None };
            let lang_suffix = if self.t6 != NONE { if self.t5 != NONE { Some(&yyinput[self.t5..self.t6]) } else { Some("") } } else { None };
            let pos_tag = if self.t7 != NONE && self.t8 != NONE { Some(&yyinput[self.t7..self.t8]) } else { None };
            return Some((Token::Word { raw_text, prefix, body, form_marker, lang_suffix, pos_tag }, start..end));
        }

        // Word with zero prefix (0 followed by word body)
        <MAIN_CONTENT> "0" @t1 w_body @t2 ("@" @t3 ("u" | "b" | "c" | "d" | "f" | "fp" | "g" | "i" | "k" | "l" | "ls" | "n" | "o" | "p" | "q" | "sas" | "si" | "sl" | "t" | "wp" | "x" | "z") (":" [a-zA-Z0-9_]+)? @t4)? ("@s" (":" @t5 [a-z][a-z][a-z]? ([+&] [a-z][a-z][a-z]?)*)? @t6)? ("$" @t7 [a-zA-Z:]+ @t8)? {
            let end = self.cursor;
            let raw_text = &yyinput[start..end];
            let body = &yyinput[self.t1..self.t2];
            let form_marker = if self.t3 != NONE && self.t4 != NONE { Some(&yyinput[self.t3..self.t4]) } else { None };
            let lang_suffix = if self.t6 != NONE { if self.t5 != NONE { Some(&yyinput[self.t5..self.t6]) } else { Some("") } } else { None };
            let pos_tag = if self.t7 != NONE && self.t8 != NONE { Some(&yyinput[self.t7..self.t8]) } else { None };
            return Some((Token::Word { raw_text, prefix: Some("0"), body, form_marker, lang_suffix, pos_tag }, start..end));
        }

        // Word without prefix (body starts immediately)
        <MAIN_CONTENT> w_body @t2 ("@" @t3 ("u" | "b" | "c" | "d" | "f" | "fp" | "g" | "i" | "k" | "l" | "ls" | "n" | "o" | "p" | "q" | "sas" | "si" | "sl" | "t" | "wp" | "x" | "z") (":" [a-zA-Z0-9_]+)? @t4)? ("@s" (":" @t5 [a-z][a-z][a-z]? ([+&] [a-z][a-z][a-z]?)*)? @t6)? ("$" @t7 [a-zA-Z:]+ @t8)? {
            let end = self.cursor;
            let raw_text = &yyinput[start..end];
            let body = &yyinput[start..self.t2];
            let form_marker = if self.t3 != NONE && self.t4 != NONE { Some(&yyinput[self.t3..self.t4]) } else { None };
            let lang_suffix = if self.t6 != NONE { if self.t5 != NONE { Some(&yyinput[self.t5..self.t6]) } else { Some("") } } else { None };
            let pos_tag = if self.t7 != NONE && self.t8 != NONE { Some(&yyinput[self.t7..self.t8]) } else { None };
            return Some((Token::Word { raw_text, prefix: None, body, form_marker, lang_suffix, pos_tag }, start..end));
        }

        // NOTE: Shortening sub-token rule removed — shadowed by Word rules above.
        // `(parens)` is matched as part of w_body by the Word rule at higher priority.

        // Unclosed paren — error recovery
        <MAIN_CONTENT> "(" [^\x00 \t\r\n)]* {
            emit!(ErrorUnclosedParen);
        }

        // ── Word sub-tokens (kept for body re-lexing and standalone use) ──

        // ── Word prefixes ──
        // grammar.js: word_prefix = choice(token('&-'), token('&~'), token('&+'))
        <MAIN_CONTENT> "&-" { emit!(PrefixFiller); }
        <MAIN_CONTENT> "&~" { emit!(PrefixNonword); }
        <MAIN_CONTENT> "&+" { emit!(PrefixFragment); }

        // grammar.js: event = seq(event_marker, event_segment+)
        // Single token: &= followed by one or more event segment chars.
        // Event segments allow colon (for &=clears:throat) but exclude
        // all structural chars (brackets, CA symbols, etc.).
        <MAIN_CONTENT> "&=" @t1 ev_char+ @t2 { emit_t1t2!(Event); }

        // grammar.js: zero = token(prec(3, '0'))
        <MAIN_CONTENT> "0" { emit!(Zero); }

        // ── Standalone colon = separator (not lengthening) ──
        // grammar.js: colon = ':' (separator); lengthening is word-internal only
        // Inside word bodies, `:` is handled by w_body's w_atom ":"+.
        // Standalone `:` between words is a separator.
        <MAIN_CONTENT> ":" { emit!(Colon); }

        // Lengthening sub-token: multiple colons that didn't match as part of
        // a word body. This is a fallback — normally `::` appears inside words.
        <MAIN_CONTENT> ":"+ { emit!(Lengthening); }

        // grammar.js: compound marker = '+' (in _word_marker)
        // Must come after all +X terminators/linkers above.
        <MAIN_CONTENT> "+" { emit!(CompoundMarker); }

        // grammar.js: overlap_point = token(prec(5, /[\u2308\u2309\u230A\u230B][1-9]?/))
        // (Standalone overlap rules moved above Word rules for priority.
        // These remain as comments for reference; body-internal overlaps
        // are handled by the Word rule's w_overlap pattern.)

        // NOTE: Stress marker sub-token rules removed — shadowed by Word rule
        // which matches ˈ and ˌ as part of w_body.

        // grammar.js: syllable_pause = '^'
        <MAIN_CONTENT> "^" { emit!(SyllablePause); }

        // grammar.js: tilde = '~'
        <MAIN_CONTENT> "~" { emit!(Tilde); }

        // grammar.js: underline_begin = token(prec(5, '\u0002\u0001'))
        <MAIN_CONTENT> "\u0002\u0001" { emit!(UnderlineBegin); }
        // grammar.js: underline_end = token(prec(5, '\u0002\u0002'))
        <MAIN_CONTENT> "\u0002\u0002" { emit!(UnderlineEnd); }

        // ── CA elements (one token per type for zero-dispatch conversion) ──
        <MAIN_CONTENT> "\u2260" { emit!(CaBlockedSegments); }   // ≠
        <MAIN_CONTENT> "\u223E" { emit!(CaConstriction); }      // ∾
        <MAIN_CONTENT> "\u2051" { emit!(CaHardening); }         // ⁑
        <MAIN_CONTENT> "\u2907" { emit!(CaHurriedStart); }      // ⤇
        <MAIN_CONTENT> "\u2219" { emit!(CaInhalation); }        // ∙
        <MAIN_CONTENT> "\u1F29" { emit!(CaLaughInWord); }       // Ἡ
        <MAIN_CONTENT> "\u2193" { emit!(CaPitchDown); }         // ↓
        <MAIN_CONTENT> "\u21BB" { emit!(CaPitchReset); }        // ↻
        <MAIN_CONTENT> "\u2191" { emit!(CaPitchUp); }           // ↑
        <MAIN_CONTENT> "\u2906" { emit!(CaSuddenStop); }        // ⤆

        // ── CA delimiters (one token per type for zero-dispatch conversion) ──
        <MAIN_CONTENT> "\u2047" { emit!(CaUnsure); }            // ⁇
        <MAIN_CONTENT> "\u00A7" { emit!(CaPrecise); }           // §
        <MAIN_CONTENT> "\u204E" { emit!(CaCreaky); }            // ⁎
        <MAIN_CONTENT> "\u00B0" { emit!(CaSofter); }            // °
        <MAIN_CONTENT> "\u21AB" { emit!(CaSegmentRepetition); } // ↫
        <MAIN_CONTENT> "\u2206" { emit!(CaFaster); }            // ∆
        <MAIN_CONTENT> "\u2207" { emit!(CaSlower); }            // ∇
        <MAIN_CONTENT> "\u222C" { emit!(CaWhisper); }           // ∬
        <MAIN_CONTENT> "\u222E" { emit!(CaSinging); }           // ∮
        <MAIN_CONTENT> "\u2581" { emit!(CaLowPitch); }          // ▁
        <MAIN_CONTENT> "\u2594" { emit!(CaHighPitch); }         // ▔
        <MAIN_CONTENT> "\u25C9" { emit!(CaLouder); }            // ◉
        <MAIN_CONTENT> "\u263A" { emit!(CaSmileVoice); }        // ☺
        <MAIN_CONTENT> "\u264B" { emit!(CaBreathyVoice); }      // ♋
        <MAIN_CONTENT> "\u03AB" { emit!(CaYawn); }              // Ϋ

        // ── Word suffix markers (tagged) ──
        // grammar.js: form_marker = token.immediate(/@(?:u|b|...|z)(?::[a-zA-Z0-9_]+)?/)
        // t1 = start of marker type, t2 = end of marker type (before optional :suffix)
        <MAIN_CONTENT> "@" @t1 ("u" | "b" | "c" | "d" | "f" | "fp" | "g" | "i" | "k" | "l" | "ls" | "n" | "o" | "p" | "q" | "sas" | "si" | "sl" | "t" | "wp" | "x" | "z") @t2 (":" [a-zA-Z0-9_]+)? {
            emit_t1!(FormMarker);
        }

        // grammar.js: word_lang_suffix = token.immediate(/@s(?::[a-z]{2,3}(?:[+&][a-z]{2,3})*)?/)
        // t1 = start of language code(s) after @s:
        <MAIN_CONTENT> "@s" (":" @t1 [a-z][a-z][a-z]? ([+&] [a-z][a-z][a-z]?)*)? {
            // Bare @s → None (shortcut). @s:eng+zho → Some("eng+zho").
            let end = self.cursor;
            let content = if self.t1 != NONE { Some(&yyinput[self.t1..end]) } else { None };
            return Some((Token::WordLangSuffix(content), start..end));
        }

        // grammar.js: pos_tag = seq(token.immediate('$'), /[a-zA-Z:]+/)
        // t1 = start of tag content after $
        <MAIN_CONTENT> "$" @t1 [a-zA-Z:]+ { emit_t1!(PosTag); }

        // ── Media bullet (RICH TOKEN with tagged timestamps) ──
        // grammar.js: bullet = seq(bullet_start, start_time, '_', end_time, bullet_end)
        // Tags mark start_time and end_time boundaries for zero-copy extraction.
        // Legacy skip dash (-) before closing NAK is silently accepted (deprecated).
        <MAIN_CONTENT, MOR_CONTENT, GRA_CONTENT, TIER_CONTENT, COM_CONTENT, USER_TIER_CONTENT> "\u0015" @t1 [0-9]+ @t2 "_" @t3 [0-9]+ @t4 "-"? "\u0015" {
            let end = self.cursor;
            return Some((Token::MediaBullet {
                raw_text: &yyinput[start..end],
                start_time: &yyinput[self.t1..self.t2],
                end_time: &yyinput[self.t3..self.t4],
            }, start..end));
        }

        // ── PHO/SIN group delimiters ──
        // grammar.js: pho_begin_group = '‹' (U+2039), pho_end_group = '›' (U+203A)
        <MAIN_CONTENT> "\u2039" { emit!(PhoGroupBegin); }
        <MAIN_CONTENT> "\u203A" { emit!(PhoGroupEnd); }
        // grammar.js: sin_begin_group = '〔' (U+3014), sin_end_group = '〕' (U+3015)
        <MAIN_CONTENT> "\u3014" { emit!(SinGroupBegin); }
        <MAIN_CONTENT> "\u3015" { emit!(SinGroupEnd); }

        // grammar.js: long_feature_begin = seq('&', '{l=', label)
        // grammar.js: long_feature_end = seq('&', '}l=', label)
        // Rich token with tag marking label start.
        <MAIN_CONTENT> "&{l=" @t1 [A-Za-z0-9@%_\-]+ { emit_t1!(LongFeatureBegin); }
        <MAIN_CONTENT> "&}l=" @t1 [A-Za-z0-9@%_\-]+ { emit_t1!(LongFeatureEnd); }

        // grammar.js: nonvocal_simple = seq('&', '{n=', label, '}')
        // Must come before nonvocal_begin (re2c longest match: &{n=BANG} > &{n=BANG)
        <MAIN_CONTENT> "&{n=" @t1 [A-Za-z0-9@%_\-]+ @t2 "}" { emit_t1t2!(NonvocalSimple); }

        // grammar.js: nonvocal_begin = seq('&', '{n=', label)
        // grammar.js: nonvocal_end = seq('&', '}n=', label)
        <MAIN_CONTENT> "&{n=" @t1 [A-Za-z0-9@%_\-]+ { emit_t1!(NonvocalBegin); }
        <MAIN_CONTENT> "&}n=" @t1 [A-Za-z0-9@%_\-]+ { emit_t1!(NonvocalEnd); }

        // grammar.js: other_spoken_event = seq(ampersand, star, speaker, colon, standalone_word)
        // RICH TOKEN: &*SPK:word — entire other spoken event as one token.
        // Tags mark speaker and word boundaries.
        // Must come before bare '&' rule (re2c picks longest match).
        //
        // The text portion uses `w_body` (the same body production used by
        // `standalone_word`). Earlier this rule used the much looser
        // `[^ \t\r\n\x00]+` pattern, which greedily absorbed the next
        // utterance terminator when there was no space between them
        // (e.g. `&*INV:oh_hm.` was tokenized with text `oh_hm.` rather
        // than `oh_hm` followed by `.`). A first attempt with `w_text`
        // narrowed correctly past `.?!` but went too far — it also
        // excluded the lengthening colon (`&*INV:ah:`), regressing 27
        // wild-corpus files. `w_body` is the right surface: it allows
        // `:+`, `^`, `~`, CA elements, etc. but stops at terminators.
        // Found by the re2c parity oracle on 2026-04-30.
        <MAIN_CONTENT> "&*" @t1 [A-Za-z0-9_'+\-]+ @t2 ":" @t3 w_body {
            let end = self.cursor;
            return Some((Token::OtherSpokenEvent {
                speaker: &yyinput[self.t1..self.t2],
                text: &yyinput[self.t3..end],
            }, start..end));
        }

        // grammar.js: ampersand = '&' (bare, for constructs not caught above)
        <MAIN_CONTENT> "&" { emit!(Ampersand); }

        // grammar.js: left_bracket = '['
        <MAIN_CONTENT> "[" { emit!(LeftBracket); }
        // grammar.js: right_bracket = ']'
        <MAIN_CONTENT> "]" { emit!(RightBracket); }

        // ── Word segment (catch-all for text runs) ──
        // grammar.js: word_segment = token(prec(5, seq(WORD_SEGMENT_FIRST_RE, WORD_SEGMENT_REST_RE)))
        // WORD_SEGMENT_FORBIDDEN_FIRST includes: ,;:!?.()[]{}⌈⌉⌊⌋〔〕\^ˈˌ←→↖↗↘↙⇗⇘<>≈≋
        //   + CA_ALL_SYMBOLS + \u0015\u0001-\u0004\u0007\u0008\t\n\r ‹›""„@*&%‡+=~∞≡$ + 0
        // WORD_SEGMENT_FORBIDDEN_REST is same but without 0 and with \\ extra
        //
        // Simplified: exclude all structural chars. Anything not matched by
        // specific rules above falls here.
        <MAIN_CONTENT> [^ \t\r\n\x00,;:!?.()[\]{}~^+$@&*%"<>\u0015\u0001\u0002\u0003\u0004\u0007\u0008\u2308\u2309\u230A\u230B\u3014\u3015\u2039\u203A\u02C8\u02CC\u201C\u201D\u201E\u2021\u2248\u224B\u221E\u2261\u21D7\u2197\u2192\u2198\u21D8\u2051\u2191\u2193\u21BB\u2260\u2219\u223E\u2906\u2907\u1F29\u2047\u00A7\u204E\u00B0\u21AB\u2206\u2207\u222C\u222E\u2581\u2594\u25C9\u263A\u264B\u03AB0] [^ \t\r\n\x00,;:!?.()[\]{}~^+$@&*%"<>\u0015\u0001\u0002\u0003\u0004\u0007\u0008\u2308\u2309\u230A\u230B\u3014\u3015\u2039\u203A\u02C8\u02CC\u201C\u201D\u201E\u2021\u2248\u224B\u221E\u2261\u21D7\u2197\u2192\u2198\u21D8\u2051\u2191\u2193\u21BB\u2260\u2219\u223E\u2906\u2907\u1F29\u2047\u00A7\u204E\u00B0\u21AB\u2206\u2207\u222C\u222E\u2581\u2594\u25C9\u263A\u264B\u03AB]* {
            emit!(WordSegment);
        }

        // ═══════════════════════════════════════════════════════
        // MOR_CONTENT — %mor tier body
        //
        // RICH TOKENS: Each mor_word (POS|lemma[-feat]*) is emitted as a SINGLE
        // MorWord token. Tags mark internal boundaries for zero-copy extraction.
        // The parser never re-scans mor word internals.
        //
        // grammar.js: mor_word = seq(mor_pos, '|', mor_lemma, repeat(seq('-', mor_feature_value)))
        // grammar.js: mor_content = seq(mor_word, repeat(seq('~', mor_word)))
        //
        // Token stream for "pron|it~aux|be-Fin ." is:
        //   MorWord("pron|it"), MorTilde("~"), MorWord("aux|be-Fin"), Whitespace, Period
        // NOT: MorPos, Pipe, MorSegment, MorTilde, MorPos, Pipe, MorSegment, Hyphen, ...
        // ═══════════════════════════════════════════════════════

        // Rich MorWord token: POS|lemma[-feature]*
        // Matches: chars-before-pipe | chars-after-pipe[-chars]*
        // The entire POS|lemma-feat-feat is one token.
        // Tags mark the pipe position for zero-copy POS/lemma extraction.
        // RICH MorWord: POS|lemma-feat1-feat2
        //
        // The ENTIRE POS|lemma[-feat]* is one token. We match:
        //   POS chars: grammar.js mor_pos (no . ? ! | + ~ $ # @ % = & [] <> () - , space)
        //   | pipe
        //   Rest: everything until space, ., ?, |, +, ~ or newline
        //         (includes - for features, ! for Basque derivational boundaries, etc.)
        //
        // grammar.js splits mor_lemma from mor_feature at -, but we capture
        // the whole thing as one rich token. The parser splits on | and -.
        <MOR_CONTENT> [^\x00.?!|+~$#@%=&[\]<>()\-,; \t\r\n\u201C\u201D]+ @t1 "|" @t2 [^\x00.?|+~ \t\r\n\u201C\u201D]+ {
            let end = self.cursor;
            return Some((Token::MorWord {
                pos: &yyinput[start..self.t1],
                lemma_features: &yyinput[self.t2..end],
            }, start..end));
        }

        // Tilde (clitic separator between mor words)
        <MOR_CONTENT> "~" { emit!(MorTilde); }

        // Whitespace between mor items
        <MOR_CONTENT> " "+ { emit!(Whitespace); }

        // ═══════════════════════════════════════════════════════
        // GRA_CONTENT — %gra tier body
        //
        // RICH TOKENS: Each gra_relation (index|head|RELATION) is emitted as
        // a SINGLE GraRelation token with tags marking field boundaries.
        //
        // grammar.js: gra_relation = seq(gra_index, '|', gra_head, '|', gra_relation_name)
        // ═══════════════════════════════════════════════════════

        // Rich GraRelation token: index|head|RELATION
        // Tags mark pipe positions for zero-copy field extraction.
        <GRA_CONTENT> [0-9]+ @t1 "|" @t2 [0-9]+ @t3 "|" @t4 [A-Z][A-Z0-9\-]* {
            let end = self.cursor;
            return Some((Token::GraRelation {
                index: &yyinput[start..self.t1],
                head: &yyinput[self.t2..self.t3],
                relation: &yyinput[self.t4..end],
            }, start..end));
        }

        <GRA_CONTENT> " "+ { emit!(Whitespace); }

        // ═══════════════════════════════════════════════════════
        // PHO_CONTENT — %pho/%mod tier body
        //
        // grammar.js: pho_groups = seq(pho_group, repeat(ws, pho_group))
        // grammar.js: pho_words = seq(pho_word, repeat(seq('+', pho_word)))
        // grammar.js: pho_word = /[a-zA-Z0-9\u0061-...\u0335*]+/
        //
        // RICH TOKEN: pho_word is a single IPA token. Plus (+) joins compounds.
        // Group delimiters ‹ › are separate tokens.
        // ═══════════════════════════════════════════════════════

        // PHO word: IPA + phonological characters.
        // grammar.js: pho_word includes (, ., ), ^, * in its character class.
        // So (..) is a pho_word, NOT a pause — pauses are main tier only.
        // First char excludes only structural separators (space, +, ‹, ›, NAK).
        // Dot and parens are INCLUDED (unlike main tier).
        <PHO_CONTENT> [^\x00 \t\r\n+\u2039\u203A\u0015] [^\x00 \t\r\n+\u2039\u203A\u0015]* {
            emit!(PhoWord);
        }

        // Plus joins compound phonological words
        <PHO_CONTENT> "+" { emit!(PhoPlus); }

        // PHO group delimiters
        <PHO_CONTENT> "\u2039" { emit!(PhoGroupBegin); }
        <PHO_CONTENT> "\u203A" { emit!(PhoGroupEnd); }

        <PHO_CONTENT> " "+ { emit!(Whitespace); }

        // ═══════════════════════════════════════════════════════
        // SIN_CONTENT — %sin tier body
        //
        // grammar.js: sin_word = choice(zero, /[a-zA-Z0-9:_-]+/)
        // Group delimiters: 〔 〕
        // ═══════════════════════════════════════════════════════

        <SIN_CONTENT> [a-zA-Z0-9:_\-]+ {
            emit!(SinWord);
        }

        // NOTE: SIN_CONTENT Zero rule removed — shadowed by SinWord rule above
        // which matches "0" via [a-zA-Z0-9:_\-]+.

        <SIN_CONTENT> "\u3014" { emit!(SinGroupBegin); }
        <SIN_CONTENT> "\u3015" { emit!(SinGroupEnd); }

        <SIN_CONTENT> " "+ { emit!(Whitespace); }

        // WOR_CONTENT is not needed — %wor uses MAIN_CONTENT rules directly.
        // The dispatch in INITIAL sends %wor:\t → MAIN_CONTENT.

        // ═══════════════════════════════════════════════════════
        // TIER_CONTENT — Standard dependent tier body (text_with_bullets)
        // grammar.js: text_with_bullets = repeat1(choice(text_segment, inline_bullet, continuation))
        // grammar.js: text_segment = /[^\u0015\r\n]+/
        // Used by: %act, %add, %cod, %err, %exp, %gpx, %int, %sit, %spa,
        //          %tim, %alt, %coh, %def, %fac, %flo, %gls, %ort, %par,
        //          %modsyl, %phosyl, %phoaln
        // ═══════════════════════════════════════════════════════

        <TIER_CONTENT> [^\x00\u0015\r\n]+ {
            emit!(TextSegment);
        }

        // ═══════════════════════════════════════════════════════
        // COM_CONTENT — %com tier body (text_with_bullets_and_pics)
        // grammar.js: text_with_bullets_and_pics = repeat1(choice(
        //   text_segment, inline_bullet, inline_pic, continuation))
        // Identical to TIER_CONTENT but adds inline_pic support.
        // ═══════════════════════════════════════════════════════

        // grammar.js: inline_pic = token(/\u0015%pic:"[a-zA-Z0-9][a-zA-Z0-9\/\-_'.]*"\u0015/)
        <COM_CONTENT> "\u0015%pic:\"" @t1 [a-zA-Z0-9] [a-zA-Z0-9/\-_'.]* @t2 "\"" "\u0015" {
            emit_t1t2!(InlinePic);
        }

        <COM_CONTENT> [^\x00\u0015\r\n]+ {
            emit!(TextSegment);
        }

        // ═══════════════════════════════════════════════════════
        // USER_TIER_CONTENT — User-defined tier body (%x*)
        // grammar.js: x_dependent_tier uses text_with_bullets
        // Currently identical to TIER_CONTENT. Having a separate
        // condition lets us evolve user-defined tier handling
        // independently (e.g., opaque text if needed).
        // ═══════════════════════════════════════════════════════

        <USER_TIER_CONTENT> [^\x00\u0015\r\n]+ {
            emit!(TextSegment);
        }

        // ═══════════════════════════════════════════════════════
        // PER-CONDITION ERROR FALLBACKS
        //
        // Each condition has a single-character catch-all that:
        // 1. Emits a context-specific error token
        // 2. Stays in the same condition (continues lexing)
        // 3. Consumes exactly ONE character to make progress
        //
        // The parser uses these error tokens to report diagnostics
        // with context about WHERE the error occurred.
        // ═══════════════════════════════════════════════════════

        // MAIN_CONTENT: unexpected char in main tier body.
        // Excludes \r\n\x00 so newline/EOF rules (<*>) still fire.
        <MAIN_CONTENT> [^\x00\r\n] { emit!(ErrorInMainContent); }

        // MOR_CONTENT: unexpected char in %mor tier body
        <MOR_CONTENT> [^\x00\r\n] { emit!(ErrorInMorContent); }

        // GRA_CONTENT: unexpected char in %gra tier body
        <GRA_CONTENT> [^\x00\r\n] { emit!(ErrorInGraContent); }

        // PHO_CONTENT: unexpected char in %pho tier body
        <PHO_CONTENT> [^\x00\r\n] { emit!(ErrorInPhoContent); }

        // SIN_CONTENT: unexpected char in %sin tier body
        <SIN_CONTENT> [^\x00\r\n] { emit!(ErrorInSinContent); }

        // TIER_CONTENT: unexpected char in generic tier body
        <TIER_CONTENT> [^\x00\r\n] { emit!(ErrorInTierContent); }

        // COM_CONTENT: unexpected char in %com tier body
        <COM_CONTENT> [^\x00\r\n] { emit!(ErrorInTierContent); }

        // USER_TIER_CONTENT: unexpected char in user-defined tier body
        <USER_TIER_CONTENT> [^\x00\r\n] { emit!(ErrorInTierContent); }

        // NOTE: HEADER_CONTENT error fallback removed — shadowed by the
        // greedy HeaderContent rule which matches [^\x00\r\n]+.

        // Global fallback — catches anything not handled above.
        // Should never fire if per-condition fallbacks are complete.
        <*> * {
            emit!(ErrorUnrecognized);
        }
    */
    }
}

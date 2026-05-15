//! Word-level emission — `<w>`, `<tagMarker>`, `<pause>`, `<t>`, and
//! the `<g>` wrappers for retraces and annotated words.
//!
//! Every function here writes the spoken-text side of a main-tier
//! chunk. `%mor` subtrees inside these elements are delegated to
//! `super::mor` via `emit_word_mor_subtree` so morphology logic stays
//! in one place.
//!
//! Staged features are reported via
//! [`XmlWriteError::FeatureNotImplemented`] with a descriptive
//! `feature:` string. This keeps the golden-XML harness producing
//! single-phenomenon failure diagnostics rather than swallowing
//! unimplemented constructs silently.

use quick_xml::events::{BytesEnd, BytesStart, Event};

use talkbank_model::alignment::TierDomain;
use talkbank_model::alignment::helpers::counts_for_tier;
use talkbank_model::model::{
    Action, Annotated, BracketedItem, CADelimiter, CADelimiterType, CAElement, CAElementType,
    ContentAnnotation, Event as CEvent, FormType, GrammaticalRelation, Linker, Mor,
    OverlapPointKind, Pause, PauseDuration, ReplacedWord, Retrace, RetraceKind, Separator,
    Terminator, Word, WordContent, WordStressMarkerType,
};

use super::error::XmlWriteError;
use super::mor::{TierCursors, UtteranceTiers, gra_entry};
use super::writer::{XmlEmitter, escape_text};

impl XmlEmitter {
    /// Emit `<w[ type="…"][ untranscribed="…"]>TEXT[<mor…/>]</w>` for
    /// a main-tier [`Word`]. Form-type suffixes (`@a`, `@b`, …),
    /// language markers (`@s`), and word-internal POS suffixes are
    /// staged — fail loud so the harness picks up the missing
    /// increments.
    pub(super) fn emit_word(
        &mut self,
        word: &Word,
        mor: Option<&Mor>,
        gra: Option<&[GrammaticalRelation]>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        let mut start = BytesStart::new("w");

        // `formType`: `@a`/`@b`/… → XSD enum; `@z:label` → `user-special-form`.
        push_form_type_attrs(&mut start, word.form_type.as_ref());

        if let Some(cat) = &word.category {
            // `CAOmission` (`(parens)`) has no `type=` attribute — it
            // renders as a whole-word shortening (see
            // `emit_word_contents`), not as a `type="omission"` word.
            if let Some(attr) = word_category_attr(cat) {
                start.push_attribute(("type", attr));
            }
        }
        // `word.untranscribed()` is case-insensitive as a Stanza/MOR
        // correctness fix; the XML schema's `untranscribed` attribute
        // is case-sensitive and attaches only to the strictly
        // lowercase placeholders — gate on literal text here.
        if let Some(status) = untranscribed_attribute_for_xml(word) {
            start.push_attribute(("untranscribed", status));
        }

        self.writer.write_event(Event::Start(start))?;

        // `<langs>` child (if present) sits at the start of the
        // word, before any other content per the XSD sequence.
        if let Some(lang) = &word.lang {
            self.emit_langs(lang)?;
        }

        self.emit_word_contents(word)?;

        // Main-tier `$pos` suffix projects onto `<pos><c>tag</c></pos>`
        // as a word child per the XSD. Subcategory `<s>` children
        // aren't represented on the main-tier `Word` (that's %mor's
        // job) — we emit just `<c>`.
        if let Some(pos_tag) = &word.part_of_speech {
            self.writer
                .write_event(Event::Start(BytesStart::new("pos")))?;
            self.writer
                .write_event(Event::Start(BytesStart::new("c")))?;
            self.writer
                .write_event(Event::Text(escape_text(pos_tag.as_str())))?;
            self.writer.write_event(Event::End(BytesEnd::new("c")))?;
            self.writer.write_event(Event::End(BytesEnd::new("pos")))?;
        }

        if let Some(mor) = mor {
            self.emit_word_mor_subtree(mor, gra, chunk_index_1based)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("w")))?;
        Ok(())
    }

    /// Emit `<langs>` child for an `@s:code` marker. The schema
    /// requires a `<single>` / `<multiple>` / `<ambiguous>` child
    /// with at least one concrete language code, so bare `@s`
    /// (`Shortcut`) — which toggles between primary/secondary
    /// languages from the file's `@Languages` header without
    /// naming one — projects to no `<langs>` element. This is a
    /// known round-trip gap: re-reading the XML will lose the
    /// `@s`-vs-no-marker distinction. A future enhancement could
    /// resolve the toggle against `@Languages` context and emit
    /// `<single>` with the resolved code, at the cost of losing
    /// the "this was a shortcut" signal.
    fn emit_langs(
        &mut self,
        lang: &talkbank_model::model::WordLanguageMarker,
    ) -> Result<(), XmlWriteError> {
        use talkbank_model::model::WordLanguageMarker;
        match lang {
            // Bare `@s` toggles to the secondary `@Languages` entry.
            // Without a secondary (single-language file), the shortcut
            // has no target — Java skips the `<langs>` emission and
            // we match. With one, emit the same shape as
            // `Explicit(secondary)`.
            WordLanguageMarker::Shortcut => {
                let Some(code) = self.secondary_language.clone() else {
                    return Ok(());
                };
                self.emit_langs_single(code.as_str())
            }
            WordLanguageMarker::Explicit(code) => self.emit_langs_single(code.as_str()),
            WordLanguageMarker::Multiple(codes) => self.emit_langs_group("multiple", codes),
            WordLanguageMarker::Ambiguous(codes) => self.emit_langs_group("ambiguous", codes),
        }
    }

    fn emit_langs_single(&mut self, code: &str) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("langs")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("single")))?;
        self.writer.write_event(Event::Text(escape_text(code)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("single")))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("langs")))?;
        Ok(())
    }

    fn emit_langs_group(
        &mut self,
        child: &'static str,
        codes: &[talkbank_model::model::LanguageCode],
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("langs")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new(child)))?;
        let mut joined = String::new();
        for (i, code) in codes.iter().enumerate() {
            if i > 0 {
                joined.push(' ');
            }
            joined.push_str(code.as_str());
        }
        self.writer.write_event(Event::Text(escape_text(&joined)))?;
        self.writer.write_event(Event::End(BytesEnd::new(child)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("langs")))?;
        Ok(())
    }

    /// Walk the word's `content` vector and emit each segment inline
    /// inside the open `<w>`. Plain text becomes XML text; word-internal
    /// markers (compound `+`, clitic `~`, syllable pause `^`, lengthening
    /// `:`, stress `ˈ`/`ˌ`) become element siblings in the same order
    /// the CHAT source presents them.
    ///
    /// Mapping table (staged increments for the rest):
    ///
    /// | CHAT | Rust variant | XML |
    /// |------|--------------|-----|
    /// | text | [`WordContent::Text`] | (raw text) |
    /// | `+` (compound) | [`WordContent::CompoundMarker`] | `<wk type="cmp"/>` |
    /// | `~` (clitic) | [`WordContent::CliticBoundary`] | `<wk type="cli"/>` |
    /// | `^` (syllable pause) | [`WordContent::SyllablePause`] | `<p type="pause"/>` |
    /// | `:` (lengthening, count N) | [`WordContent::Lengthening`] | N × `<p type="drawl"/>` |
    /// | `ˈ` primary stress | [`WordContent::StressMarker`] (Primary) | `<ca-element type="primary stress"/>` |
    /// | `ˌ` secondary stress | [`WordContent::StressMarker`] (Secondary) | `<ca-element type="secondary stress"/>` |
    ///
    /// Other variants (`(text)` shortenings, CA delimiters/elements,
    /// overlap points, underline markers) fall through to
    /// `FeatureNotImplemented`. Each is a distinct TDD increment.
    fn emit_word_contents(&mut self, word: &Word) -> Result<(), XmlWriteError> {
        // CA delimiters come in pairs (`°…°`, `∆…∆`, …) but the AST
        // stores each `CADelimiter` occurrence on its own, without an
        // explicit begin/end marker. We recover begin/end by
        // toggling a bit per delimiter type during the walk: the
        // first occurrence of a given type sets the bit (`begin`),
        // the second clears it (`end`), and a doubled `°°` pair
        // opens and immediately closes again. The state is local to
        // one word — delimiters don't span word boundaries. A `u16`
        // bitset fits all 15 `CADelimiterType` variants without
        // allocating.
        let mut ca_delim_open: u16 = 0;

        // `CAOmission` (`(fullword)`) renders as a whole-word
        // shortening in the XML schema — `<w><shortening>text</shortening></w>`
        // rather than `<w type="omission">text</w>`. Open the wrapper
        // here and close it after the content loop so every emitted
        // segment lands inside `<shortening>`.
        let ca_omission = matches!(
            word.category,
            Some(talkbank_model::model::WordCategory::CAOmission)
        );
        if ca_omission {
            self.writer
                .write_event(Event::Start(BytesStart::new("shortening")))?;
        }

        // Leading OverlapPoint items were hoisted out as siblings
        // of `<w>` by `emit_leading_overlap_points`; skip them here
        // so they don't emit twice.
        let skip_count = word
            .content
            .iter()
            .take_while(|c| matches!(c, WordContent::OverlapPoint(_)))
            .count();

        for segment in word.content.iter().skip(skip_count) {
            match segment {
                WordContent::Text(text) => {
                    self.writer
                        .write_event(Event::Text(escape_text(text.0.as_str())))?;
                }
                WordContent::CompoundMarker(_) => {
                    let mut tag = BytesStart::new("wk");
                    tag.push_attribute(("type", "cmp"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::CliticBoundary(_) => {
                    let mut tag = BytesStart::new("wk");
                    tag.push_attribute(("type", "cli"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::SyllablePause(_) => {
                    let mut tag = BytesStart::new("p");
                    tag.push_attribute(("type", "pause"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::Lengthening(lengthening) => {
                    // `no::` is `count = 2` → emit two `<p type="drawl"/>`
                    // siblings. `count` is a small positive u8 from the
                    // grammar; zero would be a parser bug.
                    for _ in 0..lengthening.count {
                        let mut tag = BytesStart::new("p");
                        tag.push_attribute(("type", "drawl"));
                        self.writer.write_event(Event::Empty(tag))?;
                    }
                }
                WordContent::StressMarker(marker) => {
                    let type_attr = match marker.marker_type {
                        WordStressMarkerType::Primary => "primary stress",
                        WordStressMarkerType::Secondary => "secondary stress",
                    };
                    let mut tag = BytesStart::new("ca-element");
                    tag.push_attribute(("type", type_attr));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::Shortening(shortening) => {
                    // `(text)` inside a word — a shortened/omitted sound
                    // segment that Java Chatter preserves as an inline
                    // `<shortening>text</shortening>` wrapper around the
                    // unspoken characters.
                    self.writer
                        .write_event(Event::Start(BytesStart::new("shortening")))?;
                    self.writer
                        .write_event(Event::Text(escape_text(shortening.0.as_str())))?;
                    self.writer
                        .write_event(Event::End(BytesEnd::new("shortening")))?;
                }
                WordContent::CAElement(element) => {
                    let mut tag = BytesStart::new("ca-element");
                    tag.push_attribute(("type", ca_element_label(element)));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::OverlapPoint(point) => {
                    self.emit_overlap_point(point)?;
                }
                WordContent::CADelimiter(delimiter) => {
                    let bit = 1u16 << ca_delimiter_bit_index(delimiter.delimiter_type);
                    let was_open = (ca_delim_open & bit) != 0;
                    ca_delim_open ^= bit;
                    let type_attr = if was_open { "end" } else { "begin" };
                    let mut tag = BytesStart::new("ca-delimiter");
                    tag.push_attribute(("type", type_attr));
                    tag.push_attribute(("label", ca_delimiter_label(delimiter)));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::UnderlineBegin(_) => {
                    let mut tag = BytesStart::new("underline");
                    tag.push_attribute(("type", "begin"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                WordContent::UnderlineEnd(_) => {
                    let mut tag = BytesStart::new("underline");
                    tag.push_attribute(("type", "end"));
                    self.writer.write_event(Event::Empty(tag))?;
                }
            }
        }

        if ca_omission {
            self.writer
                .write_event(Event::End(BytesEnd::new("shortening")))?;
        }
        Ok(())
    }

    /// Emit `<t type="p|q|e"/>` for an utterance terminator. When the
    /// paired `%mor` tier carries a terminator chunk, nests
    /// `<mor><mt/><gra/></mor>` inside the `<t>` to match Java
    /// Chatter's output shape. Staged terminators (trailing-off,
    /// interruption, …) fail loud.
    pub(super) fn emit_terminator(
        &mut self,
        terminator: &Terminator,
        tiers: &UtteranceTiers<'_>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        // CA intonation-contour terminators (`⇗`, `↗`, `→`, `↘`,
        // `⇘`) were parsed by Rust as a single `Terminator` variant
        // but Java Chatter renders them as two sibling XML elements:
        // an `<s type="rising to high"/>` separator followed by the
        // `<t type="missing CA terminator"/>` placeholder. Peel off
        // the `<s>` here when the terminator carries pitch-contour
        // semantics — the `<t>` then collapses to the "missing"
        // placeholder via the same path as utterances with no
        // terminator at all.
        if let Some(s_label) = ca_terminator_separator_label(terminator) {
            let mut s_tag = BytesStart::new("s");
            s_tag.push_attribute(("type", s_label));
            self.writer.write_event(Event::Empty(s_tag))?;
        }

        // `<t type="…"/>`. Java Chatter uses the short letter code
        // for the three standard sentence terminators and a prose
        // phrase for CA-specific variants.
        let ty = terminator_type_attr(terminator);
        let mut start = BytesStart::new("t");
        start.push_attribute(("type", ty));

        // When %mor is present and carries a terminator chunk, Java
        // Chatter nests `<mor type="mor"><mt type="X"/><gra.../></mor>`
        // inside `<t>`, making it a non-empty element. Without %mor,
        // `<t>` stays empty (structural comparator folds `<t/>` vs
        // `<t></t>`).
        // MorTier always has a terminator, so presence-check
        // reduces to "is there a %mor tier at all."
        let has_mor_terminator = tiers.mor.is_some();
        if has_mor_terminator {
            self.writer.write_event(Event::Start(start))?;
            let mut mor = BytesStart::new("mor");
            mor.push_attribute(("type", "mor"));
            self.writer.write_event(Event::Start(mor))?;
            let mut mt = BytesStart::new("mt");
            mt.push_attribute(("type", ty));
            self.writer.write_event(Event::Empty(mt))?;
            if let Some(rel) = gra_entry(tiers.gra, chunk_index_1based) {
                self.emit_gra(rel)?;
            }
            self.writer.write_event(Event::End(BytesEnd::new("mor")))?;
            self.writer.write_event(Event::End(BytesEnd::new("t")))?;
        } else {
            self.writer.write_event(Event::Empty(start))?;
        }
        Ok(())
    }

    /// Emit a main-tier separator token. Java Chatter uses two XML
    /// element shapes here depending on the separator kind:
    ///
    /// - `<tagMarker type="…"/>` for structural separators (`,`,
    ///   `;`, `:`, `„`, `‡`). The tag-marker variants that
    ///   participate in `%mor` alignment (Comma, Tag, Vocative) get
    ///   a nested `<mor>` subtree when tiers are present.
    /// - `<s type="…"/>` for CA intonation / uptake / unmarked-ending
    ///   separators. These are empty elements — no `%mor` alignment.
    ///
    /// `CaContinuation` ([^c]) is a staged increment — its schema
    /// shape differs from both of the above and it doesn't appear in
    /// the reference corpus yet.
    pub(super) fn emit_separator(
        &mut self,
        sep: &Separator,
        mor: Option<&Mor>,
        gra: Option<&[GrammaticalRelation]>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        if let Some(s_type) = s_separator_label(sep) {
            let mut tag = BytesStart::new("s");
            tag.push_attribute(("type", s_type));
            self.writer.write_event(Event::Empty(tag))?;
            return Ok(());
        }

        // Every `Separator` variant routes to either `<s>` (above)
        // or `<tagMarker>` (here); this fallthrough only fires if a
        // new variant lands in the model without updating both
        // helpers. Fail loud rather than silently emit an invalid
        // attribute — the XSD rejects anything that isn't
        // `comma|tag|vocative`.
        let tag_type =
            separator_tag_type(sep).ok_or_else(|| XmlWriteError::FeatureNotImplemented {
                feature: format!("separator variant without XML mapping: {sep:?}"),
            })?;
        let mut start = BytesStart::new("tagMarker");
        start.push_attribute(("type", tag_type));

        match mor {
            Some(mor) => {
                self.writer.write_event(Event::Start(start))?;
                self.emit_word_mor_subtree(mor, gra, chunk_index_1based)?;
                self.writer
                    .write_event(Event::End(BytesEnd::new("tagMarker")))?;
            }
            None => {
                self.writer.write_event(Event::Empty(start))?;
            }
        }
        Ok(())
    }

    /// Emit `<pause symbolic-length="…" [length="…"]/>`. Symbolic
    /// pauses use one of the three XSD enum values; timed pauses
    /// add a numeric `length` attribute while keeping
    /// `symbolic-length="simple"` since that attribute is required
    /// by the schema even when the timing is explicit.
    pub(super) fn emit_pause(&mut self, pause: &Pause) -> Result<(), XmlWriteError> {
        let length_str;
        let (symbolic, numeric) = match &pause.duration {
            PauseDuration::Short => ("simple", None),
            PauseDuration::Medium => ("long", None),
            PauseDuration::Long => ("very long", None),
            PauseDuration::Timed(timed) => {
                use talkbank_model::model::PauseTimedDuration;
                length_str = match timed {
                    PauseTimedDuration::Parsed {
                        seconds, millis, ..
                    } => match millis {
                        // Emit canonical decimal form without
                        // trailing zeros: "(3.4)" round-trips to
                        // `length="3.4"`, not `"3.400"`. The model
                        // stores millis as a 0..999 value, so trim
                        // the 3-digit string representation from the
                        // right. `Some(0)` is equivalent to no
                        // millis and collapses to `seconds`.
                        Some(0) => seconds.to_string(),
                        Some(ms) => {
                            let padded = format!("{ms:03}");
                            let trimmed = padded.trim_end_matches('0');
                            format!("{seconds}.{trimmed}")
                        }
                        None => seconds.to_string(),
                    },
                    PauseTimedDuration::Unsupported(raw) => raw.as_str().to_owned(),
                };
                ("simple", Some(length_str.as_str()))
            }
        };
        let mut start = BytesStart::new("pause");
        start.push_attribute(("symbolic-length", symbolic));
        if let Some(n) = numeric {
            start.push_attribute(("length", n));
        }
        self.writer.write_event(Event::Empty(start))?;
        Ok(())
    }

    /// Emit `<g>…<k type="retracing"/>[annotation…]</g>` for a
    /// retrace. The inner content is walked with NO `%mor` alignment
    /// (retraced text is excluded from `%mor` by CHAT convention).
    /// Scoped annotations attached to the retrace (`[/] [= text]`,
    /// `[/?] [!]`, …) are emitted as sibling children of `<k>` inside
    /// the same `<g>` wrapper, using the same dispatch as
    /// `emit_annotated_word`.
    pub(super) fn emit_retrace(&mut self, retrace: &Retrace) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;
        for item in retrace.content.content.iter() {
            self.emit_bracketed_word_only(item)?;
        }
        // Scoped annotations emit before the retrace-kind `<k>` so the
        // retrace marker closes the group, matching `nay [?] [//]`.
        for annotation in retrace.annotations.iter() {
            self.emit_scoped_annotation(annotation)?;
        }
        let kind = retrace_kind_attr(retrace.kind);
        let mut k = BytesStart::new("k");
        k.push_attribute(("type", kind));
        self.writer.write_event(Event::Empty(k))?;
        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit `<g><w>word[<mor>...]</w><error/></g>` for a word carrying
    /// a single `[*]` error annotation. Richer annotation kinds — `[=]`,
    /// `[+]`, `[!]`, overlap markers etc. — each need their own
    /// increment.
    pub(super) fn emit_annotated_word(
        &mut self,
        annotated: &Annotated<Word>,
        mor: Option<&Mor>,
        gra: Option<&[GrammaticalRelation]>,
        chunk_index_1based: usize,
    ) -> Result<(), XmlWriteError> {
        // Java Chatter wraps every annotated word in `<g>…</g>` and
        // emits one child per scoped annotation after the `<w>`. We
        // reject unknown annotation kinds up front so the harness
        // reports the missing feature precisely.
        let annotations = annotated.scoped_annotations.as_slice();
        if annotations.is_empty() {
            return Err(XmlWriteError::MissingMetadata {
                what: "annotated word reached emitter without scoped annotations".to_owned(),
            });
        }

        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;
        self.emit_word(&annotated.inner, mor, gra, chunk_index_1based)?;

        for annotation in annotations {
            self.emit_scoped_annotation(annotation)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit the XML child element corresponding to a single
    /// scoped-annotation. Mapping per `talkbank.xsd`:
    ///
    /// | CHAT | Rust variant | XML |
    /// |------|--------------|-----|
    /// | `[*]` (no code) | [`ContentAnnotation::Error`] | `<error/>` |
    /// | `[*error_text]` | [`ContentAnnotation::Error`] with code | `<error>text</error>` |
    /// | `[= text]`      | [`ContentAnnotation::Explanation`] | `<ga type="explanation">text</ga>` |
    /// | `[% text]`      | [`ContentAnnotation::PercentComment`] | `<ga type="comments">text</ga>` |
    /// | `[=? text]`     | [`ContentAnnotation::Alternative`] | `<ga type="alternative">text</ga>` |
    /// | `[=! text]`     | [`ContentAnnotation::Paralinguistic`] | `<ga type="paralinguistics">text</ga>` |
    /// | `[!]`           | [`ContentAnnotation::Stressing`] | `<k type="stressing"/>` |
    /// | `[!!]`          | [`ContentAnnotation::ContrastiveStressing`] | `<k type="contrastive stressing"/>` |
    /// | `[?]`           | [`ContentAnnotation::Uncertain`] / `BestGuess` | `<k type="best guess"/>` |
    /// | `[e]`           | [`ContentAnnotation::Exclude`] | `<k type="mor exclude"/>` |
    /// | `[<N]` / `[>N]` | overlap | `<overlap type="…" index="N"/>` |
    fn emit_scoped_annotation(
        &mut self,
        annotation: &ContentAnnotation,
    ) -> Result<(), XmlWriteError> {
        match annotation {
            ContentAnnotation::Error(err) => {
                if let Some(code) = &err.code {
                    self.writer
                        .write_event(Event::Start(BytesStart::new("error")))?;
                    self.writer
                        .write_event(Event::Text(escape_text(code.as_str())))?;
                    self.writer
                        .write_event(Event::End(BytesEnd::new("error")))?;
                } else {
                    self.writer
                        .write_event(Event::Empty(BytesStart::new("error")))?;
                }
                Ok(())
            }
            ContentAnnotation::Explanation(expl) => self.emit_ga("explanation", expl.text.as_str()),
            ContentAnnotation::PercentComment(cmt) => self.emit_ga("comments", cmt.text.as_str()),
            ContentAnnotation::Alternative(alt) => self.emit_ga("alternative", alt.text.as_str()),
            ContentAnnotation::Paralinguistic(para) => {
                self.emit_ga("paralinguistics", para.text.as_str())
            }
            ContentAnnotation::Stressing => self.emit_k("stressing"),
            ContentAnnotation::ContrastiveStressing => self.emit_k("contrastive stressing"),
            ContentAnnotation::Uncertain => self.emit_k("best guess"),
            ContentAnnotation::Exclude => self.emit_k("mor exclude"),
            ContentAnnotation::OverlapBegin(begin) => {
                let mut tag = BytesStart::new("overlap");
                tag.push_attribute(("type", "overlap precedes"));
                if let Some(index) = &begin.index {
                    let s = index.to_string();
                    let mut t2 = tag;
                    t2.push_attribute(("index", s.as_str()));
                    self.writer.write_event(Event::Empty(t2))?;
                } else {
                    self.writer.write_event(Event::Empty(tag))?;
                }
                Ok(())
            }
            ContentAnnotation::OverlapEnd(end) => {
                let mut tag = BytesStart::new("overlap");
                tag.push_attribute(("type", "overlap follows"));
                if let Some(index) = &end.index {
                    let s = index.to_string();
                    let mut t2 = tag;
                    t2.push_attribute(("index", s.as_str()));
                    self.writer.write_event(Event::Empty(t2))?;
                } else {
                    self.writer.write_event(Event::Empty(tag))?;
                }
                Ok(())
            }
            ContentAnnotation::Unknown(unknown) => {
                // Lenient-parse fallback for unrecognised `[…]`
                // annotations. Preserve marker + text as a generic
                // `<ga type="comments">` so the payload survives
                // round-trip; the validator has already flagged the
                // shape.
                let payload = format!("{}{}", unknown.marker, unknown.text);
                self.emit_ga("comments", &payload)
            }
        }
    }

    fn emit_k(&mut self, ty: &'static str) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("k");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    fn emit_ga(&mut self, ty: &'static str, text: &str) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("ga");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Start(tag))?;
        self.writer.write_event(Event::Text(escape_text(text)))?;
        self.writer.write_event(Event::End(BytesEnd::new("ga")))?;
        Ok(())
    }

    /// Emit `<w>original<replacement><w>r1</w><w>r2</w>…</replacement></w>`.
    /// Each replacement word consumes one `%mor` / `%gra` chunk, so
    /// the caller tracks the cursor via the returned
    /// `chunks_consumed` value.
    ///
    /// `%mor` items align to the *replacement* words, not the
    /// original — that matches Java Chatter's output and the CHAT
    /// convention of aligning morphology to the intended form. A
    /// single-word replacement consumes one chunk; `dunno [: don't
    /// know]` consumes two.
    ///
    /// Returns the number of `%mor`/`%gra` chunks the replacement
    /// consumed. Callers in `emit_utterance` use this to advance
    /// their running cursor.
    ///
    /// Scoped annotations attached to the replaced word are a
    /// staged increment — `[: text] [= explanation]` and similar
    /// patterns require emitting both a `<replacement>` and
    /// annotation siblings inside a `<g>` wrapper, which isn't
    /// wired yet.
    pub(super) fn emit_replaced_word(
        &mut self,
        rw: &ReplacedWord,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        // `полетел [: полетела] [*]` — replacement + error — renders
        // as `<g><w>…<replacement>…</replacement></w><error/></g>`.
        // When scoped annotations are present, wrap the whole
        // replaced-word shape in `<g>` and emit each annotation as
        // a sibling after the outer `<w>` closes.
        let wrap_in_g = !rw.scoped_annotations.is_empty();
        if wrap_in_g {
            self.writer
                .write_event(Event::Start(BytesStart::new("g")))?;
        }

        // Outer <w> with the original spoken text, no mor subtree.
        // category / untranscribed on the original carry through so
        // `0word [: replacement]` still emits `type="omission"`.
        // (CAOmission intentionally omits the attribute — see
        // `word_category_attr`.)
        let mut outer = BytesStart::new("w");
        // The original spoken-side word's form_type projects to the
        // outer `<w>`; without it, `wɨspatsing@u [: whispering]` emits
        // `<w>` with no `formType` and loses phonological coding.
        push_form_type_attrs(&mut outer, rw.word.form_type.as_ref());
        if let Some(cat) = &rw.word.category
            && let Some(attr) = word_category_attr(cat)
        {
            outer.push_attribute(("type", attr));
        }
        self.writer.write_event(Event::Start(outer))?;
        // Emit the original word's structured content rather than
        // flattened `cleaned_text()` — preserves compound markers
        // (`<wk type="cmp"/>`), clitic boundaries, stress markers, and
        // other word-internal structure that the replacement's outer
        // `<w>` needs to round-trip through XML. Flat text would fuse
        // `rocking+house` into `rockinghouse`.
        self.emit_word_contents(&rw.word)?;

        self.writer
            .write_event(Event::Start(BytesStart::new("replacement")))?;
        for replacement_word in rw.replacement.words.0.iter() {
            // Each replacement word consumes one Mor item (with its
            // post-clitics inline) plus `1 + post_clitics.len()` `%gra`
            // edges, so a `dunno [: don't know's]`-style expansion
            // stays aligned even if a token carries a clitic chain.
            let mor_for_word = tiers
                .mor
                .as_ref()
                .and_then(|mor| mor.items().get(cursors.mor_index()));
            self.emit_word(
                replacement_word,
                mor_for_word,
                tiers.gra,
                cursors.gra_chunk(),
            )?;
            if tiers.mor.is_some() {
                let post_count = mor_for_word.map(|m| m.post_clitics.len()).unwrap_or(0);
                cursors.consume_mor(post_count);
            }
        }
        self.writer
            .write_event(Event::End(BytesEnd::new("replacement")))?;

        self.writer.write_event(Event::End(BytesEnd::new("w")))?;

        if wrap_in_g {
            for annotation in rw.scoped_annotations.iter() {
                self.emit_scoped_annotation(annotation)?;
            }
            self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        }

        Ok(())
    }

    /// Emit an inline `&=descriptor` event as `<e><happening>text</happening></e>`.
    /// `&=laughs`, `&=rire`, `&=coughs` and similar non-speech event
    /// markers sit in main-tier content outside the word alignment —
    /// they never consume `%mor` chunks.
    pub(super) fn emit_event(&mut self, event: &CEvent) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("happening")))?;
        self.writer
            .write_event(Event::Text(escape_text(event.event_type.as_str())))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("happening")))?;
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit an annotated bare-action token as `<e><action/></e>`. The
    /// `Annotated<Action>` wrapper carries scoped annotations in
    /// principle; currently only the empty-annotations case is wired
    /// because that is the only shape exercised by the reference
    /// corpus. Richer cases (`0 [= description]`) fail loud.
    pub(super) fn emit_annotated_action(
        &mut self,
        annotated: &Annotated<Action>,
    ) -> Result<(), XmlWriteError> {
        // `<e><action/>[annotation…]</e>`. Scoped annotations
        // attach inside `<e>` per the XSD sequence (same as
        // `emit_annotated_event`). The bare `0 .` case has no
        // annotations and renders as `<e><action/></e>`.
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        self.writer
            .write_event(Event::Empty(BytesStart::new("action")))?;
        for annotation in annotated.scoped_annotations.iter() {
            self.emit_scoped_annotation(annotation)?;
        }
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit `<linker type="…"/>` for a discourse linker (`+<`, `++`,
    /// `+≈`, `+≋`, …). Called once per item in
    /// `utterance.main.content.linkers` before any main-tier word is
    /// written. Staged variants (`+<` lazy-overlap, `++` completion)
    /// fail loud so each missing mapping shows up in the harness.
    pub(super) fn emit_linker(&mut self, linker: &Linker) -> Result<(), XmlWriteError> {
        let ty = match linker {
            Linker::QuotationFollows => "quoted utterance next",
            Linker::QuickUptakeOverlap => "quick uptake",
            Linker::LazyOverlapPrecedes => "lazy overlap mark",
            Linker::SelfCompletion => "self completion",
            Linker::OtherCompletion => "other completion",
            Linker::TcuContinuation => "technical break TCU completion",
            Linker::NoBreakTcuContinuation => "no break TCU completion",
        };
        let mut tag = BytesStart::new("linker");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    /// Hoist leading `OverlapPoint` items out of a word's content
    /// as top-level `<overlap-point/>` siblings. The Rust parser
    /// bundles markers at the start of a word (e.g. `⌈` in
    /// `⌈°overlapping+soft⌉°`) into `word.content`; Java Chatter
    /// keeps the *leading* ones outside the `<w>` element — only
    /// internal / trailing overlap points remain inside `<w>`. This
    /// method emits the leading prefix only; `emit_word_contents`
    /// knows to skip them when walking the word body.
    pub(super) fn emit_leading_overlap_points(&mut self, word: &Word) -> Result<(), XmlWriteError> {
        for item in word.content.iter() {
            match item {
                WordContent::OverlapPoint(point) => {
                    self.emit_overlap_point(point)?;
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Emit `<overlap-point start-end=… top-bottom=… [index=…]/>`.
    /// Shared between three callers: `emit_utterance` for top-level
    /// overlap markers (siblings of `<w>`), `emit_leading_overlap_points`
    /// for hoisted leading markers, and `emit_word_contents` for
    /// word-internal trailer markers. The XML shape is identical in
    /// all three contexts — only the enclosing element differs.
    /// Concurrent overlap regions carry an `index` attribute so
    /// downstream consumers can pair up begin/end markers; absent when
    /// the file uses only one overlap pair at a time.
    pub(super) fn emit_overlap_point(
        &mut self,
        point: &talkbank_model::model::OverlapPoint,
    ) -> Result<(), XmlWriteError> {
        let (start_end, top_bottom) = match point.kind {
            OverlapPointKind::TopOverlapBegin => ("start", "top"),
            OverlapPointKind::TopOverlapEnd => ("end", "top"),
            OverlapPointKind::BottomOverlapBegin => ("start", "bottom"),
            OverlapPointKind::BottomOverlapEnd => ("end", "bottom"),
        };
        let index_str = point.index.as_ref().map(|i| i.to_string());
        let mut tag = BytesStart::new("overlap-point");
        if let Some(s) = index_str.as_deref() {
            tag.push_attribute(("index", s));
        }
        tag.push_attribute(("start-end", start_end));
        tag.push_attribute(("top-bottom", top_bottom));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    /// Emit `<g><w>word1</w>…<annotation/></g>` for a group wrapped
    /// in scoped annotations — the main-tier shape behind
    /// `<lá em casa> [!]`. Each inner `BracketedItem::Word`
    /// consumes one `%mor` item (plus its post-clitic chain) from
    /// the caller-supplied cursors; the advance counts are
    /// returned so `emit_utterance` can thread the cursors forward.
    ///
    /// Staged increments: bracketed items beyond `Word` (Event,
    /// Pause, Separator, AnnotatedWord inside a group) fail loud.
    /// The reference corpus currently only exercises the
    /// `<word word word>` shape inside `AnnotatedGroup`.
    pub(super) fn emit_annotated_group(
        &mut self,
        annotated: &talkbank_model::model::Annotated<talkbank_model::model::Group>,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        let annotations = annotated.scoped_annotations.as_slice();
        if annotations.is_empty() {
            return Err(XmlWriteError::MissingMetadata {
                what: "annotated group reached emitter without scoped annotations".to_owned(),
            });
        }

        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;

        for item in annotated.inner.content.content.iter() {
            match item {
                BracketedItem::Word(word) => {
                    self.emit_aligned_word(word, tiers, cursors)?;
                }
                BracketedItem::AnnotatedGroup(inner) => {
                    // Nested `<…<inner words> [ann]…> [outer-ann]`
                    // recurses with the same cursor so mor alignment
                    // stays correct across the nesting.
                    self.emit_annotated_group(inner, tiers, cursors)?;
                }
                // Other bracketed items (separators, events,
                // actions, quotations, overlaps, underlines) delegate
                // to the generic per-item emitter. They don't consume
                // mor/gra chunks, so cursors don't advance here.
                other => self.emit_bracketed_word_only(other)?,
            }
        }

        for annotation in annotations {
            self.emit_scoped_annotation(annotation)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit one main-tier word that participates in `%mor` alignment
    /// per the `counts_for_tier` predicate. Words that don't count
    /// (untranscribed `xxx`, omissions, phonological fragments) still
    /// emit as `<w>` but without receiving a `%mor` child and without
    /// advancing the cursors. Shared between `emit_annotated_group`
    /// and `emit_bare_group` so both stay locked to the same predicate.
    fn emit_aligned_word(
        &mut self,
        word: &Word,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        let counts = counts_for_tier(word, TierDomain::Mor);
        let mor_for_word = if counts {
            tiers
                .mor
                .as_ref()
                .and_then(|mor| mor.items().get(cursors.mor_index()))
        } else {
            None
        };
        self.emit_word(word, mor_for_word, tiers.gra, cursors.gra_chunk())?;
        if counts && tiers.mor.is_some() {
            let post_count = mor_for_word.map(|m| m.post_clitics.len()).unwrap_or(0);
            cursors.consume_mor(post_count);
        }
        Ok(())
    }

    /// Emit a single bracketed-content item inside a retrace or
    /// group. Bare words are the common case; richer bracketed
    /// content (events, pauses, annotated words, replacements,
    /// separators) is emitted with a simple recursive dispatch.
    /// Retrace content is emitted without `%mor` alignment — retraced
    /// text is excluded from `%mor` by CHAT convention.
    fn emit_bracketed_word_only(&mut self, item: &BracketedItem) -> Result<(), XmlWriteError> {
        match item {
            BracketedItem::Word(word) => self.emit_word(word, None, None, 0),
            BracketedItem::AnnotatedWord(annotated) => {
                self.emit_annotated_word(annotated, None, None, 0)
            }
            BracketedItem::ReplacedWord(rw) => {
                // Precondition: caller has already opened a `<g>`
                // parent (retrace wrapper or annotated group). Scoped
                // annotations emit as flat siblings of the `<w>`
                // inside that parent — Java shape is
                // `<g><w>tika<replacement/>…</w><error/><k/></g>`.
                // Opening a nested `<g>` here would double-wrap.
                let mut outer = BytesStart::new("w");
                push_form_type_attrs(&mut outer, rw.word.form_type.as_ref());
                if let Some(cat) = &rw.word.category
                    && let Some(attr) = word_category_attr(cat)
                {
                    outer.push_attribute(("type", attr));
                }
                self.writer.write_event(Event::Start(outer))?;
                // Same reason as the top-level `emit_replaced_word`:
                // walk the structured content to preserve compound
                // markers and other word-internal structure on the
                // original side of `[: replacement]`.
                self.emit_word_contents(&rw.word)?;
                self.writer
                    .write_event(Event::Start(BytesStart::new("replacement")))?;
                for word in rw.replacement.words.0.iter() {
                    self.emit_word(word, None, None, 0)?;
                }
                self.writer
                    .write_event(Event::End(BytesEnd::new("replacement")))?;
                self.writer.write_event(Event::End(BytesEnd::new("w")))?;

                // Scoped annotations emit flat in the outer `<g>`.
                for annotation in rw.scoped_annotations.iter() {
                    self.emit_scoped_annotation(annotation)?;
                }
                Ok(())
            }
            BracketedItem::Event(event) => self.emit_event(event),
            BracketedItem::AnnotatedEvent(annotated) => self.emit_annotated_event(annotated),
            BracketedItem::Pause(pause) => self.emit_pause(pause),
            BracketedItem::Action(_) => {
                self.writer
                    .write_event(Event::Start(BytesStart::new("e")))?;
                self.writer
                    .write_event(Event::Empty(BytesStart::new("action")))?;
                self.writer.write_event(Event::End(BytesEnd::new("e")))?;
                Ok(())
            }
            BracketedItem::AnnotatedAction(annotated) => self.emit_annotated_action(annotated),
            BracketedItem::Quotation(quotation) => self.emit_quotation(quotation),
            BracketedItem::Freecode(freecode) => self.emit_freecode(freecode),
            BracketedItem::LongFeatureBegin(lf) => {
                self.emit_long_feature("begin", lf.label.as_str())
            }
            BracketedItem::LongFeatureEnd(lf) => self.emit_long_feature("end", lf.label.as_str()),
            BracketedItem::NonvocalBegin(nv) => self.emit_nonvocal("begin", nv.label.as_str()),
            BracketedItem::NonvocalEnd(nv) => self.emit_nonvocal("end", nv.label.as_str()),
            BracketedItem::NonvocalSimple(nv) => self.emit_nonvocal("simple", nv.label.as_str()),
            BracketedItem::Separator(sep) => self.emit_separator(sep, None, None, 0),
            BracketedItem::OverlapPoint(point) => self.emit_overlap_point(point),
            BracketedItem::InternalBullet(bullet) => self.emit_internal_media(bullet),
            BracketedItem::UnderlineBegin(_) => {
                let mut tag = BytesStart::new("underline");
                tag.push_attribute(("type", "begin"));
                self.writer.write_event(Event::Empty(tag))?;
                Ok(())
            }
            BracketedItem::UnderlineEnd(_) => {
                let mut tag = BytesStart::new("underline");
                tag.push_attribute(("type", "end"));
                self.writer.write_event(Event::Empty(tag))?;
                Ok(())
            }
            BracketedItem::OtherSpokenEvent(event) => self.emit_other_spoken_event(event),
            BracketedItem::Retrace(retrace) => {
                // Nested retrace inside a group/retrace: emit
                // without cursor threading (retrace content is
                // excluded from `%mor`).
                self.emit_retrace(retrace)
            }
            BracketedItem::AnnotatedGroup(inner) => {
                // Retrace / bracketed context disables mor alignment,
                // so synthesise empty tiers and emit the group using
                // the normal path. Cursor advances are discarded —
                // retraced content is excluded from `%mor` / `%gra`
                // by CHAT convention.
                let empty = UtteranceTiers {
                    mor: None,
                    gra: None,
                    wor: None,
                    sin: None,
                    side_tiers: Vec::new(),
                };
                let mut scratch = TierCursors::new();
                self.emit_annotated_group(inner, &empty, &mut scratch)
            }
            BracketedItem::PhoGroup(_) | BracketedItem::SinGroup(_) => {
                // Phon-specific payloads share the permanent
                // out-of-scope policy with `%pho` / `%mod`.
                Err(XmlWriteError::PhoneticTierUnsupported {
                    utterance_index: usize::MAX,
                })
            }
        }
    }

    /// Emit `<internal-media start="…" end="…" unit="s"/>` for a
    /// standalone bullet encountered inside a retrace or group.
    /// Shares the seconds formatting with the `%wor` emitter.
    pub(super) fn emit_internal_media(
        &mut self,
        bullet: &talkbank_model::model::Bullet,
    ) -> Result<(), XmlWriteError> {
        let start = super::wor::format_seconds(bullet.timing.start_ms);
        let end = super::wor::format_seconds(bullet.timing.end_ms);
        let mut tag = BytesStart::new("internal-media");
        tag.push_attribute(("start", start.as_str()));
        tag.push_attribute(("end", end.as_str()));
        tag.push_attribute(("unit", "s"));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }

    /// Emit `<e><happening>text</happening>[annotation…]</e>` for an
    /// annotated event like `&=laughs [!]`. Annotations attach
    /// *inside* `<e>` per the XSD sequence.
    pub(super) fn emit_annotated_event(
        &mut self,
        annotated: &Annotated<CEvent>,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        self.writer
            .write_event(Event::Start(BytesStart::new("happening")))?;
        self.writer.write_event(Event::Text(escape_text(
            annotated.inner.event_type.as_str(),
        )))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("happening")))?;
        for annotation in annotated.scoped_annotations.iter() {
            self.emit_scoped_annotation(annotation)?;
        }
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit `<g>` for a bare (unannotated) `Group`. Each inner Word
    /// consumes one `%mor` chunk, returned as a cursor advance.
    pub(super) fn emit_bare_group(
        &mut self,
        group: &talkbank_model::model::Group,
        tiers: &UtteranceTiers<'_>,
        cursors: &mut TierCursors,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("g")))?;
        for item in group.content.content.iter() {
            if let BracketedItem::Word(word) = item {
                self.emit_aligned_word(word, tiers, cursors)?;
            } else {
                self.emit_bracketed_word_only(item)?;
            }
        }
        self.writer.write_event(Event::End(BytesEnd::new("g")))?;
        Ok(())
    }

    /// Emit `<quotation type="begin"/>…<quotation type="end"/>` for
    /// a quotation span. The Rust model holds the whole quoted
    /// content as one `Quotation`; the XSD represents it as two
    /// standalone markers with content between them.
    pub(super) fn emit_quotation(
        &mut self,
        quotation: &talkbank_model::model::Quotation,
    ) -> Result<(), XmlWriteError> {
        let mut begin = BytesStart::new("quotation");
        begin.push_attribute(("type", "begin"));
        self.writer.write_event(Event::Empty(begin))?;
        for item in quotation.content.content.iter() {
            self.emit_bracketed_word_only(item)?;
        }
        let mut end = BytesStart::new("quotation");
        end.push_attribute(("type", "end"));
        self.writer.write_event(Event::Empty(end))?;
        Ok(())
    }

    /// Emit `<freecode>text</freecode>`.
    pub(super) fn emit_freecode(
        &mut self,
        freecode: &talkbank_model::model::Freecode,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("freecode")))?;
        self.writer
            .write_event(Event::Text(escape_text(freecode.text.as_str())))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("freecode")))?;
        Ok(())
    }

    /// Emit `<long-feature type="begin|end">label</long-feature>`.
    pub(super) fn emit_long_feature(
        &mut self,
        ty: &'static str,
        label: &str,
    ) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("long-feature");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Start(tag))?;
        self.writer.write_event(Event::Text(escape_text(label)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("long-feature")))?;
        Ok(())
    }

    /// Emit `<e><otherSpokenEvent who=".." said=".."/></e>` for a
    /// `&*SPEAKER:text` interposed-speaker event. The `<e>` wrapper
    /// is required by the XSD (it holds the `<action>` / `<happening>`
    /// / `<otherSpokenEvent>` choice).
    pub(super) fn emit_other_spoken_event(
        &mut self,
        event: &talkbank_model::model::OtherSpokenEvent,
    ) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("e")))?;
        let mut inner = BytesStart::new("otherSpokenEvent");
        inner.push_attribute(("who", event.speaker.as_str()));
        inner.push_attribute(("said", event.text.as_str()));
        self.writer.write_event(Event::Empty(inner))?;
        self.writer.write_event(Event::End(BytesEnd::new("e")))?;
        Ok(())
    }

    /// Emit `<nonvocal type="begin|end|simple">label</nonvocal>`.
    pub(super) fn emit_nonvocal(
        &mut self,
        ty: &'static str,
        label: &str,
    ) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("nonvocal");
        tag.push_attribute(("type", ty));
        self.writer.write_event(Event::Start(tag))?;
        self.writer.write_event(Event::Text(escape_text(label)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("nonvocal")))?;
        Ok(())
    }
}

/// Map the three [`Separator`] variants that render as `<tagMarker
/// type="…"/>` per the XSD: `comma`, `tag`, `vocative`. These are
/// the separators that participate in `%mor` alignment. All other
/// separator variants render as `<s type="…"/>` via
/// [`s_separator_label`].
fn separator_tag_type(sep: &Separator) -> Option<&'static str> {
    Some(match sep {
        Separator::Comma { .. } => "comma",
        Separator::Tag { .. } => "tag",
        Separator::Vocative { .. } => "vocative",
        _ => return None,
    })
}

/// Recognise CA intonation-contour terminators that Java Chatter
/// splits into a preceding `<s type="…"/>` plus `<t type="missing CA
/// terminator"/>`. Returns the `<s>` label for pitch-contour
/// terminators (`CaRisingToHigh`, `CaFallingToLow`, …). Other
/// terminator variants render as a single `<t>`.
fn ca_terminator_separator_label(terminator: &Terminator) -> Option<&'static str> {
    Some(match terminator {
        Terminator::CaRisingToHigh { .. } => "rising to high",
        Terminator::CaRisingToMid { .. } => "rising to mid",
        Terminator::CaLevel { .. } => "level",
        Terminator::CaFallingToMid { .. } => "falling to mid",
        Terminator::CaFallingToLow { .. } => "falling to low",
        _ => return None,
    })
}

/// Map every [`Terminator`] variant to its `<t type="…"/>` attribute
/// value per `baseTerminatorType` in `talkbank.xsd`. The CA intonation
/// contours (`⇗`/`↗`/`→`/`↘`/`⇘`) collapse to
/// `"missing CA terminator"`; the `<s>` preamble they also carry is
/// emitted separately via [`ca_terminator_separator_label`]. CA
/// linker variants (`CaNoBreakLinker`, `CaTechnicalBreakLinker`)
/// render identically to their non-linker counterparts — the linker
/// role is expressed on the *next* utterance's `<linker>`, not on
/// this `<t>`.
pub(super) fn terminator_type_attr(terminator: &Terminator) -> &'static str {
    match terminator {
        Terminator::Period { .. } => "p",
        Terminator::Question { .. } => "q",
        Terminator::Exclamation { .. } => "e",
        Terminator::TrailingOff { .. } => "trail off",
        Terminator::TrailingOffQuestion { .. } => "trail off question",
        Terminator::BrokenQuestion { .. } => "question exclamation",
        Terminator::Interruption { .. } => "interruption",
        Terminator::InterruptedQuestion { .. } => "interruption question",
        Terminator::SelfInterruption { .. } => "self interruption",
        Terminator::SelfInterruptedQuestion { .. } => "self interruption question",
        Terminator::QuotedNewLine { .. } => "quotation next line",
        Terminator::QuotedPeriodSimple { .. } => "quotation precedes",
        Terminator::BreakForCoding { .. } => "broken for coding",
        Terminator::CaNoBreak { .. } | Terminator::CaNoBreakLinker { .. } => {
            "no break TCU continuation"
        }
        Terminator::CaTechnicalBreak { .. } | Terminator::CaTechnicalBreakLinker { .. } => {
            "technical break TCU continuation"
        }
        Terminator::CaRisingToHigh { .. }
        | Terminator::CaRisingToMid { .. }
        | Terminator::CaLevel { .. }
        | Terminator::CaFallingToMid { .. }
        | Terminator::CaFallingToLow { .. } => "missing CA terminator",
    }
}

/// Map the [`Separator`] variants that render as `<s type="…"/>`
/// per `talkbank.xsd`. Covers semicolon/colon (structural),
/// CA intonation contours, uptake, unmarked ending, and
/// CaContinuation (`[^c]`, best-matched to `clause delimiter`).
fn s_separator_label(sep: &Separator) -> Option<&'static str> {
    Some(match sep {
        Separator::Semicolon { .. } => "semicolon",
        Separator::Colon { .. } => "colon",
        Separator::CaContinuation { .. } => "clause delimiter",
        Separator::RisingToHigh { .. } => "rising to high",
        Separator::RisingToMid { .. } => "rising to mid",
        Separator::Level { .. } => "level",
        Separator::FallingToMid { .. } => "falling to mid",
        Separator::FallingToLow { .. } => "falling to low",
        Separator::UnmarkedEnding { .. } => "unmarked ending",
        Separator::Uptake { .. } => "uptake",
        _ => return None,
    })
}

/// Map [`talkbank_model::model::WordCategory`] to the `<w type="...">`
/// attribute value used by Java Chatter. Returns `None` for
/// [`WordCategory::CAOmission`] — `(parens)` is not an omission in
/// the schema sense; it renders as an all-content `<shortening>`
/// wrapper instead. See `emit_word_contents`.
fn word_category_attr(cat: &talkbank_model::model::WordCategory) -> Option<&'static str> {
    use talkbank_model::model::WordCategory;
    match cat {
        WordCategory::Omission => Some("omission"),
        WordCategory::CAOmission => None,
        WordCategory::Nonword => Some("nonword"),
        WordCategory::Filler => Some("filler"),
        WordCategory::PhonologicalFragment => Some("fragment"),
    }
}

/// Map [`talkbank_model::model::content::word::UntranscribedStatus`]
/// to the `<w untranscribed="...">` attribute value. Enum values
/// follow the XSD `<xs:enumeration>` on the `untranscribed`
/// attribute of `<w>`:
///
/// | CHAT | UntranscribedStatus | XML value |
/// |------|---------------------|-----------|
/// | `xxx` | `Unintelligible` | `"unintelligible"` |
/// | `yyy` | `Phonetic` | `"unintelligible-with-pho"` |
/// | `www` | `Untranscribed` | `"untranscribed"` |
fn untranscribed_attr(
    status: talkbank_model::model::content::word::UntranscribedStatus,
) -> &'static str {
    use talkbank_model::model::content::word::UntranscribedStatus;
    match status {
        UntranscribedStatus::Unintelligible => "unintelligible",
        UntranscribedStatus::Phonetic => "unintelligible-with-pho",
        UntranscribedStatus::Untranscribed => "untranscribed",
    }
}

/// Map a [`FormType`] to its `<w formType="…"/>` attribute value per
/// `talkbank.xsd`. Returns `None` for `FormType::UserDefined` — that
/// variant projects onto `user-special-form` instead and is handled
/// by the caller. The mapping is the CHAT `@-marker` → schema label
/// correspondence (e.g. `@b` → `"babbling"`, `@sas` → `"sign speech"`).
/// Apply the word-level `formType` / `user-special-form` attribute to
/// an open `<w>` element. Called from every place that emits an outer
/// `<w>` — plain `emit_word`, `emit_replaced_word` (top-level), and the
/// retrace-wrapped replaced-word path in `emit_bracketed_word_only`.
///
/// `FormType::UserDefined(String)` owns its label, so `code.as_str()`
/// can be passed through without cloning — the string lives as long as
/// `word.form_type` does, which is longer than `start.push_attribute`.
fn push_form_type_attrs(start: &mut BytesStart<'_>, form_type: Option<&FormType>) {
    let Some(form) = form_type else { return };
    if let Some(attr_value) = form_type_attr(form) {
        start.push_attribute(("formType", attr_value));
    } else if let FormType::UserDefined(code) = form {
        start.push_attribute(("user-special-form", code.as_str()));
    }
}

fn form_type_attr(form: &FormType) -> Option<&'static str> {
    Some(match form {
        // `@a` has no XSD enum of its own — doc string in Rust calls
        // it "approximate / phonologically consistent", matching the
        // XSD `"phonology consistent"` semantics. Collapse to that.
        FormType::A | FormType::P => "phonology consistent",
        FormType::B => "babbling",
        FormType::C => "child-invented",
        FormType::D => "dialect",
        FormType::F => "family-specific",
        FormType::FP => "filled pause",
        FormType::G => "generic",
        FormType::I => "interjection",
        FormType::K => "kana",
        FormType::L => "letter",
        FormType::LS => "letter plural",
        FormType::N => "neologism",
        FormType::O => "onomatopoeia",
        FormType::Q => "quoted metareference",
        FormType::SAS => "sign speech",
        FormType::SI => "singing",
        FormType::SL => "signed language",
        FormType::T => "test",
        FormType::U => "UNIBET",
        FormType::WP => "word play",
        FormType::X => "words to be excluded",
        FormType::UserDefined(_) => return None,
    })
}

/// CHAT-spec-strict check for the `untranscribed` XML attribute: only
/// the lowercase placeholders `xxx`, `yyy`, `www` trigger it. Bypasses
/// the model's case-insensitive `untranscribed()` helper (which is a
/// Stanza/MOR workaround, not the XML schema rule).
fn untranscribed_attribute_for_xml(word: &Word) -> Option<&'static str> {
    use talkbank_model::model::content::word::UntranscribedStatus;
    match word.cleaned_text() {
        "xxx" => Some(untranscribed_attr(UntranscribedStatus::Unintelligible)),
        "yyy" => Some(untranscribed_attr(UntranscribedStatus::Phonetic)),
        "www" => Some(untranscribed_attr(UntranscribedStatus::Untranscribed)),
        _ => None,
    }
}

/// Map a [`RetraceKind`] to the `<k type="…"/>` attribute value per
/// the `<k>` XSD enum. The enum variant names and the CHAT manual
/// names for each marker don't line up (historical drift in the
/// Rust model); the CHAT→XSD mapping below follows the XSD's own
/// documentation comments rather than the variant names:
///
/// | CHAT notation | Model variant | XSD `<k type=>` value |
/// |---|---|---|
/// | `[/]` | `Partial` | `retracing` |
/// | `[//]` | `Full` | `retracing with correction` |
/// | `[///]` | `Multiple` | `retracing reformulation` |
/// | `[/-]` | `Reformulation` | `false start` |
fn retrace_kind_attr(kind: RetraceKind) -> &'static str {
    match kind {
        RetraceKind::Partial => "retracing",
        RetraceKind::Full => "retracing with correction",
        RetraceKind::Multiple => "retracing reformulation",
        RetraceKind::Reformulation => "false start",
    }
}

/// Short display name for a [`ContentAnnotation`] variant. Used
/// inside `FeatureNotImplemented` diagnostics so the harness surfaces
/// each staged annotation kind as a distinct increment.
/// Map a [`CAElementType`] to the `<ca-element type="…"/>` attribute
/// value used by Java Chatter. Values are lowercase with a space
/// separator where the Rust enum uses CamelCase (e.g. `PitchUp` →
/// `"pitch up"`, `BlockedSegments` → `"blocked segments"`).
fn ca_element_label(element: &CAElement) -> &'static str {
    match element.element_type {
        CAElementType::BlockedSegments => "blocked segments",
        CAElementType::Constriction => "constriction",
        CAElementType::Hardening => "hardening",
        CAElementType::HurriedStart => "hurried start",
        CAElementType::Inhalation => "inhalation",
        CAElementType::LaughInWord => "laugh in word",
        CAElementType::PitchDown => "pitch down",
        CAElementType::PitchReset => "pitch reset",
        CAElementType::PitchUp => "pitch up",
        CAElementType::SuddenStop => "sudden stop",
    }
}

/// Assign a unique `[0, 15)` bit index to each [`CADelimiterType`]
/// variant so `emit_word_contents` can track begin/end state in a
/// `u16` bitset instead of a per-word `HashMap` allocation.
fn ca_delimiter_bit_index(ty: CADelimiterType) -> u8 {
    match ty {
        CADelimiterType::Faster => 0,
        CADelimiterType::Slower => 1,
        CADelimiterType::Softer => 2,
        CADelimiterType::Louder => 3,
        CADelimiterType::LowPitch => 4,
        CADelimiterType::HighPitch => 5,
        CADelimiterType::SmileVoice => 6,
        CADelimiterType::BreathyVoice => 7,
        CADelimiterType::Unsure => 8,
        CADelimiterType::Whisper => 9,
        CADelimiterType::Yawn => 10,
        CADelimiterType::Singing => 11,
        CADelimiterType::SegmentRepetition => 12,
        CADelimiterType::Creaky => 13,
        CADelimiterType::Precise => 14,
    }
}

/// Map a [`CADelimiter`] to the `<ca-delimiter label="…"/>` attribute
/// value Java Chatter emits. Labels are not a mechanical
/// transformation of the variant name — the pitch pair uses hyphens
/// (`"low-pitch"`), most two-word variants use spaces
/// (`"smile voice"`, `"breathy voice"`), and `SegmentRepetition`
/// renames to `"repeated-segment"`. Kept as an explicit table so the
/// mapping is visible.
fn ca_delimiter_label(delimiter: &CADelimiter) -> &'static str {
    match delimiter.delimiter_type {
        CADelimiterType::Faster => "faster",
        CADelimiterType::Slower => "slower",
        CADelimiterType::Softer => "softer",
        CADelimiterType::Louder => "louder",
        CADelimiterType::LowPitch => "low-pitch",
        CADelimiterType::HighPitch => "high-pitch",
        CADelimiterType::SmileVoice => "smile voice",
        CADelimiterType::BreathyVoice => "breathy voice",
        CADelimiterType::Unsure => "unsure",
        CADelimiterType::Whisper => "whisper",
        CADelimiterType::Yawn => "yawn",
        CADelimiterType::Singing => "singing",
        CADelimiterType::SegmentRepetition => "repeated-segment",
        CADelimiterType::Creaky => "creaky",
        CADelimiterType::Precise => "precise",
    }
}

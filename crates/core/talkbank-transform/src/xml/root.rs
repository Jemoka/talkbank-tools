//! Document-level emission — the `<CHAT>` root, `<Participants>`,
//! body-level headers and comments, and utterance orchestration.
//!
//! This file owns the "top-down" traversal of a `ChatFile`. Word-level
//! emission delegates to `super::word`; morphology subtrees delegate
//! to `super::mor`. Metadata helpers (corpus lookup, date/age/sex
//! formatting, `@Options` flags, per-speaker extras from body-level
//! `@Birthplace` / `@L1` headers) also live here because they feed
//! attributes on `<CHAT>` and `<participant>`.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use std::collections::HashMap;

use talkbank_model::alignment::TierDomain;
use talkbank_model::alignment::helpers::{counts_for_tier, is_tag_marker_separator};
use talkbank_model::model::{
    AgeValue, BulletContent, ChatDate, ChatFile, ChatOptionFlag, Header, Line, Month, Sex, SinItem,
    SpeakerCode, UtteranceContent,
};
use talkbank_model::validation::ValidationState;

use super::error::XmlWriteError;
use super::mor::{TierCursors, collect_utterance_tiers};
use super::writer::{
    SCHEMA_LOCATION, SCHEMA_VERSION, TALKBANK_NS, XSI_NS, XmlEmitter, escape_text,
};

impl XmlEmitter {
    /// Serialize the full document: XML decl, `<CHAT>` root with its
    /// attributes, `<Participants>`, and the body. This is the single
    /// entry point invoked by the public `write_chat_xml` wrapper.
    pub(super) fn emit_document<S: ValidationState>(
        &mut self,
        file: &ChatFile<S>,
    ) -> Result<(), XmlWriteError> {
        // <?xml version="1.0" encoding="UTF-8"?>
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        let corpus = find_corpus(file)?;

        let mut root = BytesStart::new("CHAT");
        root.push_attribute(("xmlns:xsi", XSI_NS));
        root.push_attribute(("xmlns", TALKBANK_NS));
        root.push_attribute(("xsi:schemaLocation", SCHEMA_LOCATION));
        // `@Media` comes before `Version` in Java Chatter's output on
        // audio/video files. The structural comparator ignores order,
        // but we keep Java's ordering to minimize diff noise during
        // development.
        if let Some(media) = file.media.as_deref() {
            // CHAT `@Media:` values can be bare filenames or
            // double-quoted URLs (`"https://…"`). The XSD
            // `mediaRefType` is `xs:anyURI`, which rejects the
            // embedded quotes — strip them at the emission
            // boundary so the attribute is schema-legal.
            let raw = media.filename.as_str();
            let stripped = raw
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(raw);
            root.push_attribute(("Media", stripped));
            // `Mediatypes` is a space-separated list per the XSD's
            // `mediaTypesType` enumeration
            // ({audio|video|unlinked|missing|notrans}), so emit the
            // media type and any status in the same attribute rather
            // than splitting them.
            let mediatypes = match &media.status {
                Some(status) => format!("{} {}", media.media_type.as_str(), status.as_str()),
                None => media.media_type.as_str().to_owned(),
            };
            root.push_attribute(("Mediatypes", mediatypes.as_str()));
        }
        root.push_attribute(("Version", SCHEMA_VERSION));
        root.push_attribute(("Lang", join_language_codes(file).as_str()));
        if let Some(options) = format_options_attribute(file) {
            root.push_attribute(("Options", options.as_str()));
        }
        root.push_attribute(("Corpus", corpus.as_str()));
        if let Some(pid) = find_root_pid(file) {
            root.push_attribute(("PID", pid));
        }
        if let Some(date) = find_root_date(file)? {
            root.push_attribute(("Date", date.as_str()));
        }
        // `@Types: design, activity, group` projects onto three root
        // attributes in addition to the body-level `<comment
        // type="Types">` (which is emitted during the body walk).
        // Java Chatter names these
        // `DesignType` / `ActivityType` / `GroupType`.
        if let Some(types) = find_root_types(file) {
            root.push_attribute(("DesignType", types.design.as_str()));
            root.push_attribute(("ActivityType", types.activity.as_str()));
            root.push_attribute(("GroupType", types.group.as_str()));
        }
        // Pre-Begin UI-state headers — `@Color words`, `@Font`,
        // `@Window` — round-trip as root attributes. Documented in
        // the CHAT manual even though their origin is CLAN editor
        // state. Collected in a single pass so we don't scan
        // `file.lines` three times.
        let ui = find_root_ui_attrs(file);
        if let Some(colors) = ui.colors {
            root.push_attribute(("Colorwords", colors));
        }
        if let Some(font) = ui.font {
            root.push_attribute(("Font", font));
        }
        if let Some(window) = ui.window {
            root.push_attribute(("Window", window));
        }
        self.writer.write_event(Event::Start(root))?;

        self.emit_participants(file)?;
        self.emit_body(file)?;

        self.writer.write_event(Event::End(BytesEnd::new("CHAT")))?;
        Ok(())
    }

    /// Emit the `<Participants>` block. Pulls speaker metadata from
    /// the `@Participants` / `@ID` headers plus body-level
    /// `@Birthplace` / `@L1` extras collected via
    /// [`collect_per_speaker_metadata`].
    fn emit_participants<S: ValidationState>(
        &mut self,
        file: &ChatFile<S>,
    ) -> Result<(), XmlWriteError> {
        // Pre-scan body headers that attach extra metadata to an existing
        // participant (`@Birthplace of X`, `@L1 of X`). These live outside
        // Participant itself because they are independent CHAT headers.
        let extra = collect_per_speaker_metadata(file);

        self.writer
            .write_event(Event::Start(BytesStart::new("Participants")))?;

        for participant in file.participants.values() {
            // Attribute order below matches Java Chatter's output to make
            // manual diffing against the golden easier; the structural
            // comparator treats order as insignificant.
            let mut start = BytesStart::new("participant");
            start.push_attribute(("id", participant.code.as_str()));
            start.push_attribute(("role", participant.role.as_ref()));

            let lang = join_codes_with_space(&participant.id.language.0);
            start.push_attribute(("language", lang.as_str()));

            if let Some(age) = &participant.id.age {
                let iso = format_age_iso8601(age)?;
                start.push_attribute(("age", iso.as_str()));
            }
            if let Some(sex) = &participant.id.sex {
                start.push_attribute(("sex", sex_to_xml(sex)?));
            }
            if let Some(group) = &participant.id.group {
                start.push_attribute(("group", group.as_str()));
            }
            if let Some(name) = &participant.name {
                start.push_attribute(("name", name.as_str()));
            }
            // `SES` attribute (uppercase, per Java golden). `SesValue::as_str`
            // serializes `SesOnly(UC)` as `"UC"` and `Combined { eth, ses }`
            // as `"White,MC"`, matching Java Chatter's comma-joined form.
            let ses_rendered;
            if let Some(ses) = &participant.id.ses {
                ses_rendered = ses.as_str();
                start.push_attribute(("SES", ses_rendered.as_str()));
            }
            if let Some(education) = &participant.id.education {
                start.push_attribute(("education", education.as_str()));
            }
            if let Some(birth_date) = &participant.birth_date {
                let iso = format_chat_date_iso(birth_date)?;
                start.push_attribute(("birthday", iso.as_str()));
            }
            if let Some(meta) = extra.get(&participant.code) {
                if let Some(place) = &meta.birthplace {
                    start.push_attribute(("birthplace", place.as_str()));
                }
                if let Some(lang1) = &meta.first_language {
                    start.push_attribute(("first-language", lang1.as_str()));
                }
            }
            if let Some(custom) = &participant.id.custom_field {
                start.push_attribute(("custom-field", custom.as_str()));
            }

            self.writer.write_event(Event::Empty(start))?;
        }

        self.writer
            .write_event(Event::End(BytesEnd::new("Participants")))?;
        Ok(())
    }

    /// Walk `file.lines` in order, dispatching each line to either a
    /// body-level header emitter or an utterance emitter. Root-level
    /// headers (`@Begin`, `@Languages`, `@ID`, …) that have already
    /// contributed attributes on `<CHAT>` are filtered out inside
    /// [`Self::emit_header_if_body`].
    fn emit_body<S: ValidationState>(&mut self, file: &ChatFile<S>) -> Result<(), XmlWriteError> {
        for line in file.lines.iter() {
            match line {
                Line::Header { header, .. } => self.emit_header_if_body(header)?,
                Line::Utterance(utterance) => self.emit_utterance(utterance)?,
            }
        }
        Ok(())
    }

    /// Most headers contribute to the root element or the Participants
    /// block and have already been consumed by `emit_document`; only
    /// body-level headers emit their own XML element. Stage 1 handles
    /// `@Comment`; all other body-level headers (`@Bg`/`@Eg`, `@G`,
    /// `@Media`, `@Situation`, `@Date`, `@Pid`, `@Types`, pre-begin
    /// headers, warnings, etc.) report `FeatureNotImplemented`.
    fn emit_header_if_body(&mut self, header: &Header) -> Result<(), XmlWriteError> {
        match header {
            // Scaffold + root-attribute + per-speaker metadata headers
            // already consumed by `emit_document` / `emit_participants`.
            Header::Utf8
            | Header::Begin
            | Header::End
            | Header::Languages { .. }
            | Header::Participants { .. }
            | Header::ID(_)
            | Header::Birth { .. }
            | Header::Birthplace { .. }
            | Header::L1Of { .. }
            | Header::Options { .. }
            | Header::Media(_)
            | Header::Pid { .. } => Ok(()),

            // @Bg/@Eg/@G gems render as standalone XML elements at
            // the same level as `<u>`, not as `<comment>` children.
            // The `label` attribute is required in the XSD; when CHAT
            // source omits it, emit an empty string to stay
            // schema-valid.
            Header::BeginGem { label } => self.emit_gem("begin-gem", gem_label(label)),
            Header::EndGem { label } => self.emit_gem("end-gem", gem_label(label)),
            Header::LazyGem { label } => self.emit_gem("lazy-gem", gem_label(label)),

            // @Date appears twice: once as a root `Date="YYYY-MM-DD"`
            // attribute and once as a `<comment type="Date">DD-MMM-
            // YYYY</comment>` preserving the original CHAT text.
            Header::Date { date } => self.emit_typed_comment("Date", date.as_str()),

            Header::Comment { content } => self.emit_bullet_content_comment("Generic", content),
            Header::Location { location } => self.emit_typed_comment("Location", location.as_str()),
            Header::Situation { text } => self.emit_typed_comment("Situation", text.as_str()),
            Header::Activities { activities } => {
                self.emit_typed_comment("Activities", activities.as_str())
            }
            Header::Transcriber { transcriber } => {
                self.emit_typed_comment("Transcriber", transcriber.as_str())
            }
            Header::Transcription { transcription } => {
                self.emit_typed_comment("Transcription", transcription.as_str())
            }
            Header::Warning { text } => self.emit_typed_comment("Warning", text.as_str()),
            Header::Bck { bck } => self.emit_typed_comment("Bck", bck.as_str()),
            Header::Number { number } => self.emit_typed_comment("Number", number.as_str()),
            Header::RecordingQuality { quality } => {
                self.emit_typed_comment("Recording Quality", quality.as_str())
            }
            Header::TapeLocation { location } => {
                self.emit_typed_comment("Tape Location", location.as_str())
            }
            Header::TimeDuration { duration } => {
                self.emit_typed_comment("Time Duration", duration.as_str())
            }
            Header::TimeStart { start } => self.emit_typed_comment("Time Start", start.as_str()),
            Header::RoomLayout { layout } => {
                self.emit_typed_comment("Room Layout", layout.as_str())
            }
            Header::Page { page } => self.emit_typed_comment("Page", page.as_str()),
            Header::T { text } => self.emit_typed_comment("T", text.as_str()),

            // `@Types: design, activity, group` → `<comment
            // type="Types">design, activity, group</comment>`. The three
            // fields are always emitted, comma-space separated.
            Header::Types(types) => {
                let payload = format!(
                    "{}, {}, {}",
                    types.design.as_str(),
                    types.activity.as_str(),
                    types.group.as_str()
                );
                self.emit_typed_comment("Types", &payload)
            }

            // `@Font`, `@Window`, `@Color words` project onto the
            // root `<CHAT>` element as attributes (see
            // `find_root_font` / `find_root_window` /
            // `find_root_color_words`) rather than as body-level
            // comments. Suppress here to avoid emitting them twice.
            Header::Font { .. } => Ok(()),
            Header::Window { .. } => Ok(()),
            Header::ColorWords { .. } => Ok(()),
            // `@Videos:` has no corresponding XML element in the
            // TalkBank XSD and Java Chatter silently drops it.
            // Matching that: preserve the CHAT source, emit nothing.
            Header::Videos { .. } => Ok(()),
            // Marker headers with no payload. The XSD has dedicated
            // `"New Episode"` and `"Blank"` `commentTypeType` values.
            Header::NewEpisode => self.emit_typed_comment("New Episode", ""),
            Header::Blank => self.emit_typed_comment("Blank", ""),

            // Lenient-parse fallback. Preserve the original text in a
            // generic comment so the utterance stays well-formed; the
            // diagnostic `parse_reason` / `suggested_fix` fields are
            // validator metadata, not content, and don't project to XML.
            Header::Unknown { text, .. } => self.emit_typed_comment("Generic", text.as_str()),
        }
    }

    /// Emit `<begin-gem>` / `<end-gem>` / `<lazy-gem>` with the
    /// `label` attribute. Java Chatter emits them as standalone empty
    /// elements alongside `<u>`, not inside `<comment>`.
    fn emit_gem(&mut self, element: &'static str, label: &str) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new(element);
        tag.push_attribute(("label", label));
        self.writer.write_event(Event::Empty(tag))?;
        Ok(())
    }
}

/// Resolve an optional [`GemLabel`] to the `label="…"` attribute
/// value for `<begin-gem>` / `<end-gem>` / `<lazy-gem>`. The XSD
/// marks `label` as required, so when CHAT source omits it we emit
/// an empty string — schema-legal and round-trippable.
fn gem_label(label: &Option<talkbank_model::model::GemLabel>) -> &str {
    label.as_ref().map(|l| l.as_str()).unwrap_or("")
}

// Re-open the `impl XmlEmitter` block for the remaining methods.
impl XmlEmitter {
    /// Emit `<comment type="X">text</comment>`. Shared by `@Comment`
    /// and the typed metadata headers that project onto comments in
    /// the XML schema.
    fn emit_typed_comment(&mut self, type_value: &str, text: &str) -> Result<(), XmlWriteError> {
        let mut start = BytesStart::new("comment");
        start.push_attribute(("type", type_value));
        self.writer.write_event(Event::Start(start))?;
        let normalized = collapse_whitespace(text);
        self.writer
            .write_event(Event::Text(escape_text(&normalized)))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("comment")))?;
        Ok(())
    }

    /// Emit a `<comment type=…>` element with mixed content: text
    /// segments as `<Text>` children, timing bullets as sibling
    /// `<media start=… end=… unit="s"/>` elements, and picture
    /// references preserved as inline `%pic:"…"` text (there's no
    /// structural XML element for those).
    ///
    /// Matches Java Chatter's `@Comment` emission shape — previously
    /// our emitter flattened everything to `[start_end]` text inside
    /// the comment, which lost the structural timing.
    fn emit_bullet_content_comment(
        &mut self,
        type_value: &str,
        content: &BulletContent,
    ) -> Result<(), XmlWriteError> {
        let mut start = BytesStart::new("comment");
        start.push_attribute(("type", type_value));
        self.writer.write_event(Event::Start(start))?;
        self.emit_bullet_content_children(content)?;
        self.writer
            .write_event(Event::End(BytesEnd::new("comment")))?;
        Ok(())
    }

    /// Walk a `BulletContent` and emit its segments as XML children of
    /// the currently-open element: text segments as `Text` events,
    /// timing bullets as `<media start=… end=… unit="s"/>` empty
    /// elements, picture references as inline `%pic:"…"` text.
    ///
    /// Shared between `<comment>` (header `@Comment`) and `<a>`
    /// (dependent-tier side tiers like `%cod`, `%act`, `%com`, etc.)
    /// — both take BulletContent and both produce mixed content in
    /// Java Chatter's XML output.
    pub(super) fn emit_bullet_content_children(
        &mut self,
        content: &BulletContent,
    ) -> Result<(), XmlWriteError> {
        use talkbank_model::model::BulletContentSegment;

        let mut text_buf = String::new();
        for segment in content.segments.0.iter() {
            match segment {
                BulletContentSegment::Text(text) => {
                    text_buf.push_str(&text.text);
                }
                BulletContentSegment::Bullet(bullet) => {
                    if !text_buf.is_empty() {
                        let normalized = collapse_whitespace(&text_buf);
                        self.writer
                            .write_event(Event::Text(escape_text(&normalized)))?;
                        text_buf.clear();
                    }
                    let start_s = super::wor::format_seconds(bullet.start_ms);
                    let end_s = super::wor::format_seconds(bullet.end_ms);
                    let mut media = BytesStart::new("media");
                    media.push_attribute(("start", start_s.as_str()));
                    media.push_attribute(("end", end_s.as_str()));
                    media.push_attribute(("unit", "s"));
                    self.writer.write_event(Event::Empty(media))?;
                }
                BulletContentSegment::Picture(picture) => {
                    // `%pic:"filename"` → `<mediaPic href="filename"/>`
                    // per Java Chatter. Flush any buffered text so
                    // the `<mediaPic>` lands in document order.
                    if !text_buf.is_empty() {
                        let normalized = collapse_whitespace(&text_buf);
                        self.writer
                            .write_event(Event::Text(escape_text(&normalized)))?;
                        text_buf.clear();
                    }
                    let mut tag = BytesStart::new("mediaPic");
                    tag.push_attribute(("href", picture.filename.as_str()));
                    self.writer.write_event(Event::Empty(tag))?;
                }
                BulletContentSegment::Continuation => {
                    // Tab-indented continuation — collapse_whitespace
                    // will flatten this when we flush.
                    text_buf.push(' ');
                }
            }
        }

        if !text_buf.is_empty() {
            let normalized = collapse_whitespace(&text_buf);
            self.writer
                .write_event(Event::Text(escape_text(&normalized)))?;
        }
        Ok(())
    }

    /// Emit a single `<u who=… uID=…>…<t/>…</u>` utterance. This is
    /// the orchestration point that walks main-tier content in parallel
    /// with the `%mor` cursor, dispatching each content item to the
    /// appropriate word-level emitter. The terminator closes the
    /// utterance and, if `%mor` carries a terminator chunk, picks up
    /// the matching `<gra/>` from chunk index `n+1`.
    fn emit_utterance(
        &mut self,
        utterance: &talkbank_model::model::Utterance,
    ) -> Result<(), XmlWriteError> {
        // Pre-begin headers attached to this utterance.
        for header in utterance.preceding_headers.iter() {
            self.emit_header_if_body(header)?;
        }

        // Split dependent tiers into recognized Mor / Gra / other.
        // Phonetic / syllabification tiers (%pho, %mod, %phosyl,
        // %modsyl, %phoaln) are permanently unsupported — see
        // `XmlWriteError::PhoneticTierUnsupported`. Other staged
        // tiers surface via `FeatureNotImplemented` one at a time.
        let tiers = collect_utterance_tiers(utterance, self.next_utterance_id as usize)?;

        let uid = format!("u{}", self.next_utterance_id);
        self.next_utterance_id += 1;

        let mut start = BytesStart::new("u");
        start.push_attribute(("who", utterance.main.speaker.as_str()));
        start.push_attribute(("uID", uid.as_str()));
        // `[- LANG]` pre-code promotes the utterance's baseline
        // language to a tier-scoped override; Java Chatter projects
        // that onto `<u xml:lang="LANG">`. The grammar populates
        // `main.content.language_code` directly when it parses the
        // pre-code, so we read it here rather than going through
        // the computed `utterance_language` state — the latter is
        // only populated when the caller invokes
        // `compute_language_metadata` (e.g. during the alignment
        // pipeline), but XML emission runs on the bare parse too.
        if let Some(code) = utterance.main.content.language_code.as_ref() {
            start.push_attribute(("xml:lang", code.as_str()));
        }
        self.writer.write_event(Event::Start(start))?;

        // Discourse linkers (`+<`, `++`, `+≈`, `+≋`, …) sit at the
        // very start of tier content and render as `<linker
        // type="…"/>` children of `<u>` ahead of any `<w>` content.
        for linker in utterance.main.content.linkers.0.iter() {
            self.emit_linker(linker)?;
        }

        // Per-content-arm logic reads cursors and calls
        // `cursors.consume_*` to advance. Advance rules live on
        // `TierCursors` (see its rustdoc).
        let mut cursors = TierCursors::new();
        for item in utterance.main.content.content.iter() {
            match item {
                UtteranceContent::Word(word) => {
                    // Leading overlap markers (`⌈`, `⌊`) attached to
                    // the front of a word get hoisted out as
                    // top-level `<overlap-point/>` siblings before
                    // the `<w>`. The Rust parser bundles them into
                    // `word.content`; Java Chatter emits them
                    // outside. Peel them here so the word body starts
                    // with its actual first lexical segment.
                    self.emit_leading_overlap_points(word)?;

                    // Nonword (`&~`), filler (`&-`), phonological
                    // fragment (`&+`), and untranscribed (`xxx` /
                    // `yyy` / `www`) tokens appear on the main tier
                    // but have no corresponding `%mor` item, so we
                    // pass `None` for `mor` and keep the cursor
                    // where it is. Using the model's canonical
                    // `counts_for_tier(TierDomain::Mor)` predicate
                    // keeps this check aligned with validation logic.
                    let counts_mor = counts_for_tier(word, TierDomain::Mor);
                    let mor_for_word = if counts_mor {
                        tiers
                            .mor
                            .as_ref()
                            .and_then(|mor| mor.items().get(cursors.mor_index()))
                    } else {
                        None
                    };

                    // `%sin` attaches one sign-word per main-tier
                    // word that counts for `TierDomain::Sin`, wrapping
                    // the whole pair in `<sg><w>...</w><sw>sin</sw></sg>`
                    // per Java Chatter's schema. `%sin` includes more
                    // token kinds than `%mor` (fragments, untranscribed
                    // all participate), so the gate is separate.
                    let counts_sin = tiers.sin.is_some() && counts_for_tier(word, TierDomain::Sin);
                    let sin_item = if counts_sin {
                        tiers
                            .sin
                            .as_ref()
                            .and_then(|sin| sin.items.0.get(cursors.sin_index()))
                    } else {
                        None
                    };

                    if sin_item.is_some() {
                        self.writer
                            .write_event(Event::Start(BytesStart::new("sg")))?;
                    }
                    self.emit_word(word, mor_for_word, tiers.gra, cursors.gra_chunk())?;
                    if let Some(item) = sin_item {
                        self.emit_sin_word(item)?;
                        self.writer.write_event(Event::End(BytesEnd::new("sg")))?;
                    }

                    if counts_mor && tiers.mor.is_some() {
                        let post_count = mor_for_word.map(|m| m.post_clitics.len()).unwrap_or(0);
                        cursors.consume_mor(post_count);
                    }
                    if counts_sin {
                        cursors.consume_sin();
                    }
                }
                UtteranceContent::Separator(sep) => {
                    // Only Comma / Tag / Vocative separators
                    // participate in `%mor` alignment (they produce
                    // `cm|cm`, `end|end`, `beg|beg` mor items). CA
                    // intonation markers and other structural
                    // separators render to `<s>` / `<tagMarker>`
                    // without consuming mor chunks.
                    let counts_for_mor = is_tag_marker_separator(sep);
                    let mor_for_sep = if counts_for_mor {
                        tiers
                            .mor
                            .as_ref()
                            .and_then(|mor| mor.items().get(cursors.mor_index()))
                    } else {
                        None
                    };
                    self.emit_separator(sep, mor_for_sep, tiers.gra, cursors.gra_chunk())?;
                    if counts_for_mor && tiers.mor.is_some() {
                        let post_count = mor_for_sep.map(|m| m.post_clitics.len()).unwrap_or(0);
                        cursors.consume_mor(post_count);
                    }
                }
                UtteranceContent::Pause(pause) => {
                    self.emit_pause(pause)?;
                }
                UtteranceContent::Retrace(retrace) => {
                    self.emit_retrace(retrace)?;
                }
                UtteranceContent::AnnotatedWord(annotated) => {
                    let mor_for_chunk = tiers
                        .mor
                        .and_then(|mor| mor.items().get(cursors.mor_index()));
                    self.emit_annotated_word(
                        annotated,
                        mor_for_chunk,
                        tiers.gra,
                        cursors.gra_chunk(),
                    )?;
                    if tiers.mor.is_some() {
                        let post_count = mor_for_chunk.map(|m| m.post_clitics.len()).unwrap_or(0);
                        cursors.consume_mor(post_count);
                    }
                }
                UtteranceContent::ReplacedWord(rw) => {
                    // `emit_replaced_word` consumes N mor items +
                    // their post-clitic `%gra` edges internally via
                    // the shared `cursors`.
                    self.emit_replaced_word(rw, &tiers, &mut cursors)?;
                }
                UtteranceContent::Event(event) => {
                    // Inline `&=descriptor` event in main-tier content
                    // (e.g. `&=laughs`). Events don't consume mor/gra
                    // chunks — they're outside the word alignment.
                    self.emit_event(event)?;
                }
                UtteranceContent::AnnotatedAction(annotated) => {
                    // Bare main-tier action (`0 .` utterance or `0`
                    // token) — scoped annotations on the action are a
                    // separate increment; the bare `<e><action/></e>`
                    // shape is all the reference corpus uses.
                    self.emit_annotated_action(annotated)?;
                }
                UtteranceContent::OverlapPoint(point) => {
                    // Top-level overlap markers (`⌈` / `⌉` / `⌊` /
                    // `⌋` appearing outside a word) render as
                    // `<overlap-point/>` children of `<u>`.
                    self.emit_overlap_point(point)?;
                }
                UtteranceContent::AnnotatedGroup(annotated) => {
                    // `<word1 word2> [annotation]` →
                    // `<g><w>word1</w><w>word2</w><annotation/></g>`.
                    // `emit_annotated_group` advances cursors inline.
                    self.emit_annotated_group(annotated, &tiers, &mut cursors)?;
                }
                UtteranceContent::AnnotatedEvent(annotated) => {
                    // `&=descriptor [!]` → `<e><happening>text</happening>
                    // <k type="stressing"/></e>`. Annotations on an
                    // event attach *inside* `<e>` rather than wrapping
                    // it in `<g>`, per the XSD `<e>` choice sequence.
                    self.emit_annotated_event(annotated)?;
                }
                UtteranceContent::Group(group) => {
                    // `<word word>` without scoped annotations — same
                    // shape as `AnnotatedGroup` minus the sibling
                    // annotations. Cursors advance inside.
                    self.emit_bare_group(group, &tiers, &mut cursors)?;
                }
                UtteranceContent::Quotation(quotation) => {
                    self.emit_quotation(quotation)?;
                }
                UtteranceContent::Freecode(freecode) => {
                    self.emit_freecode(freecode)?;
                }
                UtteranceContent::LongFeatureBegin(lf) => {
                    self.emit_long_feature("begin", lf.label.as_str())?;
                }
                UtteranceContent::LongFeatureEnd(lf) => {
                    self.emit_long_feature("end", lf.label.as_str())?;
                }
                UtteranceContent::NonvocalBegin(nv) => {
                    self.emit_nonvocal("begin", nv.label.as_str())?;
                }
                UtteranceContent::NonvocalEnd(nv) => {
                    self.emit_nonvocal("end", nv.label.as_str())?;
                }
                UtteranceContent::NonvocalSimple(nv) => {
                    self.emit_nonvocal("simple", nv.label.as_str())?;
                }
                UtteranceContent::UnderlineBegin(_) => {
                    let mut tag = quick_xml::events::BytesStart::new("underline");
                    tag.push_attribute(("type", "begin"));
                    self.writer
                        .write_event(quick_xml::events::Event::Empty(tag))?;
                }
                UtteranceContent::UnderlineEnd(_) => {
                    let mut tag = quick_xml::events::BytesStart::new("underline");
                    tag.push_attribute(("type", "end"));
                    self.writer
                        .write_event(quick_xml::events::Event::Empty(tag))?;
                }
                UtteranceContent::InternalBullet(bullet) => {
                    // Standalone bullet inside main content (rare;
                    // usually bullets attach to a word) — emit as
                    // `<internal-media>` using the same seconds
                    // formatting as `%wor` bullets.
                    self.emit_internal_media(bullet)?;
                }
                UtteranceContent::OtherSpokenEvent(event) => {
                    // `&*WHO=word` interposed-speaker marker. Per the
                    // XSD, `<otherSpokenEvent>` nests inside `<e>`
                    // alongside `<action>` and `<happening>`.
                    self.emit_other_spoken_event(event)?;
                }
                UtteranceContent::PhoGroup(_) | UtteranceContent::SinGroup(_) => {
                    // `<pg>` / `<sg>` are Phon-specific structured
                    // payloads. Permanently out of scope (same
                    // policy as `%pho` / `%mod` tiers) — surface as
                    // `PhoneticTierUnsupported` at the utterance
                    // level rather than an open-ended
                    // `FeatureNotImplemented`.
                    return Err(XmlWriteError::PhoneticTierUnsupported {
                        utterance_index: self.next_utterance_id.saturating_sub(1) as usize,
                    });
                }
            }
        }

        // %mor always emits exactly one extra item (the terminator
        // chunk); we feed that index to the terminator emission so
        // its `<mor>` subtree picks up the matching `<gra>`.
        match utterance.main.content.terminator.as_ref() {
            Some(terminator) => {
                self.emit_terminator(terminator, &tiers, cursors.gra_chunk())?;
            }
            None => {
                // CA transcripts commonly end an utterance with a
                // pitch-contour separator (`⇗`, `⇘`, `∞`, …) and no
                // conventional terminator. Java Chatter emits
                // `<t type="missing CA terminator"/>` in that case.
                let mut tag = quick_xml::events::BytesStart::new("t");
                tag.push_attribute(("type", "missing CA terminator"));
                self.writer
                    .write_event(quick_xml::events::Event::Empty(tag))?;
            }
        }

        // Utterance-level `<media>` element: the main tier's trailing
        // bullet (`· start_end ·` after the terminator) becomes a
        // `<media start="s.sss" end="s.sss" unit="s"/>` sibling of
        // `<t>` in Java Chatter's output. Emission order is:
        // main-tier words → `<t>` → `<media>` → `<wor>`.
        if let Some(bullet) = utterance.main.content.bullet.as_ref() {
            self.emit_utterance_media(bullet)?;
        }

        // `[+ code]` postcodes — one `<postcode>` element per code
        // in source order. Java Chatter emits these directly after
        // `<t/>` / `<media/>` and before `<wor>` / dependent-tier
        // annotations. The model stores them on
        // `main.content.postcodes`, separate from inline content.
        for postcode in utterance.main.content.postcodes.iter() {
            self.writer
                .write_event(Event::Start(BytesStart::new("postcode")))?;
            self.writer
                .write_event(Event::Text(escape_text(postcode.text.as_str())))?;
            self.writer
                .write_event(Event::End(BytesEnd::new("postcode")))?;
        }

        // `<wor>` — the word-level timing sidecar. Emitted only when
        // the utterance carried a `%wor` tier.
        if let Some(wor) = tiers.wor {
            self.emit_wor(wor)?;
        }

        // Text-content "side tiers" (`%act`, `%com`, `%exp`, `%gpx`,
        // `%sit`, `%xLABEL`) become `<a type="…">text</a>` children
        // of `<u>` per Java Chatter's XML shape.
        if !tiers.side_tiers.is_empty() {
            self.emit_side_tiers(&tiers.side_tiers)?;
        }

        self.writer.write_event(Event::End(BytesEnd::new("u")))?;
        Ok(())
    }

    /// Emit one `%sin` item as a `<sw>…</sw>` child of the surrounding
    /// `<sg>` group. Tokens render as their raw text (including the
    /// `0` "no-gesture" sentinel, which round-trips as `<sw>0</sw>` per
    /// Java Chatter's golden output). `SinGroup(…)` — multi-gesture
    /// items enclosed in `〔…〕` on CHAT — renders its joined gesture
    /// text; richer structured emission for sin-groups would go here
    /// later if the XSD requires it.
    fn emit_sin_word(&mut self, item: &SinItem) -> Result<(), XmlWriteError> {
        self.writer
            .write_event(Event::Start(BytesStart::new("sw")))?;
        match item {
            SinItem::Token(token) => {
                self.writer
                    .write_event(Event::Text(super::writer::escape_text(token.as_ref())))?;
            }
            SinItem::SinGroup(gestures) => {
                // Flatten `〔g1 g2〕` as space-separated for `<sw>`.
                let mut buf = String::new();
                for (i, gesture) in gestures.0.iter().enumerate() {
                    if i > 0 {
                        buf.push(' ');
                    }
                    buf.push_str(gesture.as_ref());
                }
                self.writer
                    .write_event(Event::Text(super::writer::escape_text(&buf)))?;
            }
        }
        self.writer.write_event(Event::End(BytesEnd::new("sw")))?;
        Ok(())
    }
}

/// Extracts the first `@ID` header's corpus field and returns it as a
/// string. Every reference-corpus file carries a populated corpus slot;
/// absence here indicates malformed input reaching the emitter.
fn find_corpus<S: ValidationState>(file: &ChatFile<S>) -> Result<String, XmlWriteError> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::ID(id) = header.as_ref()
            && let Some(corpus) = &id.corpus
        {
            return Ok(corpus.as_ref().to_owned());
        }
    }
    Err(XmlWriteError::MissingMetadata {
        what: "Corpus attribute (no @ID header with a corpus field)".to_owned(),
    })
}

/// Space-joined language codes for the root `Lang` attribute, matching
/// Java Chatter's format (`"eng ara"`, not `"eng, ara"`).
fn join_language_codes<S: ValidationState>(file: &ChatFile<S>) -> String {
    join_codes_with_space(&file.languages.0)
}

fn join_codes_with_space(codes: &[talkbank_model::model::LanguageCode]) -> String {
    let mut out = String::new();
    for (i, code) in codes.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(code.as_str());
    }
    out
}

/// Normalize runs of whitespace in header / comment text. CHAT source
/// formatting — double spaces for visual alignment, tab-indented
/// continuation lines — carries no semantic content once the header
/// has been parsed; Java Chatter's XML emitter collapses those to
/// single spaces (`xs:token`-style), and we match.
fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for tok in input.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(tok);
    }
    out
}

/// Locate the `@Types` header (if any). Returns a borrowed
/// reference so `emit_document` can push the `DesignType` /
/// `ActivityType` / `GroupType` attributes onto the `<CHAT>` root
/// without owning the underlying `TypesHeader`.
fn find_root_types<S: ValidationState>(
    file: &ChatFile<S>,
) -> Option<&talkbank_model::model::TypesHeader> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Types(types) = header.as_ref()
        {
            return Some(types);
        }
    }
    None
}

/// Locate the `@PID` header (if any). Returns a borrowed `&str` view
/// of the value for direct push onto the root element.
fn find_root_pid<S: ValidationState>(file: &ChatFile<S>) -> Option<&str> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Pid { pid } = header.as_ref()
        {
            return Some(pid.as_str());
        }
    }
    None
}

/// Pre-Begin UI-state header values destined for `<CHAT>` root
/// attributes. Populated by a single pass over `file.lines` so the
/// emitter doesn't re-scan the header block once per attribute.
#[derive(Default)]
struct RootUiAttrs<'a> {
    colors: Option<&'a str>,
    font: Option<&'a str>,
    window: Option<&'a str>,
}

fn find_root_ui_attrs<S: ValidationState>(file: &ChatFile<S>) -> RootUiAttrs<'_> {
    let mut out = RootUiAttrs::default();
    for line in file.lines.iter() {
        let Line::Header { header, .. } = line else {
            continue;
        };
        match header.as_ref() {
            Header::Window { geometry } if out.window.is_none() => {
                out.window = Some(geometry.as_str());
            }
            Header::Font { font } if out.font.is_none() => {
                out.font = Some(font.as_str());
            }
            Header::ColorWords { colors } if out.colors.is_none() => {
                out.colors = Some(colors.as_str());
            }
            _ => {}
        }
    }
    out
}

/// Find the `@Date` header (if any) and format it for the root
/// `Date="YYYY-MM-DD"` attribute. Returns `Ok(None)` when the file has
/// no `@Date`; returns an error if the date is present but unparseable.
fn find_root_date<S: ValidationState>(file: &ChatFile<S>) -> Result<Option<String>, XmlWriteError> {
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Date { date } = header.as_ref()
        {
            return Ok(Some(format_chat_date_iso(date)?));
        }
    }
    Ok(None)
}

/// Space-joined `@Options` flags for the root `Options` attribute.
/// Returns `None` when the file declares no options (so the attribute
/// is simply omitted, matching Java Chatter).
fn format_options_attribute<S: ValidationState>(file: &ChatFile<S>) -> Option<String> {
    if file.options.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, flag) in file.options.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(ChatOptionFlag::as_str(flag));
    }
    Some(out)
}

/// Per-speaker metadata that lives outside `Participant` itself but
/// gets hoisted onto the `<participant>` element in XML.
#[derive(Default)]
struct SpeakerExtras {
    birthplace: Option<String>,
    first_language: Option<String>,
}

/// Scan all body-level headers for `@Birthplace of X` / `@L1 of X`
/// entries, keyed by the participant's `SpeakerCode`. The
/// `emit_participants` pass then hoists these onto the
/// `<participant>` element whose `id` matches.
fn collect_per_speaker_metadata<S: ValidationState>(
    file: &ChatFile<S>,
) -> HashMap<SpeakerCode, SpeakerExtras> {
    let mut out: HashMap<SpeakerCode, SpeakerExtras> = HashMap::new();
    for line in file.lines.iter() {
        let Line::Header { header, .. } = line else {
            continue;
        };
        match header.as_ref() {
            Header::Birthplace { participant, place } => {
                out.entry(participant.clone()).or_default().birthplace =
                    Some(place.as_str().to_owned());
            }
            Header::L1Of {
                participant,
                language,
            } => {
                out.entry(participant.clone()).or_default().first_language =
                    Some(language.as_str().to_owned());
            }
            _ => {}
        }
    }
    out
}

/// `ChatDate` → `YYYY-MM-DD`. Rejects unsupported dates up front so the
/// emitter never writes a malformed attribute.
fn format_chat_date_iso(date: &ChatDate) -> Result<String, XmlWriteError> {
    match date {
        ChatDate::Valid {
            day, month, year, ..
        } => Ok(format!(
            "{year:04}-{month:02}-{day:02}",
            year = year,
            month = month_to_number(month),
            day = day
        )),
        ChatDate::Unsupported(raw) => Err(XmlWriteError::MissingMetadata {
            what: format!("unparseable @Birth/@Date value: {raw}"),
        }),
    }
}

fn month_to_number(month: &Month) -> u8 {
    match month {
        Month::Jan => 1,
        Month::Feb => 2,
        Month::Mar => 3,
        Month::Apr => 4,
        Month::May => 5,
        Month::Jun => 6,
        Month::Jul => 7,
        Month::Aug => 8,
        Month::Sep => 9,
        Month::Oct => 10,
        Month::Nov => 11,
        Month::Dec => 12,
    }
}

/// `AgeValue` → ISO 8601 duration. Examples:
/// - `1;08.02` → `P1Y08M02D`
/// - `43;`    → `P43Y`
/// - `2;06`   → `P2Y06M`
///
/// Months and days are zero-padded to two digits to match Java Chatter's
/// output; years are unpadded.
fn format_age_iso8601(age: &AgeValue) -> Result<String, XmlWriteError> {
    match age {
        AgeValue::Valid {
            years,
            months,
            days,
            ..
        } => {
            let mut out = format!("P{years}Y");
            if let Some(m) = months {
                out.push_str(&format!("{m:02}M"));
            }
            if let Some(d) = days {
                out.push_str(&format!("{d:02}D"));
            }
            Ok(out)
        }
        AgeValue::Unsupported(raw) => Err(XmlWriteError::MissingMetadata {
            what: format!("unparseable @ID age value: {raw}"),
        }),
    }
}

/// Map [`Sex`] to its XML attribute value. Unsupported values escalate
/// rather than silently serializing the raw text — downstream consumers
/// expect exactly `male` or `female` in this slot.
fn sex_to_xml(sex: &Sex) -> Result<&'static str, XmlWriteError> {
    match sex {
        Sex::Male => Ok("male"),
        Sex::Female => Ok("female"),
        Sex::Unsupported(raw) => Err(XmlWriteError::MissingMetadata {
            what: format!("unsupported @ID sex value: {raw}"),
        }),
    }
}

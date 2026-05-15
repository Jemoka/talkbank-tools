//! Text-content dependent-tier emission.
//!
//! These are the "side tiers" that Java Chatter maps uniformly to
//! `<a type="…">text</a>` elements inside `<u>`, placed after the
//! terminator / media / `<wor>` block:
//!
//! | CHAT tier | XML shape |
//! |-----------|-----------|
//! | `%act`    | `<a type="actions">…</a>` |
//! | `%com`    | `<a type="comments">…</a>` |
//! | `%exp`    | `<a type="explanation">…</a>` |
//! | `%gpx`    | `<a type="gesture">…</a>` |
//! | `%sit`    | `<a type="situation">…</a>` |
//! | `%xLABEL` | `<a type="extension" flavor="LABEL">…</a>` |
//!
//! Tiers with richer per-word structure (`%mor`, `%gra`, `%wor`,
//! `%pho`, `%mod`, `%sin`) are handled in their own submodules. The
//! remaining text-content tiers (`%add`, `%int`, `%spa`, `%cod` and
//! the simple `TextTier` variants `%alt`, `%coh`, `%def`, `%eng`,
//! `%err`, `%fac`, `%flo`, plus syllable tiers) are staged — they
//! fall through to `FeatureNotImplemented` so the harness surfaces
//! each one as a distinct increment rather than silently dropping
//! it.

use quick_xml::events::{BytesEnd, BytesStart, Event};

use talkbank_model::model::DependentTier;

use super::error::XmlWriteError;
use super::mor::tier_kind;
use super::writer::{XmlEmitter, escape_text};

impl XmlEmitter {
    /// Emit one or more `<a type="…">text</a>` elements for the
    /// "side tiers" collected during utterance-tier classification.
    /// Tiers are emitted in the order the CHAT source presents them
    /// — the comparator ignores order, but preserving it minimizes
    /// diff noise during development.
    pub(super) fn emit_side_tiers(
        &mut self,
        side_tiers: &[&DependentTier],
    ) -> Result<(), XmlWriteError> {
        for tier in side_tiers {
            self.emit_side_tier(tier)?;
        }
        Ok(())
    }

    fn emit_side_tier(&mut self, tier: &DependentTier) -> Result<(), XmlWriteError> {
        // Every text-content dep tier renders to `<a type=…>`. Two
        // body shapes:
        //   - BulletContent → mixed content (text + `<media/>` +
        //     `<mediaPic/>` children) via `emit_bullet_content_children`
        //   - a flat string → one text node
        //
        // Structured tiers (`%mor`, `%gra`, `%wor`, `%pho`, `%mod`,
        // syllable tiers) have dedicated emitters and should never
        // reach this path — reject with `FeatureNotImplemented`.
        match tier {
            DependentTier::Act(t) => self.emit_bullet_tier("actions", &t.content),
            DependentTier::Com(t) => self.emit_bullet_tier("comments", &t.content),
            DependentTier::Exp(t) => self.emit_bullet_tier("explanation", &t.content),
            DependentTier::Sit(t) => self.emit_bullet_tier("situation", &t.content),
            DependentTier::Gpx(t) => self.emit_bullet_tier("gesture", &t.content),
            DependentTier::Add(t) => self.emit_bullet_tier("addressee", &t.content),
            DependentTier::Int(t) => self.emit_bullet_tier("intonation", &t.content),
            DependentTier::Spa(t) => self.emit_bullet_tier("speech act", &t.content),
            DependentTier::Cod(t) => self.emit_bullet_tier("coding", &t.content),

            DependentTier::Alt(t) => self.emit_text_tier("alternative", None, t.content.as_str()),
            DependentTier::Coh(t) => self.emit_text_tier("cohesion", None, t.content.as_str()),
            DependentTier::Def(t) => self.emit_text_tier("SALT", None, t.content.as_str()),
            DependentTier::Eng(t) => {
                self.emit_text_tier("english translation", None, t.content.as_str())
            }
            DependentTier::Err(t) => self.emit_text_tier("errcoding", None, t.content.as_str()),
            DependentTier::Fac(t) => self.emit_text_tier("facial", None, t.content.as_str()),
            DependentTier::Flo(t) => self.emit_text_tier("flow", None, t.content.as_str()),
            DependentTier::Gls(t) => self.emit_text_tier("target gloss", None, t.content.as_str()),
            DependentTier::Ort(t) => self.emit_text_tier("orthography", None, t.content.as_str()),
            DependentTier::Par(t) => {
                self.emit_text_tier("paralinguistics", None, t.content.as_str())
            }
            DependentTier::Tim(t) => self.emit_text_tier("time stamp", None, t.as_str()),

            // `%xLABEL` — the tier label carries the x-prefix (e.g.
            // `xpho` for `%xpho`). Java Chatter strips the prefix
            // and uses the remainder as the `flavor` attribute.
            DependentTier::UserDefined(t) | DependentTier::Unsupported(t) => {
                let label = t.label.as_str();
                let flavor = label.strip_prefix('x').unwrap_or(label);
                self.emit_text_tier("extension", Some(flavor), t.content.as_str())
            }
            // `%sin` — structured sign-language annotation rendered
            // as plain text via `WriteChat` for round-trip fidelity.
            // See `collect_utterance_tiers` for the rationale.
            DependentTier::Sin(sin) => {
                use talkbank_model::model::WriteChat;
                let mut buf = String::new();
                sin.write_chat(&mut buf)
                    .map_err(|e| XmlWriteError::MissingMetadata {
                        what: format!("failed to serialize %sin for extension text: {e}"),
                    })?;
                self.emit_text_tier("gesture", None, &buf)
            }
            other => Err(XmlWriteError::FeatureNotImplemented {
                feature: format!(
                    "side tier emission for structured tier {}",
                    tier_kind(other)
                ),
            }),
        }
    }

    /// Emit `<a type=…[flavor=…]>TEXT</a>` for a tier whose payload
    /// is a single flat string (no bullets, no pictures).
    fn emit_text_tier(
        &mut self,
        tag_type: &str,
        flavor: Option<&str>,
        text: &str,
    ) -> Result<(), XmlWriteError> {
        self.open_a_tag(tag_type, flavor)?;
        self.writer.write_event(Event::Text(escape_text(text)))?;
        self.writer.write_event(Event::End(BytesEnd::new("a")))?;
        Ok(())
    }

    /// Emit `<a type=…>`-wrapped mixed content from a `BulletContent`:
    /// text interleaved with `<media>` and `<mediaPic>` children.
    /// Header-level counterpart is `emit_bullet_content_comment` in
    /// `xml::root`; both share `emit_bullet_content_children`.
    fn emit_bullet_tier(
        &mut self,
        tag_type: &str,
        content: &talkbank_model::model::BulletContent,
    ) -> Result<(), XmlWriteError> {
        self.open_a_tag(tag_type, None)?;
        self.emit_bullet_content_children(content)?;
        self.writer.write_event(Event::End(BytesEnd::new("a")))?;
        Ok(())
    }

    fn open_a_tag(&mut self, tag_type: &str, flavor: Option<&str>) -> Result<(), XmlWriteError> {
        let mut tag = BytesStart::new("a");
        tag.push_attribute(("type", tag_type));
        if let Some(flavor) = flavor {
            tag.push_attribute(("flavor", flavor));
        }
        self.writer.write_event(Event::Start(tag))?;
        Ok(())
    }
}

//! Structural XML comparison for the golden-XML parity harness.
//!
//! `assert_xml_structurally_equal` parses both the emitted and golden
//! XML with `quick-xml`, normalizes whitespace-only text and attribute
//! ordering, and compares the resulting event streams. It rejects
//! differences in element names, attribute names/values (regardless of
//! source order), and non-whitespace text content. It tolerates
//! differences in inter-element whitespace and indentation — which are
//! exactly the differences we don't want to over-constrain.
//!
//! # Why not raw byte diff
//!
//! Java Chatter emits a specific layout (indent = two spaces,
//! hard-coded blank lines inside `<participant>`, etc.) that carries no
//! semantic information. Downstream consumers (NLTK ElementTree,
//! childes-db xml.etree, Phon StAX) never observe these differences.
//! Matching them byte-for-byte would tie the Rust emitter to
//! formatting choices that aren't part of the contract.
//!
//! # Why not `quick-xml`'s built-in canonical mode
//!
//! `quick-xml` 0.39 does not expose a canonical serializer; structural
//! equality via normalized event streams is both simpler and easier to
//! diagnose when tests fail.

#![allow(dead_code)]

use std::fmt;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

/// Structural diff for two XML documents.
///
/// Carries enough context for a test failure message to be actionable
/// without also flooding the log with irrelevant whitespace noise.
#[derive(Debug)]
pub struct XmlStructuralDiff {
    /// Human-readable description of the first divergence encountered.
    pub detail: String,
}

impl fmt::Display for XmlStructuralDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

/// Compare two XML documents structurally. `Ok(())` on match, `Err`
/// describing the first divergence otherwise.
pub fn assert_xml_structurally_equal(
    actual: &str,
    expected: &str,
) -> Result<(), XmlStructuralDiff> {
    let actual_events = canonicalize_empties(extract_events(actual, "actual")?);
    let expected_events = canonicalize_empties(extract_events(expected, "expected")?);

    let len = actual_events.len().min(expected_events.len());
    for i in 0..len {
        if actual_events[i] != expected_events[i] {
            return Err(XmlStructuralDiff {
                detail: format!(
                    "first divergence at event {i}:\n  actual:   {:?}\n  expected: {:?}",
                    actual_events[i], expected_events[i]
                ),
            });
        }
    }
    if actual_events.len() != expected_events.len() {
        return Err(XmlStructuralDiff {
            detail: format!(
                "event count differs after {len} matching events: \
                 actual has {}, expected has {}. \
                 Next actual={:?}, next expected={:?}",
                actual_events.len(),
                expected_events.len(),
                actual_events.get(len),
                expected_events.get(len),
            ),
        });
    }
    Ok(())
}

/// One normalized event in the comparison stream.
///
/// Whitespace-only text events are dropped at extraction time; other
/// text is trimmed. Attributes are sorted by name so that Java Chatter's
/// declared attribute order and the Rust emitter's declared order are
/// treated as equivalent.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NormalizedEvent {
    Start {
        name: String,
        attributes: Vec<(String, String)>,
    },
    End {
        name: String,
    },
    Empty {
        name: String,
        attributes: Vec<(String, String)>,
    },
    Text(String),
    Cdata(String),
}

/// Fold `Start(name, attrs) + End(name)` pairs into `Empty(name, attrs)`.
/// XML treats `<e/>` and `<e></e>` as equivalent; forcing the emitter to
/// mimic one style or the other is formatting-level coupling that
/// consumers don't actually require.
fn canonicalize_empties(events: Vec<NormalizedEvent>) -> Vec<NormalizedEvent> {
    let mut out: Vec<NormalizedEvent> = Vec::with_capacity(events.len());
    for event in events {
        if let NormalizedEvent::End { name: end_name } = &event
            && let Some(NormalizedEvent::Start {
                name: start_name, ..
            }) = out.last()
            && start_name == end_name
        {
            // Safe: we just peeked and confirmed the shape.
            if let Some(NormalizedEvent::Start { name, attributes }) = out.pop() {
                out.push(NormalizedEvent::Empty { name, attributes });
                continue;
            }
        }
        out.push(event);
    }
    out
}

fn extract_events(xml: &str, label: &str) -> Result<Vec<NormalizedEvent>, XmlStructuralDiff> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut out = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(XmlStructuralDiff {
                    detail: format!(
                        "{label}: XML parse error at byte {}: {e}",
                        reader.buffer_position(),
                    ),
                });
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(tag)) => {
                let (name, attributes) = decode_start(&tag, label)?;
                out.push(NormalizedEvent::Start { name, attributes });
            }
            Ok(Event::End(tag)) => {
                let name = decode_name(tag.name().as_ref(), label)?;
                out.push(NormalizedEvent::End { name });
            }
            Ok(Event::Empty(tag)) => {
                let (name, attributes) = decode_start(&tag, label)?;
                out.push(NormalizedEvent::Empty { name, attributes });
            }
            Ok(Event::Text(text)) => {
                // trim_text(true) already strips whitespace-only text;
                // any non-empty remainder is semantic.
                let cow = text.xml_content().map_err(|e| XmlStructuralDiff {
                    detail: format!("{label}: text decode failed: {e}"),
                })?;
                let trimmed = cow.trim();
                if !trimmed.is_empty() {
                    out.push(NormalizedEvent::Text(trimmed.to_owned()));
                }
            }
            Ok(Event::CData(cdata)) => {
                let bytes = cdata.into_inner();
                let as_str = std::str::from_utf8(&bytes).map_err(|e| XmlStructuralDiff {
                    detail: format!("{label}: CDATA is not UTF-8: {e}"),
                })?;
                out.push(NormalizedEvent::Cdata(as_str.to_owned()));
            }
            // Prolog, comments, processing instructions, DOCTYPE: ignored
            // for structural equality. Java Chatter emits <?xml version=...?>
            // and `xsi:schemaLocation` etc. as attributes on <CHAT>,
            // which we do compare.
            Ok(_) => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn decode_start(
    tag: &BytesStart<'_>,
    label: &str,
) -> Result<(String, Vec<(String, String)>), XmlStructuralDiff> {
    let name = decode_name(tag.name().as_ref(), label)?;
    let mut attrs = Vec::new();
    for attr in tag.attributes() {
        let attr = attr.map_err(|e| XmlStructuralDiff {
            detail: format!("{label}: attribute parse error: {e}"),
        })?;
        let key = decode_name(attr.key.as_ref(), label)?;
        let value =
            attr.decode_and_unescape_value(tag.decoder())
                .map_err(|e| XmlStructuralDiff {
                    detail: format!("{label}: attribute '{key}' value decode failed: {e}"),
                })?;
        attrs.push((key, value.into_owned()));
    }
    attrs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((name, attrs))
}

fn decode_name(raw: &[u8], label: &str) -> Result<String, XmlStructuralDiff> {
    std::str::from_utf8(raw)
        .map(|s| s.to_owned())
        .map_err(|e| XmlStructuralDiff {
            detail: format!("{label}: element/attribute name is not UTF-8: {e}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_documents_match() {
        let a = r#"<?xml version="1.0"?><r a="1" b="2"><c/></r>"#;
        let b = r#"<?xml version="1.0"?><r a="1" b="2"><c/></r>"#;
        assert!(assert_xml_structurally_equal(a, b).is_ok());
    }

    #[test]
    fn attribute_order_ignored() {
        let a = r#"<r a="1" b="2"/>"#;
        let b = r#"<r b="2" a="1"/>"#;
        assert!(assert_xml_structurally_equal(a, b).is_ok());
    }

    #[test]
    fn whitespace_ignored() {
        let a = r#"<r><c/></r>"#;
        let b = "<r>\n  <c/>\n</r>";
        assert!(assert_xml_structurally_equal(a, b).is_ok());
    }

    #[test]
    fn attribute_value_difference_is_a_mismatch() {
        let a = r#"<r a="1"/>"#;
        let b = r#"<r a="2"/>"#;
        assert!(assert_xml_structurally_equal(a, b).is_err());
    }

    #[test]
    fn element_name_difference_is_a_mismatch() {
        let a = r#"<r><c/></r>"#;
        let b = r#"<r><d/></r>"#;
        assert!(assert_xml_structurally_equal(a, b).is_err());
    }

    #[test]
    fn text_content_is_compared_trimmed() {
        let a = "<r>hello</r>";
        let b = "<r>  hello  </r>";
        assert!(assert_xml_structurally_equal(a, b).is_ok());
    }

    #[test]
    fn self_closed_and_open_close_empty_tags_are_equivalent() {
        let a = r#"<r><t type="p"/></r>"#;
        let b = r#"<r><t type="p"></t></r>"#;
        assert!(assert_xml_structurally_equal(a, b).is_ok());
    }
}

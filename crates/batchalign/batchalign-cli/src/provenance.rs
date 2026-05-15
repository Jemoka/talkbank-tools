//! Processing provenance — injects `@Comment` headers recording what
//! batchalign3 did to a CHAT file, when, and with what engines.
//!
//! Format: `[ba3 <command> | key=val ; key=val | ISO-8601]`
//!
//! Each command run adds one comment. Re-running the same command
//! replaces the previous comment for that command, preserving comments
//! from other commands (e.g., morphotag comment survives align re-run).
//!
//! See `docs/design/processing-provenance.md` for the full spec.

use std::collections::BTreeMap;

use crate::api::ReleasedCommand;
use crate::chat_ops::{ChatFile, Header, Line, Span};
use talkbank_transform::parse::parse_lenient;
use talkbank_transform::serialize::to_chat_string;

/// Processing provenance metadata for one batchalign3 command invocation.
#[derive(Debug, Clone)]
pub struct ProvenanceComment {
    /// Command name (e.g., "morphotag", "align", "transcribe").
    pub command: String,
    /// Semantic key-value pairs (engine versions, options that affect output).
    /// BTreeMap for deterministic ordering.
    pub fields: BTreeMap<String, String>,
}

impl ProvenanceComment {
    /// Create a new provenance comment for the given command.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            fields: BTreeMap::new(),
        }
    }

    /// Add a key-value field.
    pub fn field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    /// Add a field only if the value is `true`.
    pub fn field_if(self, key: impl Into<String>, value: bool) -> Self {
        if value { self.field(key, "true") } else { self }
    }

    /// Format as the `[ba3 ...]` comment string (without `@Comment:\t` prefix).
    pub fn format(&self) -> String {
        let timestamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z");
        let fields: String = self
            .fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ; ");
        if fields.is_empty() {
            format!("[ba3 {} | {timestamp}]", self.command)
        } else {
            format!("[ba3 {} | {fields} | {timestamp}]", self.command)
        }
    }

    /// The prefix used to identify existing comments for this command.
    fn match_prefix(&self) -> String {
        format!("[ba3 {} |", self.command)
    }
}

/// Inject a provenance comment into a CHAT file's AST.
///
/// Replaces any existing `[ba3 <command> |` comment for the same command.
/// New comments are placed after the last `@ID` header.
pub fn inject_provenance(file: &mut ChatFile, comment: &ProvenanceComment) {
    let prefix = comment.match_prefix();
    let new_content = comment.format();

    // Remove existing provenance comment for this command.
    file.lines.0.retain(|line| {
        if let Line::Header { header, .. } = line
            && let Header::Comment { content } = header.as_ref()
        {
            let text = content.to_chat_string();
            return !text.trim().starts_with(&prefix);
        }
        true
    });

    // Find insertion point: after the last @ID header.
    let insert_pos = file
        .lines
        .0
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, line)| {
            if let Line::Header { header, .. } = line
                && matches!(header.as_ref(), Header::ID(_))
            {
                return Some(i + 1);
            }
            None
        })
        .unwrap_or(0);

    let bullet_content = crate::chat_ops::BulletContent::from_text(new_content);

    file.lines.0.insert(
        insert_pos,
        Line::Header {
            header: Box::new(Header::Comment {
                content: bullet_content,
            }),
            span: Span::default(),
        },
    );
}

/// Inject a provenance comment into serialized CHAT text.
///
/// Parses the text, injects the comment into the AST, and re-serializes.
/// This is a convenience wrapper for pipelines that work with CHAT strings
/// rather than AST objects.
pub fn inject_provenance_into_text(chat_text: &str, comment: &ProvenanceComment) -> String {
    let parser = crate::chat_parser();
    let (mut file, _) = parse_lenient(&parser, chat_text);
    inject_provenance(&mut file, comment);
    to_chat_string(&file)
}

/// Build a provenance comment for morphotag from engine versions.
pub fn morphotag_provenance(
    lang: &str,
    engine_version: &str,
    retokenize: bool,
    incremental: bool,
) -> ProvenanceComment {
    let mut comment = ProvenanceComment::new("morphotag")
        .field("engine", format!("stanza-{engine_version}"))
        .field("lang", lang);
    comment = comment.field_if("retokenize", retokenize);
    comment = comment.field_if("incremental", incremental);
    comment
}

/// Build a provenance comment for align from engine versions.
pub fn align_provenance(
    lang: &str,
    fa_engine: &str,
    utr_engine: Option<&str>,
    wor: bool,
    incremental: bool,
) -> ProvenanceComment {
    let mut comment = ProvenanceComment::new("align")
        .field("fa", fa_engine)
        .field("lang", lang);
    if let Some(utr) = utr_engine {
        comment = comment.field("utr", utr);
    }
    comment = comment.field_if("wor", wor);
    comment = comment.field_if("incremental", incremental);
    comment
}

/// Build a provenance comment for transcribe from engine versions.
pub fn transcribe_provenance(
    lang: &str,
    asr_engine: &str,
    diarize: bool,
    wor: bool,
) -> ProvenanceComment {
    ProvenanceComment::new("transcribe")
        .field("asr", asr_engine)
        .field("lang", lang)
        .field_if("diarize", diarize)
        .field_if("wor", wor)
}

/// Inject a human-readable "unchecked ASR" warning comment into a CHAT file.
///
/// This is separate from the machine-readable provenance comment. a user and
/// others rely on this text to know a transcript has not been human-reviewed.
/// The old batchalign (0.7/0.8) injected:
///   `@Comment: Batchalign X.Y.Z, ASR Engine rev. Unchecked output of ASR model.`
///
/// We preserve backward compatibility: if an existing unchecked comment is
/// present (from a previous transcribe run), it is replaced rather than
/// duplicated.
pub fn inject_unchecked_warning(file: &mut ChatFile, asr_engine: &str) {
    let version = env!("CARGO_PKG_VERSION");
    let warning_text =
        format!("Batchalign {version}, ASR Engine {asr_engine}. Unchecked output of ASR model.");
    let marker = "Unchecked output of ASR model";

    // Remove existing unchecked warning (if re-transcribing).
    file.lines.0.retain(|line| {
        if let Line::Header { header, .. } = line
            && let Header::Comment { content } = header.as_ref()
        {
            let text = content.to_chat_string();
            return !text.contains(marker);
        }
        true
    });

    // Insert after the last @ID header (same position as provenance).
    let insert_pos = file
        .lines
        .0
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, line)| {
            if let Line::Header { header, .. } = line
                && matches!(header.as_ref(), Header::ID(_))
            {
                return Some(i + 1);
            }
            None
        })
        .unwrap_or(0);

    let bullet_content = crate::chat_ops::BulletContent::from_text(warning_text);

    file.lines.0.insert(
        insert_pos,
        Line::Header {
            header: Box::new(Header::Comment {
                content: bullet_content,
            }),
            span: Span::default(),
        },
    );
}

/// Build a provenance comment for utseg from engine versions.
pub fn utseg_provenance(lang: &str, engine_version: &str) -> ProvenanceComment {
    ProvenanceComment::new("utseg")
        .field("engine", format!("stanza-{engine_version}"))
        .field("lang", lang)
}

/// Build a provenance comment for translate.
pub fn translate_provenance(lang: &str, engine_version: &str) -> ProvenanceComment {
    ProvenanceComment::new("translate")
        .field("engine", engine_version)
        .field("lang", lang)
}

/// Build a provenance comment for coref.
pub fn coref_provenance(lang: &str, engine_version: &str) -> ProvenanceComment {
    ProvenanceComment::new("coref")
        .field("engine", engine_version)
        .field("lang", lang)
}

// ---------------------------------------------------------------------------
// No-op write detection — recognize when a candidate output text would
// only differ from the on-disk text inside a single command's
// `[ba3 <cmd> | ...]` provenance comment, so the runner can skip
// pointless disk writes (and the spurious git-status churn they cause).
// ---------------------------------------------------------------------------

/// Returns `true` when `old_text` and `new_text` differ ONLY inside the
/// `@Comment:\t[ba3 <command> | ...]` provenance line for `command`.
///
/// Use case: a pipeline produces `new_text` after re-running a command
/// against an unchanged input; the only thing that's actually new is the
/// regenerated provenance comment's timestamp (and possibly engine
/// version). Writing `new_text` would update the file in place, advance
/// its mtime, and produce a one-line `git status` diff with zero semantic
/// content. This predicate lets the write site detect that situation and
/// skip the write entirely.
///
/// Returns `false` for byte-equal inputs (there is no diff to suppress)
/// and for any difference that extends beyond the named command's
/// provenance line — including a difference in another command's
/// provenance, in `%mor` / `%gra` / `%wor` content, or anywhere else.
///
/// Comparison is line-based and streamed: both inputs are walked in
/// lockstep, lines starting with `[ba3 <command> |` (after the
/// `@Comment:` prefix) are skipped on either side, and the function
/// returns `false` at the first mismatch. No intermediate string is
/// materialized.
pub(crate) fn is_provenance_only_difference(
    old_text: &str,
    new_text: &str,
    command: ReleasedCommand,
) -> bool {
    // A non-difference is not a provenance-only difference. The caller's
    // contract is "should I suppress this write?", and writing identical
    // bytes is already a no-op the OS will short-circuit; we don't need
    // to claim ownership of that case.
    if old_text == new_text {
        return false;
    }
    let prefix = format!("[ba3 {} |", command.as_str());
    let mut old_lines = old_text.split_inclusive('\n');
    let mut new_lines = new_text.split_inclusive('\n');
    loop {
        let old = next_non_provenance_line(&mut old_lines, &prefix);
        let new = next_non_provenance_line(&mut new_lines, &prefix);
        if old != new {
            return false;
        }
        if old.is_none() {
            // Both streams ended on identical non-provenance content.
            return true;
        }
    }
}

/// Advance `lines` past any `@Comment:\t[ba3 <prefix>` lines and return
/// the next non-skipped chunk. Returns `None` once the iterator is
/// exhausted. Each chunk includes its own line terminator (from
/// `split_inclusive`), so terminator differences propagate naturally —
/// we are not normalizing newlines.
fn next_non_provenance_line<'a>(
    lines: &mut std::str::SplitInclusive<'a, char>,
    prefix: &str,
) -> Option<&'a str> {
    for chunk in lines.by_ref() {
        let trimmed = chunk.trim_start();
        if let Some(rest) = trimmed.strip_prefix("@Comment:") {
            if rest.trim_start().starts_with(prefix) {
                continue;
            }
        }
        return Some(chunk);
    }
    None
}

// ---------------------------------------------------------------------------
// Extraction — parse provenance comments from CHAT text
// ---------------------------------------------------------------------------

/// One extracted provenance entry from a CHAT file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[cfg_attr(feature = "server", derive(utoipa::ToSchema))]
pub struct ProvenanceEntry {
    /// Command name (e.g., "morphotag", "align").
    pub command: String,
    /// Key-value fields (engine, lang, etc.).
    pub fields: BTreeMap<String, String>,
    /// ISO 8601 timestamp string.
    pub timestamp: String,
}

/// Extract all batchalign3 provenance entries from CHAT text.
///
/// Scans for `@Comment:` lines containing `[ba3 ...]` and parses them
/// into structured entries. Returns an empty vec if no provenance found.
pub fn extract_provenance(chat_text: &str) -> Vec<ProvenanceEntry> {
    let mut entries = Vec::new();
    for line in chat_text.lines() {
        let line = line.trim();
        if !line.starts_with("@Comment:") {
            continue;
        }
        let content = match line.split_once('\t') {
            Some((_, content)) => content.trim(),
            None => continue,
        };
        if !content.starts_with("[ba3 ") || !content.ends_with(']') {
            continue;
        }
        // Parse: [ba3 <command> | <fields> | <timestamp>]
        let inner = &content[5..content.len() - 1]; // strip "[ba3 " and "]"
        let parts: Vec<&str> = inner.splitn(3, " | ").collect();
        match parts.len() {
            2 => {
                // [ba3 command | timestamp] (no fields)
                entries.push(ProvenanceEntry {
                    command: parts[0].trim().to_string(),
                    fields: BTreeMap::new(),
                    timestamp: parts[1].trim().to_string(),
                });
            }
            3 => {
                // [ba3 command | fields | timestamp]
                let fields = parts[1]
                    .split(" ; ")
                    .filter_map(|kv| {
                        let (k, v) = kv.split_once('=')?;
                        Some((k.trim().to_string(), v.trim().to_string()))
                    })
                    .collect();
                entries.push(ProvenanceEntry {
                    command: parts[0].trim().to_string(),
                    fields,
                    timestamp: parts[2].trim().to_string(),
                });
            }
            _ => continue,
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_morphotag_provenance() {
        let comment = ProvenanceComment::new("morphotag")
            .field("engine", "stanza-1.11.1")
            .field("lang", "eng");
        let formatted = comment.format();
        assert!(formatted.starts_with("[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | "));
        assert!(formatted.ends_with(']'));
    }

    #[test]
    fn format_empty_fields() {
        let comment = ProvenanceComment::new("test");
        let formatted = comment.format();
        assert!(formatted.starts_with("[ba3 test | "));
        assert!(formatted.ends_with(']'));
    }

    #[test]
    fn inject_replaces_existing_comment_for_same_command() {
        let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Comment:\t[ba3 morphotag | engine=stanza-1.10.0 ; lang=eng | 2026-03-28T10:00:00-04:00]
*PAR:\thello .
@End
";
        let new_comment = ProvenanceComment::new("morphotag")
            .field("engine", "stanza-1.11.1")
            .field("lang", "eng");

        let result = inject_provenance_into_text(chat, &new_comment);

        // Old comment should be gone
        assert!(!result.contains("stanza-1.10.0"));
        // New comment should be present
        assert!(result.contains("stanza-1.11.1"));
        // Should have exactly one ba3 morphotag comment
        assert_eq!(
            result.matches("[ba3 morphotag").count(),
            1,
            "should have exactly one morphotag provenance comment"
        );
    }

    #[test]
    fn inject_preserves_comments_from_other_commands() {
        let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Comment:\t[ba3 transcribe | asr=whisper ; lang=eng | 2026-03-28T10:00:00-04:00]
*PAR:\thello .
@End
";
        let morphotag_comment = ProvenanceComment::new("morphotag")
            .field("engine", "stanza-1.11.1")
            .field("lang", "eng");

        let result = inject_provenance_into_text(chat, &morphotag_comment);

        // Transcribe comment should survive
        assert!(result.contains("[ba3 transcribe"));
        // Morphotag comment should be added
        assert!(result.contains("[ba3 morphotag"));
    }

    #[test]
    fn extract_provenance_from_chat() {
        let chat = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]
@Comment:\t[ba3 align | fa=whisper-fa-large-v2 ; lang=eng | 2026-03-29T19:15:00-04:00]
@Comment:\tThis is a regular user comment
*PAR:\thello .
@End
";
        let entries = extract_provenance(chat);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].command, "morphotag");
        assert_eq!(entries[0].fields["engine"], "stanza-1.11.1");
        assert_eq!(entries[0].timestamp, "2026-03-29T18:30:00-04:00");
        assert_eq!(entries[1].command, "align");
        assert_eq!(entries[1].fields["fa"], "whisper-fa-large-v2");
    }

    #[test]
    fn extract_provenance_ignores_non_ba3_comments() {
        let chat = "@Comment:\tBatchalign 0.1.0, ASR Engine rev.\n";
        let entries = extract_provenance(chat);
        assert!(entries.is_empty());
    }

    #[test]
    fn field_if_omits_false_values() {
        let comment = ProvenanceComment::new("morphotag")
            .field("engine", "stanza-1.11.1")
            .field_if("retokenize", false)
            .field_if("incremental", true);
        let formatted = comment.format();
        assert!(!formatted.contains("retokenize"));
        assert!(formatted.contains("incremental=true"));
    }

    // ---- is_provenance_only_difference tests ----
    //
    // The decision predicate that lets the runner skip a disk write when
    // re-running a provenance-injecting command would only change the
    // [ba3 <cmd> | ...] @Comment line (timestamp, version, etc.) without
    // changing any other content. Each test pins one shape of difference
    // we care about.

    use crate::api::ReleasedCommand;

    /// Re-running morphotag against an unchanged corpus produces a
    /// candidate text that differs from the on-disk text only in the
    /// timestamp slot of the [ba3 morphotag | ...] line. The predicate
    /// must recognize this as a "do not write" condition.
    #[test]
    fn provenance_only_diff_detects_timestamp_only_change() {
        let old_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]
*PAR:\thello .
%mor:\tco|hello .
@End
";
        let new_text = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]
*PAR:\thello .
%mor:\tco|hello .
@End
";
        assert!(is_provenance_only_difference(
            old_text,
            new_text,
            ReleasedCommand::Morphotag,
        ));
    }

    /// The bug we hit on 2026-05-08: workers fell back to writing
    /// `engine=stanza-stanza` instead of `engine=stanza-1.11.1`. Even
    /// after that's fixed, the predicate must not flag this as a
    /// non-writeable diff if the engine slot changes — because the
    /// new candidate's correct engine name IS information worth writing.
    /// On the other hand, if both old and new have the SAME (correct)
    /// engine slot and only the timestamp moved, that's the "skip" case.
    /// This test pins the shape of the latter.
    #[test]
    fn provenance_only_diff_detects_same_engine_only_timestamp_change() {
        let old_text = "@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]\n*PAR:\thello .\n";
        let new_text = "@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]\n*PAR:\thello .\n";
        assert!(is_provenance_only_difference(
            old_text,
            new_text,
            ReleasedCommand::Morphotag,
        ));
    }

    /// Real %mor content change must NOT be classified as
    /// provenance-only. The predicate's whole purpose is to preserve
    /// real updates while suppressing pointless ones.
    #[test]
    fn provenance_only_diff_returns_false_for_mor_change() {
        let old_text = "\
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]
*PAR:\thello .
%mor:\tco|hello .
";
        let new_text = "\
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]
*PAR:\thello .
%mor:\tco|hi .
";
        assert!(!is_provenance_only_difference(
            old_text,
            new_text,
            ReleasedCommand::Morphotag,
        ));
    }

    /// Identical input should never be flagged — there is literally no
    /// difference to suppress. The predicate is a "diff-only-in-X"
    /// detector, not a "skip the write because everything matches"
    /// shortcut (the caller can byte-compare for that).
    #[test]
    fn provenance_only_diff_returns_false_for_identical_text() {
        let text = "@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]\n*PAR:\thello .\n";
        assert!(!is_provenance_only_difference(
            text,
            text,
            ReleasedCommand::Morphotag,
        ));
    }

    /// When checking whether morphotag would write pointlessly, only the
    /// morphotag provenance line is allowed to differ. A diff in the
    /// align provenance line is a real change from morphotag's
    /// perspective and must not be hidden — otherwise re-running
    /// morphotag could clobber an unrelated align update on disk.
    #[test]
    fn provenance_only_diff_returns_false_when_other_commands_provenance_differs() {
        let old_text = "\
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-03-29T18:30:00-04:00]
@Comment:\t[ba3 align | fa=whisper-fa-large-v2 ; lang=eng | 2026-03-29T19:15:00-04:00]
*PAR:\thello .
";
        let new_text = "\
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]
@Comment:\t[ba3 align | fa=whisper-fa-large-v2 ; lang=eng | 2026-05-08T03:00:00-04:00]
*PAR:\thello .
";
        assert!(!is_provenance_only_difference(
            old_text,
            new_text,
            ReleasedCommand::Morphotag,
        ));
    }

    /// First-run case: file had no morphotag provenance before, and
    /// running morphotag now adds both the [ba3 morphotag] header AND
    /// fresh %mor tiers. The %mor addition is a real content change;
    /// the predicate must return false even though one side has no
    /// morphotag provenance to strip.
    #[test]
    fn provenance_only_diff_returns_false_on_first_run_with_real_content_added() {
        let old_text = "\
@UTF8
*PAR:\thello .
@End
";
        let new_text = "\
@UTF8
@Comment:\t[ba3 morphotag | engine=stanza-1.11.1 ; lang=eng | 2026-05-08T02:52:17-04:00]
*PAR:\thello .
%mor:\tco|hello .
@End
";
        assert!(!is_provenance_only_difference(
            old_text,
            new_text,
            ReleasedCommand::Morphotag,
        ));
    }
}

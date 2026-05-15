//! Generate CHECK parity audit report.
//!
//! Compares CLAN CHECK rules in ~/OSX-CLAN/CHECK-rules.md against TalkBank
//! error codes and emits docs/audits/check-parity-audit.md.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use talkbank_parser_tests::test_error::TestError;

static ROW_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\|\s*(\d+)\s*\|\s*([^|]+)\|.*$"#).expect("valid regex"));

static CODE_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*#\[code\("([EW]\d{3})"\)\]"#).expect("valid regex"));

static VARIANT_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*([A-Za-z][A-Za-z0-9_]*)\b"#).expect("valid regex"));

/// Data container for CheckRule.
#[derive(Clone, Debug)]
struct CheckRule {
    id: u16,
    message: String,
    category: String,
}

/// Data container for ErrorCodeInfo.
#[derive(Clone, Debug)]
struct ErrorCodeInfo {
    code: String,
    variant: String,
    deprecated: bool,
}

/// Data container for MappingResult.
#[derive(Clone, Debug)]
struct MappingResult {
    check: CheckRule,
    talkbank_codes: Vec<String>,
    semantic_parity: &'static str,
    behavioral_parity: &'static str,
    strictness_delta: &'static str,
    divergence_type: &'static str,
    rationale: String,
    action: &'static str,
    priority: &'static str,
}

/// Entry point for this binary target.
fn main() -> Result<(), TestError> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| TestError::Failure("Cannot resolve repo root".to_string()))?
        .to_path_buf();

    // Look for OSX-CLAN in workspace (../OSX-CLAN/) or legacy home path (~/OSX-CLAN/)
    let check_path = std::env::var("CLAN_SOURCE_DIR")
        .map(|d| format!("{d}/CHECK-rules.md"))
        .unwrap_or_else(|_| {
            let workspace = repo_root.parent().unwrap_or(&repo_root);
            let workspace_path = workspace.join("OSX-CLAN/CHECK-rules.md");
            if workspace_path.exists() {
                workspace_path.to_string_lossy().into_owned()
            } else {
                shellexpand::tilde("~/OSX-CLAN/CHECK-rules.md").into_owned()
            }
        });
    let check_rules = parse_check_rules(Path::new(&check_path))?;
    let talkbank_codes = parse_talkbank_codes(
        &repo_root.join("crates/talkbank-model/src/errors/codes/error_code.rs"),
    )?;

    let talkbank_set: HashSet<String> = talkbank_codes.iter().map(|c| c.code.clone()).collect();
    let mut mappings: Vec<MappingResult> = Vec::new();
    for rule in check_rules {
        mappings.push(map_rule(rule, &talkbank_set));
    }

    let report = render_report(&mappings, &talkbank_codes);
    let out_path = repo_root.join("docs/audits/check-parity-audit.md");
    fs::write(&out_path, report)?;

    println!(
        "Wrote CHECK parity audit for {} CHECK rules and {} TalkBank codes to {}",
        mappings.len(),
        talkbank_codes.len(),
        out_path.display()
    );
    Ok(())
}

/// Parses check rules.
fn parse_check_rules(path: &Path) -> Result<Vec<CheckRule>, TestError> {
    let content = fs::read_to_string(path)?;
    let mut out = Vec::new();
    let mut current_category = String::from("Uncategorized");

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("### ") {
            current_category = rest.trim().to_string();
            continue;
        }
        if line.starts_with("## ") {
            current_category = line.trim_start_matches('#').trim().to_string();
            continue;
        }
        if let Some(caps) = ROW_RE.captures(line) {
            let id: u16 = caps
                .get(1)
                .and_then(|m| m.as_str().parse::<u16>().ok())
                .ok_or_else(|| TestError::Failure(format!("bad CHECK rule row: {line}")))?;
            let message = caps
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            out.push(CheckRule {
                id,
                message,
                category: current_category.clone(),
            });
        }
    }
    Ok(out)
}

/// Parses talkbank codes.
fn parse_talkbank_codes(path: &Path) -> Result<Vec<ErrorCodeInfo>, TestError> {
    let content = fs::read_to_string(path)?;
    let mut out = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        if let Some(caps) = CODE_ATTR_RE.captures(lines[i]) {
            let code = caps
                .get(1)
                .map(|m| m.as_str())
                .unwrap_or_default()
                .to_string();
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j >= lines.len() {
                break;
            }
            let variant_line = lines[j];
            let variant = VARIANT_NAME_RE
                .captures(variant_line)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "UnknownVariant".to_string());
            let deprecated = variant_line.contains("DEPRECATED")
                || lines.get(j + 1).is_some_and(|l| l.contains("DEPRECATED"));
            out.push(ErrorCodeInfo {
                code,
                variant,
                deprecated,
            });
            i = j;
        }
        i += 1;
    }
    Ok(out)
}

/// Maps rule.
fn map_rule(rule: CheckRule, talkbank_codes: &HashSet<String>) -> MappingResult {
    let mut mapped = map_by_id(rule.id);
    if mapped.is_empty() {
        mapped = map_by_message(&rule.message);
    }
    mapped.retain(|c| talkbank_codes.contains(c));
    mapped.sort();
    mapped.dedup();

    let anomaly = is_behavioral_anomaly(rule.id, &rule.message);
    let (
        semantic_parity,
        behavioral_parity,
        strictness_delta,
        divergence_type,
        action,
        priority,
        rationale,
    ) = if mapped.is_empty() {
        (
            "none",
            "none",
            "TalkBank looser",
            "bug-risk",
            "add rule",
            "P1",
            "No direct TalkBank error code mapping found for this CHECK rule.".to_string(),
        )
    } else if anomaly {
        (
                "full",
                "partial",
                "TalkBank stricter",
                "intentional",
                "no action",
                "P2",
                "CHECK rule is known to have counter/toggle anomaly; TalkBank should match semantic intent, not flawed literal behavior."
                    .to_string(),
            )
    } else {
        (
            "full",
            "full",
            "equal",
            "none",
            "no action",
            "P3",
            "TalkBank has overlapping validation code(s) for this CHECK rule family.".to_string(),
        )
    };

    MappingResult {
        check: rule,
        talkbank_codes: mapped,
        semantic_parity,
        behavioral_parity,
        strictness_delta,
        divergence_type,
        rationale,
        action,
        priority,
    }
}

/// Returns whether behavioral anomaly.
fn is_behavioral_anomaly(id: u16, message: &str) -> bool {
    if [22u16, 23, 24, 25, 26, 27, 117, 128, 129, 130, 131, 136, 137].contains(&id) {
        return true;
    }
    let lower = message.to_lowercase();
    lower.contains("unmatched [")
        || lower.contains("unmatched ]")
        || lower.contains("unmatched <")
        || lower.contains("unmatched >")
        || lower.contains("must be used in pairs")
}

/// Maps by id.
fn map_by_id(id: u16) -> Vec<String> {
    let codes: &[&str] = match id {
        6 => &["E501"],
        7 => &["E502"],
        18 => &["E522", "E308"],
        21 => &["E304"],
        22 => &["E345"],
        23 => &["E346"],
        24 => &["E347"],
        25 => &["E348"],
        31 => &["E305"],
        36 => &["E305"],
        38 | 47 => &["E220"],
        40 | 140 => &["E401", "E705", "E706", "E720"],
        41 | 155 => &["E212"],
        50 => &["E305"],
        51 => &["E347", "E348"],
        52 => &["E370"],
        55 | 56 => &["E231"],
        57 => &["E243"],
        60 => &["E522"],
        69 => &["E507"],
        70 => &["E253"],
        81 => &["E360"],
        82 => &["E361"],
        83 => &["E701", "E362"],
        84 => &["E704"],
        85 => &["E700"],
        89 | 90 => &["E360", "E361"],
        91 => &["E303"],
        92 | 93 | 160 | 161 => &["W210", "W211", "E243"],
        94 => &["E705", "E706", "E714", "E715", "E718", "E719", "E720"],
        110 => &["E360"],
        117 => &["E230", "E356", "E357"],
        118 => &["E360"],
        120 => &["E248"],
        121 => &["E519"],
        122 => &["E519"],
        128 => &["E345"],
        129 => &["E346"],
        130 => &["E345"],
        131 => &["E346"],
        136 | 137 => &["E242"],
        141 => &["E387", "E388", "E389"],
        142 => &["E532"],
        143 => &["E505"],
        151 => &["E370"],
        153 => &["E517"],
        156 => &["E243"],
        159 => &["E370"],
        _ => &[],
    };
    codes.iter().map(|s| (*s).to_string()).collect()
}

/// Maps by message.
fn map_by_message(message: &str) -> Vec<String> {
    let m = message.to_lowercase();
    let mut out = Vec::new();
    let rules = [
        (
            "unmatched",
            &["E230", "E231", "E242", "E345", "E346", "E356", "E357"][..],
        ),
        ("delimiter", &["E304", "E305", "E360"][..]),
        ("speaker", &["E308", "E522", "E532"][..]),
        ("participants", &["E522", "E523", "E524"][..]),
        ("@id", &["E505", "E517", "E519", "E522", "E523", "E524"][..]),
        ("language", &["E248", "E249", "E519"][..]),
        ("numbers", &["E220"][..]),
        ("bullet", &["E360", "E361", "E362", "E701", "E704"][..]),
        ("mor", &["E702", "E705", "E706", "E720"][..]),
        ("gra", &["E708", "E709", "E710", "E712", "E713", "E720"][..]),
        (
            "replacement",
            &["E208", "E387", "E388", "E389", "E390", "E391"][..],
        ),
        ("quotation", &["E242", "E341", "E372"][..]),
        ("parentheses", &["E212", "E231"][..]),
        ("space", &["W210", "W211", "E243"][..]),
    ];
    for (kw, codes) in rules {
        if m.contains(kw) {
            out.extend(codes.iter().map(|c| (*c).to_string()));
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Renders report.
fn render_report(mappings: &[MappingResult], talkbank_codes: &[ErrorCodeInfo]) -> String {
    let total_check = mappings.len();
    let overlap = mappings
        .iter()
        .filter(|m| !m.talkbank_codes.is_empty())
        .count();
    let missing = total_check - overlap;
    let semantic_full = mappings
        .iter()
        .filter(|m| m.semantic_parity == "full")
        .count();
    let behavioral_full = mappings
        .iter()
        .filter(|m| m.behavioral_parity == "full")
        .count();
    let intentional = mappings
        .iter()
        .filter(|m| m.divergence_type == "intentional")
        .count();

    let mapped_codes: BTreeSet<String> = mappings
        .iter()
        .flat_map(|m| m.talkbank_codes.iter().cloned())
        .collect();
    let enhancements: Vec<&ErrorCodeInfo> = talkbank_codes
        .iter()
        .filter(|c| !mapped_codes.contains(&c.code))
        .collect();

    let mut md = String::new();
    md.push_str("# CHECK Parity Audit (CLAN CHECK vs TalkBank)\n\n");
    md.push_str(
        "Reference: `OSX-CLAN/CHECK-rules.md` (treated as canonical CHECK behavior reference).\n\n",
    );
    md.push_str("## Executive Summary\n\n");
    md.push_str(&format!("- CHECK rules parsed: `{}`\n", total_check));
    md.push_str(&format!("- Overlap with TalkBank codes: `{}`\n", overlap));
    md.push_str(&format!(
        "- CHECK rules missing direct TalkBank mapping: `{}`\n",
        missing
    ));
    md.push_str(&format!("- Semantic parity `full`: `{}`\n", semantic_full));
    md.push_str(&format!(
        "- Behavioral parity `full`: `{}`\n",
        behavioral_full
    ));
    md.push_str(&format!(
        "- Intentional divergence (semantic full + behavioral partial due to CHECK anomalies): `{}`\n",
        intentional
    ));
    md.push_str(&format!(
        "- TalkBank enhancements beyond CHECK (no mapped CHECK rule): `{}`\n\n",
        enhancements.len()
    ));

    md.push_str("## Method\n\n");
    md.push_str("- Parsed CHECK rules from markdown error tables (`| # | Message | ... |`).\n");
    md.push_str(
        "- Mapped CHECK rules to TalkBank codes via explicit ID mapping plus keyword fallback.\n",
    );
    md.push_str("- Reported two parity dimensions:\n");
    md.push_str("  - `semantic`: intended rule meaning parity.\n");
    md.push_str("  - `behavioral`: literal CHECK runtime behavior parity (including documented anomalies).\n");
    md.push_str("- Strictness policy: TalkBank should be at least as strict semantically.\n\n");

    md.push_str("## Master Mapping (CHECK -> TalkBank)\n\n");
    md.push_str("| CHECK # | CHECK Message | Category | TalkBank Codes | Semantic | Behavioral | Strictness | Divergence | Action | Priority |\n");
    md.push_str("|---:|---|---|---|---|---|---|---|---|---|\n");
    for m in mappings {
        let codes = if m.talkbank_codes.is_empty() {
            "None".to_string()
        } else {
            m.talkbank_codes
                .iter()
                .map(|c| format!("`{}`", c))
                .collect::<Vec<_>>()
                .join(", ")
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            m.check.id,
            esc(&m.check.message),
            esc(&m.check.category),
            codes,
            m.semantic_parity,
            m.behavioral_parity,
            m.strictness_delta,
            m.divergence_type,
            m.action,
            m.priority
        ));
    }
    md.push('\n');

    md.push_str("## Gaps: CHECK Rules Missing in TalkBank\n\n");
    for m in mappings.iter().filter(|m| m.talkbank_codes.is_empty()) {
        md.push_str(&format!(
            "- CHECK `{}`: {} (`{}`) -> action: `{}` ({})\n",
            m.check.id, m.check.message, m.check.category, m.action, m.priority
        ));
    }
    md.push('\n');

    md.push_str("## Intentional Divergences (Behavioral Mismatch, Semantic Match)\n\n");
    for m in mappings
        .iter()
        .filter(|m| m.divergence_type == "intentional")
    {
        md.push_str(&format!(
            "- CHECK `{}` {} -> TalkBank {}. Rationale: {}\n",
            m.check.id,
            m.check.message,
            if m.talkbank_codes.is_empty() {
                "None".to_string()
            } else {
                m.talkbank_codes.join(", ")
            },
            m.rationale
        ));
    }
    md.push('\n');

    md.push_str("## TalkBank Enhancements Beyond CHECK\n\n");
    for c in &enhancements {
        md.push_str(&format!(
            "- `{}` `{}`{}\n",
            c.code,
            c.variant,
            if c.deprecated { " (deprecated)" } else { "" }
        ));
    }
    md.push('\n');

    md.push_str("## Reverse Mapping (TalkBank -> CHECK)\n\n");
    let mut rev: BTreeMap<String, Vec<u16>> = BTreeMap::new();
    for m in mappings {
        for c in &m.talkbank_codes {
            rev.entry(c.clone()).or_default().push(m.check.id);
        }
    }
    md.push_str("| TalkBank Code | Variant | CHECK Rules |\n");
    md.push_str("|---|---|---|\n");
    for c in talkbank_codes {
        let check_ids = rev.get(&c.code).cloned().unwrap_or_default();
        let joined = if check_ids.is_empty() {
            "None".to_string()
        } else {
            let mut ids = check_ids;
            ids.sort();
            ids.dedup();
            ids.into_iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        md.push_str(&format!(
            "| `{}` | `{}` | {} |\n",
            c.code, c.variant, joined
        ));
    }
    md.push('\n');

    md.push_str("## Priority Action Plan\n\n");
    let mut grouped: BTreeMap<&str, Vec<&MappingResult>> = BTreeMap::new();
    for m in mappings.iter().filter(|m| m.action != "no action") {
        grouped.entry(m.priority).or_default().push(m);
    }
    for p in ["P0", "P1", "P2", "P3"] {
        md.push_str(&format!("### {}\n\n", p));
        if let Some(items) = grouped.get(p) {
            for m in items {
                md.push_str(&format!(
                    "- CHECK `{}` `{}` -> {} ({}; {} parity)\n",
                    m.check.id, m.check.message, m.action, m.strictness_delta, m.semantic_parity
                ));
            }
        } else {
            md.push_str("- None\n");
        }
        md.push('\n');
    }

    md.push_str("## Notes and Caveats\n\n");
    md.push_str(
        "- This mapping is comprehensive but heuristic for rules with broad/generic wording.\n",
    );
    md.push_str("- CHECK rule anomalies from the reference doc are explicitly modeled as intentional behavioral divergences when TalkBank enforces stricter semantics.\n");
    md.push_str("- Remaining `None` mappings should be triaged manually for true coverage gaps vs non-equivalent CHECK legacy behavior.\n");

    md
}

/// Escape Markdown table cell content.
fn esc(s: &str) -> String {
    s.replace('|', "\\|")
}

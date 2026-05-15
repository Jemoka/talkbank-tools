//! Generate comprehensive error-code audit report.
//!
//! Output: docs/audits/error-code-audit.md
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use talkbank_parser_tests::test_error::TestError;
use walkdir::WalkDir;

static CODE_ATTR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*#\[code\("([EW]\d{3})"\)\]"#).expect("valid regex"));

static VARIANT_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*([A-Za-z][A-Za-z0-9_]*)\b"#).expect("valid regex"));

static VARIANT_REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"ErrorCode::([A-Za-z][A-Za-z0-9_]*)"#).expect("valid regex"));

static QUOTED_CODE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([EW]\d{3})""#).expect("valid regex"));

static QUOTED_STRING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([^"\\]*(?:\\.[^"\\]*)*)""#).expect("valid regex"));

static SUGGESTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"with_suggestion\(\s*"([^"]+)""#).expect("valid regex"));

/// Data container for CodeInfo.
#[derive(Clone, Debug)]
struct CodeInfo {
    code: String,
    variant: String,
    deprecated: bool,
}

/// Data container for RefHit.
#[derive(Clone, Debug)]
struct RefHit {
    path: String,
    line: usize,
    snippet: String,
    is_test: bool,
    is_emission: bool,
    near_ignore: bool,
    message_hint: Option<String>,
    suggestion_hint: Option<String>,
}

/// Data container for CodeRefs.
#[derive(Clone, Debug, Default)]
struct CodeRefs {
    hits: Vec<RefHit>,
}

/// Entry point for this binary target.
fn main() -> Result<(), TestError> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .ok_or_else(|| {
            TestError::Failure("Cannot resolve repo root from CARGO_MANIFEST_DIR".to_string())
        })?
        .to_path_buf();

    let enum_path = repo_root.join("crates/talkbank-model/src/errors/codes/error_code.rs");
    let codes = extract_codes(&enum_path)?;
    let variant_to_code: HashMap<String, String> = codes
        .iter()
        .map(|c| (c.variant.clone(), c.code.clone()))
        .collect();
    let code_set: BTreeSet<String> = codes.iter().map(|c| c.code.clone()).collect();

    let refs = scan_refs(&repo_root, &variant_to_code, &code_set)?;

    let report = render_report(&codes, &refs);
    let out_path = repo_root.join("docs/audits/error-code-audit.md");
    fs::write(&out_path, report)?;

    println!(
        "Wrote audit report for {} codes to {}",
        codes.len(),
        out_path.display()
    );
    Ok(())
}

/// Extracts codes.
fn extract_codes(path: &Path) -> Result<Vec<CodeInfo>, TestError> {
    let content = fs::read_to_string(path)?;

    let mut out = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        if let Some(caps) = CODE_ATTR_RE.captures(lines[i]) {
            let code = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim().is_empty() {
                j += 1;
            }
            if j >= lines.len() {
                return Err(TestError::Failure(format!(
                    "Missing variant after code {} in {}",
                    code,
                    path.display()
                )));
            }
            let variant_line = lines[j];
            let variant = VARIANT_NAME_RE
                .captures(variant_line)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .ok_or_else(|| {
                    TestError::Failure(format!(
                        "Could not parse variant line after {}: {}",
                        code, variant_line
                    ))
                })?;
            let deprecated = variant_line.contains("DEPRECATED")
                || lines.get(j + 1).is_some_and(|l| l.contains("DEPRECATED"));
            out.push(CodeInfo {
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

/// Scan repository sources for error-code references and emissions.
fn scan_refs(
    repo_root: &Path,
    variant_to_code: &HashMap<String, String>,
    code_set: &BTreeSet<String>,
) -> Result<BTreeMap<String, CodeRefs>, TestError> {
    let mut out: BTreeMap<String, CodeRefs> = BTreeMap::new();

    for entry in WalkDir::new(&repo_root).into_iter() {
        let entry = entry.map_err(|e| TestError::Failure(format!("walk error: {e}")))?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let rel = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        if rel.contains("crates/talkbank-parser-tests/src/bin/") {
            continue;
        }
        let content = fs::read_to_string(path)?;
        let lines: Vec<&str> = content.lines().collect();
        let is_test_path =
            rel.contains("/tests/") || rel.ends_with("_test.rs") || rel.ends_with("_tests.rs");

        for (idx, line) in lines.iter().enumerate() {
            let mut line_codes: Vec<String> = Vec::new();
            for cap in VARIANT_REF_RE.captures_iter(line) {
                let variant = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
                if let Some(code) = variant_to_code.get(variant) {
                    line_codes.push(code.clone());
                }
            }
            for cap in QUOTED_CODE_RE.captures_iter(line) {
                let code = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
                if code_set.contains(code) {
                    line_codes.push(code.to_string());
                }
            }
            if line_codes.is_empty() {
                continue;
            }

            let start = idx.saturating_sub(8);
            let end = (idx + 8).min(lines.len().saturating_sub(1));
            let window: Vec<&str> = lines[start..=end].to_vec();
            let window_text = window.join("\n");
            let near_ignore = lines
                .get(idx.saturating_sub(1))
                .is_some_and(|l| l.contains("#[ignore"))
                || lines
                    .get(idx.saturating_sub(2))
                    .is_some_and(|l| l.contains("#[ignore"));
            let is_emission = !is_test_path
                && (window_text.contains("ParseError::new")
                    || window_text.contains("ParseError::build")
                    || window_text.contains(".report(")
                    || window_text.contains(".with_suggestion("));

            let message_hint = if window_text.contains("ParseError::new") {
                QUOTED_STRING_RE
                    .captures_iter(&window_text)
                    .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
                    .find(|s| {
                        s.len() > 12
                            && !s.starts_with('E')
                            && !s.starts_with("http")
                            && s.contains(' ')
                    })
            } else {
                None
            };
            let suggestion_hint = SUGGESTION_RE
                .captures(&window_text)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()));

            for code in line_codes {
                out.entry(code).or_default().hits.push(RefHit {
                    path: rel.clone(),
                    line: idx + 1,
                    snippet: line.trim().to_string(),
                    is_test: is_test_path,
                    is_emission,
                    near_ignore,
                    message_hint: message_hint.clone(),
                    suggestion_hint: suggestion_hint.clone(),
                });
            }
        }
    }

    Ok(out)
}

/// Renders report.
fn render_report(codes: &[CodeInfo], refs: &BTreeMap<String, CodeRefs>) -> String {
    let mut md = String::new();
    md.push_str("# Error Code Audit Report\n\n");
    md.push_str("This report audits all `ErrorCode` variants defined in `talkbank-model`.\n");
    md.push_str("Scope: all enum codes, including deprecated and disabled/ignored paths.\n\n");

    let total = codes.len();
    let mut strong = 0usize;
    let mut partial = 0usize;
    let mut none = 0usize;
    let mut disabled = 0usize;
    let mut mismatch = 0usize;
    let mut non_user_friendly = 0usize;
    let mut weak_suggestions = 0usize;

    for info in codes {
        let analysis = analyze(info, refs.get(&info.code));
        match analysis.coverage.as_str() {
            "Strong" => strong += 1,
            "Partial" => partial += 1,
            "Disabled-only" => disabled += 1,
            _ => none += 1,
        }
        if analysis.name_message_fit != "Good fit" {
            mismatch += 1;
        }
        if analysis.user_friendly != "Good" {
            non_user_friendly += 1;
        }
        if analysis.suggestion_quality != "Sensible" {
            weak_suggestions += 1;
        }
    }

    md.push_str("## Summary\n\n");
    md.push_str(&format!("- Total codes audited: {}\n", total));
    md.push_str(&format!(
        "- Coverage: `Strong {}`, `Partial {}`, `Disabled-only {}`, `None {}`\n",
        strong, partial, disabled, none
    ));
    md.push_str(&format!(
        "- Name/message fit needing review: {}\n",
        mismatch
    ));
    md.push_str(&format!(
        "- Messages needing user-language improvement: {}\n",
        non_user_friendly
    ));
    md.push_str(&format!(
        "- Suggestions missing/weak: {}\n\n",
        weak_suggestions
    ));

    md.push_str("## Method\n\n");
    md.push_str("- Inventory source: `crates/talkbank-model/src/errors/codes/error_code.rs`\n");
    md.push_str(
        "- Emission evidence: static scan of `ParseError` creation/report sites in Rust source.\n",
    );
    md.push_str("- Coverage evidence: assertions/snapshots/corpus mentions in Rust test code.\n");
    md.push_str("- Ratings are heuristic and intended to identify manual follow-up hotspots.\n\n");

    let mut by_group: BTreeMap<String, Vec<&CodeInfo>> = BTreeMap::new();
    for c in codes {
        by_group.entry(group(&c.code)).or_default().push(c);
    }

    for (grp, items) in by_group {
        md.push_str(&format!("## {}\n\n", grp));
        md.push_str("| Code | Name | Primary Construct | Referenced Constructs | Name/Message Fit | User-Friendly Message | Suggested Fixes | Coverage |\n");
        md.push_str("|---|---|---|---|---|---|---|---|\n");
        for info in &items {
            let a = analyze(info, refs.get(&info.code));
            md.push_str(&format!(
                "| `{}`{} | `{}` | {} | {} | {} | {} | {} | {} |\n",
                info.code,
                if info.deprecated { " (deprecated)" } else { "" },
                info.variant,
                esc(&a.primary_construct),
                esc(&a.referenced_constructs),
                esc(&a.name_message_fit),
                esc(&a.user_friendly),
                esc(&a.suggestion_quality),
                esc(&a.coverage),
            ));
        }
        md.push('\n');

        for info in &items {
            let a = analyze(info, refs.get(&info.code));
            md.push_str(&format!("### `{}` `{}`\n\n", info.code, info.variant));
            md.push_str(&format!(
                "- Primary construct: {}\n- Relevant referenced constructs: {}\n- Name/message assessment: {}\n- User-language assessment: {}\n- Suggested-fix assessment: {}\n- Coverage: {}\n",
                a.primary_construct, a.referenced_constructs, a.name_message_fit, a.user_friendly, a.suggestion_quality, a.coverage
            ));
            if let Some(msg) = a.message_example {
                md.push_str(&format!("- Message example: `{}`\n", msg));
            }
            if let Some(sug) = a.suggestion_example {
                md.push_str(&format!("- Suggested fix example: `{}`\n", sug));
            }
            md.push_str(&format!("- Emission refs: {}\n", a.emission_refs));
            md.push_str(&format!("- Test refs: {}\n\n", a.test_refs));
        }
    }

    md
}

/// Analysis summary for one error code.
#[derive(Clone, Debug)]
struct Analysis {
    primary_construct: String,
    referenced_constructs: String,
    name_message_fit: String,
    user_friendly: String,
    suggestion_quality: String,
    coverage: String,
    message_example: Option<String>,
    suggestion_example: Option<String>,
    emission_refs: String,
    test_refs: String,
}

/// Analyze one error code across references and emitted diagnostics.
fn analyze(info: &CodeInfo, refs: Option<&CodeRefs>) -> Analysis {
    let empty = Vec::<RefHit>::new();
    let hits = refs.map(|r| &r.hits).unwrap_or(&empty);
    let emissions: Vec<&RefHit> = hits.iter().filter(|h| h.is_emission).collect();
    let tests: Vec<&RefHit> = hits.iter().filter(|h| h.is_test).collect();

    let primary_construct = classify_primary(&emissions);
    let referenced_constructs = classify_referenced(&emissions);
    let message_example = emissions.iter().find_map(|h| h.message_hint.clone());
    let suggestion_example = emissions.iter().find_map(|h| h.suggestion_hint.clone());
    let name_message_fit =
        assess_name_message_fit(info, message_example.as_deref(), &primary_construct);
    let user_friendly = assess_user_friendly(message_example.as_deref());
    let suggestion_quality = assess_suggestion_quality(suggestion_example.as_deref());
    let coverage = classify_coverage(&tests);
    let emission_refs = format_refs(&emissions);
    let test_refs = format_refs(&tests);

    Analysis {
        primary_construct,
        referenced_constructs,
        name_message_fit,
        user_friendly,
        suggestion_quality,
        coverage,
        message_example,
        suggestion_example,
        emission_refs,
        test_refs,
    }
}

/// Classify whether a code appears parser-, validator-, or mixed-primary.
fn classify_primary(emissions: &[&RefHit]) -> String {
    if emissions.is_empty() {
        return "No primary emission site found (manual review)".to_string();
    }
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for h in emissions {
        let c = match () {
            _ if h.path.contains("/validation/word/") || h.path.contains("/content/word/") => {
                "Word"
            }
            _ if h.path.contains("/validation/utterance/")
                || h.path.contains("/main_tier/")
                || h.path.contains("/utterance_parser")
                || h.path.contains("/validation/main_tier") =>
            {
                "Main tier / Utterance"
            }
            _ if h.path.contains("/validation/header/") || h.path.contains("/header/") => "Header",
            _ if h.path.contains("/alignment/") || h.path.contains("/validation/temporal/") => {
                "Alignment / Temporal"
            }
            _ if h.path.contains("/tier_parsers/") || h.path.contains("/tree_parsing/") => {
                "Parser CST/Tree"
            }
            // Direct parser was removed; this branch is retained for auditing historical data
            _ => "Generic/Internal",
        };
        *counts.entry(c).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k.to_string())
        .unwrap_or_else(|| "Generic/Internal".to_string())
}

/// Classify which constructs an error code appears to reference.
fn classify_referenced(emissions: &[&RefHit]) -> String {
    let mut keys = BTreeSet::new();
    for h in emissions {
        let s = format!("{} {}", h.path.to_lowercase(), h.snippet.to_lowercase());
        for (kw, label) in [
            ("replacement", "replacement annotations"),
            ("overlap", "overlap markers"),
            ("quotation", "quotation/linkers"),
            ("underline", "underline markers"),
            ("bullet", "media bullets/timestamps"),
            ("timestamp", "timestamps"),
            ("speaker", "speaker codes"),
            ("participant", "participants"),
            ("@id", "ID headers"),
            ("mor", "%mor alignment"),
            ("gra", "%gra alignment"),
            ("pho", "%pho alignment"),
            ("sin", "%sin alignment"),
            ("wor", "%wor alignment"),
            ("retrace", "retracing"),
            ("delimiter", "CA delimiters"),
            ("shortening", "shortening markers"),
            ("compound", "compound markers"),
            ("language", "language metadata"),
            ("form", "form-type markers"),
            ("annotation", "scoped annotations"),
        ] {
            if s.contains(kw) {
                keys.insert(label.to_string());
            }
        }
    }
    if keys.is_empty() {
        "None identified from static references".to_string()
    } else {
        keys.into_iter().collect::<Vec<_>>().join(", ")
    }
}

/// Assess whether variant naming and emitted messages align.
fn assess_name_message_fit(
    info: &CodeInfo,
    message: Option<&str>,
    primary_construct: &str,
) -> String {
    if info.deprecated {
        return "Deprecated code; verify no active user-facing emission".to_string();
    }
    let Some(m) = message else {
        return "No message found at emission site (manual review needed)".to_string();
    };
    let lower = m.to_lowercase();
    if lower.contains("node")
        || lower.contains("cst")
        || lower.contains("tree")
        || lower.contains("parser helper")
    {
        return "Potential mismatch: message uses implementation terms".to_string();
    }
    if primary_construct.contains("Word")
        && (lower.contains("header") || lower.contains("participant"))
    {
        return "Potential mismatch with primary construct".to_string();
    }
    if primary_construct.contains("Header") && lower.contains("word") && !lower.contains("@") {
        return "Potential mismatch with primary construct".to_string();
    }
    "Good fit".to_string()
}

/// Assess end-user readability of emitted messages.
fn assess_user_friendly(message: Option<&str>) -> String {
    let Some(m) = message else {
        return "No message evidence".to_string();
    };
    let lower = m.to_lowercase();
    if lower.contains("node")
        || lower.contains("cst")
        || lower.contains("ast")
        || lower.contains("unexpected child")
        || lower.contains("span")
        || lower.contains("offset")
    {
        return "Needs improvement (internal implementation wording)".to_string();
    }
    if m.len() < 12 {
        return "Needs improvement (too terse)".to_string();
    }
    "Good".to_string()
}

/// Assess quality of emitted suggested-fix text.
fn assess_suggestion_quality(suggestion: Option<&str>) -> String {
    let Some(s) = suggestion else {
        return "Missing".to_string();
    };
    let lower = s.to_lowercase();
    if lower.contains("future version") || lower.contains("not yet supported") {
        return "Weak/placeholder".to_string();
    }
    if [
        "add ", "remove ", "use ", "ensure ", "provide ", "replace ", "move ",
    ]
    .iter()
    .any(|v| lower.contains(v))
    {
        return "Sensible".to_string();
    }
    "Needs review".to_string()
}

/// Classify estimated test coverage strength for this code.
fn classify_coverage(tests: &[&RefHit]) -> String {
    if tests.is_empty() {
        return "None".to_string();
    }
    let has_ignore = tests.iter().all(|h| h.near_ignore);
    if has_ignore {
        return "Disabled-only".to_string();
    }
    let strong = tests.iter().any(|h| {
        let s = h.snippet.to_lowercase();
        s.contains("assert")
            || s.contains("expected e")
            || s.contains(".code.as_str() ==")
            || s.contains("errorcode::")
    });
    if strong {
        "Strong".to_string()
    } else {
        "Partial".to_string()
    }
}

/// Formats refs.
fn format_refs(hits: &[&RefHit]) -> String {
    if hits.is_empty() {
        return "None".to_string();
    }
    let mut uniq = BTreeSet::new();
    for h in hits.iter().take(6) {
        uniq.insert(format!("`{}:{}`", h.path, h.line));
    }
    uniq.into_iter().collect::<Vec<_>>().join(", ")
}

/// Group an error code by numeric family for report sections.
fn group(code: &str) -> String {
    if code.starts_with('W') {
        return "Wxxx Warnings".to_string();
    }
    let cat = code.chars().nth(1).unwrap_or('9');
    match cat {
        '0' | '1' => "E0xx-E1xx Internal/System".to_string(),
        '2' => "E2xx Word".to_string(),
        '3' => "E3xx Parser/Main-tier".to_string(),
        '4' => "E4xx Dependent-tier".to_string(),
        '5' => "E5xx Header/Metadata".to_string(),
        '6' => "E6xx Tier Validation".to_string(),
        '7' => "E7xx Alignment/Temporal".to_string(),
        _ => "Other".to_string(),
    }
}

/// Escape Markdown table cell content.
fn esc(s: &str) -> String {
    s.replace('|', "\\|")
}

//! Dead enum-variant audit.
//!
//! The model crate's parser/builder code is the canonical producer of
//! its data types. Any enum variant defined there but never constructed
//! anywhere in the workspace's non-test code is dead.
//!
//! Detection is the same shape as the predecessor Python script
//! `scripts/find_dead_model_variants.py`, but ported into xtask so we
//! can:
//!   - Share workspace-`.rs` walking with `wide_struct_audit` (both
//!     consume `crate::rust_scan`).
//!   - Replace the per-variant `rg` invocations (~1250 spawns on
//!     talkbank-model) with a single in-process scan over all `.rs`
//!     files. The scan looks for any `Qualifier::Identifier`
//!     occurrence and indexes by `(qualifier, identifier)`. Every
//!     enum variant looks itself up at zero search cost.
//!
//! Detection covers:
//!   - `EnumName::Variant` full-path (anywhere in search root, non-test).
//!   - `Self::Variant` restricted to the enum's home file (in-impl
//!     construction). The home-file restriction is critical: `Self::X`
//!     in some other type's impl refers to a different type.
//!   - thiserror `#[from]` on a tuple field — auto-implements
//!     `From<T> for EnumName`, so the variant is constructed implicitly.
//!   - Enums whose `#[derive(...)]` includes a derive that builds
//!     variants from external input (`Subcommand`, `ValueEnum`, `Args`,
//!     `Parser`, `Deserialize`, `JsonSchema`, `EnumString`, `FromRepr`).
//!     All variants of those enums are skipped from the audit.
//!
//! Negative shapes (a match for `Foo::Bar` that doesn't count as a
//! constructor):
//!   - Pattern-match arm: `Foo::Bar(...)` followed by `=>` on the same
//!     line.
//!   - Markdown rustdoc link: `[`Foo::Bar`]` (the backtick
//!     disambiguates from a Rust slice literal `[Foo::A, Foo::B]`,
//!     which IS a valid constructor context).
//!   - `use` import lines.
//!   - Doc-comment lines (`///` / `//!`).
//!   - `matches!(...)` invocations.
//!
//! Limitations: macro-generated constructors other than thiserror
//! `#[from]` and the listed input-driven derives are not detected. A
//! `Default` impl outside the enum's home file that constructs a
//! variant is not seen.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::Result;
use crate::rust_scan::{brace_delta, is_test_path, walkdir};

/// Derives that construct variants from external input. When ANY of
/// these is on the enum's `#[derive(...)]` list, ALL of its variants
/// are reachable via the macro-generated path even if no source line
/// names them.
const EXTERNAL_INPUT_DERIVES: &[&str] = &[
    "Subcommand",
    "ValueEnum",
    "Args",
    "Parser",
    "Deserialize",
    "JsonSchema",
    "EnumString",
    "FromRepr",
];

#[derive(Debug, Clone)]
struct EnumInfo {
    /// Fully-qualified-by-context type name as written.
    name: String,
    /// Variant identifiers in declaration order.
    variants: Vec<String>,
    /// Path the enum is defined in, relative to the search root.
    home_path: String,
    /// Variants with `#[from]` on a tuple field (thiserror auto-From).
    from_variants: BTreeSet<String>,
    /// True when a derive on the enum constructs variants from input.
    externally_constructible: bool,
}

#[derive(Debug, Clone)]
struct MatchEntry {
    /// Path relative to search root. Used to gate `Self::Variant`
    /// matches to the enum's home file (Self in another file refers
    /// to a different type).
    path: String,
    /// Original line text. The constructor heuristic operates on the
    /// full line, not just the variant token.
    content: String,
    /// True when the matched qualifier was `Self` (vs the enum name).
    via_self: bool,
}

/// Walk `model_root` (the search root, scoped to its model crate(s))
/// and collect every `pub enum`'s variants. Returns enums by name.
fn find_enums(search_root: &Path, model_root: &Path) -> Vec<EnumInfo> {
    let mut out: Vec<EnumInfo> = Vec::new();
    for path in walkdir(model_root) {
        let relative = match path.strip_prefix(search_root) {
            Ok(p) => p.to_string_lossy().into_owned(),
            Err(_) => continue,
        };
        if is_test_path(&relative) {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        out.extend(parse_enums_in_file(&relative, &text));
    }
    out.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.home_path.cmp(&right.home_path))
    });
    out
}

/// Parse `pub enum` declarations and their bodies in one source file.
fn parse_enums_in_file(relative_path: &str, text: &str) -> Vec<EnumInfo> {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim_start();
        let Some(name) = enum_name_from_declaration(trimmed) else {
            index += 1;
            continue;
        };
        // Look back through the immediately-preceding lines to find any
        // `#[derive(...)]` blocks (multiple `#[derive]` lines and
        // `#[cfg_attr(...)]`-gated derives are common).
        let externally_constructible = preceding_derives_have_external_input(&lines, index);

        // Walk the enum body, accumulating variant identifiers and
        // detecting `#[from]` attributes.
        //
        // We track BOTH brace depth (`{` / `}`) and paren depth
        // (`(` / `)`). Variant names appear only at the top level of
        // the enum body — `brace_depth == 1` AND `paren_depth == 0`.
        // The paren guard catches multi-line `#[error("..." ... )]`
        // attributes whose continuation lines may begin with
        // CamelCase words (e.g. `"This guard..."`, `"Set FOO..."`)
        // that look like variants but are payload-string text.
        let mut brace_depth = brace_delta(lines[index]);
        let mut paren_depth: isize = 0;
        let mut variants: Vec<String> = Vec::new();
        let mut from_variants: BTreeSet<String> = BTreeSet::new();
        let mut pending_from_attr = false;
        index += 1;
        while index < lines.len() && brace_depth > 0 {
            let raw = lines[index];
            let line = raw.trim();
            if brace_depth == 1 && paren_depth == 0 {
                if line.contains("#[from]") {
                    pending_from_attr = true;
                }
                if let Some(variant_name) = extract_variant_name(line) {
                    variants.push(variant_name.clone());
                    if pending_from_attr {
                        from_variants.insert(variant_name);
                    }
                    pending_from_attr = false;
                }
            }
            brace_depth += brace_delta(raw);
            paren_depth += paren_delta(raw);
            index += 1;
        }

        if !variants.is_empty() {
            result.push(EnumInfo {
                name,
                variants,
                home_path: relative_path.to_string(),
                from_variants,
                externally_constructible,
            });
        }
    }
    result
}

/// Net change in `(`/`)` count on one line. Sibling of
/// `rust_scan::brace_delta` for parentheses; used to skip continuation
/// lines of multi-line attributes like `#[error("..." ... )]`.
fn paren_delta(line: &str) -> isize {
    line.chars().fold(0isize, |delta, ch| match ch {
        '(' => delta + 1,
        ')' => delta - 1,
        _ => delta,
    })
}

/// Match `pub enum Name {` (with optional generics like `<T>`).
fn enum_name_from_declaration(line: &str) -> Option<String> {
    let declaration = line.strip_prefix("pub enum ")?;
    if !declaration.contains('{') {
        return None;
    }
    let name = declaration.split('{').next()?.trim();
    let name = name.split('<').next()?.trim();
    let name = name.split_whitespace().next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Extract a variant identifier from a single trimmed line.
///
/// Recognized shapes:
///   `Variant,`
///   `Variant { ... },` or just `Variant {` (struct variant header)
///   `Variant(...),` or just `Variant(` (tuple variant header)
///   `#[serde(rename = "...")]\n    Variant,` (attribute on prior line)
fn extract_variant_name(line: &str) -> Option<String> {
    if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
        return None;
    }
    // Skip lines that are inside a struct-variant body (start with a
    // field name + colon) or are random tokens.
    let first = line
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>();
    if first.is_empty() {
        return None;
    }
    let first_char = first.chars().next()?;
    if !first_char.is_uppercase() {
        return None;
    }
    let after = &line[first.len()..];
    let next_char = after.chars().next();
    match next_char {
        Some(',') | Some('(') | Some('{') | None => Some(first),
        Some(c) if c.is_whitespace() => Some(first),
        _ => None,
    }
}

/// Walk a few lines preceding the enum declaration looking for
/// `#[derive(...)]` clauses. Return true if any derive name matches
/// `EXTERNAL_INPUT_DERIVES`. Path-qualified names (`serde::Deserialize`)
/// are normalized to the bare identifier before matching.
fn preceding_derives_have_external_input(lines: &[&str], enum_decl_index: usize) -> bool {
    // Step back through immediately-adjacent attribute / blank lines.
    let mut idx = enum_decl_index;
    while idx > 0 {
        idx -= 1;
        let line = lines[idx].trim_start();
        if line.is_empty() || line.starts_with("///") || line.starts_with("//!") {
            continue;
        }
        if line.starts_with("#[") || line.starts_with("#![") {
            // Look for derive clauses on this line.
            if let Some(start) = line.find("derive") {
                // Find the parenthesized list following `derive`.
                let after = &line[start..];
                if let Some(open) = after.find('(') {
                    if let Some(close) = after[open + 1..].find(')') {
                        let derive_list = &after[open + 1..open + 1 + close];
                        if derive_list.split(',').any(|d| {
                            let last = d.trim().rsplit("::").next().unwrap_or("").trim();
                            EXTERNAL_INPUT_DERIVES.contains(&last)
                        }) {
                            return true;
                        }
                    }
                }
            }
            continue;
        }
        // Hit a non-attribute line; stop.
        break;
    }
    false
}

/// One pass over every `.rs` file in the search root, indexing every
/// `Qualifier::Identifier` occurrence into a hash table. This replaces
/// the per-variant ripgrep approach: instead of N searches, one walk.
fn build_constructor_index(
    search_root: &Path,
    enum_names: &HashSet<String>,
) -> HashMap<(String, String), Vec<MatchEntry>> {
    let mut index: HashMap<(String, String), Vec<MatchEntry>> = HashMap::new();
    for path in walkdir(search_root) {
        let relative = match path.strip_prefix(search_root) {
            Ok(p) => p.to_string_lossy().into_owned(),
            Err(_) => continue,
        };
        if is_test_path(&relative) {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => continue,
        };
        for line in text.lines() {
            scan_line_for_paths(line, &relative, enum_names, &mut index);
        }
    }
    index
}

/// Find every `Qualifier::Identifier` in a line and, when the
/// qualifier matches a known enum name (or is `Self`), append a
/// `MatchEntry`. We do this with a manual char scan rather than regex
/// for clarity and to avoid the regex dependency.
fn scan_line_for_paths(
    line: &str,
    relative_path: &str,
    enum_names: &HashSet<String>,
    index: &mut HashMap<(String, String), Vec<MatchEntry>>,
) {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        // Look for `::`.
        if bytes[i] == b':' && bytes[i + 1] == b':' {
            // Walk back to find the qualifier identifier.
            let mut q_end = i;
            while q_end > 0 && is_ident_byte(bytes[q_end - 1]) {
                q_end -= 1;
            }
            let qualifier = &line[q_end..i];
            // Walk forward to find the variant identifier.
            let v_start = i + 2;
            let mut v_end = v_start;
            while v_end < bytes.len() && is_ident_byte(bytes[v_end]) {
                v_end += 1;
            }
            if !qualifier.is_empty() && v_end > v_start {
                let variant = &line[v_start..v_end];
                let first_char = variant.chars().next().unwrap_or(' ');
                if first_char.is_ascii_uppercase() {
                    let via_self = qualifier == "Self";
                    if via_self || enum_names.contains(qualifier) {
                        let key = if via_self {
                            // Self::Variant — qualifier resolved later
                            // by the home-file check.
                            ("Self".to_string(), variant.to_string())
                        } else {
                            (qualifier.to_string(), variant.to_string())
                        };
                        index.entry(key).or_default().push(MatchEntry {
                            path: relative_path.to_string(),
                            content: line.to_string(),
                            via_self,
                        });
                    }
                }
            }
            i = v_end.max(i + 2);
        } else {
            i += 1;
        }
    }
}

fn is_ident_byte(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
}

/// Classify a single match line: is this a CONSTRUCTOR (or other
/// live use) or a non-constructor reference (pattern arm, doc link,
/// `use`, doc comment, `matches!`)?
fn is_constructor(content: &str, qualifier: &str, variant: &str) -> bool {
    let trimmed = content.trim_start();
    if trimmed.starts_with("///") || trimmed.starts_with("//!") {
        return false;
    }
    // Strip a trailing line comment.
    let s = match content.find("//") {
        Some(idx) => content[..idx].trim(),
        None => content.trim(),
    };
    if s.is_empty() {
        return false;
    }
    if s.contains("matches!") {
        return false;
    }
    if s.starts_with("use ") || s.starts_with("pub use ") {
        return false;
    }

    let needle = format!("{qualifier}::{variant}");
    let Some(idx) = s.find(&needle) else {
        return false;
    };
    let head = &s[..idx];
    // Markdown rustdoc link: `[`Foo::Bar`]` — backtick
    // disambiguates from a Rust slice literal `[Foo::A, ...]`.
    if head.ends_with("[`") {
        return false;
    }
    let tail = &s[idx + needle.len()..];
    if tail.contains("=>") {
        return false;
    }
    true
}

/// Whether any of the entries in `entries` is a constructor for
/// `(qualifier, variant)`. For Self:: matches, also checks that the
/// match is in the enum's home file.
fn has_constructor(entries: &[MatchEntry], enum_info: &EnumInfo, variant: &str) -> bool {
    entries.iter().any(|entry| {
        let qualifier = if entry.via_self {
            "Self"
        } else {
            &enum_info.name
        };
        if entry.via_self && entry.path != enum_info.home_path {
            return false;
        }
        is_constructor(&entry.content, qualifier, variant)
    })
}

fn render_report(
    label: &str,
    model_root: &Path,
    search_root: &Path,
    enums: &[EnumInfo],
    index: &HashMap<(String, String), Vec<MatchEntry>>,
) -> (String, ReportCounts) {
    let mut out = String::new();
    let mut counts = ReportCounts::default();
    counts.enums_total = enums.len();

    out.push_str(&format!("# Dead Variant Audit — `{label}`\n\n"));
    out.push_str("Generated by `cargo run -q -p xtask -- lint-dead-variants`.\n\n");
    out.push_str(&format!(
        "**Method.** Walks `{}` for every `pub enum` and indexes every \
         `Qualifier::Identifier` occurrence in `{}` (non-test code). A \
         variant is flagged dead when it has no explicit constructor \
         (`EnumName::Variant` / `Self::Variant`), no thiserror `#[from]` \
         auto-constructor, and no other live reference outside \
         pattern-match arms / doc links / `use` statements.\n\n",
        model_root.display(),
        search_root.display()
    ));
    out.push_str(
        "**Limitation.** Macro-generated constructors other than \
         thiserror `#[from]` and clap/serde/strum derive families are \
         not detected. Inspect manually before removing.\n",
    );

    for enum_info in enums {
        counts.variants_total += enum_info.variants.len();
        if enum_info.externally_constructible {
            counts.input_derive_skipped += enum_info.variants.len();
            continue;
        }
        out.push_str(&format!(
            "\n## `{}` ({} variants)\n\n",
            enum_info.name,
            enum_info.variants.len()
        ));
        let mut any_dead = false;
        for variant in &enum_info.variants {
            if enum_info.from_variants.contains(variant) {
                counts.from_skipped += 1;
                continue;
            }
            // Combine full-path matches and Self:: matches (latter
            // restricted to home file inside has_constructor).
            let mut entries: Vec<MatchEntry> = Vec::new();
            if let Some(found) = index.get(&(enum_info.name.clone(), variant.clone())) {
                entries.extend(found.iter().cloned());
            }
            if let Some(found) = index.get(&("Self".to_string(), variant.clone())) {
                entries.extend(
                    found
                        .iter()
                        .filter(|e| e.path == enum_info.home_path)
                        .cloned(),
                );
            }
            let live = has_constructor(&entries, enum_info, variant);
            if !live {
                let inert_refs = entries.len();
                if inert_refs == 0 {
                    out.push_str(&format!(
                        "- ☠️ **`{}`** — DEAD (zero non-test references)\n",
                        variant
                    ));
                } else {
                    out.push_str(&format!(
                        "- ☠️ **`{}`** — DEAD ({} ref(s), all pattern-match arms / doc links / use stmts; no constructor)\n",
                        variant, inert_refs
                    ));
                }
                counts.dead += 1;
                any_dead = true;
            }
        }
        if !any_dead {
            out.push_str("(all variants have constructors)\n");
        }
    }

    out.push_str("\n## Summary\n\n");
    out.push_str(&format!("- Enums scanned: **{}**\n", counts.enums_total));
    out.push_str(&format!(
        "- Variants total: **{}**\n",
        counts.variants_total
    ));
    out.push_str(&format!("- Confirmed dead: **{}**\n", counts.dead));
    out.push_str(&format!(
        "- Skipped (`#[from]` thiserror auto-constructor): **{}**\n",
        counts.from_skipped
    ));
    out.push_str(&format!(
        "- Skipped (clap / serde / strum / similar derive constructs variants from input): **{}**\n",
        counts.input_derive_skipped
    ));

    (out, counts)
}

#[derive(Default, Debug)]
struct ReportCounts {
    enums_total: usize,
    variants_total: usize,
    dead: usize,
    from_skipped: usize,
    input_derive_skipped: usize,
}

/// xtask entry point. Parses positional arguments after `lint-dead-variants`.
///
/// Usage:
///   cargo run -q -p xtask -- lint-dead-variants \
///       --model-root <PATH> --search-root <PATH> --output <PATH> [--label <NAME>]
pub fn run(repo_root: &Path, args: Vec<String>) -> Result<()> {
    let mut model_root: Option<PathBuf> = None;
    let mut search_root: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut label: Option<String> = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--model-root" => {
                model_root = Some(PathBuf::from(
                    iter.next()
                        .ok_or("lint-dead-variants: --model-root requires a path argument")?,
                ));
            }
            "--search-root" => {
                search_root = Some(PathBuf::from(
                    iter.next()
                        .ok_or("lint-dead-variants: --search-root requires a path argument")?,
                ));
            }
            "--output" => {
                output = Some(PathBuf::from(
                    iter.next()
                        .ok_or("lint-dead-variants: --output requires a path argument")?,
                ));
            }
            "--label" => {
                label = Some(
                    iter.next()
                        .ok_or("lint-dead-variants: --label requires a string argument")?,
                );
            }
            other => return Err(format!("lint-dead-variants: unknown arg {other:?}").into()),
        }
    }

    let model_root = model_root.ok_or("lint-dead-variants: --model-root is required")?;
    let search_root = search_root.ok_or("lint-dead-variants: --search-root is required")?;
    let output = output.ok_or("lint-dead-variants: --output is required")?;

    let model_root = if model_root.is_absolute() {
        model_root
    } else {
        repo_root.join(model_root)
    };
    let search_root = if search_root.is_absolute() {
        search_root
    } else {
        repo_root.join(search_root)
    };
    let output = if output.is_absolute() {
        output
    } else {
        repo_root.join(output)
    };

    if !model_root.is_dir() {
        return Err(format!("--model-root not a directory: {}", model_root.display()).into());
    }
    if !search_root.is_dir() {
        return Err(format!("--search-root not a directory: {}", search_root.display()).into());
    }

    let label = label.unwrap_or_else(|| {
        model_root
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("model")
            .to_string()
    });

    let enums = find_enums(&search_root, &model_root);
    if enums.is_empty() {
        return Err(format!("no enums found in {}", model_root.display()).into());
    }
    let enum_names: HashSet<String> = enums.iter().map(|e| e.name.clone()).collect();
    let index = build_constructor_index(&search_root, &enum_names);
    let (text, counts) = render_report(&label, &model_root, &search_root, &enums, &index);

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output, &text)?;

    eprintln!("wrote {}", output.display());
    eprintln!(
        "  {} enums, {} variants, {} dead, {} via #[from], {} via input-derives",
        counts.enums_total,
        counts.variants_total,
        counts.dead,
        counts.from_skipped,
        counts.input_derive_skipped,
    );
    Ok(())
}

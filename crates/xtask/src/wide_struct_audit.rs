//! Audit wide Rust structs so field-bag growth stays explicit and reviewed.
//!
//! Scans all `.rs` files under the audit roots for structs with ≥10
//! named fields and ensures each is registered in `WIDE_STRUCT_ALLOWANCES`
//! with a reviewed field cap and classification.
//!
//! Contributor entrypoints:
//! - run `cargo run -q -p xtask -- lint-wide-structs` for the audit itself
//! - run `cargo nextest run -p talkbank-tools --test wide_struct_audit` for the
//!   thin integration-test proxy that keeps this audit visible in test output
//!   without duplicating logic across CLI crates

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::Result;
use crate::rust_scan::{brace_delta, rust_scan_roots, walkdir};

const WIDE_STRUCT_THRESHOLD: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WideStructDisposition {
    BoundaryShim,
    TransportRecord,
    RealAggregate,
    RefactorTarget,
}

impl WideStructDisposition {
    fn label(self) -> &'static str {
        match self {
            Self::BoundaryShim => "boundary shim",
            Self::TransportRecord => "transport record",
            Self::RealAggregate => "real aggregate",
            Self::RefactorTarget => "refactor target",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct WideStructAllowance {
    path: &'static str,
    struct_name: &'static str,
    max_fields: usize,
    max_bool_fields: usize,
    disposition: WideStructDisposition,
    reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NamedStructInfo {
    path: String,
    line: usize,
    struct_name: String,
    field_count: usize,
    bool_field_count: usize,
}

const WIDE_STRUCT_ALLOWANCES: &[WideStructAllowance] = &[
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/commands/eval.rs",
        struct_name: "SpeakerEval",
        max_fields: 25,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "metric record for one EVAL speaker report",
    },
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/commands/kideval.rs",
        struct_name: "SpeakerKideval",
        max_fields: 21,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "metric record for one KIDEVAL speaker report",
    },
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/commands/complexity.rs",
        struct_name: "SpeakerAccum",
        max_fields: 20,
        max_bool_fields: 2,
        disposition: WideStructDisposition::RefactorTarget,
        reason: "accumulator mixes counters and mode flags",
    },
    WideStructAllowance {
        path: "src/test_dashboard/app.rs",
        struct_name: "AppState",
        max_fields: 19,
        max_bool_fields: 2,
        disposition: WideStructDisposition::RefactorTarget,
        reason: "dashboard state still mixes corpus progress, global totals, render flags, and timing",
    },
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/commands/complexity.rs",
        struct_name: "SpeakerComplexity",
        max_fields: 19,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "complexity report record",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-parser-re2c/src/generated/lexer.rs",
        struct_name: "Lexer",
        max_fields: 22,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "re2c-generated DFA lexer state — fields are scanner registers, not refactorable",
    },
    WideStructAllowance {
        path: "crates/chatter/chatter-cli/src/ui/validation_tui/state.rs",
        struct_name: "TuiState",
        max_fields: 15,
        max_bool_fields: 1,
        disposition: WideStructDisposition::RefactorTarget,
        reason: "mixes widget state, progress counters, discovery flags, and final summary state",
    },
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/framework/mor.rs",
        struct_name: "MorPosCount",
        max_fields: 14,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "report record for morphology counts",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-model/src/model/alignment_set.rs",
        struct_name: "AlignmentUnits",
        max_fields: 14,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "cohesive alignment domain aggregate",
    },
    // spec/runtime-tools entries removed — bootstrap tooling being dismantled
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/commands/flucalc.rs",
        struct_name: "SpeakerFluency",
        max_fields: 12,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "fluency report record",
    },
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/commands/linker_audit.rs",
        struct_name: "FileStats",
        max_fields: 51,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "per-file linker/terminator audit statistics record",
    },
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/commands/linker_audit.rs",
        struct_name: "CorpusSummary",
        max_fields: 49,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "corpus-wide linker/terminator audit summary record",
    },
    // (removed: extract_corpus_candidates Args — bootstrap tooling being dismantled)
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/database/entry.rs",
        struct_name: "DbMetadata",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "database metadata row shape",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-parser-tests/src/generated_traversal.rs",
        struct_name: "IdDemographicFieldsChildren",
        max_fields: 16,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "generated CST traversal struct for @ID demographic fields",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-parser-tests/src/generated_traversal.rs",
        struct_name: "IdRoleFieldsChildren",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "generated CST traversal struct for @ID role fields",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-parser-tests/src/generated_traversal.rs",
        struct_name: "TypesHeaderChildren",
        max_fields: 12,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "generated CST traversal struct for @Types header",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-parser-re2c/src/ast.rs",
        struct_name: "IdHeaderParsed",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "parsed @ID header with all demographic/role fields",
    },
    WideStructAllowance {
        path: "crates/chatter/chatter-cli/src/cli/args/clan_common.rs",
        struct_name: "CommonAnalysisArgs",
        max_fields: 10,
        max_bool_fields: 2,
        disposition: WideStructDisposition::BoundaryShim,
        reason: "shared clap boundary shape for analysis commands",
    },
    WideStructAllowance {
        path: "crates/chatter/clan-core/src/service_types.rs",
        struct_name: "AnalysisOptions",
        max_fields: 25,
        max_bool_fields: 5,
        disposition: WideStructDisposition::BoundaryShim,
        reason: "raw adapter-to-library option bag consumed by AnalysisRequestBuilder before defaults and validation",
    },
    WideStructAllowance {
        path: "crates/chatter/chatter-lsp/src/backend/contracts.rs",
        struct_name: "AnalysisOptionsPayload",
        max_fields: 22,
        max_bool_fields: 5,
        disposition: WideStructDisposition::BoundaryShim,
        reason: "public editor/server JSON payload for talkbank/analyze kept flat for schema generation and transport clarity",
    },
    WideStructAllowance {
        path: "crates/chatter/chatter-cli/src/ui/theme.rs",
        struct_name: "Theme",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "cohesive color palette aggregate",
    },
    WideStructAllowance {
        path: "crates/chatter/chatter-lsp/src/backend/participants.rs",
        struct_name: "ParticipantFields",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "participant edit payload for the editor boundary",
    },
    WideStructAllowance {
        path: "crates/chatter/chatter-lsp/src/backend/state.rs",
        struct_name: "Backend",
        max_fields: 10,
        max_bool_fields: 1,
        disposition: WideStructDisposition::RefactorTarget,
        reason: "service-root aggregate that still wants cache and service grouping",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-model/src/model/alignment_set.rs",
        struct_name: "AlignmentSet",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "cohesive alignment container",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-model/src/model/content/word/word_serialize.rs",
        struct_name: "WordJsonSchema",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "JSON schema boundary for word serialization",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-model/src/model/content/word/word_type.rs",
        struct_name: "Word",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "core word domain aggregate",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-model/src/model/header/id.rs",
        struct_name: "IDHeader",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "header record mirrors one CHAT ID line",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-parser-tests/src/bin/audit_error_codes.rs",
        struct_name: "Analysis",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "audit tool report row",
    },
    WideStructAllowance {
        path: "crates/spec/talkbank-spec-testgen/src/bin/corpus_node_coverage.rs",
        struct_name: "CoverageReport",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "spec-tool coverage report shape",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-model/src/alignment/helpers/walk/tests.rs",
        struct_name: "ContentCounts",
        max_fields: 17,
        max_bool_fields: 0,
        disposition: WideStructDisposition::TransportRecord,
        reason: "test-only counter struct — one field per UtteranceContent variant",
    },
    WideStructAllowance {
        path: "crates/core/talkbank-transform/src/morphosyntax.rs",
        struct_name: "UdWord",
        max_fields: 10,
        max_bool_fields: 0,
        disposition: WideStructDisposition::RealAggregate,
        reason: "core UD word domain aggregate shared by morphosyntax transforms",
    },
];

fn scan_named_structs(root: &Path) -> Vec<NamedStructInfo> {
    let mut structs = Vec::new();
    for base in rust_scan_roots(root) {
        if !base.exists() {
            continue;
        }
        for path in walkdir(&base) {
            let relative = path
                .strip_prefix(root)
                .expect("scan path should be inside repo")
                .to_string_lossy()
                .into_owned();
            let text = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => continue,
            };
            structs.extend(parse_named_structs_in_file(&relative, &text));
        }
    }
    structs.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.struct_name.cmp(&right.struct_name))
    });
    structs
}

fn parse_named_structs_in_file(relative_path: &str, text: &str) -> Vec<NamedStructInfo> {
    let lines: Vec<&str> = text.lines().collect();
    let mut result = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].trim();
        let Some(struct_name) = struct_name_from_declaration(line) else {
            index += 1;
            continue;
        };
        let mut depth = brace_delta(line);
        let mut field_count = 0;
        let mut bool_field_count = 0;
        let start_line = index + 1;
        index += 1;
        while index < lines.len() && depth > 0 {
            let current = lines[index];
            let trimmed = current.trim();
            if depth == 1 && is_named_field(trimmed) {
                field_count += 1;
                if field_type(trimmed).is_some_and(|value| value.contains("bool")) {
                    bool_field_count += 1;
                }
            }
            depth += brace_delta(current);
            index += 1;
        }
        result.push(NamedStructInfo {
            path: relative_path.to_string(),
            line: start_line,
            struct_name,
            field_count,
            bool_field_count,
        });
    }
    result
}

fn struct_name_from_declaration(line: &str) -> Option<String> {
    let declaration = line
        .strip_prefix("pub struct ")
        .or_else(|| line.strip_prefix("struct "))?;
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

fn is_named_field(line: &str) -> bool {
    if line.is_empty()
        || line.starts_with("//")
        || line.starts_with("///")
        || line.starts_with("#[")
        || line.starts_with("pub use ")
    {
        return false;
    }
    line.contains(':') && !line.starts_with("fn ") && !line.starts_with("where ")
}

fn field_type(line: &str) -> Option<&str> {
    let (_, ty) = line.split_once(':')?;
    Some(ty.trim().trim_end_matches(','))
}

pub fn run(root: &Path) -> Result<()> {
    let wide_structs: Vec<NamedStructInfo> = scan_named_structs(root)
        .into_iter()
        .filter(|info| info.field_count >= WIDE_STRUCT_THRESHOLD)
        .collect();

    let actual_by_key: BTreeMap<(String, String), NamedStructInfo> = wide_structs
        .iter()
        .cloned()
        .map(|info| ((info.path.clone(), info.struct_name.clone()), info))
        .collect();
    let expected_keys: BTreeSet<(String, String)> = WIDE_STRUCT_ALLOWANCES
        .iter()
        .map(|entry| (entry.path.to_string(), entry.struct_name.to_string()))
        .collect();

    let mut failures = Vec::new();

    for info in &wide_structs {
        let Some(allowance) = WIDE_STRUCT_ALLOWANCES
            .iter()
            .find(|entry| entry.path == info.path && entry.struct_name == info.struct_name)
        else {
            failures.push(format!(
                "{}:{}: {} has {} fields and {} bool fields but no audit entry",
                info.path, info.line, info.struct_name, info.field_count, info.bool_field_count
            ));
            continue;
        };
        if info.field_count > allowance.max_fields {
            failures.push(format!(
                "{}:{}: {} grew from reviewed max {} fields to {} ({}, {})",
                info.path,
                info.line,
                info.struct_name,
                allowance.max_fields,
                info.field_count,
                allowance.disposition.label(),
                allowance.reason
            ));
        }
        if info.bool_field_count > allowance.max_bool_fields {
            failures.push(format!(
                "{}:{}: {} grew from reviewed max {} bool fields to {} ({}, {})",
                info.path,
                info.line,
                info.struct_name,
                allowance.max_bool_fields,
                info.bool_field_count,
                allowance.disposition.label(),
                allowance.reason
            ));
        }
    }

    for allowance in WIDE_STRUCT_ALLOWANCES {
        let key = (
            allowance.path.to_string(),
            allowance.struct_name.to_string(),
        );
        if !actual_by_key.contains_key(&key) {
            failures.push(format!(
                "{}: stale audit entry for {} ({}, {})",
                allowance.path,
                allowance.struct_name,
                allowance.disposition.label(),
                allowance.reason
            ));
        }
    }

    for key in actual_by_key.keys() {
        if !expected_keys.contains(key) {
            failures.push(format!(
                "{}: unexpected wide struct audit state for {}",
                key.0, key.1
            ));
        }
    }

    if failures.is_empty() {
        println!("wide struct audit: OK");
        Ok(())
    } else {
        Err(format!("wide struct audit failures:\n- {}", failures.join("\n- ")).into())
    }
}

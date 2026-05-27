//! Emit a unified JSON Schema document for every proto type that crosses the
//! Rust ↔ Python worker boundary.
//!
//! ## Contract
//!
//! The emitter is **closed for modification**. It walks
//! `inventory::iter::<ProtoSchemaEntry>` — every type registered via
//! `crate::register_proto_schema!` in its own source file lands here
//! automatically. Adding a new wire type means: define the struct, call
//! `register_proto_schema!(NewThing)` next to it, rebuild. No edits here.
//!
//! ## Output
//!
//! A single JSON Schema 2020-12 document with every registered type under
//! `$defs`. `TaskInput` and `TaskOutput` (the closed-set union enums) ride
//! along the same path as ordinary structs because they self-register in
//! `base.rs`. Variants of tagged enums (`LanguageSpec`, `TaskInput`,
//! `TaskOutput`) get a generated `discriminator` block and per-variant
//! `title` so `datamodel-code-generator` materializes them as clean
//! discriminated unions rather than `Spec1` / `Spec2` / `Spec3` siblings.
//!
//! ## Usage
//!
//!     emit_proto_schema <out.json>
//!
//! Wired into Bazel as a `rust_binary` and consumed by the
//! `//crates/batchalign/batchalign-core:proto_schema_json` genrule. Pure
//! cargo: `cargo run -p batchalign-core --bin emit_proto_schema --
//! /tmp/proto.schema.json` (used inside `build.rs`-style flows; not a
//! workflow developers invoke directly).

use std::process::ExitCode;

use batchalign_core::proto::ProtoSchemaEntry;
use serde_json::{Map, Value};

fn main() -> ExitCode {
    let out_path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: emit_proto_schema <out.json>");
            return ExitCode::from(2);
        }
    };

    let mut defs: Map<String, Value> = Map::new();

    // Walk every link-time registration. Order is link order; we sort the
    // final $defs map by key below so the output stays deterministic
    // across builds and `bazel test` diffs cleanly.
    for entry in inventory::iter::<ProtoSchemaEntry> {
        (entry.ingest)(&mut defs);
    }

    // Sort $defs by key so successive `bazel build` runs produce
    // byte-identical output (Bazel caches by content hash; an unstable
    // order would invalidate downstream caches needlessly).
    let mut sorted: Vec<(String, Value)> = defs.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut defs: Map<String, Value> = sorted.into_iter().collect();

    // Splice discriminators + per-variant titles into tagged-enum oneOf
    // shapes. schemars (v1) doesn't emit either, and without them
    // datamodel-codegen materializes `LanguageSpec1/2/3` siblings.
    let names: Vec<String> = defs.keys().cloned().collect();
    for name in names {
        if let Some(value) = defs.get_mut(&name) {
            add_discriminator_if_tagged(&name, value);
        }
    }

    if defs.is_empty() {
        eprintln!(
            "emit_proto_schema: ProtoSchemaEntry inventory is empty. \
             This means batchalign-core was linked without any \
             `register_proto_schema!` invocation, which usually points at a \
             dead-code-elimination problem in the binary's link flags. \
             Refusing to emit an empty schema."
        );
        return ExitCode::from(1);
    }

    // Bare `$defs` document — no outer title/description, otherwise
    // datamodel-code-generator manufactures a useless root model around
    // the entire bag of types.
    let root = Value::Object({
        let mut m = Map::new();
        m.insert(
            "$schema".into(),
            Value::String("https://json-schema.org/draft/2020-12/schema".into()),
        );
        m.insert("$defs".into(), Value::Object(defs));
        m
    });

    let pretty = match serde_json::to_string_pretty(&root) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("serialize schema: {e}");
            return ExitCode::from(1);
        }
    };

    if let Err(e) = std::fs::write(&out_path, format!("{pretty}\n")) {
        eprintln!("write {out_path}: {e}");
        return ExitCode::from(1);
    }

    eprintln!("emitted proto schema → {out_path}");
    ExitCode::SUCCESS
}

/// If `schema` is a oneOf where every variant has the same single `const`
/// property (e.g. `kind: "auto"`, `kind: "code"`, …), splice in an OpenAPI
/// `discriminator` block AND give each variant a unique `title` derived from
/// the const value. Without these annotations `datamodel-code-generator`
/// emits `LanguageSpec1` / `LanguageSpec2` / `LanguageSpec3` siblings; with
/// them it emits `LanguageSpecAuto` / `LanguageSpecCode` / `LanguageSpecPerFile`
/// plus a `LanguageSpec` discriminated-union alias.
fn add_discriminator_if_tagged(parent_name: &str, schema: &mut Value) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    let Some(Value::Array(_)) = obj.get("oneOf") else {
        return;
    };

    // First pass: find the tag property (one every variant marks `const`).
    let tag: Option<String> = {
        let variants = obj.get("oneOf").and_then(Value::as_array).cloned();
        let Some(variants) = variants else {
            return;
        };
        if variants.is_empty() {
            return;
        }
        find_const_tag(&variants)
    };
    let Some(tag) = tag else { return };

    // Second pass: title each variant.
    if let Some(Value::Array(variants)) = obj.get_mut("oneOf") {
        for variant in variants {
            let Some(v_obj) = variant.as_object_mut() else {
                continue;
            };
            let const_val: Option<String> = v_obj
                .get("properties")
                .and_then(|p| p.get(&tag))
                .and_then(|t| t.get("const"))
                .and_then(|c| c.as_str())
                .map(str::to_string);
            if let Some(c) = const_val {
                v_obj.insert(
                    "title".into(),
                    Value::String(format!("{parent_name}{}", to_pascal(&c))),
                );
            }
        }
    }

    obj.insert(
        "discriminator".into(),
        Value::Object({
            let mut m = Map::new();
            m.insert("propertyName".into(), Value::String(tag));
            m
        }),
    );
}

/// Walk the variants of a oneOf and return the single property name that
/// every variant marks as `const`. Returns `None` when variants disagree.
fn find_const_tag(variants: &[Value]) -> Option<String> {
    let mut tag: Option<String> = None;
    for variant in variants {
        let props = variant.as_object()?.get("properties")?.as_object()?;
        let mut local: Option<String> = None;
        for (name, value) in props {
            if value.get("const").is_some() {
                if local.is_some() {
                    return None;
                }
                local = Some(name.clone());
            }
        }
        let local = local?;
        match tag.as_deref() {
            None => tag = Some(local),
            Some(prior) if prior == local => {}
            Some(_) => return None,
        }
    }
    tag
}

/// `per_file` → `PerFile`, `Asr` → `Asr`.
fn to_pascal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = true;
    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch == ' ' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

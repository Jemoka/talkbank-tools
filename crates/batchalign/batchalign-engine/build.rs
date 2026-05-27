//! Stage `python/batchalign/_core/_proto_generated.py` for the
//! cargo/maturin path so `cd python && uv run maturin develop` works
//! without a manual codegen step.
//!
//! ## Contract
//!
//! - In **Bazel**: the build script no-ops on the codegen side. Bazel runs
//!   inside a sandbox where writing back to the source tree is forbidden,
//!   and the `//python/batchalign/_core:_proto_generated_py` genrule is
//!   the authoritative path anyway. We detect the sandbox by the absence
//!   of a writable source tree at `python/batchalign/_core/`.
//!
//! - In **cargo / maturin**: walk the inventory of registered proto types
//!   (via `batchalign-core` as a `[build-dependencies]` link), emit the
//!   JSON Schema to `$OUT_DIR/proto.schema.json`, then shell out to a
//!   Python interpreter to run `bazel/python/codegen_proto.py`. The
//!   resulting `_proto_generated.py` lands in the source tree where
//!   maturin packages it into the wheel.
//!
//! Failure modes are non-fatal: if Python or `datamodel-code-generator`
//! isn't on `PATH`, the build script logs a warning and continues. The
//! resulting wheel still works for whoever staged the file by some other
//! means (Bazel `bazel build //python/batchalign/_core:_proto_generated_py`,
//! a prior `bazel run //python/batchalign:wheel`, …). The clear error
//! message in `batchalign._core.__init__.py` tells the user what to do
//! when the file is missing at import time.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    // Rebuild on any change to a registered proto type. `batchalign-core`
    // is a build-dep so cargo already invalidates us on its source
    // changes; the explicit rerun-if directives keep things robust
    // against build-dep graph quirks (and document intent).
    println!("cargo:rerun-if-changed=build.rs");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let proto_src_dir = manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("batchalign/batchalign-core/src/proto"))
        .expect("locate proto/ relative to engine");
    if proto_src_dir.is_dir() {
        // cargo only takes one rerun-if-changed path per println; the
        // simplest is rerun-if-changed on each .rs in proto/.
        if let Ok(entries) = std::fs::read_dir(&proto_src_dir) {
            for entry in entries.flatten() {
                println!("cargo:rerun-if-changed={}", entry.path().display());
            }
        }
    }

    // Locate the project root and the source-tree target for the
    // generated module. We walk up from CARGO_MANIFEST_DIR
    // (`<root>/crates/batchalign/batchalign-engine`) by three levels.
    let project_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("locate workspace root from CARGO_MANIFEST_DIR")
        .to_path_buf();
    let target_path = project_root
        .join("python/batchalign/_core/_proto_generated.py");
    let codegen_script = project_root.join("bazel/python/codegen_proto.py");

    if !target_path.parent().map(Path::is_dir).unwrap_or(false) {
        // Bazel sandbox typically does NOT mirror the entire workspace.
        // If the target directory is unreachable, we're in a sandbox and
        // the genrule path handles codegen separately — silently skip.
        return;
    }

    // Walk the inventory and produce the unified schema document.
    let mut defs: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for entry in inventory::iter::<batchalign_core::proto::ProtoSchemaEntry> {
        (entry.ingest)(&mut defs);
    }
    if defs.is_empty() {
        println!(
            "cargo:warning=batchalign-engine build.rs: proto registry is \
             empty; skipping _proto_generated.py codegen. The wheel will \
             import-error at runtime if no other path stages the file."
        );
        return;
    }

    // Sort for determinism so the schema digest is stable across builds.
    let mut sorted: Vec<(String, serde_json::Value)> = defs.into_iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut defs: serde_json::Map<String, serde_json::Value> = sorted.into_iter().collect();
    let names: Vec<String> = defs.keys().cloned().collect();
    for name in names {
        if let Some(value) = defs.get_mut(&name) {
            splice_discriminator(&name, value);
        }
    }

    let root = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": defs,
    });
    let schema_path = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"))
        .join("proto.schema.json");
    let schema_text = match serde_json::to_string_pretty(&root) {
        Ok(s) => s,
        Err(e) => {
            println!("cargo:warning=batchalign-engine build.rs: schema serialize failed: {e}");
            return;
        }
    };
    if let Err(e) = std::fs::write(&schema_path, format!("{schema_text}\n")) {
        println!("cargo:warning=batchalign-engine build.rs: write schema {}: {e}", schema_path.display());
        return;
    }

    // Resolve the Python interpreter. `maturin develop` runs inside the
    // active venv, so `python3` on PATH IS the venv's interpreter. When
    // the venv's interpreter isn't called `python3` (Windows), fall back
    // to `python`.
    let python = which("python3").or_else(|| which("python")).unwrap_or_else(|| PathBuf::from("python3"));
    if !codegen_script.is_file() {
        println!(
            "cargo:warning=batchalign-engine build.rs: codegen script not found at {} \
             (cargo workspace may be a partial checkout?); skipping _proto_generated.py",
            codegen_script.display()
        );
        return;
    }
    let status = Command::new(&python)
        .arg(&codegen_script)
        .arg(&schema_path)
        .arg(&target_path)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!(
                "cargo:warning=batchalign-engine build.rs: regenerated {}",
                target_path.display()
            );
        }
        Ok(s) => {
            println!(
                "cargo:warning=batchalign-engine build.rs: codegen exited {s} — \
                 datamodel-code-generator probably not installed. The Bazel genrule \
                 path is unaffected; pure-maturin paths need it via `uv sync --dev`."
            );
        }
        Err(e) => {
            println!(
                "cargo:warning=batchalign-engine build.rs: spawn {python:?} failed: {e}. \
                 Bazel-managed paths are unaffected."
            );
        }
    }
}

/// `which`-style PATH lookup; avoids pulling the `which` crate as a
/// build-dep over a sub-200-line build script.
fn which(cmd: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Mirror of `emit_proto_schema`'s discriminator-splice logic. Duplicated
/// here so the build script doesn't have to depend on a private helper.
fn splice_discriminator(parent_name: &str, schema: &mut serde_json::Value) {
    let Some(obj) = schema.as_object_mut() else {
        return;
    };
    let Some(serde_json::Value::Array(_)) = obj.get("oneOf") else {
        return;
    };
    let tag: Option<String> = {
        let variants = obj.get("oneOf").and_then(serde_json::Value::as_array).cloned();
        let Some(variants) = variants else {
            return;
        };
        if variants.is_empty() {
            return;
        }
        find_const_tag(&variants)
    };
    let Some(tag) = tag else { return };

    if let Some(serde_json::Value::Array(variants)) = obj.get_mut("oneOf") {
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
                    serde_json::Value::String(format!("{parent_name}{}", to_pascal(&c))),
                );
            }
        }
    }
    obj.insert(
        "discriminator".into(),
        serde_json::Value::Object({
            let mut m = serde_json::Map::new();
            m.insert("propertyName".into(), serde_json::Value::String(tag));
            m
        }),
    );
}

fn find_const_tag(variants: &[serde_json::Value]) -> Option<String> {
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

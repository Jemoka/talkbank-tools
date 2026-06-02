//! Wire-protocol types crossing the worker boundary.
//!
//! ## Self-registration
//!
//! Each proto type calls [`register_proto_schema!`] once below its
//! `#[derive(Serialize, Deserialize, JsonSchema)]`. The macro submits a
//! [`ProtoSchemaEntry`] into the link-time inventory; the schema emitter
//! binary (`src/bin/emit_proto_schema.rs`) walks
//! `inventory::iter::<ProtoSchemaEntry>` and serializes each one.
//!
//! **Contract.** Adding a new proto type means: define the struct in the
//! appropriate `proto/<task>.rs` file, `register_proto_schema!(NewThing);`
//! immediately below it, rebuild. The Python-side `_proto_generated.py`
//! and every Bazel/maturin consumer pick the new type up automatically on
//! the next build — no edits to the emitter, no `just gen-proto`, no
//! manual codegen step. Forgetting the macro call means the type stays
//! invisible to Python; the doctest at the end of this file fails fast if
//! `TaskInput`/`TaskOutput` are missing, which catches the common slip.
//!
//! Schemars also flattens any transitively-reachable type into `$defs`
//! automatically, so a struct embedded inside a registered type doesn't
//! need its own registration. Register types that backends construct or
//! `isinstance`-check directly.

pub mod asr;
pub mod compare;
pub mod coref;
pub mod fa;
pub mod morphosyntax;
pub mod speaker;
pub mod translate;
pub mod utseg;

pub use asr::{AsrInput, AsrOptions, AsrOutput, AsrSegment, AsrWord, LanguageSpec};
pub use compare::{CompareInput, CompareMetrics, CompareMetricsPos, CompareOutput};
pub use coref::{CorefInput, CorefOutput};
pub use fa::{FaInput, FaOutput};
pub use morphosyntax::{
    GraTerminator, MorphosyntaxInput, MorphosyntaxOutput, MorphosyntaxToken, MorphosyntaxUnit,
    MorphosyntaxUtterance,
};
pub use speaker::{Diarization, DiarizationSegment, SpeakerInput, SpeakerOutput};
pub use translate::{TranslateInput, TranslateOutput};
pub use utseg::{UtSegInput, UtSegOutput, UtteranceSpan};

use serde_json::{Map, Value};

/// One row in the link-time wire-schema registry. The schema emitter
/// (`src/bin/emit_proto_schema.rs`) iterates these and pours each entry's
/// schema (plus its transitive `$defs`) into the unified document.
pub struct ProtoSchemaEntry {
    /// Type name as it appears in the generated Python module (e.g.
    /// `"AsrInput"`). Must match the Rust type's leaf identifier.
    pub name: &'static str,
    /// Generate the schema for the registered type and merge it into the
    /// shared `$defs` map. Implemented inline by [`register_proto_schema!`].
    pub ingest: fn(&mut Map<String, Value>),
}

inventory::collect!(ProtoSchemaEntry);

/// Generate the schema for `T`, lift its nested `$defs` into the shared
/// map, and store the root under `name`. Re-exported here so the emitter
/// binary and `register_proto_schema!` agree on shape.
pub fn ingest_schema<T: schemars::JsonSchema>(name: &str, defs: &mut Map<String, Value>) {
    let schema = schemars::schema_for!(T);
    let mut value = serde_json::to_value(&schema).expect("schema serializable");
    if let Some(obj) = value.as_object_mut() {
        if let Some(Value::Object(nested)) = obj.remove("$defs") {
            for (k, v) in nested {
                defs.entry(k).or_insert(v);
            }
        }
        // Strip the top-level `$schema` URI — `$defs` entries don't carry one.
        obj.remove("$schema");
    }
    defs.insert(name.to_string(), value);
}

/// Register a type with the link-time wire-schema inventory.
///
/// Place one call below each `#[derive(Serialize, Deserialize, JsonSchema)]`
/// proto type:
///
/// ```ignore
/// #[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
/// pub struct AsrInput { /* … */ }
/// register_proto_schema!(AsrInput);
/// ```
///
/// The macro submits a [`ProtoSchemaEntry`] into the inventory; the
/// emitter binary picks it up automatically. There is no separate list
/// to edit and no codegen step to invoke by hand.
#[macro_export]
macro_rules! register_proto_schema {
    ($t:ty) => {
        ::inventory::submit! {
            $crate::proto::ProtoSchemaEntry {
                name: stringify!($t),
                ingest: |defs| $crate::proto::ingest_schema::<$t>(stringify!($t), defs),
            }
        }
    };
}

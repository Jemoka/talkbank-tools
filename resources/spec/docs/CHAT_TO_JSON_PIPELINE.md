# CHAT-to-JSON Pipeline

How a raw CHAT text file becomes a validated, aligned, JSON-serialized AST.

## Pipeline Overview

```
Raw CHAT text (String)
  │
  │  [1] Tree-sitter Parsing
  │      talkbank-parser
  ▼
tree_sitter::Tree (CST)
  │
  │  [2] CST-to-AST Conversion
  │      parse_lines() → Vec<Line>
  ▼
ChatFile { lines, participants }
  │
  │  [3] Validation
  │      ChatFile::validate()
  ▼
ChatFile (with diagnostics emitted)
  │
  │  [4] Alignment
  │      Utterance::compute_alignments()
  │      Distributes dependent tier items into per-word fields
  ▼
ChatFile (with embedded alignment on every Word)
  │
  │  [5] JSON Serialization
  │      serde_json::to_string() + jsonschema validation
  ▼
JSON String
```

---

## Stage 1: Entry Point

### `talkbank-transform` orchestration

The top-level function is `parse_and_validate()`:

```
talkbank-transform/src/pipeline/parse.rs
```

```rust
pub fn parse_and_validate(
    content: &str,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError>
```

This creates a `TreeSitterParser`, then delegates to the `_with_parser` variant:

```rust
pub fn parse_and_validate_with_parser(
    parser: &TreeSitterParser,
    content: &str,
    options: ParseValidateOptions,
) -> Result<ChatFile, PipelineError>
```

The function:

1. Creates an `ErrorCollector` to collect parse errors
2. Calls `parser.parse_chat_file_fragment(content, 0, &parse_errors)`
3. If any `Severity::Error` errors, returns `Err(PipelineError::Parse(...))`
4. If `options.alignment` is set, calls `chat_file.validate_with_alignment()`
5. Otherwise if `options.validate`, calls `chat_file.validate()`
6. Returns `Ok(ChatFile)`

For JSON conversion specifically:

```
talkbank-transform/src/pipeline/convert.rs
```

```rust
pub fn chat_to_json(
    content: &str,
    options: ParseValidateOptions,
    pretty: bool,
) -> Result<String, PipelineError> {
    let chat_file = parse_and_validate(content, options)?;
    let json = if pretty {
        to_json_pretty_validated(&chat_file)
    } else {
        to_json_validated(&chat_file)
    }?;
    Ok(json)
}
```

---

## Stage 2: Tree-sitter Parsing

### TreeSitterParser

```
talkbank-parser/src/parser/chat_file_parser/parser_struct.rs
```

`TreeSitterParser` is the sole parser. Create one and reuse it:

```rust
let parser = TreeSitterParser::new()?;

// Full-file parsing (Result API):
let chat_file = parser.parse_chat_file(input)?;

// Fragment parsing with offset + streaming errors:
parser.parse_chat_file_fragment(input, offset, &errors)  // -> ParseOutcome<ChatFile>
parser.parse_word_fragment(input, offset, &errors)        // -> ParseOutcome<Word>
parser.parse_main_tier_fragment(input, offset, &errors)   // -> ParseOutcome<MainTier>
parser.parse_mor_tier_fragment(input, offset, &errors)    // -> ParseOutcome<MorTier>
parser.parse_gra_tier_fragment(input, offset, &errors)    // -> ParseOutcome<GraTier>
// ... every CHAT construct is independently parseable
```

`ParseOutcome<T>` replaces ambiguous `Option<T>`:

```rust
pub enum ParseOutcome<T> {
    Parsed(T),  // parser produced semantic output
    Rejected,   // parser could not produce semantic output
}
```

### TreeSitterParser

```
talkbank-parser/src/parser/chat_file_parser/
```

The core method is `parse_chat_file_streaming()`:

```rust
pub fn parse_chat_file_streaming(
    &self, input: &str, errors: &impl ErrorSink,
) -> ChatFile {
    let mut lines = parse_lines(self, input, errors);
    let (participants, participant_errors) =
        build_participants_from_lines(&lines);
    let ca_mode = headers_enable_ca_mode(&all_headers);
    if ca_mode { normalize_ca_omissions(&mut lines); }
    ChatFile::with_participants(lines, participants)
}
```

### parse_lines: CST to AST

```
talkbank-parser/src/parser/chat_file_parser/chat_file/helpers.rs
```

This is the heart of tree-sitter CST traversal:

1. Calls `parser.parser.borrow_mut().parse(input, None)` to get a
   `tree_sitter::Tree` (the CST)
2. Walks the root node's children with `root_node.children(&mut cursor)`
3. For each child node, dispatches on `child.kind()`:
   - `UTF8_HEADER`, `BEGIN_HEADER`, `END_HEADER` -- creates `Line::Header`
   - `LINE` -- walks sub-children:
     - `HEADER` node -- calls `parse_header_node()` producing `Header` enum
     - `UTTERANCE` node -- calls `parse_utterance_node()` producing
       `Utterance` with main tier + dependent tiers
   - Error/missing nodes -- attempts recovery, else emits diagnostic

### parse_utterance_node

This function converts a single CST utterance node into the model `Utterance`.
It walks the utterance's children to find:

- The **main tier** (`*SPEAKER:\t words terminator bullet`) -- parsed into
  `MainTier` with `UtteranceContent` items (words, groups, separators, pauses,
  etc.)
- **Dependent tiers** (`%mor:`, `%gra:`, `%pho:`, `%wor:`, `%sin:`, `%act:`,
  `%com:`, etc.) -- each dispatched to a specialized sub-parser

Structured dependent tiers (`%mor`, `%gra`, `%pho`, `%wor`, `%sin`) are parsed
into their full tier types (e.g., `MorTier`, `WorTier`) and then stored on the
utterance as **markers** (e.g., `MorTierMarker`) that hold the parsed items in
a `pending_items` field.

---

## Stage 3: The AST (Model Types)

### ChatFile

```
talkbank-model/src/model/file/chat_file/core.rs
```

```rust
pub struct ChatFile {
    pub lines: ChatFileLines,   // Vec<Line> preserving header/utterance order
    pub participants: IndexMap<SpeakerCode, Participant>,
}
```

### Line

```
talkbank-model/src/model/file/line.rs
```

```rust
#[serde(tag = "line_type", rename_all = "lowercase")]
pub enum Line {
    Header { header: Header, span: Span },
    Utterance(Box<Utterance>),
}
```

Preserves exact interleaving order of headers and utterances.

### Utterance

```
talkbank-model/src/model/file/utterance/core.rs
```

```rust
pub struct Utterance {
    pub preceding_headers: SmallVec<[Header; 2]>,    // @Comment, @Bg, etc.
    pub main: MainTier,                               // *SPEAKER: words .
    pub dependent_tiers: SmallVec<[DependentTier; 3]>, // %mor, %gra, ...
    pub alignments: Option<AlignmentSet>,             // computed summary
    pub alignment_diagnostics: Vec<ParseError>,       // (serde skip)
    pub parse_health: Option<ParseHealth>,            // (serde skip)
    pub utterance_language: UtteranceLanguage,
    pub language_metadata: UtteranceLanguageMetadata,
}
```

Key design: `dependent_tiers` stores tiers in their original order as they
appeared in the file.

### DependentTier enum

```
talkbank-model/src/model/dependent_tier/types.rs
```

```rust
#[serde(tag = "type", content = "data")]
pub enum DependentTier {
    // Structured linguistic tiers stored as markers:
    Mor(MorTierMarker),
    Gra(GraTierMarker),
    Pho(PhoTierMarker),
    Mod(PhoTierMarker),
    Sin(SinTierMarker),
    Wor(WorTierMarker),
    // Structured tiers stored directly:
    Act(ActTier),
    Cod(CodTier),
    // Text tiers:
    Com(ComTier), Exp(ExpTier), Gpx(GpxTier), ...
    // User-defined:
    UserDefined(UserDefinedDependentTier),
}
```

### Tier Markers (the "pending items" pattern)

Structured tiers use a **marker** type instead of storing the full parsed tier.
Example:

```rust
pub struct MorTierMarker {
    pub span: Span,             // (serde skip)
    pub item_count: usize,      // number of parsed items
    pub chunk_count: usize,     // including clitics
    pub terminator: Option<String>,
    pub raw_chat: Option<String>,         // (serde skip) roundtrip fallback
    pub pending_items: Vec<Mor>,          // (serde skip) waiting for alignment
    pub extra_items: Vec<Mor>,            // (serde skip) surplus beyond alignment
}
```

The `pending_items` field holds the parsed tier items. During the alignment
phase, these are "taken" from the marker and distributed into per-word alignment
fields on the main tier's `Word` objects. Items beyond the main tier's alignable
count go into `extra_items`.

This "marker + pending items" pattern exists because:

1. The parser produces the full tier immediately
2. But the alignment phase needs to distribute items 1:1 onto words
3. After distribution, the tier can be reconstructed ("materialized") from the
   per-word state for CHAT roundtrip serialization
4. For JSON, the per-word alignment fields serialize naturally via serde

### Word (with embedded alignment fields)

```
talkbank-model/src/model/content/word/types.rs
```

```rust
pub struct Word {
    pub span: Span,
    pub raw_text: String,
    pub cleaned_text: String,
    pub content: WordContents,
    pub category: Option<WordCategory>,
    pub form_type: Option<FormType>,
    pub untranscribed: Option<UntranscribedStatus>,
    pub lang: Option<WordLanguageMarker>,
    pub part_of_speech: Option<String>,

    // Embedded alignment fields (computed during alignment phase):
    pub mor_alignment: WordMorphologyAlignment,   // → %mor
    pub timing_alignment: WordTimingAlignment,    // → %wor
    pub pho_alignment: PhoItemAlignment,          // → %pho
    pub mod_alignment: PhoItemAlignment,          // → %mod
    pub sin_alignment: SinItemAlignment,          // → %sin
}
```

All alignment fields default to `Uncomputed` and are conditionally serialized
(`skip_serializing_if = "is_uncomputed"`). After alignment runs, they hold the
concrete dependent tier item for that word.

### Alignment state enums

Each alignment field uses an explicit state enum. Example:

```rust
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum WordMorphologyAlignment {
    Uncomputed,              // pipeline hasn't run
    NotAlignable,            // word excluded from alignment
    Aligned(Box<Mor>),       // concrete %mor item for this word
    Missing,                 // word is alignable but no %mor item
    SkippedParseHealth,      // tier tainted, alignment skipped
}
```

Similar enums: `WordTimingAlignment` (with `Timed(WordTiming)` and `Untimed`
variants), `PhoItemAlignment`, `SinItemAlignment`, `MorChunkGraAlignment` (for
`%gra` relations embedded on `%mor` chunks).

---

## Stage 4: Validation

### The Validate trait

```
talkbank-model/src/validation/trait.rs
```

```rust
pub trait Validate {
    fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink);
}
```

All model types implement this for hierarchical validation.

### ChatFile::validate()

```
talkbank-model/src/model/file/chat_file/validate.rs
```

1. Builds `ValidationContext` from headers (languages, participants, CA mode)
2. Validates header structure (required headers, duplicates)
3. Validates each header individually
4. Validates each utterance (speaker, main tier content, dependent tiers)
5. Cross-utterance patterns (quotation balance, terminator pairing)
6. Bullet timestamp monotonicity
7. Temporal constraints (speaker self-overlap E704)

### ChatFile::validate_with_alignment()

```
talkbank-model/src/model/file/chat_file/validate.rs
```

This method runs alignment **first**, then validation:

```rust
pub fn validate_with_alignment(
    &mut self, errors: &impl ErrorSink, filename: Option<&str>,
) {
    let context = build_validation_context(...);
    for line in &mut self.lines {
        if let Line::Utterance(utterance) = line {
            utterance.compute_alignments(&context);   // ← alignment
            utterance.compute_language_metadata(...);
        }
    }
    self.validate(errors, filename)  // ← validation (emits alignment errors)
}
```

---

## Stage 5: Alignment

### Utterance::compute_alignments()

```
talkbank-model/src/model/file/utterance/metadata/alignment.rs
```

This is the core alignment orchestration for a single utterance. It:

1. **Resets** all embedded alignment states on words to `Uncomputed`
2. **Takes** pending items from markers:
   - `take_pending_morphology_items()` -- from `MorTierMarker.pending_items`
   - `take_pending_relation_items()` -- from `GraTierMarker.pending_items`
   - `take_pending_wor_content()` -- from `WorTierMarker.pending_words`
   - `take_pending_phonology_items()` -- from `PhoTierMarker.pending_items`
   - `take_pending_sin_items()` -- from `SinTierMarker.pending_items`
3. **Checks ParseHealth** before each alignment pair:

   | Alignment        | Required flags              |
   |------------------|-----------------------------|
   | Main → `%mor`    | `main_clean && mor_clean`   |
   | `%mor` → `%gra`  | `mor_clean && gra_clean`    |
   | Main → `%pho`    | `main_clean && pho_clean`   |
   | Main → `%mod`    | `main_clean && mod_clean`   |
   | Main → `%wor`    | `main_clean && wor_clean`   |
   | Main → `%sin`    | `main_clean && sin_clean`   |

   If either tier is tainted, alignment is **skipped** and a warning emitted.

4. **Runs alignment algorithms** (counting, pairing, error detection):
   - `align_main_to_mor(&self.main, &mor_tier)` → `MorAlignment`
   - `align_mor_to_gra(&mor_tier, &gra_tier)` → `GraAlignment`
   - `align_main_to_pho(...)` via `build_phonology_alignment_from_counts()`
   - `align_main_to_wor(&self.main, &wor_tier)` → `WorAlignment`
   - `align_main_to_sin(...)` via item counting

5. **Embeds** aligned items into per-word fields. For each word in the main
   tier, if it is alignable in the given domain, the corresponding dependent
   tier item is placed into its alignment field:

   ```
   word.mor_alignment  = Aligned(Box::new(mor_item))
   word.timing_alignment = Timed(WordTiming { start_ms, end_ms })
   word.pho_alignment  = Aligned(Box::new(pho_item))
   word.sin_alignment  = Aligned(Box::new(sin_item))
   ```

   For `%gra`, relations are embedded on `%mor` chunks:

   ```
   mor_chunk.gra_alignment = MorChunkGraAlignment::Aligned(relation)
   ```

6. **Stores extras** back on markers. Items beyond the main tier's alignable
   count go into `marker.extra_items`.

7. **Stores results** in `self.alignments` (`AlignmentSet`) and error
   diagnostics in `self.alignment_diagnostics`.

### Alignment algorithms

All alignment functions follow the same pattern:

```
talkbank-model/src/alignment/{mor,pho,sin,wor,gra/align}.rs
```

1. **Count** alignable items from source tier (cheap, count-only)
2. **Count** items on target tier
3. **Pair** 1:1 for `min(source_count, target_count)` indices
4. If counts differ:
   a. **Extract** full item text (lazy, only for error messages)
   b. **Format** mismatch error with positional diff
   c. **Add placeholder** pairs for extras
5. Return alignment with pairs and errors

The counting is centralized in:

```
talkbank-model/src/alignment/helpers/count.rs   -- count_alignable_content()
talkbank-model/src/alignment/helpers/rules.rs   -- word_is_alignable(), etc.
talkbank-model/src/alignment/helpers/domain.rs  -- AlignmentDomain enum
```

See `resources/spec/docs/ALIGNMENT_RULES.md` for the full per-tier alignment rules.

---

## Stage 6: JSON Serialization

### Serde-driven serialization

The entire model derives `Serialize` and `Deserialize`. Key serde attributes:

| Type | Tag strategy | Example JSON |
|------|-------------|-------------|
| `Line` | `#[serde(tag = "line_type")]` | `{"line_type": "utterance", ...}` |
| `DependentTier` | `#[serde(tag = "type", content = "data")]` | `{"type": "Mor", "data": {...}}` |
| `WordMorphologyAlignment` | `#[serde(tag = "state", content = "value")]` | `{"state": "aligned", "value": {...}}` |
| Alignment fields | `skip_serializing_if = "is_uncomputed"` | Omitted entirely when not computed |
| Runtime-only fields | `#[serde(skip)]` | `span`, `raw_chat`, `parse_health`, etc. |

### What appears in JSON for an aligned word

When alignment has been computed, a word in the JSON output looks like:

```json
{
  "raw_text": "goed",
  "cleaned_text": "goed",
  "content": [{"type": "text", "text": "goed"}],
  "mor_alignment": {
    "state": "aligned",
    "value": {
      "pos": "v",
      "lemma": "go",
      "features": ["PAST"]
    }
  },
  "timing_alignment": {
    "state": "timed",
    "value": { "start_ms": 1000, "end_ms": 1400 }
  },
  "pho_alignment": {
    "state": "aligned",
    "value": { "text": "gowd" }
  }
}
```

When alignment has **not** been computed (no `--alignment` flag), the alignment
fields are omitted entirely from the JSON (via `skip_serializing_if`).

### Dependent tiers in JSON

The `dependent_tiers` array on each utterance still contains the tier **markers**
with their metadata (item counts, etc.). The structured content is embedded on
the words themselves:

```json
{
  "main": {
    "speaker": "CHI",
    "content": [
      { "type": "word", "raw_text": "want", "cleaned_text": "want",
        "mor_alignment": {"state": "aligned", "value": {"pos": "v", ...}},
        "timing_alignment": {"state": "timed", "value": {"start_ms": 100, ...}}
      },
      { "type": "word", "raw_text": "cookie", "cleaned_text": "cookie",
        "mor_alignment": {"state": "aligned", "value": {"pos": "n", ...}},
        "timing_alignment": {"state": "timed", "value": {"start_ms": 400, ...}}
      }
    ],
    "terminator": "."
  },
  "dependent_tiers": [
    {"type": "Mor", "data": {"item_count": 2, "chunk_count": 2, ...}},
    {"type": "Wor", "data": {"item_count": 2, ...}}
  ]
}
```

### talkbank-transform JSON module

```
talkbank-transform/src/json/mod.rs
```

```rust
pub fn to_json_validated<T: Serialize>(value: &T) -> JsonResult<String> {
    let json_string = serde_json::to_string(value)?;
    validate_json_string(&json_string)?;
    Ok(json_string)
}
```

The schema is loaded once via `LazyLock` from `schemas/chat-file/chat-file.schema.json`
and compiled into a `jsonschema::Validator`. Every production JSON output is
validated against this schema to catch drift between the model and schema.

---

## Stage 7: CHAT Roundtrip (Materialization)

For CHAT output (not JSON), the embedded per-word alignment data must be
reconstructed into full tier lines. This is the "materialization" step.

```
talkbank-model/src/model/file/utterance/serialization.rs
```

The `WriteChat` impl for `Utterance`:

```rust
impl WriteChat for Utterance {
    fn write_chat(&self, w: &mut W) -> std::fmt::Result {
        // Write preceding headers
        // Write main tier
        let materialized_mor = self.materialized_mor_tier();
        let materialized_gra = self.materialized_gra_tier();
        let materialized_wor = self.materialized_wor_tier();
        let materialized_pho = self.materialized_pho_tier();
        let materialized_mod = self.materialized_mod_tier();
        let materialized_sin = self.materialized_sin_tier();

        // Write dependent tiers in original order
        for tier in &self.dependent_tiers {
            write_dependent_tier(tier, &materialized, w)?;
        }
        Ok(())
    }
}
```

Each `materialized_*_tier()` method:

1. Walks the main tier words
2. Collects their embedded alignment states (e.g., `Aligned(mor)` → `mor`)
3. Appends `extra_items` from the marker
4. Verifies count matches `marker.item_count`
5. Returns `Some(tier)` on success

If materialization fails (e.g., `Uncomputed` states, count mismatch), it
returns `None` and `write_dependent_tier()` falls back to `marker.raw_chat`
(the original tier text preserved during parsing). This ensures 100% roundtrip
fidelity.

The fallback chain for structured tiers:

```rust
DependentTier::Mor(marker) => match materialized.mor {
    Some(tier) => tier.write_chat(w),          // reconstructed from per-word state
    None => match marker.raw_chat.as_deref() {
        Some(raw) => w.write_str(raw),         // original text
        None => w.write_str("%mor:\t"),         // last resort: empty tier
    },
},
```

---

## CLI Invocation

### `chatter to-json`

```
chatter-cli/src/commands/json.rs
```

```rust
pub fn chat_to_json(
    input: &PathBuf, output: Option<&PathBuf>,
    pretty: bool, validate: bool, alignment: bool,
    skip_schema_validation: bool,
) {
    let content = fs::read_to_string(input)?;
    let mut options = ParseValidateOptions::default();
    if validate { options = options.with_validation(); }
    if alignment { options = options.with_alignment(); }
    let json = if skip_schema_validation {
        talkbank_transform::chat_to_json_unvalidated(&content, options, pretty)
    } else {
        talkbank_transform::chat_to_json(&content, options, pretty)
    }?;
    // Write to file or stdout
}
```

### `chatter from-json` (reverse direction)

```rust
pub fn json_to_chat(input: &PathBuf, output: Option<&PathBuf>) {
    let content = fs::read_to_string(input)?;
    let chat_file: ChatFile = serde_json::from_str(&content)?;
    let chat_text = chat_file.to_chat_string();  // WriteChat trait
    // Write to file or stdout
}
```

---

## Key Crate Roles

| Crate | Role |
|-------|------|
| `talkbank-parser` | CST parsing + CST→AST conversion |
| `talkbank-model` | `ParseOutcome`, AST types, validation, alignment, `ErrorSink`, `ParseError`, error codes |
| `talkbank-transform` | Pipeline orchestration (parse + validate + convert), JSON serialization + schema validation, `ParseValidateOptions` |
| `chatter-cli` | CLI commands invoking the pipeline |

---

## Key Architectural Patterns

### Embed-then-Materialize

Dependent tier items are parsed into full tier objects, then distributed into
per-word alignment fields during the alignment phase. For JSON output, the
embedded fields serialize naturally. For CHAT roundtrip, the items are collected
back from the words into reconstructed tier objects. If reconstruction fails,
the original text (`raw_chat`) is used as a fallback.

### Parse Health Gating

Each tier tracks whether it was parsed cleanly or through error recovery
(`ParseHealth`). Before running any alignment check, the system verifies both
tiers are clean. Tainted tiers get `SkippedParseHealth` alignment state instead
of potentially spurious mismatch errors.

### Conditional Serialization

Alignment fields on `Word` use `skip_serializing_if = "is_uncomputed"` so they
are omitted from JSON when the alignment pipeline hasn't run. This means the
same `ChatFile` type can produce either a "parse-only" JSON (no alignment data)
or a "fully aligned" JSON depending on the pipeline options.

### Marker + Pending Items

Structured dependent tiers use marker types with `pending_items` (consumed
during alignment) and `extra_items` (surplus). The marker itself serializes to
JSON with metadata (`item_count`, `chunk_count`), while the actual content is
embedded on words. The `raw_chat` field (skipped from JSON) provides the
original text for CHAT roundtrip fallback.

---

Last Updated: 2026-02-12

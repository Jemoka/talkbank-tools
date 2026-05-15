# Error Specifications

This directory contains formal markdown specifications for CHAT format validation errors.

## Purpose

Error specifications serve multiple purposes:
1. **Documentation**: Human-readable descriptions of what each error means
2. **Test Generation**: Automated generation of validation test fixtures
3. **Consistency**: Ensures error messages and behavior are well-defined
4. **Validation**: Machine-checkable specs that can be validated for completeness

## Directory Structure

```
resources/spec/errors/
├── README.md                    # This file
├── ERROR_SPEC_FORMAT.md         # Specification format documentation
├── E241_illegal_untranscribed_marker.md
├── E522_undefined_participant.md
├── E604_gra_without_mor.md
└── ... (other error specs)
```

## Spec Format

Each error specification is a markdown file with the following sections:

```markdown
# Error Title

## Description
Brief description of the error

## Metadata
- **Error Code**: E###
- **Category**: error_category
- **Level**: word|utterance|tier|header|file
- **Layer**: parser|validation

## Example
### CHAT Input
```chat
... example CHAT code that triggers the error ...
```

### Expected Behavior
Description of what should happen

## CHAT Specification Rule
Link to relevant section in CHAT manual

## Notes
Additional implementation notes
```

See [ERROR_SPEC_FORMAT.md](ERROR_SPEC_FORMAT.md) for complete format documentation.

## Workflow

### Creating a New Error Spec

#### Option 1: Manual Creation

1. Create a new markdown file named `E###_descriptive_name.md`
2. Follow the format in ERROR_SPEC_FORMAT.md
3. Validate the spec:
   ```bash
   cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- --spec-dir spec/errors
   ```

#### Option 2: Generate from Error Corpus

If an error corpus file already exists:

**Step 1: Generate specs from corpus**
```bash
cd spec/tools
cargo run --bin corpus_to_specs -- \
  --corpus-dir tests/error_corpus \
  --spec-dir ../spec/errors
```

**Step 2: Fix layer classifications**
```bash
cargo run --bin fix_spec_layers -- --spec-dir ../spec/errors
```

**Step 3: Enhance specs with manual references**
```bash
cargo run --bin enhance_specs -- --spec-dir ../spec/errors
```

**Step 4: Validate specs**
```bash
cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- --spec-dir spec/errors
```

This automated pipeline generates `E###_auto.md` files with:
- Correct layer classification (parser vs validation)
- CHAT manual references
- Proper Expected Behavior text
- 100% validation pass rate

### Generating Validation Tests

Once you have error specs, generate validation tests:

```bash
cd spec/tools
cargo run --bin gen_validation_tests -- \
  --spec-dir ../spec/errors \
  --output-dir crates/talkbank-parser-tests/tests/generated \
  --fixture-dir crates/talkbank-parser-tests/tests/fixtures/errors
```

This generates:
- Test fixture files (`.cha` files with error examples)
- Test code (`generated_validation_tests.rs`)

### Implementing Validators

After generating tests:

1. Run tests to verify they fail (TDD red phase):
   ```bash
   cd ..
   cargo nextest run -p talkbank-parser-tests -E 'test(validation_tests)'
   ```

2. Implement the validator in the appropriate module:
   - **E2xx** (word errors): `talkbank-model/src/validation/word/`
   - **E3xx** (main tier): `talkbank-model/src/validation/main_tier.rs`
   - **E4xx** (dependent tier): `talkbank-model/src/validation/utterance/tiers.rs`
   - **E5xx** (header): `talkbank-model/src/validation/header/`
   - **E6xx** (alignment): `talkbank-model/src/alignment/`

3. Implement validation following existing patterns:
   ```rust
   impl Validate for MyType {
       fn validate(&self, context: &ValidationContext, errors: &impl ErrorSink) {
           if bad_condition {
               errors.report(ParseError::new(
                   ErrorCode::MyError,
                   Severity::Error,
                   SourceLocation::new(self.span),
                   ErrorContext::new(&self.text, self.span, &self.text),
                   "Error message",
               ).with_suggestion("How to fix"));
           }
       }
   }
   ```

4. Run tests to verify they pass (TDD green phase):
   ```bash
   cargo nextest run -p talkbank-parser-tests -E 'test(validation_tests)'
   ```

5. Verify no regressions on reference corpus:
   ```bash
   cd ..
   bazel build //... && bazel test //...
   ```

## Tools

### validate_error_specs

Validates that error specs follow the correct format and have proper metadata.

```bash
cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- --spec-dir spec/errors
```

Checks:
- Required sections present
- Metadata fields complete
- Layer classification correct (parser vs validation)
- Error code format valid

### corpus_to_specs

Converts existing error corpus files to markdown specs.

```bash
cargo run --bin corpus_to_specs -- \
  --corpus-dir tests/error_corpus \
  --spec-dir ../spec/errors \
  [--overwrite]
```

Options:
- `--overwrite`: Overwrite existing spec files

Generates `E###_auto.md` files with basic metadata, descriptions, and CHAT examples extracted from `@Comment` headers.

### fix_spec_layers

Automatically corrects layer classification (parser vs validation) based on actual parse behavior.

```bash
cargo run --bin fix_spec_layers -- \
  --spec-dir ../spec/errors \
  [--dry-run]
```

**How it works**:
- Tests if CHAT example parses successfully using tree-sitter
- If parsing **fails** → layer should be `parser` (structural error)
- If parsing **succeeds** → layer should be `validation` (semantic error)

Run this after generating specs from corpus to ensure correct layer classification.

### enhance_specs

Enhances auto-generated specs with CHAT manual references and corrected Expected Behavior text.

```bash
cargo run --bin enhance_specs -- \
  --spec-dir ../spec/errors \
  [--dry-run]
```

**Enhancements**:
- Adds CHAT manual links to CHAT Rule section (contextual by error category)
- Fixes Expected Behavior text to match layer:
  - Parser layer: "parser should reject this CHAT input"
  - Validation layer: "parser should succeed, validation should report error"

### gen_validation_tests

Generates test fixtures and test code from error specs.

```bash
cargo run --bin gen_validation_tests -- \
  --spec-dir ../spec/errors \
  --output-dir crates/talkbank-parser-tests/tests/generated \
  --fixture-dir crates/talkbank-parser-tests/tests/fixtures/errors
```

Generates:
- `.cha` test fixture files (one per spec)
- `generated_validation_tests.rs` (test module)
- `generated_validation_tests_body.rs` (test implementations)

## Status

### Specification Coverage

**Total Specs**: 62 files
- **Manual specs**: 3 (E241, E522, E604) - Fully documented validation-layer errors
- **Auto-generated specs**: 59 - Generated from error corpus, enhanced with CHAT manual links

**Layer Classification**:
- **Parser layer**: 51 specs (structural/syntactic errors caught by grammar)
- **Validation layer**: 3 specs (semantic errors caught post-parse)
- **Other**: 8 specs (non-E### codes: Alignment, Complex, Events, etc.)

### Implemented Validators with Specs

- **E241**: Illegal Untranscribed Marker ('xx' should be 'xxx') ✅
- **E522**: Undefined Participant in Utterance ✅
- **E604**: %gra Tier Without Required %mor Tier ✅
- **E401**: Duplicate Dependent Tiers ✅ (implemented, test may need review)

### Enhancement Status

All 59 auto-generated specs have been enhanced with:
- ✅ **CHAT manual references** - Links to https://talkbank.org/0info/manuals/CHAT.pdf with contextual descriptions
- ✅ **Corrected Expected Behavior** - Text matches actual layer (parser vs validation)
- ⏳ **Basic descriptions** - Concise and accurate, could be enhanced further

For guidelines on manually improving descriptions and examples, see [SPEC_ENHANCEMENT_GUIDE.md](SPEC_ENHANCEMENT_GUIDE.md).

### Error Corpus

The legacy error corpus (`tests/error_corpus/`) contains 101 test files covering ~60 unique error codes. These files use `@Comment` headers to document expected errors and can be converted to formal specs using `corpus_to_specs`.

## Contributing

When adding a new validation rule:

1. Create error spec (or generate from corpus file)
2. Validate spec format
3. Generate tests
4. Implement validator (TDD: fail → pass)
5. Verify reference corpus still passes
6. Commit spec, tests, and validator together

## See Also

- [ERROR_SPEC_FORMAT.md](ERROR_SPEC_FORMAT.md) - Detailed format specification
- [talkbank-model validation CLAUDE.md](../../crates/talkbank-model/src/validation/CLAUDE.md) - Validator implementation patterns
- [Root CLAUDE.md](../../CLAUDE.md) - TDD and testing requirements

---

Last Updated: 2026-01-19

# Error Specification Enhancement Guide

This document provides guidelines for improving auto-generated error specifications with better descriptions, examples, and CHAT Rule sections.

## Current Status

### Auto-Generated Specs: 59 files

All auto-generated specs (`*_auto.md`) have been enhanced with:
- ✅ **CHAT manual references** - All specs now link to https://talkbank.org/0info/manuals/CHAT.pdf with contextual descriptions
- ✅ **Corrected Expected Behavior** - All specs now correctly describe parser vs validation behavior
- ⏳ **Basic descriptions** - Descriptions are concise but could be more explanatory

### Manual Specs: 3 files

These specs serve as examples of good documentation:
- `E241_illegal_untranscribed_marker.md` - Detailed description, clear rule
- `E522_undefined_participant_in_utterance.md` - Well-documented validation error
- `E604_gra_tier_without_mor_tier.md` - Clear dependent tier requirement

## Description Quality

### Good Descriptions (Specific and Contextual)

Examples of well-written descriptions from auto specs:
- E360: "Speaker already used in same bullet group"
- E529: "Nested background with identical label"
- E705: "Mor count mismatch - too few mor items"

### Basic Descriptions (Functional but Brief)

Examples that could be improved:
- E202: "Missing form type after @" → Could explain what @ symbols represent
- E220: "Unsupported main tier content type" → Could specify what content types are supported
- E304: "Expected terminator not found" → Could list valid terminators (., ?, !)

### Generic Descriptions (Need Enhancement)

A few specs have placeholder descriptions:
- Alignment_auto.md: "Auto-generated from corpus"
- Complex_auto.md: "Auto-generated from corpus"
- E342_auto.md: "Auto-generated from corpus"

## Enhancement Guidelines

### 1. Writing Better Descriptions

**Formula**: `[What] + [Why it's an error] + [What's expected instead]`

#### Example: E304 (Missing Terminator)

**Current**:
```markdown
## Description

Expected terminator not found
```

**Enhanced**:
```markdown
## Description

Every main tier utterance in CHAT must end with a terminator character. The parser expects one of three terminators: period (.), question mark (?), or exclamation point (!). An utterance without a terminator is syntactically invalid and cannot be parsed.
```

#### Example: E507 (Empty @Languages)

**Current**:
```markdown
## Description

@Languages header cannot be empty
```

**Enhanced**:
```markdown
## Description

The @Languages header must specify at least one three-letter language code (e.g., 'eng' for English, 'spa' for Spanish). An empty @Languages header violates CHAT format requirements and prevents the parser from determining the transcript's language context.
```

### 2. Enhancing CHAT Rule Sections

The auto-generated CHAT Rule sections provide general manual links. These can be enhanced with specific rule details.

#### Example: E304 (Missing Terminator)

**Current** (auto-generated):
```markdown
## CHAT Rule

See CHAT manual sections on main tier syntax and utterance structure. Every utterance must end with a terminator (., ?, or !). The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf
```

**Enhanced**:
```markdown
## CHAT Rule

**CHAT Manual Reference**: Main tier syntax (Section X.X)

Every main tier utterance must end with one of three terminators:
- `.` (declarative statement)
- `?` (question)
- `!` (exclamation or emphasis)

The terminator must be the last token on the line before any trailing whitespace. Utterances cannot end with other punctuation marks like commas or semicolons without a terminator.

**Manual**: https://talkbank.org/0info/manuals/CHAT.pdf
```

### 3. Adding Multiple Examples

For complex errors, multiple examples showing different triggers can be helpful.

#### Example: E241 (Illegal Untranscribed Marker)

The manual spec shows how to document variations:

```markdown
## Example

### Using 'xx' (invalid)
```chat
*CHI:	I said xx today .
```

### Correct usage of 'xxx'
```chat
*CHI:	I said xxx today .
```
```

### 4. Notes Section Enhancement

The Notes section can document:
- Implementation status (parser vs validator)
- Known edge cases
- Related error codes
- Migration notes from legacy parsers

#### Example:

```markdown
## Notes

- Auto-generated from error corpus file: `E304_missing_terminator.cha`
- **Implementation**: This is a parser-layer error - the tree-sitter grammar rejects utterances without terminators
- **Related errors**: E303 (unexpected content), E305 (expected main tier content)
- **Edge cases**: Terminators inside quotations or postcodes don't count as utterance terminators
```

## Priority for Enhancement

### High Priority (User-Facing Errors)

These errors are commonly encountered and should have excellent documentation:

**Main Tier** (E3xx):
- E301 - Empty speaker code
- E304 - Expected terminator not found
- E308 - Invalid speaker format

**Headers** (E5xx):
- E507 - Empty @Languages
- E522 - Empty @Participants (note: conflicts with E522 manual spec)
- E505 - Invalid @ID format

### Medium Priority (Developer-Facing)

**Dependent Tiers** (E4xx, E7xx):
- E702 - Invalid MOR chunk format
- E705 - Mor count mismatch (too few)
- E706 - Mor count mismatch (too many)

### Low Priority (Rare Errors)

**Word-level** (E2xx):
- E202 - Missing form type after @
- E210 - Replacement not allowed for phonological fragment

## Workflow for Manual Enhancement

1. **Choose a spec file** from the priority list above
2. **Read the error corpus file** (path is in the Notes section)
3. **Understand the trigger** - What causes this error?
4. **Write enhanced description** - Follow the formula: what + why + expected
5. **Improve CHAT Rule section** - Add specific rule details from manual
6. **Add examples if helpful** - Show both invalid and valid cases
7. **Update Notes** - Add implementation details, edge cases
8. **Validate** - Run `cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml`
9. **Test** - Ensure generated tests still work

## Example Enhancement: Complete File

See `E241_illegal_untranscribed_marker.md` for an example of a fully enhanced spec with:
- Clear, detailed description
- Comprehensive CHAT Rule section
- Good example with context
- Helpful notes on implementation

## Tools

### Validate Specs
```bash
cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- --spec-dir spec/errors
```

### Regenerate Tests
```bash
cargo run --bin gen_validation_tests -- \
  --spec-dir ../spec/errors \
  --output-dir crates/talkbank-parser-tests/tests/generated \
  --fixture-dir crates/talkbank-parser-tests/tests/fixtures/errors
```

## Contributing

When enhancing specs:
1. Keep descriptions accurate and concise
2. Link to CHAT manual where possible
3. Provide concrete examples
4. Document edge cases in Notes
5. Maintain consistency with other specs

Focus on clarity and usefulness for:
- Users encountering errors in their CHAT files
- Developers implementing validators
- Maintainers updating the CHAT format

---

Last Updated: 2026-01-19

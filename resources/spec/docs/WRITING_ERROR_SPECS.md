# Writing Error Specs — Quick Reference

See [ERROR_SPEC_FORMAT.md](ERROR_SPEC_FORMAT.md) for the complete format
reference. This page covers the practical workflow.

## Adding a New Error Spec

1. **Create the file**: `resources/spec/errors/E{NNN}.md` or `E{NNN}_{suffix}.md`

2. **Write the spec** with these sections:
   ```markdown
   # E{NNN}: ErrorName

   ## Description
   What this error means.

   ## Metadata
   - **Error Code**: E{NNN}
   - **Category**: Parser error
   - **Level**: utterance
   - **Layer**: parser

   ## Example 1
   **Trigger**: What triggers this error

   ```chat
   @UTF8
   @Begin
   @Languages:	eng
   @Participants:	CHI Child
   @ID:	eng|corpus|CHI|||||Child|||
   *CHI:	the bad input here .
   @End
   ```

   ## Notes
   - Any implementation notes.
   ```

3. **Choose Layer correctly**:
   - `parser` → input causes `parse_chat_file()` to return `Err`
   - `validation` → parser succeeds but error is reported via error sink

4. **Verify the example triggers the right code**:
   ```bash
   chatter validate /tmp/test.cha --force
   ```

5. **Regenerate tests**:
   ```bash
   just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs
   ```

6. **Run tests**:
   ```bash
   cargo nextest run -p talkbank-parser-tests --release
   ```

## Common Mistakes

| Mistake | Symptom | Fix |
|---------|---------|-----|
| `Layer: parser` on a recovery error | "Expected parse error but parsing succeeded" | Change to `Layer: validation` |
| Example triggers wrong code | Test fails with wrong error code | Add `**Expected Error Codes**: E{actual}` or fix example |
| Missing `Status: not_implemented` | Test runs but fails | Add status if code isn't wired up yet |
| Wrong code fence info string | Parse method mismatch | Use `chat` for full files |

## Validating Specs

```bash
# Check all spec format/layer correctness
cargo run --bin validate_error_specs --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml

# Check coverage (all error codes have specs)
cargo run --bin coverage --manifest-path crates/spec/talkbank-spec-testgen/Cargo.toml -- --spec-dir spec --errors
```

---
Last Updated: 2026-02-27

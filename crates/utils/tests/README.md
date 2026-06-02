# Tests

Integration tests and fixtures for the talkbank-tools workspace.

## Running Tests

```bash
cargo nextest run --workspace           # All tests (parallel per-test, preferred)
cargo test --doc                        # Doctests (nextest can't run these)
bazel build //... && bazel test //...                             # Pre-merge gates including parser equivalence
```

## Test Organization

### Core Corpus Tests
| File | Purpose |
|------|---------|
| `roundtrip_corpus.rs` | Parse → serialize → parse roundtrip on reference corpus |
| `recovery_corpus.rs` | Parser error recovery on malformed input |
| `error_corpus.rs` | Legacy error corpus (prefer `resources/spec/errors/` for new tests) |
| `single_file_roundtrip.rs` | Roundtrip a single file (useful for debugging) |

### Validation Tests
| File | Purpose |
|------|---------|
| `alignment_validation.rs` | %mor/%wor tier alignment checks |
| `alignment_corpus_tests.rs` | Alignment on reference corpus files |
| `validation_gaps.rs` | Known validation gaps (tracked for future work) |
| `test_validation_comprehensive.rs` | Comprehensive validation scenario tests |

### Parser Tests
| File | Purpose |
|------|---------|
| `parse_chat_file_tests.rs` | Full-file parsing integration tests |
| `parse_header_tests.rs` | Header parsing tests |
| `header_roundtrip_tests.rs` | Header roundtrip fidelity |
| `component_roundtrip_tests.rs` | Per-component roundtrip tests |
| `bare_timestamp_regression.rs` | Regression test for bare timestamp parsing |

### Participant / Speaker Tests
| File | Purpose |
|------|---------|
| `participant_integration.rs` | Participant header/ID integration |
| `test_participant_errors.rs` | Participant validation error tests |
| `test_speaker_validation.rs` | Speaker tier validation |

### TUI / Display Tests
| File | Purpose |
|------|---------|
| `tui_*.rs` (9 files) | Terminal UI formatting, alignment, underline markers |

### Generated Tests
| Directory | Purpose |
|-----------|---------|
| `generated/` | Auto-generated from `resources/spec/errors/` and `resources/spec/constructs/` — **do not edit** |

### Other
| File | Purpose |
|------|---------|
| `mutation_tests.rs` | Property-based mutation testing |
| `generate_schema.rs` | JSON schema generation test |
| `build_corpus_manifest.rs` | Corpus manifest building test |

## Adding New Tests

- **Error codes**: Create or update a spec in `resources/spec/errors/`, then `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`
- **Construct coverage**: Create a spec in `resources/spec/constructs/`, then `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs`
- **Integration tests**: Add a new `.rs` file here with a descriptive name

See [resources/spec/errors/README.md](../../../resources/spec/errors/README.md) for the spec format.

---
Last Updated: 2026-06-01

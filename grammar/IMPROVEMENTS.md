# Grammar Improvement Ideas

**Status:** Reference
**Last updated:** 2026-03-12

Ideas for improving the tree-sitter CHAT grammar, informed by studying
[tree-sitter-scala](https://github.com/tree-sitter/tree-sitter-scala) (a
mature, production-grade grammar with 1,912 lines, 20 externals, 44 conflicts,
and multi-binding distribution). While Scala is a programming language and CHAT
is a transcript format, several patterns transfer well.

None of these are urgent — the grammar is architecturally sound. These are
polish items for editor integration and maintainability.

## Query File Enhancements

### Expand `highlights.scm` with predicate filters

**Current state:** 41 lines, 18 capture groups. Covers major structure but
treats all words uniformly and lacks conditional highlighting.

**What Scala does:** 179 lines of highlight queries with predicate filters like
`#match? @type "^[A-Z]"` to conditionally highlight based on node text content.

**Ideas for CHAT:**

```scheme
; Highlight known header prefixes differently from unsupported ones
(unsupported_header) @warning

; Distinguish MOR POS categories by prefix pattern
(mor_pos) @type.builtin
; Could add: (#match? @type.builtin "^(n|v|adj|adv|pro)")
; to highlight open-class POS differently from closed-class

; Highlight error-related annotations more prominently
(error_marker_annotation) @error
(best_guess_annotation) @warning

; Timing bullets (currently not captured)
(inline_bullet) @number

; Overlap markers
(overlap_begin) @punctuation.bracket
(overlap_end) @punctuation.bracket

; Postcodes
(postcode) @attribute
```

**Effort:** Small. Can be done incrementally — each new capture group is
independent.

### Add `indents.scm` for auto-indentation

**Current state:** No `indents.scm` exists.

**What Scala does:** Uses `@indent.begin` and `@indent.end` captures to tell
editors when to increase/decrease indentation. Editors like Neovim consume this
for automatic indentation.

**Why this matters for CHAT:** CHAT continuation lines use tab indentation.
When a user hits Enter in the middle of a long utterance or header value, the
editor should auto-insert a tab to continue the line. Similarly, after a main
tier line (`*CHI:\t`), dependent tiers (`%mor:\t`) maintain the same indentation
level.

**Sketch:**

```scheme
; After a main tier speaker+tab, maintain indent for dependent tiers
(main_tier) @indent.begin
(utterance) @indent.end

; Continuation lines within headers
(free_text
  (continuation) @indent.begin)
```

**Effort:** Small. Requires testing with the VS Code extension to verify
behavior matches expectations.

### Expand `locals.scm` for richer speaker scoping

**Current state:** 14 lines. Defines document scope, speaker definitions in
`@Participants`, and speaker references in main tiers.

**What Scala does:** Full local variable scoping — definition sites, reference
sites, and scope boundaries. This enables IDE features like "go to definition"
and "find all references."

**Ideas for CHAT:**

```scheme
(document) @local.scope

; Speaker definitions in @Participants
(participant
  code: (speaker) @local.definition)

; Speaker references in main tiers
(main_tier
  speaker: (speaker) @local.reference)

; Speaker references in @ID headers (the speaker code field)
(id_header
  (id_contents
    (id_participant
      (speaker) @local.reference)))

; Speaker references in dependent tiers (implicit — the tier belongs
; to the preceding main tier's speaker)
; Note: tree-sitter locals may not handle this implicit scoping well.
; May need LSP-level logic instead.
```

**Value:** Clicking a speaker code in a main tier line and jumping to its
`@Participants` definition. Hovering to see the participant's name/role from
the `@ID` header. Finding all turns by a speaker.

**Effort:** Medium. The basic definition→reference link is straightforward.
Cross-tier implicit speaker association may need LSP support rather than
query-level scoping.

### Expand `tags.scm` for richer navigation

**Current state:** 17 lines. Speaker turns, participant declarations, and
file anchors.

**What Scala does:** ~67 lines mapping classes, functions, imports, and
references for IDE symbol navigation (breadcrumbs, outline, jump-to-definition).

**Ideas for CHAT:**

```scheme
; Headers as section markers (for document outline)
(languages_header) @definition.module
(participants_header) @definition.module
(id_header) @definition.type
(options_header) @definition.type
(date_header) @definition.type

; Media references
(media_header) @reference.call

; Speaker turns (already present, but could add tier info)
(main_tier
  speaker: (speaker) @name) @definition.function

; Dependent tiers as sub-items of the preceding turn
(mor_dependent_tier) @reference.type
(gra_dependent_tier) @reference.type
```

**Value:** VS Code outline view showing headers as sections, speaker turns as
items, and dependent tiers as sub-items. Makes navigating long transcripts much
faster.

**Effort:** Small. Just adding capture patterns.

## Grammar-Level Improvements

### Named precedence groups

**Current state:** Numeric precedence values: `prec(1)`, `prec(5)`, `prec(8)`,
`prec(10)`.

**What Scala does:** Named precedence groups via the `precedences` array:

```javascript
precedences: $ => [
  ["mod", "soft_id"],
  ["end", "soft_id"],
],
```

This is more self-documenting than bare numbers.

**Idea for CHAT:**

```javascript
// Current:
token(prec(10, '+<'))           // structural
token(prec(5, /word_regex/))    // word-level
token(prec(8, /annotation/))    // annotation
token(prec(1, /event/))         // catch-all

// Could become named groups in the grammar's precedences array,
// with rules referencing group names instead of magic numbers.
```

**Caveat:** tree-sitter's `precedences` array resolves conflicts between named
groups, but individual `token(prec(N, ...))` calls still use numeric values.
The two systems are complementary, not interchangeable. Named groups work best
for inter-rule conflicts (declared in `conflicts`), while numeric `prec()` is
for intra-rule token priority. Worth investigating whether our 4-level numeric
scheme could be partially replaced.

**Effort:** Small. No behavioral change — just self-documentation.

### `inline` directive for intermediate rules

**Current state:** Not used.

**What Scala does:** Marks rules like `$._pattern`, `$._semicolon`,
`$._definition` as `inline`, which removes them from the parse tree. This
reduces tree size and simplifies queries — consumers see the concrete variants
directly without an intermediate wrapper node.

**Idea for CHAT:** Some of our rules exist purely as grouping wrappers. If
downstream consumers (Rust parser, queries) always match on the concrete
variants rather than the wrapper, inlining the wrapper would simplify the tree.

Candidates to evaluate:

```javascript
// If queries and parser never match on `contents` directly,
// but always on its children:
inline: $ => [
  // Evaluate each — only inline if no consumer matches on the wrapper
],
```

**Caveat:** Inlining changes the CST shape, which would require updating the
Rust parser's CST traversal code. Must be done carefully with the 4-step
verification sequence. Worth evaluating but not a priority.

**Effort:** Medium. Requires auditing Rust parser code for each candidate rule.

## CI and Testing

### Corpus smoke test in CI

**Current state:** CI runs `tree-sitter test` (corpus tests) and
`cargo test -p talkbank-parser-tests` (parser equivalence against 74 reference
files).

**What Scala does:** CI parses the entire Scala 2.13 and 3.5 standard libraries
(~15k files) as an additional smoke test. This catches regressions that corpus
tests miss — real-world code exercises parser paths that hand-crafted tests
don't.

**Idea for CHAT:** Add a CI step that parses a representative sample from
real corpora (e.g., 100–500 files from `data/childes-data/`) and asserts:
- Zero parser crashes (any file that causes tree-sitter to error is a bug)
- Error rate doesn't regress (count of `ERROR` nodes stays at or below baseline)

This would complement the 74-file reference corpus with broader coverage.

**Caveats:**
- Requires corpus data in CI (currently only in the `talkbank-dev` workspace).
  Could use a small curated subset checked into `resources/corpus/smoke/`.
- Must handle expected parse errors (CHAT files with intentional quirks) without
  false positives.

**Effort:** Medium. Need to curate a smoke test subset and write the assertion
script.

### Highlight query tests

**What Scala does:** Has `.scala` files in `test/highlight/` with embedded
comment markers that verify highlight queries produce expected captures:

```scala
func foo() {}
//    ^ function.call
```

tree-sitter CLI can verify these automatically.

**Idea for CHAT:** Create `test/highlight/` with `.cha` files verifying that
our `highlights.scm` captures are correct. Example:

```
@Begin
; ^ keyword
*CHI:	the dog .
; <- variable.builtin
;          ^ punctuation.special
```

**Value:** Prevents highlight regressions as the grammar evolves. Currently,
highlight correctness is only verified by manual inspection in VS Code.

**Effort:** Small. Create test files, add `tree-sitter test` invocation.

## Not Recommended for CHAT

These patterns from tree-sitter-scala were evaluated and found inappropriate:

### External scanner (C/C++ custom lexer)

Scala needs a 532-line C scanner for significant indentation, string
interpolation, and context-sensitive keywords. Our grammar deliberately avoids
an external scanner — CHAT's line-oriented structure works well with pure DSL
rules and precedence. Adding a C scanner would:
- Complicate the build (must compile C alongside generated parser)
- Make the grammar harder to understand and modify
- Add serialization/deserialization complexity for incremental parsing
- Our "opaque word token + Rust downstream parser" pattern achieves the same
  goal (handling complex leaf structure) without scanner state.

### Dynamic precedence

`prec.dynamic()` resolves ambiguities using runtime parse state. Scala needs
this for dangling-else and similar context-sensitive constructs. CHAT doesn't
have this class of ambiguity — our 4-level static precedence scheme is
sufficient.

### Large conflict count

Scala declares 44 conflicts. We have 5, which is appropriate for CHAT's simpler
structure. More conflicts mean more GLR ambiguity paths and slower parsing.
Don't add conflicts as a shortcut — prefer restructuring rules to eliminate
ambiguity.

## References

- [tree-sitter-scala grammar.js](https://github.com/tree-sitter/tree-sitter-scala/blob/master/grammar.js) — 1,912 lines, 11 precedence levels, 20 externals
- [tree-sitter-scala scanner.c](https://github.com/tree-sitter/tree-sitter-scala/blob/master/src/scanner.c) — 532 lines, indentation + string state
- [tree-sitter-scala queries/](https://github.com/tree-sitter/tree-sitter-scala/tree/master/queries) — highlights (179 lines), tags, locals, indents
- [tree-sitter-scala CI](https://github.com/tree-sitter/tree-sitter-scala/blob/master/.github/workflows/ci.yml) — multi-OS matrix, stdlib smoke tests
- Our grammar: `grammar/grammar.js` (2,045 lines), `grammar/GRAMMAR.md` (architecture)

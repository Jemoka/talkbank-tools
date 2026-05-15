# Reference XML Goldens

**Status:** Reference
**Last updated:** 2026-04-21 10:23 EDT

TalkBank-XML output produced by the legacy Java Chatter tool,
paired with the CHAT sources in `resources/corpus/reference/`. Used as the
parity oracle for the Rust XML emitter in
`crates/talkbank-transform/src/xml/`. See
`../docs/reference-xml-coverage-gaps.md` for the broad coverage
story (65 of 98 reference files have no golden, all because Java
Chatter rejected them).

## Intentionally absent goldens

Some reference files have no paired `.xml` here **by design**:

- **`content/deprecated-skip-bullet.cha`** — the `<bullet>-` skip
  flag was formally retired from CHAT on 2026-03-31 (confirmed
  by Brian MacWhinney). Modern Rust chatter intentionally strips
  the trailing dash during parsing and does not preserve the
  `skip="true"` attribute that Java Chatter emitted. The `.cha`
  fixture stays so the E360 validation path has a known input;
  the XML golden was removed because reproducing Java's
  pre-deprecation output would require a model change for a dead
  feature.

When adding a new `.cha` file to `resources/corpus/reference/`, the default
assumption is that a paired `.xml` golden should exist. Document
any intentional omission here.

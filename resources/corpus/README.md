# Reference Corpus

## Overview

The reference corpus at `resources/corpus/reference/` is the 100%-pass quality gate for all parser/grammar changes. Both parsers (tree-sitter and chumsky direct parser) must agree on every file. Every file is self-describing with `@Comment:` headers explaining what it demonstrates.

## Structure

```
resources/corpus/reference/
├── core/           12 files — document structure, headers, metadata
├── content/        11 files — main tier: words, terminators, linkers, pauses
├── annotation/      8 files — brackets, retraces, groups, scoping
├── tiers/          10 files — dependent tiers: %mor, %gra, %pho, %wor, etc.
├── ca/              4 files — conversation analysis: overlaps, intonation
├── audio/           5 files — audio-linked with %wor (5 .mp3 files)
└── languages/      20 files — one per language, morphotagged with %mor/%gra
```

**Total: 74 files** across 20 languages. **Node coverage: 334/334 concrete types (100%)**.

## Subdirectory Details

### `core/` — Document Structure (12 files)

Headers, metadata, pre-begin headers, episodes, comments, warnings, unsupported constructs, multiline continuation.

### `content/` — Main Tier Content (11 files)

Words (basic, special forms, prosody), terminators (standard, continuation, question, quote/special), pauses and events, linkers, postcodes/freecodes, language switching, quotations, media bullets, separators.

### `annotation/` — Bracketed Annotations (8 files)

Error markers/replacements, retraces, overlap markers, scope markers, other markers, regular groups, phonological groups, sign groups, long features.

### `tiers/` — Dependent Tiers (10 files)

%mor/%gra, %pho, %sin, %wor, descriptive (%com/%act/%gpx/%sit/%exp), coding (%cod/%spa/%int etc.), other (%add/%alt/%def etc.), user-defined (%xtra/%xcod), unsupported, multi-tier utterance.

### `ca/` — Conversation Analysis (4 files)

Overlaps, intonation contours, uptake/special, nonvocal and long features. All files use `@Options: CA`.

### `audio/` — Audio-Linked Files (5 files)

| File | Language | Audio |
|------|----------|-------|
| `english-child-speech.cha` | eng | `english-child-speech.mp3` |
| `chinese-adult-conversation.cha` | zho | `chinese-adult-conversation.mp3` |
| `russian-child-narrative.cha` | rus | `russian-child-narrative.mp3` |
| `spanish-child-speech.cha` | spa | `spanish-child-speech.mp3` |
| `french-child-speech.cha` | fra | `french-child-speech.mp3` |

### `languages/` — One Per Language (20 files)

ara, dan, deu, ell, eng, est, fra, heb, hrv, hun, isl, ita, jpn, nld, pol, por, rus, spa, tur, zho. Each file has 3–5 utterances with %mor/%gra from batchalign3 morphotag.

## Validation

```bash
just batchalign test
```

## Key Policies

- All 74 files must remain valid Batchalign fixture inputs.
- Every file has `@Comment:` headers explaining its purpose and constructs.
- Language files have fresh %mor/%gra from batchalign3 morphotag.
- Canonical parser and grammar validation lives in Chatter.

---
Last Updated: 2026-03-01

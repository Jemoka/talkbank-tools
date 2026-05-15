# CHAT Error Reference

Complete reference for all CHAT parser and validation errors.

Status legend: ✅ = active in the validator, ⏳ = documented but not yet enforced.

## internal (E0x)

Internal invariant failure. This error indicates a bug in the parseritself, not in the CHAT input. It cannot be triggered by any CHAT file.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E001](E001.md) | E001: InternalError | error | ✅ |

## internal (E0x)

Test-only sentinel error code. Used exclusively in the test suite toverify error handling plumbing. Never emitted in production.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E002](E002.md) | E002: TestError | error | ✅ |

## validation (E0x)

The input string is empty. E003 (EmptyString) is the default error code forempty fields during model validation, but an empty filedoes not trigger E003 end-to-end. Instead, the parser produces headervalidation errors (missing @UTF8, @End, @Participants, etc.) and E316(unparsable content) because there are no headers to find.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E003](E003.md) | E003: Empty string input | error | ⏳ |

## validation (E1x)

A line in the CHAT file does not match any valid line format (must start with, , , or be a continuation tab). E101 (InvalidLineFormat) is definedas an error code but is not currently emitted by the tree-sitter parser. Theparser produces header validation errors for the missing scaffolding and doesnot reach E101 detection.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E101](E101.md) | E101: Invalid line format | error | ⏳ |

## Parser error (E2x)

Missing form type after @

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E202](E202.md) | E202: Missing form type after @ | error | ✅ |

## Word validation (E2x)

A word contains at a position where a form type marker is expected, butno valid form type follows. Tree-sitter produces an ERROR node at the .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E202](E202.md) | E202: Missing form type after @ | error | ✅ |

## validation (E2x)

Word contains an invalid or undeclared form type marker (e.g., has multiple stacked markers).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E203](E203.md) | E203: Invalid form type marker | error | ✅ |

## Word validation (E2x)

Unknown scoped annotation marker

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E207](E207.md) | E207: Unknown scoped annotation marker | error | ✅ |

## validation (E2x)

Empty replacement

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E208](E208.md) | E208: Empty replacement | error | ✅ |

## validation (E2x)

A word on the main tier consists entirely of shortening notation withno actual spoken material. In CHAT, means the sounds were omitted — itis not the same as the word being spoken. To mark an omitted word, use (zero-word) instead.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E209](E209.md) | E209 — Word has no spoken content | error | ✅ |

## Word validation (E2x)

Deprecated. This error code was replaced by E387 ().The validation logic now emits E387 instead of E210 for the same condition.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E210](E210.md) | E210: Deprecated — replaced by E387 | error | ? |

## Word validation (E2x)

A word on the main tier has an invalid format that does not match any recognizedCHAT word structure. The validator reports E212 for specific structuralviolations such as CA omissions used outside CA mode, CA omissions withoutspoken text, or standalone shortenings.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E212](E212.md) | E212: Invalid word format | error | ⏳ |

## Word validation (E2x)

Deprecated. This error code was replaced by E391(). The validation logic now emits E391instead of E213 for the same condition.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E213](E213.md) | E213: Deprecated — replaced by E391 | error | ? |

## validation (E2x)

A scoped annotation (e.g., error annotation , replacement ) hasan empty content list. The validator reports E214 when annotated content haszero scoped annotations attached.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E214](E214.md) | E214: Empty scoped annotation content | error | ⏳ |

## Word validation (E2x)

A word on the main tier contains numeric digits in a language context that doesnot permit them. Most natural languages (English, Spanish, French, etc.) do notallow bare digits in words on the main tier. A small set of languages (Chinese,Welsh, Vietnamese, Thai, Cantonese, etc.) permit digits as part of tonenotation or numerals.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E220](E220.md) | E220 — Illegal digits in word content | error | ✅ |

## validation (E2x)

Compound delimiter () is not properly balanced — opening delimiter has no matching closing delimiter.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E230](E230.md) | E230: Unbalanced CA delimiter | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E231](E231.md) | generated from corpus | error | ✅ |

## validation (E2x)

Compound marker () cannot be at the start of a word. Valid compounds have the form .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E232](E232.md) | E232: Compound marker at word start | error | ✅ |

## validation (E2x)

Compound markers () must connect two non-empty parts. Adjacent compound markers() create an empty part between them, which is invalid.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E233](E233.md) | E233: Empty compound part | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E241](E241.md) | generated from corpus | error | ✅ |

## word_validation (E2x)

The marker 'xx' is used for untranscribed speech, but this is not allowed in CHAT. The correct marker for untranscribed speech is 'xxx' (three x's).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E241](E241.md) | E241: Illegal Untranscribed Marker 'xx' | error | ✅ |

## validation (E2x)

Quotation marks must be balanced within an utterance.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E242](E242.md) | E242: Unbalanced quotation marks | error | ✅ |

## validation (E2x)

Word contains illegal characters such as whitespace, control characters, or bullet markers that are not valid in word content.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E243](E243.md) | E243: Illegal characters in word | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E244](E244.md) | generated from corpus | error | ✅ |

## validation (E2x)

A primary stress marker () or secondary stress marker appears at the startof a word but is not followed by any spoken material. The marker has nothingto attach to.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E245](E245.md) | E245 — Stress marker without following spoken material | error | ✅ |

## validation (E2x)

A lengthening marker () appears before any spoken material in a word ratherthan after it. In CHAT, the colon indicates phonological lengthening andmust follow the spoken text it modifies (e.g., is valid, isnot).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E246](E246.md) | E246: Lengthening marker not after spoken material | error | ⏳ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E247](E247.md) | generated from corpus | error | ✅ |

## validation (E2x)

The bare shortcut toggles between the first two languages declared in. When an utterance is scoped to a tertiary language (position3 or later in the list) via , bare is ambiguous —it could mean either the primary or secondary language. The speaker must use anexplicit code (, , etc.) instead.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E248](E248.md) | E248 — Bare  shortcut in tertiary language context | error | ✅ |

## validation (E2x)

The shortcut means "the other language" — it toggles between the primaryand secondary language declared in . When there is no secondarylanguage (the header lists only one language), has notarget to resolve to. The speaker must use an explicit language code(, , etc.) or add a second language to the header.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E249](E249.md) | E249 — Bare  shortcut with no secondary language | error | ✅ |

## validation (E2x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E250](E250.md) | generated from corpus | error | ✅ |

## validation (E2x)

A word content text segment (the spoken text portion of a word or the textinside a shortening) is empty. The validator reports E251 when a or element validates to empty via its inner wrapper.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E251](E251.md) | E251: Empty word content text | error | ⏳ |

## Prosodic marker placement (E2x)

Syntax error - caret at word start

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E252](E252.md) | caret at word start | error | ⏳ |

## validation (E2x)

A parsed Word object has empty content — the word node exists in the CST but contains no text.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E253](E253.md) | E253: Empty word content | error | ✅ |

## validation (E2x)

Consecutive commas

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E258](E258.md) | E258: Consecutive commas | error | ✅ |

## validation (E2x)

Comma without any preceding spoken word in the utterance

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E259](E259.md) | spoken content | error | ✅ |

## Main tier validation (E3x)

Empty speaker code

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E301](E301.md) | E301: Empty speaker code | error | ✅ |

## validation (E3x)

Expected tree-sitter node is missing. E302 (MissingNode) fires whentree-sitter's error recovery inserts a MISSING placeholder node, indicatingthe grammar expected a specific construct that was not found. This is aninternal parser condition triggered by tree-sitter error recovery, not byspecific CHAT syntax patterns. It also fires in speaker code validation forinvalid characters.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E302](E302.md) | E302: Missing required node | error | ⏳ |

## Parser bugs (experimental) (E3x)

Unexpected node - helper function

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E303](E303.md) | helper function | error | ⏳ |

## Main tier validation (E3x)

Expected terminator not found

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E304](E304.md) | E304: Expected terminator not found | error | ✅ |

## Main tier validation (E3x)

Expected main tier content

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E305](E305.md) | E305: Expected main tier content | error | ✅ |

## Main tier validation (E3x)

Utterance has no content

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E306](E306.md) | E306: Utterance has no content | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E307](E307.md) | generated from corpus | error | ✅ |

## Main tier validation (E3x)

Invalid speaker format

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E308](E308.md) | E308: Invalid speaker format | error | ✅ |

## validation (E3x)

Unexpected syntax encountered during parsing. E309 (UnexpectedSyntax) fireswhen the parser encounters an ERROR node from tree-sitter that containsunexpected content. The error is emitted from in.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E309](E309.md) | E309: Unexpected syntax | error | ⏳ |

## Main tier validation (E3x)

Tree-sitter's internal parser returned (e.g., due to timeout orcancellation) or the parse outcome was rejected with no other errors collected.E310 is a catch-all for complete parse failures where no more specific errorcode applies.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E310](E310.md) | E310: Parser failed to produce valid parse tree | error | ⏳ |

## Main tier validation (E3x)

Failed to parse utterance

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E311](E311.md) | E311: Failed to parse utterance | error | ⏳ |

## validation (E3x)

Opening bracket on the main tier has no matching closing bracket .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E312](E312.md) | E312: Unclosed bracket | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E313](E313.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E314](E314.md) | generated from corpus | error | ✅ |

## validation (E3x)

Main tier or dependent tier contains an invalid control character (e.g., embedded NUL, SOH, or other non-printable ASCII).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E315](E315.md) | E315: Invalid control character | error | ✅ |

## Dependent tier validation (E3x)

A tier entry contains an angle-bracketed prefix inside the stemposition (e.g., , ). The CHATmanual's %mor grammar uses these separators inside the stem: (feature), (fusion), (prefix), (category), (clitic), (compound). Angle brackets are not valid stemcontent. The parser produces an ERROR node at the and thevalidator reports E316 on the surrounding region.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E316](E316.md) | bracketed annotation inside %mor stem is invalid | error | ✅ |

## Main tier validation (E3x)

Unparsable content

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E316](E316.md) | E316: Unparsable content | error | ✅ |

## parser_recovery (E3x)

A line could not be classified as a header, utterance, or dependent tier.This is a fallback error emitted when tree-sitter produces an ERROR nodefor a line whose children cannot be identified as either a header orutterance context.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E319](E319.md) | E319: UnparsableLine | error | ⏳ |

## parser_recovery (E3x)

A header line (starting with @) could not be parsed. This is a fallbackerror emitted when tree-sitter produces an ERROR node in header context,but the header type is not one of the specifically handled types(@Participants, @Languages, @Date, @Media, @ID).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E320](E320.md) | E320: UnparsableHeader | error | ⏳ |

## parser_recovery (E3x)

An utterance line (starting with *SPEAKER:) could not be parsed. Theutterance body contains syntax errors that tree-sitter cannot recoverfrom, and the error doesn't match any of the specifically checkedpatterns (missing form type, empty replacement, unknown annotation).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E321](E321.md) | E321: UnparsableUtterance | error | ⏳ |

## parser_recovery (E3x)

The main tier speaker prefix has a zero-width (MISSING) colon node.This occurs when tree-sitter synthesizes an empty colon placeholderbecause the speaker code has no colon at all.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E322](E322.md) | E322: EmptyColon | error | ⏳ |

## validation (E3x)

Missing colon after speaker code on main tier. E323 (MissingColonAfterSpeaker)fires in when the tree-sitter grammar parses a main tier but thecolon child node is missing. However, when the colon is absent, the grammartypically fails to match the main tier pattern at all, producing an ERROR node(E316 UnparsableContent) rather than a partial main tier with a missing colon.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E323](E323.md) | E323: Missing colon after speaker code | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E324](E324.md) | generated from corpus | error | ✅ |

## parser_recovery (E3x)

An unexpected child node was found inside a parsed utterance. The CSTcontains a node that is neither the main tier nor a recognized dependenttier kind. This typically indicates a tree-sitter error recovery scenariowhere an unusual node type ends up inside an utterance subtree.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E325](E325.md) | E325: UnexpectedUtteranceChild | error | ⏳ |

## parser_recovery (E3x)

A line was classified as an unexpected type during file structure parsing.This covers two sub-cases:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E326](E326.md) | E326: UnexpectedLineType | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E330](E330.md) | generated from corpus | error | ✅ |

## parser_recovery (E3x)

A tree-sitter node appeared in a syntactic context where it is not expected. The nodetype itself is valid CHAT syntax, but it occurs at a position in the AST that violatesthe grammar. This error is emitted during tree-sitter error recovery — the parserattempts to continue after encountering invalid syntax, and the recovered structurecontains nodes in unexpected positions.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E331](E331.md) | E331: UnexpectedNodeInContext | error | ⏳ |

## parser_recovery (E3x)

Main tier content could not be classified as any known word or constructtype. This fires when a CST node has a child kindthat the Rust parser doesn't recognize — indicating a grammar/parsermismatch (the grammar produces a new node type that the parser hasn'tbeen updated to handle).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E340](E340.md) | E340: UnknownBaseContent | error | ✅ |

## cross_utterance (E3x)

A quotation-follows terminator () is used but the next utterancefrom the same speaker does not begin with a quotation precedes linker(). This indicates an unbalanced cross-utterance quotation sequence.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E341](E341.md) | E341: UnbalancedQuotationCrossUtterance | error | ⏳ |

## Word validation (E3x)

Missing required element

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E342](E342.md) | E342: Missing required element | error | ⏳ |

## validation (E3x)

Invalid nesting of scoped annotations (quotation precedes pattern). This is a cross-utterance validator () that is currently DISABLED ().

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E344](E344.md) | E344: Invalid scoped annotation nesting | error | ⏳ |

## validation (E3x)

Unmatched scoped annotation end marker ( without matching ). This is a cross-utterance validator () that is currently DISABLED ().

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E346](E346.md) | E346: Unmatched scoped annotation end | error | ⏳ |

## validation (E3x)

An indexed top overlap region (e.g., ) on one speaker has nomatching indexed bottom overlap region () from a different speaker,or vice versa. Reported as a warning because some onset-only markingconventions exist.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E347](E347.md) | speaker overlap (indexed markers) | error | ✅ |

## validation (E3x)

Reserved for within-utterance overlap pairing violations: a closing marker( or ) without a preceding opening marker ( or ) in the sameutterance, or vice versa.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E348](E348.md) | E348 — Unpaired overlap marker within utterance | error | ⏳ |

## cross_utterance (E3x)

A self-completion linker () was used but there is no prior utterancefrom the same speaker. The linker requires a preceding interruptedutterance from the same speaker to complete.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E351](E351.md) | E351: MissingQuoteBegin | error | ⏳ |

## cross_utterance (E3x)

A self-completion linker () was used and there IS a prior utterancefrom the same speaker, but that prior utterance did not end with a (interruption) terminator.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E352](E352.md) | E352: MissingQuoteEnd | error | ⏳ |

## cross_utterance (E3x)

An other-completion linker () was used but it is the very firstutterance in the file. The linker requires a preceding utterance(from a different speaker) to complete.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E353](E353.md) | E353: MissingOtherCompletionContext | error | ⏳ |

## cross_utterance (E3x)

An other-completion linker () was used and the preceding utterance isfrom a different speaker, but that preceding utterance did not end with (trailing off). The other-completion convention requires theprevious speaker to have trailed off.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E354](E354.md) | E354: MissingTrailingOffTerminator | error | ⏳ |

## cross_utterance (E3x)

An other-completion linker () was used but the preceding utterance isfrom the same speaker. The linker is for other-completion(completing a different speaker's utterance). To complete one's ownutterance, use (self-completion) instead.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E355](E355.md) | E355: InterleavedScopedAnnotations | error | ⏳ |

## underline_balance (E3x)

An underline begin marker was found without a matching underline end markerin the same utterance. Underline markers (used in CA transcription to markstressed syllables) must occur in matched begin/end pairs within a singleutterance.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E356](E356.md) | E356: UnmatchedUnderlineBegin | error | ✅ |

## underline_balance (E3x)

An underline end marker was found without a preceding underline beginmarker in the same utterance. The end marker has no open underline toclose.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E357](E357.md) | E357: UnmatchedUnderlineEnd | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E358](E358.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E359](E359.md) | generated from corpus | error | ✅ |

## validation (E3x)

Media bullet (timestamp marker) contains malformed content — e.g., non-numeric characters, missing underscore separator, or structurally invalid timestamp format.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E360](E360.md) | E360: Invalid media bullet | error | ⏳ |

## Main tier validation (E3x)

The media bullet contains a deprecated skip flag (dash before closing NAK delimiter). The skip flag is deprecated. Only a small number of occurrences exist across the corpus.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E360](E360.md) | E360: Deprecated Skip Bullet | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E361](E361.md) | generated from corpus | error | ✅ |

## validation (E3x)

Bullet timestamps must be monotonic

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E362](E362.md) | E362: Bullet timestamps must be monotonic | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E363](E363.md) | generated from corpus | error | ✅ |

## validation (E3x)

Word content is structurally malformed — the parser recognized a word node but its internal structure is invalid (e.g., with instead of a language code).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E364](E364.md) | E364: Malformed word content | error | ⏳ |

## validation (E3x)

A header or tier has content that does not match any recognized CHAT headerstructure. The parser reports E365 when it encounters an unknown node typeduring header dispatch in the CST.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E365](E365.md) | E365: Malformed tier content | error | ⏳ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E367](E367.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E368](E368.md) | generated from corpus | error | ✅ |

## Alignment count mismatch (E3x)

A structural ordering violation in the utterance content, such as groups orreplacements that do not align correctly with dependent tier items.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E370](E370.md) | E370 — Structural order error | error | ⏳ |

## validation (E3x)

Pause inside phonological group

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E371](E371.md) | E371: Pause inside phonological group | error | ✅ |

## validation (E3x)

Nested quotation

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E372](E372.md) | E372: Nested quotation | error | ✅ |

## overlap (E3x)

An overlap marker has an index value outside the valid range. For CAoverlap brackets (), the index must be 2–9. For scoped overlapannotations (, ), the index must be 1–9.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E373](E373.md) | E373: InvalidOverlapIndex | error | ✅ |

## Parser bugs (experimental) (E3x)

Scoped annotation parse error

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E375](E375.md) | E375: Scoped annotation parse error | error | ✅ |

## Word validation (E3x)

Failed to parse replacement annotation content. The replacementannotation contains content that cannot be parsed as valid replacementwords.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E376](E376.md) | E376: Replacement parse error | error | ✅ |

## Dependent tier parsing (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E382](E382.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E387](E387.md) | generated from corpus | error | ✅ |

## validation (E3x)

Replacement annotation is attached to a non-word element (e.g., a paralinguistic event like ), which cannot be replaced.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E388](E388.md) | word | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E389](E389.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E390](E390.md) | generated from corpus | error | ✅ |

## validation (E3x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E391](E391.md) | generated from corpus | error | ✅ |

## validation (E4x)

Duplicate dependent tiers

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E401](E401.md) | E401: Duplicate dependent tiers | error | ✅ |

## validation (E4x)

A dependent tier (, , etc.) appears before any main tier in thefile. E404 (OrphanedDependentTier) is emitted by inwhen a -prefixed ERROR node appears before any utterance.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E404](E404.md) | E404: Orphaned dependent tier | error | ✅ |

## Header validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E501](E501.md) | generated from corpus | error | ✅ |

## validation (E5x)

Every valid CHAT file must end with an header. This error indicates the file is missing , usually because the file is truncated, empty, or was saved incompletely.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E502](E502.md) | E502: Missing required @End header | error | ✅ |

## parser (E5x)

When a tier contains invalid content (e.g., an action marker like )AND the %wor line has 7+ words after the error, tree-sitter's error recovery failscatastrophically: instead of isolating the ERROR to the %wor tier, the entire filebecomes one ERROR node. This causes:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E502 (false positive)](E502 (false positive).md) | E502 false positive: %wor parse error cascades to entire file | error | ✅ |

## Header validation (E5x)

Every valid CHAT file must begin with an header as its first line. This error indicates the file is missing , which means the file's character encoding is unspecified. All modern CHAT files are expected to be UTF-8 encoded.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E503](E503.md) | E503: Missing required @UTF8 header | error | ✅ |

## Header validation (E5x)

Missing required header

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E504](E504.md) | E504: Missing required header | error | ✅ |

## Header validation (E5x)

Invalid @ID format

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E505](E505.md) | E505: Invalid @ID format | error | ✅ |

## Header validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E506](E506.md) | generated from corpus | error | ✅ |

## Header validation (E5x)

@Languages header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E507](E507.md) | E507: @Languages header cannot be empty | error | ✅ |

## Header validation (E5x)

@Date header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E508](E508.md) | E508: @Date header cannot be empty | error | ✅ |

## Header validation (E5x)

@Media header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E509](E509.md) | E509: @Media header cannot be empty | error | ✅ |

## Header validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E510](E510.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E511](E511.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E512](E512.md) | generated from corpus | error | ✅ |

## Header validation (E5x)

Participant entry should have both code and role

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E513](E513.md) | E513: Participant entry should have both code and role | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E515](E515.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E516](E516.md) | generated from corpus | error | ✅ |

## header_validation (E5x)

The header's fourth field () must conform to one of thethree legal CHAT date patterns defined by CLAN's authoritative:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E517](E517.md) | E517:  age field does not match a legal CHAT date pattern | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E518](E518.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E519](E519.md) | generated from corpus | error | ✅ |

## Header validation (E5x)

@Participants header cannot be empty

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E522](E522.md) | E522: @Participants header cannot be empty | error | ✅ |

## header_validation (E5x)

An utterance uses a speaker code that was not defined in the @Participants header. All speaker codes used in utterances must be declared in the @Participants header.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E522](E522.md) | E522: Undefined Participant in Utterance | error | ✅ |

## Participant validation (E5x)

Orphan @ID header

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E523](E523.md) | E523: Orphan @ID header | error | ✅ |

## Participant validation (E5x)

@Birth header for unknown participant

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E524](E524.md) | E524: @Birth header for unknown participant | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E525](E525.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E526](E526.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E527](E527.md) | generated from corpus | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E528](E528.md) | generated from corpus | error | ✅ |

## validation (E5x)

Nested background with identical label

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E529](E529.md) | E529: Nested background with identical label | error | ✅ |

## validation (E5x)

Lazy gem inside background

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E530](E530.md) | E530: Lazy gem inside background | error | ✅ |

## validation (E5x)

The filename in the header does not match the name of the CHAT filebeing parsed (case-insensitive comparison). For example, if contains, E531 is reported because does not match .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E531](E531.md) | E531: Media filename mismatch | error | ⏳ |

## validation (E5x)

Invalid participant role

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E532](E532.md) | E532: Invalid participant role | error | ✅ |

## validation (E5x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E533](E533.md) | generated from corpus | error | ✅ |

## header_validation (E5x)

An header contains a flag that is not one of the recognized option values. The file parses successfully but the unsupported flag is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E534](E534.md) | E534: Unsupported @Options Value | error | ✅ |

## header_validation (E5x)

An header contains a media type that is not one of the recognized values. The file parses successfully but the unsupported type is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E535](E535.md) | E535: Unsupported @Media Type | error | ✅ |

## header_validation (E5x)

An header contains a status value that is not one of the recognized values. The file parses successfully but the unsupported status is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E536](E536.md) | E536: Unsupported @Media Status | error | ✅ |

## header_validation (E5x)

An header contains a value that is not one of the recognized number options. The file parses successfully but the unsupported value is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E537](E537.md) | E537: Unsupported @Number Value | error | ✅ |

## header_validation (E5x)

An header contains a value that is not one of the recognized quality ratings. The file parses successfully but the unsupported value is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E538](E538.md) | E538: Unsupported @Recording Quality Value | error | ✅ |

## header_validation (E5x)

An header contains a value that is not one of the recognized transcription types. The file parses successfully but the unsupported value is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E539](E539.md) | E539: Unsupported @Transcription Value | error | ✅ |

## header_validation (E5x)

An header must match one of the three time patternsthat CLAN's authoritative declares legal:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E540](E540.md) | E540:  does not match a legal CLAN time pattern | error | ✅ |

## header_validation (E5x)

An header must match one of the two time patterns thatCLAN's authoritative declares legal:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E541](E541.md) | E541:  does not match a legal CLAN time pattern | error | ✅ |

## header_validation (E5x)

An header contains a sex field value that is not one of the recognized values. The file parses successfully but the unsupported value is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E542](E542.md) | E542: Unsupported @ID Sex Value | error | ✅ |

## header_validation (E5x)

A header appears out of canonical order. For example, or appears before . CHAT headers must follow the canonical ordering: , , , , then other headers like and .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E543](E543.md) | E543: Header out of canonical order | error | ✅ |

## header_validation (E5x)

An header declares a linked media file (no / / status), but the transcript body contains noevidence that any utterance is actually linked to that media. Bythe CHAT manual's semantics, an unqualified declaration isa promise that the transcript is time-linked to the named file;this check catches transcripts that make that promise withoutkeeping it.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E544](E544.md) | E544:  claims linkage but transcript has no timing evidence | error | ✅ |

## header_validation (E5x)

An header must carry a date matching CLAN'sauthoritative date template:

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E545](E545.md) | E545:  date does not match a legal CHAT date pattern | error | ✅ |

## header_validation (E5x)

An header contains an SES (socioeconomic status) field value that is not one of the recognized values. The file parses successfully but the unsupported value is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E546](E546.md) | E546: Unsupported @ID SES Value | error | ✅ |

## validation (E6x)

A dependent tier (typically ) had parse errors during lenient recovery, so thevalidator cannot verify alignment between tiers. Alignment checks (main↔%mor, %mor↔%gra)are skipped for the affected utterance. This is a warning, not an error — the filestill parses, but alignment correctness is unverified for tainted tiers.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E600](E600.md) | E600: Tier alignment skipped due to parse errors | error | ✅ |

## validation (E6x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E601](E601.md) | generated from corpus | error | ✅ |

## validation (E6x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E602](E602.md) | generated from corpus | error | ✅ |

## tier_validation (E6x)

A dependent tier contains content that does not match the expected time format. The tier parses successfully but the invalid content is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E603](E603.md) | E603: Invalid %tim Tier Format | error | ✅ |

## Dependent tier parsing (E6x)

Empty GRA relation

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E604](E604.md) | E604: Empty GRA relation | error | ✅ |

## tier_validation (E6x)

A %gra (grammatical relations) tier appears without a corresponding %mor (morphology) tier. According to CHAT rules, %gra depends on %mor and cannot exist independently.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E604](E604.md) | E604: %gra Tier Without %mor Tier | error | ✅ |

## tier_validation (E6x)

An utterance contains a dependent tier with a label that is not a standard CHAT tier name and does not follow the user-defined tier naming convention. The file parses successfully but the tier is stored as and flagged during validation.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E605](E605.md) | E605: Unsupported Dependent Tier | error | ✅ |

## Temporal validation (E7x)

Each utterance's first media bullet must have a start time greater than orequal to the previous utterance's first bullet start time (for the samespeaker). Corresponds to CLAN CHECK Error 83.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E701](E701.md) | speaker start | error | ✅ |

## Dependent tier parsing (E7x)

Invalid MOR chunk format - missing |

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E702](E702.md) | missing | | error | ⏳ |

## validation (E7x)

A single speaker has consecutive utterances with overlap markers (/) that overlap with each other. Overlap markers should indicate simultaneous speech between different speakers, not self-overlap.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E704](E704.md) | overlap — overlapping overlap markers | error | ✅ |

## Alignment count mismatch (E7x)

Mor count mismatch - too few items

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E705](E705.md) | too few items | error | ✅ |

## Alignment count mismatch (E7x)

Mor count mismatch - too many mor items

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E706](E706.md) | too many mor items | error | ✅ |

## Alignment terminator mismatch (E7x)

The %mor tier has a terminator but the main tier does not, or vice versa.One tier ends with a sentence-final punctuation mark while the other does not.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E707](E707.md) | E707: Mor terminator presence mismatch | error | ✅ |

## Dependent tier parsing (E7x)

A grammar relation on the tier is malformed — missing an index, head,or relation label, or containing non-integer values where integers are expected.The tier format is for each word.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E708](E708.md) | E708: Malformed grammar relation on %gra tier | error | ⏳ |

## validation (E7x)

A relation uses an invalid index. indices are 1-indexed: thefirst word is , and is reserved for the ROOT attachment in thedependent slot (). Using in the first (index) slot of arelation triggers E709.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E709](E709.md) | E709: Invalid grammar index | error | ✅ |

## Dependent tier parsing (E7x)

Invalid GRA format

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E710](E710.md) | E710: Invalid GRA format | error | ✅ |

## Mor content validation (E7x)

A %mor word has an empty stem, POS category, prefix, or suffix. Everymorphosyntax item on the %mor tier must have a non-empty POS categoryand a non-empty stem at minimum.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E711](E711.md) | E711: Mor empty content | error | ⏳ |

## validation (E7x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E712](E712.md) | generated from corpus | error | ✅ |

## validation (E7x)

A relation has a head index that falls outside the valid range, where is the number of chunks in the utterance. Index is reserved for the ROOT head; otherwise the head index must point toan existing chunk.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E713](E713.md) | E713: Gra head index invalid | error | ✅ |

## Alignment count mismatch (E7x)

The (actual phonology) tier has fewer alignable tokens than the main tier.Each main-tier word must have a corresponding token.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E714](E714.md) | E714:  alignment count mismatch — too few tokens | error | ✅ |

## Alignment count mismatch (E7x)

The (actual phonology) tier has more alignable tokens than the main tier.Remove the extra tokens so counts match.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E715](E715.md) | E715:  alignment count mismatch — too many tokens | error | ✅ |

## Alignment terminator mismatch (E7x)

The %mor tier has a terminator that does not match the main tier's terminator.Both tiers have terminators, but they differ (e.g., main tier ends with "?"but %mor ends with "."). This typically indicates stale or incorrectly cachedmorphosyntax data.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E716](E716.md) | E716: Mor terminator value mismatch | error | ✅ |

## Alignment count mismatch (E7x)

Sin count mismatch - too few sin tokens

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E718](E718.md) | too few sin tokens | error | ✅ |

## Alignment count mismatch (E7x)

Sin count mismatch - too many sin tokens

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E719](E719.md) | too many sin tokens | error | ✅ |

## Alignment count mismatch (E7x)

The number of chunks does not equal the number of relationsfor an utterance. aligns 1-to-1 with chunks (not items — a item with post-clitics produces multiple chunks).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E720](E720.md) | Gra count mismatch | error | ✅ |

## validation (E7x)

tier indices must be sequential (1, 2, 3, ..., N). Non-sequential indices indicate a malformed dependency structure.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E721](E721.md) | sequential index | error | ✅ |

## validation (E7x)

tier has no ROOT relation. Every tier must have exactly one relation with or (the ROOT of the dependency tree).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E722](E722.md) | E722: GRA has no ROOT | error | ✅ |

## validation (E7x)

tier has multiple ROOT relations. Every tier should have exactly one ROOT (relation with or ).

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E723](E723.md) | E723: GRA has multiple ROOTs | error | ✅ |

## validation (E7x)

A %gra tier contains a circular dependency where following parent pointers creates a cycle. This violates the fundamental requirement that dependency structures must form a tree.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E724](E724.md) | E724: GRA has circular dependency | error | ✅ |

## Alignment count mismatch (E7x)

The tier word count does not match the tier word count. Each word-level entry in must correspond one-to-one with a word-level entry in .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E725](E725.md) | E725: Modsyl tier word count does not match mod tier | error | ✅ |

## Alignment count mismatch (E7x)

The tier word count does not match the tier word count. Each word-level entry in must correspond one-to-one with a word-level entry in .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E726](E726.md) | E726: Phosyl tier word count does not match pho tier | error | ✅ |

## Alignment count mismatch (E7x)

The tier word count does not match the tier word count. Each word-level entry in must correspond one-to-one with a word-level entry in .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E727](E727.md) | E727: Phoaln tier word count does not match mod tier | error | ✅ |

## Alignment count mismatch (E7x)

The tier word count does not match the tier word count. Each word-level entry in must correspond one-to-one with a word-level entry in .

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E728](E728.md) | E728: Phoaln tier word count does not match pho tier | error | ✅ |

## Alignment count mismatch (E7x)

The (model/target phonology) tier has fewer alignable tokens than themain tier. Each main-tier word must have a corresponding token.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E733](E733.md) | E733:  alignment count mismatch — too few tokens | error | ✅ |

## Alignment count mismatch (E7x)

The (model/target phonology) tier has more alignable tokens than themain tier. Remove the extra tokens so counts match.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E734](E734.md) | E734:  alignment count mismatch — too many tokens | error | ✅ |

## Alignment count mismatch (E9x)

Unknown error

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [E999](E999.md) | E999: Unknown error | error | ✅ |

## validation (W1x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [W108](W108.md) | generated from corpus | error | ✅ |

## Warnings (W6x)

Auto-generated from corpus

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [W601](W601.md) | generated from corpus | error | ✅ |

## Warnings (W6x)

A user-defined dependent tier () uses a label that matches a knownstandard tier name. For example, should be updated to since is now a recognized standard tier. This is a warning to encouragemigration from legacy experimental naming to the current standard.

| Code | Name | Severity | Status |
|------|------|----------|--------|
| [W602](W602.md) | W602: Deprecated experimental tier name | error | ⏳ |


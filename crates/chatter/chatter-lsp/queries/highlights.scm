; tree-sitter highlight queries for CHAT format
; See: https://tree-sitter.github.io/tree-sitter/syntax-highlighting

; ===== Headers =====

; Special headers (keywords)
(utf8_header) @keyword
(begin_header) @keyword
(end_header) @keyword

; Header names as directives
(participants_header) @keyword.directive
(id_header) @keyword.directive
(languages_header) @keyword.directive
(options_header) @keyword.directive
(media_header) @keyword.directive
(date_header) @keyword.directive
(time_duration_header) @keyword.directive
(tape_location_header) @keyword.directive
(pid_header) @keyword.directive
(g_header) @keyword.directive

; Generic header fallback
(header) @keyword.directive

; ===== Speakers =====

(speaker) @variable.builtin

; ===== Main Tier Content =====

; Words
(word_with_optional_annotations) @string

; Pauses
(pause_token) @comment.block

; Events (&=laughs, &=coughs)
(event) @tag

; Other spoken events (&*CHI:word)
(other_spoken_event) @tag

; Retracing ([/], [//], [///], [/-], [/?])
(retrace_complete) @keyword.control
(retrace_partial) @keyword.control
(retrace_multiple) @keyword.control
(retrace_reformulation) @keyword.control

; Freecodes ([^ text])
(freecode) @constant

; Quotations ("...")
(quotation) @string.delimiter

; Groups with annotations (<content> [annotation])
(group_with_annotations) @string

; CA elements and delimiters: coarsened into standalone_word token (Phase 2)
; Highlighting handled at word level via (word_with_optional_annotations)

; ===== Overlap Markers =====

; Overlap points: ⌈ ⌉ ⌊ ⌋ (with optional index digit)
(overlap_point) @punctuation.bracket

; Overlap annotations: [<], [>], [<2], [>2]
(indexed_overlap_precedes) @punctuation.bracket
(indexed_overlap_follows) @punctuation.bracket

; ===== Linkers =====

; Utterance linkers: ++, +<, +^, +", +,, +≈, +≋
(linker) @keyword.control

; ===== Annotations =====

; Scoped annotations ([!], [!!], [?])
(scoped_stressing) @operator
(scoped_contrastive_stressing) @operator
(scoped_uncertain) @operator

; Replacement text ([: word])
(replacement) @string.special

; Error annotations ([*], [* code])
(error_marker_annotation) @error

; Explanation ([= text])
(explanation_annotation) @comment

; Paralinguistic ([=! text])
(para_annotation) @comment

; Alternative transcription ([=? text])
(alt_annotation) @string.special

; Language codes
(language_code) @constant.language

; Postcodes ([+ code])
(postcode) @attribute

; Media timing bullets (unified rule — was inline_bullet + media_url before grammar consolidation)
(bullet) @number

; ===== Nonwords & Special Items =====

; Nonwords with annotations (events and zero actions with optional scope)
(nonword_with_optional_annotations) @tag

; Long feature spans (&{l=label ... &}l=label)
(long_feature) @attribute

; Nonvocal spans (&{n=label ... &}n=label)
(nonvocal) @attribute

; shortening removed — coarsened into standalone_word token (Phase 2)

; ===== Dependent Tiers =====

; Morphology tiers
(mor_dependent_tier) @type
(wor_dependent_tier) @type

; Grammar tiers
(gra_dependent_tier) @type

; Phonology tiers
(pho_dependent_tier) @type
(mod_dependent_tier) @type

; Gesture/Sign tier
(sin_dependent_tier) @type

; Comment and text tiers
(com_dependent_tier) @comment
(exp_dependent_tier) @comment
(add_dependent_tier) @comment
(spa_dependent_tier) @comment
(sit_dependent_tier) @comment
(int_dependent_tier) @comment

; Action/coding tiers
(act_dependent_tier) @function
(cod_dependent_tier) @function

; Translation tiers
(gls_dependent_tier) @string.special
(eng_dependent_tier) @string.special

; Other tiers
(ort_dependent_tier) @type
(err_dependent_tier) @error
(tim_dependent_tier) @number
(alt_dependent_tier) @string.special
(gpx_dependent_tier) @tag

; Unsupported/unknown tiers and headers (warn-level highlighting)
(unsupported_dependent_tier) @comment
(unsupported_header) @keyword.directive
(unsupported_line) @comment

; ===== Morphology Elements =====

; Mor POS tags (UPOS categories: VERB, NOUN, DET, etc.)
(mor_pos) @type.builtin

; Mor words/items
(mor_word) @type

; Mor lemma
(mor_lemma) @string

; Mor features (-PROG, -PAST, etc.)
(mor_feature) @type.qualifier

; Mor post-clitics (~word)
(mor_post_clitic) @type.qualifier

; ===== Grammar Relations =====

; Gra relations (index|head|RELATION)
(gra_relation) @function.method

; ===== Terminators =====

(terminator) @punctuation.special

; ===== Special Symbols =====

; Tabs and separators (structural)
(tab) @punctuation.delimiter
(separator) @punctuation.delimiter

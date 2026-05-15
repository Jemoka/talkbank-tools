"""Italian probe cases.

Covers:

* Clitic-article elisions (``l'ami``, ``l'amico``, ``l'opera``) — the
  old ``MwtTaggedExact("l'") → SuppressMwt`` target. Strict 1-to-1.
* Preposition+clitic elisions (``dell'opera``, ``nell'anno``,
  ``sull'ora``, ``all'amore``) — strict 1-to-1. The
  ``dell_opera_in_context`` case is xfail-pinned against Defect 6
  (POS-layer verb-clitic split on ``parla``).
* ``lei`` (3sg.f pronoun) and adjacent ``le`` + ``i`` — the old
  ``le + i → lei`` merge target. Strict 1-to-1 in both the real-``lei``
  cases and the adjacent-``le i`` cases.
* Preposition+article natives (``al``, ``del``, ``nel``, ``sul``,
  ``della``) — observe only; MWT Range reassembly design.
* Noun / adjective pseudo-verb observations
  (``arancione_noun_bogus_verb``, ``piccolo_adj_bogus_verb``,
  ``gomitolo_noun_bogus_verb``, ``divano_noun_bogus_verb``) —
  corpus-audit-derived pins for Defect 6's non-verb subclass.
  Stanza spuriously analyzes these clitic-shaped nouns/adjectives
  as verb+enclitic compounds (``arancione → verb|arancio~pron|ne``
  with ``Part Past`` features), producing committed-corpus %mor
  content that is linguistically wrong. UD-level count mismatch
  xfails pin Stanza's current behavior.
* Real-corpus ``parla`` observations (``parla_3sg_storia_context``,
  ``parla_imperative_forte``, ``parla_imperative_piu_forte``) —
  pulled verbatim from ita-only files under the CHILDES Italian
  Frogs/Italian-Roma subtree. Strict UD-level 1-to-1 with xfail marks
  pinning Stanza's POS/MWT misbehavior. The xfails are
  **Stanza-behavior observations** at the UD level, not `%mor`
  injection-failure indicators — the count invariant at
  ``|%mor items| == mor_alignable_word_count`` holds, Stage 3's
  ``assemble_mors`` collapses MWT Ranges into compound `%mor`
  entries. The defect is content quality; see the per-case
  ``XfailMark.reason`` for the specifics.

* Counterexample probes (``dammela_counterexample`` etc.) —
  observation-only. Stanza produces the correct imperative+clitic
  analysis for bare compound utterances
  (``dammela → da/dare/VERB + me/me/PRON + la/la/PRON``), Stage 3
  serializes it as
  ``verb|dare-Inf-Ind-Imp-S2~pron|me-Prs-S1~pron|la-Prs-S3``, and
  any content-correction rule for Italian must preserve this shape.
"""

from __future__ import annotations

from .._probe_types import Phenomenon, ProbeCase, XfailMark


def _per_favore(
    word: str,
    phenomenon: Phenomenon = Phenomenon.CLITIC_ELISION,
) -> ProbeCase:
    """Build an observation-only probe for `per favore <word>`.

    The `per favore` mid-sentence context is the standard probe
    shape for Italian compound-imperative Defect 8/12/13
    candidates: placing the candidate surface mid-sentence after
    an adp+noun prefix triggers Stanza's mid-sentence
    mis-classification behavior that differs from its bare
    single-utterance analysis.

    ``phenomenon`` defaults to CLITIC_ELISION since that's the
    taxonomy bucket for compound-imperative+clitic surfaces. Pass
    a different value for non-imperative surfaces that happen to
    fit the `per favore <word>` frame.
    """
    return ProbeCase(
        label=f"{word}_mid_sentence",
        words=("per", "favore", word),
        phenomenon=phenomenon,
    )


CASES: tuple[ProbeCase, ...] = (
    # ── l'X clitic-article elisions ──
    ProbeCase("l_ami_alone", ("l'ami",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_amico_alone", ("l'amico",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_opera_alone", ("l'opera",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_amore_alone", ("l'amore",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_anno_alone", ("l'anno",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_ora_alone", ("l'ora",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_ultimo_alone", ("l'ultimo",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_uomo_alone", ("l'uomo",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase("l_oggetto_alone", ("l'oggetto",), Phenomenon.CLITIC_ELISION, 1),
    ProbeCase(
        "l_ami_in_context",
        ("vedo", "l'ami", "di", "Maria"),
        Phenomenon.CLITIC_ELISION,
        4,
    ),
    # ── Preposition+clitic elisions (all', dell', nell', sull') ──
    ProbeCase(
        "all_amore_in_context",
        ("amo", "all'amore", "eterno"),
        Phenomenon.CLITIC_ELISION,
        3,
    ),
    ProbeCase(
        "dell_opera_in_context",
        ("parla", "dell'opera", "nuova"),
        Phenomenon.CLITIC_ELISION,
        3,
        xfail=XfailMark(
            defect_slug="stanza-it-verb-clitic-pos-split",
            reason=(
                "UD-level pin: Stanza's Italian POS layer splits `parla` "
                "into `par + la` with lemma=`par` (fake), yielding 4 UD "
                "words for 3 CHAT words. End-to-end `%mor` injection "
                "STILL SUCCEEDS (Stage 3's assemble_mors collapses the "
                "MWT Range into one compound item) but the content is "
                "junk: `verb|par-Inf-S~pron|la-Prs-S3 ...`. Defect 6."
            ),
        ),
    ),
    ProbeCase(
        "nell_anno_in_context",
        ("nell'anno", "scorso"),
        Phenomenon.CLITIC_ELISION,
        2,
    ),
    ProbeCase(
        "sull_ora_in_context",
        ("discutono", "sull'ora", "corretta"),
        Phenomenon.CLITIC_ELISION,
        3,
    ),
    # ── lei family (old le+i → lei merge target) ──
    ProbeCase.strict_alone("lei"),
    ProbeCase("lei_in_context", ("dice", "lei"), Phenomenon.CONTROL, 2),
    ProbeCase(
        "lei_subject",
        ("lei", "mangia", "la", "pasta"),
        Phenomenon.CONTROL,
        4,
    ),
    ProbeCase(
        "lei_object",
        ("lo", "vede", "lei"),
        Phenomenon.CONTROL,
        3,
    ),
    ProbeCase(
        "lei_in_pp",
        ("con", "lei", "sempre"),
        Phenomenon.CONTROL,
        3,
    ),
    # le + i as two adjacent CHAT words — merge rule would spuriously
    # collapse them to `lei` if still in effect.
    ProbeCase(
        "le_i_separate_risk",
        ("le", "i"),
        Phenomenon.CONTROL,
        2,
    ),
    ProbeCase(
        "le_i_in_sentence",
        ("ho", "le", "i", "libri"),
        Phenomenon.CONTROL,
        4,
    ),
    # ── Native MWT controls (2026-04-23 parity audit: locked at
    #    observed counts as Stanza-drift sentinels. Bare forms stay
    #    1-to-1 under our postprocessor; in-context forms expand
    #    via Stanza MWT and rely on downstream Range reassembly. ──
    ProbeCase.strict_alone("al", Phenomenon.NATIVE_MWT),
    ProbeCase.strict_alone("del", Phenomenon.NATIVE_MWT),
    ProbeCase.strict_alone("nel", Phenomenon.NATIVE_MWT),
    ProbeCase.strict_alone("sul", Phenomenon.NATIVE_MWT),
    ProbeCase("della_alone", ("della",), Phenomenon.NATIVE_MWT, 2),
    ProbeCase(
        "al_in_context",
        ("vado", "al", "cinema"),
        Phenomenon.NATIVE_MWT,
        4,
    ),
    ProbeCase(
        "del_in_context",
        ("il", "libro", "del", "ragazzo"),
        Phenomenon.NATIVE_MWT,
        5,
    ),
    # ── Defect 6 + Defect 7 real-corpus observations ──
    # Pulled verbatim from ita-only CHAT files in CHILDES. The
    # 2026-04-22 observe-only run on Stanza 1.11.1 (see
    # stanza-limitations.md §6 "Scope evidence") characterized the
    # split pattern: sentence-initial `parla` always mis-splits (Defect
    # 6), but mid-sentence `parla` with an explicit subject stays
    # intact — and instead surfaces a separate sentence-initial-`la`
    # expansion (Defect 7). Cases are now strict 1-to-1 with xfail
    # marks so a Stanza upgrade that fixes either defect flips the
    # corresponding probe from XFAIL to XPASS.
    #
    # Source: childes-other-data/Frogs/Italian-Roma/10/10dancop.cha —
    #   "questa storia parla di un bambino" (3sg indicative). Trimmed
    #   to six words starting with the article `la`. Verb stays intact;
    #   sentence-initial `la` is the defect target.
    ProbeCase(
        "parla_3sg_storia_context",
        ("la", "storia", "parla", "di", "un", "bambino"),
        Phenomenon.CLITIC_ELISION,
        6,
        xfail=XfailMark(
            defect_slug="stanza-it-la-sentence-initial-split",
            reason=(
                "UD-level pin: Stanza expands sentence-initial `la` "
                "into `il + i` (both DET, both lemma=`il`), yielding 7 "
                "UD words for 6 CHAT words. `parla` itself is NOT "
                "mis-analyzed in this mid-sentence position — gets "
                "correct `parlare` lemma. End-to-end `%mor` injection "
                "SUCCEEDS but `la`'s %mor item is junk: "
                "`det|il-Masc-Def-Art-Sing~det|il-Masc-Def-Art-Plur` "
                "for what should be `det|la-Fem-Def-Art-Sing`. Defect 7."
            ),
        ),
    ),
    # Source: childes-other-data/Frogs/Italian-Roma/06/06danbov.cha —
    #   "parla forte" as a bare 2sg imperative without any clitic
    #   pronoun present in the input. Sentence-initial position.
    ProbeCase(
        "parla_imperative_forte",
        ("parla", "forte"),
        Phenomenon.CLITIC_ELISION,
        2,
        xfail=XfailMark(
            defect_slug="stanza-it-verb-clitic-pos-split",
            reason=(
                "UD-level pin: sentence-initial `parla` splits into "
                "`par + la` with lemma=`par` even with no clitic in "
                "the input. 2 CHAT words → 3 UD words. End-to-end "
                "`%mor` emitted as `verb|par-Inf-S~pron|la-Prs-S3 "
                "adj|forte-S1` — correct count, junk content (should "
                "be `verb|parlare-Imp-S2 adj|forte-S1`). Defect 6."
            ),
        ),
    ),
    # Source: childes-other-data/Frogs/Italian-Roma/05/05giovel.cha —
    #   "parla un po' più forte" (2sg imperative with adverbial
    #   modification). Longer context to confirm the adverb chain does
    #   not shift Stanza's analysis away from the clitic reading.
    ProbeCase(
        "parla_imperative_piu_forte",
        ("parla", "più", "forte"),
        Phenomenon.CLITIC_ELISION,
        3,
        xfail=XfailMark(
            defect_slug="stanza-it-verb-clitic-pos-split",
            reason=(
                "UD-level pin: sentence-initial `parla` splits into "
                "`par + la` despite trailing adverb chain. 3 CHAT "
                "words → 4 UD words. End-to-end `%mor` emits "
                "`verb|par-...~pron|la-... adv|più adj|forte-S1` — "
                "correct count, junk content. Reinforces Defect 6."
            ),
        ),
    ),
    # ── Defect 6 non-verb subclass: Stanza mis-analyzes nouns/adjectives ──
    # ── with clitic-shaped endings as verb+enclitic compounds ──
    # Pulled from a 2026-04-22 audit of committed %mor content via
    # scripts/analysis/audit_italian_mor_content.py. Each surface below
    # ships in the corpus today with `verb|STEM~pron|CLITIC` where
    # the stem is a non-word fragment and the features are
    # typically `Part Past`. Strict UD-level 1-to-1 with xfail so
    # a Stanza upgrade that fixes the POS-layer misanalysis flips
    # the pin to XPASS.
    #
    # Sources: corpus hits in
    #   childes-romance-germanic-data/Romance/Italian/Burgato/23
    #     (arancione), Tonelli/Marco/011026 (gomitolo),
    #     Tonelli/Marco/010803 (divano),
    #     Calambrone/Martina/020322 (piccolo)
    ProbeCase(
        "arancione_noun_bogus_verb",
        ("arancione",),
        Phenomenon.CLITIC_ELISION,
        1,
        xfail=XfailMark(
            defect_slug="stanza-it-verb-clitic-pos-split",
            reason=(
                "UD-level pin: `arancione` (noun, \"orange\") is "
                "spuriously split by Stanza's POS layer into "
                "`arancio + ne` (lemma=`arancio`, clitic=`ne`), "
                "tagged verb+pron with Part Past features. The "
                "committed %mor in the Italian corpus ships as "
                "`verb|arancio~pron|ne`. Should be `noun|arancione` "
                "or `adj|arancione`. Defect 6 non-verb subclass."
            ),
        ),
    ),
    ProbeCase(
        "piccolo_adj_bogus_verb",
        ("piccolo",),
        Phenomenon.CLITIC_ELISION,
        1,
        xfail=XfailMark(
            defect_slug="stanza-it-verb-clitic-pos-split",
            reason=(
                "UD-level pin: `piccolo` (adjective, \"small\" m.sg) "
                "is spuriously split into `picco + lo` tagged "
                "verb+pron with Part Past features. Committed corpus "
                "ships `verb|picco~pron|lo`; should be "
                "`adj|piccolo-Masc-Sing`. Defect 6 non-verb subclass."
            ),
        ),
    ),
    ProbeCase(
        "gomitolo_noun_bogus_verb",
        ("gomitolo",),
        Phenomenon.CLITIC_ELISION,
        1,
        xfail=XfailMark(
            defect_slug="stanza-it-verb-clitic-pos-split",
            reason=(
                "UD-level pin: `gomitolo` (noun, \"ball of yarn\") is "
                "spuriously split into `gomito + lo`. Committed "
                "corpus ships `verb|gomito~pron|lo`; should be "
                "`noun|gomitolo-Masc-Sing`. Defect 6 non-verb subclass."
            ),
        ),
    ),
    ProbeCase(
        "divano_noun_bogus_verb",
        ("divano",),
        Phenomenon.CLITIC_ELISION,
        1,
        xfail=XfailMark(
            defect_slug="stanza-it-verb-clitic-pos-split",
            reason=(
                "UD-level pin: `divano` (noun, \"sofa\") is "
                "spuriously split into `diva + no` — `no` is not "
                "even a valid Italian clitic ending for verbs, yet "
                "Stanza tags this as verb+pron with Part Past. "
                "Committed corpus ships `verb|diva~pron|no`; should "
                "be `noun|divano-Masc-Sing`. Defect 6 non-verb subclass."
            ),
        ),
    ),
    # ── Genuine verb-clitic compounds (correctness control group) ──
    # Stanza handles bare-compound imperatives correctly (e.g.,
    # `dammela → verb|dare-Imp-S2~pron|me~pron|la`) and Stage 3's
    # assemble_mors serializes the compound `%mor` faithfully. Any
    # future Italian content-quality rule must preserve these exact
    # analyses.
    ProbeCase(
        "dammela_counterexample",
        ("dammela",),
        Phenomenon.CLITIC_ELISION,
    ),
    ProbeCase(
        "dammelo_counterexample",
        ("dammelo",),
        Phenomenon.CLITIC_ELISION,
    ),
    ProbeCase(
        "portalo_counterexample",
        ("portalo",),
        Phenomenon.CLITIC_ELISION,
    ),
    ProbeCase(
        "dammela_in_context_counterexample",
        ("per", "favore", "dammela"),
        Phenomenon.CLITIC_ELISION,
    ),
    # ── Defect 8 candidates from the 2026-04-23 CHILDES-ita corpus
    #    surface-frequency scan
    #    (`scripts/analysis/scan_italian_compound_imperative_candidates.py`).
    #    Each surface below appeared ≥50× in the 184-file corpus
    #    and has verb-imperative+clitic morphology. Observe-only:
    #    the first golden run reveals whether Stanza mid-sentence
    #    mis-tags them as ADJ (Defect 8) or handles them correctly.
    #    Locked-count cases migrate to the Defect 8 allowlist in
    #    `lang_it.rs::IT_COMPOUND_IMPERATIVES`.
    _per_favore("diglielo"),
    _per_favore("mettilo"),
    _per_favore("mettila"),
    _per_favore("mettili"),
    _per_favore("mettiti"),
    _per_favore("prendilo"),
    # ── Dative clitic stack (`-glie-`) observation pass (2026-04-24) ──
    # `diglielo_mid_sentence` above is the baseline — Stanza MWT-expands
    # it cleanly to `di + glie + lo` with correct lemmas. The cases
    # below probe whether the other dative-stacked imperatives behave
    # the same way. Observe-only until the first run; any case that
    # mis-classifies (ADJ-tag + surface-echo lemma, like `prendilo`)
    # migrates to `IT_COMPOUND_IMPERATIVES`. Any case that Range-
    # expands with bogus lemmas would be a new Defect 6 stacked
    # variant and needs its own allowlist decision.
    _per_favore("digliela"),
    _per_favore("dagliela"),
    _per_favore("portagliela"),
    _per_favore("prendigliela"),
    # ── Step 3 corpus-scan probes (2026-04-24) ──
    # The 2026-04-24 CHILDES-ita scan (184 files) surfaced these
    # high-frequency candidates that are not yet in any allowlist.
    # `mettici` extends the `mettere` family that was observation-only
    # in 2026-04-23. The noun candidates (`marrone`, `pallone`,
    # `bastone`, `cappello`) share the `-one`/`-ello` ending that
    # Defect 6's non-verb subclass flagged for `arancione` — probe
    # to see whether Stanza mis-splits them similarly. `difficile`
    # ends in `-le` and may trigger a Stanza adj/adverb mis-POS.
    _per_favore("mettici"),
    ProbeCase.observation_alone("marrone"),
    ProbeCase.observation_alone("pallone"),
    ProbeCase.observation_alone("bastone"),
    ProbeCase.observation_alone("cappello"),
    ProbeCase.observation_alone("difficile"),
    # ── Step 3 audit-driven probes (2026-04-24) ──
    # Surfaced by audit_italian_mor_content.py run against a
    # JSON-parsed corpus mirror of the Italian data repos. These
    # are surfaces where the COMMITTED %mor shows
    # `verb|STEM~pron|CLITIC` with STEM+CLITIC == surface and
    # STEM is not a real Italian verb — classic Defect 6 shape.
    # Probing to confirm current Stanza still mis-splits them before
    # extending the allowlist.
    ProbeCase.observation_alone("seggiola"),
    ProbeCase.observation_alone("piccola"),
    ProbeCase.observation_alone("trottola"),
    ProbeCase.observation_alone("bottone"),
    # ── Audit-surfaced singletons (2026-04-24) ──
    # Each of these appeared exactly once in the JSON-parsed
    # Italian corpus audit as Defect-6-shaped. Probe to see
    # whether current Stanza still mis-splits them; if yes,
    # consider allowlist entry (priority calibrated by word
    # commonness, not just hit count).
    ProbeCase.observation_alone("cielo"),
    ProbeCase.observation_alone("normale"),
    ProbeCase.observation_alone("cavallone"),
    ProbeCase.observation_alone("soffioni"),
    ProbeCase.observation_alone("coccole"),
    ProbeCase.observation_alone("coccolo"),
    ProbeCase.observation_alone("posala"),
    ProbeCase.observation_alone("pettole"),
    ProbeCase.observation_alone("babbolo"),
    ProbeCase.observation_alone("tecala"),
    # ── Defect 10 family probes (2026-04-24) ──
    # `posala` surfaced as a singleton Defect 10 candidate (genuine
    # MWT, correct head POS=VERB, but head lemma=`posa` surface-echo
    # instead of canonical infinitive `posare`). Probe sibling forms
    # and bare imperatives to see whether Defect 10 is a family or
    # a true singleton. If several verbs in the `-a/-are` family
    # show surface-echo lemma, a head-lemma-only reconciler gate is
    # justified.
    ProbeCase(
        label="posa_bare_imperative",
        words=("posa",),
        phenomenon=Phenomenon.CLITIC_ELISION,
    ),
    ProbeCase.observation_alone("posalo"),
    ProbeCase.observation_alone("posali"),
    ProbeCase.observation_alone("posami"),
    # Cross-verb probes — `-a/-are` family imperatives with enclitic
    # accusative. If any of these show surface-echo lemma, the
    # defect is not posa-specific.
    ProbeCase.observation_alone("guardala"),
    ProbeCase.observation_alone("toccala"),
    ProbeCase.observation_alone("aspettala"),
    ProbeCase.observation_alone("mangiala"),
    # Extended -are family sweep (2026-04-24) — establish whether
    # Defect 10 is truly posare-specific or extends to other low-
    # frequency verbs. Probe `-are` imperatives that are CHILDES-
    # common but less frequent than the already-probed
    # `guardare`/`toccare`/`aspettare`/`mangiare`.
    ProbeCase.observation_alone("chiamala"),
    ProbeCase.observation_alone("lasciala"),
    ProbeCase.observation_alone("cambiala"),
    ProbeCase.observation_alone("provala"),
    ProbeCase.observation_alone("giocala"),
    ProbeCase.observation_alone("portala"),
    ProbeCase.observation_alone("suonala"),
    ProbeCase.observation_alone("chiudila"),
    ProbeCase.observation_alone("aprila"),
    # `-ire` family probes — `aprila` surfaced as single-NOUN
    # mis-classification (Defect 8 variant with NOUN gate instead
    # of ADJ). Probe family members to see whether this is a
    # paradigm-level issue or `aprila`-specific.
    ProbeCase.observation_alone("aprilo"),
    ProbeCase.observation_alone("aprili"),
    ProbeCase.observation_alone("sentila"),
    ProbeCase.observation_alone("dormila"),
    ProbeCase.observation_alone("finila"),
    ProbeCase.observation_alone("leggila"),
)

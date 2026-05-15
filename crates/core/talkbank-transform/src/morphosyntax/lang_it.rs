//! Italian-specific morphosyntax rules.
//!
//! This module exists to work around known Stanza 1.11.1 Italian
//! MWT mis-splits (Defect 6 / Defect 7) that corrupt %mor content.
//! When Stanza emits an MWT Range for an Italian input token that
//! we know it mis-analyzes, the reconciler here overrides with a
//! single hand-curated Mor carrying the correct POS, lemma, and
//! features — bypassing the `~`-joined `verb|STEM~pron|CLITIC`
//! assembly that Stanza's split would otherwise produce.
//!
//! This is explicitly a **hack**. The allowlist
//! ([`IT_MIS_SPLIT_OVERRIDES`]) is a closed table of specific
//! mis-splits observed in production Italian corpora. It does not
//! auto-detect novel cases; new ones require an explicit entry plus
//! a regression test. Periodic re-audits should retire entries
//! whose Stanza-raw output has been fixed upstream.
//!
//! See the design plan at
//! `docs/investigations/2026-04-23-italian-defect-6-reconciler-plan.md`
//! and the user-facing documentation at
//! `book/src/reference/languages/italian.md` (§"Reconciler hacks").

use crate::morphosyntax::UdId;
use crate::morphosyntax::{
    ChunkHead, ChunkProvenance, MappingContext, MappingError, MorProvenance, UdPunctable, UdWord,
    UniversalPos, assemble_mors, map_ud_word_to_mor, normalize_deprel, provenance_for_ud_word,
};
use smallvec::{SmallVec, smallvec};
use talkbank_model::model::dependent_tier::mor::Mor;

/// One entry in the Italian Defect-6/7 mis-split allowlist.
///
/// Each entry documents an input token text that Stanza's Italian
/// MWT processor splits into a fake verb+clitic (or fake
/// article+suffix) analysis. When the reconciler matches an entry,
/// it synthesizes a single-word `UdWord` with the overridden POS /
/// lemma / feats and runs it through the normal
/// `map_ud_word_to_mor` pipeline to produce a correct `Mor`.
#[derive(Debug, Clone)]
pub(crate) struct MisSplitOverride {
    /// The input token text that Stanza mis-analyzes. Matched
    /// case-insensitively against the MWT Range parent's `text`
    /// field (i.e., the original input word before any split).
    pub joined_text: &'static str,
    /// POS to assign to the reconciled single-word Mor.
    pub pos: UniversalPos,
    /// Lemma for the reconciled Mor (the correct lexical root
    /// for the word, ignoring Stanza's nonsense component lemma).
    pub lemma: &'static str,
    /// UD feats for the reconciled Mor. `None` produces a Mor with
    /// no morphological suffixes beyond what the POS-specific
    /// feature dispatch computes from an empty feature string.
    pub feats: Option<&'static str>,
}

/// Table of known Italian mis-splits (Defect 6 + Defect 7 family).
///
/// Each entry is sourced from a specific committed %mor regression
/// or a known Stanza-limitation test pin in
/// `_cases/italian.py`. When a new Stanza-mis-split case surfaces
/// in production, add one row here plus a regression test in
/// `morphosyntax/mod.rs` tests.
pub(crate) const IT_MIS_SPLIT_OVERRIDES: &[MisSplitOverride] = &[
    // Defect 6 — verb: `parla → par + la` (2sg/3sg indicative, also
    // 2sg imperative — Stanza's analysis varies by context but the
    // override is the same: `parlare` is the correct lemma either
    // way). Source: childes-other-data/Frogs/Italian-Roma/06/06danbov.cha.
    MisSplitOverride {
        joined_text: "parla",
        pos: UniversalPos::Verb,
        lemma: "parlare",
        feats: Some("Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin"),
    },
    // Defect 6 — noun: `arancione → arancio + ne`. Source:
    // childes-romance-germanic-data/Romance/Italian/Burgato/23.
    MisSplitOverride {
        joined_text: "arancione",
        pos: UniversalPos::Noun,
        lemma: "arancione",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    // Defect 6 — adjective: `piccolo → picco + lo`. Source:
    // childes-romance-germanic-data/Romance/Italian/Calambrone/Martina/020322.
    MisSplitOverride {
        joined_text: "piccolo",
        pos: UniversalPos::Adj,
        lemma: "piccolo",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    // Defect 6 — noun: `gomitolo → gomito + lo`. Source:
    // childes-romance-germanic-data/Romance/Italian/Tonelli/Marco/011026.
    MisSplitOverride {
        joined_text: "gomitolo",
        pos: UniversalPos::Noun,
        lemma: "gomitolo",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    // Defect 6 — noun: `divano → diva + no`. Note that `no` is not
    // even a valid Italian enclitic ending, yet Stanza tags the
    // split as verb+pron. Source:
    // childes-romance-germanic-data/Romance/Italian/Tonelli/Marco/010803.
    MisSplitOverride {
        joined_text: "divano",
        pos: UniversalPos::Noun,
        lemma: "divano",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    // Defect 6 non-verb entries surfaced by the 2026-04-24 CHILDES-ita
    // scan + direct probe. All four mis-splits share the shape
    // `verb|STEM + pron|CLITIC` with a nonsense STEM lemma
    // (`pallare`, `bastare`, `cappere`, `diffire` — none are real
    // Italian verbs). Corpus frequencies: 94 / 48 / 56 / 46
    // respectively across the 184-file scan.
    MisSplitOverride {
        joined_text: "pallone",
        pos: UniversalPos::Noun,
        lemma: "pallone",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    MisSplitOverride {
        joined_text: "bastone",
        pos: UniversalPos::Noun,
        lemma: "bastone",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    MisSplitOverride {
        joined_text: "cappello",
        pos: UniversalPos::Noun,
        lemma: "cappello",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    // `difficile` is an adjective rather than a noun — the `-le`
    // ending masquerades as the pronoun `le` to Stanza's splitter.
    MisSplitOverride {
        joined_text: "difficile",
        pos: UniversalPos::Adj,
        lemma: "difficile",
        feats: Some("Number=Sing"),
    },
    // Defect 6 non-verb entries surfaced by the 2026-04-24
    // audit_italian_mor_content.py run against a pre-parsed JSON
    // corpus mirror covering the Italian data repos (childes-*,
    // aphasia, slabank, ca-data MOVIN). Detected via the
    // `STEM + CLITIC == surface` signature in committed %mor
    // with nonsense STEM lemma. Current-Stanza probes on
    // 2026-04-24 confirmed each is still mis-split.
    MisSplitOverride {
        joined_text: "seggiola",
        pos: UniversalPos::Noun,
        lemma: "seggiola",
        feats: Some("Gender=Fem|Number=Sing"),
    },
    // `piccola` is the feminine of already-handled `piccolo`;
    // Stanza splits it as `picco + la` (distinct from `picco + lo`
    // for the masculine), so a separate allowlist entry is needed.
    MisSplitOverride {
        joined_text: "piccola",
        pos: UniversalPos::Adj,
        lemma: "piccolo",
        feats: Some("Gender=Fem|Number=Sing"),
    },
    MisSplitOverride {
        joined_text: "trottola",
        pos: UniversalPos::Noun,
        lemma: "trottola",
        feats: Some("Gender=Fem|Number=Sing"),
    },
    MisSplitOverride {
        joined_text: "bottone",
        pos: UniversalPos::Noun,
        lemma: "bottone",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    // Singleton audit hits (2026-04-24) — each appeared exactly
    // once in the committed-corpus audit, but each is a common
    // enough Italian word that adding the entry is low-cost
    // insurance against future corpus processing. Rare/dialectal
    // singletons (`soffioni`, `coccolo`, `pettole`, `babbolo`,
    // `tecala`) were deliberately skipped — see the pause notes
    // for the long-tail deferral reasoning.
    MisSplitOverride {
        joined_text: "cielo",
        pos: UniversalPos::Noun,
        lemma: "cielo",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    // Italian UD tags consonant-final adjectives without gender
    // (the form is gender-invariant); only Number is marked.
    MisSplitOverride {
        joined_text: "normale",
        pos: UniversalPos::Adj,
        lemma: "normale",
        feats: Some("Number=Sing"),
    },
    // Augmentative of the common noun `cavallo`; Stanza tags
    // `cavallo` itself correctly but splits the `-one`
    // augmentative. Keep lemma as the augmentative surface per
    // Italian convention (augmentatives lemmatize to themselves,
    // not to the base noun).
    MisSplitOverride {
        joined_text: "cavallone",
        pos: UniversalPos::Noun,
        lemma: "cavallone",
        feats: Some("Gender=Masc|Number=Sing"),
    },
    MisSplitOverride {
        joined_text: "coccole",
        pos: UniversalPos::Noun,
        lemma: "coccole",
        feats: Some("Gender=Fem|Number=Plur"),
    },
    // Defect 7 — sentence-initial article `la → il + i`. Stanza
    // expands the feminine singular article into a masculine
    // singular + masculine plural article pair. Source:
    // childes-other-data/Frogs/Italian-Roma/10/10dancop.cha —
    // "la storia parla di un bambino".
    MisSplitOverride {
        joined_text: "la",
        pos: UniversalPos::Det,
        lemma: "il",
        feats: Some("Definite=Def|Gender=Fem|Number=Sing|PronType=Art"),
    },
];

/// Look up a potential mis-split override for an MWT Range's
/// parent token.
///
/// The key is the Range parent's `text` field — the original input
/// word Stanza received before any MWT expansion. Matching is
/// case-insensitive.
///
/// Returns `None` for any input not in the allowlist; the normal
/// `assemble_mors` path then handles it. This is what keeps genuine
/// verb+clitic compounds like `dammela`, `portalo`, `dammelo` on
/// the correct `~`-merged path.
fn check_italian_mis_split(range_parent_text: &str) -> Option<&'static MisSplitOverride> {
    // All allowlist `joined_text` entries are pure ASCII, so
    // `eq_ignore_ascii_case` is correct and zero-allocation.
    IT_MIS_SPLIT_OVERRIDES
        .iter()
        .find(|o| o.joined_text.eq_ignore_ascii_case(range_parent_text))
}

/// Apply a mis-split override by synthesizing a single-word
/// `UdWord` with the overridden POS/lemma/feats and mapping it
/// through the normal `map_ud_word_to_mor` pipeline.
///
/// Head and deprel are taken from a sample (typically the first
/// component word Stanza emitted) so the GRA relation that will
/// later be built for this chunk preserves the sentence's
/// dependency structure.
fn apply_mis_split_override(
    over: &MisSplitOverride,
    sample_head: usize,
    sample_deprel: &str,
    ctx: &MappingContext,
) -> Result<Mor, MappingError> {
    let synthetic = UdWord::synthetic(
        over.joined_text,
        over.lemma,
        over.pos,
        over.feats,
        sample_head,
        sample_deprel,
    );
    map_ud_word_to_mor(&synthetic, ctx)
}

/// Handle Italian Range-token reconciler cases if the parent token is in one of
/// the known Stanza mis-split allowlists.
pub fn try_handle_italian_range_override(
    ud: &UdWord,
    components: &[UdWord],
    ctx: &MappingContext,
) -> Result<Option<(Mor, MorProvenance)>, MappingError> {
    if let Some(over) = check_italian_mis_split(&ud.text) {
        let head_comp = components.first().unwrap_or(ud);
        let mor = apply_mis_split_override(over, head_comp.head, &head_comp.deprel, ctx)?;
        let UdId::Range(start, end) = &ud.id else {
            return Ok(None);
        };
        let source_ud_ids: SmallVec<[usize; 1]> = (*start..=*end).collect();
        let deprel = normalize_deprel(&head_comp.deprel, || {
            format!("collapsed Range {:?}", ud.text)
        })?;
        let provenance = smallvec![ChunkProvenance::collapsed_range(
            source_ud_ids,
            ChunkHead::from_ud_head(head_comp.head),
            deprel,
        )];
        return Ok(Some((mor, provenance)));
    }

    if let Some(rewrite) = check_italian_component_rewrite(&ud.text) {
        let rewritten = apply_component_rewrite(rewrite, components);
        return assemble_mors(&rewritten, ctx).map(Some);
    }

    Ok(None)
}

// ─── Defect 8 — mid-sentence compound imperative mis-classification ──
//
// Stanza-1.11.1 Italian mis-tags certain imperative+enclitic
// compound words as ADJ with a vowel-normalized lemma when they
// appear mid-sentence (e.g. `per favore dammela` → single UD word
// `dammela` tagged ADJ with lemma `dammelo`). There is **no MWT
// Range** — Stanza emits one word per input, unlike the standalone
// case where Stanza correctly fires its MWT processor and produces
// a 3-word verb+clitic decomposition. The injection pipeline
// therefore can't rely on the same Range-branch hook used by
// Defects 6 and 7; the fix fires on `UdId::Single` values that
// match a surface-form allowlist of known compound imperatives.
//
// Scope trade-off: this reconciler emits a **single-chunk** `Mor`
// overriding the POS (`ADJ → VERB`) and lemma (`dammelo → dare`).
// It does NOT decompose the compound into its main verb plus
// clitic post-clitics — that would require producing a multi-
// chunk Mor from one UdId::Single, which would invalidate the
// chunk-index accounting used by `build_gra_and_validate`.
// Consumers lose the clitic structure mid-sentence but gain the
// correct POS and verb lemma, which is still a substantial
// improvement over `adj|dammelo-S1`. Multi-chunk decomposition is
// a future enhancement if the chunk accounting is extended to
// support per-UD-word expansion counts.

/// One post-clitic in a reconciled Italian compound imperative.
///
/// Each entry in `IT_COMPOUND_IMPERATIVES` carries a static list
/// of post-clitics; the reconciler synthesizes one `MorWord` per
/// clitic via `map_ud_word_to_mor` and attaches it to the main
/// verb `Mor` via `with_post_clitic`. This produces the full
/// multi-chunk `verb|LEMMA~pron|X~pron|Y` output that matches
/// Stanza's native analysis for the bare-compound case.
///
/// An empty clitic slice means "single-chunk mode" — the
/// reconciler emits only the main verb Mor. Use this when the
/// historical single-chunk output is sufficient and the clitic
/// decomposition isn't worth spelling out.
#[derive(Debug, Clone)]
pub(crate) struct CliticSpec {
    /// Surface text of the clitic (`me`, `mi`, `la`, `lo`, …).
    pub text: &'static str,
    /// UD lemma for the clitic. Italian UD tends to use the
    /// clitic's canonical form as lemma (`me`, `la`, `gli`).
    pub lemma: &'static str,
    /// UD POS. Typically `Pron`; `Adv` only for `ne` in some
    /// analyses.
    pub upos: UniversalPos,
    /// UD feats string for the clitic — number, person, gender,
    /// PronType, etc.
    pub feats: &'static str,
    /// UD dependency relation from this clitic to the main verb:
    /// `obj` for direct-object clitics (`la`, `lo`, `le`, `li`),
    /// `iobj` for indirect-object clitics (`me`, `mi`, `ti`,
    /// `ci`, `vi`, `gli`), etc. The GRA builder uses this to
    /// emit a relation pointing from the clitic chunk back to
    /// the main verb's chunk.
    pub deprel: &'static str,
}

/// Known Italian imperative+enclitic compounds that Stanza
/// mis-classifies mid-sentence.
///
/// Each entry pairs the surface form (as Stanza sees it) with the
/// correct verb lemma, feats, and the clitic stack. Add entries
/// when corpus scans surface new mis-classifications.
///
/// See also `IT_COMPOUND_IMPERATIVE_GATE_POS` — the set of Stanza
/// POS tags that signal a potential mis-classification and allow
/// the gate to fire.
#[derive(Debug, Clone)]
pub(crate) struct CompoundImperativeOverride {
    /// Surface form Stanza received, matched case-insensitively
    /// against the UdWord's `text` field.
    pub surface: &'static str,
    /// The correct verb lemma (e.g. `dare` for `dammela`,
    /// `portare` for `portalo`).
    pub verb_lemma: &'static str,
    /// UD feats for the reconciled verb Mor. Encodes imperative
    /// 2sg (or 2pl) by default.
    pub verb_feats: &'static str,
    /// Post-clitics stacked on the main verb, in serialization
    /// order. Empty slice means "emit main verb only" —
    /// single-chunk mode preserved for allowlist entries where
    /// the clitic decomposition isn't worth spelling out.
    pub clitics: &'static [CliticSpec],
}

/// Common Italian clitic specs reused across multiple entries.
/// Kept as module-level constants so `IT_COMPOUND_IMPERATIVES`
/// entries stay compact and mutation-free.
const CLITIC_ME: CliticSpec = CliticSpec {
    text: "me",
    lemma: "me",
    upos: UniversalPos::Pron,
    feats: "Number=Sing|Person=1|PronType=Prs",
    deprel: "iobj",
};
const CLITIC_LA: CliticSpec = CliticSpec {
    text: "la",
    lemma: "la",
    upos: UniversalPos::Pron,
    feats: "Gender=Fem|Number=Sing|Person=3|PronType=Prs",
    deprel: "obj",
};
const CLITIC_LO: CliticSpec = CliticSpec {
    text: "lo",
    lemma: "lo",
    upos: UniversalPos::Pron,
    feats: "Gender=Masc|Number=Sing|Person=3|PronType=Prs",
    deprel: "obj",
};
const CLITIC_LI: CliticSpec = CliticSpec {
    text: "li",
    lemma: "li",
    upos: UniversalPos::Pron,
    feats: "Gender=Masc|Number=Plur|Person=3|PronType=Prs",
    deprel: "obj",
};
const CLITIC_LE: CliticSpec = CliticSpec {
    text: "le",
    lemma: "le",
    upos: UniversalPos::Pron,
    feats: "Gender=Fem|Number=Plur|Person=3|PronType=Prs",
    deprel: "obj",
};

/// Table of known Italian mid-sentence compound imperative
/// mis-classifications. Conservative seed — only two surfaces
/// confirmed via direct Stanza observation (2026-04-23 probes).
/// Extension is mechanical: one row per newly-observed surface.
pub(crate) const IT_COMPOUND_IMPERATIVES: &[CompoundImperativeOverride] = &[
    // dammela / dammelo (dare + me + la/lo): direct probe observation
    // — Stanza mid-sentence tags ADJ with lemma=`dammelo`.
    CompoundImperativeOverride {
        surface: "dammela",
        verb_lemma: "dare",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_ME, CLITIC_LA],
    },
    CompoundImperativeOverride {
        surface: "dammelo",
        verb_lemma: "dare",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_ME, CLITIC_LO],
    },
    // prendilo / prendila / prendili / prendile
    // (prendere + lo/la/li/le): surfaced by the 2026-04-23 corpus
    // scan (`scripts/analysis/scan_italian_compound_imperative_candidates.py`);
    // `prendilo` confirmed via direct Stanza probe as ADJ-tagged
    // mid-sentence. The other three are family additions — Stanza's
    // behavior is consistent within an imperative+clitic paradigm
    // once one surface mis-classifies, but each should receive its
    // own probe-observation in a follow-up pass.
    CompoundImperativeOverride {
        surface: "prendilo",
        verb_lemma: "prendere",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LO],
    },
    CompoundImperativeOverride {
        surface: "prendila",
        verb_lemma: "prendere",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LA],
    },
    CompoundImperativeOverride {
        surface: "prendili",
        verb_lemma: "prendere",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LI],
    },
    CompoundImperativeOverride {
        surface: "prendile",
        verb_lemma: "prendere",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LE],
    },
    // `-ire` family surfaces where Stanza fails to detect the MWT
    // boundary and mis-classifies the single-word surface.
    // Observed 2026-04-24 via direct probe:
    // - `aprila` / `aprili` — tagged NOUN (not ADJ); `aprili`
    //   additionally gets a homograph lemma `aprile` (month April).
    //   Both should be imperative `aprire` + clitic.
    // - `finila` — tagged ADJ with lemma `finile` (vowel-normalized
    //   surface echo); should be imperative `finire` + clitic.
    // Gate accepts both ADJ and NOUN since both signatures appear.
    CompoundImperativeOverride {
        surface: "aprila",
        verb_lemma: "aprire",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LA],
    },
    CompoundImperativeOverride {
        surface: "aprili",
        verb_lemma: "aprire",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LI],
    },
    CompoundImperativeOverride {
        surface: "finila",
        verb_lemma: "finire",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LA],
    },
    // Defect 12 — `aprilo`: Stanza tags it as single-word VERB
    // with correct lemma `aprire`, but fails to do the MWT
    // expansion. Only the clitic decomposition is missing.
    // Direct probe 2026-04-24.
    CompoundImperativeOverride {
        surface: "aprilo",
        verb_lemma: "aprire",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LO],
    },
    // Defect 13 — `leggila`: Stanza tags it as single-word VERB
    // with a **fabricated** lemma `leggilare` (not a real Italian
    // verb). Should be imperative `leggere` + `la`. Direct probe
    // 2026-04-24.
    CompoundImperativeOverride {
        surface: "leggila",
        verb_lemma: "leggere",
        verb_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        clitics: &[CLITIC_LA],
    },
];

/// Look up a potential mid-sentence compound imperative override
/// for a single UdWord.
///
/// The gate is two-pronged: Stanza must have tagged the word with
/// a POS we have seen indicate mis-classification (ADJ, NOUN, or
/// VERB with missing MWT expansion) AND the text must appear in
/// the allowlist. Returns `None` when either fails, leaving the
/// normal `map_ud_word_to_mor` path in charge.
///
/// Gate rationale for accepting ADJ, NOUN, and VERB:
/// - ADJ: the original Defect 8 signature (`dammela`, `prendilo`,
///   `finila` — Stanza vowel-normalizes the lemma and tags
///   adjective).
/// - NOUN: `-ire` imperative+clitic surfaces where Stanza doesn't
///   detect the MWT boundary and tags the whole compound as a
///   single noun (`aprila → aprila/NOUN/aprila`) or a homograph
///   noun (`aprili → aprili/NOUN/aprile`).
/// - VERB: Defects 12 and 13 — Stanza gets POS=VERB right but
///   either fails to MWT-expand (`aprilo → aprilo/VERB/aprire`)
///   or invents a fabricated lemma (`leggila → leggila/VERB/
///   leggilare`, where `leggilare` is not a real Italian verb).
///
/// The allowlist is a closed curated set; accepting all three
/// POS values expands the set of mis-classifications the gate
/// can catch. Legitimate tokens whose surface matches an
/// allowlist entry would only be over-matched if a real word
/// shares spelling with one of the entries — no such collisions
/// are known for current entries. Each entry also preserves the
/// original UdWord's head/deprel for the main chunk so GRA
/// reindexing stays consistent.
fn check_italian_compound_imperative(
    text: &str,
    upos: &UdPunctable<UniversalPos>,
) -> Option<&'static CompoundImperativeOverride> {
    if !matches!(
        upos,
        UdPunctable::Value(UniversalPos::Adj)
            | UdPunctable::Value(UniversalPos::Noun)
            | UdPunctable::Value(UniversalPos::Verb),
    ) {
        return None;
    }
    // All allowlist `surface` entries are pure ASCII.
    IT_COMPOUND_IMPERATIVES
        .iter()
        .find(|o| o.surface.eq_ignore_ascii_case(text))
}

/// Apply a compound-imperative override by synthesizing the main
/// verb UdWord plus one UdWord per post-clitic, mapping each
/// through `map_ud_word_to_mor`, and stacking the clitics onto
/// the main `Mor`.
///
/// The result is a multi-chunk `Mor` of shape
/// `verb|LEMMA-<feats>~pron|CLITIC1-<feats>~pron|CLITIC2-<feats>`,
/// matching the output Stanza produces natively for the bare-
/// compound case. When `over.clitics` is empty the output is a
/// single-chunk Mor — that mode is still available but not used
/// by any current allowlist entry.
///
/// Preserves the original UdWord's head and deprel for the main
/// chunk; GRA relations for the post-clitics point back to the
/// main chunk via the caller (`build_gra_and_validate`).
fn apply_compound_imperative_override(
    over: &CompoundImperativeOverride,
    original_head: usize,
    original_deprel: &str,
    ctx: &MappingContext,
) -> Result<Mor, MappingError> {
    let main_ud = UdWord::synthetic(
        over.surface,
        over.verb_lemma,
        UniversalPos::Verb,
        Some(over.verb_feats),
        original_head,
        original_deprel,
    );
    let mut mor = map_ud_word_to_mor(&main_ud, ctx)?;
    for clitic in over.clitics {
        let clitic_ud = UdWord::synthetic(
            clitic.text,
            clitic.lemma,
            clitic.upos,
            Some(clitic.feats),
            0,
            clitic.deprel,
        );
        let clitic_mor = map_ud_word_to_mor(&clitic_ud, ctx)?;
        mor = mor.with_post_clitic(clitic_mor.main);
    }
    Ok(mor)
}

/// Handle Italian single-token compound-imperative reconciler cases if the
/// token matches one of the known Stanza misclassification allowlists.
pub fn try_handle_italian_single_override(
    ud: &UdWord,
    ctx: &MappingContext,
) -> Result<Option<(Mor, MorProvenance)>, MappingError> {
    if let Some(over) = check_italian_compound_imperative(&ud.text, &ud.upos) {
        let mor = apply_compound_imperative_override(over, ud.head, &ud.deprel, ctx)?;
        let mut provenance: MorProvenance = SmallVec::new();
        provenance.push(provenance_for_ud_word(ud)?);
        for clitic in over.clitics {
            let deprel = normalize_deprel(clitic.deprel, || {
                format!("synthesized clitic {:?} in {:?}", clitic.text, over.surface)
            })?;
            provenance.push(ChunkProvenance::synthetic_post_clitic(deprel));
        }
        return Ok(Some((mor, provenance)));
    }

    Ok(None)
}

// ─── Defect 9 — Range-expansion with wrong head POS ──────────────
//
// Stanza-1.11.1 expands certain imperative+clitic compound verbs as
// a structurally-correct MWT Range but tags the head component with
// a homographic non-verb POS (e.g. ADP for `da`) and a surface-echo
// lemma. Observed case: `dagliela` (2sg imperative of `dare` + 3sg
// dative `glie` + 3sg.f.acc `la`) expands as
// `da/ADP/da + glie/PRON/gli + la/PRON/la` instead of
// `da/VERB/dare + glie/PRON/gli + la/PRON/la`. The MWT expansion
// shape is right — it's the head POS/lemma that's wrong.
//
// Sibling imperatives in the same dative-stack family
// (`digliela`, `portagliela`, `prendigliela`) are analyzed
// correctly by Stanza. The defect is specific to surfaces where the
// head clitic-stripped form is homographic with a non-verb word.
//
// Fix shape: rewrite component 0's POS/lemma/feats in-place before
// `assemble_mors` runs. The remaining components pass through
// unchanged, so the 3-chunk `~`-joined Mor shape is preserved —
// unlike Defect 6 which collapses the Range into a single chunk.

/// One entry in the Italian Defect-9 component-rewrite allowlist.
///
/// Matched case-insensitively against the Range parent's `text`
/// field (i.e. the joined surface form Stanza received before MWT
/// expansion). When matched, the reconciler produces a rewritten
/// slice of components with component 0's POS/lemma/feats replaced,
/// then passes the rewritten slice to `assemble_mors`.
#[derive(Debug, Clone)]
pub(crate) struct ComponentRewriteOverride {
    /// The Range parent text Stanza mis-analyzes. Matched
    /// case-insensitively. Example: `"dagliela"`.
    pub joined_text: &'static str,
    /// POS to assign to component 0 of the expansion.
    pub head_pos: UniversalPos,
    /// Lemma for component 0 (the correct lexical root).
    pub head_lemma: &'static str,
    /// UD feats string for component 0 (e.g. imperative 2sg).
    pub head_feats: &'static str,
}

/// Table of known Italian Defect-9 component rewrites. Closed
/// allowlist; each entry is seeded from a specific direct-probe
/// observation against Stanza. See the module docstring for the
/// scope trade-off and the plan file at
/// `docs/investigations/2026-04-23-italian-defect-6-reconciler-plan.md`.
pub(crate) const IT_COMPONENT_REWRITES: &[ComponentRewriteOverride] = &[
    // Defect 9 — `dagliela` (imperative `dare` + `glie` + `la`):
    // Stanza emits `da/ADP/da + glie/PRON/gli + la/PRON/la`. The
    // expansion is shape-correct (3 pieces for a 3-piece clitic
    // stack) but the head is mis-tagged ADP with surface-echo lemma.
    // Probe source:
    // `batchalign/tests/investigations/_cases/italian.py`
    // `dagliela_mid_sentence` (2026-04-24). Sibling forms
    // `digliela`/`portagliela`/`prendigliela` are Stanza-correct
    // and must stay off this allowlist.
    ComponentRewriteOverride {
        joined_text: "dagliela",
        head_pos: UniversalPos::Verb,
        head_lemma: "dare",
        head_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
    },
    // Defect 10 — `posala` / `posalo` (imperative `posare` +
    // `la`/`lo`): Stanza emits `posa/VERB/posa + la/PRON` — shape
    // correct, head POS correct, but head lemma is surface-echo
    // `posa` instead of the canonical infinitive `posare`. Unlike
    // Defect 9, the POS rewrite is a no-op; only the lemma is wrong.
    // Fits the same component-rewrite shape, so no new allowlist is
    // needed.
    //
    // Verb-specific to `posare`: the 2026-04-24 cross-verb probe
    // confirmed `guardare`, `toccare`, `aspettare`, `mangiare` all
    // lemmatize correctly in this position. Stanza's Italian model
    // has a specific weakness on the `posare` paradigm.
    //
    // Audit source: 1 hit in the 2026-04-24 fleet JSON audit
    // (`posala`). `posalo` added as a family member via direct
    // probe confirmation.
    ComponentRewriteOverride {
        joined_text: "posala",
        head_pos: UniversalPos::Verb,
        head_lemma: "posare",
        head_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
    },
    ComponentRewriteOverride {
        joined_text: "posalo",
        head_pos: UniversalPos::Verb,
        head_lemma: "posare",
        head_feats: "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
    },
];

/// Look up a potential Defect-9 component-rewrite override for an
/// MWT Range's parent token. Returns `None` when the input is not
/// in the allowlist, leaving the normal `assemble_mors` path in
/// charge — which is what keeps correctly-analyzed siblings
/// (`digliela`, `portagliela`, `prendigliela`, `dammela`, ...) on
/// the correct path.
fn check_italian_component_rewrite(
    range_parent_text: &str,
) -> Option<&'static ComponentRewriteOverride> {
    // All allowlist `joined_text` entries are pure ASCII.
    IT_COMPONENT_REWRITES
        .iter()
        .find(|o| o.joined_text.eq_ignore_ascii_case(range_parent_text))
}

/// Apply a Defect-9 component-rewrite by cloning the components
/// slice and overwriting component 0's POS/lemma/feats. The
/// returned Vec is owned by the caller and can be passed directly
/// to `assemble_mors`.
///
/// The rewrite is purely local — only component 0's `upos`,
/// `lemma`, and `feats` change. All other fields (id, text, head,
/// deprel, deps, misc) are preserved so GRA arc reindexing stays
/// consistent with the normal Range path.
fn apply_component_rewrite(over: &ComponentRewriteOverride, components: &[UdWord]) -> Vec<UdWord> {
    let mut rewritten: Vec<UdWord> = components.to_vec();
    if let Some(head) = rewritten.first_mut() {
        head.upos = UdPunctable::Value(over.head_pos);
        head.lemma = over.head_lemma.to_string();
        head.feats = Some(over.head_feats.to_string());
    }
    rewritten
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_returns_some_for_allowlist_entries() {
        for over in IT_MIS_SPLIT_OVERRIDES {
            let found = check_italian_mis_split(over.joined_text);
            assert!(
                found.is_some(),
                "expected {} to be found in allowlist",
                over.joined_text
            );
        }
    }

    #[test]
    fn check_is_case_insensitive() {
        assert!(check_italian_mis_split("Parla").is_some());
        assert!(check_italian_mis_split("PARLA").is_some());
        assert!(check_italian_mis_split("PiCcOlO").is_some());
    }

    #[test]
    fn check_returns_none_for_genuine_compounds() {
        // Genuine verb+clitic compounds must not be in the allowlist.
        assert!(check_italian_mis_split("dammela").is_none());
        assert!(check_italian_mis_split("dammelo").is_none());
        assert!(check_italian_mis_split("portalo").is_none());
        assert!(check_italian_mis_split("guardarla").is_none());
    }

    #[test]
    fn check_returns_none_for_unrelated_words() {
        assert!(check_italian_mis_split("casa").is_none());
        assert!(check_italian_mis_split("bambino").is_none());
        assert!(check_italian_mis_split("").is_none());
    }

    // ─── Defect 9 component-rewrite tests ──

    #[test]
    fn component_rewrite_fires_for_dagliela() {
        assert!(check_italian_component_rewrite("dagliela").is_some());
        assert!(check_italian_component_rewrite("DAGLIELA").is_some());
        assert!(check_italian_component_rewrite("DaGlIeLa").is_some());
    }

    #[test]
    fn component_rewrite_skips_stanza_correct_siblings() {
        // Stanza analyses these correctly — must NOT be in the
        // Defect 9 allowlist or the reconciler would corrupt their
        // already-good head POS/lemma.
        assert!(check_italian_component_rewrite("digliela").is_none());
        assert!(check_italian_component_rewrite("portagliela").is_none());
        assert!(check_italian_component_rewrite("prendigliela").is_none());
        // Bare 2sg imperatives with clitics (Defect 8 / correct MWT
        // territory, not Defect 9).
        assert!(check_italian_component_rewrite("dammela").is_none());
        assert!(check_italian_component_rewrite("portalo").is_none());
    }

    #[test]
    fn apply_component_rewrite_only_touches_head() {
        let over = &IT_COMPONENT_REWRITES[0];
        let components = vec![
            UdWord {
                id: UdId::Single(1),
                text: "da".into(),
                lemma: "da".into(),
                upos: UdPunctable::Value(UniversalPos::Adp),
                xpos: None,
                feats: None,
                head: 0,
                deprel: "root".into(),
                deps: None,
                misc: None,
            },
            UdWord {
                id: UdId::Single(2),
                text: "glie".into(),
                lemma: "gli".into(),
                upos: UdPunctable::Value(UniversalPos::Pron),
                xpos: None,
                feats: Some("Person=3".into()),
                head: 1,
                deprel: "iobj".into(),
                deps: None,
                misc: None,
            },
        ];
        let rewritten = apply_component_rewrite(over, &components);
        assert_eq!(rewritten.len(), 2);
        // Head rewritten.
        assert!(matches!(
            rewritten[0].upos,
            UdPunctable::Value(UniversalPos::Verb)
        ));
        assert_eq!(rewritten[0].lemma, "dare");
        assert_eq!(
            rewritten[0].feats.as_deref(),
            Some("Mood=Imp|Number=Sing|Person=2|VerbForm=Fin")
        );
        // Head text / id / head / deprel preserved so GRA reindexing
        // stays consistent.
        assert_eq!(rewritten[0].text, "da");
        assert_eq!(rewritten[0].deprel, "root");
        // Tail component untouched.
        assert_eq!(rewritten[1], components[1]);
    }
}

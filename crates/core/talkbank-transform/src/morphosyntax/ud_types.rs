//! Typed views over Universal Dependencies values: the 17 UPOS tags,
//! the dependency-relation labels, the `VerbForm` feature, and the
//! per-token record (`UdWord`/`UdSentence`/`UdResponse`) that the
//! pipeline marshals back from the Stanza worker. Plus the small
//! cleanup pass (`validate_and_clean`, `is_bogus_lemma`,
//! `sanitize_mor_text`) and the canonical UD feature-bundle constants.

/// The 17 Universal POS tags as defined by Universal Dependencies v2.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum UniversalPos {
    /// Adjective.
    Adj,
    /// Adposition.
    Adp,
    /// Adverb.
    Adv,
    /// Auxiliary verb.
    Aux,
    /// Coordinating conjunction.
    Cconj,
    /// Determiner.
    Det,
    /// Pronoun.
    Pron,
    /// Common noun.
    Noun,
    /// Proper noun.
    Propn,
    /// Numeral.
    Num,
    /// Particle.
    Part,
    /// Main verb.
    Verb,
    /// Subordinating conjunction.
    Sconj,
    /// Punctuation.
    Punct,
    /// Symbol.
    Sym,
    /// Interjection.
    Intj,
    /// Other / unknown.
    X,
}

impl UniversalPos {
    /// The lowercase CHAT POS category name for this UPOS.
    pub fn to_chat_pos_name(self) -> &'static str {
        match self {
            Self::Adj => "adj",
            Self::Adp => "adp",
            Self::Adv => "adv",
            Self::Aux => "aux",
            Self::Cconj => "cconj",
            Self::Det => "det",
            Self::Intj => "intj",
            Self::Noun => "noun",
            Self::Num => "num",
            Self::Part => "part",
            Self::Pron => "pron",
            Self::Propn => "propn",
            Self::Punct => "punct",
            Self::Sconj => "sconj",
            Self::Sym | Self::X => "x",
            Self::Verb => "verb",
        }
    }

    /// Parse a POS category name into a `UniversalPos`.
    pub fn from_pos_name(name: &str) -> Option<Self> {
        let eq = |s: &str| name.eq_ignore_ascii_case(s);
        if eq("adj") {
            Some(Self::Adj)
        } else if eq("adp") {
            Some(Self::Adp)
        } else if eq("adv") {
            Some(Self::Adv)
        } else if eq("aux") {
            Some(Self::Aux)
        } else if eq("cconj") {
            Some(Self::Cconj)
        } else if eq("det") {
            Some(Self::Det)
        } else if eq("intj") {
            Some(Self::Intj)
        } else if eq("noun") {
            Some(Self::Noun)
        } else if eq("num") {
            Some(Self::Num)
        } else if eq("part") {
            Some(Self::Part)
        } else if eq("pron") {
            Some(Self::Pron)
        } else if eq("propn") {
            Some(Self::Propn)
        } else if eq("punct") {
            Some(Self::Punct)
        } else if eq("sconj") {
            Some(Self::Sconj)
        } else if eq("verb") {
            Some(Self::Verb)
        } else if eq("sym") || eq("x") {
            Some(Self::X)
        } else {
            None
        }
    }
}

/// Universal Dependencies relation label.
///
/// Known values get dedicated variants so call sites compile-check against
/// typos. Unknown relations land in `Other(String)` so round-tripping stays
/// lossless without allocating on the known-value hot path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DepRel {
    /// `root` — the sentence-level root.
    Root,
    /// `nsubj` — nominal subject.
    NSubj,
    /// `nsubj:pass` — nominal passive subject.
    NSubjPass,
    /// `obj` — direct object.
    Obj,
    /// `aux` — auxiliary.
    Aux,
    /// `aux:pass` — passive auxiliary.
    AuxPass,
    /// `cop` — copula.
    Cop,
    /// `case` — case-marking word, including possessive `'s`.
    Case,
    /// `nmod:poss` — possessive nominal modifier.
    NmodPoss,
    /// `det` — determiner.
    Det,
    /// `cc` — coordinating conjunction.
    Cc,
    /// `conj` — conjoined element.
    Conj,
    /// `compound` — compound modifier.
    Compound,
    /// `compound:prt` — phrasal verb particle.
    CompoundPrt,
    /// `amod` — adjectival modifier.
    Amod,
    /// `advmod` — adverbial modifier.
    AdvMod,
    /// `punct` — punctuation.
    Punct,
    /// `discourse` — discourse element.
    Discourse,
    /// `mark` — subordinating marker.
    Mark,
    /// `expl` — expletive, such as existential `there`.
    Expl,
    /// Any other UD relation, preserved as its original string.
    Other(String),
}

impl DepRel {
    /// Parse a UD relation string into a typed variant.
    pub fn parse(s: &str) -> Self {
        match s {
            "root" => Self::Root,
            "nsubj" => Self::NSubj,
            "nsubj:pass" => Self::NSubjPass,
            "obj" => Self::Obj,
            "aux" => Self::Aux,
            "aux:pass" => Self::AuxPass,
            "cop" => Self::Cop,
            "case" => Self::Case,
            "nmod:poss" => Self::NmodPoss,
            "det" => Self::Det,
            "cc" => Self::Cc,
            "conj" => Self::Conj,
            "compound" => Self::Compound,
            "compound:prt" => Self::CompoundPrt,
            "amod" => Self::Amod,
            "advmod" => Self::AdvMod,
            "punct" => Self::Punct,
            "discourse" => Self::Discourse,
            "mark" => Self::Mark,
            "expl" => Self::Expl,
            other => Self::Other(other.to_string()),
        }
    }

    /// Serialize back to the UD relation string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Root => "root",
            Self::NSubj => "nsubj",
            Self::NSubjPass => "nsubj:pass",
            Self::Obj => "obj",
            Self::Aux => "aux",
            Self::AuxPass => "aux:pass",
            Self::Cop => "cop",
            Self::Case => "case",
            Self::NmodPoss => "nmod:poss",
            Self::Det => "det",
            Self::Cc => "cc",
            Self::Conj => "conj",
            Self::Compound => "compound",
            Self::CompoundPrt => "compound:prt",
            Self::Amod => "amod",
            Self::AdvMod => "advmod",
            Self::Punct => "punct",
            Self::Discourse => "discourse",
            Self::Mark => "mark",
            Self::Expl => "expl",
            Self::Other(s) => s.as_str(),
        }
    }
}

/// UD feat string for a finite indicative present 3rd-person-singular form
/// such as a contracted copula or auxiliary.
pub const FINITE_COPULA_PRES_3SG: &str = "Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin";

/// UD feat string for a present participle, such as `going` or `washing`.
pub const PRESENT_PARTICIPLE: &str = "Tense=Pres|VerbForm=Part";

/// Typed view over UD's `VerbForm` feature values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VerbForm {
    /// Finite form.
    Fin,
    /// Participle form.
    Part,
    /// Gerund.
    Ger,
    /// Infinitive form.
    Inf,
    /// Supine form.
    Sup,
    /// Converb.
    Conv,
    /// Verbal noun.
    Vnoun,
    /// Any other UD `VerbForm` value, preserved as written.
    Other(String),
}

impl VerbForm {
    /// Parse a UD `VerbForm` value into a typed variant.
    pub fn parse(s: &str) -> Self {
        match s {
            "Fin" => Self::Fin,
            "Part" => Self::Part,
            "Ger" => Self::Ger,
            "Inf" => Self::Inf,
            "Sup" => Self::Sup,
            "Conv" => Self::Conv,
            "Vnoun" => Self::Vnoun,
            other => Self::Other(other.to_string()),
        }
    }

    /// Serialize this typed value back to its UD string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Fin => "Fin",
            Self::Part => "Part",
            Self::Ger => "Ger",
            Self::Inf => "Inf",
            Self::Sup => "Sup",
            Self::Conv => "Conv",
            Self::Vnoun => "Vnoun",
            Self::Other(s) => s.as_str(),
        }
    }
}

/// Whether a feat string contains an exact `Key=Value` pair.
pub fn has_key_value(feats: Option<&str>, key: &str, value: &str) -> bool {
    let Some(s) = feats else {
        return false;
    };
    let target = format!("{key}={value}");
    s.split('|').any(|pair| pair == target)
}

/// Whether a feat string declares a finite verb form (`VerbForm=Fin`).
pub fn has_verb_form_fin(feats: Option<&str>) -> bool {
    feats.is_some_and(|s| s.contains("VerbForm=Fin"))
}

/// UD IDs can be single integers (`1`), ranges (`1-2`) for MWTs, or decimals
/// (`1.1`) for empty nodes.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(untagged)]
pub enum UdId {
    /// Regular word index.
    Single(usize),
    /// Multi-word token range.
    Range(usize, usize),
    /// Empty-node index.
    Decimal(f64),
}

/// Wrapper for UD fields that may contain either a semantic value or raw
/// punctuation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(untagged)]
pub enum UdPunctable<T> {
    /// A semantic value.
    Value(T),
    /// A punctuation token with no semantic category.
    Punct(String),
}

/// Typed UD token record used by the morphosyntax mapping layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct UdWord {
    /// Word index within the sentence.
    pub id: UdId,
    /// Surface form.
    pub text: String,
    /// Lemma or stem.
    pub lemma: String,
    /// Universal part-of-speech tag.
    pub upos: UdPunctable<UniversalPos>,
    /// Language-specific part-of-speech tag, if present.
    pub xpos: Option<String>,
    /// UD feature bundle, if present.
    pub feats: Option<String>,
    /// Head token index, with `0` meaning root.
    pub head: usize,
    /// Universal dependency relation to the head.
    pub deprel: String,
    /// Enhanced dependency information, if present.
    pub deps: Option<String>,
    /// Miscellaneous annotation, if present.
    pub misc: Option<String>,
}

impl UdWord {
    /// Typed view over this word's dependency relation.
    pub fn dep_rel(&self) -> DepRel {
        DepRel::parse(&self.deprel)
    }

    /// Whether this word carries a finite-verb marker (`VerbForm=Fin`).
    pub fn has_finite_verb_form(&self) -> bool {
        has_verb_form_fin(self.feats.as_deref())
    }

    /// Build a synthetic `UdWord` for language-specific repair passes.
    pub fn synthetic(
        text: impl Into<String>,
        lemma: impl Into<String>,
        upos: UniversalPos,
        feats: Option<&str>,
        head: usize,
        deprel: impl Into<String>,
    ) -> Self {
        Self {
            id: UdId::Single(0),
            text: text.into(),
            lemma: lemma.into(),
            upos: UdPunctable::Value(upos),
            xpos: None,
            feats: feats.map(|f| f.to_string()),
            head,
            deprel: deprel.into(),
            deps: None,
            misc: None,
        }
    }
}

/// A single UD sentence: an ordered sequence of token records.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct UdSentence {
    /// Ordered token records for this sentence.
    pub words: Vec<UdWord>,
}

/// Top-level UD response for one utterance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct UdResponse {
    /// One or more UD sentences produced by the NLP engine.
    pub sentences: Vec<UdSentence>,
}

/// Apply post-parse validation and cleanup to one Stanza-produced UD word.
pub fn validate_and_clean(word: &mut UdWord) {
    if word.deprel.starts_with('<') && word.deprel.ends_with('>') {
        tracing::warn!(
            deprel = %word.deprel,
            text = %word.text,
            "Stanza emitted pad deprel — replacing with 'dep'"
        );
        word.deprel = "dep".to_string();
    }

    if !matches!(word.id, UdId::Range(_, _)) && is_bogus_lemma(&word.text, &word.lemma) {
        tracing::warn!(
            lemma = %word.lemma,
            text = %word.text,
            "Stanza returned bogus lemma — falling back to surface form"
        );
        word.lemma = word.text.clone();
    }
}

/// Detect when Stanza returns a pure-punctuation lemma for a word with letters.
pub fn is_bogus_lemma(text: &str, lemma: &str) -> bool {
    if text == lemma || lemma.is_empty() {
        return false;
    }

    let text_has_letters = text.chars().any(|c| c.is_alphabetic());
    let lemma_all_punct = lemma
        .chars()
        .all(|c| !c.is_alphanumeric() && !c.is_whitespace() && !c.is_control());

    text_has_letters && lemma_all_punct
}

/// Sanitize a string for use in a `%mor` field by replacing structural
/// separators with underscores and stripping whitespace.
pub fn sanitize_mor_text(s: &str) -> String {
    let mut result = s.replace(['|', '#', '-', '&', '$', '~'], "_");
    result.retain(|c| !c.is_whitespace());
    result
}

//! POS tag → human-readable description mapping for `%mor` hover display.
//!
//! Covers both legacy CHAT `%mor` tags (e.g. `pro:sub`, `v:aux`) and UD-style
//! tags (e.g. `NOUN`, `VERB`) because corpora in the field contain a mix of
//! both conventions.
/// Return a readable label for a `%mor` part-of-speech tag.
pub(super) fn get_pos_description(pos: &str) -> &'static str {
    match pos {
        // Basic POS
        "n" => "noun",
        "v" => "verb",
        "adj" => "adjective",
        "adv" => "adverb",
        "det" => "determiner",
        "pro" => "pronoun",
        "prep" => "preposition",
        "conj" => "conjunction",
        "aux" => "auxiliary",
        "mod" => "modal",
        "part" => "particle",
        "inf" => "infinitive marker",
        "co" => "communicator",
        "qn" => "quantifier",
        "num" => "number",
        "neg" => "negation",

        // Pronoun subtypes
        "pro:dem" => "demonstrative pronoun",
        "pro:poss" => "possessive pronoun",
        "pro:refl" => "reflexive pronoun",
        "pro:indef" => "indefinite pronoun",
        "pro:int" => "interrogative pronoun",
        "pro:rel" => "relative pronoun",
        "pro:sub" => "subject pronoun",
        "pro:obj" => "object pronoun",

        // Determiner subtypes
        "det:art" => "article",
        "det:dem" => "demonstrative determiner",
        "det:poss" => "possessive determiner",
        "det:num" => "numeral determiner",
        "det:int" => "interrogative determiner",

        // Noun subtypes
        "n:prop" => "proper noun",
        "n:gerund" => "gerund",

        // Other
        "on" => "onomatopoeia",
        "wplay" => "word play",
        "neo" => "neologism",
        "fam" => "family word",

        // UD-style tags
        "noun" => "noun",
        "verb" => "verb",
        "pron" => "pronoun",
        "propn" => "proper noun",
        "cconj" => "coordinating conjunction",
        "intj" => "interjection",
        "adp" => "adposition",
        "punct" => "punctuation",
        "ls" => "letter/sound",
        "cm" => "communicator",
        "x" => "other/unanalyzed",
        "coord" => "coordinator",

        _ => "unknown",
    }
}

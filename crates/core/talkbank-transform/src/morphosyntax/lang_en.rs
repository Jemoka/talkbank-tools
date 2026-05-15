//! English-specific morphosyntax rules.

use std::collections::HashMap;
use std::sync::LazyLock;

static IRREGULAR_VERBS: LazyLock<HashMap<&'static str, Vec<&'static str>>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("abide", vec!["abode"]);
    m.insert("arise", vec!["arose", "arisen"]);
    m.insert("awake", vec!["awoke", "awoken"]);
    m.insert("be", vec!["was", "were", "been"]);
    m.insert("bear", vec!["bore", "borne"]);
    m.insert("beat", vec!["beat", "beaten"]);
    m.insert("become", vec!["became", "become"]);
    m.insert("befall", vec!["befell", "befallen"]);
    m.insert("begin", vec!["began", "begun"]);
    m.insert("beget", vec!["begot", "begotten"]);
    m.insert("behold", vec!["beheld", "beholden"]);
    m.insert("bend", vec!["bent"]);
    m.insert("bereave", vec!["bereft"]);
    m.insert("beseek", vec!["besought"]);
    m.insert("bet", vec!["bet"]);
    m.insert("betake", vec!["betook", "betaken"]);
    m.insert("bid", vec!["bid"]);
    m.insert("bade", vec!["bidden"]);
    m.insert("bind", vec!["bound"]);
    m.insert("bite", vec!["bit", "bitten"]);
    m.insert("bleed", vec!["bled"]);
    m.insert("blow", vec!["blew", "blown"]);
    m.insert("break", vec!["broke", "broken"]);
    m.insert("breed", vec!["bred"]);
    m.insert("bring", vec!["brought"]);
    m.insert("build", vec!["built"]);
    m.insert("burn", vec!["burnt"]);
    m.insert("burst", vec!["burst"]);
    m.insert("buy", vec!["bought"]);
    m.insert("cast", vec!["cast"]);
    m.insert("catch", vec!["caught"]);
    m.insert("choose", vec!["chose", "chosen"]);
    m.insert("clad", vec!["clad"]);
    m.insert("cleave", vec!["cleft", "cloven"]);
    m.insert("cling", vec!["clung"]);
    m.insert("come", vec!["came", "come"]);
    m.insert("cost", vec!["cost"]);
    m.insert("creep", vec!["crept"]);
    m.insert("cut", vec!["cut"]);
    m.insert("deal", vec!["dealt"]);
    m.insert("dig", vec!["dug"]);
    m.insert("dive", vec!["dove", "dived"]);
    m.insert("do", vec!["did", "done"]);
    m.insert("draw", vec!["drew", "drawn"]);
    m.insert("dream", vec!["dreamt"]);
    m.insert("drink", vec!["drank", "drunk"]);
    m.insert("drive", vec!["drove", "driven"]);
    m.insert("dwell", vec!["dwelt"]);
    m.insert("eat", vec!["ate", "eaten"]);
    m.insert("fall", vec!["fell", "fallen"]);
    m.insert("feed", vec!["fed"]);
    m.insert("feel", vec!["felt"]);
    m.insert("fight", vec!["fought"]);
    m.insert("find", vec!["found"]);
    m.insert("fit", vec!["fit"]);
    m.insert("flee", vec!["fled"]);
    m.insert("fling", vec!["flung"]);
    m.insert("fly", vec!["flew", "flown"]);
    m.insert("forbid", vec!["forbade", "forbidden"]);
    m.insert("forecast", vec!["forecast"]);
    m.insert("forget", vec!["forgot", "forgotten"]);
    m.insert("forgo", vec!["forewent", "foregone"]);
    m.insert("foresee", vec!["foresaw", "foreseen"]);
    m.insert("foretell", vec!["foretold"]);
    m.insert("forgive", vec!["forgave", "forgiven"]);
    m.insert("forsake", vec!["forsook", "forsaken"]);
    m.insert("forswear", vec!["forswore", "forsworn"]);
    m.insert("freeze", vec!["froze", "frozen"]);
    m.insert("get", vec!["got", "gotten"]);
    m.insert("gild", vec!["gilt"]);
    m.insert("give", vec!["gave", "given"]);
    m.insert("go", vec!["went", "gone"]);
    m.insert("grind", vec!["ground"]);
    m.insert("grow", vec!["grew", "grown"]);
    m.insert("hang", vec!["hung"]);
    m.insert("have", vec!["had"]);
    m.insert("hear", vec!["heard"]);
    m.insert("hew", vec!["hewn"]);
    m.insert("hide", vec!["hid", "hidden"]);
    m.insert("hit", vec!["hit"]);
    m.insert("hold", vec!["held"]);
    m.insert("hurt", vec!["hurt"]);
    m.insert("inlay", vec!["inlaid"]);
    m.insert("inset", vec!["inset"]);
    m.insert("input", vec!["input"]);
    m.insert("interlay", vec!["interlaid"]);
    m.insert("interweave", vec!["interwoven"]);
    m.insert("keep", vec!["kept"]);
    m.insert("kneel", vec!["knelt"]);
    m.insert("knit", vec!["knit"]);
    m.insert("know", vec!["knew", "known"]);
    m.insert("lay", vec!["laid"]);
    m.insert("lead", vec!["led"]);
    m.insert("leap", vec!["leapt"]);
    m.insert("led", vec!["led"]);
    m.insert("leave", vec!["left"]);
    m.insert("lend", vec!["lent"]);
    m.insert("let", vec!["let"]);
    m.insert("lie", vec!["lay", "lain"]);
    m.insert("lose", vec!["lost"]);
    m.insert("make", vec!["made"]);
    m.insert("mean", vec!["meant"]);
    m.insert("meet", vec!["met"]);
    m.insert("misspeak", vec!["misspoke", "mispoken"]);
    m.insert("mistake", vec!["mistook", "mistaken"]);
    m.insert("offset", vec!["offset"]);
    m.insert("overdo", vec!["overdid", "overdone"]);
    m.insert("outbid", vec!["outbid"]);
    m.insert("pay", vec!["paid"]);
    m.insert("partake", vec!["partook", "partaken"]);
    m.insert("plead", vec!["pled"]);
    m.insert("prepay", vec!["prepaid"]);
    m.insert("prove", vec!["proven"]);
    m.insert("put", vec!["put"]);
    m.insert("quit", vec!["quit"]);
    m.insert("recast", vec!["recast"]);
    m.insert("redo", vec!["redid", "redone"]);
    m.insert("remake", vec!["remade"]);
    m.insert("reset", vec!["reset"]);
    m.insert("read", vec!["read"]);
    m.insert("rend", vec!["rent"]);
    m.insert("rid", vec!["rid", "ridden"]);
    m.insert("ride", vec!["rode", "ridden"]);
    m.insert("ring", vec!["rang", "rung"]);
    m.insert("rise", vec!["rose", "risen"]);
    m.insert("run", vec!["ran", "run"]);
    m.insert("say", vec!["said"]);
    m.insert("seek", vec!["sought"]);
    m.insert("see", vec!["saw", "seen"]);
    m.insert("sell", vec!["sold"]);
    m.insert("send", vec!["sent"]);
    m.insert("set", vec!["set"]);
    m.insert("sew", vec!["sewn"]);
    m.insert("shake", vec!["shook", "shaken"]);
    m.insert("shave", vec!["shaven"]);
    m.insert("shed", vec!["shed"]);
    m.insert("shine", vec!["shone"]);
    m.insert("shoot", vec!["shot"]);
    m.insert("show", vec!["shown"]);
    m.insert("shrink", vec!["shrank", "shrunk"]);
    m.insert("shut", vec!["shut"]);
    m.insert("sing", vec!["sang", "sung"]);
    m.insert("sink", vec!["sank", "sunk"]);
    m.insert("sit", vec!["sat"]);
    m.insert("slay", vec!["slew", "slain"]);
    m.insert("sleep", vec!["slept"]);
    m.insert("slide", vec!["slid"]);
    m.insert("slink", vec!["slunk"]);
    m.insert("slit", vec!["slit"]);
    m.insert("smite", vec!["smote", "smitten"]);
    m.insert("sneak", vec!["snuck"]);
    m.insert("speak", vec!["spoke", "spoken"]);
    m.insert("speed", vec!["sped"]);
    m.insert("spend", vec!["spent"]);
    m.insert("spin", vec!["spun"]);
    m.insert("spit", vec!["spit"]);
    m.insert("split", vec!["split"]);
    m.insert("spread", vec!["spread"]);
    m.insert("spring", vec!["sprang", "sprung"]);
    m.insert("stand", vec!["stood"]);
    m.insert("steal", vec!["stole", "stolen"]);
    m.insert("stick", vec!["stuck"]);
    m.insert("sting", vec!["stung"]);
    m.insert("stink", vec!["stank", "stunk"]);
    m.insert("strew", vec!["strewn"]);
    m.insert("strike", vec!["struck"]);
    m.insert("string", vec!["strung"]);
    m.insert("strive", vec!["strove", "striven"]);
    m.insert("swear", vec!["swore", "sworn"]);
    m.insert("sweep", vec!["swept"]);
    m.insert("swell", vec!["swollen"]);
    m.insert("swim", vec!["swam", "swum"]);
    m.insert("swing", vec!["swung"]);
    m.insert("take", vec!["took", "taken"]);
    m.insert("teach", vec!["taught"]);
    m.insert("tear", vec!["tore", "torn"]);
    m.insert("tell", vec!["told"]);
    m.insert("think", vec!["thought"]);
    m.insert("throw", vec!["threw", "thrown"]);
    m.insert("thrust", vec!["thrust"]);
    m.insert("tread", vec!["trod"]);
    m.insert("unbend", vec!["unbended", "unbent"]);
    m.insert("underlie", vec!["underlay", "underlain"]);
    m.insert("undergo", vec!["underwent", "undergone"]);
    m.insert("understand", vec!["understood"]);
    m.insert("upset", vec!["upset"]);
    m.insert("wake", vec!["woke", "woken"]);
    m.insert("waylay", vec!["waylaid"]);
    m.insert("wear", vec!["wore", "worn"]);
    m.insert("weave", vec!["wove", "woven"]);
    m.insert("wed", vec!["wed"]);
    m.insert("weep", vec!["wept"]);
    m.insert("wet", vec!["wet"]);
    m.insert("win", vec!["won"]);
    m.insert("wind", vec!["wound"]);
    m.insert("withdraw", vec!["withdrew", "withdrawn"]);
    m.insert("withhold", vec!["withheld"]);
    m.insert("withstand", vec!["withstood"]);
    m.insert("wring", vec!["wrung"]);
    m.insert("write", vec!["wrote", "written"]);
    m.insert("wreak", vec!["wrought", "wrough"]);
    m
});

/// Return `true` if `form` is a known irregular inflection of `lemma`.
pub fn is_irregular(lemma: &str, form: &str) -> bool {
    let lemma_lower = lemma.to_lowercase();
    let form_lower = form.to_lowercase();
    IRREGULAR_VERBS
        .get(lemma_lower.as_str())
        .map(|forms| forms.contains(&form_lower.as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regular_verb() {
        assert!(!is_irregular("walk", "walked"));
    }

    #[test]
    fn test_irregular_past() {
        assert!(is_irregular("go", "went"));
        assert!(is_irregular("be", "was"));
        assert!(is_irregular("have", "had"));
    }

    #[test]
    fn test_irregular_participle() {
        assert!(is_irregular("go", "gone"));
        assert!(is_irregular("write", "written"));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(is_irregular("Go", "Went"));
        assert!(is_irregular("GO", "WENT"));
    }

    #[test]
    fn test_unknown_lemma() {
        assert!(!is_irregular("xyzzy", "xyzzy"));
    }
}

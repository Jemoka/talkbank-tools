//! Japanese-specific morphosyntax rules.

/// Result of a Japanese verb form override.
pub struct JaOverride {
    /// The new POS category name.
    pub pos: &'static str,
    /// The new lemma to use.
    pub lemma: &'static str,
}

/// Apply Japanese verb-form overrides for one UD token.
pub fn japanese_verbform(upos: &str, target: &str, text: &str) -> Option<JaOverride> {
    if text.contains("ちゃ") {
        return Some(JaOverride {
            pos: "sconj",
            lemma: "ば",
        });
    }
    if text.contains("なきゃ") {
        return Some(JaOverride {
            pos: "sconj",
            lemma: "なきゃ",
        });
    }
    if text.contains("じゃ") {
        return Some(JaOverride {
            pos: "sconj",
            lemma: "ちゃ",
        });
    }
    if text.contains("れる") {
        return Some(JaOverride {
            pos: "aux",
            lemma: "られる",
        });
    }
    if text.contains("じゃう") {
        return Some(JaOverride {
            pos: "aux",
            lemma: "ちゃう",
        });
    }
    if text.contains("よう") {
        return Some(JaOverride {
            pos: "aux",
            lemma: "おう",
        });
    }
    if text.contains("だら") {
        return Some(JaOverride {
            pos: "aux",
            lemma: "たら",
        });
    }
    if target.contains("だ") {
        return Some(JaOverride {
            pos: "aux",
            lemma: "た",
        });
    }
    if target.contains("為る") && text == "さ" {
        return Some(JaOverride {
            pos: "part",
            lemma: "為る",
        });
    }
    if target.contains("無い") {
        return Some(JaOverride {
            pos: "aux",
            lemma: "ない",
        });
    }
    if target.contains("せる") {
        return Some(JaOverride {
            pos: "aux",
            lemma: "させる",
        });
    }
    if text.contains("撮る") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "撮る",
        });
    }
    if text.contains("貼る") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "貼る",
        });
    }
    if text.contains("混ぜ") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "混ぜる",
        });
    }
    if text.contains("釣る") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "釣る",
        });
    }
    if text.contains("速い") && upos == "adj" {
        return Some(JaOverride {
            pos: "adj",
            lemma: "速い",
        });
    }
    if text.contains("治ま") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "治まる",
        });
    }
    if text.contains("刺す") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "刺す",
        });
    }
    if text.contains("降り") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "降りる",
        });
    }
    if text.contains("降") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "降る",
        });
    }
    if text.contains("載せ") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "載せる",
        });
    }
    if text.contains("帰") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "帰る",
        });
    }
    if text.contains("はい") {
        return Some(JaOverride {
            pos: "intj",
            lemma: "はい",
        });
    }
    if text.contains("うん") {
        return Some(JaOverride {
            pos: "intj",
            lemma: "うん",
        });
    }
    if text.contains("おっ") {
        return Some(JaOverride {
            pos: "intj",
            lemma: "おっ",
        });
    }
    if text.contains("ほら") {
        return Some(JaOverride {
            pos: "intj",
            lemma: "ほら",
        });
    }
    if text.contains("ヤッホー") {
        return Some(JaOverride {
            pos: "intj",
            lemma: "ヤッホー",
        });
    }
    if text.contains("ただいま") {
        return Some(JaOverride {
            pos: "intj",
            lemma: "ただいま",
        });
    }
    if text.contains("あたし") {
        return Some(JaOverride {
            pos: "pron",
            lemma: "あたし",
        });
    }
    if text.contains("舐め") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "舐める",
        });
    }
    if text.contains("バツ") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "バツ",
        });
    }
    if text.contains("ブラシ") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "ブラシ",
        });
    }
    if text.contains("引き出し") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "引き出し",
        });
    }
    if text.contains("下さい") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "下さい",
        });
    }
    if target == "シャャミー" || target == "物コャミ" {
        return Some(JaOverride {
            pos: "noun",
            lemma: "クシャミ",
        });
    }
    if text.contains("マヨネーズ") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "マヨネーズ",
        });
    }
    if text.contains("マヨ") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "マヨ",
        });
    }
    if text.contains("チップス") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "チップス",
        });
    }
    if text.contains("ゴロンっ") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "ゴロンっ",
        });
    }
    if text.contains("モチーンっ") {
        return Some(JaOverride {
            pos: "noun",
            lemma: "モチーンっ",
        });
    }
    if text == "人っ" {
        return Some(JaOverride {
            pos: "noun",
            lemma: "人",
        });
    }
    if text == "掻く" {
        return Some(JaOverride {
            pos: "part",
            lemma: "かい",
        });
    }
    if text.contains("遣") && upos == "noun" {
        return Some(JaOverride {
            pos: "verb",
            lemma: "遣る",
        });
    }
    if text.contains("死") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "死ぬ",
        });
    }
    if text.contains("立") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "立つ",
        });
    }
    if text.contains("引") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "引く",
        });
    }
    if text.contains("出") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "出す",
        });
    }
    if text.contains("飲") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "飲む",
        });
    }
    if text.contains("呼") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "呼ぶ",
        });
    }
    if text.contains("脱") {
        return Some(JaOverride {
            pos: "verb",
            lemma: "脱ぐ",
        });
    }
    if text == "な" && upos == "part" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "な",
        });
    }
    if text == "呼ん" {
        return Some(JaOverride {
            pos: "verb",
            lemma: "呼ぶ",
        });
    }
    if text == "な" && upos == "aux" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "な",
        });
    }
    if text == "だり" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "たり",
        });
    }
    if text == "たり" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "たり",
        });
    }
    if text == "たら" {
        return Some(JaOverride {
            pos: "sconj",
            lemma: "たら",
        });
    }
    if text == "たっ" {
        return Some(JaOverride {
            pos: "sconj",
            lemma: "たっ",
        });
    }
    if text == "なさい" && target == "為さる" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "為さい",
        });
    }
    if target == "ちゃ" {
        return Some(JaOverride {
            pos: "sconj",
            lemma: "ちゃ",
        });
    }
    if target == "ない" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "ない",
        });
    }
    if text == "な" && upos == "part" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "な",
        });
    }
    if text == "脱" && upos == "noun" {
        return Some(JaOverride {
            pos: "verb",
            lemma: "脱",
        });
    }
    if text == "よう" && upos == "aux" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "よう",
        });
    }
    if text == "ろ" && upos == "aux" && target == "為る" {
        return Some(JaOverride {
            pos: "aux",
            lemma: "ろ",
        });
    }
    if text == "で" {
        return Some(JaOverride {
            pos: "sconj",
            lemma: "で",
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sconj_override() {
        let r = japanese_verbform("verb", "食べる", "食べちゃう").unwrap();
        assert_eq!(r.pos, "sconj");
        assert_eq!(r.lemma, "ば");
    }

    #[test]
    fn test_intj_override() {
        let r = japanese_verbform("noun", "はい", "はい").unwrap();
        assert_eq!(r.pos, "intj");
        assert_eq!(r.lemma, "はい");
    }

    #[test]
    fn test_no_override() {
        assert!(japanese_verbform("noun", "犬", "犬").is_none());
    }

    #[test]
    fn test_de_override() {
        let r = japanese_verbform("sconj", "で", "で").unwrap();
        assert_eq!(r.pos, "sconj");
        assert_eq!(r.lemma, "で");
    }
}

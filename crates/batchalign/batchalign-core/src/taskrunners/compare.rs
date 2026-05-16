//! `CompareTaskRunner` — gold-vs-main transcript comparison (spec2.md §19).
//!
//! Pure-AST runner. Consumes `BAValue::Paired { main, gold }`, walks the gold
//! utterances against the main transcript, and annotates `main` with:
//!
//! - `%xref:` per-utterance — gold-projected token list with per-token status
//!   suffix (`/match`, `/sub`, `/ins`, `/del`).
//! - `%xcmp:` per-utterance — numeric metrics (`wer=… ed=… gold=… main=…
//!   match=… sub=… ins=… del=…`).
//! - `@Comment: ba.compare.summary: <json>` — corpus totals.
//!
//! Converts the variant to `BAValue::Chat(main)` on success.
//!
//! ## Algorithm (ported from `batchalign2/batchalign/pipelines/analysis/compare.py`)
//!
//! 1. Token extraction per utterance via `walk_words` (skips fillers).
//! 2. `conform()` normalization: contractions + common informal contractions.
//!    PORTED: `'s`→`is`, `'ve`→`have`, `'d`→`had`, `'m`→`am`,
//!    `gonna`/`wanna`/`gotta`/`kinda`/`sorta`/`gimme`/`dunno`/`hafta`/`havta`,
//!    `ok`→`okay`, `til`→`until`, `mm`/`hmm`→`hm`, dash/underscore splitting.
//!    DEFERRED: BA2's `compounds`, `abbrev`, `names` lexicons.
//! 3. Per gold utterance: find length-±2 bag-of-words window in remaining main
//!    tokens (`find_best_segment`), then run a Levenshtein DP aligner with
//!    traceback (`levenshtein_align`) inside that window.
//! 4. Aggregate per-utt and total metrics; serialize to `%xref:`/`%xcmp:` and
//!    a `@Comment:` summary header via `ast_mut()`.

use crate::base::{Chat, Validated};
use crate::utils::{BAError, BAResult};
use crate::base::Paired;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::{Dispatcher, TaskRunner};
use crate::base::{BAValue};
use crate::utils::SourceId;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::mem;

use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::{
    BulletContent, ChatFile, DependentTier, Header, Line, NonEmptyString, Span,
    UserDefinedDependentTier, Utterance,
};

// ---------------------------------------------------------------------------
// Config + runner
// ---------------------------------------------------------------------------

/// Per-task config.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CompareConfig {
    /// Emit `%xref:` and `%xcmp:` tiers per utterance.
    pub emit_tiers: bool,
    /// Emit `@Comment: ba.compare.summary: <json>` header.
    pub emit_summary_header: bool,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            emit_tiers: true,
            emit_summary_header: true,
        }
    }
}

/// Pure-AST runner: aligns main vs gold and annotates main.
pub struct CompareTaskRunner;

#[async_trait]
impl TaskRunner for CompareTaskRunner {
    const TASK: Task = Task::Compare;
    type Config = CompareConfig;

    async fn apply(
        &self,
        cfg: &Self::Config,
        value: &mut BAValue,
        _dispatcher: &dyn Dispatcher,
        _sink: &dyn ProgressSink,
    ) -> BAResult<()> {
        let sid_for_placeholder = value.source_id();
        let placeholder = BAValue::Failed {
            source_id: sid_for_placeholder.clone(),
            error: BAError::Internal("compare: in-flight placeholder".into()),
            partial: None,
        };
        let taken = mem::replace(value, placeholder);
        let paired = match taken {
            BAValue::Paired(p) => p,
            other => {
                let kind = other.kind();
                *value = other;
                return Err(BAError::Internal(format!(
                    "CompareTaskRunner expected BAValue::Paired, got {kind}"
                )));
            }
        };

        let new_chat = run_compare(paired, cfg, &sid_for_placeholder)?;
        *value = BAValue::Chat(new_chat);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Core algorithm
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SrcWord {
    utt_idx: usize,
    text: String,
}

#[derive(Debug, Clone)]
struct ConformedTok {
    text: String,
    src_idx: usize,
}

fn run_compare(
    paired: Paired,
    cfg: &CompareConfig,
    _source_id: &SourceId,
) -> BAResult<Chat<Validated>> {
    let (mut main_chat, gold_chat) = paired.into_parts();

    let main_words = extract_words(main_chat.ast());
    let gold_words = extract_words(gold_chat.ast());

    let main_conformed = conform_with_mapping(&main_words);
    let gold_conformed = conform_with_mapping(&gold_words);

    let gold_utt_count = gold_chat.ast().utterances().count();
    let mut gold_by_utt: Vec<Vec<ConformedTok>> = vec![Vec::new(); gold_utt_count];
    for tok in &gold_conformed {
        let utt = gold_words[tok.src_idx].utt_idx;
        if utt < gold_by_utt.len() {
            gold_by_utt[utt].push(tok.clone());
        }
    }

    let mut per_utt: Vec<UttCmp> = Vec::with_capacity(gold_utt_count);
    let mut search_start = 0usize;
    for g_tokens in &gold_by_utt {
        if g_tokens.is_empty() {
            per_utt.push(UttCmp::default());
            continue;
        }
        let g_text: Vec<&str> = g_tokens.iter().map(|t| t.text.as_str()).collect();
        let remaining: Vec<&str> = main_conformed[search_start..]
            .iter()
            .map(|t| t.text.as_str())
            .collect();
        let (win_start, win_end) = find_best_segment(&g_text, &remaining);
        let abs_start = search_start + win_start;
        let abs_end = search_start + win_end;
        let window_main: Vec<&str> = main_conformed[abs_start..abs_end]
            .iter()
            .map(|t| t.text.as_str())
            .collect();
        let alignment = levenshtein_align(&window_main, &g_text);
        let cmp = build_utt_cmp(&alignment, &window_main, &g_text);
        per_utt.push(cmp);
        search_start = abs_end;
    }

    let summary = summarize(&per_utt);

    if cfg.emit_tiers {
        inject_per_utt_tiers(main_chat.ast_mut(), &per_utt, gold_utt_count)?;
    }
    if cfg.emit_summary_header {
        inject_summary_header(main_chat.ast_mut(), &summary)?;
    }

    Ok(main_chat)
}

fn extract_words(ast: &ChatFile<talkbank_model::validation::Validated>) -> Vec<SrcWord> {
    let mut out: Vec<SrcWord> = Vec::new();
    for (utt_idx, utt) in ast.utterances().enumerate() {
        let mut visit = |item: WordItem<'_>| {
            let txt: String = match item {
                WordItem::Word(w) => w.cleaned_text().to_owned(),
                WordItem::ReplacedWord(rw) => {
                    let r = rw
                        .replacement
                        .words
                        .first()
                        .map(|w| w.cleaned_text().to_owned())
                        .unwrap_or_default();
                    if r.is_empty() {
                        rw.word.cleaned_text().to_owned()
                    } else {
                        r
                    }
                }
                WordItem::Separator(_) => return,
            };
            let trimmed = txt.trim();
            if trimmed.is_empty() {
                return;
            }
            if is_filler(trimmed) {
                return;
            }
            out.push(SrcWord {
                utt_idx,
                text: trimmed.to_owned(),
            });
        };
        walk_words(&utt.main.content.content.0, None, &mut visit);
    }
    out
}

fn is_filler(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "um" | "uhm" | "em" | "mhm" | "uhhm" | "eh" | "uh" | "hm"
    )
}

// ---------------------------------------------------------------------------
// conform()
// ---------------------------------------------------------------------------

fn conform_one(word: &str) -> Vec<String> {
    let lower = word.trim().to_lowercase();
    if lower.is_empty() {
        return Vec::new();
    }

    let apos_variants: &[(&str, &str)] = &[
        ("'s", "is"),
        ("\u{2019}s", "is"),
        ("'ve", "have"),
        ("\u{2019}ve", "have"),
        ("'d", "had"),
        ("\u{2019}d", "had"),
        ("'m", "am"),
        ("\u{2019}m", "am"),
    ];
    for (suffix, expansion) in apos_variants {
        if let Some(stem) = lower.strip_suffix(suffix) {
            if !stem.is_empty() {
                return vec![stem.to_owned(), (*expansion).to_owned()];
            }
        }
    }

    match lower.as_str() {
        "ok" => return vec!["okay".to_owned()],
        "gimme" => return vec!["give".to_owned(), "me".to_owned()],
        "hafta" | "havta" => return vec!["have".to_owned(), "to".to_owned()],
        "dunno" => return vec!["don't".to_owned(), "know".to_owned()],
        "wanna" => return vec!["want".to_owned(), "to".to_owned()],
        "gonna" => return vec!["going".to_owned(), "to".to_owned()],
        "gotta" => return vec!["got".to_owned(), "to".to_owned()],
        "kinda" => return vec!["kind".to_owned(), "of".to_owned()],
        "sorta" => return vec!["sort".to_owned(), "of".to_owned()],
        "alright" | "alrightie" => {
            return vec!["all".to_owned(), "right".to_owned()];
        }
        "til" => return vec!["until".to_owned()],
        "mm" | "hmm" => return vec!["hm".to_owned()],
        _ => {}
    }

    if lower.contains('-') {
        return lower
            .split('-')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
    }
    if lower.contains('_') {
        return lower
            .split('_')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect();
    }

    vec![lower]
}

fn conform_with_mapping(words: &[SrcWord]) -> Vec<ConformedTok> {
    let mut out = Vec::new();
    for (idx, w) in words.iter().enumerate() {
        for tok in conform_one(&w.text) {
            out.push(ConformedTok {
                text: tok,
                src_idx: idx,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// find_best_segment — bag-of-words windowed match (port from BA2)
// ---------------------------------------------------------------------------

fn find_best_segment(gold: &[&str], main: &[&str]) -> (usize, usize) {
    use std::collections::HashMap;
    if gold.is_empty() || main.is_empty() {
        return (0, 0);
    }
    let gold_len = gold.len();
    let main_len = main.len();

    let mut gold_counts: HashMap<&str, i32> = HashMap::new();
    for t in gold {
        *gold_counts.entry(*t).or_insert(0) += 1;
    }

    let min_window = gold_len.saturating_sub(2).max(1);
    let max_window = (gold_len + 2).min(main_len);

    let mut best: (usize, usize) = (0, gold_len.min(main_len));
    let mut best_score: f64 = -1.0;
    let mut best_len_delta: Option<usize> = None;

    let mut span = min_window;
    while span <= max_window {
        if span > main_len {
            break;
        }
        let mut window_counts: HashMap<&str, i32> = HashMap::new();
        for t in &main[..span] {
            *window_counts.entry(*t).or_insert(0) += 1;
        }
        let mut overlap: i32 = window_counts
            .iter()
            .map(|(k, v)| (*v).min(*gold_counts.get(k).unwrap_or(&0)))
            .sum();

        let max_start = main_len - span;
        let mut start = 0usize;
        loop {
            if start > 0 {
                let left = main[start - 1];
                let right = main[start + span - 1];
                let wl = *window_counts.get(left).unwrap_or(&0);
                let gl = *gold_counts.get(left).unwrap_or(&0);
                overlap -= wl.min(gl);
                let new_wl = wl - 1;
                if new_wl == 0 {
                    window_counts.remove(left);
                } else {
                    window_counts.insert(left, new_wl);
                }
                overlap += new_wl.min(gl);

                let wr = *window_counts.get(right).unwrap_or(&0);
                let gr = *gold_counts.get(right).unwrap_or(&0);
                overlap -= wr.min(gr);
                let new_wr = wr + 1;
                window_counts.insert(right, new_wr);
                overlap += new_wr.min(gr);
            }
            let score = overlap as f64 / gold_len as f64;
            let len_delta = span.abs_diff(gold_len);
            let end = start + span;
            if score > best_score {
                best = (start, end);
                best_score = score;
                best_len_delta = Some(len_delta);
            } else if (score - best_score).abs() < f64::EPSILON {
                match best_len_delta {
                    None => {
                        best = (start, end);
                        best_len_delta = Some(len_delta);
                    }
                    Some(prev) if len_delta < prev => {
                        best = (start, end);
                        best_len_delta = Some(len_delta);
                    }
                    Some(prev) if len_delta == prev && end > best.1 => {
                        best = (start, end);
                    }
                    _ => {}
                }
            }
            if start == max_start {
                break;
            }
            start += 1;
        }
        span += 1;
    }

    if best_score <= 0.0 {
        return (0, 0);
    }
    best
}

// ---------------------------------------------------------------------------
// Levenshtein alignment with traceback
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlignItem {
    Match { i: usize, j: usize },
    Sub { i: usize, j: usize },
    Insert { i: usize },
    Delete { j: usize },
}

fn levenshtein_align(main: &[&str], gold: &[&str]) -> Vec<AlignItem> {
    let n = main.len();
    let m = gold.len();
    if n == 0 && m == 0 {
        return Vec::new();
    }
    if n == 0 {
        return (0..m).map(|j| AlignItem::Delete { j }).collect();
    }
    if m == 0 {
        return (0..n).map(|i| AlignItem::Insert { i }).collect();
    }
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in 0..=n {
        dp[i][0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost_sub = usize::from(main[i - 1] != gold[j - 1]);
            let sub = dp[i - 1][j - 1] + cost_sub;
            let del = dp[i][j - 1] + 1; // gold consumed -> Delete
            let ins = dp[i - 1][j] + 1; // main consumed -> Insert
            dp[i][j] = sub.min(del).min(ins);
        }
    }
    let mut out: Vec<AlignItem> = Vec::with_capacity(n + m);
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 {
            let cost_sub = usize::from(main[i - 1] != gold[j - 1]);
            if dp[i][j] == dp[i - 1][j - 1] + cost_sub {
                let ii = i - 1;
                let jj = j - 1;
                if cost_sub == 0 {
                    out.push(AlignItem::Match { i: ii, j: jj });
                } else {
                    out.push(AlignItem::Sub { i: ii, j: jj });
                }
                i -= 1;
                j -= 1;
                continue;
            }
        }
        if j > 0 && (i == 0 || dp[i][j] == dp[i][j - 1] + 1) {
            out.push(AlignItem::Delete { j: j - 1 });
            j -= 1;
            continue;
        }
        out.push(AlignItem::Insert { i: i - 1 });
        i -= 1;
    }
    out.reverse();
    out
}

// ---------------------------------------------------------------------------
// Per-utterance result + summary
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct UttCmp {
    tokens: Vec<(String, &'static str)>,
    matches: u32,
    subs: u32,
    inserts: u32,
    deletes: u32,
}

impl UttCmp {
    fn total_gold(&self) -> u32 {
        self.matches + self.subs + self.deletes
    }
    fn total_main(&self) -> u32 {
        self.matches + self.subs + self.inserts
    }
    fn edit_distance(&self) -> u32 {
        self.subs + self.inserts + self.deletes
    }
    fn wer(&self) -> f64 {
        let g = self.total_gold();
        if g == 0 {
            0.0
        } else {
            self.edit_distance() as f64 / g as f64
        }
    }
}

fn build_utt_cmp(items: &[AlignItem], main: &[&str], gold: &[&str]) -> UttCmp {
    let mut out = UttCmp::default();
    for it in items {
        match *it {
            AlignItem::Match { i, j } => {
                out.tokens.push((gold[j].to_owned(), "match"));
                out.matches += 1;
                let _ = i;
            }
            AlignItem::Sub { i, j } => {
                out.tokens.push((gold[j].to_owned(), "sub"));
                out.subs += 1;
                let _ = i;
            }
            AlignItem::Insert { i } => {
                out.tokens.push((main[i].to_owned(), "ins"));
                out.inserts += 1;
            }
            AlignItem::Delete { j } => {
                out.tokens.push((gold[j].to_owned(), "del"));
                out.deletes += 1;
            }
        }
    }
    out
}

#[derive(Debug, Default)]
struct Summary {
    matches: u32,
    substitutions: u32,
    insertions: u32,
    deletions: u32,
    total_gold_words: u32,
    total_main_words: u32,
    wer: f64,
    accuracy: f64,
}

fn summarize(per: &[UttCmp]) -> Summary {
    let mut s = Summary::default();
    for u in per {
        s.matches += u.matches;
        s.substitutions += u.subs;
        s.insertions += u.inserts;
        s.deletions += u.deletes;
    }
    s.total_gold_words = s.matches + s.substitutions + s.deletions;
    s.total_main_words = s.matches + s.substitutions + s.insertions;
    s.wer = if s.total_gold_words == 0 {
        0.0
    } else {
        (s.substitutions + s.insertions + s.deletions) as f64 / s.total_gold_words as f64
    };
    s.accuracy = 1.0 - s.wer;
    s
}

fn summary_json(s: &Summary) -> String {
    format!(
        "{{\"wer\":{:.4},\"accuracy\":{:.4},\"matches\":{},\"substitutions\":{},\"insertions\":{},\"deletions\":{},\"total_gold_words\":{},\"total_main_words\":{}}}",
        s.wer,
        s.accuracy,
        s.matches,
        s.substitutions,
        s.insertions,
        s.deletions,
        s.total_gold_words,
        s.total_main_words
    )
}

// ---------------------------------------------------------------------------
// AST injection
// ---------------------------------------------------------------------------

fn inject_per_utt_tiers(
    ast: &mut ChatFile<talkbank_model::validation::Validated>,
    per_utt: &[UttCmp],
    _gold_utt_count: usize,
) -> BAResult<()> {
    let mut idx = 0usize;
    for line in ast.lines.iter_mut() {
        if let Line::Utterance(u) = line {
            if let Some(cmp) = per_utt.get(idx) {
                if !cmp.tokens.is_empty() {
                    push_xref(u, cmp)?;
                    push_xcmp(u, cmp)?;
                }
            }
            idx += 1;
        }
    }
    Ok(())
}

fn push_xref(u: &mut Utterance, cmp: &UttCmp) -> BAResult<()> {
    let payload = serialize_xref_content(cmp);
    let label = ne("xref")?;
    let content = ne(&payload)?;
    u.dependent_tiers
        .push(DependentTier::UserDefined(UserDefinedDependentTier {
            label,
            content,
            span: Span::DUMMY,
        }));
    Ok(())
}

fn push_xcmp(u: &mut Utterance, cmp: &UttCmp) -> BAResult<()> {
    let payload = serialize_xcmp_content(cmp);
    let label = ne("xcmp")?;
    let content = ne(&payload)?;
    u.dependent_tiers
        .push(DependentTier::UserDefined(UserDefinedDependentTier {
            label,
            content,
            span: Span::DUMMY,
        }));
    Ok(())
}

fn ne(s: &str) -> BAResult<NonEmptyString> {
    NonEmptyString::new(s).ok_or_else(|| {
        BAError::Internal(format!(
            "compare: refusing to construct empty NonEmptyString for {s:?}"
        ))
    })
}

fn serialize_xref_content(cmp: &UttCmp) -> String {
    let mut s = String::new();
    let mut first = true;
    for (tok, status) in &cmp.tokens {
        if !first {
            s.push(' ');
        }
        first = false;
        let safe: String = tok
            .chars()
            .map(|c| if c.is_whitespace() { '_' } else { c })
            .collect();
        s.push_str(&safe);
        s.push('/');
        s.push_str(status);
    }
    if cmp.tokens.is_empty() {
        s.push('0');
    }
    s
}

fn serialize_xcmp_content(cmp: &UttCmp) -> String {
    format!(
        "wer={:.4} ed={} gold={} main={} match={} sub={} ins={} del={}",
        cmp.wer(),
        cmp.edit_distance(),
        cmp.total_gold(),
        cmp.total_main(),
        cmp.matches,
        cmp.subs,
        cmp.inserts,
        cmp.deletes
    )
}

fn inject_summary_header(
    ast: &mut ChatFile<talkbank_model::validation::Validated>,
    s: &Summary,
) -> BAResult<()> {
    let body = format!("ba.compare.summary: {}", summary_json(s));
    let header = Header::Comment {
        content: BulletContent::from_text(body),
    };
    let insert_at = ast
        .lines
        .iter()
        .position(|l| matches!(l, Line::Utterance(_)))
        .unwrap_or(ast.lines.len());
    ast.lines
        .insert(insert_at, Line::header_with_span(header, Span::DUMMY));
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::NullSink;
    use crate::base::Dispatcher as DispatcherTrait;
    use crate::base::{TaskInput, TaskOutput};

    struct NoDispatcher;
    #[async_trait]
    impl DispatcherTrait for NoDispatcher {
        async fn dispatch(&self, _input: TaskInput) -> BAResult<TaskOutput> {
            Err(BAError::Internal("compare needs no dispatcher".into()))
        }
    }

    const MAIN_CHA: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello there friend .\n*CHI:\thow are you .\n@End\n";
    const GOLD_CHA: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello there .\n*CHI:\thow are you doing .\n@End\n";

    #[test]
    fn find_best_segment_basic() {
        let gold = vec!["the", "cat", "sat"];
        let main = vec!["um", "the", "cat", "sat", "down"];
        let (s, e) = find_best_segment(&gold, &main);
        assert!(e > s, "non-empty window expected");
        let win: Vec<&str> = main[s..e].to_vec();
        assert!(win.contains(&"cat") || win.contains(&"sat"));
    }

    #[test]
    fn conform_contractions() {
        assert_eq!(
            conform_one("he's"),
            vec!["he".to_string(), "is".to_string()]
        );
        assert_eq!(
            conform_one("gonna"),
            vec!["going".to_string(), "to".to_string()]
        );
        assert_eq!(conform_one("OK"), vec!["okay".to_string()]);
    }

    #[test]
    fn levenshtein_simple() {
        let a = vec!["a", "b", "c"];
        let b = vec!["a", "x", "c"];
        let r = levenshtein_align(&a, &b);
        assert_eq!(r.len(), 3);
        assert!(matches!(r[0], AlignItem::Match { .. }));
        assert!(matches!(r[1], AlignItem::Sub { .. }));
        assert!(matches!(r[2], AlignItem::Match { .. }));
    }

    #[test]
    fn compare_smoke() {
        let sid = SourceId::try_new("test").expect("sid");
        let main = Chat::parse(MAIN_CHA, sid.clone()).expect("main parse");
        let gold_sid = SourceId::try_new("gold").expect("gsid");
        let gold = Chat::parse(GOLD_CHA, gold_sid).expect("gold parse");
        let paired = Paired::new(main, gold);
        let mut value = BAValue::Paired(paired);

        let runner = CompareTaskRunner;
        let cfg = CompareConfig::default();
        let disp = NoDispatcher;
        let sink = NullSink;

        let res = futures::executor::block_on(runner.apply(&cfg, &mut value, &disp, &sink));
        assert!(res.is_ok(), "compare.apply failed: {:?}", res.err());
        let out = match value {
            BAValue::Chat(c) => c.to_chat(),
            other => panic!("expected Chat, got {}", other.kind()),
        };
        assert!(out.contains("%xref:"), "no %xref in output:\n{out}");
        assert!(out.contains("%xcmp:"), "no %xcmp in output:\n{out}");
        assert!(
            out.contains("ba.compare.summary:"),
            "no summary header in output:\n{out}"
        );
    }
}

// TODO(spec2.md §19/§20 follow-up):
// - Port BA2's `compounds`, `abbrev`, `names` lexicons for bug-for-bug
//   conform() parity. Today's subset is robust enough for general WER but
//   diverges on lexicon-driven edge cases.
// - Emit `MetricsArtifact` (long-format DataFrame: source_id, utt_idx,
//   wer, ed, match, sub, ins, del) per spec §20.
// - Optionally project gold POS into `%xref:` status suffixes (BA2 ports a
//   `pos` field on each CompareToken). Requires Stanza/morphosyntax which
//   the compare runner currently does not require.

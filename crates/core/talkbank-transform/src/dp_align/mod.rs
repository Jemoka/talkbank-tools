//! Hirschberg divide-and-conquer sequence alignment.
//!
//! Implements the same edit-distance semantics as Python's `batchalign/utils/dp.py`:
//! - match cost = 0
//! - substitution cost = 2
//! - insertion/deletion cost = 1
//!
//! Uses linear space via Hirschberg's algorithm. Both `String` and `char`
//! element types are supported through the [`Alignable`] trait, which
//! eliminates code duplication between the two entry points.

/// Cost of a substitution (mismatch).
const COST_SUB: usize = 2;
/// Cost of an insertion or deletion (gap).
const COST_GAP: usize = 1;

/// Threshold below which we use the full-table alignment (avoids recursion overhead).
const SMALL_CUTOFF: usize = 2048;

/// Result of aligning two sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignResult {
    /// Payload and reference matched at the given indices.
    Match {
        /// Matched string key.
        key: String,
        /// Index into the payload sequence.
        payload_idx: usize,
        /// Index into the reference sequence.
        reference_idx: usize,
    },
    /// Extra element in the payload (insertion).
    ExtraPayload {
        /// Unmatched string key from the payload.
        key: String,
        /// Index into the payload sequence.
        payload_idx: usize,
    },
    /// Extra element in the reference (deletion).
    ExtraReference {
        /// Unmatched string key from the reference.
        key: String,
        /// Index into the reference sequence.
        reference_idx: usize,
    },
}

/// Controls how string keys are compared during sequence alignment.
///
/// The choice of match mode affects both alignment accuracy and which
/// pairs the aligner considers equivalent. Use `Exact` when the source
/// texts are already normalized (e.g., aligning two CHAT tiers from the
/// same parse). Use `CaseInsensitive` when comparing across sources that
/// may differ in capitalization (e.g., aligning ASR output against a
/// reference transcript).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MatchMode {
    /// Byte-for-byte string equality (`a == b`).
    ///
    /// Appropriate when both sequences originate from the same
    /// normalization pipeline and casing carries semantic weight
    /// (e.g., proper nouns vs. common nouns). Faster than
    /// case-insensitive comparison because it avoids per-character
    /// lowercasing.
    Exact,

    /// ASCII case-insensitive equality (`eq_ignore_ascii_case`).
    ///
    /// Appropriate when casing is unreliable -- for example, aligning
    /// ASR hypotheses (often all-lowercase) against gold transcripts
    /// (mixed case). Note this only folds ASCII letters (A-Z); Unicode
    /// case variants (e.g., accented characters) are compared as-is.
    CaseInsensitive,

    /// Fuzzy matching using Jaro-Winkler similarity.
    ///
    /// Accepts a match when `jaro_winkler(a.lower(), b.lower()) >= threshold`.
    /// Jaro-Winkler is preferred over Levenshtein for short words (backchannels
    /// like "mhm", "yeah", "uh huh") because it weights prefix matches more
    /// heavily and doesn't penalize length differences as harshly.
    ///
    /// Typical thresholds:
    /// - 0.90: strict — allows minor typos ("gonna" ≈ "gona")
    /// - 0.85: moderate — allows ASR normalizations ("going" ≈ "goin")
    /// - 0.80: lenient — allows dialectal variants ("yes" ≈ "yeah" is ~0.78,
    ///   so 0.80 would NOT match this)
    ///
    /// Always tries exact case-insensitive match first (fast path).
    Fuzzy {
        /// Minimum Jaro-Winkler similarity to accept (0.0–1.0).
        threshold: f64,
    },
}

// ---------------------------------------------------------------------------
// Alignable trait — unifies String and char element types
// ---------------------------------------------------------------------------

/// Element type that can participate in Hirschberg alignment.
///
/// Implemented for `String` (word-level) and `char` (character-level).
/// Monomorphization ensures zero overhead compared to the previous
/// copy-pasted implementations.
trait Alignable {
    fn matches(&self, other: &Self, mode: MatchMode) -> bool;
    fn to_key(&self) -> String;
}

impl Alignable for String {
    fn matches(&self, other: &Self, mode: MatchMode) -> bool {
        match mode {
            MatchMode::Exact => self == other,
            MatchMode::CaseInsensitive => self.eq_ignore_ascii_case(other),
            MatchMode::Fuzzy { threshold } => {
                // Fast path: exact case-insensitive match
                if self.eq_ignore_ascii_case(other) {
                    return true;
                }
                // Fuzzy: Jaro-Winkler on lowercased strings
                let sim = strsim::jaro_winkler(&self.to_lowercase(), &other.to_lowercase());
                sim >= threshold
            }
        }
    }

    fn to_key(&self) -> String {
        self.clone()
    }
}

impl Alignable for char {
    fn matches(&self, other: &Self, mode: MatchMode) -> bool {
        match mode {
            MatchMode::Exact => self == other,
            // Fuzzy at char level degrades to case-insensitive (single chars
            // can't meaningfully fuzzy-match).
            MatchMode::CaseInsensitive | MatchMode::Fuzzy { .. } => {
                self.eq_ignore_ascii_case(other)
            }
        }
    }

    fn to_key(&self) -> String {
        self.to_string()
    }
}

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Align two string sequences using the Hirschberg algorithm.
///
/// Strips matching prefixes and suffixes in O(n) before entering the
/// O(mn) DP core, which dramatically reduces the effective problem size
/// when the sequences are mostly identical (the common case for WER and
/// transcript comparison).
///
/// Returns a list of `AlignResult` items in sequence order.
pub fn align(payload: &[String], reference: &[String], mode: MatchMode) -> Vec<AlignResult> {
    align_generic(payload, reference, mode)
}

/// Align two character sequences using the Hirschberg algorithm.
///
/// Like [`align`] but accepts `&[char]` instead of `&[String]`, avoiding the
/// per-character `String` allocation that retokenization would otherwise need.
/// Returns `AlignResult` with single-character `key` strings.
///
/// Applies the same prefix/suffix stripping optimization as [`align`].
pub fn align_chars(payload: &[char], reference: &[char], mode: MatchMode) -> Vec<AlignResult> {
    align_generic(payload, reference, mode)
}

// ---------------------------------------------------------------------------
// Generic Hirschberg implementation
// ---------------------------------------------------------------------------

/// Prefix/suffix stripping + Hirschberg dispatch for any [`Alignable`] type.
fn align_generic<T: Alignable>(
    payload: &[T],
    reference: &[T],
    mode: MatchMode,
) -> Vec<AlignResult> {
    // Strip common prefix
    let prefix_len = payload
        .iter()
        .zip(reference.iter())
        .take_while(|(p, r)| p.matches(r, mode))
        .count();

    // Strip common suffix (after prefix)
    let suffix_len = payload[prefix_len..]
        .iter()
        .rev()
        .zip(reference[prefix_len..].iter().rev())
        .take_while(|(p, r)| p.matches(r, mode))
        .count();

    // Build prefix matches
    let mut result = Vec::with_capacity(payload.len().max(reference.len()));
    for (i, ref_item) in reference.iter().enumerate().take(prefix_len) {
        result.push(AlignResult::Match {
            key: ref_item.to_key(),
            payload_idx: i,
            reference_idx: i,
        });
    }

    // DP on the middle part only
    let mid_pay = &payload[prefix_len..payload.len() - suffix_len];
    let mid_ref = &reference[prefix_len..reference.len() - suffix_len];
    result.extend(hirschberg(mid_pay, mid_ref, prefix_len, prefix_len, mode));

    // Build suffix matches
    let pay_suffix_start = payload.len() - suffix_len;
    let ref_suffix_start = reference.len() - suffix_len;
    for i in 0..suffix_len {
        result.push(AlignResult::Match {
            key: reference[ref_suffix_start + i].to_key(),
            payload_idx: pay_suffix_start + i,
            reference_idx: ref_suffix_start + i,
        });
    }

    result
}

/// Hirschberg's divide-and-conquer alignment (linear space).
fn hirschberg<T: Alignable>(
    payload: &[T],
    reference: &[T],
    pay_offset: usize,
    ref_offset: usize,
    mode: MatchMode,
) -> Vec<AlignResult> {
    if reference.is_empty() {
        return payload
            .iter()
            .enumerate()
            .map(|(i, k)| AlignResult::ExtraPayload {
                key: k.to_key(),
                payload_idx: pay_offset + i,
            })
            .collect();
    }
    if payload.is_empty() {
        return reference
            .iter()
            .enumerate()
            .map(|(i, k)| AlignResult::ExtraReference {
                key: k.to_key(),
                reference_idx: ref_offset + i,
            })
            .collect();
    }

    if reference.len() <= 1 || payload.len() <= 1 || reference.len() * payload.len() <= SMALL_CUTOFF
    {
        return align_small(payload, reference, pay_offset, ref_offset, mode);
    }

    let mid = reference.len() / 2;
    let left_ref = &reference[..mid];
    let right_ref = &reference[mid..];

    let score_left = row_costs(left_ref, payload, mode);
    let score_right = row_costs_rev(right_ref, payload, mode);

    let pay_len = payload.len();
    let mut split = 0;
    let mut best = usize::MAX;
    for k in 0..=pay_len {
        let cost = score_left[k] + score_right[pay_len - k];
        if cost < best {
            best = cost;
            split = k;
        }
    }

    let mut left_result = hirschberg(&payload[..split], left_ref, pay_offset, ref_offset, mode);
    let right_result = hirschberg(
        &payload[split..],
        right_ref,
        pay_offset + split,
        ref_offset + mid,
        mode,
    );
    left_result.extend(right_result);
    left_result
}

/// Compute the last row of the DP cost matrix for reference vs payload.
///
/// Reuses a scratch buffer (`cur`) across rows instead of allocating a fresh
/// `Vec` per reference item. See `book/src/developer/arena-allocators.md`
/// Pattern 2 (scratch buffers).
fn row_costs<T: Alignable>(reference: &[T], payload: &[T], mode: MatchMode) -> Vec<usize> {
    let pay_len = payload.len();
    let mut prev: Vec<usize> = (0..=pay_len).collect();
    let mut cur = Vec::with_capacity(pay_len + 1);

    for ref_item in reference.iter() {
        cur.clear();
        cur.push(prev[0] + COST_GAP);
        for (pay_idx, pay_item) in payload.iter().enumerate() {
            let is_match = ref_item.matches(pay_item, mode);
            let sub_cost = prev[pay_idx] + if is_match { 0 } else { COST_SUB };
            let del_cost = prev[pay_idx + 1] + COST_GAP;
            let ins_cost = cur[pay_idx] + COST_GAP;
            cur.push(sub_cost.min(del_cost).min(ins_cost));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev
}

/// Compute `row_costs` on reversed reference and payload without cloning.
fn row_costs_rev<T: Alignable>(reference: &[T], payload: &[T], mode: MatchMode) -> Vec<usize> {
    let pay_len = payload.len();
    let mut prev: Vec<usize> = (0..=pay_len).collect();
    let mut cur = Vec::with_capacity(pay_len + 1);

    for ref_item in reference.iter().rev() {
        cur.clear();
        cur.push(prev[0] + COST_GAP);
        for (pay_idx, pay_item) in payload.iter().rev().enumerate() {
            let is_match = ref_item.matches(pay_item, mode);
            let sub_cost = prev[pay_idx] + if is_match { 0 } else { COST_SUB };
            let del_cost = prev[pay_idx + 1] + COST_GAP;
            let ins_cost = cur[pay_idx] + COST_GAP;
            cur.push(sub_cost.min(del_cost).min(ins_cost));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev
}

/// Traceback action for the full-table aligner.
#[derive(Debug, Clone, Copy)]
enum Action {
    Start,
    Match,
    Substitution,
    ExtraPayload,
    ExtraReference,
}

/// Full-table alignment for small problems.
///
/// Uses a flat `Vec` instead of `Vec<Vec<...>>` to reduce allocation count
/// from `rows + 1` to `1`. See `book/src/developer/arena-allocators.md`
/// Pattern 3 (flat table).
fn align_small<T: Alignable>(
    payload: &[T],
    reference: &[T],
    pay_offset: usize,
    ref_offset: usize,
    mode: MatchMode,
) -> Vec<AlignResult> {
    let rows = reference.len() + 1;
    let cols = payload.len() + 1;

    // Flat table: dp[i * cols + j] instead of dp[i][j]
    let mut dp = vec![(0usize, Action::Start, 0usize, 0usize); rows * cols];
    let idx = |r: usize, c: usize| r * cols + c;

    for i in 1..rows {
        dp[idx(i, 0)] = (i * COST_GAP, Action::ExtraReference, i - 1, 0);
    }
    for j in 1..cols {
        dp[idx(0, j)] = (j * COST_GAP, Action::ExtraPayload, 0, j - 1);
    }

    for i in 1..rows {
        for j in 1..cols {
            let is_match = reference[i - 1].matches(&payload[j - 1], mode);
            let sub_cost = dp[idx(i - 1, j - 1)].0 + if is_match { 0 } else { COST_SUB };
            let del_cost = dp[idx(i - 1, j)].0 + COST_GAP;
            let ins_cost = dp[idx(i, j - 1)].0 + COST_GAP;

            if sub_cost <= del_cost && sub_cost <= ins_cost {
                let action = if is_match {
                    Action::Match
                } else {
                    Action::Substitution
                };
                dp[idx(i, j)] = (sub_cost, action, i - 1, j - 1);
            } else if del_cost <= sub_cost && del_cost <= ins_cost {
                dp[idx(i, j)] = (del_cost, Action::ExtraReference, i - 1, j);
            } else {
                dp[idx(i, j)] = (ins_cost, Action::ExtraPayload, i, j - 1);
            }
        }
    }

    let mut output = Vec::new();
    let mut i = rows - 1;
    let mut j = cols - 1;

    while i > 0 || j > 0 {
        let (_, action, pi, pj) = dp[idx(i, j)];
        match action {
            Action::Match => {
                output.push(AlignResult::Match {
                    key: reference[pi].to_key(),
                    payload_idx: pay_offset + pj,
                    reference_idx: ref_offset + pi,
                });
            }
            Action::Substitution => {
                output.push(AlignResult::ExtraPayload {
                    key: payload[pj].to_key(),
                    payload_idx: pay_offset + pj,
                });
                output.push(AlignResult::ExtraReference {
                    key: reference[pi].to_key(),
                    reference_idx: ref_offset + pi,
                });
            }
            Action::ExtraPayload => {
                output.push(AlignResult::ExtraPayload {
                    key: payload[pj].to_key(),
                    payload_idx: pay_offset + pj,
                });
            }
            Action::ExtraReference => {
                output.push(AlignResult::ExtraReference {
                    key: reference[pi].to_key(),
                    reference_idx: ref_offset + pi,
                });
            }
            Action::Start => break,
        }
        i = pi;
        j = pj;
    }

    output.reverse();
    output
}

#[cfg(test)]
mod tests;

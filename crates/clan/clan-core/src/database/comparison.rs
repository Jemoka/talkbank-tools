//! Statistical comparison of a speaker's scores against normative data.
//!
//! Given a speaker's score vector and a filtered set of database entries,
//! computes mean, standard deviation, and z-score (standard deviations from
//! the norm) for each measure. This replicates CLAN's `compute_SD()` logic.

use serde::Serialize;

use crate::database::entry::DatabaseEntry;

/// Comparison statistics for a single measure.
#[derive(Debug, Clone, Serialize)]
pub struct MeasureComparison {
    /// The speaker's observed score.
    pub score: f64,
    /// Mean of the matched normative population for this measure.
    pub db_mean: f64,
    /// Standard deviation of the matched normative population.
    pub db_sd: f64,
    /// Z-score: `(score - mean) / sd`. `None` if SD is zero (no variance).
    pub z_score: Option<f64>,
    /// Number of database entries used for comparison.
    pub db_n: usize,
}

/// Full comparison result: one [`MeasureComparison`] per score column.
#[derive(Debug, Clone, Serialize)]
pub struct ComparisonResult {
    /// Per-measure comparisons, indexed by column position.
    pub measures: Vec<MeasureComparison>,
    /// Total number of matched database entries.
    pub matched_entries: usize,
}

/// Compare a speaker's scores against a filtered set of normative entries.
///
/// `speaker_scores` and each entry's `scores` vector are parallel arrays —
/// column `i` in the speaker maps to column `i` in the database.
///
/// Columns where the database has fewer entries than the speaker are skipped
/// (the resulting `measures` vector will be shorter in that case).
pub fn compare_to_norms(
    speaker_scores: &[f64],
    matched_entries: &[&DatabaseEntry],
) -> ComparisonResult {
    let n = matched_entries.len();

    if n == 0 {
        return ComparisonResult {
            measures: speaker_scores
                .iter()
                .map(|&score| MeasureComparison {
                    score,
                    db_mean: 0.0,
                    db_sd: 0.0,
                    z_score: None,
                    db_n: 0,
                })
                .collect(),
            matched_entries: 0,
        };
    }

    let mut measures = Vec::with_capacity(speaker_scores.len());

    for (col, &speaker_score) in speaker_scores.iter().enumerate() {
        let (sum, sum_sq, count) = matched_entries.iter().fold(
            (0.0_f64, 0.0_f64, 0_usize),
            |(sum, sum_sq, count), entry| {
                if let Some(&val) = entry.scores.get(col) {
                    (sum + val, sum_sq + val * val, count + 1)
                } else {
                    (sum, sum_sq, count)
                }
            },
        );

        if count == 0 {
            measures.push(MeasureComparison {
                score: speaker_score,
                db_mean: 0.0,
                db_sd: 0.0,
                z_score: None,
                db_n: 0,
            });
            continue;
        }

        let mean = sum / count as f64;

        // Sample variance: (sum_sq - mean^2 * n) / (n - 1)
        // Matches CLAN's compute_SD(): `(sqr_mean - (mean * mean / num)) / (num - 1)`
        let sd = if count > 1 {
            let variance = (sum_sq - (sum * sum / count as f64)) / (count as f64 - 1.0);
            // Guard against floating-point rounding producing negative variance
            if variance > 0.0 { variance.sqrt() } else { 0.0 }
        } else {
            0.0
        };

        let z_score = if sd > 0.0 {
            Some((speaker_score - mean) / sd)
        } else {
            None
        };

        measures.push(MeasureComparison {
            score: speaker_score,
            db_mean: mean,
            db_sd: sd,
            z_score,
            db_n: count,
        });
    }

    ComparisonResult {
        measures,
        matched_entries: n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::entry::{DatabaseEntry, DbMetadata};

    fn make_entry(scores: Vec<f64>) -> DatabaseEntry {
        DatabaseEntry {
            file_path: String::new(),
            metadata: DbMetadata {
                language: "eng".to_owned(),
                corpus: String::new(),
                speaker_code: "CHI".to_owned(),
                age_months: Some(24),
                sex: None,
                group: String::new(),
                ses: String::new(),
                role: String::new(),
                education: String::new(),
                custom: String::new(),
            },
            scores,
        }
    }

    #[test]
    fn no_entries() {
        let result = compare_to_norms(&[10.0, 20.0], &[]);
        assert_eq!(result.matched_entries, 0);
        assert_eq!(result.measures.len(), 2);
        assert!(result.measures[0].z_score.is_none());
    }

    #[test]
    fn single_entry_no_variance() {
        let e = make_entry(vec![10.0, 20.0]);
        let result = compare_to_norms(&[15.0, 25.0], &[&e]);
        assert_eq!(result.matched_entries, 1);
        assert_eq!(result.measures.len(), 2);
        assert!((result.measures[0].db_mean - 10.0).abs() < f64::EPSILON);
        assert!((result.measures[0].db_sd).abs() < f64::EPSILON);
        assert!(result.measures[0].z_score.is_none()); // SD=0 → no z-score
    }

    #[test]
    fn two_entries_with_variance() {
        let e1 = make_entry(vec![10.0, 20.0]);
        let e2 = make_entry(vec![20.0, 40.0]);
        let result = compare_to_norms(&[15.0, 30.0], &[&e1, &e2]);

        assert_eq!(result.matched_entries, 2);

        // Column 0: mean=15, sample SD = sqrt((100+400 - 900/2) / 1) = sqrt(50) ≈ 7.071
        let m0 = &result.measures[0];
        assert!((m0.db_mean - 15.0).abs() < 1e-10);
        assert!((m0.db_sd - (50.0_f64).sqrt()).abs() < 1e-10);
        // z-score = (15 - 15) / 7.071 = 0
        assert!((m0.z_score.unwrap()).abs() < 1e-10);

        // Column 1: mean=30, SD = sqrt((400+1600 - 3600/2) / 1) = sqrt(200) ≈ 14.142
        let m1 = &result.measures[1];
        assert!((m1.db_mean - 30.0).abs() < 1e-10);
        assert!((m1.z_score.unwrap()).abs() < 1e-10);
    }

    #[test]
    fn z_score_direction() {
        let e1 = make_entry(vec![100.0]);
        let e2 = make_entry(vec![200.0]);
        // Speaker score well above mean
        let result = compare_to_norms(&[300.0], &[&e1, &e2]);
        assert!(result.measures[0].z_score.unwrap() > 0.0);

        // Speaker score well below mean
        let result = compare_to_norms(&[0.0], &[&e1, &e2]);
        assert!(result.measures[0].z_score.unwrap() < 0.0);
    }
}

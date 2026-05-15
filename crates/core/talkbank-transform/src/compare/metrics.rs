use std::collections::BTreeMap;

use super::model::{CompareMetrics, CompareStatus, CompareToken, PosErrorCounts};
use super::serialize::{ComparePosLabel, CompareSerializationError};

/// Structured compare metrics table for CSV output.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareMetricsCsvTable {
    /// Data rows written after the header row.
    pub rows: Vec<CompareMetricsCsvRow>,
}

impl CompareMetricsCsvTable {
    /// Build a structured CSV table from aggregate compare metrics.
    pub fn from_metrics(metrics: &CompareMetrics) -> Result<Self, CompareSerializationError> {
        let mut rows = vec![
            CompareMetricsCsvRow::new(
                CompareMetricName::Wer,
                CompareMetricValue::Decimal(metrics.wer),
            ),
            CompareMetricsCsvRow::new(
                CompareMetricName::Accuracy,
                CompareMetricValue::Decimal(metrics.accuracy),
            ),
            CompareMetricsCsvRow::new(
                CompareMetricName::Matches,
                CompareMetricValue::Count(metrics.matches),
            ),
            CompareMetricsCsvRow::new(
                CompareMetricName::Insertions,
                CompareMetricValue::Count(metrics.insertions),
            ),
            CompareMetricsCsvRow::new(
                CompareMetricName::Deletions,
                CompareMetricValue::Count(metrics.deletions),
            ),
            CompareMetricsCsvRow::new(
                CompareMetricName::TotalGoldWords,
                CompareMetricValue::Count(metrics.total_gold_words),
            ),
            CompareMetricsCsvRow::new(
                CompareMetricName::TotalMainWords,
                CompareMetricValue::Count(metrics.total_main_words),
            ),
        ];

        for (pos, counts) in &metrics.pos_counts {
            let pos = ComparePosLabel::for_metrics(pos)?;
            rows.push(CompareMetricsCsvRow::new(
                CompareMetricName::Pos {
                    pos: pos.clone(),
                    kind: ComparePosMetricKind::Matches,
                },
                CompareMetricValue::Count(counts.matches),
            ));
            rows.push(CompareMetricsCsvRow::new(
                CompareMetricName::Pos {
                    pos: pos.clone(),
                    kind: ComparePosMetricKind::Insertions,
                },
                CompareMetricValue::Count(counts.insertions),
            ));
            rows.push(CompareMetricsCsvRow::new(
                CompareMetricName::Pos {
                    pos: pos.clone(),
                    kind: ComparePosMetricKind::Deletions,
                },
                CompareMetricValue::Count(counts.deletions),
            ));
            rows.push(CompareMetricsCsvRow::new(
                CompareMetricName::Pos {
                    pos,
                    kind: ComparePosMetricKind::Total,
                },
                CompareMetricValue::Count(counts.matches + counts.deletions),
            ));
        }

        Ok(Self { rows })
    }

    /// Serialize the structured compare metrics table with the standard CSV crate.
    pub fn to_csv_string(&self) -> Result<String, CompareSerializationError> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .from_writer(Vec::new());
        writer.write_record([
            CompareCsvHeader::Metric.as_str(),
            CompareCsvHeader::Value.as_str(),
        ])?;
        for row in &self.rows {
            writer.write_record([row.metric.to_csv_field(), row.value.to_csv_field()])?;
        }
        let bytes = writer
            .into_inner()
            .map_err(|err| CompareSerializationError::Csv(err.into_error().into()))?;
        Ok(String::from_utf8(bytes)?)
    }
}

/// One data row in the compare metrics CSV.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareMetricsCsvRow {
    /// Structured metric key.
    pub metric: CompareMetricName,
    /// Structured metric value.
    pub value: CompareMetricValue,
}

impl CompareMetricsCsvRow {
    fn new(metric: CompareMetricName, value: CompareMetricValue) -> Self {
        Self { metric, value }
    }
}

/// CSV header names for compare metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareCsvHeader {
    /// `metric`
    Metric,
    /// `value`
    Value,
}

impl CompareCsvHeader {
    fn as_str(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Value => "value",
        }
    }
}

/// Structured metric key for compare CSV output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareMetricName {
    /// Aggregate word error rate.
    Wer,
    /// Aggregate token accuracy.
    Accuracy,
    /// Aggregate exact-match token count.
    Matches,
    /// Aggregate insertion count.
    Insertions,
    /// Aggregate deletion count.
    Deletions,
    /// Aggregate gold/reference token count.
    TotalGoldWords,
    /// Aggregate main/hypothesis token count.
    TotalMainWords,
    /// Per-POS metric row.
    Pos {
        /// POS label rendered in the metric key.
        pos: ComparePosLabel,
        /// Which per-POS aggregate this row carries.
        kind: ComparePosMetricKind,
    },
}

impl CompareMetricName {
    fn to_csv_field(&self) -> String {
        match self {
            Self::Wer => "wer".to_string(),
            Self::Accuracy => "accuracy".to_string(),
            Self::Matches => "matches".to_string(),
            Self::Insertions => "insertions".to_string(),
            Self::Deletions => "deletions".to_string(),
            Self::TotalGoldWords => "total_gold_words".to_string(),
            Self::TotalMainWords => "total_main_words".to_string(),
            Self::Pos { pos, kind } => format!("{}:{}", pos.as_str(), kind.as_str()),
        }
    }
}

/// Structured value for compare CSV output.
#[derive(Debug, Clone, PartialEq)]
pub enum CompareMetricValue {
    /// Fixed-precision decimal metric.
    Decimal(f64),
    /// Nonnegative count metric.
    Count(usize),
}

impl CompareMetricValue {
    fn to_csv_field(&self) -> String {
        match self {
            Self::Decimal(value) => format!("{value:.4}"),
            Self::Count(value) => value.to_string(),
        }
    }
}

/// Per-POS metric subtype in compare CSV output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparePosMetricKind {
    /// Per-POS exact-match count.
    Matches,
    /// Per-POS insertion count.
    Insertions,
    /// Per-POS deletion count.
    Deletions,
    /// Per-POS gold/reference total.
    Total,
}

impl ComparePosMetricKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Matches => "matches",
            Self::Insertions => "insertions",
            Self::Deletions => "deletions",
            Self::Total => "total",
        }
    }
}

#[derive(Default)]
pub(in crate::compare) struct MetricAccumulator {
    matches: usize,
    insertions: usize,
    deletions: usize,
    pos_counts: BTreeMap<String, PosErrorCounts>,
}

impl MetricAccumulator {
    pub(in crate::compare) fn record(&mut self, token: &CompareToken) {
        if token.pos.as_deref() == Some("PUNCT") {
            return;
        }

        match token.status {
            CompareStatus::Match => {
                self.matches += 1;
                self.pos_counts
                    .entry(metric_pos_label(token.pos.as_deref()))
                    .or_default()
                    .matches += 1;
            }
            CompareStatus::ExtraMain => {
                self.insertions += 1;
                self.pos_counts
                    .entry(metric_pos_label(token.pos.as_deref()))
                    .or_default()
                    .insertions += 1;
            }
            CompareStatus::ExtraGold => {
                self.deletions += 1;
                self.pos_counts
                    .entry(metric_pos_label(token.pos.as_deref()))
                    .or_default()
                    .deletions += 1;
            }
        }
    }

    pub(in crate::compare) fn finish(self) -> CompareMetrics {
        let total_gold = self.matches + self.deletions;
        let total_main = self.matches + self.insertions;
        let wer = if total_gold > 0 {
            (self.insertions + self.deletions) as f64 / total_gold as f64
        } else {
            0.0
        };
        let accuracy = (1.0 - wer).clamp(0.0, 1.0);

        CompareMetrics {
            wer,
            accuracy,
            matches: self.matches,
            insertions: self.insertions,
            deletions: self.deletions,
            total_gold_words: total_gold,
            total_main_words: total_main,
            pos_counts: self.pos_counts,
        }
    }
}

pub(in crate::compare) fn metric_pos_label(pos: Option<&str>) -> String {
    pos.unwrap_or("?").to_uppercase()
}

/// Serialize comparison metrics as CSV rows with header.
pub fn format_metrics_csv(metrics: &CompareMetrics) -> Result<String, CompareSerializationError> {
    CompareMetricsCsvTable::from_metrics(metrics)?.to_csv_string()
}

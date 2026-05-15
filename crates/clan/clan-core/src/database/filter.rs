//! Demographic filtering for normative database entries.
//!
//! Users select a comparison population by specifying criteria such as language,
//! corpus type, age range, and gender. The [`DatabaseFilter`] applies these
//! criteria to select matching entries from a parsed database.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::database::entry::{DatabaseEntry, Sex};

/// Gender filter for database comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Gender {
    /// Include only male participants.
    Male,
    /// Include only female participants.
    Female,
    /// Include both genders.
    Both,
}

/// Criteria for selecting a comparison population from a normative database.
///
/// All fields are optional — omitted fields match everything.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct DatabaseFilter {
    /// Required language code (e.g., "eng"). If `None`, matches all languages.
    pub language: Option<String>,
    /// Required group (e.g., "TD", "Broca", "Control"). If `None`, matches all groups.
    pub group: Option<String>,
    /// Gender filter. If `None`, defaults to [`Gender::Both`].
    pub gender: Option<Gender>,
    /// Minimum age in months (inclusive). If `None`, no lower bound.
    pub age_from_months: Option<u32>,
    /// Maximum age in months (inclusive). If `None`, no upper bound.
    pub age_to_months: Option<u32>,
    /// Required speaker codes (e.g., `["CHI"]`). If empty, matches all speakers.
    pub speaker_codes: Vec<String>,
}

impl DatabaseFilter {
    /// Test whether a database entry matches all filter criteria.
    pub fn matches(&self, entry: &DatabaseEntry) -> bool {
        let meta = &entry.metadata;

        // Language
        if let Some(ref lang) = self.language
            && !meta.language.eq_ignore_ascii_case(lang)
        {
            return false;
        }

        // Group
        if let Some(ref group) = self.group
            && !meta.group.eq_ignore_ascii_case(group)
        {
            return false;
        }

        // Gender
        match self.gender {
            Some(Gender::Male) => {
                if meta.sex != Some(Sex::Male) {
                    return false;
                }
            }
            Some(Gender::Female) => {
                if meta.sex != Some(Sex::Female) {
                    return false;
                }
            }
            Some(Gender::Both) | None => {}
        }

        // Age range
        if let Some(min) = self.age_from_months {
            match meta.age_months {
                Some(age) if age >= min => {}
                Some(_) => return false,
                None => return false,
            }
        }
        if let Some(max) = self.age_to_months {
            match meta.age_months {
                Some(age) if age <= max => {}
                Some(_) => return false,
                None => return false,
            }
        }

        // Speaker codes
        if !self.speaker_codes.is_empty()
            && !self
                .speaker_codes
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&meta.speaker_code))
        {
            return false;
        }

        true
    }

    /// Filter a slice of entries, returning only those that match.
    pub fn apply<'a>(&self, entries: &'a [DatabaseEntry]) -> Vec<&'a DatabaseEntry> {
        entries.iter().filter(|e| self.matches(e)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::entry::DbMetadata;

    fn make_entry(lang: &str, age: Option<u32>, sex: Option<Sex>, group: &str) -> DatabaseEntry {
        DatabaseEntry {
            file_path: String::new(),
            metadata: DbMetadata {
                language: lang.to_owned(),
                corpus: String::new(),
                speaker_code: "CHI".to_owned(),
                age_months: age,
                sex,
                group: group.to_owned(),
                ses: String::new(),
                role: "Target_Child".to_owned(),
                education: String::new(),
                custom: String::new(),
            },
            scores: vec![1.0, 2.0, 3.0],
        }
    }

    #[test]
    fn empty_filter_matches_all() {
        let filter = DatabaseFilter::default();
        let e = make_entry("eng", Some(24), Some(Sex::Male), "TD");
        assert!(filter.matches(&e));
    }

    #[test]
    fn language_filter() {
        let filter = DatabaseFilter {
            language: Some("eng".to_owned()),
            ..Default::default()
        };
        assert!(filter.matches(&make_entry("eng", Some(24), None, "")));
        assert!(!filter.matches(&make_entry("fra", Some(24), None, "")));
        // Case-insensitive
        assert!(filter.matches(&make_entry("ENG", Some(24), None, "")));
    }

    #[test]
    fn age_range_filter() {
        let filter = DatabaseFilter {
            age_from_months: Some(18),
            age_to_months: Some(30),
            ..Default::default()
        };
        assert!(filter.matches(&make_entry("eng", Some(24), None, "")));
        assert!(filter.matches(&make_entry("eng", Some(18), None, "")));
        assert!(filter.matches(&make_entry("eng", Some(30), None, "")));
        assert!(!filter.matches(&make_entry("eng", Some(17), None, "")));
        assert!(!filter.matches(&make_entry("eng", Some(31), None, "")));
        // Missing age fails when age bounds are set
        assert!(!filter.matches(&make_entry("eng", None, None, "")));
    }

    #[test]
    fn gender_filter() {
        let male_only = DatabaseFilter {
            gender: Some(Gender::Male),
            ..Default::default()
        };
        assert!(male_only.matches(&make_entry("eng", Some(24), Some(Sex::Male), "")));
        assert!(!male_only.matches(&make_entry("eng", Some(24), Some(Sex::Female), "")));
        assert!(!male_only.matches(&make_entry("eng", Some(24), None, "")));
    }

    #[test]
    fn combined_filter() {
        let filter = DatabaseFilter {
            language: Some("eng".to_owned()),
            gender: Some(Gender::Female),
            age_from_months: Some(18),
            age_to_months: Some(36),
            ..Default::default()
        };
        assert!(filter.matches(&make_entry("eng", Some(24), Some(Sex::Female), "")));
        assert!(!filter.matches(&make_entry("fra", Some(24), Some(Sex::Female), "")));
        assert!(!filter.matches(&make_entry("eng", Some(24), Some(Sex::Male), "")));
        assert!(!filter.matches(&make_entry("eng", Some(40), Some(Sex::Female), "")));
    }
}

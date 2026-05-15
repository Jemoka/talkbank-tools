//! Participant-role validation helpers for `@Participants` and `@ID`.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Role_Field>

/// Canonical participant role labels accepted by CHAT headers.
///
/// Roles here are used by `@Participants` and validated in `@ID` role fields.
const VALID_PARTICIPANT_ROLES: &[&str] = &[
    "Target_Child",
    "Target_Adult",
    "Child",
    "Mother",
    "Father",
    "Brother",
    "Sister",
    "Sibling",
    "Grandfather",
    "Grandmother",
    "Relative",
    "Participant",
    "Therapist",
    "Informant",
    "Subject",
    "Investigator",
    "Partner",
    "Boy",
    "Girl",
    "Adult",
    "Teenager",
    "Male",
    "Female",
    "Visitor",
    "Friend",
    "Playmate",
    "Caretaker",
    "Environment",
    "Group",
    "Unidentified",
    "Uncertain",
    "Other",
    "Text",
    "Media",
    "PlayRole",
    "LENA",
    "Justice",
    "Attorney",
    "Doctor",
    "Nurse",
    "Student",
    "Teacher",
    "Host",
    "Guest",
    "Leader",
    "Member",
    "Narrator",
    "Speaker",
    "Audience",
];

/// Returns whether a participant role is canonical per CHAT conventions.
///
/// Matching is exact and case-sensitive because canonical role strings are
/// serialized and validated as stable header tokens.
pub fn is_allowed_participant_role(role: &str) -> bool {
    VALID_PARTICIPANT_ROLES.contains(&role)
}

/// Suggest a likely canonical role for a misspelled or colloquial input.
///
/// This is a heuristic fallback intended for user-facing diagnostics, not a
/// semantic normalization layer.
pub fn suggest_similar_role(invalid_role: &str) -> &'static str {
    let lower = invalid_role.to_lowercase();

    if lower.contains("child") || lower.contains("chi") {
        "Child or Target_Child"
    } else if lower.contains("moth") || lower.contains("mom") || lower.contains("mot") {
        "Mother"
    } else if lower.contains("fath") || lower.contains("dad") || lower.contains("fat") {
        "Father"
    } else if lower.contains("adult") || lower.contains("adu") {
        "Adult or Target_Adult"
    } else if lower.contains("teach") {
        "Teacher"
    } else if lower.contains("stud") {
        "Student"
    } else {
        "Child, Mother, Father, Adult"
    }
}

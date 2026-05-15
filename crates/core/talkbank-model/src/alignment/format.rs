//! Shared formatting for alignment mismatch error messages.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Morphological_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Word_Timing_Tier>

use super::helpers::TierPosition;

/// Format a positional alignment mismatch for cross-domain tiers.
///
/// Uses simple positional pairing. Appropriate for tiers like %mor, %pho,
/// and %sin where the dependent tier items are in a completely different
/// domain (morphological tags, phonological tokens, gestures) and string
/// matching is meaningless.
pub fn format_positional_mismatch(
    left_name: &str,
    right_name: &str,
    left_items: &[TierPosition],
    right_items: &[TierPosition],
) -> String {
    let left_count = left_items.len();
    let right_count = right_items.len();

    let mut msg = format!(
        "{left_name} has {left_count} alignable items, but {right_name} has {right_count} items\n\n",
    );

    msg.push_str(&format!("{:<30} {}\n", left_name, right_name));
    msg.push_str(&format!("{:<30} {}\n", "─".repeat(28), "─".repeat(28)));

    let max_len = left_items.len().max(right_items.len());
    for i in 0..max_len {
        let left_text = left_items.get(i).map_or("—", |item| &item.text);
        let right_text = right_items.get(i).map_or("—", |item| &item.text);

        let marker = if i >= left_items.len() {
            " ⊕"
        } else if i >= right_items.len() {
            " ⊖"
        } else {
            ""
        };

        msg.push_str(&format!("{:<30} {}{}\n", left_text, right_text, marker));
    }

    msg
}

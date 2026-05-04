//! Context truncation strategies for `ContextBuilder`.
//!
//! Implement `ContextTruncator` to replace the hard-coded oldest-first policy.

/// A strategy that decides how to trim a list of (path, body) sections to fit within `max_chars`.
///
/// The default implementation (`OldestFirstTruncator`) removes sections from the front
/// (oldest ancestors) until the total character count is within budget.
pub trait ContextTruncator: Send + Sync + 'static {
    /// Trim `sections` in-place so that the total body length is ≤ `max_chars`.
    ///
    /// Sections are ordered oldest → newest.  The last section (selected conversation)
    /// should be kept if at all possible.
    fn truncate(&self, sections: &mut Vec<(String, String)>, max_chars: usize);
}

// ── Default implementation ────────────────────────────────────────────────────

/// Removes sections from the front (oldest ancestor first) until the total fits.
///
/// Preserves at least the last section even if it alone exceeds `max_chars`.
pub struct OldestFirstTruncator;

impl ContextTruncator for OldestFirstTruncator {
    fn truncate(&self, sections: &mut Vec<(String, String)>, max_chars: usize) {
        let mut total: usize = sections.iter().map(|(_, b)| b.len()).sum();
        while total > max_chars && sections.len() > 1 {
            let removed = sections.remove(0);
            total -= removed.1.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sections(sizes: &[usize]) -> Vec<(String, String)> {
        sizes
            .iter()
            .enumerate()
            .map(|(i, &n)| (format!("path{i}"), "x".repeat(n)))
            .collect()
    }

    #[test]
    fn no_truncation_when_within_budget() {
        let truncator = OldestFirstTruncator;
        let mut sections = make_sections(&[100, 200, 300]);
        truncator.truncate(&mut sections, 1000);
        assert_eq!(sections.len(), 3);
    }

    #[test]
    fn removes_oldest_first() {
        let truncator = OldestFirstTruncator;
        let mut sections = make_sections(&[100, 200, 300]);
        // Budget = 400 — remove first (100) then check if 200+300=500 > 400, remove next (200).
        truncator.truncate(&mut sections, 400);
        // Only the last section (300) remains.
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "path2");
    }

    #[test]
    fn preserves_last_section_even_if_oversized() {
        let truncator = OldestFirstTruncator;
        let mut sections = make_sections(&[100, 5000]);
        truncator.truncate(&mut sections, 100);
        // Only the last section remains even though it exceeds the budget.
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].0, "path1");
    }

    #[test]
    fn single_section_unchanged() {
        let truncator = OldestFirstTruncator;
        let mut sections = make_sections(&[9999]);
        truncator.truncate(&mut sections, 1);
        assert_eq!(sections.len(), 1);
    }
}

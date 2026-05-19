use std::collections::VecDeque;

pub struct ReversePromptSearch {
    pub query: String,
    pub match_idx: Option<usize>,
}

/// Find the most recent (highest-index) prompt history entry strictly before
/// `before` that contains `query`. Empty query matches the most recent entry.
pub(super) fn find_match(entries: &VecDeque<String>, query: &str, before: usize) -> Option<usize> {
    let upper = before.min(entries.len());
    if upper == 0 {
        return None;
    }
    if query.is_empty() {
        return Some(upper - 1);
    }
    (0..upper).rev().find(|&i| entries[i].contains(query))
}

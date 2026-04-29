use crate::models::SshHost;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

#[derive(Debug, Clone)]
pub struct FilteredHost {
    pub original_index: usize,
    pub score: i64,
    pub matched_indices: Vec<usize>,
}

/// Fuzzy-search state over the host list. Owns its query string,
/// filtered results, and current selection within the result set.
#[derive(Debug, Default)]
pub struct SearchState {
    pub query: String,
    pub filtered: Vec<FilteredHost>,
    pub selected: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Recompute filtered results against the given host list.
    /// Empty query → empty results.
    pub fn filter(&mut self, hosts: &[SshHost]) {
        if self.query.is_empty() {
            self.filtered.clear();
        } else {
            let matcher = SkimMatcherV2::default();
            let mut results: Vec<FilteredHost> = hosts
                .iter()
                .enumerate()
                .filter_map(|(idx, host)| {
                    let full = format!(
                        "{} {} {} {}",
                        host.alias,
                        host.user,
                        host.host,
                        host.group.as_deref().unwrap_or("")
                    );
                    matcher
                        .fuzzy_indices(&full, &self.query)
                        .map(|(score, indices)| FilteredHost {
                            original_index: idx,
                            score,
                            matched_indices: indices,
                        })
                })
                .collect();
            results.sort_by(|a, b| b.score.cmp(&a.score));
            self.filtered = results;
        }

        if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len().saturating_sub(1);
        }
    }

    /// Cycle selection forward, wrapping at the end.
    pub fn select_next(&mut self) {
        if self.filtered.is_empty() {
            self.selected = 0;
            return;
        }
        let next = self.selected + 1;
        self.selected = if next < self.filtered.len() { next } else { 0 };
    }

    /// Cycle selection backward, wrapping at the start.
    pub fn select_previous(&mut self) {
        if self.filtered.is_empty() {
            self.selected = 0;
            return;
        }
        self.selected = if self.selected > 0 {
            self.selected - 1
        } else {
            self.filtered.len() - 1
        };
    }

    /// Clear query, filtered results, and selection.
    pub fn clear(&mut self) {
        self.query.clear();
        self.filtered.clear();
        self.selected = 0;
    }

    /// Index into the original host list for the currently-selected
    /// search result, if any.
    pub fn current_host_index(&self) -> Option<usize> {
        self.filtered.get(self.selected).map(|f| f.original_index)
    }
}

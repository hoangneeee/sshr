use crate::app::App;
use crate::app::types::{FilteredHost, InputMode};
use crate::models::SshHost;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

impl App {
    pub fn filter_hosts(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_hosts.clear();
        } else {
            let matcher = SkimMatcherV2::default();
            let query = &self.search_query;

            let mut results: Vec<FilteredHost> = self
                .hosts
                .iter()
                .enumerate()
                .filter_map(|(idx, host)| {
                    let full_string = format!(
                        "{} {} {} {}",
                        host.alias,
                        host.user,
                        host.host,
                        host.group.as_deref().unwrap_or("")
                    );
                    matcher
                        .fuzzy_indices(&full_string, query)
                        .map(|(score, indices)| FilteredHost {
                            original_index: idx,
                            score,
                            matched_indices: indices,
                        })
                })
                .collect();

            results.sort_by(|a, b| b.score.cmp(&a.score));
            self.filtered_hosts = results;
        }

        if self.search_selected >= self.filtered_hosts.len() {
            self.search_selected = self.filtered_hosts.len().saturating_sub(1);
        }
        self.host_list_state.select(Some(self.search_selected));
    }

    pub fn get_current_selected_host(&self) -> Option<&SshHost> {
        match self.input_mode {
            InputMode::Search => self
                .filtered_hosts
                .get(self.search_selected)
                .and_then(|filtered_host| self.hosts.get(filtered_host.original_index)),
            InputMode::Normal => self
                .hosts_in_current_group
                .get(self.selected_host)
                .and_then(|&idx| self.hosts.get(idx)),
            InputMode::Sftp => None,
        }
    }

    pub fn search_select_next(&mut self) {
        if self.filtered_hosts.is_empty() {
            self.search_selected = 0;
            return;
        }
        let next = self.search_selected + 1;
        if next < self.filtered_hosts.len() {
            self.search_selected = next;
        } else {
            self.search_selected = 0;
        }
        self.host_list_state.select(Some(self.search_selected));
    }

    pub fn search_select_previous(&mut self) {
        if self.filtered_hosts.is_empty() {
            self.search_selected = 0;
            return;
        }
        if self.search_selected > 0 {
            self.search_selected -= 1;
        } else {
            self.search_selected = self.filtered_hosts.len() - 1;
        }
        self.host_list_state.select(Some(self.search_selected));
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.input_mode = InputMode::Normal;
        self.filtered_hosts.clear();
        self.search_selected = 0;
        self.host_list_state.select(Some(self.selected_host));
    }

    pub fn enter_search_mode(&mut self) {
        self.input_mode = InputMode::Search;
        self.search_query.clear();
        self.switch_to_hosts();
        self.filter_hosts();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::app::types::InputMode;
    use crate::models::SshHost;

    fn make_hosts() -> Vec<SshHost> {
        vec![
            SshHost { alias: "web-prod".to_string(), host: "10.0.0.1".to_string(), user: "admin".to_string(), port: Some(22), description: None, group: Some("production".to_string()) },
            SshHost { alias: "db-prod".to_string(), host: "10.0.0.2".to_string(), user: "root".to_string(), port: Some(22), description: None, group: Some("production".to_string()) },
            SshHost { alias: "dev-box".to_string(), host: "192.168.1.5".to_string(), user: "dev".to_string(), port: Some(22), description: None, group: Some("dev".to_string()) },
        ]
    }

    #[test]
    fn test_filter_hosts_empty_query_clears_results() {
        let mut app = App::with_hosts(make_hosts());
        app.search_query = "web".to_string();
        app.filter_hosts();
        assert!(!app.filtered_hosts.is_empty());

        app.search_query.clear();
        app.filter_hosts();
        assert!(app.filtered_hosts.is_empty());
    }

    #[test]
    fn test_filter_hosts_matches_alias() {
        let mut app = App::with_hosts(make_hosts());
        app.search_query = "web".to_string();
        app.filter_hosts();
        assert!(!app.filtered_hosts.is_empty());
        let matched_host = app.hosts.get(app.filtered_hosts[0].original_index).unwrap();
        assert!(matched_host.alias.contains("web"));
    }

    #[test]
    fn test_filter_hosts_matches_group() {
        let mut app = App::with_hosts(make_hosts());
        app.search_query = "production".to_string();
        app.filter_hosts();
        assert_eq!(app.filtered_hosts.len(), 2);
    }

    #[test]
    fn test_filter_hosts_no_match_returns_empty() {
        let mut app = App::with_hosts(make_hosts());
        app.search_query = "zzzznotfound".to_string();
        app.filter_hosts();
        assert!(app.filtered_hosts.is_empty());
    }

    #[test]
    fn test_clear_search_resets_state() {
        let mut app = App::with_hosts(make_hosts());
        app.input_mode = InputMode::Search;
        app.search_query = "web".to_string();
        app.filter_hosts();
        app.search_selected = 1;

        app.clear_search();

        assert!(app.search_query.is_empty());
        assert!(app.filtered_hosts.is_empty());
        assert_eq!(app.search_selected, 0);
        assert_eq!(app.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_search_select_next_wraps() {
        let mut app = App::with_hosts(make_hosts());
        app.search_query = "production".to_string();
        app.filter_hosts();
        assert_eq!(app.filtered_hosts.len(), 2);

        app.search_selected = 0;
        app.search_select_next();
        assert_eq!(app.search_selected, 1);
        app.search_select_next();
        assert_eq!(app.search_selected, 0); // wraps back to 0
    }

    #[test]
    fn test_search_select_previous_wraps() {
        let mut app = App::with_hosts(make_hosts());
        app.search_query = "production".to_string();
        app.filter_hosts();

        app.search_selected = 0;
        app.search_select_previous();
        assert_eq!(app.search_selected, app.filtered_hosts.len() - 1); // wraps to last
    }

    #[test]
    fn test_get_current_selected_host_in_search_mode() {
        let mut app = App::with_hosts(make_hosts());
        app.input_mode = InputMode::Search;
        app.search_query = "web".to_string();
        app.filter_hosts();
        app.search_selected = 0;

        let host = app.get_current_selected_host();
        assert!(host.is_some());
        assert!(host.unwrap().alias.contains("web"));
    }

    #[test]
    fn test_get_current_selected_host_sftp_mode_returns_none() {
        let mut app = App::with_hosts(make_hosts());
        app.input_mode = InputMode::Sftp;
        assert!(app.get_current_selected_host().is_none());
    }
}

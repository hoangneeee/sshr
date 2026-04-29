use crate::app::App;
use crate::app::InputMode;
use crate::models::SshHost;

impl App {
    /// Re-run fuzzy filter against the current host list.
    pub fn filter_hosts(&mut self) {
        self.search.filter(&self.hosts.hosts);
        self.hosts.host_list_state.select(Some(self.search.selected));
    }

    /// Currently-selected host, considering whether we're in search or normal mode.
    pub fn get_current_selected_host(&self) -> Option<&SshHost> {
        match self.ui.input_mode {
            InputMode::Search => self
                .search
                .current_host_index()
                .and_then(|idx| self.hosts.hosts.get(idx)),
            InputMode::Normal => self.hosts.current_host(),
            InputMode::Sftp => None,
        }
    }

    pub fn search_select_next(&mut self) {
        self.search.select_next();
        self.hosts.host_list_state.select(Some(self.search.selected));
    }

    pub fn search_select_previous(&mut self) {
        self.search.select_previous();
        self.hosts.host_list_state.select(Some(self.search.selected));
    }

    pub fn clear_search(&mut self) {
        self.search.clear();
        self.ui.input_mode = InputMode::Normal;
        self.hosts.host_list_state.select(Some(self.hosts.selected_host));
    }

    pub fn enter_search_mode(&mut self) {
        self.ui.input_mode = InputMode::Search;
        self.search.clear();
        self.hosts.switch_to_hosts();
        self.filter_hosts();
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::app::InputMode;
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
        app.search.query = "web".to_string();
        app.filter_hosts();
        assert!(!app.search.filtered.is_empty());

        app.search.query.clear();
        app.filter_hosts();
        assert!(app.search.filtered.is_empty());
    }

    #[test]
    fn test_filter_hosts_matches_alias() {
        let mut app = App::with_hosts(make_hosts());
        app.search.query = "web".to_string();
        app.filter_hosts();
        assert!(!app.search.filtered.is_empty());
        let matched_host = app.hosts.hosts.get(app.search.filtered[0].original_index).unwrap();
        assert!(matched_host.alias.contains("web"));
    }

    #[test]
    fn test_filter_hosts_matches_group() {
        let mut app = App::with_hosts(make_hosts());
        app.search.query = "production".to_string();
        app.filter_hosts();
        assert_eq!(app.search.filtered.len(), 2);
    }

    #[test]
    fn test_filter_hosts_no_match_returns_empty() {
        let mut app = App::with_hosts(make_hosts());
        app.search.query = "zzzznotfound".to_string();
        app.filter_hosts();
        assert!(app.search.filtered.is_empty());
    }

    #[test]
    fn test_clear_search_resets_state() {
        let mut app = App::with_hosts(make_hosts());
        app.ui.input_mode = InputMode::Search;
        app.search.query = "web".to_string();
        app.filter_hosts();
        app.search.selected = 1;

        app.clear_search();

        assert!(app.search.query.is_empty());
        assert!(app.search.filtered.is_empty());
        assert_eq!(app.search.selected, 0);
        assert_eq!(app.ui.input_mode, InputMode::Normal);
    }

    #[test]
    fn test_search_select_next_wraps() {
        let mut app = App::with_hosts(make_hosts());
        app.search.query = "production".to_string();
        app.filter_hosts();
        assert_eq!(app.search.filtered.len(), 2);

        app.search.selected = 0;
        app.search_select_next();
        assert_eq!(app.search.selected, 1);
        app.search_select_next();
        assert_eq!(app.search.selected, 0); // wraps back to 0
    }

    #[test]
    fn test_search_select_previous_wraps() {
        let mut app = App::with_hosts(make_hosts());
        app.search.query = "production".to_string();
        app.filter_hosts();

        app.search.selected = 0;
        app.search_select_previous();
        assert_eq!(app.search.selected, app.search.filtered.len() - 1); // wraps to last
    }

    #[test]
    fn test_get_current_selected_host_in_search_mode() {
        let mut app = App::with_hosts(make_hosts());
        app.ui.input_mode = InputMode::Search;
        app.search.query = "web".to_string();
        app.filter_hosts();
        app.search.selected = 0;

        let host = app.get_current_selected_host();
        assert!(host.is_some());
        assert!(host.unwrap().alias.contains("web"));
    }

    #[test]
    fn test_get_current_selected_host_sftp_mode_returns_none() {
        let mut app = App::with_hosts(make_hosts());
        app.ui.input_mode = InputMode::Sftp;
        assert!(app.get_current_selected_host().is_none());
    }
}

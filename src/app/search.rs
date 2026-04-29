use crate::app::App;

impl App {
    /// Re-run fuzzy filter against the current host list.
    pub fn filter_hosts(&mut self) {
        self.search.filter(&self.hosts.hosts);
        self.hosts
            .host_list_state
            .select(Some(self.search.selected));
    }

    pub fn search_select_next(&mut self) {
        self.search.select_next();
        self.hosts
            .host_list_state
            .select(Some(self.search.selected));
    }

    pub fn search_select_previous(&mut self) {
        self.search.select_previous();
        self.hosts
            .host_list_state
            .select(Some(self.search.selected));
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
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
}

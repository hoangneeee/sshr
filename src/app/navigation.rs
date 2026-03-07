use crate::app::App;
use crate::app::types::ActivePanel;
use crate::models::SshHost;

impl App {
    pub fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Groups => ActivePanel::Hosts,
            ActivePanel::Hosts => ActivePanel::Groups,
        };

        tracing::info!("Switched to {:?} panel", self.active_panel);

        // When switching to Hosts panel, ensure selected_host is within bounds
        if self.active_panel == ActivePanel::Hosts && !self.hosts_in_current_group.is_empty() {
            self.selected_host = std::cmp::min(
                self.selected_host,
                self.hosts_in_current_group.len().saturating_sub(1),
            );
            self.host_list_state.select(Some(self.selected_host));
        }
    }

    pub fn switch_to_hosts(&mut self) {
        self.active_panel = ActivePanel::Hosts;
        tracing::info!("Switched to Hosts panel");

        // When switching to Hosts panel, ensure selected_host is within bounds
        if !self.hosts_in_current_group.is_empty() {
            self.selected_host = std::cmp::min(
                self.selected_host,
                self.hosts_in_current_group.len().saturating_sub(1),
            );
            self.host_list_state.select(Some(self.selected_host));
        }
    }

    pub fn update_hosts_for_selected_group(&mut self) {
        if self.groups.is_empty() {
            self.hosts_in_current_group.clear();
            return;
        }

        let current_group = &self.groups[self.selected_group];
        self.hosts_in_current_group = self
            .hosts
            .iter()
            .enumerate()
            .filter_map(|(i, host)| {
                let group_name = host.group.as_deref().unwrap_or("Ungrouped");
                if group_name == current_group {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        // Reset selected host when group changes
        if !self.hosts_in_current_group.is_empty() {
            self.selected_host = 0;
            self.host_list_state.select(Some(0));
        } else {
            self.selected_host = 0;
            self.host_list_state.select(None);
        }
    }

    pub fn get_current_host(&self) -> Option<&SshHost> {
        self.hosts_in_current_group
            .get(self.selected_host)
            .and_then(|&idx| self.hosts.get(idx))
    }

    pub fn select_next(&mut self) {
        match self.active_panel {
            ActivePanel::Groups => {
                if self.groups.is_empty() {
                    return;
                }
                self.selected_group = (self.selected_group + 1) % self.groups.len();
                self.group_list_state.select(Some(self.selected_group));
                self.update_hosts_for_selected_group();
            }
            ActivePanel::Hosts => {
                if self.hosts_in_current_group.is_empty() {
                    return;
                }
                self.selected_host = (self.selected_host + 1) % self.hosts_in_current_group.len();
                tracing::info!("Selected host: {}", self.selected_host);
                self.host_list_state.select(Some(self.selected_host));
            }
        }
    }

    pub fn select_previous(&mut self) {
        match self.active_panel {
            ActivePanel::Groups => {
                if self.groups.is_empty() {
                    return;
                }
                let total = self.groups.len();
                self.selected_group = (self.selected_group + total - 1) % total;
                self.group_list_state.select(Some(self.selected_group));
                self.update_hosts_for_selected_group();
            }
            ActivePanel::Hosts => {
                if self.hosts_in_current_group.is_empty() {
                    return;
                }
                let total = self.hosts_in_current_group.len();
                self.selected_host = (self.selected_host + total - 1) % total;
                tracing::info!("Selected host: {}", self.selected_host);
                self.host_list_state.select(Some(self.selected_host));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::app::types::ActivePanel;
    use crate::models::SshHost;

    fn make_hosts() -> Vec<SshHost> {
        vec![
            SshHost { alias: "web1".to_string(), host: "10.0.0.1".to_string(), user: "admin".to_string(), port: Some(22), description: None, group: Some("prod".to_string()) },
            SshHost { alias: "web2".to_string(), host: "10.0.0.2".to_string(), user: "admin".to_string(), port: Some(22), description: None, group: Some("prod".to_string()) },
            SshHost { alias: "dev1".to_string(), host: "192.168.1.1".to_string(), user: "dev".to_string(), port: Some(22), description: None, group: Some("dev".to_string()) },
        ]
    }

    #[test]
    fn test_select_next_wraps_hosts() {
        let mut app = App::with_hosts(make_hosts());
        app.active_panel = ActivePanel::Hosts;
        // prod group: 2 hosts
        assert_eq!(app.hosts_in_current_group.len(), 2);
        app.selected_host = 0;
        app.select_next();
        assert_eq!(app.selected_host, 1);
        app.select_next();
        assert_eq!(app.selected_host, 0); // wraps
    }

    #[test]
    fn test_select_previous_wraps_hosts() {
        let mut app = App::with_hosts(make_hosts());
        app.active_panel = ActivePanel::Hosts;
        app.selected_host = 0;
        app.select_previous();
        assert_eq!(app.selected_host, app.hosts_in_current_group.len() - 1); // wraps to last
    }

    #[test]
    fn test_select_next_empty_hosts_no_panic() {
        let mut app = App::with_hosts(vec![]);
        app.active_panel = ActivePanel::Hosts;
        app.select_next(); // should not panic
        assert_eq!(app.selected_host, 0);
    }

    #[test]
    fn test_switch_panel_toggles() {
        let mut app = App::with_hosts(make_hosts());
        assert_eq!(app.active_panel, ActivePanel::Groups);
        app.switch_panel();
        assert_eq!(app.active_panel, ActivePanel::Hosts);
        app.switch_panel();
        assert_eq!(app.active_panel, ActivePanel::Groups);
    }

    #[test]
    fn test_update_hosts_for_selected_group_filters_correctly() {
        let mut app = App::with_hosts(make_hosts());
        // Default group is first: "prod" with 2 hosts
        assert_eq!(app.hosts_in_current_group.len(), 2);

        // Switch to second group "dev"
        app.selected_group = 1;
        app.update_hosts_for_selected_group();
        assert_eq!(app.hosts_in_current_group.len(), 1);
    }

    #[test]
    fn test_select_next_groups_wraps() {
        let mut app = App::with_hosts(make_hosts());
        app.active_panel = ActivePanel::Groups;
        assert_eq!(app.groups.len(), 2);
        app.selected_group = 0;
        app.select_next();
        assert_eq!(app.selected_group, 1);
        app.select_next();
        assert_eq!(app.selected_group, 0); // wraps
    }
}

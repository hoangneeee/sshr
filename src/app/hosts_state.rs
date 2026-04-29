use crate::models::SshHost;
use ratatui::widgets::ListState;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivePanel {
    Groups,
    Hosts,
}

/// Host list, group derivation, and selection state for the main TUI.
#[derive(Debug)]
pub struct HostsState {
    pub hosts: Vec<SshHost>,
    pub groups: Vec<String>,
    /// Indices into `hosts` belonging to the currently-selected group.
    pub hosts_in_current_group: Vec<usize>,
    pub selected_host: usize,
    pub selected_group: usize,
    pub active_panel: ActivePanel,
    pub host_list_state: ListState,
    pub group_list_state: ListState,
}

impl Default for HostsState {
    fn default() -> Self {
        Self::new()
    }
}

impl HostsState {
    pub fn new() -> Self {
        Self {
            hosts: Vec::new(),
            groups: Vec::new(),
            hosts_in_current_group: Vec::new(),
            selected_host: 0,
            selected_group: 0,
            active_panel: ActivePanel::Groups,
            host_list_state: ListState::default(),
            group_list_state: ListState::default(),
        }
    }

    /// Currently-selected host according to the host panel position.
    pub fn current_host(&self) -> Option<&SshHost> {
        self.hosts_in_current_group
            .get(self.selected_host)
            .and_then(|&idx| self.hosts.get(idx))
    }

    pub fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Groups => ActivePanel::Hosts,
            ActivePanel::Hosts => ActivePanel::Groups,
        };
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
        if !self.hosts_in_current_group.is_empty() {
            self.selected_host = std::cmp::min(
                self.selected_host,
                self.hosts_in_current_group.len().saturating_sub(1),
            );
            self.host_list_state.select(Some(self.selected_host));
        }
    }

    /// Recompute `hosts_in_current_group` from current `selected_group`.
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
                (group_name == current_group).then_some(i)
            })
            .collect();

        if !self.hosts_in_current_group.is_empty() {
            self.selected_host = 0;
            self.host_list_state.select(Some(0));
        } else {
            self.selected_host = 0;
            self.host_list_state.select(None);
        }
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
                self.selected_host =
                    (self.selected_host + 1) % self.hosts_in_current_group.len();
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
                self.host_list_state.select(Some(self.selected_host));
            }
        }
    }

    /// Cycle group panel forward/backward, wrap-around.
    pub fn cycle_group(&mut self, forward: bool) {
        if self.groups.is_empty() {
            return;
        }
        let total = self.groups.len();
        self.selected_group = if forward {
            (self.selected_group + 1) % total
        } else {
            (self.selected_group + total - 1) % total
        };
        self.group_list_state.select(Some(self.selected_group));
        self.update_hosts_for_selected_group();
    }

    /// Recompute groups list from `hosts`. Called after loading hosts.
    pub fn rebuild_groups(&mut self) {
        let mut groups: Vec<String> = self
            .hosts
            .iter()
            .filter_map(|h| h.group.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        groups.sort();
        if self.hosts.iter().any(|h| h.group.is_none()) {
            groups.push("Ungrouped".to_string());
        }
        self.groups = groups;

        if self.groups.is_empty() {
            self.hosts_in_current_group.clear();
            return;
        }
        if self.selected_group >= self.groups.len() {
            self.selected_group = self.groups.len().saturating_sub(1);
        }
        self.update_hosts_for_selected_group();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hosts() -> Vec<SshHost> {
        vec![
            SshHost {
                alias: "web1".into(), host: "10.0.0.1".into(), user: "admin".into(),
                port: Some(22), description: None, group: Some("prod".into()),
            },
            SshHost {
                alias: "web2".into(), host: "10.0.0.2".into(), user: "admin".into(),
                port: Some(22), description: None, group: Some("prod".into()),
            },
            SshHost {
                alias: "dev1".into(), host: "192.168.1.1".into(), user: "dev".into(),
                port: Some(22), description: None, group: Some("dev".into()),
            },
        ]
    }

    fn populated() -> HostsState {
        let mut state = HostsState::new();
        state.hosts = make_hosts();
        state.rebuild_groups();
        state
    }

    #[test]
    fn select_next_wraps_hosts() {
        let mut state = populated();
        // groups are alphabetical: ["dev", "prod"], pick "prod" (2 hosts)
        state.selected_group = 1;
        state.update_hosts_for_selected_group();
        state.active_panel = ActivePanel::Hosts;
        assert_eq!(state.hosts_in_current_group.len(), 2);
        state.selected_host = 0;
        state.select_next();
        assert_eq!(state.selected_host, 1);
        state.select_next();
        assert_eq!(state.selected_host, 0); // wraps
    }

    #[test]
    fn select_previous_wraps_hosts() {
        let mut state = populated();
        state.selected_group = 1; // "prod"
        state.update_hosts_for_selected_group();
        state.active_panel = ActivePanel::Hosts;
        state.selected_host = 0;
        state.select_previous();
        assert_eq!(state.selected_host, state.hosts_in_current_group.len() - 1);
    }

    #[test]
    fn select_next_empty_no_panic() {
        let mut state = HostsState::new();
        state.active_panel = ActivePanel::Hosts;
        state.select_next();
        assert_eq!(state.selected_host, 0);
    }

    #[test]
    fn switch_panel_toggles() {
        let mut state = populated();
        assert_eq!(state.active_panel, ActivePanel::Groups);
        state.switch_panel();
        assert_eq!(state.active_panel, ActivePanel::Hosts);
        state.switch_panel();
        assert_eq!(state.active_panel, ActivePanel::Groups);
    }

    #[test]
    fn group_filter_correct() {
        let mut state = populated();
        // groups sorted alphabetically: ["dev", "prod"]
        assert_eq!(state.groups, vec!["dev".to_string(), "prod".to_string()]);
        // default selected_group = 0 → "dev" → 1 host
        assert_eq!(state.hosts_in_current_group.len(), 1);
        state.selected_group = 1; // "prod"
        state.update_hosts_for_selected_group();
        assert_eq!(state.hosts_in_current_group.len(), 2);
    }

    #[test]
    fn cycle_group_wraps() {
        let mut state = populated();
        state.active_panel = ActivePanel::Groups;
        assert_eq!(state.groups.len(), 2);
        state.selected_group = 0;
        state.cycle_group(true);
        assert_eq!(state.selected_group, 1);
        state.cycle_group(true);
        assert_eq!(state.selected_group, 0);
    }
}

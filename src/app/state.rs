use crate::app::App;
use crate::config::ConfigManager;
use crate::theme::ResolvedTheme;
use anyhow::{Context, Result};
use ratatui::widgets::ListState;
use std::path::PathBuf;

use crate::app::types::{ActivePanel, InputMode};

impl App {
    pub fn new() -> Result<Self> {
        let config_manager = ConfigManager::new().context("Failed to initialize config manager")?;
        let app_config = config_manager.load_config().context("Failed to load config")?;

        let ssh_config_path = PathBuf::from(&app_config.ssh_file_config);
        let strict_host_key_checking = app_config.strict_host_key_checking.clone();
        let theme = app_config
            .themes
            .iter()
            .find(|t| t.name == app_config.default_theme)
            .map(|t| ResolvedTheme::from_config(&t.colors))
            .unwrap_or_default();

        tracing::info!("SSH config path: {:?}", ssh_config_path);

        let mut app = Self::build(config_manager, ssh_config_path, strict_host_key_checking, theme);
        app.load_all_hosts().context("Failed to load hosts")?;
        app.host_list_state.select(Some(app.selected_host));
        Ok(app)
    }

    fn build(
        config_manager: ConfigManager,
        ssh_config_path: PathBuf,
        strict_host_key_checking: String,
        theme: ResolvedTheme,
    ) -> Self {
        Self {
            should_quit: false,
            hosts: Vec::new(),
            selected_host: 0,
            selected_group: 0,
            active_panel: ActivePanel::Groups,
            ssh_config_path,
            config_manager,
            input_mode: InputMode::Normal,
            strict_host_key_checking,
            theme,
            is_connecting: false,
            connecting_host: None,
            status_message: None,
            // SSH
            ssh_receiver: None,
            ssh_ready_for_terminal: false,
            // SFTP
            sftp_receiver: None,
            sftp_ready_for_terminal: false,
            is_sftp_loading: false,
            sftp_state: None,
            transfer_receiver: None,

            // Search
            search_query: String::new(),
            filtered_hosts: Vec::new(),
            search_selected: 0,

            // Group State
            groups: Vec::new(),
            hosts_in_current_group: Vec::new(),

            host_list_state: ListState::default(),
            group_list_state: ListState::default(),
        }
    }

    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    #[cfg(test)]
    pub fn with_hosts(hosts: Vec<crate::models::SshHost>) -> Self {
        let config_manager =
            ConfigManager::new().expect("test ConfigManager initialization failed");
        let mut app = Self::build(
            config_manager,
            PathBuf::new(),
            "accept-new".to_string(),
            ResolvedTheme::default(),
        );

        let mut groups: Vec<String> = Vec::new();
        for host in &hosts {
            let group = host.group.as_deref().unwrap_or("Ungrouped").to_string();
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
        app.hosts = hosts;
        app.groups = groups;
        app.update_hosts_for_selected_group();
        app
    }
}

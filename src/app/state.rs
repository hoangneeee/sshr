use crate::app::context::AppContext;
use crate::app::hosts_state::HostsState;
use crate::app::screen::{AppScreen, HostsScreen};
use crate::app::search_state::SearchState;
use crate::app::session::SessionState;
use crate::app::ui_state::UiState;
use crate::app::App;
use crate::config::ConfigManager;
use crate::theme::ResolvedTheme;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

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

        let ctx = AppContext {
            config_manager,
            ssh_config_path,
            strict_host_key_checking,
            theme: Arc::new(theme),
        };

        let mut app = Self::build(ctx);
        app.load_all_hosts().context("Failed to load hosts")?;
        app.hosts.host_list_state.select(Some(app.hosts.selected_host));
        Ok(app)
    }

    fn build(ctx: AppContext) -> Self {
        Self {
            should_quit: false,
            ctx,
            hosts: HostsState::new(),
            search: SearchState::new(),
            ui: UiState::new(),
            session: SessionState::Idle,
            screens: vec![AppScreen::Hosts(HostsScreen::new())],
        }
    }

    pub fn clear_status_message(&mut self) {
        self.ui.clear_status();
    }

    #[cfg(test)]
    pub fn with_hosts(hosts: Vec<crate::models::SshHost>) -> Self {
        let config_manager =
            ConfigManager::new().expect("test ConfigManager initialization failed");
        let ctx = AppContext {
            config_manager,
            ssh_config_path: PathBuf::new(),
            strict_host_key_checking: "accept-new".to_string(),
            theme: Arc::new(ResolvedTheme::default()),
        };
        let mut app = Self::build(ctx);
        app.hosts.hosts = hosts;
        app.hosts.rebuild_groups();
        app
    }
}

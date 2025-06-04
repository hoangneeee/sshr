use crate::config::{AppConfig, ConfigManager};
use crate::models::SshHost;
use crate::{ssh_service, ui};
use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use open;
use ratatui::{backend::Backend, Terminal};
use std::collections::HashSet;
use std::fs;
use std::net::ToSocketAddrs;
use std::path::PathBuf;

#[derive(Debug)]
pub enum InputMode {
    Normal,
}

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub hosts: Vec<SshHost>,
    pub selected: usize,
    pub ssh_config_path: PathBuf,
    pub config_manager: ConfigManager,
    pub app_config: AppConfig,
    pub input_mode: InputMode,
}

impl Default for App {
    fn default() -> Self {
        // Initialize config manager
        let config_manager = ConfigManager::new().unwrap_or_else(|e| {
            eprintln!("Failed to initialize config manager: {}", e);
            std::process::exit(1);
        });
        let app_config = config_manager.load_config().unwrap_or_else(|e| {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        });

        let ssh_config_path = PathBuf::from(app_config.ssh_file_config.clone());

        tracing::info!("SSH config path: {:?}", ssh_config_path);
        Self {
            should_quit: false,
            hosts: Vec::new(),
            selected: 0,
            ssh_config_path,
            config_manager,
            app_config,
            input_mode: InputMode::Normal,
        }
    }
}

impl App {
    pub fn new() -> Result<Self> {
        let mut app = Self::default();
        app.load_all_hosts().context("Failed to load hosts")?;
        Ok(app)
    }

    pub fn load_all_hosts(&mut self) -> Result<()> {
        self.load_ssh_config()
            .context("Failed to load SSH config")?;
        self.load_custom_hosts()
            .context("Failed to load custom hosts")?;
        self.handle_duplicate_hosts();
        Ok(())
    }

    pub fn load_ssh_config(&mut self) -> Result<()> {
        self.hosts.clear();

        if !self.ssh_config_path.exists() {
            tracing::warn!(
                "System SSH config file not found at {:?}",
                self.ssh_config_path
            );
            return Ok(());
        }

        let config_content =
            fs::read_to_string(&self.ssh_config_path).context("Failed to read SSH config file")?;

        let mut current_host: Option<SshHost> = None;

        for line in config_content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.to_lowercase().starts_with("host ") {
                // Save previous host if exists
                if let Some(host) = current_host.take() {
                    self.hosts.push(host);
                }

                // Start new host
                let alias = line[5..].trim().to_string();
                current_host = Some(SshHost::new(alias, String::new(), "root".to_string()));
            } else if let Some(host) = &mut current_host {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 2 {
                    continue;
                }

                match parts[0].to_lowercase().as_str() {
                    "hostname" => host.host = parts[1].to_string(),
                    "user" => host.user = parts[1].to_string(),
                    "port" => {
                        if let Ok(port) = parts[1].parse::<u16>() {
                            host.port = Some(port);
                        }
                    }
                    _ => {}
                }
            }
        }

        tracing::info!("Loaded {} hosts from SSH config", self.hosts.len());

        // Don't forget to add the last host
        if let Some(host) = current_host {
            self.hosts.push(host);
        }

        // Check reachability for each host
        for host in &mut self.hosts {
            let socket_addr = format!("{}:{}", host.host, host.port.unwrap_or(22))
                .to_socket_addrs()
                .ok()
                .and_then(|mut addrs| addrs.next());

            host.description = if socket_addr.is_some() {
                Some("Reachable".to_string())
            } else {
                Some("Unreachable".to_string())
            };
        }

        Ok(())
    }

    // Load custome hosts from hosts.toml
    pub fn load_custom_hosts(&mut self) -> Result<()> {
        match self.config_manager.load_hosts() {
            Ok(custom_hosts) => {
                self.hosts
                    .extend(custom_hosts.into_iter().map(|host| crate::models::SshHost {
                        alias: host.alias,
                        host: host.host,
                        user: host.user,
                        port: host.port,
                        description: host.description,
                        group: host.group,
                    }));
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to load custom hosts: {}", e);
                Ok(())
            }
        }
    }

    // Remove duplicate hosts
    pub fn handle_duplicate_hosts(&mut self) {
        let mut seen_aliases = HashSet::new();
        self.hosts.retain(|host| {
            if seen_aliases.contains(&host.alias) {
                // tracing::warn!("Duplicate alias found: {}", host.alias);
                false
            } else {
                seen_aliases.insert(host.alias.clone());
                true
            }
        });
    }

    // Get selected host
    pub fn get_selected_host(&self) -> Option<&SshHost> {
        if self.hosts.is_empty() {
            None
        } else {
            self.hosts.get(self.selected)
        }
    }

    // Improve navigation
    pub fn select_next(&mut self) {
        if self.hosts.is_empty() {
            return;
        }
        if self.selected >= self.hosts.len() - 1 {
            self.selected = 0; // Loop back to the first host
        } else {
            self.selected += 1;
        }
    }

    pub fn select_previous(&mut self) {
        if self.hosts.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.hosts.len() - 1; // Loop back to the last host
        } else {
            self.selected -= 1;
        }
    }

    // Handle key
    pub fn handle_key_enter<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_selected_host().cloned() {
            // Clone to avoid borrow issue
            tracing::info!("Enter pressed, selected host: {:?}", selected_host.alias);

            // 1. Stop TUI, restore terminal to normal state
            disable_raw_mode().context("Failed to disable raw mode for SSH")?;
            let mut stdout = std::io::stdout();
            execute!(
                &mut stdout,
                LeaveAlternateScreen,
                DisableMouseCapture // Important: If you are using mouse capture
            )
            .context("Failed to leave alternate screen for SSH")?;
            terminal
                .show_cursor()
                .context("Failed to show cursor for SSH")?;

            // Clear screen before running ssh to clean up ssh output
            // (Optional, ssh usually manages the screen itself)
            // print!("\x1B[2J\x1B[1;1H");
            // io::stdout().flush().unwrap();

            // 2. Execute SSH command
            match ssh_service::connect_to_host(&selected_host) {
                Ok(_) => {
                    tracing::info!("SSH session for {} ended.", selected_host.alias);
                }
                Err(e) => {
                    // This error will be logged, ssh usually displays its own error
                    tracing::error!("SSH connection to {} failed: {:?}", selected_host.alias, e);
                    // You can display a short error message on the TUI after returning
                    // app.status_message = Some(format!("SSH failed: {}", e));
                }
            }

            // 3. Restore TUI
            // Important: must clear terminal to redraw TUI after ssh ends
            terminal
                .clear()
                .context("Failed to clear terminal post-SSH")?; // Remove ssh traces
            enable_raw_mode().context("Failed to re-enable raw mode post-SSH")?;
            let mut stdout = std::io::stdout();
            crossterm::execute!(
                &mut stdout,
                EnterAlternateScreen,
                EnableMouseCapture // If you are using mouse capture
            )
            .context("Failed to re-enter alternate screen post-SSH")?;
            // No need to call terminal.show_cursor() here if TUI doesn't use cursor

            // Request to redraw the entire UI
            terminal.draw(|f| ui::draw::<B>(f, self))?;
        } else {
            tracing::warn!("Enter pressed but no host selected.");
        }
        Ok(())
    }

    pub fn handle_key_q(&mut self) -> Result<()> {
        self.should_quit = true;
        Ok(())
    }

    pub fn handle_key_e(&mut self) -> Result<()> {
        // Get the path to the hosts file
        let hosts_path = self.config_manager.get_hosts_path();

        // Create the file if it doesn't exist
        if !hosts_path.exists() {
            if let Some(parent) = hosts_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&hosts_path, "")?;
        }

        // TODO: Can use nvim, vim, nano if exist instead of default text editor
        // Open the file with the default text editor
        if let Err(e) = open::that(&hosts_path) {
            tracing::error!("Failed to open editor: {}", e);
            return Err(anyhow::anyhow!("Failed to open editor: {}", e));
        }

        // Reload the config after the editor is closed
        self.load_custom_hosts()?;

        Ok(())
    }

    pub fn handle_key_esc(&mut self) -> Result<()> {
        self.input_mode = InputMode::Normal;
        Ok(())
    }
}

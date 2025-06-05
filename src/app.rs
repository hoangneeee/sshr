use crate::config::{AppConfig, ConfigManager};
use crate::models::SshHost;
use crate::ui;
use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::time::Duration;
use tokio::process::Command as TokioCommand;

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
    #[allow(dead_code)]
    pub app_config: AppConfig,
    pub input_mode: InputMode,
    pub is_connecting: bool,
    pub status_message: Option<(String, std::time::Instant)>,
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
            is_connecting: false,
            status_message: None,
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

    pub fn clear_status_message(&mut self) {
        self.status_message = None;
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
    pub async fn handle_key_enter_async<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some(selected_host) = self.get_selected_host().cloned() {
            tracing::info!("Enter pressed, selected host: {:?}", selected_host.alias);

            // Show loading UI with animation
            self.is_connecting = true;
            self.status_message = Some((
                format!("Connecting to {}...", selected_host.alias),
                std::time::Instant::now(),
            ));

            // Draw loading UI multiple times với animation effect
            for i in 0..10 {
                terminal.draw(|f| ui::draw::<B>(f, self))?;
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Update connecting message với animation
                let dots = ".".repeat((i % 4) + 1);
                self.status_message = Some((
                    format!("Connecting to {}{}", selected_host.alias, dots),
                    std::time::Instant::now(),
                ));
            }

            // Sau khi hiển thị loading, chuyển sang SSH mode
            self.transition_to_ssh_mode(terminal, &selected_host)
                .await?;
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
        self.load_all_hosts()?;

        Ok(())
    }

    pub fn handle_key_esc(&mut self) -> Result<()> {
        self.input_mode = InputMode::Normal;
        Ok(())
    }

    async fn transition_to_ssh_mode<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        host: &SshHost,
    ) -> Result<()> {
        // 1. Disable TUI
        disable_raw_mode().context("Failed to disable raw mode for SSH")?;
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, LeaveAlternateScreen, DisableMouseCapture)
            .context("Failed to leave alternate screen for SSH")?;
        terminal
            .show_cursor()
            .context("Failed to show cursor for SSH")?;

        // 2. Execute SSH
        let ssh_result = self.execute_ssh_async(host).await;

        // 3. Restore TUI
        self.restore_tui_mode(terminal).await?;

        // 4. Handle result
        self.handle_ssh_result(ssh_result, host);

        Ok(())
    }

    async fn execute_ssh_async(&self, host: &SshHost) -> Result<()> {
        let port_str = host.port.unwrap_or(22).to_string();
        let connection_str = format!("{}@{}", host.user, host.host);

        tracing::info!(
            "Attempting to connect: ssh {} -p {}",
            connection_str,
            port_str
        );

        let mut cmd = TokioCommand::new("ssh");
        cmd.arg(&connection_str)
            .arg("-p")
            .arg(&port_str)
            .arg("-o")
            .arg("ConnectTimeout=30")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());

        let status = cmd
            .status()
            .await
            .with_context(|| format!("Failed to execute SSH command for {}", host.alias))?;

        if !status.success() {
            tracing::error!("SSH command finished with status: {}", status);
        }

        Ok(())
    }

    fn handle_ssh_result(&mut self, result: Result<()>, host: &SshHost) {
        match result {
            Ok(_) => {
                tracing::info!("SSH session for {} ended.", host.alias);
                self.status_message = Some((
                    format!("SSH session to {} completed successfully", host.alias),
                    std::time::Instant::now(),
                ));
            }
            Err(e) => {
                tracing::error!("SSH connection to {} failed: {:?}", host.alias, e);
                self.status_message = Some((
                    format!("SSH connection failed: {}", e),
                    std::time::Instant::now(),
                ));
            }
        }
    }

    async fn restore_tui_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        enable_raw_mode().context("Failed to re-enable raw mode post-SSH")?;
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to re-enter alternate screen post-SSH")?;

        terminal
            .clear()
            .context("Failed to clear terminal post-SSH")?;
        self.is_connecting = false;
        terminal.draw(|f| ui::draw::<B>(f, self))?;

        Ok(())
    }
}

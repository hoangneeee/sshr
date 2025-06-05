use crate::config::{AppConfig, ConfigManager};
use crate::models::SshHost;
use crate::ui;
use anyhow::{Context, Result};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use open;
use ratatui::{backend::Backend, Terminal};
use std::collections::HashSet;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::{fs, thread};

#[derive(Debug, Clone)]
pub enum SshEvent {
    Connecting,
    Connected,
    Error(String),
    Disconnected,
}

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
    pub ssh_receiver: Option<Receiver<SshEvent>>,
    pub ssh_ready_for_terminal: bool,
}

impl Default for App {
    fn default() -> Self {
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
            ssh_receiver: None,
            ssh_ready_for_terminal: false,
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
    pub fn handle_key_enter<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_selected_host().cloned() {
            tracing::info!("Enter pressed, selected host: {:?}", selected_host.alias);

            // Tạo channel để communication
            let (sender, receiver) = mpsc::channel::<SshEvent>();
            self.ssh_receiver = Some(receiver);

            // Set connecting state
            self.is_connecting = true;
            self.ssh_ready_for_terminal = false;
            self.status_message = Some((
                format!("Connecting to {}...", selected_host.alias),
                Instant::now(),
            ));

            // Spawn SSH thread
            let host_clone = selected_host.clone();
            thread::spawn(move || {
                Self::ssh_thread_worker(sender, host_clone);
            });

            // Redraw UI để hiển thị loading
            terminal.draw(|f| ui::draw::<B>(f, self))?;
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

    fn transition_to_ssh_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Disable TUI mode
        disable_raw_mode().context("Failed to disable raw mode for SSH")?;
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, LeaveAlternateScreen, DisableMouseCapture)
            .context("Failed to leave alternate screen for SSH")?;
        terminal
            .show_cursor()
            .context("Failed to show cursor for SSH")?;

        tracing::info!("TUI disabled for SSH mode - main thread will suspend polling");
        Ok(())
    }

    fn restore_tui_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Re-enable TUI mode
        enable_raw_mode().context("Failed to re-enable raw mode post-SSH")?;
        let mut stdout = std::io::stdout();
        execute!(&mut stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("Failed to re-enter alternate screen post-SSH")?;

        terminal
            .clear()
            .context("Failed to clear terminal post-SSH")?;
        tracing::info!("TUI restored after SSH session - resuming main thread polling");
        Ok(())
    }

    // Worker function run in SSH thread
    fn ssh_thread_worker(sender: Sender<SshEvent>, host: SshHost) {
        tracing::info!("SSH thread started for host: {}", host.alias);
        
        // Send event connecting
        if sender.send(SshEvent::Connecting).is_err() {
            tracing::error!("Failed to send Connecting event");
            return;
        }

        // Perform SSH connection test first
        match Self::test_ssh_connection(&host) {
            Ok(_) => {
                tracing::info!("SSH connection test successful for {}", host.alias);
                
                // If connection test success, send Connected event
                if sender.send(SshEvent::Connected).is_ok() {
                    // Wait a little bit for main thread to process transition
                    thread::sleep(Duration::from_millis(200));

                    // Execute SSH connection (this will block until SSH session ends)
                    tracing::info!("Starting SSH session for {}", host.alias);
                    match Self::execute_ssh_blocking(&host) {
                        Ok(_) => {
                            tracing::info!("SSH session ended normally for {}", host.alias);
                            let _ = sender.send(SshEvent::Disconnected);
                        }
                        Err(e) => {
                            tracing::error!("SSH session error for {}: {}", host.alias, e);
                            let _ = sender.send(SshEvent::Error(e.to_string()));
                        }
                    }
                } else {
                    tracing::error!("Failed to send Connected event");
                }
            }
            Err(e) => {
                tracing::error!("SSH connection test failed for {}: {}", host.alias, e);
                let _ = sender.send(SshEvent::Error(format!("Connection test failed: {}", e)));
            }
        }
        
        tracing::info!("SSH thread ending for host: {}", host.alias);
    }

    // Test SSH connection trước khi thực sự connect
    fn test_ssh_connection(host: &SshHost) -> Result<()> {
        use std::process::Command;

        let port_str = host.port.unwrap_or(22).to_string();

        tracing::info!("Testing SSH connection to {}@{}:{}", host.user, host.host, port_str);

        // Test connection with short timeout
        let output = Command::new("ssh")
            .arg(format!("{}@{}", host.user, host.host))
            .arg("-p")
            .arg(&port_str)
            .arg("-o")
            .arg("ConnectTimeout=5")
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg("-o")
            .arg("LogLevel=ERROR") // Reduce verbose output
            .arg("exit")
            .output()
            .context("Failed to test SSH connection")?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow::anyhow!("SSH connection test failed: {}", stderr.trim()))
        }
    }

    // Execute SSH connection (blocking) - This gives complete control to SSH
    fn execute_ssh_blocking(host: &SshHost) -> Result<()> {
        use std::process::Command;

        let port_str = host.port.unwrap_or(22).to_string();
        let connection_str = format!("{}@{}", host.user, host.host);

        tracing::info!("Executing SSH: ssh {} -p {}", connection_str, port_str);

        // Execute SSH with full control of terminal
        let status = Command::new("ssh")
            .arg(&connection_str)
            .arg("-p")
            .arg(&port_str)
            .arg("-o")
            .arg("ConnectTimeout=30")
            .arg("-o")
            .arg("ServerAliveInterval=60")
            .arg("-o")
            .arg("ServerAliveCountMax=3")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("Failed to execute SSH command")?;

        if status.success() {
            tracing::info!("SSH command completed successfully");
            Ok(())
        } else {
            let error_msg = format!("SSH command failed with status: {}", status);
            tracing::error!("{}", error_msg);
            Err(anyhow::anyhow!(error_msg))
        }
    }

    // Process SSH events from channel
    pub fn process_ssh_events<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<bool> {
        if let Some(receiver) = &self.ssh_receiver {
            // Non-blocking receive
            if let Ok(event) = receiver.try_recv() {
                match event {
                    SshEvent::Connecting => {
                        self.status_message =
                            Some(("Testing connection...".to_string(), Instant::now()));
                        return Ok(false);
                    }
                    SshEvent::Connected => {
                        self.status_message = Some((
                            "Connection successful! Launching SSH...".to_string(),
                            Instant::now(),
                        ));
                        
                        // Transition to SSH terminal mode
                        self.transition_to_ssh_mode(terminal)?;
                        self.ssh_ready_for_terminal = true;
                        
                        return Ok(false);
                    }
                    SshEvent::Error(err) => {
                        tracing::error!("SSH error: {}", err);
                        self.is_connecting = false;
                        self.ssh_ready_for_terminal = false;
                        self.ssh_receiver = None;
                        self.status_message = Some((format!("SSH Error: {}", err), Instant::now()));
                        return Ok(false);
                    }
                    SshEvent::Disconnected => {
                        tracing::info!("SSH session disconnected, restoring TUI");
                        
                        // SSH session ended, restore TUI
                        self.restore_tui_mode(terminal)?;
                        self.is_connecting = false;
                        self.ssh_ready_for_terminal = false;
                        self.ssh_receiver = None;
                        self.status_message =
                            Some(("SSH session ended".to_string(), Instant::now()));
                        return Ok(true); // Indicate we need to redraw
                    }
                }
            }
        }
        Ok(false)
    }
}
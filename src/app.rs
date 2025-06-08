use crate::config::ConfigManager;
use crate::models::SshHost;
use crate::sftp_logic::{AppSftpState};
use crate::ui;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::app_event::{SftpEvent, SshEvent};
use open;
use ratatui::{backend::Backend, widgets::ListState, Terminal};
use std::collections::HashSet;
use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::{fs, thread};
use ui::hosts_list::draw;

#[derive(Debug)]
pub enum InputMode {
    Normal,
    Search,
    Sftp,
}

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub hosts: Vec<SshHost>,
    pub selected: usize,
    pub ssh_config_path: PathBuf,
    pub config_manager: ConfigManager,
    pub input_mode: InputMode,

    pub status_message: Option<(String, std::time::Instant)>,

    // SSH Mode
    pub is_connecting: bool,
    pub ssh_ready_for_terminal: bool,
    pub ssh_receiver: Option<Receiver<SshEvent>>,

    // SFTP Mode
    pub is_sftp_loading: bool,
    pub sftp_ready_for_terminal: bool,
    pub sftp_receiver: Option<Receiver<SftpEvent>>,
    pub sftp_state: Option<AppSftpState>,

    // Search Mode
    pub search_query: String,
    pub filtered_hosts: Vec<usize>, // Indices of filtered hosts
    pub search_selected: usize,

    pub host_list_state: ListState,
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
            input_mode: InputMode::Normal,
            is_connecting: false,
            status_message: None,
            // SSH
            ssh_receiver: None,
            ssh_ready_for_terminal: false,
            // SFTP
            sftp_receiver: None,
            sftp_ready_for_terminal: false,
            is_sftp_loading: false,
            sftp_state: None,

            // Search
            search_query: String::new(),
            filtered_hosts: Vec::new(),
            search_selected: 0,
            host_list_state: ListState::default(),
        }
    }
}

impl App {
    pub fn new() -> Result<Self> {
        let mut app = Self::default();
        app.load_all_hosts().context("Failed to load hosts")?;
        app.host_list_state.select(Some(app.selected));
        Ok(app)
    }

    pub fn load_all_hosts(&mut self) -> Result<()> {
        self.load_ssh_config()
            .context("Failed to load SSH config")?;
        self.load_custom_hosts()
            .context("Failed to load custom hosts")?;
        self.handle_duplicate_hosts();

        if self.hosts.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.hosts.len() {
            self.selected = self.hosts.len() - 1;
        }
        self.filter_hosts();
        Ok(())
    }

    pub fn load_ssh_config(&mut self) -> Result<()> {
        // Clear only system-loaded hosts to allow custom hosts to persist across reloads
        self.hosts.retain(|h| h.group.is_some()); // Retain only custom hosts (those with a group)

        if !self.ssh_config_path.exists() {
            tracing::warn!(
                "System SSH config file not found at {:?}",
                self.ssh_config_path
            );
            return Ok(());
        }

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
                    // Check if a host with this alias already exists from custom config
                    if !self.hosts.iter().any(|h| h.alias == host.alias) {
                        self.hosts.push(host);
                    } else {
                        tracing::warn!(
                            "Skipping SSH config host '{}' as it's duplicated by a custom host.",
                            host.alias
                        );
                    }
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
            if !self.hosts.iter().any(|h| h.alias == host.alias) {
                self.hosts.push(host);
            } else {
                tracing::warn!(
                    "Skipping SSH config host '{}' as it's duplicated by a custom host.",
                    host.alias
                );
            }
        }

        tracing::info!(
            "Loaded {} hosts from SSH config (after merging with custom hosts)",
            self.hosts.len()
        );

        // Check reachability for each host
        for host in &mut self.hosts {
            if host.group.is_none() {
                // Only update description for system hosts if not already set by custom
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
        }

        Ok(())
    }

    // Load custome hosts from hosts.toml
    pub fn load_custom_hosts(&mut self) -> Result<()> {
        match self.config_manager.load_hosts() {
            Ok(mut custom_hosts) => {
                // Prepend custom hosts to the list, as they often take precedence or are more frequently used.
                // Filter out any custom hosts that might have duplicate aliases with existing system hosts
                // (though `handle_duplicate_hosts` will catch this later, this is a good defensive step).
                let mut existing_aliases: HashSet<String> =
                    self.hosts.iter().map(|h| h.alias.clone()).collect();
                custom_hosts.retain(|host| {
                    if existing_aliases.contains(&host.alias) {
                        tracing::warn!("Skipping custom host '{}' due to duplicate alias. System host will be used.", host.alias);
                        false
                    } else {
                        existing_aliases.insert(host.alias.clone());
                        true
                    }
                });

                // Insert custom hosts at the beginning
                self.hosts.splice(0..0, custom_hosts);
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to load custom hosts: {}", e);
                // Don't propagate error, just log it, so app can still run.
                Ok(())
            }
        }
    }

    // Remove duplicate hosts
    pub fn handle_duplicate_hosts(&mut self) {
        let mut seen_aliases = HashSet::new();
        let mut unique_hosts = Vec::new();
        for host in self.hosts.drain(..) {
            // Use drain to consume self.hosts
            if seen_aliases.contains(&host.alias) {
                tracing::warn!("Duplicate alias found: {}", host.alias);
            } else {
                seen_aliases.insert(host.alias.clone());
                unique_hosts.push(host);
            }
        }
        self.hosts = unique_hosts;
    }

    // Get selected host
    pub fn get_selected_host(&self) -> Option<&SshHost> {
        if self.hosts.is_empty() {
            None
        } else {
            // Adjust selected index if it's out of bounds after filtering/reloading
            let current_selected = match self.input_mode {
                InputMode::Normal => self.selected,
                InputMode::Search => self
                    .filtered_hosts
                    .get(self.search_selected)
                    .copied()
                    .unwrap_or(0),
                InputMode::Sftp => self.selected, // SFTP doesn't change overall host selection
            };
            self.hosts.get(current_selected)
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
        let total_hosts = self.hosts.len();
        self.selected = (self.selected + 1) % total_hosts;
        self.host_list_state.select(Some(self.selected));
    }

    pub fn select_previous(&mut self) {
        if self.hosts.is_empty() {
            return;
        }
        let total_hosts = self.hosts.len();
        self.selected = (self.selected + total_hosts - 1) % total_hosts;
        self.host_list_state.select(Some(self.selected));
    }

    // Handle key
    pub fn handle_key_enter<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_current_selected_host().cloned() {
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
            terminal.draw(|f| draw::<B>(f, self))?;
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

        tracing::info!(
            "Testing SSH connection to {}@{}:{}",
            host.user,
            host.host,
            port_str
        );

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
            Err(anyhow::anyhow!(
                "SSH connection test failed: {}",
                stderr.trim()
            ))
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

    // Search logic
    pub fn filter_hosts(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_hosts = (0..self.hosts.len()).collect();
        } else {
            self.filtered_hosts = self
                .hosts
                .iter()
                .enumerate()
                .filter(|(_, host)| {
                    host.alias
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase())
                        || host
                            .host
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                        || host
                            .user
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                })
                .map(|(i, _)| i)
                .collect();
        }

        // Reset selection if current selection is out of bounds
        if self.search_selected >= self.filtered_hosts.len() {
            self.search_selected = 0;
        }

        match self.input_mode {
            InputMode::Normal => self.host_list_state.select(Some(self.selected)),
            InputMode::Search => self.host_list_state.select(Some(self.search_selected)),
            InputMode::Sftp => {}
        }
    }

    // pub fn get_filtered_host(&self, index: usize) -> Option<&SshHost> {
    //     self.filtered_hosts
    //         .get(index)
    //         .and_then(|&host_index| self.hosts.get(host_index))
    // }

    pub fn get_current_selected_host(&self) -> Option<&SshHost> {
        match self.input_mode {
            InputMode::Normal => self.get_selected_host(),
            InputMode::Search => {
                if let Some(&host_index) = self.filtered_hosts.get(self.search_selected) {
                    self.hosts.get(host_index)
                } else {
                    None
                }
            }
            InputMode::Sftp => None,
        }
    }

    pub fn search_select_next(&mut self) {
        if self.filtered_hosts.is_empty() {
            return;
        }
        if self.search_selected >= self.filtered_hosts.len() - 1 {
            self.search_selected = 0;
        } else {
            self.search_selected += 1;
        }
        self.host_list_state.select(Some(self.search_selected));
    }

    pub fn search_select_previous(&mut self) {
        if self.filtered_hosts.is_empty() {
            return;
        }
        if self.search_selected == 0 {
            self.search_selected = self.filtered_hosts.len() - 1;
        } else {
            self.search_selected -= 1;
        }
        self.host_list_state.select(Some(self.search_selected));
    }

    // Clear search and return to normal mode
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.input_mode = InputMode::Normal;
        self.filtered_hosts = (0..self.hosts.len()).collect();
        self.search_selected = 0;
        self.host_list_state.select(Some(self.selected));
    }

    // Enter search mode
    pub fn enter_search_mode(&mut self) {
        self.input_mode = InputMode::Search;
        self.search_query.clear();
        self.filter_hosts();
    }

    /// Enter SFTP mode with the currently selected host
    pub fn enter_sftp_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_current_selected_host().cloned() {
            // Create channel to communication
            let (sender, receiver) = mpsc::channel::<SftpEvent>();
            self.sftp_receiver = Some(receiver);

            // Turn on loading status
            self.is_sftp_loading = true;
            self.sftp_ready_for_terminal = true;
            self.status_message = Some((
                format!("Initializing SFTP for {}...", selected_host.alias),
                Instant::now(),
            ));

            // Initialize AppSftpState asynchronously
            let host_clone = selected_host.clone();
            thread::spawn(move || {
                Self::sftp_thread_worker(sender, host_clone);
            });

            // Redraw UI to show loading
            terminal.draw(|f| draw::<B>(f, self))?;
        }
        Ok(())
    }

    pub fn exit_sftp_mode(&mut self) {
        tracing::info!("Exiting SFTP mode");
        self.sftp_state = None;
        self.input_mode = InputMode::Normal;
        self.is_sftp_loading = false;
        self.sftp_ready_for_terminal = false;
        self.status_message = Some(("Exited SFTP mode".to_string(), Instant::now()));
    }

    pub async fn handle_sftp_key<B: Backend>(
        &mut self,
        key: KeyEvent,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        if let Some(sftp_state) = &mut self.sftp_state {
            match key.code {
                KeyCode::Char('q') => {
                    self.exit_sftp_mode();
                }
                KeyCode::Up => {
                    sftp_state.navigate_up();
                }
                KeyCode::Down => {
                    sftp_state.navigate_down();
                }
                KeyCode::Enter => {
                    if let Err(e) = sftp_state.open_selected() {
                        sftp_state.set_status_message(&format!("Error: {}", e));
                    }
                }
                KeyCode::Backspace => {
                    if let Err(e) = sftp_state.open_selected() {
                        // Assuming Backspace goes to parent
                        sftp_state.set_status_message(&format!("Error: {}", e));
                    }
                }
                KeyCode::Tab => {
                    sftp_state.switch_panel();
                }
                KeyCode::Char('u') => {
                    if let Err(e) = sftp_state.upload_file().await {
                        sftp_state.set_status_message(&format!("Upload error: {}", e));
                    }
                }
                KeyCode::Char('d') => {
                    if let Err(e) = sftp_state.download_file().await {
                        sftp_state.set_status_message(&format!("Download error: {}", e));
                    }
                }
                KeyCode::Char('r') => {
                    if let Err(e) = sftp_state.refresh_local() {
                        sftp_state.set_status_message(&format!("Local refresh error: {}", e));
                    }
                    if let Err(e) = sftp_state.refresh_remote() {
                        sftp_state.set_status_message(&format!("Remote refresh error: {}", e));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    // Process SFTP events from channel
    pub fn process_sftp_events<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<bool> {
        if let Some(receiver) = &self.sftp_receiver {
            // Non-blocking receive
            if let Ok(event) = receiver.try_recv() {
                match event {
                    SftpEvent::PreConnected(sftp_state) => {
                        self.sftp_state = Some(sftp_state);
                        self.input_mode = InputMode::Sftp;
                        self.status_message = Some((
                            format!(
                                "SFTP mode active for {}",
                                self.sftp_state.as_ref().unwrap().ssh_host
                            ),
                            Instant::now(),
                        ));
                        return Ok(false);
                    }
                    SftpEvent::Connecting => {
                        self.status_message =
                            Some(("Testing connection...".to_string(), Instant::now()));
                        return Ok(false);
                    }
                    SftpEvent::Connected => {
                        self.status_message = Some((
                            "Connection successful! Launching SFTP...".to_string(),
                            Instant::now(),
                        ));
                        self.sftp_ready_for_terminal = true;
                        return Ok(false);
                    }
                    SftpEvent::Error(err) => {
                        tracing::error!("SFTP error: {}", err);
                        self.is_sftp_loading = false;
                        self.sftp_ready_for_terminal = false;
                        self.sftp_receiver = None;
                        self.status_message =
                            Some((format!("SFTP Error: {}", err), Instant::now()));
                        return Ok(false);
                    }
                    SftpEvent::Disconnected => {
                        tracing::info!("SFTP session disconnected, restoring TUI");

                        self.is_sftp_loading = false;
                        self.sftp_ready_for_terminal = false;
                        self.sftp_receiver = None;
                        self.status_message =
                            Some(("SFTP session ended".to_string(), Instant::now()));
                        return Ok(true); // Indicate we need to redraw
                    }
                    // _ => {}
                }
            }
        }
        Ok(false)
    }

    // Worker function run in SFTP thread
    fn sftp_thread_worker(sender: Sender<SftpEvent>, host: SshHost) {
        tracing::info!("SFTP thread started for host: {}", host.alias);

        // Send event connecting
        if sender.send(SftpEvent::Connecting).is_err() {
            tracing::error!("Failed to send Connecting event");
            return;
        }

        // Perform SSH connection test first
        match AppSftpState::new(&host.user, &host.host, host.port.unwrap_or(22)) {
            Ok(sftp_state) => {
                tracing::info!("SFTP connection test successful for {}", host.alias);

                // Send PreConnected event
                if sender.send(SftpEvent::PreConnected(sftp_state)).is_err() {
                    tracing::error!("Failed to send PreConnected event");
                    return;
                }
                // If connection test success, send Connected event
                if sender.send(SftpEvent::Connected).is_ok() {
                    // Wait a little bit for main thread to process transition
                    thread::sleep(Duration::from_millis(200));
                    // Execute SSH connection (this will block until SSH session ends)
                    tracing::info!("Starting SFTP session for {}", host.alias);
                } else {
                    tracing::error!("Failed to send Connected event");
                }
            }
            Err(e) => {
                tracing::error!("SFTP connection test failed for {}: {}", host.alias, e);
                let _ = sender.send(SftpEvent::Error(format!("Connection test failed: {}", e)));
            }
        }

        tracing::info!("SFTP thread ending for host: {}", host.alias);
    }
}

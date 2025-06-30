use crate::app::App;
use crate::config::ConfigManager;
use crate::models::SshHost;
use crate::sftp_logic::AppSftpState;
use crate::ui;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use crate::app_event::{SftpEvent, SshEvent};
use ratatui::{backend::Backend, widgets::ListState, Terminal};
use std::path::PathBuf;
use std::{thread};
use ui::hosts_list::draw;

use crate::app::types::{ActivePanel, InputMode};

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
            selected_host: 0,
            selected_group: 0,
            active_panel: ActivePanel::Groups,
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

            // Group State
            groups: Vec::new(),
            hosts_in_current_group: Vec::new(),

            host_list_state: ListState::default(),
            group_list_state: ListState::default(),
        }
    }
}

impl App {
    pub fn new() -> Result<Self> {
        let mut app = Self::default();
        app.load_all_hosts().context("Failed to load hosts")?;
        app.host_list_state.select(Some(app.selected_host));
        Ok(app)
    }


    pub fn clear_status_message(&mut self) {
        self.status_message = None;
    }

    pub fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Groups => ActivePanel::Hosts,
            ActivePanel::Hosts => ActivePanel::Groups,
        };

        tracing::info!("Switched to {:?} panel", self.active_panel);

        // When switching to Hosts panel, ensure selected_host is within bounds
        if self.active_panel == ActivePanel::Hosts && !self.hosts_in_current_group.is_empty() {
            self.selected_host = std::cmp::min(self.selected_host, self.hosts_in_current_group.len().saturating_sub(1));
            self.host_list_state.select(Some(self.selected_host));
        }
    }

    pub fn update_hosts_for_selected_group(&mut self) {
        if self.groups.is_empty() {
            self.hosts_in_current_group.clear();
            return;
        }

        let current_group = &self.groups[self.selected_group];
        self.hosts_in_current_group = self.hosts
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

    // Improve navigation
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
    pub fn ssh_thread_worker(sender: Sender<SshEvent>, host: SshHost) {
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
            InputMode::Normal => self.host_list_state.select(Some(self.selected_host)),
            InputMode::Search => self.host_list_state.select(Some(self.search_selected)),
            InputMode::Sftp => {}
        }
    }

    pub fn get_current_selected_host(&self) -> Option<&SshHost> {
        match self.input_mode {
            InputMode::Normal => {
                // In normal mode, use the filtered hosts_in_current_group
                self.hosts_in_current_group
                    .get(self.selected_host)
                    .and_then(|&idx| self.hosts.get(idx))
            }
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
        self.selected_host = self.selected_host.saturating_sub(1);
        self.host_list_state.select(Some(self.selected_host));
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

    pub async fn handle_sftp_key(&mut self, key: KeyEvent) -> Result<()> {
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
    pub fn process_sftp_events<B: Backend>(&mut self, _terminal: &mut Terminal<B>) -> Result<bool> {
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
                    } // _ => {}
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

//! Screen abstraction for the TUI.
//!
//! `AppScreen` represents the currently-visible screen. The main loop pushes
//! and pops screens in response to user actions and worker events. Each
//! screen owns its dispatch logic; `App` only holds shared state.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::Backend, Frame, Terminal};
use std::time::Instant;

use crate::app::hosts_state::ActivePanel;
use crate::app::session::{SftpStage, SshStage};
use crate::app::App;
use crate::app_event::{SftpEvent, SshEvent, TransferEvent};
use crate::models::SshHost;
use crate::sftp_logic::types::{DownloadProgress, UploadProgress};
use crate::ui::{hosts_list, sftp};

/// Result of handling a key or polling worker events on the active screen.
pub enum ScreenAction {
    /// No screen change.
    None,
    /// Quit the application.
    Quit,
    /// Push a new screen on top of the current one.
    Push(AppScreen),
    /// Pop the current screen, returning to whatever's underneath.
    Pop,
}

/// The closed set of screens in the TUI.
#[derive(Debug)]
pub enum AppScreen {
    Hosts(HostsScreen),
    Sftp(SftpScreen),
    /// Placeholder while the SSH child process owns the terminal.
    SshActive(SshActiveScreen),
}

/// Inner mode of the host browser screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostsMode {
    Normal,
    Search,
}

#[derive(Debug)]
pub struct HostsScreen {
    pub mode: HostsMode,
}

impl Default for HostsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl HostsScreen {
    pub fn new() -> Self {
        Self {
            mode: HostsMode::Normal,
        }
    }
}

#[derive(Debug, Default)]
pub struct SftpScreen;

#[derive(Debug, Default)]
pub struct SshActiveScreen;

impl AppScreen {
    pub fn draw(&mut self, f: &mut Frame, app: &mut App) {
        match self {
            Self::Hosts(s) => {
                hosts_list::draw(f, app, s.mode == HostsMode::Search);
            }
            Self::Sftp(_) => {
                let theme = app.ctx.theme.clone();
                if let Some(state) = app.session.sftp_data_mut() {
                    sftp::draw_sftp(f, state, &theme);
                } else {
                    // SFTP data unexpectedly gone — fall back to host list so
                    // the user isn't stuck on a blank screen.
                    hosts_list::draw(f, app, false);
                }
            }
            Self::SshActive(_) => {
                // TUI is suspended while the foreground ssh child runs.
            }
        }
    }

    /// Whether the main loop should poll keyboard input on this screen.
    /// Returns false while the TUI is suspended for a foreground process.
    pub fn wants_input(&self) -> bool {
        !matches!(self, Self::SshActive(_))
    }

    /// Non-blocking poll of worker channels relevant to this screen.
    pub fn poll<B: Backend>(
        &mut self,
        app: &mut App,
        terminal: &mut Terminal<B>,
    ) -> Result<ScreenAction> {
        match self {
            Self::Hosts(_) => poll_hosts(app, terminal),
            Self::Sftp(_) => poll_sftp(app),
            Self::SshActive(_) => Ok(ScreenAction::None),
        }
    }

    /// Blocking await for screens that have nothing else to do
    /// (currently: SshActive, which sits idle until SSH exits).
    pub async fn await_blocking<B: Backend>(
        &mut self,
        app: &mut App,
        terminal: &mut Terminal<B>,
    ) -> Result<ScreenAction> {
        match self {
            Self::SshActive(_) => await_ssh_end(app, terminal).await,
            _ => Ok(ScreenAction::None),
        }
    }

    pub fn handle_key<B: Backend>(
        &mut self,
        key: KeyEvent,
        app: &mut App,
        _terminal: &mut Terminal<B>,
    ) -> Result<ScreenAction> {
        match self {
            Self::Hosts(s) => s.handle_key(key, app),
            Self::Sftp(s) => s.handle_key(key, app),
            Self::SshActive(_) => Ok(ScreenAction::None),
        }
    }
}

// -----------------------------------------------------------------------------
// HostsScreen key handling
// -----------------------------------------------------------------------------

impl HostsScreen {
    pub fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Result<ScreenAction> {
        match self.mode {
            HostsMode::Normal => self.handle_normal_key(key, app),
            HostsMode::Search => self.handle_search_key(key, app),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent, app: &mut App) -> Result<ScreenAction> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Ok(ScreenAction::Quit),
            KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => Ok(ScreenAction::Quit),
            KeyCode::Char('s') => {
                self.mode = HostsMode::Search;
                app.search.clear();
                app.hosts.switch_to_hosts();
                app.filter_hosts();
                Ok(ScreenAction::None)
            }
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    app.hosts.cycle_group(false);
                } else {
                    app.hosts.switch_panel();
                }
                Ok(ScreenAction::None)
            }
            KeyCode::Right => {
                match app.hosts.active_panel {
                    ActivePanel::Groups => app.hosts.cycle_group(true),
                    ActivePanel::Hosts => app.hosts.select_next(),
                }
                Ok(ScreenAction::None)
            }
            KeyCode::Left => {
                match app.hosts.active_panel {
                    ActivePanel::Groups => app.hosts.cycle_group(false),
                    ActivePanel::Hosts => app.hosts.select_previous(),
                }
                Ok(ScreenAction::None)
            }
            KeyCode::Char('f') => {
                if let Some(host) = host_for_mode(app, self.mode).cloned() {
                    app.connect_sftp(host);
                }
                Ok(ScreenAction::None)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.hosts.select_previous();
                Ok(ScreenAction::None)
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.hosts.select_next();
                Ok(ScreenAction::None)
            }
            KeyCode::Char('e') => {
                if let Err(e) = app.open_hosts_editor() {
                    tracing::error!("Failed to open editor: {}", e);
                    app.ui.status_message =
                        Some((format!("Failed to open editor: {}", e), Instant::now()));
                }
                Ok(ScreenAction::None)
            }
            KeyCode::Esc => Ok(ScreenAction::None),
            KeyCode::Enter => {
                if let Some(host) = host_for_mode(app, self.mode).cloned() {
                    app.connect_ssh(host);
                }
                Ok(ScreenAction::None)
            }
            KeyCode::Char('r') => {
                tracing::info!("Reloading SSH config...");
                if let Err(e) = app.load_all_hosts() {
                    tracing::error!("Failed to reload SSH config: {}", e);
                    app.ui.status_message =
                        Some((format!("Reload failed: {}", e), Instant::now()));
                } else {
                    app.ui.status_message =
                        Some(("Config reloaded successfully".to_string(), Instant::now()));
                }
                Ok(ScreenAction::None)
            }
            _ => Ok(ScreenAction::None),
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent, app: &mut App) -> Result<ScreenAction> {
        match key.code {
            KeyCode::Char(c) => {
                app.search.query.push(c);
                app.filter_hosts();
                Ok(ScreenAction::None)
            }
            KeyCode::Backspace | KeyCode::Delete => {
                app.search.query.pop();
                app.filter_hosts();
                Ok(ScreenAction::None)
            }
            KeyCode::Enter => {
                if let Some(host) = host_for_mode(app, self.mode).cloned() {
                    app.connect_ssh(host);
                }
                self.exit_search(app);
                Ok(ScreenAction::None)
            }
            KeyCode::Esc => {
                self.exit_search(app);
                Ok(ScreenAction::None)
            }
            KeyCode::Up => {
                app.search_select_previous();
                Ok(ScreenAction::None)
            }
            KeyCode::Down => {
                app.search_select_next();
                Ok(ScreenAction::None)
            }
            _ => Ok(ScreenAction::None),
        }
    }

    fn exit_search(&mut self, app: &mut App) {
        self.mode = HostsMode::Normal;
        app.search.clear();
        app.hosts.host_list_state.select(Some(app.hosts.selected_host));
    }
}

// -----------------------------------------------------------------------------
// SftpScreen key handling
// -----------------------------------------------------------------------------

impl SftpScreen {
    pub fn handle_key(&mut self, key: KeyEvent, app: &mut App) -> Result<ScreenAction> {
        if key.code == KeyCode::Char('q') {
            tracing::info!("Exiting SFTP mode");
            app.session.reset();
            app.ui.status_message = Some(("Exited SFTP mode".to_string(), Instant::now()));
            return Ok(ScreenAction::Pop);
        }

        let Some(sftp_state) = app.session.sftp_data_mut() else {
            return Ok(ScreenAction::Pop);
        };

        match key.code {
            KeyCode::Up => sftp_state.navigate_up(),
            KeyCode::Down => sftp_state.navigate_down(),
            KeyCode::Enter | KeyCode::Backspace => {
                if let Err(e) = sftp_state.open_selected() {
                    sftp_state.set_status_message(&format!("Error: {}", e));
                }
            }
            KeyCode::Tab => sftp_state.switch_panel(),
            KeyCode::Char('u') => {
                if sftp_state.upload_progress.is_none() {
                    sftp_state.upload_file();
                } else {
                    sftp_state.set_status_message("Upload already in progress");
                }
            }
            KeyCode::Char('d') => {
                if sftp_state.download_progress.is_none() {
                    sftp_state.download_file();
                } else {
                    sftp_state.set_status_message("Download already in progress");
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
        Ok(ScreenAction::None)
    }
}

// -----------------------------------------------------------------------------
// Worker event polling
// -----------------------------------------------------------------------------

fn poll_hosts<B: Backend>(
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<ScreenAction> {
    if let Some(ev) = take_ssh_event(app) {
        return apply_ssh_event_in_hosts(app, ev, terminal);
    }
    if let Some(ev) = take_sftp_event(app) {
        return apply_sftp_event_in_hosts(app, ev);
    }
    Ok(ScreenAction::None)
}

fn poll_sftp(app: &mut App) -> Result<ScreenAction> {
    if let Some(ev) = take_sftp_event(app) {
        match ev {
            SftpEvent::Error(err) => {
                tracing::error!("SFTP error: {}", err);
                app.session.reset();
                app.ui.status_message =
                    Some((format!("SFTP Error: {}", err), Instant::now()));
                return Ok(ScreenAction::Pop);
            }
            SftpEvent::Disconnected => {
                tracing::info!("SFTP session disconnected");
                app.session.reset();
                app.ui.status_message =
                    Some(("SFTP session ended".to_string(), Instant::now()));
                return Ok(ScreenAction::Pop);
            }
            // Bootstrap events shouldn't reach an active SFTP screen.
            _ => {}
        }
    }
    apply_transfer_event(app);
    Ok(ScreenAction::None)
}

fn take_ssh_event(app: &mut App) -> Option<SshEvent> {
    app.session
        .ssh_session_mut()
        .and_then(|s| s.event_rx.try_recv().ok())
}

fn take_sftp_event(app: &mut App) -> Option<SftpEvent> {
    app.session
        .sftp_session_mut()
        .and_then(|s| s.event_rx.try_recv().ok())
}

fn apply_ssh_event_in_hosts<B: Backend>(
    app: &mut App,
    event: SshEvent,
    terminal: &mut Terminal<B>,
) -> Result<ScreenAction> {
    match event {
        SshEvent::Connecting => {
            app.ui.status_message =
                Some(("Testing connection...".to_string(), Instant::now()));
            Ok(ScreenAction::None)
        }
        SshEvent::Connected => {
            app.ui.status_message = Some((
                "Connection successful! Launching SSH...".to_string(),
                Instant::now(),
            ));
            app.transition_to_ssh_mode(terminal)?;
            if let Some(s) = app.session.ssh_session_mut() {
                s.stage = SshStage::Active;
            }
            Ok(ScreenAction::Push(AppScreen::SshActive(SshActiveScreen)))
        }
        SshEvent::Error(err) => {
            tracing::error!("SSH error: {}", err);
            app.ui.status_message = Some((format!("SSH Error: {}", err), Instant::now()));
            app.session.reset();
            Ok(ScreenAction::None)
        }
        SshEvent::Disconnected => {
            tracing::info!("SSH disconnected before reaching Active stage");
            app.session.reset();
            app.ui.status_message = Some(("SSH session ended".to_string(), Instant::now()));
            Ok(ScreenAction::None)
        }
    }
}

fn apply_sftp_event_in_hosts(app: &mut App, event: SftpEvent) -> Result<ScreenAction> {
    match event {
        SftpEvent::PreConnected(sftp_state) => {
            let host_alias = sftp_state.ssh_host.clone();
            if let Some(s) = app.session.sftp_session_mut() {
                s.data = Some(*sftp_state);
            }
            app.ui.status_message = Some((
                format!("SFTP mode active for {}", host_alias),
                Instant::now(),
            ));
            Ok(ScreenAction::Push(AppScreen::Sftp(SftpScreen)))
        }
        SftpEvent::Connecting => {
            app.ui.status_message =
                Some(("Testing connection...".to_string(), Instant::now()));
            Ok(ScreenAction::None)
        }
        SftpEvent::Connected => {
            app.ui.status_message = Some((
                "Connection successful! Launching SFTP...".to_string(),
                Instant::now(),
            ));
            if let Some(s) = app.session.sftp_session_mut() {
                s.stage = SftpStage::Active;
            }
            Ok(ScreenAction::None)
        }
        SftpEvent::Error(err) => {
            tracing::error!("SFTP error: {}", err);
            app.session.reset();
            app.ui.status_message =
                Some((format!("SFTP Error: {}", err), Instant::now()));
            Ok(ScreenAction::None)
        }
        SftpEvent::Disconnected => {
            tracing::info!("SFTP session disconnected");
            app.session.reset();
            app.ui.status_message =
                Some(("SFTP session ended".to_string(), Instant::now()));
            Ok(ScreenAction::None)
        }
    }
}

fn apply_transfer_event(app: &mut App) {
    let Some(session) = app.session.sftp_session_mut() else {
        return;
    };
    let Ok(event) = session.transfer_rx.try_recv() else {
        return;
    };
    let Some(sftp_state) = session.data.as_mut() else {
        return;
    };

    match event {
        TransferEvent::UploadProgress(file_name, uploaded, total) => {
            sftp_state.upload_progress = Some(UploadProgress {
                file_name,
                uploaded_size: uploaded,
                total_size: total,
            });
        }
        TransferEvent::UploadComplete(file_name) => {
            sftp_state.upload_progress = None;
            tracing::info!("Successfully uploaded {}", file_name);
            sftp_state.set_status_message(&format!("Successfully uploaded {}", file_name));
            let _ = sftp_state.refresh_remote();
        }
        TransferEvent::UploadError(file_name, error) => {
            sftp_state.upload_progress = None;
            sftp_state.set_status_message(&format!("Upload failed for {}: {}", file_name, error));
            let _ = sftp_state.refresh_remote();
        }
        TransferEvent::DownloadProgress(file_name, downloaded, total) => {
            sftp_state.download_progress = Some(DownloadProgress {
                file_name,
                downloaded_size: downloaded,
                total_size: total,
            });
        }
        TransferEvent::DownloadComplete(file_name) => {
            sftp_state.download_progress = None;
            sftp_state.set_status_message(&format!("Successfully downloaded {}", file_name));
            let _ = sftp_state.refresh_local();
        }
        TransferEvent::DownloadError(file_name, error) => {
            sftp_state.download_progress = None;
            sftp_state.set_status_message(&format!(
                "Download failed for {}: {}",
                file_name, error
            ));
        }
    }
}

async fn await_ssh_end<B: Backend>(
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<ScreenAction> {
    let event = match app.session.ssh_session_mut() {
        Some(s) => s.event_rx.recv().await,
        None => return Ok(ScreenAction::Pop),
    };
    match event {
        Some(SshEvent::Disconnected) => {
            tracing::info!("SSH session disconnected, restoring TUI");
            app.restore_tui_mode(terminal)?;
            app.session.reset();
            app.ui.status_message =
                Some(("SSH session ended".to_string(), Instant::now()));
            Ok(ScreenAction::Pop)
        }
        Some(SshEvent::Error(err)) => {
            tracing::error!("SSH error: {}", err);
            app.ui.status_message = Some((format!("SSH Error: {}", err), Instant::now()));
            app.session.reset();
            if let Err(e) = app.restore_tui_mode(terminal) {
                tracing::error!("Failed to restore TUI mode after SSH error: {}", e);
            }
            Ok(ScreenAction::Pop)
        }
        Some(_) => Ok(ScreenAction::None),
        None => {
            tracing::info!("SSH event channel closed without explicit Disconnected");
            app.restore_tui_mode(terminal)?;
            app.session.reset();
            Ok(ScreenAction::Pop)
        }
    }
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn host_for_mode(app: &App, mode: HostsMode) -> Option<&SshHost> {
    match mode {
        HostsMode::Search => app
            .search
            .current_host_index()
            .and_then(|i| app.hosts.hosts.get(i)),
        HostsMode::Normal => app.hosts.current_host(),
    }
}

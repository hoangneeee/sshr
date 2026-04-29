use crate::app::session::{SessionState, SftpSession, SftpStage};
use crate::app::App;
use crate::app_event::{SftpEvent, TransferEvent};
use crate::constants::{
    SFTP_EVENT_CHANNEL_BUFFER, SSH_PRE_LAUNCH_DELAY, TRANSFER_CHANNEL_BUFFER,
};
use crate::models::SshHost;
use crate::sftp_logic::AppSftpState;
use crate::sftp_logic::types::{DownloadProgress, UploadProgress};
use crate::ui::hosts_list::draw;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::thread;
use std::time::Instant;
use tokio::sync::mpsc as tokio_mpsc;

impl App {
    pub fn enter_sftp_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let Some(selected_host) = self.get_current_selected_host().cloned() else {
            return Ok(());
        };

        let (sftp_sender, sftp_rx) =
            tokio_mpsc::channel::<SftpEvent>(SFTP_EVENT_CHANNEL_BUFFER);
        let (transfer_sender, transfer_rx) =
            tokio_mpsc::channel::<TransferEvent>(TRANSFER_CHANNEL_BUFFER);

        self.session = SessionState::Sftp(SftpSession {
            stage: SftpStage::Loading,
            event_rx: sftp_rx,
            transfer_rx,
            data: None,
        });
        self.ui.status_message = Some((
            format!("Initializing SFTP for {}...", selected_host.alias),
            Instant::now(),
        ));

        // Worker shells out to ssh (blocking) — run on tokio's blocking pool.
        let host_clone = selected_host.clone();
        let strict_host_key_checking = self.ctx.strict_host_key_checking.clone();
        tokio::task::spawn_blocking(move || {
            Self::sftp_thread_worker(
                sftp_sender,
                host_clone,
                transfer_sender,
                strict_host_key_checking,
            );
        });

        terminal.draw(|f| draw::<B>(f, self))?;
        Ok(())
    }

    pub fn exit_sftp_mode(&mut self) {
        tracing::info!("Exiting SFTP mode");
        self.session.reset();
        self.ui.input_mode = crate::app::InputMode::Normal;
        self.ui.status_message = Some(("Exited SFTP mode".to_string(), Instant::now()));
    }

    pub async fn handle_sftp_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('q') {
            self.exit_sftp_mode();
            return Ok(());
        }

        let Some(sftp_state) = self.session.sftp_data_mut() else {
            return Ok(());
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
        Ok(())
    }

    pub fn process_sftp_events(&mut self) -> Result<bool> {
        let event = self
            .session
            .sftp_session_mut()
            .and_then(|s| s.event_rx.try_recv().ok());

        if let Some(event) = event {
            match event {
                SftpEvent::PreConnected(sftp_state) => {
                    if let Some(s) = self.session.sftp_session_mut() {
                        let host_alias = sftp_state.ssh_host.clone();
                        s.data = Some(*sftp_state);
                        self.ui.input_mode = crate::app::InputMode::Sftp;
                        self.ui.status_message = Some((
                            format!("SFTP mode active for {}", host_alias),
                            Instant::now(),
                        ));
                    }
                    return Ok(true);
                }
                SftpEvent::Connecting => {
                    self.ui.status_message =
                        Some(("Testing connection...".to_string(), Instant::now()));
                    return Ok(false);
                }
                SftpEvent::Connected => {
                    self.ui.status_message = Some((
                        "Connection successful! Launching SFTP...".to_string(),
                        Instant::now(),
                    ));
                    if let Some(s) = self.session.sftp_session_mut() {
                        s.stage = SftpStage::Active;
                    }
                    return Ok(false);
                }
                SftpEvent::Error(err) => {
                    tracing::error!("SFTP error: {}", err);
                    self.session.reset();
                    self.ui.status_message =
                        Some((format!("SFTP Error: {}", err), Instant::now()));
                    return Ok(true);
                }
                SftpEvent::Disconnected => {
                    tracing::info!("SFTP session disconnected, restoring TUI");
                    self.session.reset();
                    self.ui.status_message =
                        Some(("SFTP session ended".to_string(), Instant::now()));
                    return Ok(true);
                }
            }
        }

        if let Some(sftp_state) = self.session.sftp_data() {
            if sftp_state.upload_progress.is_some() || sftp_state.download_progress.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn process_transfer_events(&mut self) -> Result<bool> {
        let Some(session) = self.session.sftp_session_mut() else {
            return Ok(false);
        };
        let Ok(event) = session.transfer_rx.try_recv() else {
            return Ok(false);
        };
        let Some(sftp_state) = session.data.as_mut() else {
            return Ok(false);
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
                sftp_state
                    .set_status_message(&format!("Upload failed for {}: {}", file_name, error));
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
                sftp_state
                    .set_status_message(&format!("Successfully downloaded {}", file_name));
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
        Ok(true)
    }

    fn sftp_thread_worker(
        sender: tokio_mpsc::Sender<SftpEvent>,
        host: SshHost,
        transfer_tx: tokio_mpsc::Sender<TransferEvent>,
        strict_host_key_checking: String,
    ) {
        tracing::info!("SFTP thread started for host: {}", host.alias);

        if sender.blocking_send(SftpEvent::Connecting).is_err() {
            tracing::error!("Failed to send Connecting event");
            return;
        }

        match AppSftpState::new(
            &host.user,
            &host.host,
            host.port.unwrap_or(22),
            transfer_tx,
            &strict_host_key_checking,
        ) {
            Ok(sftp_state) => {
                tracing::info!("SFTP connection test successful for {}", host.alias);

                if sender
                    .blocking_send(SftpEvent::PreConnected(Box::new(sftp_state)))
                    .is_err()
                {
                    tracing::error!("Failed to send PreConnected event");
                    return;
                }
                if sender.blocking_send(SftpEvent::Connected).is_ok() {
                    thread::sleep(SSH_PRE_LAUNCH_DELAY);
                    tracing::info!("Starting SFTP session for {}", host.alias);
                } else {
                    tracing::error!("Failed to send Connected event");
                }
            }
            Err(e) => {
                tracing::error!("SFTP connection test failed for {}: {}", host.alias, e);
                let _ = sender
                    .blocking_send(SftpEvent::Error(format!("Connection test failed: {}", e)));
            }
        }
    }
}

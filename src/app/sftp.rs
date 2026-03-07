use crate::app::App;
use crate::app_event::{SftpEvent, TransferEvent};
use crate::models::SshHost;
use crate::sftp_logic::AppSftpState;
use crate::sftp_logic::types::{DownloadProgress, UploadProgress};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use tokio::sync::mpsc as tokio_mpsc;
use ui::hosts_list::draw;

use crate::ui;

impl App {
    pub fn enter_sftp_mode<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(selected_host) = self.get_current_selected_host().cloned() {
            let (sftp_sender, sftp_receiver) = mpsc::channel::<SftpEvent>();
            self.sftp_receiver = Some(sftp_receiver);

            let (transfer_sender, transfer_receiver) = tokio_mpsc::channel::<TransferEvent>(100);
            self.transfer_receiver = Some(transfer_receiver);

            self.is_sftp_loading = true;
            self.sftp_ready_for_terminal = true;
            self.status_message = Some((
                format!("Initializing SFTP for {}...", selected_host.alias),
                Instant::now(),
            ));

            let host_clone = selected_host.clone();
            let strict_host_key_checking = self.strict_host_key_checking.clone();
            thread::spawn(move || {
                Self::sftp_thread_worker(sftp_sender, host_clone, transfer_sender, strict_host_key_checking);
            });

            terminal.draw(|f| draw::<B>(f, self))?;
        }
        Ok(())
    }

    pub fn exit_sftp_mode(&mut self) {
        tracing::info!("Exiting SFTP mode");
        self.sftp_state = None;
        self.input_mode = crate::app::types::InputMode::Normal;
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
                        sftp_state.set_status_message(&format!("Error: {}", e));
                    }
                }
                KeyCode::Tab => {
                    sftp_state.switch_panel();
                }
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
        }
        Ok(())
    }

    pub fn process_sftp_events<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<bool> {
        if let Some(receiver) = &self.sftp_receiver {
            if let Ok(event) = receiver.try_recv() {
                match event {
                    SftpEvent::PreConnected(sftp_state) => {
                        self.sftp_state = Some(sftp_state);
                        self.input_mode = crate::app::types::InputMode::Sftp;
                        self.status_message = Some((
                            format!(
                                "SFTP mode active for {}",
                                self.sftp_state.as_ref().unwrap().ssh_host
                            ),
                            Instant::now(),
                        ));
                        return Ok(true);
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
                        return Ok(true);
                    }
                    SftpEvent::Disconnected => {
                        tracing::info!("SFTP session disconnected, restoring TUI");

                        self.is_sftp_loading = false;
                        self.sftp_ready_for_terminal = false;
                        self.sftp_receiver = None;
                        self.status_message =
                            Some(("SFTP session ended".to_string(), Instant::now()));
                        return Ok(true);
                    }
                }
            }
        }
        if let Some(sftp_state) = &self.sftp_state {
            if sftp_state.upload_progress.is_some() || sftp_state.download_progress.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn process_transfer_events(&mut self) -> Result<bool> {
        if let Some(receiver) = &mut self.transfer_receiver {
            if let Ok(event) = receiver.try_recv() {
                if let Some(sftp_state) = &mut self.sftp_state {
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
                            tracing::info!("Downloading {}", file_name);
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
                            sftp_state.set_status_message(&format!("Download failed for {}: {}", file_name, error));
                        }
                    }
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn sftp_thread_worker(
        sender: mpsc::Sender<SftpEvent>,
        host: SshHost,
        transfer_tx: tokio_mpsc::Sender<TransferEvent>,
        strict_host_key_checking: String,
    ) {
        tracing::info!("SFTP thread started for host: {}", host.alias);

        if sender.send(SftpEvent::Connecting).is_err() {
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

                if sender.send(SftpEvent::PreConnected(sftp_state)).is_err() {
                    tracing::error!("Failed to send PreConnected event");
                    return;
                }
                if sender.send(SftpEvent::Connected).is_ok() {
                    thread::sleep(std::time::Duration::from_millis(200));
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
    }
}

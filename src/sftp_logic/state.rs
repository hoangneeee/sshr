use super::types::{AppSftpState, PanelSide};
use crate::app_event::TransferEvent;
use crate::ssh_client::{ConnectOpts, SftpClient};
use anyhow::{Context, Result};
use ratatui::widgets::ListState;
use std::sync::Arc;
use tokio::sync::mpsc;

impl AppSftpState {
    /// Open a new SFTP session for `host` and populate the initial local
    /// + remote directory listings.
    ///
    /// Auth is attempted in order: ssh-agent → ~/.ssh keys → `password` (if
    /// `Some`). Returns the underlying `SshClientError` so the caller can
    /// distinguish auth-needed-password from other failures.
    pub async fn new(
        ssh_user: &str,
        ssh_host: &str,
        ssh_port: u16,
        transfer_tx: mpsc::Sender<TransferEvent>,
        strict_host_key_checking: &str,
        password: Option<String>,
    ) -> Result<Self, crate::ssh_client::SshClientError> {
        let opts = ConnectOpts {
            user: ssh_user.to_string(),
            host: ssh_host.to_string(),
            port: ssh_port,
            strict_host_key_checking: strict_host_key_checking.to_string(),
            password,
        };
        let client = Arc::new(SftpClient::connect(&opts).await?);

        let current_dir = std::env::current_dir()
            .context("Failed to get current directory")
            .map_err(crate::ssh_client::SshClientError::Other)?;

        let mut state = Self {
            active_panel: PanelSide::Local,
            local_current_path: current_dir,
            local_files: Vec::new(),
            local_selected: 0,
            local_list_state: ListState::default(),

            remote_current_path: "/".to_string(),
            remote_files: Vec::new(),
            remote_selected: 0,
            remote_list_state: ListState::default(),

            ssh_host: ssh_host.to_string(),
            status_message: None,
            status_message_time: None,
            upload_progress: None,
            download_progress: None,
            transfer_tx,
            client,
        };

        state
            .refresh_local()
            .map_err(crate::ssh_client::SshClientError::Other)?;
        state
            .refresh_remote()
            .await
            .map_err(crate::ssh_client::SshClientError::Other)?;

        Ok(state)
    }

    pub fn set_status_message(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
        self.status_message_time = Some(std::time::Instant::now());
    }

    pub fn clear_status_message(&mut self) {
        self.status_message = None;
        self.status_message_time = None;
    }

    pub fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            PanelSide::Local => PanelSide::Remote,
            PanelSide::Remote => PanelSide::Local,
        };
    }

    pub fn navigate_up(&mut self) {
        match self.active_panel {
            PanelSide::Local => self.navigate_local_up(),
            PanelSide::Remote => self.navigate_remote_up(),
        };
    }

    pub fn navigate_down(&mut self) {
        match self.active_panel {
            PanelSide::Local => self.navigate_local_down(),
            PanelSide::Remote => self.navigate_remote_down(),
        };
    }

    pub async fn open_selected(&mut self) -> Result<()> {
        match self.active_panel {
            PanelSide::Local => {
                let _ = self.open_local_selected();
            }
            PanelSide::Remote => {
                let _ = self.open_remote_selected().await;
            }
        };
        Ok(())
    }
}

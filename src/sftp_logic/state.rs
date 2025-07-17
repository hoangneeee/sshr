use super::types::{AppSftpState, PanelSide, UploadProgress, DownloadProgress};
use anyhow::{Context, Result};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;
use crate::app_event::TransferEvent;

impl AppSftpState {
    /// Create a new instance of AppSftpState
    pub fn new(
        ssh_user: &str,
        ssh_host: &str,
        ssh_port: u16,
        transfer_tx: mpsc::Sender<TransferEvent>,
    ) -> Result<Self> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;

        let mut state = Self {
            active_panel: PanelSide::Local,
            // LOCAL
            local_current_path: current_dir,
            local_files: Vec::new(),
            local_selected: 0,
            local_list_state: ListState::default(),

            // REMOTE
            remote_current_path: "/".to_string(),
            remote_files: Vec::new(),
            remote_selected: 0,
            remote_list_state: ListState::default(),

            ssh_host: ssh_host.to_string(),
            ssh_user: ssh_user.to_string(),
            ssh_port,
            status_message: None,
            status_message_time: None,
            upload_progress: None,
            download_progress: None,
            transfer_tx: Some(transfer_tx),
        };

        // Load initial directory contents
        state.refresh_local()?;
        state.refresh_remote()?;

        Ok(state)
    }


    /// Set a status message to be displayed to the user
    pub fn set_status_message(&mut self, message: &str) {
        self.status_message = Some(message.to_string());
        self.status_message_time = Some(std::time::Instant::now());
    }

    /// Clear the current status message
    pub fn clear_status_message(&mut self) {
        self.status_message = None;
        self.status_message_time = None;
    }

    /// Switch the active panel between local and remote
    pub fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            PanelSide::Local => PanelSide::Remote,
            PanelSide::Remote => PanelSide::Local,
        };
    }

    pub fn navigate_up(&mut self) {
        match self.active_panel {
            PanelSide::Local => {
                self.navigate_local_up();
            }
            PanelSide::Remote => {
                self.navigate_remote_up();
            }
        };
    }

    pub fn navigate_down(&mut self) {
        match self.active_panel {
            PanelSide::Local => {
                self.navigate_local_down();
            }
            PanelSide::Remote => {
                self.navigate_remote_down();
            }
        };
    }

    pub fn open_selected(&mut self) -> Result<()> {
        match self.active_panel {
            PanelSide::Local => {
                let _ = self.open_local_selected();
            }
            PanelSide::Remote => {
                let _ = self.open_remote_selected();
            }
        };
        Ok(())
    }

}

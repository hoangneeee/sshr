use crate::app_event::TransferEvent;
use crate::ssh_client::SftpClient;
use ratatui::widgets::ListState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

/// Represents a file or directory item in the file browser
#[derive(Debug, Clone)]
pub enum FileItem {
    Directory { name: String },
    File { name: String, size: u64 },
}

/// Represents which panel (local or remote) is currently active
#[derive(Debug, Clone, PartialEq)]
pub enum PanelSide {
    Local,
    Remote,
}

#[derive(Debug, Clone)]
pub struct UploadProgress {
    pub file_name: String,
    pub uploaded_size: u64,
    pub total_size: u64,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub file_name: String,
    pub downloaded_size: u64,
    pub total_size: u64,
}

/// Main application state for the SFTP file browser.
///
/// Holds the persistent SFTP client connection plus local/remote panel
/// state. Cloning is intentionally not derived — the `client` is shared
/// across spawned tasks via Arc.
#[derive(Debug)]
pub struct AppSftpState {
    pub active_panel: PanelSide,

    // Local panel state
    pub local_current_path: PathBuf,
    pub local_files: Vec<FileItem>,
    pub local_selected: usize,
    pub local_list_state: ListState,

    // Remote panel state
    pub remote_current_path: String,
    pub remote_files: Vec<FileItem>,
    pub remote_selected: usize,
    pub remote_list_state: ListState,

    /// Used in status messages and the SFTP overlay title.
    pub ssh_host: String,

    // UI state
    pub status_message: Option<String>,
    pub status_message_time: Option<Instant>,

    // Transfer progress
    pub upload_progress: Option<UploadProgress>,
    pub download_progress: Option<DownloadProgress>,

    // Channel for transfer worker tasks to post progress back to the UI.
    pub transfer_tx: mpsc::Sender<TransferEvent>,

    /// Persistent SSH+SFTP session, shared with spawned upload/download tasks.
    pub client: Arc<SftpClient>,
}

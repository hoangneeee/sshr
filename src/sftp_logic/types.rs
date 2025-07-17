use std::path::PathBuf;
use std::time::Instant;
use ratatui::widgets::ListState;
use tokio::sync::mpsc;
use crate::app_event::TransferEvent;

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

/// Represents upload progress information
#[derive(Debug, Clone)]
pub struct UploadProgress {
    pub file_name: String,
    pub uploaded_size: u64,
    pub total_size: u64,
}

/// Represents download progress information
#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub file_name: String,
    pub downloaded_size: u64,
    pub total_size: u64,
}

/// Main application state for the SFTP file browser
#[derive(Debug, Clone)]
pub struct AppSftpState {
    /// Currently active panel (local or remote)
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
    
    // SFTP connection info
    pub ssh_host: String,
    pub ssh_user: String,
    pub ssh_port: u16,
    
    // UI state
    pub status_message: Option<String>,
    pub status_message_time: Option<Instant>,
    
    // Upload progress
    pub upload_progress: Option<UploadProgress>,
    // Download progress
    pub download_progress: Option<DownloadProgress>,

    // Transfer event sender
    pub transfer_tx: Option<mpsc::Sender<TransferEvent>>,
}

impl FileItem {
    pub fn name(&self) -> &str {
        match self {
            FileItem::Directory { name } => name,
            FileItem::File { name, .. } => name,
        }
    }
}

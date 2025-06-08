use std::path::PathBuf;
use std::time::Instant;
use ratatui::widgets::ListState;

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
}

use std::path::PathBuf;

use crate::sftp::AppSftpState;
use crate::{config::ConfigManager, models::SshHost};

use crate::events::{SftpEvent, SshEvent, TransferEvent};
use ratatui::widgets::ListState;
use std::sync::mpsc::Receiver;
use tokio::sync::mpsc as tokio_mpsc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivePanel {
    Groups,
    Hosts,
}

#[derive(Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Sftp,
}

#[derive(Debug, Clone)]
pub struct FilteredHost {
    pub original_index: usize,
    pub score: i64,
    pub matched_indices: Vec<usize>,
}

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub hosts: Vec<SshHost>,
    pub selected_host: usize,
    pub selected_group: usize,
    pub active_panel: ActivePanel,
    pub ssh_config_path: PathBuf,
    pub config_manager: ConfigManager,
    pub input_mode: InputMode,
    pub show_help: bool,
    pub help_scroll_position: u16,

    pub status_message: Option<(String, std::time::Instant)>,

    // SSH Mode
    pub is_connecting: bool,
    pub connecting_host: Option<SshHost>,
    pub ssh_ready_for_terminal: bool,
    pub ssh_receiver: Option<Receiver<SshEvent>>,

    // SFTP Mode
    pub is_sftp_loading: bool,
    pub sftp_ready_for_terminal: bool,
    pub sftp_receiver: Option<Receiver<SftpEvent>>,
    pub sftp_state: Option<AppSftpState>,
    pub transfer_receiver: Option<tokio_mpsc::Receiver<TransferEvent>>,

    // Search Mode
    pub search_query: String,
    pub filtered_hosts: Vec<FilteredHost>, // Indices of filtered hosts
    pub search_selected: usize,

    // Group State
    pub groups: Vec<String>,
    pub hosts_in_current_group: Vec<usize>,


    pub host_list_state: ListState,
    pub group_list_state: ListState,
}

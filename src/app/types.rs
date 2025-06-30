use std::path::PathBuf;

use crate::sftp_logic::AppSftpState;
use crate::{config::ConfigManager, models::SshHost};

use crate::app_event::{SftpEvent, SshEvent};
use ratatui::{widgets::ListState};
use std::sync::mpsc::{Receiver};


#[derive(Debug)]
pub enum InputMode {
    Normal,
    Search,
    Sftp,
}

#[derive(Debug)]
pub struct App {
    pub should_quit: bool,
    pub hosts: Vec<SshHost>,
    pub selected: usize,
    pub ssh_config_path: PathBuf,
    pub config_manager: ConfigManager,
    pub input_mode: InputMode,

    pub status_message: Option<(String, std::time::Instant)>,

    // SSH Mode
    pub is_connecting: bool,
    pub ssh_ready_for_terminal: bool,
    pub ssh_receiver: Option<Receiver<SshEvent>>,

    // SFTP Mode
    pub is_sftp_loading: bool,
    pub sftp_ready_for_terminal: bool,
    pub sftp_receiver: Option<Receiver<SftpEvent>>,
    pub sftp_state: Option<AppSftpState>,

    // Search Mode
    pub search_query: String,
    pub filtered_hosts: Vec<usize>, // Indices of filtered hosts
    pub search_selected: usize,

    // Group State
    pub collapsed_groups: std::collections::HashSet<String>,
    pub current_group_index: usize,
    pub groups: Vec<String>,

    pub host_list_state: ListState,
}

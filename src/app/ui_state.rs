use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    Sftp,
}

/// Transient UI state: input mode and ephemeral status banner.
#[derive(Debug)]
pub struct UiState {
    pub input_mode: InputMode,
    pub status_message: Option<(String, Instant)>,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

impl UiState {
    pub fn new() -> Self {
        Self {
            input_mode: InputMode::Normal,
            status_message: None,
        }
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}

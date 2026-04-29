use std::time::Instant;

/// Transient UI state shared across screens (currently just the status banner).
#[derive(Debug)]
pub struct UiState {
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
            status_message: None,
        }
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}
